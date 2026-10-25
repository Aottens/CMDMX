# CMDMX Desk wire map (HW-20)

The fixed table of which MIDI message each physical control of the desk sends and receives.
One class-compliant USB-MIDI device `CMDMX Desk`, USB vendor `0x1209` (pid.codes); prototypes use
the pid.codes test PID `0x0001` until the project's own PID is granted. Everything is on **MIDI
channel 1**. Code: `logic/src/wiremap.rs`, `logic/src/midi.rs`, `logic/src/palette.rs`; tests in
`logic/tests/hw.rs`.

## Controls

| Control | Sends | Receives (feedback, CTL-17) |
| --- | --- | --- |
| Playback fader 1–10 | `cc14` CC 0–9 (LSB CC 32–41) | same `cc14`: motor target |
| Preset fader 1–10 | `cc14` CC 10–19 (LSB CC 42–51) | ignored (no motor) |
| Grand master | `cc14` CC 20 (LSB CC 52) | ignored |
| Wheel 1–4 | relative CC 64–67, `twos_complement` (CTL-11) | ignored |
| Playback key upper 1–10 / lower 1–10 | note 0–9 / note 10–19 | same note: LED |
| Preset key 1–10 | note 20–29 | same note: LED |
| Macro key 1–10 | note 30–39 | same note: LED |
| Wheel push 1–4 | note 40–43 | same note: LED |
| DBO, Go, Back, Stop, Release, Page ◀, Page ▶, Fan | note 44–51 | same note: LED |
| Attribute groups I P C G B E S FX | note 52–59 | same note: LED |
| Keypad and programmer keys | note 60–89, table below | same note: LED |

- **Faders** send the MSB CC and then the LSB CC (value 0–16383, MSB = value ÷ 128) in **one
  USB transfer**, so Learn (CTL-26) recognises them as `cc14`. A still fader sends nothing (HW-14:
  deadband 0.25 % of travel = 41 steps); within the deadband of an end stop the value snaps to
  0 or 16383. After power-up or a new USB connection a fader sends its settled position once
  (HW-18).
- **Wheels** send the steps counted since the last message once per USB frame (1 ms), at most ±63
  per message (the rest follows in the next one): 1–63 up, 127–65 down (`twos_complement`).
  Default divider: one step per 4 quadrature counts (HW-22). A to B leading is up.
- **Keys** send note on with velocity 127 at press and note off (velocity 0) at release.

## Keypad and programmer keys (note 60 upward)

In the order of the CTL-05 catalogue. Keys that HW-07 lays out for actions CTL-05 does not have yet
(e.g. Record) get the next free notes from 90 when the programmer panel (HWB-06) is drawn.

| Note | Key | CTL-05 action |
| --- | --- | --- |
| 60–69 | 0–9 | `keypad 0` … `keypad 9` |
| 70 | . | `keypad .` |
| 71 | @ | `keypad @` |
| 72 | Thro | `keypad Thro` |
| 73 | And | `keypad And` |
| 74 | Not | `keypad Not` |
| 75 | Fixture | `keypad Fixture` |
| 76 | Group | `keypad Group` |
| 77 | Palette | `keypad Palette` |
| 78 | Full | `keypad Full` |
| 79 | Enter | `keypad Enter` |
| 80 | Backspace | `keypad Backspace` |
| 81 | Clear | `key clear` |
| 82 | Locate | `key locate` |
| 83 | Highlight | `key highlight` |
| 84 | Rem Dim | `key rem_dim` |
| 85 | Undo | `key undo` |
| 86 | Redo | `key redo` |
| 87 | All | `key all` |
| 88 | Next (Fix +1) | `key step_next` |
| 89 | Prev (Fix −1) | `key step_prev` |

## Colour palette (feedback velocity)

A key LED shows the colour of the velocity it last received on its note; note off is off. The
colour is chosen per block of 8 velocities, so a profile sets `on_value`/`off_value` (CTL-18) to
the first velocity of a block. The firmware scales the palette by a fixed LED brightness
(`BRIGHTNESS` in `box/src/main.rs`; the surface knob of HW-19 replaces it on the desk).

| Velocity | Colour | RGB |
| --- | --- | --- |
| 0–7 | off | 0, 0, 0 |
| 8–15 | red | 255, 0, 0 |
| 16–23 | orange | 255, 96, 0 |
| 24–31 | amber | 255, 160, 0 |
| 32–39 | yellow | 255, 255, 0 |
| 40–47 | lime | 128, 255, 0 |
| 48–55 | green | 0, 255, 0 |
| 56–63 | teal | 0, 255, 128 |
| 64–71 | cyan | 0, 255, 255 |
| 72–79 | sky | 0, 128, 255 |
| 80–87 | blue | 0, 0, 255 |
| 88–95 | violet | 128, 0, 255 |
| 96–103 | magenta | 255, 0, 255 |
| 104–111 | pink | 255, 64, 128 |
| 112–119 | warm white | 255, 180, 100 |
| 120–127 | white | 255, 255, 255 |

The default `on_value` 127 is white, `off_value` 0 is off.

**Disconnected** (HW-32): while the USB host is gone, or connected but has not sent any feedback
yet, every key LED shows dim red (32, 0, 0); the first
feedback message from the host ends the pattern and the keys show their own state (off until the
host says otherwise).

## The USB-MIDI box (HWB-01, HW-61)

The proof of concept carries five controls of this map:

| Box control | Wire map control | Message |
| --- | --- | --- |
| Fader 1 | playback fader 1 | `cc14` CC 0 / 32 |
| Fader 2 | playback fader 2 | `cc14` CC 1 / 33 (motor target ignored: no motor) |
| Key 1 | upper 1 | note 0 |
| Key 2 | upper 2 | note 1 |
| Key 3 | lower 1 | note 10 |
| Key 4 | lower 2 | note 11 |
| Encoder | wheel 1 | relative CC 64 |
