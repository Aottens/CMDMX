# ColourmotionDMX — State

_Last updated: 2026-10-17_

## Current focus
Vertical slice 1: one hand-written fixture → patch on universe 1 → one fader in the app → Art-Net + sACN out → Eurolite MK2 over serial → BCF2000 fader moves it with motor feedback.

## Working
- Repo scaffold: meta-layer, ADR-001..004, PRD, module specs (3 written, 3 skeleton)
- `cmdmx-core`: universe buffer, patch of a dimmer/generic fixture, HTP/LTP merge, playback level → DMX frame; scenario test runner over `tests/scenarios/*.json`
- `cmdmx-protocols`: Art-Net ArtDmx, sACN E1.31 data packet, ENTTEC Pro "send DMX" frame — all byte-exact tested
- `cmdmx-control`: Mackie Control decode/encode for fader pitchbend and button notes (unit tested)
- `cmdmx-daemon`: 40 Hz tick loop; sends line 1 as Art-Net (VS1-02 done) and sACN universe 1 multicast, priority 100 (VS1-03 done), and ENTTEC Pro serial with product-string detection and hotplug (VS1-04, hardware unverified), each node on its own thread. IPC v0 over WebSocket on 127.0.0.1:7319 (VS1-05 done); the app's faders drive DMX
- ADR-006 accepted, `fixture-library.md` LIB-01..16 and `patch.md` PAT-01..14 approved; build tasks VS2-08..13 in BACKLOG
- ADR-005 accepted: one BT.2020 linear target colour per fixture, White/Lime as direct attributes, nominal mapping in phase 1
- Programmer spec `specs/modules/programmer.md` PRG-01..37 approved (P1-02); tasks VS2-02..07 in BACKLOG
- `cmdmx-core::programmer`: ordered selection, groups, Fix ±1 stepping, patterns, All, record group (VS2-02, PRG-01..07, 14); command line `Fixture/Group/Thro/And/Not/@/Enter` (VS2-03, PRG-08/09/11); programmer values in the merge at Programmer priority, relative wheels Normal/Fine/Coarse, wheel layout and active group, `contents(mask)` (VS2-04); over IPC with the church demo rig, and in the app: fixtures + groups windows, attribute bar with wheels, command line (VS2-07, IPC-10..17; owner run `tests/manual/runs/2026-10-17-VS2-07.md` passed). HTP without a source now outputs 0 (ENG-07 part)
- UI v0.1: clickable main-screen sketch `specs/ux/main-screen.html` (UX-01 done; Artifact https://claude.ai/artifact/YNEG3JsZAnjWhG1793MDUd), built from `specs/ux/research.md`; reference for the SwiftUI work and the PRG/PAL/PBK specs
- `subprojects/app`: SwiftPM app builds and runs on the owner's Mac (CLT only); offline banner, Fixtures + Groups windows, attribute bar (IPCGBES tabs, wheels, Fix ±1/Odd-Even/All/Clear, command line), 8 faders + GM. No Swift test target (XCTest/Testing absent from CLT)

## In progress
- nothing (owner exports the church patch from Lightkey on 2026-10-18)

## Blocked
- VS1-04 gate 3: the Eurolite MK2 is at the church; run the serial lines of `tests/hardware/output.md` on the next visit

## Hardware available for testing
- Behringer BCF2000 (use in Mackie Control mode)
- Eurolite USB-DMX512 PRO Interface MK2 (FTDI FT232R, ENTTEC Pro protocol, product string "Eurolite DMX512 Pro") — **at the church, not on the desk**; its hardware checks wait for a visit
- Mac (Apple Silicon), Command Line Tools instead of Xcode.app

## Repo
- Public: https://github.com/Aottens/ColourmotionDMX (one PR per task)

## Resume next session with
Start a fresh session (read PROJECT/STATE/ARCHITECTURE/DECISIONS, CLAUDE.md). Next: **VS2-08** profile model v2 (LIB-01..03, 16) — failing tests first, Sonnet implements, main session reviews. Then in dependency order: VS2-06 (colour value) and VS2-09 (QLC+ importer) and VS2-10 (patch core), then VS2-11, VS2-12, VS2-13 (church patch from the owner's Lightkey export, expected 2026-10-18; the owner may also hand over a Lightkey show file to decode). Still open: VS2-05 (Locate/Highlight/Off/Rem Dim/Clear options incl. freeze/release), P1-09 show file (needed for PAT-08 persistence), P1-12 GDTF importer spec, P1-13 per-cell selection spec, UX-02 (UI v0.1 layout, split first). Every PR updates README (CLAUDE.md step 5). The church is the playground, not the limit (owner): no church-driven feature cuts.
