# ColourmotionDMX — Product Requirements (Phase 1)

Source: "Avolites Titan — analyse als basis voor eigen macOS DMX-console" (owner's Claude Doc, 2026-10-16), section "Eerste speclist". This file is the repo-side copy of that table, trimmed to Phase 1 and mapped to module specs.

## Goal
One evening of busking on a Mac with a BCF2000, using Avolites muscle memory: programmer, IPCGBES palettes on a grid, fader pages with Titan fader modes and release behaviour, chases on a BPM master, Art-Net/sACN/USB-DMX out.

## Non-goals (phase 1)
Shapes, pixel mapper, cue lists with tracking, timelines, media, visualiser, multi-console, timecode.

## Module map

| Area | Phase-1 requirement | Module spec | Criterion prefix |
| --- | --- | --- | --- |
| HTP/LTP engine | intensity HTP, other attributes LTP, 5 priority levels, programmer above playbacks (except Very High), release to previous value / quick palette, timed Clear | `specs/modules/engine-merge.md` | ENG |
| Output | Art-Net (unicast/broadcast, continuous), sACN (priority, sync), ENTTEC-Pro serial (Eurolite MK2 reference); lines → nodes mapping | `specs/modules/output-protocols.md` | OUT |
| Show file | one readable JSON, autosave + atomic rename, versioned schema, git-friendly | `specs/modules/showfile.md` | SHW |
| Fixture library | GDTF import + overlay; minimal JSON profile for tests only | `specs/modules/fixture-library.md` | LIB |
| Patch | profile + mode + universe + address + user number; cells | `specs/modules/patch.md` | PAT |
| Programmer & wheels | ordered selection, touched attributes only, Locate/Highlight outside programmer, Clear options, 16-bit, encoders | `specs/modules/programmer.md` | PRG |
| Palettes & preset grid | reference model; Global/Shared/Normal auto; quick record with window mask; quick palette; typed fade + overlap; auto-legend + halo; configurable grid | `specs/modules/palettes.md` | PAL |
| Playbacks & pages | fader modes 0/1/2, priority, release mask/time, kill point, handle paging locks, Flash/Swop + key profiles, `faders_per_page` config, Grand Master, BPM master, group masters, playback groups, chases | `specs/modules/playbacks.md` | PBK |
| Control surfaces | MCU driver (BCF2000/X-Touch) with motor feedback; generic MIDI mapping; OSC schema | `specs/modules/control-surfaces.md` | CTL |
| IPC & app shell | daemon ↔ app contract, reconnect, engine-offline state, thin client | `specs/modules/ipc.md` | IPC |

## Vertical slice 1 (the first milestone)
One hand-written profile → patch on universe 1 → one fader in the app → Art-Net + sACN out → Eurolite MK2 serial out → BCF2000 fader 1 moves the level with motor feedback. Criteria involved: ENG-01..04, OUT-01..09, SHW-01..04, CTL-01..04 (to be written), IPC-01..03 (to be written).

## Definition of done (per task)
A task is done when: its criteria have passing tests with the criterion id in the name; `cargo test`, `cargo clippy -D warnings` and `cargo fmt --check` pass; STATE/BACKLOG are updated; hardware criteria have a ticked checklist in `tests/hardware/`.
