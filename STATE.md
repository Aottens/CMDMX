# CMDMX — State

The handoff, one screen: what is true now, what is next, what waits for whom. No history: what was built is in
`BACKLOG.md`, what works in `README.md`. See it live with `scripts/dashboard.py`. `scripts/check-tracking.py`
fails CI when this file drifts from `BACKLOG.md` (`specs/modules/tracking.md`).

## Now
- Phase 1: a well-working general DMX console. Its finish line is a real Sunday service on CMDMX (VS4-03).
- The church keeps running on Lightkey (owner, 2026-11-05). The first service on CMDMX has no date yet.
- IDEA-35 batch B, changing what is recorded, is released, and its gap review is taken into the batches.
- Phase 10, the open-hardware desk (IDEA-23), runs beside phase 1 in its own PRs and waits for HWB-01's parts.

## Next
1. HLP-01, HLP-02, HLP-03: Help opens the manual, offline, and a chapter on taking over a show.
2. The batches after B, in the owner's order: C to G, then I and J (same file). Each needs its spec approved first.
   P1-31 (Blind and Edit) stays apart from them. C now starts with L1 and J8.
3. The proposals of the third gap review wait for approval (same file, *Proposed changes to the batches*).

## Waiting on the owner
Runs only the owner can do (gate 3):
- VS4-03 the first service on CMDMX, Lightkey as fallback: pick a Sunday. It needs VS4-07 first and the programming
  below (`tests/hardware/church.md`).
- VS4-07 the DMG on the church Mac, not in the week of a service; VS8-01 the DMG on the owner's own Mac, which still
  has the app from before the rename (`tests/hardware/release.md`).
- SPF-03 a real sleep during a running fade on the Eurolite and Art-Net (`tests/hardware/sleep.md`).
- VS1-07 the BCF2000 (`tests/hardware/bcf2000.md`): postponed until it is found.
- VS8-08 the notarised install (`tests/hardware/release.md`): needs the Apple Developer account first.
- HWB-01 the USB-MIDI box (`tests/hardware/hw-box.md`): order the parts first.

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
- The website's domain name. Find the Colourmotion logo (VS8-02).
- Phases of IDEA-10, IDEA-24, IDEA-29, IDEA-30, IDEA-37, IDEA-38 and IDEA-39.
- Licence (`BACKLOG.md`, open questions; reopened by IDEA-21).

## Good to know
- Never run the Demo show on the real rig: its demo fixtures on 9–150 light real lamps.
- On the desk: Eurolite USB-DMX512 PRO MK2 (`--serial <path>`) with six dimmers (185–190) and an RGBL par (361–364);
  Stream Deck Original (15 keys); a Mac with Command Line Tools only. The BCF2000 is missing.
- Manual runs use `--show-dir /tmp/…`, so real shows stay untouched. The owner is walked through them step by step.
- CI runs a newer Rust than the Mac: check PR CI. Disk is nearly full: `cargo clean` after a task.
- No `git checkout` in the owner's tree while subagents work.
