# USB-MIDI box checklist — CMDMX Desk on a Pico 2 (HWB-01, HW-14, 18, 20, 22, 30, 32, 61)

Gate 3: only the owner ticks these. Copy to `tests/hardware/runs/<date>-HWB-01.md` for a run.
Firmware: `subprojects/hardware/firmware/` (`box/`), wire map
[`wire-map.md`](../../subprojects/hardware/firmware/wire-map.md). The box needs no software
change: it is learned on the Mac (CTL-25).

## Parts to order (none on the desk on 2026-10-25)
Quality before price (HW-03); part numbers are suggestions, check the datasheet before ordering.
- [ ] 1 × **Raspberry Pi Pico 2** (RP2350A) with headers (or solder them), and a USB data cable
      (micro-USB to the Mac; a charge-only cable will not enumerate)
- [ ] 2 × **plain 100 mm linear slide potentiometer, 10 kΩ linear (B10K)**, e.g. ALPS RSA0N
      series or Bourns PSL series, with two fader knobs
- [ ] 1 × **optical incremental encoder, smooth (no detents), 5 V, open-collector or push-pull
      quadrature A/B**, e.g. Bourns ENA1J-B28 (128 PPR), with a knob. A mechanical EC11 works for
      a first try (the firmware does not care), but the desk wants optical (HW-07)
- [ ] 4 × **MX-compatible switch** (e.g. Cherry MX or Gateron, any feel) with clear caps
- [ ] 4 × **SK6812 RGB LED** in a chain: a strip cut to 4 LEDs (5 V, 60/m) or 4 breakout boards
- [ ] 1 × **74AHCT125** (or 74HCT245) level shifter for the LED data line (3.3 V → 5 V), plus a
      330 Ω resistor in the data line and a 470 µF capacitor across the LED supply
- [ ] Breadboard, jumper wires, 2 × 2.2 kΩ pull-ups if the encoder datasheet asks for them

## Wiring (pin table at the top of `box/src/main.rs`)
| Pico 2 pin | Connects to |
| --- | --- |
| GP26 (ADC0) | fader 1 wiper; fader ends to 3V3 (pin 36) and AGND (pin 33) |
| GP27 (ADC1) | fader 2 wiper; ends like fader 1 |
| GP2, GP3, GP4, GP5 | key 1 (`upper 1`), key 2 (`upper 2`), key 3 (`lower 1`), key 4 (`lower 2`), other side to GND |
| GP6, GP7 | encoder A, B (encoder at 5 V from VBUS pin 40, its outputs pulled up to **3V3**, never to 5 V) |
| GP16 | 74AHCT125 input; its output through 330 Ω to the first SK6812 DIN; LEDs in key order 1–4 |
| VBUS (pin 40), GND | 5 V and ground for the LEDs, the level shifter and the encoder |

The ADC pins (GP26, GP27) must never see more than 3.3 V.

## Build and flash
- [ ] Once: `brew install picotool` and (in `subprojects/hardware/firmware`) `rustup target add
      thumbv8m.main-none-eabihf` (rust-toolchain.toml does it on the first build)
- [ ] Host tests pass: `cargo test` in `subprojects/hardware/firmware`
- [ ] Hold **BOOTSEL** on the Pico 2 while plugging it into the Mac, then `cargo flash` in
      `subprojects/hardware/firmware`: picotool loads the firmware and the Pico restarts
- [ ] macOS Audio MIDI Setup (Window › Show MIDI Studio) shows **CMDMX Desk** with one in and one
      out port, no driver asked for (HW-20). Name as listed: ________
- [ ] With the daemon not running: all four key LEDs are **dim red** (HW-32)

## Learn on the Mac (CTL-25, HW-61)
Console running with a show in `--show-dir /tmp/…` (a few playbacks on page 1, a fixture with
an attribute on wheel 1, e.g. a dimmer). Output visible on Art-Net (Claude listens) or in the app.
- [ ] The Control screen lists `CMDMX Desk` `Disabled`; enable it (empty mapping, CTL-24)
- [ ] Learn `fader 1`: click fader 1 on the main screen, move hardware fader 1: `Learned: CMDMX
      Desk CC 0 ch 1 → fader 1`, guessed as `cc14` (CTL-26)
- [ ] Learn `fader 2` the same way: CC 1, `cc14`
- [ ] Learn `upper 1`, `upper 2`, `lower 1`, `lower 2` on keys 1–4: notes 0, 1, 10, 11
- [ ] Learn `wheel 1` (Control screen: a binding with action `wheel 1`, then its Learn): turn the
      encoder a little right and then left; learned as CC 64, encoding `twos_complement`
- [ ] Once a key is learned the dim red pattern is gone; unlit keys are off (HW-32)

## Faders (HW-14, 18)
- [ ] Fader 1 moves playback 1 smoothly from 0 % to 100 %; the bottom reads exactly 0 % and the
      top exactly 100 %
- [ ] Fader 2 moves playback 2
- [ ] A fader left alone sends nothing: the Control screen's last input stays still for 30 s with
      both faders at mid travel (no flicker in the value)
- [ ] Set fader 1 at about 50 %, unplug the box, move the playback on screen to 20 %, plug the box
      back: playback 1 follows the fader's real position (~50 %) or waits for pickup; it never
      jumps to 0 % first (HW-18)

## Keys and LED feedback (HW-20)
- [ ] Key 1 does what playback 1's upper button does (Flash: held while pressed) and its LED is
      white while playback 1 is on; key 3 does `lower 1` (Swop or the lower function)
- [ ] Keys 2 and 4 do the same for playback 2
- [ ] A LED colour from the palette: set `on_value` of `upper 1` to 8 in the bindings table: its LED
      lights **red** when on; 80 gives **blue** (wire-map.md)
- [ ] A fast tap gives exactly one press and one release (no double Flash)

## Wheel 1 (HW-22)
- [ ] With the fixture selected, turning right raises wheel 1's attribute, left lowers it (if the
      direction is wrong, swap A and B)
- [ ] A slow turn changes the value step by step; a fast spin moves it far without skipping back
- [ ] One full turn of the knob gives about ______ steps (128 PPR encoder: about 128)

## Show-proof (HW-30, 32)
- [ ] Quit the daemon with the box plugged in: the keys keep their last state or go dim red; start
      it again: the LEDs follow the engine within 2 s (CTL-16, 21)
- [ ] Unplug the box: the daemon shows it `Missing`, output keeps running; plug it back: `Ok`
      within 2 s, keys dim red until the first feedback, then the engine's state (HW-32)
- [ ] Mac to sleep and awake: the box comes back by itself
- [ ] Watchdog (HW-30): the box ran for 30 minutes of normal use without a hang or a reset (a reset
      shows as `Missing` and back in the Control screen)

## Open points
- none
