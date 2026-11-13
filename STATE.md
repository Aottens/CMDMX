# CMDMX — State

The handoff, one screen: what is true now, what is next, what waits for whom. No history: what was built is in
`BACKLOG.md`, what works in `README.md`. See it live with `scripts/dashboard.py`. `scripts/check-tracking.py`
fails CI when this file drifts from `BACKLOG.md` (`specs/modules/tracking.md`).

## Now
- Phase 1: a well-working general DMX console. Its finish line is a real Sunday service on CMDMX (VS4-03).
- The church keeps running on Lightkey until Christmas, then on CMDMX (owner, 2026-11-06).
- IDEA-35 batches D (working on many at once), E (patch and profiles) and F (controls and small things) are complete.
- Phase 10, the open-hardware desk (IDEA-23), runs beside phase 1 in its own PRs; HWB-01's parts are ordered.

## Next
1. IDEA-35 batch G, show files (spec approved): builds FIL-01 + FIL-02 + FIL-03, FIL-04 + FIL-05, then FIL-06
   (M1 first, a New that keeps groups and palettes). Then batches I and J in the owner's order
   (`docs/research/everyday-gaps.md`), each spec approved first. P1-31 (Blind and Edit) stays apart from them.
2. The other proposals of the third gap review wait for approval (same file, *Proposed changes to the batches*).
3. Auto-review (IDEA-57, `specs/modules/autoreview.md`): Claude reviews every PR on GitHub and the risk gate labels it,
   in shadow; after five PRs through the gate, ARB-03 lets green PRs merge by themselves.

## Waiting on the owner
Runs only the owner can do (gate 3):
- VS4-03 the first service on CMDMX, Lightkey as fallback: pick a Sunday. It needs VS4-07 first and the programming
  below (`tests/hardware/church.md`).
- VS4-07 the DMG on the church Mac, not in the week of a service; VS8-01 the DMG on the owner's own Mac, which still
  has the app from before the rename (`tests/hardware/release.md`).
- SPF-03 a real sleep during a running fade on the Eurolite and Art-Net (`tests/hardware/sleep.md`).
- VS8-08 the notarised install (`tests/hardware/release.md`): needs the Apple Developer account first.
- HWB-01 the USB-MIDI box (`tests/hardware/hw-box.md`): the parts are ordered, run it when they arrive.

For auto-review: GitHub refuses the `CLAUDE_CODE_OAUTH_TOKEN` secret (401, invalid bearer token). Run
`claude setup-token` again, store only the token it prints with `gh secret set CLAUDE_CODE_OAUTH_TOKEN`. Claude never
handles the token.

Programming and small checks:
- Program the service in *Kerk* at home on Art-Net as `tests/hardware/church.md` asks (steady looks, *Front band*,
  *Front spreker*), and say which fresnels are band and speaker. Use the release that goes on the church Mac: a show
  saved with a newer schema does not open on an older one.
- Tick the two open lines in `tests/manual/selection-orders.md`; the next *Import GDTF* run in
  `tests/manual/app-patch.md` also checks the sheet fixes.

## Blocked
- VS2-50 (the full LEE gel table): waits for LEE/Panavision's written permission, asked on 2026-10-28.

## Owner to decide
- Apple Developer Program: when, and as Colourmotion or as an individual; unlocks VS8-08 and VS8-10.
- The dashboard's port: it shares 7320 with the phone remote's default (TRB-03).
- The website's domain name. Find the Colourmotion logo (VS8-02).
- Phases of IDEA-10, IDEA-24, IDEA-29, IDEA-30, IDEA-37, IDEA-45, IDEA-47, IDEA-48, IDEA-49 and IDEA-56.
- Licence (`BACKLOG.md`, open questions; reopened by IDEA-21).

## Good to know
- Never run the Demo show on the real rig: its demo fixtures on 9–150 light real lamps.
- On the desk: Eurolite USB-DMX512 PRO MK2 (`--serial <path>`) with six dimmers (185–190) and an RGBL par (361–364);
  Stream Deck Original (15 keys); a Mac with Command Line Tools only.
- The BCF2000 is back, over DIN MIDI through the H4MIDI-WC: its USB output does not work on this Mac.
- Manual runs use `--show-dir /tmp/…`, so real shows stay untouched. The owner is walked through them step by step.
- The app has a CPU budget (UI-70): `scripts/app-cpu.sh` in every app PR; live values go on layers, not SwiftUI (UI-71).
- CI runs a newer Rust than the Mac: check PR CI. Disk is nearly full: `cargo clean` after a task.
- No `git checkout` in the owner's tree while subagents work.
