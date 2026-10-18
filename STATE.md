# ColourmotionDMX — State

_Last updated: 2026-10-18_

## Current focus
Vertical slice 1: one hand-written fixture → patch on universe 1 → one fader in the app → Art-Net + sACN out → Eurolite MK2 over serial → BCF2000 fader moves it with motor feedback.

## Working
- Church patch (VS2-13, merged #35): `patch.md` PAT-15..18 approved (owner, 2026-10-18); `cargo run -p cmdmx-daemon --example church -- ~/Documents/ColourmotionDMX/Lightkey/BGS_Shows.tsv` writes the show *Kerk* (56 fixtures, numbers in blocks of 100 per type: 101–120 Showtec pars … 1201) and 9 user profiles, never overwrites; checklist `tests/hardware/church.md` (gate 3). Dry run into a temp folder opened in the daemon without warnings. The Lightkey files stay out of git
- Show file spec `specs/modules/showfile.md` SHW-01..17 approved (P1-09, 2026-10-18): autosave in place, shows in `~/Documents/ColourmotionDMX/Shows/`, live state not saved. SHW-a (core, #30) and SHW-b (daemon: shows folder, last show at start with Demo fallback, autosave in place on a saver thread, `.bak`, `.tmp` recovery, `show.*` over IPC, #31) merged; SHW-c (app: show name + autosave age in the status bar, Show screen with Open/New/Save As) merged (#32)
- Repo scaffold: meta-layer, ADR-001..004, PRD, module specs (3 written, 3 skeleton)
- `cmdmx-core`: universe buffer, patch of a dimmer/generic fixture, HTP/LTP merge, playback level → DMX frame; scenario test runner over `tests/scenarios/*.json`
- `cmdmx-protocols`: Art-Net ArtDmx, sACN E1.31 data packet, ENTTEC Pro "send DMX" frame — all byte-exact tested
- `cmdmx-control`: Mackie Control decode/encode for fader pitchbend and button notes (unit tested)
- `cmdmx-daemon`: 40 Hz tick loop; sends line 1 as Art-Net (VS1-02 done) and sACN universe 1 multicast, priority 100 (VS1-03 done), and ENTTEC Pro serial at 250000 baud, ≤ 1 write per 30 ms, product-string detection (the owner's Eurolite MK2 needs `--serial <path>`) and hotplug (VS1-04, verified on the MK2 except mid-run unplug, VS4-02), each node on its own thread. IPC v0 over WebSocket on 127.0.0.1:7319 (VS1-05 done); the app's faders drive DMX
- ADR-006 accepted, `fixture-library.md` LIB-01..17 and `patch.md` PAT-01..14 approved; build tasks VS2-08..13 in BACKLOG. LIB-17 (owner, 2026-10-17): emitters are generic — recognised by an `emitter` flag, never by name, any number (Amber, Warm/Cold White, UV, Royal Blue, …)
- Patch over IPC + Patch screen (VS2-12, merged): snapshot `fixtures` entries carry number, name, profile, mode, universe.address, footprint, virtual dimmer and handle, a `fixtures` delta after any patch change, `patch.add/move/remove/renumber/name/change_mode/handle` (PAT-09); library fixtures are patched under `Manufacturer/Model`, fetched from the library in the client thread; the app has a workspace rail (Program / Patch) and a Patch screen with the table, an Add panel with library search and row edits with two-step confirm for Remove and Change Mode (PAT-10). Owner run passed: `tests/manual/runs/2026-10-17-VS2-12.md` (findings → IDEA-08 browse by manufacturer, IDEA-09 multi-select)
- Library (VS2-11, merged): the QLC+ set (1781 fixtures, pinned, fetched by `subprojects/engine/scripts/fetch-qlcplus.sh`, not in git) indexed in the background at daemon start; word search, user profiles that hide the QLC+ one, profiles from a channel list (LIB-11), edited profiles saved atomically (LIB-12), `library.*` over IPC (LIB-14); 1779 of 1781 QLC+ files import
- QLC+ importer (VS2-09, merged): `cmdmx_core::library::qlcplus::import(xml, source)` → profile + import report (`quick-xml`); presets, groups, colour hints, fine channels, heads → cells, slots with sRGB colours, defaults, physical ranges; LIB-18 conventions (owner, 2026-10-17); the Shehds 19x15, Showtec LED Par 56 and LumiPar 7QPro files (QLC+ commit 3d832ce, Apache-2.0) reproduce their expected profiles
- Patch core (VS2-10, merged): engine-assigned internal ids never reused, user numbers on the command line (PAT-12), `patch_add` with auto address/number and all-or-nothing clash checks ("226 would overlap 301 at 1.67"), move, remove, renumber, change mode (drops what the new mode lacks, with warnings), handles in pages of `handles_per_page` (default 40)
- Colour in the app (VS2-14, merged; owner run `tests/manual/runs/2026-10-17-VS2-14.md` passed): demo rig = church rig with RGBW washes, RGBW / RGB+Lime / RGB (virtual dimmer) pars and front spots (IPC-14 amended); `programmer.colour_model` (IPC-18); HSI/RGB/CMY buttons and Hue in degrees in the app (IPC-19)
- Colour as one value (VS2-06, merged): `Value::Level | Value::Colour` through programmer, playbacks and merge; HSI (default) / RGB / CMY wheels as views of one target colour, emitter wheels; nominal RGB/CMY mapping; virtual dimmer scales colour and emitters (GM included); Colour defaults to open white and LTP values are held; cells follow the fixture's Colour/White/Dimmer (owner decisions 2026-10-17)
- Profile model v2 (VS2-08, merged): `cmdmx-core::profile` with modes, cells, colour mixing system, emitter flag, defaults/locate/highlight, slots, physical ranges; validated on load (LIB-02, 17); fixtures are patched in a mode; unproposed LTP attributes output their default (LIB-03); scenario `{attributes}` profiles still load
- ADR-005 accepted: one BT.2020 linear target colour per fixture, White/Lime as direct attributes, nominal mapping in phase 1
- Programmer spec `specs/modules/programmer.md` PRG-01..37 approved (P1-02); tasks VS2-02..07 in BACKLOG
- `cmdmx-core::programmer`: ordered selection, groups, Fix ±1 stepping, patterns, All, record group (VS2-02, PRG-01..07, 14); command line `Fixture/Group/Thro/And/Not/@/Enter` (VS2-03, PRG-08/09/11); programmer values in the merge at Programmer priority, relative wheels Normal/Fine/Coarse, wheel layout and active group, `contents(mask)` (VS2-04); over IPC with the church demo rig, and in the app: fixtures + groups windows, attribute bar with wheels, command line (VS2-07, IPC-10..17; owner run `tests/manual/runs/2026-10-17-VS2-07.md` passed). HTP without a source now outputs 0 (ENG-07 part)
- Main-screen spec `specs/modules/ui.md` UI-01..28 + IPC-20..23 approved (UX-02.0, 2026-10-17; UI-16/19 amended, UI-27/28 added 2026-10-18); UX-02a (shell), UX-02b, UX-02c (side column, #33) and UX-02d + VS1-06 (playback area, pages of playbacks PBK-01..05, #40) merged
- Palettes done (P1-03, PAL-01..32): core (VS2-17, #41), fades and overlap (VS2-18, #42), IPC + app (VS2-19, #43; owner run `tests/manual/runs/2026-10-18-VS2-19.md` passed): palette windows on handles, Record/Update/Include/Copy/Move/Delete, legend editor with icons, Palette softkeys, cyan references and fade progress in the Programmer window. Known points: the palette windows scroll on the laptop (check on the console screen); live output during fades waits for IDEA-10 (output lamps); IDEA-15 (selection orders, random, 1 in 3) is phase 2
- UI v0.1: clickable main-screen sketch `specs/ux/main-screen.html` (UX-01 done; Artifact https://claude.ai/artifact/YNEG3JsZAnjWhG1793MDUd), built from `specs/ux/research.md`; reference for the SwiftUI work and the PRG/PAL/PBK specs
- `subprojects/app`: SwiftPM app builds and runs on the owner's Mac (CLT only); offline banner, Fixtures + Groups windows, attribute bar (IPCGBES tabs, wheels, Fix ±1/Odd-Even/All/Clear, command line), 8 faders + GM. No Swift test target (XCTest/Testing absent from CLT)

## In progress
- P1-04 playbacks: `specs/modules/playbacks.md` PBK-06..45 approved (owner, 2026-10-18; all seven proposals: Record By Channel, the mode 0/1 time model, fixed Swop/Flash/Latch, automatic halo, attribute-group times and chases with BPM/Rate masters in P1-08, group masters P1-16, Titan release defaults). VS2-20 (core storage: handles and locks, record, Copy/Move/Delete, options, legends/icons/halos, show schema 3 with migration) in review; next VS2-21 (behaviour on the frame) and VS2-22 (IPC + app)
- VS4-02 (#34) and VS2-13 (#35) merged; *Kerk* is in the owner's Shows folder (56 fixtures, 9 user profiles). Run `tests/hardware/runs/2026-10-18-church.md`: the Patch-screen line passed at home; the desk lines wait for the Eurolite, the rest for the church
- VS2-16 merged (#38): snapshot `fixtures` entries carry `modes` (PAT-09 amended), Change Mode is a dropdown of them (PAT-10 amended); owner run `tests/manual/runs/2026-10-18-VS2-16.md` passed

## Blocked
- VS4-02 / VS1-04 gate 3 (Eurolite MK2, on the owner's desk): run `tests/hardware/runs/2026-10-18-VS4-02.md`
  passed except two lines. The MK2 needs 250000 baud (OUT-20), is not auto-detected (FTDI default string
  "FT232R USB UART", no reply to ENTTEC requests; the log names it with a `--serial` hint, OUT-23), and flickers
  an LED par at 40 Hz, so serial writes at most every 30 ms (OUT-24, 20 Hz). Left: mid-run unplug (OUT-12),
  and "some visible steps" in a dragged fade (20 Hz, or the par's low end? try a 33 Hz own timer, compare
  with Lightkey). Never run the Demo show on the real rig: its demo fixtures on 9–150 lit real lamps.

## Hardware available for testing
- Behringer BCF2000 (use in Mackie Control mode)
- Eurolite USB-DMX512 PRO Interface MK2 (FTDI FT232R, product string "FT232R USB UART", ENTTEC Pro label 6 at 250000 baud, ≤ 33 Hz) — on the owner's desk since 2026-10-18 with six 1-channel dimmers (185–190) and an RGBL par (361–364)
- Mac (Apple Silicon), Command Line Tools instead of Xcode.app

## Repo
- Public: https://github.com/Aottens/ColourmotionDMX (one PR per task)

## Resume next session with
Start a fresh session (read PROJECT/STATE/ARCHITECTURE/DECISIONS, CLAUDE.md). The app now runs in
its real main screen (`specs/modules/ui.md`, UI-01..28; README screenshot); the side column, keypad, token command line, Mac keys and command history work (UX-02c). The show file is
done (SHW-01..17, SHW-a/b/c): the daemon opens the last show from
`~/Documents/ColourmotionDMX/Shows/` and autosaves it in place; the app has a Show screen. Manual
runs use `--show-dir /tmp/…` so the owner's real shows stay untouched. Next, owner's choice:
- **VS2-13** gate 3: finish `tests/hardware/runs/2026-10-18-church.md` (desk lines with the Eurolite, the rest at the church).
- **VS2-20** in review; then VS2-21 (fader modes, times, release, Flash/Swop, DBO on the frame: scenarios first) and VS2-22 (IPC + app). After that P1-08 cue lists; IDEA-10 (output lamps) shows live fades.
- Specs waiting: P1-14 (library browsing + rethink of the Add panel, sketch first), P1-15
  (multi-select in the patch table), P1-08 cue lists, P1-05 control surfaces.
Working notes: one task per PR; Sonnet subagents write the code, the main session writes spec and
failing tests and reviews. The owner likes to be walked through manual runs step by step (one
step per message, tick the run file in `tests/manual/runs/`). CI runs a newer Rust than the Mac:
check PR CI. Disk is nearly full: `cargo clean` after a task. Don't run `git checkout` in the
owner's tree while subagents work. The two Eurolite IP PIX Strobe QLC+ files stay refused (owner, 2026-10-18). IDEA-10 (output
lamps) phase still to confirm; IDEA-13 (slot names + defaults editor, ONYX as reference) is phase 1 with P1-14 (owner, 2026-10-18). Open point from the SHW-c run: engine messages are all amber, also
good news ("autosave … works again"); a calmer info colour needs an `info` message type.
