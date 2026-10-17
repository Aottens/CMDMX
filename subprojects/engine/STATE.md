# engine — State

_Last updated: 2026-10-17_

## Current step
Scaffold landed; vertical slice 1 starts with VS1-02 (Art-Net socket on its own thread).

## Working
- `cmdmx-core`: attribute model (IPCGBES-FX, HTP/LTP, 8/16-bit), minimal profiles, patch with range checks, playbacks (priority, fader mode 0/1/2, activation + trigger counters), merge engine (ENG-01/02/03/05/06/12), scenario runner. 5 scenario files green.
- `cmdmx-protocols`: Art-Net ArtDmx, sACN E1.31 data packet, ENTTEC Pro label-6 frame + widget-params request + product-string detection. Golden-byte tests (OUT-01/02/05/06/07/08/09).
- `cmdmx-control`: MCU decode/encode for faders (10-bit pitchbend), buttons, V-Pots, ring LEDs, fader touch (CTL-01..04 provisional ids).
- `cmdmx-daemon`: 40 Hz loop over a demo patch; line 1 to an Art-Net node (`artnet_out.rs`, OUT-03/04/15) and an sACN node (`sacn_out.rs`, multicast + loopback, fixed CID, OUT-06/07/16/17), each on its own thread via `output.rs` (non-blocking hand-off, Ok/Degraded health, socket reopen, panic caught; OUT-11/12). ENTTEC Pro serial node (`serial_out.rs`, `serialport` crate: product-string detection, raw 115200 8N1, rescan ≤ 1/s; OUT-09/20/21/22) — unverified on hardware. IPC v0 (`ipc.rs`): WebSocket server on 127.0.0.1:7319, refuses browser Origins, snapshot + per-tick deltas, one thread per client, bounded queues (IPC-01..07). Demo show: one dimmer per fader; `--demo` for the breathing fader 1. Flags: `--ticks N`, `--ipc-port N`, `--demo`, `--artnet-target`, `--no-artnet`, `--sacn-unicast`, `--sacn-priority`, `--no-sacn`, `--serial PATH`, `--no-serial`. No MIDI yet.
- `cmdmx-core::programmer` (VS2-02): ordered selection with toggle, groups (record/select, order kept), new selection after a change (PRG-03), Fix ±1 stepping, n-stage patterns, All; `Engine::set_programmer_value` stores manual values (not in the merge yet). 21 tests in `cmdmx-core/tests/programmer.rs`.
- `cmdmx-core::command` (VS2-03): Titan keypad parser (`Fixture`/`Group`/`Thro`/`And`/`Not`/`@`/`Enter`) + `Engine::execute_command`: typed selection replaces, `@ v` sets I/HTP attributes, parse-then-apply. 16 tests in `cmdmx-core/tests/command_line.rs`; shared rig in `tests/common/mod.rs`.
- Workspace lints deny unwrap/expect/panic/indexing/todo; `cargo clippy -D warnings` and `cargo fmt --check` pass.

## In progress
- nothing

## Known gaps (by design, each waits for its spec)
- Programmer in the merge (ENG-04, VS2-04), release masks/times (ENG-07..10), allocation-free tick (ENG-13), chases, masters other than GM, show file (SHW-*), IPC (IPC-*), GDTF (LIB-*).

## Resume with
`cargo test && cargo clippy --all-targets -- -D warnings`, then VS1-07 (MCU driver in the daemon via `midir`) or VS1-05 (IPC).
