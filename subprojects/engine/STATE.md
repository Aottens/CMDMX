# engine — State

_Last updated: 2026-10-16_

## Current step
Scaffold landed; vertical slice 1 starts with VS1-02 (Art-Net socket on its own thread).

## Working
- `cmdmx-core`: attribute model (IPCGBES-FX, HTP/LTP, 8/16-bit), minimal profiles, patch with range checks, playbacks (priority, fader mode 0/1/2, activation + trigger counters), merge engine (ENG-01/02/03/05/06/12), scenario runner. 5 scenario files green.
- `cmdmx-protocols`: Art-Net ArtDmx, sACN E1.31 data packet, ENTTEC Pro label-6 frame + widget-params request + product-string detection. Golden-byte tests (OUT-01/02/05/06/07/08/09).
- `cmdmx-control`: MCU decode/encode for faders (10-bit pitchbend), buttons, V-Pots, ring LEDs, fader touch (CTL-01..04 provisional ids).
- `cmdmx-daemon`: 40 Hz loop over a demo patch; line 1 to an Art-Net node (`artnet_out.rs`, OUT-03/04/15) and an sACN node (`sacn_out.rs`, multicast + loopback, fixed CID, OUT-06/07/16/17), each on its own thread via `output.rs` (non-blocking hand-off, Ok/Degraded health, socket reopen, panic caught; OUT-11/12). ENTTEC Pro serial node (`serial_out.rs`, `serialport` crate: product-string detection, raw 115200 8N1, rescan ≤ 1/s; OUT-09/20/21/22) — unverified on hardware. Flags: `--ticks N`, `--artnet-target`, `--no-artnet`, `--sacn-unicast`, `--sacn-priority`, `--no-sacn`, `--serial PATH`, `--no-serial`. No MIDI yet.
- Workspace lints deny unwrap/expect/panic/indexing/todo; `cargo clippy -D warnings` and `cargo fmt --check` pass.

## In progress
- VS1-04 in review; hardware check waits for the church

## Known gaps (by design, each waits for its spec)
- Programmer (ENG-04), release masks/times (ENG-07..10), allocation-free tick (ENG-13), chases, masters other than GM, show file (SHW-*), IPC (IPC-*), GDTF (LIB-*).

## Resume with
`cargo test && cargo clippy --all-targets -- -D warnings`, then VS1-07 (MCU driver in the daemon via `midir`) or VS1-05 (IPC).
