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
use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU8, Ordering};

use cmdmx_desk_logic::boxmap::{self, FADERS, KEYS, WHEEL};
use cmdmx_desk_logic::encoder::{Accumulator, Quadrature};
use cmdmx_desk_logic::fader::{adc12_to_14, Fader, DEFAULT_DEADBAND};
use cmdmx_desk_logic::keys::Debounce;
use cmdmx_desk_logic::link::Link;
use cmdmx_desk_logic::midi::{
    cc14_packets, cc_packet, note_packet, parse_cc, parse_feedback, Cc14Rx,
};
use cmdmx_desk_logic::motor::{Drive, Motor, MotorConfig};
use cmdmx_desk_logic::palette::{colour, Rgb, DISCONNECTED};
use cmdmx_desk_logic::touch::Touch;
use cmdmx_desk_logic::wiremap::{cc, note};
use embassy_executor::Spawner;
use embassy_futures::join::join4;
use embassy_futures::poll_once;
use embassy_futures::select::select;
use embassy_rp::adc::{Adc, Channel, Config as AdcConfig};
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Flex, Input, Level, Output, Pull};
use embassy_rp::peripherals::{DMA_CH0, PIO0, USB};
use embassy_rp::pio::{InterruptHandler as PioIrq, Pio};
use embassy_rp::pio_programs::ws2812::{PioWs2812, PioWs2812Program};
use embassy_rp::pwm::{Config as PwmConfig, Pwm};
use embassy_rp::usb::{Driver, InterruptHandler as UsbIrq};
use embassy_rp::watchdog::Watchdog;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_time::{with_timeout, Duration, Instant, Ticker};
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
//   GP10 PWMA, GP11 PWMB  motor 1 / 2 speed (TB6612FNG, PWM slice 5 A/B, 20 kHz)
//   GP12 AIN1, GP13 AIN2  motor 1 direction    GP14 BIN1, GP15 BIN2  motor 2 direction
//   GP9  STBY            TB6612FNG standby, high while running
//   GP20 / GP21  touch track of fader 1 / 2, each with 1 MOhm to 3V3 (charge-time sensing)
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
/// Motor stops within this many 14-bit steps of the target (HW-16).
/// ponytail: a guess, tune on the bench; too small and the motor hunts around the target.
const MOTOR_TOLERANCE: u16 = 40;
/// Drive in permille duty per 14-bit step of error (P gain).
/// ponytail: a guess, tune on the bench; 2 means full drive from 500 steps away.
const MOTOR_KP: u16 = 2;
/// Lowest duty in permille that still moves the fader (stiction).
/// ponytail: a guess, tune on the bench with the real motor and supply.
const MOTOR_MIN_DUTY: u16 = 300;
/// Highest duty in permille; lower it if the fader hits the end stops too hard.
const MOTOR_MAX_DUTY: u16 = 1000;
/// A move that has not arrived after this long is abandoned (HW-17).
const MOTOR_TIMEOUT: u32 = 1000;
/// Charge-time increase (in `read_touch` loop counts) that counts as a touch.
/// ponytail: a guess, read the untouched and touched counts on the bench and set it between them.
const TOUCH_THRESHOLD: u16 = 20;
/// Longest wait for a touch track to charge, in loop counts (about 200 us), so a scan never blocks long.
const TOUCH_CAP: u16 = 3000;
/// PWM wrap value: 150 MHz / (7499 + 1) = 20 kHz, above hearing.
const PWM_TOP: u16 = 7499;

const MOTOR: MotorConfig = MotorConfig {
    tolerance: MOTOR_TOLERANCE,
    kp_permille_per_step: MOTOR_KP,
    min_duty: MOTOR_MIN_DUTY,
    max_duty: MOTOR_MAX_DUTY,
    timeout_ms: MOTOR_TIMEOUT,
};

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

/// Last motor target from the host per fader (14 bit) and whether it is new (HW-16).
static TARGET: [(AtomicBool, AtomicU16); 2] = [
    (AtomicBool::new(false), AtomicU16::new(0)),
    (AtomicBool::new(false), AtomicU16::new(0)),
];

/// Everything one motorised fader needs.
struct Slot<'d> {
    fader: Fader,
    adc: Channel<'d>,
    motor: Motor,
    touch: Touch,
    touch_pin: Flex<'d>,
    in1: Output<'d>,
    in2: Output<'d>,
    /// The motor was driving on the previous scan.
    was_moving: bool,
}

/// Charge time of a touch track: discharge it, release it and count until the 1 MOhm pull-up has
/// raised it to a logic high (bounded by `TOUCH_CAP`).
fn read_touch(pin: &mut Flex<'_>) -> u16 {
    pin.set_low();
    pin.set_as_output();
    cortex_m::asm::delay(600); // about 4 us at 150 MHz
    pin.set_as_input();
    let mut n = 0;
    while n < TOUCH_CAP && !pin.is_high() {
        n += 1;
    }
    n
}

impl Slot<'_> {
    /// No target, motor off (HW-32: the fader stays where it is).
    fn stop(&mut self) {
        self.motor.set_touched(true);
        self.motor.set_touched(false);
    }

    fn set_bridge(&mut self, drive: Drive) -> u16 {
        let (a, b, duty) = match drive {
            Drive::Coast => (false, false, 0),
            Drive::Up(d) => (true, false, d),
            Drive::Down(d) => (false, true, d),
        };
        self.in1.set_level(a.into());
        self.in2.set_level(b.into());
        u16::try_from(u32::from(duty.min(1000)) * (u32::from(PWM_TOP) + 1) / 1000).unwrap_or(0)
    }
}

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
    let mut stby = Output::new(p.PIN_9, Level::High);
    stby.set_high();
    let mut pwm_cfg = PwmConfig::default();
    pwm_cfg.top = PWM_TOP;
    pwm_cfg.compare_a = 0;
    pwm_cfg.compare_b = 0;
    let mut pwm = Pwm::new_output_ab(p.PWM_SLICE5, p.PIN_10, p.PIN_11, pwm_cfg.clone());
    let new_touch_pin = |pin: Flex<'static>| {
        let mut pin = pin;
        pin.set_pull(Pull::None);
        pin
    };
    let mut slots = [
        Slot {
            fader: Fader::new(DEADBAND, SETTLE_SAMPLES),
            adc: Channel::new_pin(p.PIN_26, Pull::None),
            motor: Motor::new(MOTOR),
            touch: Touch::new(TOUCH_THRESHOLD),
            touch_pin: new_touch_pin(Flex::new(p.PIN_20)),
            in1: Output::new(p.PIN_12, Level::Low),
            in2: Output::new(p.PIN_13, Level::Low),
            was_moving: false,
        },
        Slot {
            fader: Fader::new(DEADBAND, SETTLE_SAMPLES),
            adc: Channel::new_pin(p.PIN_27, Pull::None),
            motor: Motor::new(MOTOR),
            touch: Touch::new(TOUCH_THRESHOLD),
            touch_pin: new_touch_pin(Flex::new(p.PIN_21)),
            in1: Output::new(p.PIN_14, Level::Low),
            in2: Output::new(p.PIN_15, Level::Low),
            was_moving: false,
        },
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
        let mut cc14 = Cc14Rx::new();
        loop {
            receiver.wait_connection().await;
            while let Ok(n) = receiver.read_packet(&mut buf).await {
                for pkt in buf.get(..n).unwrap_or(&[]).as_chunks::<4>().0 {
                    // Motor targets: fader CC pairs from the host (HW-16).
                    if let Some((c, v)) = parse_cc(pkt) {
                        if let Some((msb_cc, value)) = cc14.feed(c, v) {
                            for (ctl, (new, target)) in FADERS.into_iter().zip(&TARGET) {
                                if cc(ctl) == Some(msb_cc) {
                                    target.store(value, Ordering::Relaxed);
                                    new.store(true, Ordering::Relaxed);
                                }
                            }
                        }
                        continue;
                    }
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
                slots.iter_mut().for_each(|s| s.fader.reset());
                for v in &VELOCITY {
                    v.store(0, Ordering::Relaxed);
                }
                FEEDBACK.store(false, Ordering::Relaxed);
                link.set_configured(true);
            }
            // No host, no target: motors stay where they are (HW-32); never driven before the
            // first target arrives (HW-18).
            if !connected {
                for (slot, (new, _)) in slots.iter_mut().zip(&TARGET) {
                    new.store(false, Ordering::Relaxed);
                    slot.stop();
                }
            }
            if FEEDBACK.swap(false, Ordering::Relaxed) {
                link.feedback_received();
            }

            let mut out = Packets {
                buf: [0; 64],
                len: 0,
            };
            let now = Instant::now().as_millis() as u32; // wraps; the motor timer is wrap-safe
            let mut compare = [0u16; 2];
            for (((slot, ctl), (new, target)), duty) in slots
                .iter_mut()
                .zip(FADERS)
                .zip(&TARGET)
                .zip(compare.iter_mut())
            {
                let touched = slot.touch.update(read_touch(&mut slot.touch_pin));
                slot.motor.set_touched(touched);
                if new.swap(false, Ordering::Relaxed) {
                    slot.motor.set_target(target.load(Ordering::Relaxed), now);
                }
                let raw = read_fader(&mut adc, &mut slot.adc);
                let drive = slot.motor.update(raw, now);
                *duty = slot.set_bridge(drive);
                // A stall is reported on the strip's status console (HW-17), not on USB-MIDI.
                let stalled = slot.motor.take_stall();
                if slot.motor.moving() {
                    slot.was_moving = true;
                    continue; // the host must not hear the motor's own move
                }
                if slot.was_moving && !touched && !stalled {
                    slot.fader.follow(raw); // arrived: no echo (HW-14)
                }
                slot.was_moving = false;
                if let (Some(v), Some(msb)) = (slot.fader.update(raw), cc(ctl)) {
                    let [a, b] = cc14_packets(msb, v);
                    out.push(&a);
                    out.push(&b);
                }
            }
            pwm_cfg.compare_a = compare[0];
            pwm_cfg.compare_b = compare[1];
            pwm.set_config(&pwm_cfg);
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
