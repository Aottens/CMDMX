# USB-MIDI box checklist — CMDMX Desk on a Pico 2 (HWB-01, HW-14, 16, 17, 18, 20, 22, 30, 32, 61)

Gate 3: only the owner ticks these. Copy to `tests/hardware/runs/<date>-HWB-01.md` for a run.
Firmware: `subprojects/hardware/firmware/` (`box/`), wire map
[`wire-map.md`](../../subprojects/hardware/firmware/wire-map.md). The box needs no software
change: it is learned on the Mac (CTL-25).

## Parts (DigiKey order placed by the owner on 2026-11-08, not yet on the desk)
One DigiKey order (digikey.nl, free shipping from €75), prices excl. VAT seen on 2026-11-08; the switches come from Amazon.nl,
because no distributor carries the MX2A Honey (DECISIONS 2026-11-08). The owner has a bench supply, breadboards,
jumper wires, through-hole resistors and a micro-USB data cable.
- [ ] 2 × **Bourns PSM01-082A-103B2** motorised fader, 100 mm, 10 kΩ, 10 V motor, touch track (about €26)
- [ ] 1 × **Bourns EM14A0D-C24-L064N** optical encoder, 64 PPR, no detent, 1/4" shaft (about €30)
- [ ] 1 × **Raspberry Pi Pico 2 H** (SC1632, headers fitted; about €5)
- [ ] 1 × **Adafruit 2448** TB6612FNG dual motor driver breakout (DigiKey 1528-1212-ND, about €6), headers to
      solder. Not the Pololu 713: a marketplace item with its own shipping
- [ ] 1 × **Adafruit 1426** NeoPixel Stick, 8 RGB LEDs (DigiKey 1528-1354-ND, about €5; not 6509, the white/amber one): stands in for the SK6812 Mini-E, which needs
      the key PCB; the firmware lights the first four
- [ ] 1 × **SN74AHCT125N** level shifter, DIP-14 (about €1)
- [ ] 1 × **470 µF / 16 V** electrolytic, e.g. Panasonic EEU-FR1C471, across the LED supply
- [ ] Amazon.nl: **CHERRY MX Experience Box 2nd Edition** (DMXAA-XC1X-2A, 10 different 5-pin switches, one of each,
      €14.99 incl. VAT, free delivery): four go in the box, including MX2A Honey. The desk's ~100 switches are
      ordered after the feel is chosen
- [ ] From the drawer: 2 × 1 MΩ (touch), 2 × 10 kΩ and 2 × 20 kΩ (encoder divider), 330 Ω (LED data)
- [ ] Solder short wires to the faders, the encoder and the driver carrier: their pins do not fit a breadboard
- [ ] Knobs come later with the front plate; the bare fader lever is the touch contact

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

## Motors and touch (HW-16, 17, 18)
Bench supply at 10 V, current limit about 1 A. The calibration knobs are constants at the top of
`box/src/main.rs` (`MOTOR_*`, `TOUCH_THRESHOLD`).
- [ ] Power-up: neither motor moves until the console sends a position (HW-18)
- [ ] Move playback 1 on screen to 75 %: hardware fader 1 drives there and stops, without hunting
      or buzzing; same for fader 2. Values used: `MOTOR_KP` ____ `MOTOR_MIN_DUTY` ____
- [ ] No echo: after the motor stops, the Control screen's last input does not show fader 1 sending
      (HW-14)
- [ ] Recall a cue or page with other fader levels: both faders follow
- [ ] Touch: hold the lever while the screen moves playback 1: the motor stays off, nothing fights
      the hand; let go: the fader stays where it was left, then follows the next change (HW-16).
      Value used: `TOUCH_THRESHOLD` ____
- [ ] Stall: hold the lever by its edge with a glove or a non-conductive tool while the screen moves
      it: the motor gives up within about 1 s and stays off until the next target (HW-17)
- [ ] Quit the daemon: the faders stay where they are (HW-32)

## Keys and LED feedback (HW-20)
- [ ] Key 1 does what playback 1's upper button does (Flash: held while pressed) and its LED is
      white while playback 1 is on; key 3 does `lower 1` (Swop or the lower function)
- [ ] Keys 2 and 4 do the same for playback 2
- [ ] A LED colour from the palette: set `on_value` of `upper 1` to 8 in the bindings table: its LED
      lights **red** when on; 80 gives **blue** (wire-map.md)
- [ ] A fast tap gives exactly one press and one release (no double Flash)
- [ ] Feel: with a cap on each, compare MX2A Honey, MX Illuma Brown and MX2A Silent Red, then the rest of the box.
      Switch chosen for the desk: ________

## Wheel 1 (HW-22)
- [ ] With the fixture selected, turning right raises wheel 1's attribute, left lowers it (if the
      direction is wrong, swap A and B)
- [ ] A slow turn changes the value step by step; a fast spin moves it far without skipping back
- [ ] One full turn of the knob gives about ______ steps (64 PPR encoder: about 64). Too
      sensitive or too coarse: change `ENCODER_DIVIDER` in `box/src/main.rs`; value used: ______

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
