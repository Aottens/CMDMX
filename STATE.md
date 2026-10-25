# ColourmotionDMX — State

_Last updated: 2026-10-25 (hardware track set up beside phase 1)_

This file is the handoff: where we are, what is next, what waits. It is not a log. What each task
built lives in its `BACKLOG.md` row and in the subproject logs (`subprojects/engine/STATE.md`,
`subprojects/app/STATE.md`); what works is in `README.md`.

## Current focus
Phase 1: a well-working general DMX console; first proof is a Sunday service in the church
instead of Lightkey (owner, 2026-10-22). Undo, Locate/Clear, multi-universe output, the MIDI + OSC
control layer, cue lists, group masters, an own TouchOSC layout (VS4-04, run on the church iPad passed), the app-crash check (VS1-08: output holds through `kill -9` of the app), library browsing, the profile editor, multi-select in the patch table
(VS2-27/28) and Fan (VS2-35/36) are done.

Parallel track: phase 3, the open-hardware desk (IDEA-23), runs beside phase 1 in its own
sessions and PRs (`subprojects/hardware/`, DECISIONS 2026-10-25); it never touches engine or app code.

## Next
1. VS2-32..34 (cells: storage PRG-52/53, protocol v1 PRG-55, app Unfold; VS2-33 raises the
   protocol to v1 and its run checks the VS1-09 mismatch line). Cells already merge (VS2-31) but
   their values are not recorded until VS2-32.
2. Next Sunday in the church: the two TouchOSC lines in `tests/hardware/church.md` (Lightkey
   answers the iPad again; lag on the church wifi), and if it holds, the service run (VS4-01, VS4-03).

## Waiting for a hardware run (gate 3, owner)
Rows with status `gate3` in `BACKLOG.md`:
- VS1-07 BCF2000, `tests/hardware/bcf2000.md` (virtual BCF2000 passed in the VS7-04 run); postponed
  until the owner finds the BCF2000.

Never run the Demo show on the real rig: its demo fixtures on 9–150 light real lamps.

## Blocked
- Nothing.

## Owner to decide
- HW-01: the eleven open questions of `docs/research/hardware-desk.md` (hardware track).
- IDEA-29 (serial fade steps after VS4-05): confirm the phase.
- IDEA-10 (output lamps) and IDEA-24 (info message type): confirm the phase.
- Licence (`BACKLOG.md`, open questions; reopened by IDEA-21).

## Hardware available for testing
- Behringer BCF2000 (use in Mackie Control mode)
- Eurolite USB-DMX512 PRO Interface MK2 (FTDI FT232R, product string "FT232R USB UART", ENTTEC Pro label 6 at 250000 baud, ≤ 33 Hz, needs `--serial <path>`) — on the owner's desk since 2026-10-18 with six 1-channel dimmers (185–190) and an RGBL par (361–364)
- Elgato Stream Deck Original (first generation, USB `0fd9:0060`, 15 keys) — at home, on the owner's Mac (VS7-05); `verified` since the VS7-07 run; no Elgato app installed
- Mac (Apple Silicon), Command Line Tools instead of Xcode.app

## Repo
- Public: https://github.com/Aottens/ColourmotionDMX (one PR per task)

## Working notes
One task per PR; Sonnet subagents write the code, the main session writes spec and failing tests
and reviews. A task PR sets its own `BACKLOG.md` row to `done (date, #PR)` (or `gate3: <checklist>`)
and rewrites *Next* above; CI fails on a status outside the vocabulary. The owner likes to be
walked through manual runs step by step (one step per message, tick the run file in
`tests/manual/runs/`). CI runs a newer Rust than the Mac: check PR CI. Disk is nearly full:
`cargo clean` after a task. Don't run `git checkout` in the owner's tree while subagents work. The
two Eurolite IP PIX Strobe QLC+ files stay refused (owner, 2026-10-18). Manual runs use
`--show-dir /tmp/…` so the owner's real shows stay untouched.
