# hardware — State

_Last updated: 2026-11-01 (the hardware phase is now phase 10)_

## Current step
`specs/modules/hardware.md` is approved (HW-01): a Tiger Touch II-size desk with one 15.6" touch
screen, Linux reference compute (cross-platform UI is IDEA-30; development on the Mac), 10
motorised playback faders with two keys, 10 plain preset faders with one key, four optical wheels,
keypad, four isolated DMX ports on an own board, CERN-OHL-S v2. Firmware in Rust (embassy,
RP2350).

HWB-01 (USB-MIDI box) is written and waits for the owner's run (gate 3, `tests/hardware/hw-box.md`;
the parts are ordered at DigiKey (2026-11-08), the checklist lists them). Its faders are the
real motorised Bourns PSM on a TB6612FNG, with touch sense (HW-16, 17). `firmware/` is its own cargo
workspace: `logic/` (`cmdmx-desk-logic`, `no_std`: `Fader` deadband 41 + end-snap + settle (HW-14,
18), `Quadrature` + `Accumulator` (HW-22), `Debounce`, `Link` (HW-32), `Motor` P-control + 1 s stall
(HW-16, 17), `Touch` charge-time baseline, `midi` USB-MIDI packets and `Cc14Rx`,
`palette` 16 colours per 8 velocities, `wiremap` all of HW-20, `boxmap`), host tests in
`logic/tests/hw.rs`; `box/` (`cmdmx-box`, embassy on a Pico 2: two faders GP26/27, keys GP2–5,
encoder GP6/7, SK6812 GP16, TB6612 GP9–15, touch GP20/21, watchdog 500 ms fed by the scan loop, `CMDMX Desk` on `1209:0001`).
`cargo test`, `cargo box`, `cargo flash` (picotool); CI job `firmware`. Wire map and palette:
`firmware/wire-map.md`. Next after the run passes: HWB-02, the playback strip.

## Build order (HW-60)
HWB-01 box → HWB-02 playback strip → HWB-03 main controller → HWB-04 preset strip → HWB-05 DMX
board → HWB-06 programmer panel → HWB-07 enclosure → HWB-08 flight case; HWB-09 build guide and BOM.

## Tools
- PCB: KiCad (not yet installed on the owner's Mac).
- Firmware: Rust with embassy for the RP2350 (own cargo workspace under `firmware/`).
- Enclosure and flight case: Fusion (its MCP server did not connect on 2026-10-25); publish STEP
  and DXF next to the native files (HW-53).
