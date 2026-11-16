# CMDMX — State

The handoff, one screen: what is true now, what is next, what waits for whom. No history: what was built is in
`BACKLOG.md`, what works in `README.md`. See it live with `scripts/dashboard.py`. `scripts/check-tracking.py`
fails CI when this file drifts from `BACKLOG.md` (`specs/modules/tracking.md`).

## Now
- Phase 1: a well-working general DMX console. Its finish line is a real Sunday service on CMDMX (VS4-03).
- The church keeps running on Lightkey until Christmas, then on CMDMX (owner, 2026-11-06).
- IDEA-35 batches D (working on many at once), E (patch and profiles), F (controls and small things), G (show files),
  I (behaviour that differs from Titan) and J (input paths) are complete; schema 15 holds batch I's keys.
- Phase 10, the open-hardware desk (IDEA-23), runs beside phase 1 in its own PRs; HWB-01's parts are ordered.

## Next
1. The first service: VS4-07 and VS4-03 below, on the service candidate. The first stable release follows a service
   that went well, with its gap review's fixes (REL-26, DECISIONS 2026-11-16). P1-31 (Blind and Edit) stays apart.
2. The other proposals of the third gap review wait for approval (same file, *Proposed changes to the batches*).
3. Auto-review (IDEA-57, `specs/modules/autoreview.md`): green PRs can merge by themselves once the owner turns
   `AUTOMERGE` on (below); until then the gate runs in shadow. Fix blocks: FIX-08 first (the merge step must run
   `main`'s code), then FIX-04 (core), FIX-03, FIX-06 + FIX-07.

## Waiting on the owner
Runs only the owner can do (gate 3):
- VS4-03 the first service on CMDMX, Lightkey as fallback: pick a Sunday. It needs VS4-07 first and the programming
  below (`tests/hardware/church.md`); the same run checks FIL-03 on the Eurolite (Open keeps this Mac's output).
- VS4-07 the service candidate's DMG on the church Mac, not in the week of a service (`tests/hardware/release.md`).
- SPF-03 a real sleep during a running fade on the Eurolite and Art-Net (`tests/hardware/sleep.md`).
- VS8-08 the notarised install (`tests/hardware/release.md`): needs the Apple Developer account first.
- HWB-01 the USB-MIDI box (`tests/hardware/hw-box.md`): the parts are ordered, run it when they arrive.
- HWB-10 the screen flap (`tests/hardware/flap.md`): print it, have the walnut frame made and buy the parts in
  `subprojects/hardware/cad/klep/README.md` (the hinge is a €2 black one from a Dutch shop). The stand can be tested
  now with an acrylic stand-in for the glass; the rest needs the screen below.

Programming and small checks:
- Program the service in *Kerk* at home on Art-Net as `tests/hardware/church.md` asks (steady looks, *Front band*,
  *Front spreker*), and say which fresnels are band and speaker. Use the release that goes on the church Mac: a show
  saved with a newer schema does not open on an older one.
- With a Stream Deck or the BCF2000 on the desk, tick *Programming from a surface* and *Lock from a surface* in
  `tests/hardware/streamdeck.md`.
- Tick the two open lines in `tests/manual/selection-orders.md`; the next *Import GDTF* run in
  `tests/manual/app-patch.md` also checks the sheet fixes.
- HWB-11 the desk's top plate (`tests/hardware/top-plate.md`): the two test strips (1.5 mm engraving plate, 2 mm black
  cast acrylic) are ordered at Laserlokaal. Print both backings in `acryl-proef/onderframe/` now; when the strips
  arrive, screw the 1.5 mm one on, push, and say which backing is stiff enough, which MX hole clips, which legends read.

## Blocked
- VS2-50 (the full LEE gel table): waits for LEE/Panavision's written permission, asked on 2026-10-28.

## Owner to decide
- Auto-merge, only after FIX-08 (set no `AUTOMERGE_TOKEN` before it): read the shadow run of fifteen PRs in #333,
  then turn it on with the steps in that PR (auto-merge, the token secret, a ruleset, `AUTOMERGE=on`), or wait.
- REL-26 names no patch series for the release installed now, the one before the service candidate
  (`specs/modules/release.md:41`, review of #327): name one, or say it gets none.
- Buy the screen for HWB-10's run, the desk's screen flap: Waveshare 15.6" HDMI LCD (H), about €170–240 from Eckstein or
  Welectron (Germany, EU stock, VAT included, no import).
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
