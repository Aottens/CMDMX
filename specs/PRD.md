# ColourmotionDMX — Product Requirements (Phase 1)

Source: "Avolites Titan — analyse als basis voor eigen macOS DMX-console" (owner's Claude Doc, 2026-10-16), section "Eerste speclist". This file is the repo-side copy of that table, trimmed to Phase 1 and mapped to module specs.

## Goal
Program the building blocks of any show on a Mac, the Titan way: a robust programmer, palettes on a grid, cues recorded onto playbacks and into cue lists; then run them — as a cue list, as busking pages, or both — with Art-Net/sACN/USB-DMX out. Programming is the focus; running proves what was programmed.

Finish line: a real Sunday morning service in the church run on ColourmotionDMX instead of Lightkey (`PROJECT.md`, reference venue). Baseline: the church rig, ~30 fixtures on one universe. Headroom target (provisional): 16 universes, 500 fixtures, 1000 cues per show.

## Non-goals (phase 1)
Shapes, pixel mapper, timelines and timecode, media, visualiser, multi-console.

## Module map

| Area | Phase-1 requirement | Module spec | Criterion prefix |
| --- | --- | --- | --- |
| HTP/LTP engine | intensity HTP, other attributes LTP, 5 priority levels, programmer above playbacks (except Very High), release to previous value / quick palette, timed Clear | `specs/modules/engine-merge.md` | ENG |
| Output | Art-Net (unicast/broadcast, continuous), sACN (priority, sync), ENTTEC-Pro serial (Eurolite MK2 reference); lines → nodes mapping | `specs/modules/output-protocols.md` | OUT |
| Show file | one readable JSON, autosave + atomic rename, versioned schema, git-friendly | `specs/modules/showfile.md` | SHW |
| Fixture library | GDTF import + overlay; minimal JSON profile for tests only | `specs/modules/fixture-library.md` | LIB |
| Patch | profile + mode + universe + address + user number; cells | `specs/modules/patch.md` | PAT |
| Programmer & wheels | **the core of phase 1**: ordered selection, groups, touched attributes only, colour picker RGB/CMY/HSI, Locate/Highlight outside programmer, Clear options, 16-bit, encoders | `specs/modules/programmer.md` | PRG |
| Palettes & preset grid | reference model; Global/Shared/Normal auto; quick record with window mask; quick palette; typed fade + overlap; auto-legend + halo; configurable grid | `specs/modules/palettes.md` | PAL |
| Cue lists | record cues (touched attributes only), Go/Back, fade + delay per cue, attribute-group times, link/follow, tracking (owner, 2026-10-21), chases with BPM/Tap, BPM and Rate masters | `specs/modules/cuelists.md` | CUE |
| Playbacks & pages | fader modes 0/1/2, priority, release mask/time, kill point, handle paging locks, Flash/Swop + key profiles, `faders_per_page` config, Grand Master, BPM master, group masters (`specs/modules/group-masters.md`, GRM), playback groups, chases | `specs/modules/playbacks.md` | PBK |
| Control surfaces | MCU driver (BCF2000/X-Touch) with motor feedback; generic MIDI mapping; OSC schema | `specs/modules/control-surfaces.md` | CTL |
| IPC & app shell | daemon ↔ app contract, reconnect, engine-offline state, thin client | `specs/modules/ipc.md` | IPC |

## Vertical slice 1 (the first milestone)
One hand-written profile → patch on universe 1 → one fader in the app → Art-Net + sACN out → Eurolite MK2 serial out → BCF2000 fader 1 moves the level with motor feedback. Criteria involved: ENG-01..04, OUT-01..09, SHW-01..04, CTL-01..04 (to be written), IPC-01..03 (to be written).

## Definition of done (per task)
A task is done when: its criteria have passing tests with the criterion id in the name; `cargo test`, `cargo clippy -D warnings` and `cargo fmt --check` pass; STATE/BACKLOG are updated; hardware criteria have a ticked checklist in `tests/hardware/`.
