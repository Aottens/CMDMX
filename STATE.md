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
- ADR-006 accepted, `fixture-library.md` LIB-01..17 and `patch.md` PAT-01..14 approved; build tasks VS2-08..13 in BACKLOG. LIB-17 (owner, 2026-10-17): emitters are generic — recognised by an `emitter` flag, never by name, any number (Amber, Warm/Cold White, UV, Royal Blue, …)
- Patch over IPC + Patch screen (VS2-12, in review): snapshot `fixtures` entries carry number, name, profile, mode, universe.address, footprint, virtual dimmer and handle, a `fixtures` delta after any patch change, `patch.add/move/remove/renumber/name/change_mode/handle` (PAT-09); library fixtures are patched under `Manufacturer/Model`, fetched from the library in the client thread; the app has a workspace rail (Program / Patch) and a Patch screen with the table, an Add panel with library search and row edits with two-step confirm for Remove and Change Mode (PAT-10). Owner run passed: `tests/manual/runs/2026-10-17-VS2-12.md` (findings → IDEA-08 browse by manufacturer, IDEA-09 multi-select)
- Library (VS2-11, merged): the QLC+ set (1781 fixtures, pinned, fetched by `subprojects/engine/scripts/fetch-qlcplus.sh`, not in git) indexed in the background at daemon start; word search, user profiles that hide the QLC+ one, profiles from a channel list (LIB-11), edited profiles saved atomically (LIB-12), `library.*` over IPC (LIB-14); 1779 of 1781 QLC+ files import
- QLC+ importer (VS2-09, merged): `cmdmx_core::library::qlcplus::import(xml, source)` → profile + import report (`quick-xml`); presets, groups, colour hints, fine channels, heads → cells, slots with sRGB colours, defaults, physical ranges; LIB-18 conventions (owner, 2026-10-17); the Shehds 19x15, Showtec LED Par 56 and LumiPar 7QPro files (QLC+ commit 3d832ce, Apache-2.0) reproduce their expected profiles
- Patch core (VS2-10, merged): engine-assigned internal ids never reused, user numbers on the command line (PAT-12), `patch_add` with auto address/number and all-or-nothing clash checks ("226 would overlap 301 at 1.67"), move, remove, renumber, change mode (drops what the new mode lacks, with warnings), handles in pages of `handles_per_page` (default 40)
- Colour in the app (VS2-14, merged; owner run `tests/manual/runs/2026-10-17-VS2-14.md` passed): demo rig = church rig with RGBW washes, RGBW / RGB+Lime / RGB (virtual dimmer) pars and front spots (IPC-14 amended); `programmer.colour_model` (IPC-18); HSI/RGB/CMY buttons and Hue in degrees in the app (IPC-19)
- Colour as one value (VS2-06, merged): `Value::Level | Value::Colour` through programmer, playbacks and merge; HSI (default) / RGB / CMY wheels as views of one target colour, emitter wheels; nominal RGB/CMY mapping; virtual dimmer scales colour and emitters (GM included); Colour defaults to open white and LTP values are held; cells follow the fixture's Colour/White/Dimmer (owner decisions 2026-10-17)
- Profile model v2 (VS2-08, merged): `cmdmx-core::profile` with modes, cells, colour mixing system, emitter flag, defaults/locate/highlight, slots, physical ranges; validated on load (LIB-02, 17); fixtures are patched in a mode; unproposed LTP attributes output their default (LIB-03); scenario `{attributes}` profiles still load
- ADR-005 accepted: one BT.2020 linear target colour per fixture, White/Lime as direct attributes, nominal mapping in phase 1
- Programmer spec `specs/modules/programmer.md` PRG-01..37 approved (P1-02); tasks VS2-02..07 in BACKLOG
- `cmdmx-core::programmer`: ordered selection, groups, Fix ±1 stepping, patterns, All, record group (VS2-02, PRG-01..07, 14); command line `Fixture/Group/Thro/And/Not/@/Enter` (VS2-03, PRG-08/09/11); programmer values in the merge at Programmer priority, relative wheels Normal/Fine/Coarse, wheel layout and active group, `contents(mask)` (VS2-04); over IPC with the church demo rig, and in the app: fixtures + groups windows, attribute bar with wheels, command line (VS2-07, IPC-10..17; owner run `tests/manual/runs/2026-10-17-VS2-07.md` passed). HTP without a source now outputs 0 (ENG-07 part)
- UI v0.1: clickable main-screen sketch `specs/ux/main-screen.html` (UX-01 done; Artifact https://claude.ai/artifact/YNEG3JsZAnjWhG1793MDUd), built from `specs/ux/research.md`; reference for the SwiftUI work and the PRG/PAL/PBK specs
- `subprojects/app`: SwiftPM app builds and runs on the owner's Mac (CLT only); offline banner, Fixtures + Groups windows, attribute bar (IPCGBES tabs, wheels, Fix ±1/Odd-Even/All/Clear, command line), 8 faders + GM. No Swift test target (XCTest/Testing absent from CLT)

## In progress
- VS2-12 in review (PR #23; owner run passed, merge pending); owner exports the church patch from Lightkey on 2026-10-18

## Blocked
- VS1-04 gate 3: the Eurolite MK2 is at the church; run the serial lines of `tests/hardware/output.md` on the next visit

## Hardware available for testing
- Behringer BCF2000 (use in Mackie Control mode)
- Eurolite USB-DMX512 PRO Interface MK2 (FTDI FT232R, ENTTEC Pro protocol, product string "Eurolite DMX512 Pro") — **at the church, not on the desk**; its hardware checks wait for a visit
- Mac (Apple Silicon), Command Line Tools instead of Xcode.app

## Repo
- Public: https://github.com/Aottens/ColourmotionDMX (one PR per task)

## Resume next session with
Start a fresh session (read PROJECT/STATE/ARCHITECTURE/DECISIONS, CLAUDE.md). Next, in dependency order: finish VS2-12 (merge PR #23; run `subprojects/engine/scripts/fetch-qlcplus.sh` once on a fresh clone), P1-14/P1-15 (specs for library browsing and multi-select in the patch table, owner 2026-10-17: phase 1), VS2-13 (church patch from the owner's Lightkey export, expected 2026-10-18; the owner may also hand over a Lightkey show file to decode). Still open: VS2-05 (Locate/Highlight/Off/Rem Dim/Clear options incl. freeze/release), P1-09 show file (needed for PAT-08 persistence), P1-12 GDTF importer spec, P1-13 per-cell selection spec, UX-02 (UI v0.1 layout, split first). Every PR updates README (CLAUDE.md step 5). The church is the playground, not the limit (owner): no church-driven feature cuts. Working notes: CI (`.github/workflows/ci.yml`) runs a newer stable Rust than the owner's Mac, so a clippy lint can fail only in CI — check the PR's CI before calling it done. The owner's disk is nearly full: `cargo clean` after a task, avoid needless release builds. Open owner question: read the two Eurolite IP PIX Strobe QLC+ files leniently (ignore the bad head channel with a warning) or keep refusing them (LIB-10)? IDEA-07 (double-click a wheel name = Off for that attribute) is built with VS2-05.
