# ColourmotionDMX — State

_Last updated: 2026-10-25 (VS2-30)_

This file is the handoff: where we are, what is next, what waits. It is not a log. What each task
built lives in its `BACKLOG.md` row and in the subproject logs (`subprojects/engine/STATE.md`,
`subprojects/app/STATE.md`); what works is in `README.md`.

## Current focus
Phase 1: a well-working general DMX console; first proof is a Sunday service in the church
instead of Lightkey (owner, 2026-10-22). Undo, Locate/Clear, multi-universe output, the MIDI + OSC
control layer, cue lists, group masters, library browsing, the profile editor and multi-select in the patch table
(VS2-27/28) are done.

## Next
1. The owner picks the next task; candidates: VS4-04 (own TouchOSC Mk1 layout; iPad facts from the
   2026-10-25 church run), VS2-31..34 (per-cell selection, PRG-47, 51..55; VS2-30's cell items are in the core; VS2-33
   raises the protocol to v1 and its run checks the VS1-09 mismatch line), VS7-05..08 (Stream Deck,
   CTL-44..58, ADR-009; VS7-05 first), VS2-35..36 (Fan, PRG-56..62; VS2-35 first).
2. The church hardware runs below, when the hardware is at hand.

## Waiting for a hardware run (gate 3, owner)
Rows with status `gate3` in `BACKLOG.md`:
- VS1-04 / VS4-02 Eurolite serial lines, `tests/hardware/output.md`: mid-run unplug (OUT-12) and the
  fade steps are left; VS4-05 `tests/hardware/runs/2026-10-21-VS4-05.md` compares a fade on the RGBL
  par at 361 with Lightkey at ~33 Hz.
- VS1-07 BCF2000, `tests/hardware/bcf2000.md` (virtual BCF2000 passed in the VS7-04 run).
- VS2-13 church patch, `tests/hardware/church.md` (run `tests/hardware/runs/2026-10-18-church.md`).
- VS6-03 Eurolite steps of `tests/manual/runs/2026-10-22-VS6-03.md`, in the church.

Never run the Demo show on the real rig: its demo fixtures on 9–150 light real lamps.

## Blocked
- Nothing.

## Owner to decide
- Which Stream Deck model the owner has, for the VS7-07 gate-3 run (`tests/hardware/streamdeck.md`).
- IDEA-10 (output lamps) and IDEA-24 (info message type): confirm the phase.
- Licence (`BACKLOG.md`, open questions; reopened by IDEA-21).

## Hardware available for testing
- Behringer BCF2000 (use in Mackie Control mode)
- Eurolite USB-DMX512 PRO Interface MK2 (FTDI FT232R, product string "FT232R USB UART", ENTTEC Pro label 6 at 250000 baud, ≤ 33 Hz, needs `--serial <path>`) — on the owner's desk since 2026-10-18 with six 1-channel dimmers (185–190) and an RGBL par (361–364)
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
