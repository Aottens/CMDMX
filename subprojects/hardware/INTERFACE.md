# hardware — Interface

## Publishes
- A control surface that shows up as a USB-MIDI device (CTL); its default mapping becomes a
  profile in `subprojects/engine/profiles/` through a software task, not a hardware PR.
- Build plans: PCB (KiCad), firmware, enclosure and flight case CAD, BOM, build guide.

## Consumes
- The general control layer's MIDI input and feedback (CTL in `specs/modules/control-surfaces.md`).
- `faders_per_page` stays configuration; the hardware never assumes a count.

## Does NOT handle
- Any lighting logic or DMX output of its own unless `hardware.md` adds DMX ports by criterion.
- Changes under `subprojects/engine` or `subprojects/app`.

## Show-proof
- A lost or rebooting surface never changes levels; the daemon keeps output alive.
