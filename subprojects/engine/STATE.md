# engine — State

_Last updated: 2026-10-16_

## Current step
Scaffold landed; vertical slice 1 starts with VS1-02 (Art-Net socket on its own thread).

## Working
- `cmdmx-core`: attribute model (IPCGBES-FX, HTP/LTP, 8/16-bit), minimal profiles, patch with range checks, playbacks (priority, fader mode 0/1/2, activation + trigger counters), merge engine (ENG-01/02/03/05/06/12), scenario runner. 5 scenario files green.
- `cmdmx-protocols`: Art-Net ArtDmx, sACN E1.31 data packet, ENTTEC Pro label-6 frame + widget-params request + product-string detection. Golden-byte tests (OUT-01/02/05/06/07/08/09).
- `cmdmx-control`: MCU decode/encode for faders (10-bit pitchbend), buttons, V-Pots, ring LEDs, fader touch (CTL-01..04 provisional ids).
- `cmdmx-daemon`: 40 Hz loop over a demo patch; Art-Net output thread for line 1 (`artnet_out.rs`: non-blocking hand-off, Ok/Degraded health, socket reopen, panic caught) — OUT-03/04/11/12/15. `--ticks N`, `--artnet-target IPV4`, `--no-artnet`. No sACN socket/serial/MIDI yet.
- Workspace lints deny unwrap/expect/panic/indexing/todo; `cargo clippy -D warnings` and `cargo fmt --check` pass.

## In progress
- VS1-02 in review

## Known gaps (by design, each waits for its spec)
- Programmer (ENG-04), release masks/times (ENG-07..10), allocation-free tick (ENG-13), chases, masters other than GM, show file (SHW-*), IPC (IPC-*), GDTF (LIB-*).

## Resume with
`cargo test && cargo clippy --all-targets -- -D warnings`, then VS1-03: add an sACN node next to `artnet_out.rs` (same thread pattern; consider sharing `Health`).
