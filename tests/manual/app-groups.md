# App groups checklist (PRG-85..95) — template

No Swift test target exists (CLAUDE.md), so the Groups window's function buttons of
`specs/modules/programmer.md` PRG-90 and PRG-94 (and `ui.md` UI-12, UI-16) are checked by hand;
PRG-95 lists what this run walks. Copy to `tests/manual/runs/<date>-<task>.md` and tick it there.

Setup: a fresh shows folder so your real one stays untouched.
- `rm -rf /tmp/cmdmx-grp`
- Daemon: `cd subprojects/engine && cargo run -p cmdmx-daemon -- --no-sacn --show-dir /tmp/cmdmx-grp`.
- App: `cd subprojects/app && swift build && scripts/bundle-app.sh && open .build/CMDMX.app`
  (or the copy in `~/Applications` when Claude drives the run).
- Demo show: church rig (washes 101–104, pars 201–226, front spots 301–306).

Output cannot be seen without lamps: Claude listens to the daemon's Art-Net on universe 1 during
the run and reports the front spots' levels.

Who does what (owner, 2026-10-22): **[Claude]** steps Claude does by computer use and IPC before
or between the owner's steps; **[Jij]** steps the owner does by hand, one per message, with
Claude reading Art-Net. Every step message shows done/total ("Stap 3/8", the [Jij] steps count).

## Record and Set Legend (PRG-90, UI-12)
- [ ] [Claude] Pars 201–208 selected, Record, tap group 5. 221–226, Record, group 8. Front spots
      301–306, Record, group 6. The cells read "Group 5", "Group 8", "Group 6" with their
      fixture counts.
- [ ] [Jij] Set Legend, tap group 5: the editor opens with the text field only (no icon library,
      no number). Type "Pars outside in", Set. The cell reads "Pars outside in".
- [ ] [Claude] Tap group 5: it selects, the message line reads "Group 5 Pars outside in". Tap group 8:
      "Group 8". Type `Group 5 Enter`: the same message (PRG-96). `Group 7 Enter` on an empty cell, with
      fixtures selected: "Group 7 is empty", the selection stays.
- [ ] [Claude] Set Legend on group 5, empty the field, Set: the cell reads "Group 5" again. Undo (label
      "Legend group 5"): "Pars outside in" is back. Set Legend on an empty cell: "Group 7 is empty".
- [ ] [Claude] Set Legend armed, tap group 8: the selection does not change. Clear disarms: the
      function button goes out, a tap selects again.

## Copy and Move (PRG-87..89)
- [ ] [Jij] Copy, tap group 5, tap empty cell 9. Group 9 is 201–208 with the legend "Pars outside
      in"; group 5 is unchanged, and the selection did not change.
- [ ] [Claude] Undo (label "Copy group 5"): cell 9 is empty.
- [ ] [Jij] Move, tap group 8, tap empty cell 10. Cell 8 is empty, cell 10 holds it.
- [ ] [Claude] Undo (label "Move group 8"): it is back on 8.
- [ ] [Claude] Copy, tap 5, tap used cell 6: refused with "Group 6 is used" on the message line, nothing
      changes. Same for Move. Copy, tap 5, tap 5: refused. Copy, tap empty cell 7: asks for a
      recorded group as the source. Clear in the middle of Copy: disarmed, nothing sent.

## Move under a group master (PRG-88)
- [ ] [Claude] Record → Group Master, tap group 6, tap an empty handle (say 3.8). Front spots 301–306 at
      80 % in a playback on 3.1, raised. Claude reads them at 80 %.
- [ ] [Jij] Pull 3.8 to 0: Claude reads the spots at 0. Move, tap group 6, tap empty cell 12. Cell 6 is
      empty, the strip stays on the group (legend or "Group 12"), and Claude reads the spots
      still at 0 through the whole move: the stage does not change.
- [ ] [Claude] Undo (label "Move group 6"): the group and the master are back on 6, spots still at 0.
      Redo. Push 3.8 to full: the spots come back to 80 %, so the master still masters them.
- [ ] [Jij] Set Legend on group 12, "Front spots": the strip reads "Front spots". The Playback Options
      Master tab reads "Group 12 Front spots".

## Delete (PRG-85, 86, PAT-10)
- [ ] [Jij] Delete, tap group 5: the cell is marked, softkey A reads "Delete group 5" with "Pars outside
      in" under it, and the message line asks to confirm. A tap does not select. Softkey Cancel:
      nothing changed, disarmed.
- [ ] [Jij] Delete, tap group 5, softkey A: the cell is empty; the selection and every cue still hold the
      fixtures (Claude reads the same levels on Art-Net). `Group 5 Enter` is refused.
- [ ] [Claude] Undo (label "Delete group 5"): items, grid and legend are back. Delete on an empty cell:
      "Group 7 is empty", no softkey A.
- [ ] [Jij] Delete, tap group 12 (the mastered one), softkey A: refused with "Group 12 is mastered by
      handle 3.8: delete the master first" on the message line; the group stays, Claude reads no
      change on stage.

## Restart (PRG-92)
- [ ] [Claude] Wait for the autosave, stop the daemon, start it again on the same folder: the groups and
      their legends are back in the cells and on the strip.

## Open points
- none
