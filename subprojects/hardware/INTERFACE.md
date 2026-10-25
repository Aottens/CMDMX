# hardware — Interface

## Publishes
- A control surface that shows up as a USB-MIDI device (CTL); its default mapping becomes a
  profile in `subprojects/engine/profiles/` through a software task, not a hardware PR.
- A DMX board with four isolated ports, seen as four ENTTEC-Pro-compatible serial ports (HW-41).
- Build plans: PCB (KiCad), firmware, enclosure and flight case CAD, BOM, build guide.

## Consumes
- The general control layer's MIDI input and feedback (CTL in `specs/modules/control-surfaces.md`).
- `faders_per_page` stays configuration; the hardware never assumes a count.

## Does NOT handle
- Any lighting logic. DMX output only through the DMX board (HW-40..44), which the daemon drives
  as ENTTEC Pro serial nodes (OUT-31); it never decides levels itself.
- Changes under `subprojects/engine` or `subprojects/app`.

## Show-proof
- A lost or rebooting surface never changes levels; the daemon keeps output alive.
