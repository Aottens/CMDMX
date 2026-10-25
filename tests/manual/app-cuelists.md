# App cue lists checklist (CUE-40..48, 54) — template

No Swift test target exists (CLAUDE.md), so the Go block, Cue list window, Cue view, strips,
Record softkeys, Playback Options tabs and Space of `specs/modules/cuelists.md` CUE-42..47 and 54
(and `ui.md` UI-03, 13, 18, 19, 22, 23 as amended for VS3-05) are checked by hand. Copy to
`tests/manual/runs/<date>-<task>.md` and tick it there. This run is VS3-01's demo (CUE-48).

Setup: two terminals, and a fresh shows folder so your real one stays untouched.
- `rm -rf /tmp/cmdmx-cue`
- Daemon: `cd subprojects/engine && cargo run -p cmdmx-daemon -- --no-sacn --show-dir /tmp/cmdmx-cue`.
- App: `cd subprojects/app && swift build && scripts/bundle-app.sh && open .build/ColourmotionDMX.app`.
- Demo show: church rig (washes 101–104, pars 201–226, front spots 301–306); pages 1 and 2 hold
  the demo playbacks, page 3 is empty. Workspace 4 (Playbacks) shows the Cue list window.

Output over time cannot be seen without lamps: Claude listens to the daemon's Art-Net on
universe 1 during the run and reports the measured values and times.

## Before any list (UI-23, CUE-42, 43)
- [ ] The Go block reads "No cue list" and is disabled; the Cue list window shows its empty
      table and "No cue list connected". Space with Keys on does nothing.

## Building the service list with tracking (CUE-03..05, 45, 50)
- [ ] Page 3. Pars 201–208 at 45 %, Record, tap 3.1: a memory "3.1".
- [ ] Without Clear: front spots 301–306 at 80 %, Record, tap 3.1: the softkeys show Replace,
      Merge, Append, Insert, Cancel. Append → 3.1 is now a cue list: Q2, the strip buttons read
      GO / FLASH, the legend's second line "Q– › 1", and the Go block connects to it (the only
      list). The Cue list window lists Q1, Q2.
- [ ] Clear. Append Q3 (washes 101–104 blue, Pan 30 %), Q4 (washes red), Q5 (spots 100 %), Q6
      (pars 15 %, spots 0), Q7 (washes white, pars 60 %) — one cue each, Clear between them, so
      each cue holds only what changed.
- [ ] Set Legend on each row via the Cue softkeys (right-click a row): Walk-in, Welcome, Worship,
      Prayer, Sermon, Video, Walk-out.

## Running from the Go block (CUE-10..12, 18, 19, 42)
- [ ] Header shows "Service"-less legend "3.1" (or its legend), "fader 0 %" in amber; the GO
      button shows "Q1 Walk-in". Raise 3.1 to full: the amber note goes, Q1 fires (fire first
      cue): Claude reads the pars at 45 %.
- [ ] GO: Q2 runs; the "now" line shows Q2 Welcome in green with its progress, the GO button
      "Q3 Worship". The pars stay at 45 % (tracked). The Cue list window marks Q2 green, Q3
      outlined; the strip legend reads "Q2 › 3".
- [ ] Current and next apart (CUE-42, VS4-06): the "now" line is the largest text in the Go block,
      green with a green bar on its left; the GO button shows "Next: Q3 Worship" small and grey,
      never green; at the last cue it reads "end of list" in the same small grey.
- [ ] Back: Q1 again, no delay. Stop during a fade (give Q3 fade in 5 s on the Times tab first):
      "paused" in amber, Stop lit; Claude sees the values freeze. GO resumes. Stop twice = Back.
- [ ] Tap row Q5 in the Cue list window: it becomes next (outlined), nothing changes on stage.
      GO runs Q5. Right-click Q2 → Go to Cue: Q2 runs at once.
- [ ] At Q7: GO shows "end of list" and is disabled.

## Tracking and the Cue view (CUE-50..54, UI-03)
- [ ] Right-click Q4 → View Cue: the Cue view replaces the Cue list window; the pars show 45 %
      in magenta (tracked from Q1), the washes red in normal text (hard). Back to List returns.
- [ ] Q6 Video: on the Times tab set track Cue Only. Go to Q6, then GO to Q7: pars 60 % (Q7 hard),
      spots back to Q5's 100 % (Q6 does not track). The Track column shows CO on Q6.
- [ ] In the Cue view of Q7 right-click the pars' value → Remove, confirm: the pars now show the
      Q5/Q1 value in magenta.

## Update modes (CUE-52)
- [ ] Go to Q3. Pars 201 at 80 %, Record, tap the Q3 row: Replace, Merge, Update, Cancel and the
      Update Mode softkey. With Forwards: Q3 and Q4 show 201 at 80 % in the Cue view.
- [ ] Repeat with Cue Only (Q4 gets the old value back as a hard value), Backwards (stored in
      the cue it tracked from, Q1) and Both. Check each in the Cue view.

## Editing (CUE-05, 07..09, 14, 20)
- [ ] Standing on Q2: Record, tap 3.1, Insert: a Q2.5 appears as next.
- [ ] Right-click Q2.5 → Renumber with "8" typed: it moves to the end. Copy with "9" typed: a
      copy. Delete Q9 (two-step on softkey A).
- [ ] Options 3.1 → Cue list tab: tracking, fader, fire first cue, loop, and a Renumber All
      button: the cues become Q1..Q8 in their order.
- [ ] Times tab, cue Q5: link After 0. GO on Q5: Q6 follows by itself once Q5 is done.
- [ ] Times tab, cue Q3: group time P delay 1 s fade 5 s. Go to Q3: Claude reads colour in the
      cue's fade, Pan starting 1 s later and arriving after 6 s.

## Space, handle Go and connecting (CUE-26, 44, 47)
- [ ] Keys on: Space = GO, Shift+Space = Back. Keys off: Space does nothing. With a legend field
      focused, Space types a space.
- [ ] Record a second list on 3.2 (two cues). Plain tap on its legend: the Go block and Cue list
      window follow 3.2 and its strip gets the blue frame. The GO button on strip 3.1 still runs
      3.1 (and, with Auto connect on, connects it).
- [ ] Settings tab: Auto connect off; GO on 3.2's strip no longer moves the Go block.

## Fader, release and reset (CUE-15..17, 22)
- [ ] 3.1 at Q4, pull its fader to 0: KillWithOff keeps Q4 (HTP out, Go still works). Cue list
      tab → fader Kill At 0: the next fall to 0 kills the list, next = Q1.
- [ ] Release in the Go block: the list releases and starts from Q1 next time. Flash on a list
      that is off fires it at its next cue.

## Chases and masters (CUE-28..37, 46)
- [ ] Record a three-step chase on 3.3 (Append twice), Options → Cue list tab → kind Chase. The
      Chase tab shows BPM 120, crossfade 0, Forward, Loop, speed Free. The strip buttons read TAP /
      FLASH and its legend "1/3 · 120". Raise the fader: it steps (Claude reads 2 steps/s).
- [ ] Connect it: the Go block shows "120 BPM · xfade 0 %", Tap and the BPM field. Tap four times
      about 0.5 s apart: ~120 BPM; type 90 in the field; scroll it. Crossfade 50 %, Bounce.
- [ ] Record → BPM Master, tap 3.4: "BPM 1 · 120 ×1". Chase speed source BPM 1; tap the master's
      lower button: the chase follows. Master tab: fader Bpm (fader sets the tempo up to 240),
      then Multiplier (zones /4 … ×4), then None.
- [ ] Record → Rate Master, tap 3.5: "RATE 1 · ×1.0" at fader 50 %. Chase speed source Rate 1:
      fader full doubles the chase speed.

## Only what applies (CUE-44, 46; VS3-08)
- [ ] BPM master 3.4, Master tab fader Multiplier: with the fader at about 70 % the strip reads
      "BPM 1 · 240 ×2" (the zone's ×, not the stored ×1); at 10 % "… ×0.25". Back to None: the
      stored × again.
- [ ] Options on a memory: tabs Fader, Times, Release, Playback, Handle, Cue list, Settings. On the
      service list (3.1): no Fader tab; the Cue list tab still has tracking, fader, fire first cue
      and loop. On the chase (3.3): no Fader tab, the Cue list tab shows only Kind and Renumber
      All, the Times tab has no Track row.
- [ ] Times tab on a cue without group times: one row of group letters with a "—" button each, no
      delay / fade rows. Tap "—" under I: the delay and fade rows appear; × on I: back to one row.

## Handle buttons (CUE-25, 55)
- [ ] Settings tab → handle buttons Titan: every list/chase strip shows SWOP / FLASH; GO only in
      the Go block. Custom: Cue list = Go / Back; the 3.1 strip shows GO / BACK and they work.
      Back to Best practice.

## Restart (CUE-24, SHW)
- [ ] Wait 5 s (autosave), stop the daemon (Ctrl-C) and start it again with the same command:
      the app reconnects, the lists, cues, legends, tracking options, chase and masters are back;
      every list is off with Q1 next.
