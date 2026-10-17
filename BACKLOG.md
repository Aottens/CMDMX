# ColourmotionDMX — Backlog

Conventions (from the project-manage skill): one sentence per task with a verifiable done
criterion; T-shirt size (S ≤ 2 h, M ≤ half day, L ≤ day, XL = split it); owner reviews every PR.
Status: `todo` · `doing` · `review` · `done` · `blocked`.

## Vertical slice 1
Software first (owner, 2026-10-17): VS1-05/06 next; VS1-07 and the VS1-04 hardware check form a hardware track for when the hardware is at hand.

| Id | Task (one sentence, verifiable) | Spec ids | Size | Status |
| --- | --- | --- | --- | --- |
| VS1-01 | `cargo test` is green on a fresh clone, `swift build` produces a binary in `subprojects/app` on the owner's Mac. | — | S | done (2026-10-16; no Swift test target, see CLAUDE.md) |
| VS1-02 | Daemon opens an Art-Net broadcast socket and emits the current frame of universe 1 at 40 Hz; owner sees it in an Art-Net monitor. | OUT-01..04, 11, 12, 15 | M | done (2026-10-16; hardware run `tests/hardware/runs/2026-10-16-VS1-02.md`) |
| VS1-03 | Daemon emits sACN universe 1 (priority 100) alongside Art-Net; owner sees it in sACNView. | OUT-05..07, 16, 17 | S | done (2026-10-17; hardware run `tests/hardware/runs/2026-10-17-VS1-03.md`) |
| VS1-04 | Daemon detects the Eurolite MK2 by USB product string, opens its serial port and sends ENTTEC-Pro frames; owner's dimmer follows a fader. | OUT-08, 09, 12, 20..22, hardware checklist | M | blocked (code merged; gate 3 at the church, see VS4-02) |
| VS1-05 | IPC contract v0: app connects, receives state snapshot, sends `fader.set`, receives delta; reconnect after daemon restart. | IPC-01..09 | M | done (2026-10-17; manual run `tests/manual/runs/2026-10-17-VS1-05.md`) |
| VS1-06 | App shows `faders_per_page` on-screen faders bound to playbacks of page 1; moving one changes DMX. | PBK (subset) | M | todo |
| VS1-07 | MCU driver: BCF2000 fader 1 pitchbend moves playback 1; engine echoes level back and the motor follows after a page change. | CTL-01..04 | M | parked (hardware track: BCF2000 not at hand) |
| VS1-08 | Supervisor-less resilience check: kill the app while output runs; DMX keeps streaming; app reconnects and shows the live state. | ADR-002 | S | todo |

## Phase 1 milestones — finish line: replace Lightkey in the church
Phase 1 ends when a real Sunday morning service in the church (reference rig, `PROJECT.md`) is
programmed and run on ColourmotionDMX instead of Lightkey. Three demonstrable milestones; tasks
after the specs are added when each spec is approved (no sizing of code before its spec).

### VS2 — Programming: select, set, record, play back (screen only, output visible via Art-Net)
| Id | Task (one sentence, verifiable) | Spec | Size | Status |
| --- | --- | --- | --- | --- |
| UX-01 | Clickable HTML sketch of the Titan-style main screen (fixtures/groups, palette grid, programmer + encoders, playback faders, cue list), max two iterations; owner says it feels right. Guides the SwiftUI work. | — | M | review (iteration 2: `specs/ux/main-screen.html`, research `specs/ux/research.md`) |
| P1-01 | Write `fixture-library.md`: GDTF subset + overlay, **and a profile from a plain channel list** for fixtures without GDTF (the church's Chinese pars); owner approves. | LIB | M | todo |
| P1-07 | ADR-005: colour is stored as a device-independent target colour and mapped per fixture to RGB/RGBW/RGB+Lime/CMY; owner approves before P1-03. | PAL, SHW | S | todo |
| P1-02 | Write `programmer.md` from the Titan analysis (ordered selection, groups, touched attributes, colour picker RGB/CMY/HSI, Clear options, Locate/Highlight). | PRG | M | todo |
| P1-03 | Write `palettes.md` (Global/Shared/Normal auto-detection, quick record masks, quick palette, fade/overlap). | PAL | M | todo |
| VS2-01 | Demo: patch the church rig, select the LED pars, set a colour and intensity, record a palette and a playback, bring it up on an on-screen fader; Art-Net shows the right values on every fixture type. | PAT, PRG, PAL, PBK | — | todo (split after specs) |

### VS3 — Running a show: cue list and busking pages
| Id | Task (one sentence, verifiable) | Spec | Size | Status |
| --- | --- | --- | --- | --- |
| P1-08 | Write `cuelists.md` (Titan cue list: record, Go/Back, fade/delay, link/follow; touched attributes only so tracking can follow in phase 2). | CUE | M | todo |
| P1-04 | Write `playbacks.md` (fader modes 0/1/2, priority, release mask/time, kill point, locks, playback groups, chases, masters). | PBK | L | todo |
| P1-09 | Write `showfile.md` criteria (SHW): one readable JSON, autosave + atomic rename, versioned schema. | SHW | M | todo |
| VS3-01 | Demo: a service-sized cue list (walk-in, worship, sermon, video, walk-out) plus the 6 front spots on their own fader runs with Go/Back and survives a daemon restart via the show file. | CUE, PBK, SHW | — | todo (split after specs) |

### VS4 — The church: replace Lightkey
| Id | Task (one sentence, verifiable) | Spec | Size | Status |
| --- | --- | --- | --- | --- |
| VS4-01 | Write `tests/hardware/church.md`: checklist for running a service (rig patched, all fixture types match, front-out works, fallback to Lightkey ready). | — | S | todo |
| VS4-02 | At the church: VS1-04 serial lines of `tests/hardware/output.md` pass with the Eurolite. | OUT-09, 20..22 | S | blocked (needs a church visit) |
| VS4-04 | The church TouchOSC layout drives ColourmotionDMX through OSC (CTL/P1-05), so the remote in the other room keeps working. | CTL | M | todo |
| VS4-03 | One real Sunday morning service run entirely on ColourmotionDMX, Lightkey on standby as fallback; checklist ticked. | all phase 1 | — | todo |

### Supporting specs (needed by the milestones above)
| Id | Task | Spec | Size | Status |
| --- | --- | --- | --- | --- |
| P1-05 | Write `control-surfaces.md` (MCU map, generic MIDI map, OSC schema). | CTL | M | todo |
| P1-06 | Write `ipc.md` (message schema, versioning, snapshot/delta, reconnect) — VS1-05 writes the first subset. | IPC | M | todo |

## Later phases
Ideas waiting for their phase live in `IDEAS.md` (IDEA-01..06). They get backlog rows when their phase is planned.

## Open questions (owner decides)
- Licence: MIT or GPL-3.0?
