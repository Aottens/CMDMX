# ADR-003: Mackie Control Universal is the first control-surface protocol

**Status:** accepted, amended 2026-10-23
**Date:** 2026-10-16

> **Amendment 2026-10-23 (owner, P1-05):** the generic MIDI and OSC layer comes first and Mackie Control is a *surface profile* on it, not the first driver with generic MIDI after it. The wire format below stays (CTL-01..04); the bank model is `control-surfaces.md` CTL-31/32 (a window of the page, never a hard-coded 8). See DECISIONS 2026-10-23.

## Context
The owner's controller is a Behringer BCF2000: 8 motorised faders, 8 push-encoders with LED rings in 4 encoder groups, 16 buttons under the encoders, 4 free buttons, no display. It supports a plain MIDI mode (7-bit CC faders, user-assignable) and a Mackie Control emulation (10-bit pitchbend faders, motor feedback, standard button/LED map). The same MCU protocol is native to Behringer X-Touch, X-Touch Compact, iCON and others.

## Decision
- `cmdmx-control` implements **Mackie Control Universal** first: fader positions as pitchbend per channel (14-bit message, 10-bit effective), fader motor feedback by echoing pitchbend, buttons as note on/off per the MCU map, V-Pots as relative CC with LED-ring feedback, bank/channel buttons as page change.
- The BCF2000 is operated in **MC mode**.
- A **generic MIDI note/CC mapping** driver follows as the second implementation of the same `ControlSurface` trait, for nanoKONTROL/APC/Launch Control class devices without motors.
- The daemon owns the MIDI ports; `cmdmx-control` is pure encode/decode.

## Alternatives
- Custom CC mapping per controller: 7-bit faders and a configuration matrix per device; no shared feedback model.
- HUI: older, less supported on current hardware.

## Consequences
- One driver covers the BCF2000 and the X-Touch family; X-Touch adds scribble strips for legends via MCU sysex (later task).
- MCU's bank model is 8 channels wide; `faders_per_page` other than 8 maps to MCU extenders or to bank offsets (spec it when it comes up).
- Pick-up / pass-through behaviour for non-motorised faders stays an open question for the generic driver only; with MCU motors it does not arise.
- Proposed BCF2000 mapping (to be confirmed on hardware, see `tests/hardware/bcf2000.md`):
  faders → playbacks of active page; upper button row → Swop/Select; lower row → Flash;
  encoder group 1 → Intensity + Position wheels; group 2 → Colour; group 3 → Gobo + Beam;
  group 4 → FX/Special (+ Grand Master); four right-hand buttons → Page−, Page+, Clear, Locate.
