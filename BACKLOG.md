# ColourmotionDMX — Backlog

Conventions (from the project-manage skill): one sentence per task with a verifiable done
criterion; T-shirt size (S ≤ 2 h, M ≤ half day, L ≤ day, XL = split it); owner reviews every PR.
Status: `todo` · `doing` · `review` · `done` · `blocked`.

## Vertical slice 1

| Id | Task (one sentence, verifiable) | Spec ids | Size | Status |
| --- | --- | --- | --- | --- |
| VS1-01 | `cargo test` is green on a fresh clone, `swift build` produces a binary in `subprojects/app` on the owner's Mac. | — | S | done (2026-10-16; no Swift test target, see CLAUDE.md) |
| VS1-02 | Daemon opens an Art-Net broadcast socket and emits the current frame of universe 1 at 40 Hz; owner sees it in an Art-Net monitor. | OUT-01..04, 11, 12, 15 | M | done (2026-10-16; hardware run `tests/hardware/runs/2026-10-16-VS1-02.md`) |
| VS1-03 | Daemon emits sACN universe 1 (priority 100) alongside Art-Net; owner sees it in sACNView. | OUT-05..07, 16, 17 | S | done (2026-10-17; hardware run `tests/hardware/runs/2026-10-17-VS1-03.md`) |
| VS1-04 | Daemon detects the Eurolite MK2 by USB product string, opens its serial port and sends ENTTEC-Pro frames; owner's dimmer follows a fader. | OUT-08, 09, 12, 20..22, hardware checklist | M | doing (spec awaiting approval) |
| VS1-05 | IPC contract v0: app connects, receives state snapshot, sends `fader.set`, receives delta; reconnect after daemon restart. | IPC-01..03 | M | todo |
| VS1-06 | App shows `faders_per_page` on-screen faders bound to playbacks of page 1; moving one changes DMX. | PBK (subset) | M | todo |
| VS1-07 | MCU driver: BCF2000 fader 1 pitchbend moves playback 1; engine echoes level back and the motor follows after a page change. | CTL-01..04 | M | todo |
| VS1-08 | Supervisor-less resilience check: kill the app while output runs; DMX keeps streaming; app reconnects and shows the live state. | ADR-002 | S | todo |

## Phase 1 modules (after the slice)

| Id | Task | Spec | Size | Status |
| --- | --- | --- | --- | --- |
| P1-01 | Write `fixture-library.md` spec incl. GDTF subset to parse and the overlay schema; owner approves. | LIB | M | todo |
| P1-02 | Write `programmer.md` spec from the Titan analysis (selection order, touched attributes, Clear options, Locate masks). | PRG | M | todo |
| P1-03 | Write `palettes.md` spec (Global/Shared/Normal auto-detection, quick record masks, quick palette, fade/overlap). | PAL | M | todo |
| P1-04 | Write `playbacks.md` spec (fader modes 0/1/2, priority, release mask/time, kill point, locks, playback groups, chases, masters). | PBK | L | todo |
| P1-05 | Write `control-surfaces.md` spec (MCU map, generic MIDI map, OSC schema). | CTL | M | todo |
| P1-06 | Write `ipc.md` spec (message schema, versioning, snapshot/delta, reconnect). | IPC | M | todo |

## Open questions (owner decides)
- Licence: MIT or GPL-3.0?
- IPC transport: raw TCP with length-prefixed JSON, or WebSocket (easier for future web/iPad clients)?
