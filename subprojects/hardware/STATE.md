# hardware — State

_Last updated: 2026-10-25 (HW-01: `hardware.md` approved)_

## Current step
`specs/modules/hardware.md` is approved (HW-01): a Tiger Touch II-size desk with one 15.6" touch
screen, Linux reference compute (cross-platform UI is IDEA-30; development on the Mac), 10
motorised playback faders with two keys, 10 plain preset faders with one key, four optical wheels,
keypad, four isolated DMX ports on an own board, CERN-OHL-S v2. Firmware in Rust (embassy,
RP2350). Next: HWB-01, the USB-MIDI box on a Pico 2, learned on the Mac.

## Build order (HW-60)
HWB-01 box → HWB-02 playback strip → HWB-03 main controller → HWB-04 preset strip → HWB-05 DMX
board → HWB-06 programmer panel → HWB-07 enclosure → HWB-08 flight case; HWB-09 build guide and BOM.

## Tools
- PCB: KiCad (not yet installed on the owner's Mac).
- Firmware: Rust with embassy for the RP2350 (own cargo workspace under `firmware/`).
- Enclosure and flight case: Fusion (its MCP server did not connect on 2026-10-25); publish STEP
  and DXF next to the native files (HW-53).
