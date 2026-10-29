# CMDMX — State

_Last updated: 2026-10-29 (FXB-03 effects in cues and palettes)_

This file is the handoff: where we are, what is next, what waits. It is not a log. What each task
built lives in its `BACKLOG.md` row and in the subproject logs (`subprojects/engine/STATE.md`,
`subprojects/app/STATE.md`); what works is in `README.md`.

## Current focus
Phase 1: a well-working general DMX console; first proof is a Sunday service in the church
instead of Lightkey (owner, 2026-10-22). Undo, Locate/Clear, multi-universe output, the MIDI + OSC
control layer, cue lists, group masters, an own TouchOSC layout (VS4-04, run on the church iPad passed), the app-crash check (VS1-08: output holds through `kill -9` of the app), library browsing, the profile editor, multi-select in the patch table
(VS2-27/28), Fan (VS2-35/36), emitter auto-mix with the Kelvin view and CTO/CTB steps (VS2-44..46, IDEA-32), LEE gel numbers (VS2-48/49; the table waits for LEE), GDTF import (VS2-41/42, a GDTF-share file imported in the owner's run) and cells end to end (VS2-30..34, 37, 38, 40: engine, protocol v1, Unfold, cell rows and Remove in the cue view) are done.

Feature work now: the FX generator (IDEA-18, phase 2 flagship, owner 2026-10-29): spec P2-01
(`effects.md`, ADR-011) approved 2026-10-29; build FXB-01..06, one PR each, in BACKLOG order.

Parallel track: phase 3, the open-hardware desk (IDEA-23), runs beside phase 1 in its own
sessions and PRs (`subprojects/hardware/`, DECISIONS 2026-10-25); it never touches engine or app code.
HWB-01, the USB-MIDI box firmware on a Pico 2, waits for parts and the owner's run; HWB-02 (playback strip) follows it.

## Next
0. FX generator: FXB-01 (core shapes), FXB-02 (programmer effects) and FXB-03 (effects in cues and
   palettes, show schema 8) done; next FXB-04 (IPC), then FXB-05 (the Shapes workspace), tests first.
1. VS8-01 and VS4-07: install `CMDMX-0.2.1.dmg` (the latest release) on the owner's Mac (About, shows came over from
   the old folder), then on the church Mac (`tests/hardware/release.md`, gate 3; not in the week of a
   service). The repo is renamed and the release is out.
2. At home on Art-Net: program the service in *Kerk* as `tests/hardware/church.md` asks (steady
   looks, *Front band*, *Front spreker*); the owner fills in which fresnels are band and speaker.
3. Sunday in the church: the two TouchOSC lines in `church.md`, then *Before the service* with the
   fallback rehearsal, and the service run (VS4-03) once the DMG is on the church Mac.
4. Releases build ad hoc until the owner joins the Apple Developer Program (VS8-09); then VS8-10
   (secrets, a `workflow_dispatch` ending `ok (notarised)`, the first notarised tag, the site announced).
   VS8-02 (logo) when the Colourmotion logo is found.
5. The owner's next *Import GDTF* run in `tests/manual/app-patch.md` also checks the VS2-43 sheet
   fixes (Became wraps, full text on hover, Escape closes).
6. Gel numbers (IDEA-02): core and gel book done (VS2-48/49); VS2-50 (the full LEE table) once LEE
   answers the owner's permission request.

## Waiting for a hardware run (gate 3, owner)
Rows with status `gate3` in `BACKLOG.md`:
- VS4-07 the DMG on the church Mac, and VS8-01 the renamed DMG on the owner's Mac (About, shows moved
  from the old folder), both in `tests/hardware/release.md`.
- VS1-07 BCF2000, `tests/hardware/bcf2000.md` (virtual BCF2000 passed in the VS7-04 run); postponed
  until the owner finds the BCF2000.
- VS8-08 notarised install, `tests/hardware/release.md` *Notarised install*: needs the Apple
  Developer account and the five repo secrets first (Owner to decide).
- HWB-01 USB-MIDI box, `tests/hardware/hw-box.md`: order the parts first (list at the top of the
  checklist), then flash, Learn on the Mac and tick.

Never run the Demo show on the real rig: its demo fixtures on 9–150 light real lamps.

## Blocked
- VS2-50 (the full LEE gel table) waits for LEE/Panavision's written permission; the owner sent the
  request through leefilters.com/contact on 2026-10-28 (answer to aottens@gmail.com).

## Owner to decide
- When to join the Apple Developer Program (postponed for cost, VS8-09); then for VS8-08/10: join the Apple Developer Program, as Colourmotion if it
  is a registered business (D-U-N-S number) or else as an individual; make the Developer ID
  Application certificate and an App Store Connect API key; set the five repo secrets (REL-20).
- The website's domain name: register it and tell Claude the name; it becomes one line in
  `subprojects/website/CNAME` (until then the site is `aottens.github.io/CMDMX`).
- Find the Colourmotion logo (VS8-02 builds the CMDMX concepts on it).
- IDEA-30 (cross-platform UI for the Linux desk): phase 3 after the surface; toolkit open.
- IDEA-29 (serial fade steps after VS4-05): confirm the phase.
- IDEA-10 (output lamps) and IDEA-24 (info message type): confirm the phase.
- Licence (`BACKLOG.md`, open questions; reopened by IDEA-21).

## Hardware available for testing
- Behringer BCF2000 (use in Mackie Control mode)
- Eurolite USB-DMX512 PRO Interface MK2 (FTDI FT232R, product string "FT232R USB UART", ENTTEC Pro label 6 at 250000 baud, ≤ 33 Hz, needs `--serial <path>`) — on the owner's desk since 2026-10-18 with six 1-channel dimmers (185–190) and an RGBL par (361–364)
- Elgato Stream Deck Original (first generation, USB `0fd9:0060`, 15 keys) — at home, on the owner's Mac (VS7-05); `verified` since the VS7-07 run; no Elgato app installed
- Mac (Apple Silicon), Command Line Tools instead of Xcode.app

## Repo
- Public: https://github.com/Aottens/CMDMX (one PR per task)

## Working notes
One task per PR; Sonnet subagents write the code, the main session writes spec and failing tests
and reviews. A task PR sets its own `BACKLOG.md` row to `done (date, #PR)` (or `gate3: <checklist>`)
and rewrites *Next* above; CI fails on a status outside the vocabulary. The owner likes to be
walked through manual runs step by step (one step per message, tick the run file in
`tests/manual/runs/`). CI runs a newer Rust than the Mac: check PR CI. Disk is nearly full:
`cargo clean` after a task. Don't run `git checkout` in the owner's tree while subagents work. The
two Eurolite IP PIX Strobe QLC+ files stay refused (owner, 2026-10-18). Manual runs use
`--show-dir /tmp/…` so the owner's real shows stay untouched.
