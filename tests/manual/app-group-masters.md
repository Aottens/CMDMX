# App group masters checklist (GRM-15..18) — template

No Swift test target exists (CLAUDE.md), so the Group Master softkey, the strips and the Master
tab of `specs/modules/group-masters.md` GRM-16..17 (and `ui.md` as amended for VS3-07) are
checked by hand; GRM-18 lists what this run walks. Copy to `tests/manual/runs/<date>-<task>.md`
and tick it there. This run also finishes VS3-01's demo: the front spots on their own fader next
to the service cue list.

Setup: two terminals, and a fresh shows folder so your real one stays untouched.
- `rm -rf /tmp/cmdmx-grm`
- Daemon: `cd subprojects/engine && cargo run -p cmdmx-daemon -- --no-sacn --show-dir /tmp/cmdmx-grm`.
- App: `cd subprojects/app && swift build && scripts/bundle-app.sh && open .build/ColourmotionDMX.app`
  (or the copy in `~/Applications` when Claude drives the run).
- Demo show: church rig (washes 101–104, pars 201–226, front spots 301–306); group 4 = the front
  spots, group 3 = the pars. Pages 1 and 2 hold the demo playbacks, page 3 is empty. 3.8 stands
  in for the church's 1.8 (the demo uses 1.8).

Output cannot be seen without lamps: Claude listens to the daemon's Art-Net on universe 1 during
the run and reports the front spots' and pars' levels.

Who does what (owner, 2026-10-22): the run is how the owner learns the feature, but not every
click. **[Claude]** steps (setup, building show data, repeats, checks, the restart) Claude does
by computer use and IPC before or between the owner's steps; **[Jij]** steps the owner does by
hand, one per message, with Claude reading Art-Net.

## The service list and the front-spot master (GRM-01, 03, 05, 07, 16; VS3-01)
- [ ] [Claude] Page 3. Front spots 301–306 at 80 %, Record, tap 3.1: a memory. Clear. Spots at 40 %,
      Record, tap 3.1, Append: 3.1 is a list Q1 (80 %), Q2 (40 %). Clear. Raise 3.1: Q1 fires,
      Claude reads the spots at 80 %.
- [ ] [Jij] Record: the softkeys (nothing tapped yet) show **BPM Master**, **Rate Master** and
      **Group Master**. Group Master → the prompt asks for a group; tap group 4 → the prompt asks
      for an empty handle; tap 3.8. The strip reads "Group 4" with the meta line
      "GRP · SCALE 100 %", the buttons — / FLASH, the level bar at 100 %. Nothing changes on stage.
- [ ] [Claude] Group Master, then a used handle (3.1): refused with a message, nothing recorded. Record
      onto 3.8 with a selection: refused ("Handle 3.8 holds the master of group 4").
- [ ] [Claude] Playback Options on 3.8 (right-click its legend): Handle and Master tabs only. On Handle,
      set paging `Locked`.
- [ ] [Jij] Pull 3.8 to 0: the level shows 0 % in amber; Claude reads the spots at 0 ("out for the
      video"). GO on the list (Q2): the spots stay at 0.
- [ ] [Jij] Push 3.8 back to full: Claude reads the spots at 40 % (where the list has them now, not
      80 %); the amber goes.
- [ ] [Claude] Page 1: the master is on slot 8 (Locked). Pull it to 50 %: the spots at 20 %. Back to full.

## Scale 200 % (GRM-07, 17)
- [ ] [Claude] Master tab: mode Scale, scale picker 200 %. The meta line reads "GRP · SCALE 200 %". 3.8 at
      50 %: the spots at 40 % (unchanged); at full: 80 %. Scale back to 100 %.

## HTP master as a submaster (GRM-08)
- [ ] [Jij] Group Master on group 3 (pars) onto 3.7. Master tab: mode HTP; the scale picker is gone,
      the meta line reads "GRP · HTP", the level bar at 0 %. Raise 3.7 to 60 %: the level in amber,
      Claude reads the pars at 60 % with no playback holding them. Back to 0: pars out.

## Limit under a full programmer value (GRM-09)
- [ ] [Claude] Mode Limit on 3.7 ("GRP · LIMIT"), 3.7 at 30 % (amber). Pars 201–204 @ Full in the
      programmer: Claude reads 201–204 at 30 %. Clear; 3.7 back to full.

## Take Over (GRM-10, 12)
- [ ] [Jij] 3.8 mode Take Over ("GRP·TAKE OVER", no amber, the FLASH button refused with a
      message); the mode change puts it at 100 % (GRM-20). With the list at Q2 (spots 40 %),
      pull 3.8 down slowly: nothing changes until it passes 40 %, then the spots are in the
      programmer (Programmer window) and follow the fader down. Clear; mode back to Scale.

## Disabled (GRM-11, 16)
- [ ] [Jij] 3.8 at 50 % (spots at 20 %). Master tab: Disabled on: the meta line adds "LOCKED", the
      level bar stays at 50 %. Drag 3.8 to 0 and to full: the bar and the spots do not move;
      FLASH is refused. Disabled off with the fader at full: the spots jump to 40 %.

## Flash (GRM-12)
- [ ] [Claude] 3.8 at 0 (spots out). Hold FLASH: the spots at 40 % while held, out again on release.

## Two masters on one fixture (GRM-01, 06)
- [ ] [Claude] A second Group Master on group 4 onto 3.6 (Scale). 3.8 at 50 % and 3.6 at 50 %: the spots
      at 10 % (40 % × 0.5 × 0.5). Both back to full.

## Restart (GRM-05, 13, 14)
- [ ] [Claude] 3.8 at 50 %, 3.7 Limit at 30 %. Wait for the autosave (status bar), quit the daemon
      (Ctrl+C) and start it again: the app reconnects, 3.8 shows 50 %, 3.7 "GRP · LIMIT" at 30 %,
      3.6 and the list are back; Claude reads the stage as before the restart.

## Result
- [ ] Passed / findings recorded below.

Findings:
