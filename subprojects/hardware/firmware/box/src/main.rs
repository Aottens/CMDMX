//! USB-MIDI box proof of concept on a Raspberry Pi Pico 2 (HWB-01, HW-61): two faders, four
//! keys with RGB LEDs and one encoder, speaking the wire map of `wire-map.md`.
#![no_std]
#![no_main]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use core::cell::Cell;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use cmdmx_desk_logic::boxmap::{self, FADERS, KEYS, WHEEL};
use cmdmx_desk_logic::encoder::{Accumulator, Quadrature};
use cmdmx_desk_logic::fader::{adc12_to_14, Fader, DEFAULT_DEADBAND};
use cmdmx_desk_logic::keys::Debounce;
use cmdmx_desk_logic::link::Link;
use cmdmx_desk_logic::midi::{cc14_packets, cc_packet, note_packet, parse_feedback};
use cmdmx_desk_logic::palette::{colour, Rgb, DISCONNECTED};
use cmdmx_desk_logic::wiremap::{cc, note};
use embassy_executor::Spawner;
use embassy_futures::join::join4;
use embassy_futures::poll_once;
use embassy_futures::select::select;
use embassy_rp::adc::{Adc, Channel, Config as AdcConfig};
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::peripherals::{DMA_CH0, PIO0, USB};
use embassy_rp::pio::{InterruptHandler as PioIrq, Pio};
use embassy_rp::pio_programs::ws2812::{PioWs2812, PioWs2812Program};
use embassy_rp::usb::{Driver, InterruptHandler as UsbIrq};
use embassy_rp::watchdog::Watchdog;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_time::{with_timeout, Duration, Ticker};
use embassy_usb::class::midi::MidiClass;
use embassy_usb::{Builder, Config};
use panic_halt as _;
use smart_leds::RGB8;

// Pins (Pico 2, RP2350A):
//   GP26 / ADC0  fader 1 wiper      GP2  key 1 (upper 1) to GND, pull-up
//   GP27 / ADC1  fader 2 wiper      GP3  key 2 (upper 2) to GND, pull-up
//   GP6          encoder A, pull-up GP4  key 3 (lower 1) to GND, pull-up
//   GP7          encoder B, pull-up GP5  key 4 (lower 2) to GND, pull-up
//   GP16         SK6812 data (PIO0), 4 LEDs in key order
// Peripherals are taken by name below (`p.PIN_26` ...); keep this table in step with them.

/// Fader deadband in 14-bit steps (HW-14).
const DEADBAND: u16 = DEFAULT_DEADBAND;
/// Samples averaged after boot / new host before the position is sent (HW-18).
const SETTLE_SAMPLES: u16 = 8;
/// ADC reads averaged into one fader sample.
const ADC_OVERSAMPLE: u32 = 16;
const SCAN_PERIOD: Duration = Duration::from_millis(1);
/// Consecutive scans a key must read the same before it counts.
const DEBOUNCE_SCANS: u16 = 5;
/// Quadrature counts per wheel step (HW-22).
const ENCODER_DIVIDER: u16 = 4;
/// LED brightness of 255; the palette is scaled by it.
const BRIGHTNESS: u16 = 64;
/// Reset if the scan loop does not run for this long (HW-30).
const WATCHDOG_TIMEOUT: Duration = Duration::from_millis(500);
/// A host that does not take a packet within this long counts as gone.
const WRITE_TIMEOUT: Duration = Duration::from_millis(100);

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => UsbIrq<USB>;
    PIO0_IRQ_0 => PioIrq<PIO0>;
    DMA_IRQ_0 => embassy_rp::dma::InterruptHandler<DMA_CH0>;
});

/// Quadrature counts since the main loop last looked.
static ENCODER: Mutex<CriticalSectionRawMutex, Cell<i32>> = Mutex::new(Cell::new(0));
/// Last feedback velocity per key, in `KEYS` order.
static VELOCITY: [AtomicU8; 4] = [
    AtomicU8::new(0),
    AtomicU8::new(0),
    AtomicU8::new(0),
    AtomicU8::new(0),
];
/// Set by the receiver on every feedback message.
static FEEDBACK: AtomicBool = AtomicBool::new(false);

/// Up to 64 bytes of USB-MIDI packets, sent in one transfer.
struct Packets {
    buf: [u8; 64],
    len: usize,
}

impl Packets {
    fn push(&mut self, p: &[u8; 4]) {
        if let Some(dst) = self.buf.get_mut(self.len..self.len + 4) {
            dst.copy_from_slice(p);
            self.len += 4;
        }
    }
}

fn scaled(c: Rgb) -> RGB8 {
    let s = |v: u8| u8::try_from(u16::from(v) * BRIGHTNESS / 255).unwrap_or(u8::MAX);
    RGB8 {
        r: s(c.r),
        g: s(c.g),
        b: s(c.b),
    }
}

/// One fader sample: `ADC_OVERSAMPLE` blocking reads averaged, as 14 bit.
fn read_fader(adc: &mut Adc<'_, embassy_rp::adc::Blocking>, ch: &mut Channel<'_>) -> u16 {
    let mut sum = 0u32;
    for _ in 0..ADC_OVERSAMPLE {
        sum += u32::from(adc.blocking_read(ch).unwrap_or(0));
    }
    adc12_to_14(u16::try_from(sum / ADC_OVERSAMPLE).unwrap_or(4095))
}

#[embassy_executor::main(
    executor = "embassy_rp::executor::Executor",
    entry = "cortex_m_rt::entry"
)]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let mut watchdog = Watchdog::new(p.WATCHDOG);
    watchdog.start(WATCHDOG_TIMEOUT);

    let mut adc = Adc::new_blocking(p.ADC, AdcConfig::default());
    let mut channels = [
        Channel::new_pin(p.PIN_26, Pull::None),
        Channel::new_pin(p.PIN_27, Pull::None),
    ];
    let mut faders = [
        Fader::new(DEADBAND, SETTLE_SAMPLES),
        Fader::new(DEADBAND, SETTLE_SAMPLES),
    ];

    let keys = [
        Input::new(p.PIN_2, Pull::Up),
        Input::new(p.PIN_3, Pull::Up),
        Input::new(p.PIN_4, Pull::Up),
        Input::new(p.PIN_5, Pull::Up),
    ];
    let mut debounce = [
        Debounce::new(DEBOUNCE_SCANS),
        Debounce::new(DEBOUNCE_SCANS),
        Debounce::new(DEBOUNCE_SCANS),
        Debounce::new(DEBOUNCE_SCANS),
    ];
    let mut wheel = Accumulator::new(ENCODER_DIVIDER);
    let mut enc_a = Input::new(p.PIN_6, Pull::Up);
    let mut enc_b = Input::new(p.PIN_7, Pull::Up);

    let Pio {
        mut common, sm0, ..
    } = Pio::new(p.PIO0, Irqs);
    let program = PioWs2812Program::new(&mut common);
    let mut leds: PioWs2812<'_, PIO0, 0, 4, _> =
        PioWs2812::new(&mut common, sm0, p.DMA_CH0, Irqs, p.PIN_16, &program);

    // USB: pid.codes test PID until the project has its own (wire-map.md).
    let mut config = Config::new(0x1209, 0x0001);
    config.manufacturer = Some("Colourmotion");
    config.product = Some("CMDMX Desk");
    config.max_power = 100;
    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    let mut control_buf = [0; 64];
    let mut builder = Builder::new(
        Driver::new(p.USB, Irqs),
        config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut [],
        &mut control_buf,
    );
    let class = MidiClass::new(&mut builder, 1, 1, 64);
    let mut usb = builder.build();
    let (mut sender, mut receiver) = class.split();

    // Encoder: count every edge of A and B (HW-22).
    let encoder = async {
        let mut quad = Quadrature::new(enc_a.is_high(), enc_b.is_high());
        loop {
            select(enc_a.wait_for_any_edge(), enc_b.wait_for_any_edge()).await;
            let d = i32::from(quad.update(enc_a.is_high(), enc_b.is_high()));
            ENCODER.lock(|c| c.set(c.get().saturating_add(d)));
        }
    };

    // Feedback from the host: key LED velocities (CTL-17).
    let rx = async {
        let mut buf = [0u8; 64];
        loop {
            receiver.wait_connection().await;
            while let Ok(n) = receiver.read_packet(&mut buf).await {
                for pkt in buf.get(..n).unwrap_or(&[]).as_chunks::<4>().0 {
                    let Some((note, vel)) = parse_feedback(pkt) else {
                        continue;
                    };
                    if let Some(slot) = boxmap::key_for_note(note).and_then(|i| VELOCITY.get(i)) {
                        slot.store(vel, Ordering::Relaxed);
                    }
                    FEEDBACK.store(true, Ordering::Relaxed);
                }
            }
        }
    };

    // Scan loop: the only place that feeds the watchdog (HW-30).
    let scan = async {
        let mut ticker = Ticker::every(SCAN_PERIOD);
        let mut link = Link::new();
        let mut connected = false;
        let mut shown: Option<[Rgb; 4]> = None;
        loop {
            ticker.next().await;
            watchdog.feed(WATCHDOG_TIMEOUT);

            if !connected && poll_once(sender.wait_connection()).is_ready() {
                // New host: send the settled fader positions again (HW-18).
                connected = true;
                faders.iter_mut().for_each(Fader::reset);
                for v in &VELOCITY {
                    v.store(0, Ordering::Relaxed);
                }
                FEEDBACK.store(false, Ordering::Relaxed);
                link.set_configured(true);
            }
            if FEEDBACK.swap(false, Ordering::Relaxed) {
                link.feedback_received();
            }

            let mut out = Packets {
                buf: [0; 64],
                len: 0,
            };
            for ((fader, ch), ctl) in faders.iter_mut().zip(channels.iter_mut()).zip(FADERS) {
                let raw = read_fader(&mut adc, ch);
                if let (Some(v), Some(msb)) = (fader.update(raw), cc(ctl)) {
                    let [a, b] = cc14_packets(msb, v);
                    out.push(&a);
                    out.push(&b);
                }
            }
            for ((key, deb), ctl) in keys.iter().zip(debounce.iter_mut()).zip(KEYS) {
                if let (Some(on), Some(n)) = (deb.update(key.is_low()), note(ctl)) {
                    out.push(&note_packet(n, on));
                }
            }
            wheel.add(ENCODER.lock(|c| c.replace(0)));
            if let (Some(steps), Some(n)) = (wheel.take(), cc(WHEEL)) {
                out.push(&cc_packet(n, steps));
            }

            if connected && out.len > 0 {
                let data = out.buf.get(..out.len).unwrap_or(&[]);
                if !matches!(
                    with_timeout(WRITE_TIMEOUT, sender.write_packet(data)).await,
                    Ok(Ok(()))
                ) {
                    connected = false;
                    link.set_configured(false);
                }
            }

            let want = if link.disconnected() {
                [DISCONNECTED; 4]
            } else {
                let mut c = [Rgb { r: 0, g: 0, b: 0 }; 4];
                for (c, v) in c.iter_mut().zip(&VELOCITY) {
                    *c = colour(v.load(Ordering::Relaxed));
                }
                c
            };
            if shown != Some(want) {
                leds.write(&want.map(scaled)).await;
                shown = Some(want);
            }
        }
    };

    join4(usb.run(), encoder, rx, scan).await;
}
