# App cue lists checklist (CUE-40..48, 54) — template

No Swift test target exists (CLAUDE.md), so the Go block, Cue list window, Cue view, strips,
Record softkeys, Playback Options tabs and Space of `specs/modules/cuelists.md` CUE-42..47 and 54
(and `ui.md` UI-03, 13, 18, 19, 22, 23 as amended for VS3-05) are checked by hand. Copy to
`tests/manual/runs/<date>-<task>.md` and tick it there. This run is VS3-01's demo (CUE-48).

Setup: two terminals, and a fresh shows folder so your real one stays untouched.
- `rm -rf /tmp/cmdmx-cue`
- Daemon: `cd subprojects/engine && cargo run -p cmdmx-daemon -- --no-sacn --show-dir /tmp/cmdmx-cue`.
- App: `cd subprojects/app && swift build && scripts/bundle-app.sh && open .build/CMDMX.app`.
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
- [ ] On page 3 (3.1–3.3 used), Record → BPM Master: it lands on 3.4 at once, "BPM 1 · 120 ×1". Chase speed source BPM 1; tap the master's
      lower button: the chase follows. Master tab: fader Bpm (fader sets the tempo up to 240),
      then Multiplier (zones /4 … ×4), then None.
- [ ] Record → Rate Master: it lands on 3.5, "RATE 1 · ×1.0" at fader 50 %. Chase speed source Rate 1:
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

## New cue times (PBK-46)
- [ ] Settings tab of any playback: *New cue times* shows delay 0, delay out and fade out empty with "as in", fade in 0, overlap 100 in a new show.
- [ ] Set delay 0.5, fade in 3, fade out 4, overlap 50. Record a new memory: its Times tab shows those four, and no Delay out row.
- [ ] Append a cue to the service list and a step to the chase: the new cue and step have those times; the
      earlier cues keep theirs.
- [ ] Fade in 601 is refused, the warning shows under the tab without scrolling and the setting stays 3. Fire the new memory: the Art-Net level
      takes 3 s after 0.5 s delay.
- [ ] Wait 5 s (autosave), restart the daemon: the four values are back. Set them to 0, 0, overlap 100, and empty fade out.

## As In (CUE-56, 57, UI-52)
Claude listens to Art-Net; the times below are read there, not on screen.
- [ ] Cue list Q2 par at 80 %, Q3 the same par at 20 %. Q3's Times tab: delay out and fade out empty with "as in". Fade in 3, Go: down over 3 s.
- [ ] Q3 fade out 5, Return: the field stays 5; back to Q2, Go: down over 5 s.
- [ ] Empty the fade out field, Return: "as in" again; back to Q2, Go: down over 3 s.
- [ ] Q3 delay out 2 with delay 0: Go: the fall waits 2 s; a par that rises in Q3 starts at once.
- [ ] Fade out 0: Go: down at once, and the field shows 0, not "as in".
- [ ] Back from Q3 with delay out 2: no wait.
- [ ] Settings tab: fade in 2, fade out and delay out "as in". Record a cue: its Times tab shows "as in". Fade out 4 in the Settings tab, record another: it shows 4. Empty it again.
- [ ] A memory in fader mode 1 with fade in 2, As In: pull the fader down: falls over 2 s. No Delay out row.
- [ ] An old show (schema 10) with fade in 3 and fade out 0 opens: the Times tab shows 0, the cue still snaps down; save it: it stays that way on the next open.
- [ ] Type fade in 602, Return: the warning shows, and half a second later the field shows the old number, focused or not. The same in delay out and in *New cue times*.

## Times in the grid (UI-51, IPC-56)
Claude listens to Art-Net; the times below are read there, not on screen. Taps and drags need a real mouse, not a background click.
- [ ] **Times** in the Cue list window: columns Q, Legend, Delay in, Delay out, Fade in, Fade out, Overlap; an As In value is dim. A tap on a cell lights it and does not move Next.
- [ ] Drag Fade in over Q2 to Q5: "Q2–Q5 · Fade in", no As In softkey. A number and Set: all four change, one Undo takes all four back.
- [ ] Tap Q2 Fade out, Shift-tap Q3, a number, Enter: both change. As In: both dim. Read the fall on Art-Net.
- [ ] Change a time while its fade runs: the running fade keeps its time on Art-Net, the cell reads the new one.
- [ ] A refused number (601): the warning, the cell keeps its value and stays selected.
- [ ] A function armed clears the cells, and a tap then targets the row (Delete: "Confirm on softkey A"). Right-click: the Cue softkeys.
- [ ] Done: the cells go, the view stays. Esc: the cells go; Esc again: the plain view. There a tap sets Next.
- [ ] A chase has no Times toggle.

## One-cue playback in the Cue window (UI-53)
- [ ] Tap a memory's legend in the Playbacks workspace: title "Cue · 1.2", one row, no Times toggle. The output, the Go block and the connected list's frame do not change.
- [ ] A tap on the row does nothing. **⋯**: softkeys View Cue, Edit Times, Set Legend and Cancel only; each works.
- [ ] Copy, Move or Delete armed, tap the row: refused, naming the handle, still armed. Set Legend armed: the legend editor.
- [ ] **View** on the row, Back to List: the one row again. Esc in the Cue view, Esc again: the list.
- [ ] A tap on a list's legend ends it; so does another list getting connected (Auto connect, Go on its handle). The same list stays connected through all of it.
- [ ] Tap a memory's legend in another workspace: "1.2 holds one cue: the Playbacks workspace shows it."

## A row says what it can do (UI-54)
- [ ] Each row ends in **View** and **⋯**, in the plain view and the Times view, on a list and a chase. No column is cut off in the Times view.
- [ ] **View** opens the Cue view; **⋯** the Cue softkeys, as a right-click, which still works. Neither moves Next or the output (Art-Net).
- [ ] With Delete armed, **View** and **⋯** on a row: "Delete cue n? Confirm on softkey A", as a tap on the row. Cancel.
- [ ] Times view with cells selected: **View** and **⋯** clear the cells.

## Effects in the Cue view, a whole fixture out (CUE-59, 60)
Claude listens to Art-Net for what goes on stage.
- [ ] Q1 with an effect on two fixtures, Q2 with an effect of its own on one of them. Cue view of Q2: two rows under the fixtures, look or shape, attributes, member count; Q1's in magenta, with one member left.
- [ ] A right-click on the magenta row: "tracks from Q1", nothing picked. A right-click on Q2's row, **Remove**, confirm: the row goes, Q1's shows two members again, one Undo brings it back.
- [ ] A cue with only an effect shows its row, not "This cue holds no values."
- [ ] Remove on the live cue: on Art-Net the channel stops on the next tick.
- [ ] A right-click on a fixture number: **Remove** names "fixture 201"; on a cell row "901.2". Confirm: its values and its place in the cue's effects go; a tracked value of it stays, magenta. A fixture holding nothing of its own: the warning, nothing changes.

## Off and On (CUE-61, IPC-57)
Claude listens to Art-Net; what goes on stage is read there.
- [ ] Q1 dimmer 1 at 50 %, Q2 dimmer 1 at 100 %, dimmer 2 at 100 %. Cue view of Q2, right-click 1's value, **Off**: struck through and dim, and next to it Q1's 50 % in magenta. On Art-Net with Q2 live: channel 1 goes to 128 on the next tick, no fade. Channel 2 stays.
- [ ] Back, Go into Q2 with it off: channel 1 stays 128 through the Go.
- [ ] Right-click the struck value, **On**: back at 100 %, 255 on the next tick. One Undo takes it off again.
- [ ] A fixture number, **Off**, **On**: all of that fixture. An item holding only values that are off keeps its row.
- [ ] Off on a tracked value: "tracks from Q1", nothing picked. **Remove** on a value that is off: "That value is off". Off and On with nothing picked say what to do.
- [ ] Merge the attribute into the cue from the programmer: the new value, the struck one gone. Replace: all struck ones gone.
- [ ] Save, reopen: the struck value is still there and still off on Art-Net.

## Include a cue (PBK-48, UI-42)
Claude listens to Art-Net; what goes on stage is read there.
- [ ] Include armed: softkey **Mask** "all". A tap on a group or a fixture is refused, naming palettes, playback handles and cues.
- [ ] Include + the legend of a running list: its current cue's own values in the programmer, those fixtures selected in the cue's order, on stage at once. A tracked value is not in the programmer.
- [ ] Include + the legend of a list that is off: "Name a cue: <legend> is not running", still armed, nothing in the programmer, that list connected. Then a row of it: that cue.
- [ ] Include + a memory's legend: its cue. A cue with a value off: that value does not come along.
- [ ] Include + a cue with a palette reference: the programmer shows it as a reference. A cue with an effect: the effect runs from the programmer.
- [ ] **Mask**: eight groups on, **Back**. I only: no Pan, no effect. FX only: the effect, no values. Nothing of the mask in the cue: "Q3 holds nothing to include".
- [ ] Change a value at once, Record Merge on that cue: only what the cue held and the change; tracked values stay magenta.
- [ ] One Undo takes it all out of the programmer. A Flash button with Include armed is a tap on the legend, no flash on Art-Net.

## Update chooses its targets (PBK-49, IPC-57)
Claude listens to Art-Net; what goes on stage is read there.
- [ ] A value changed, two playbacks up, one holding it: Update armed shows that one ticked, the palette holding it unticked, **Store**, **Cancel**. No **More**.
- [ ] Ticks toggle. More than five targets: **More** "1/2" pages on, ticks stay.
- [ ] **Store** with the playback ticked: the cue has the value, the palette has not; the cue's reference is now a value. One Undo brings it back.
- [ ] Palette ticked too: the palette has the value, the cue keeps its reference; with the programmer cleared the channel reads the new value. Another attribute in the programmer goes into the cue. One Undo brings back both.
- [ ] Enter on an empty line is **Store**. All unticked or no targets: "Update: no targets; tap a palette, playback handle or cue.", still armed. A tap on a target stays one direct update.
- [ ] A value off in the current cue: not a target without tracking. With tracking, Store stores it and the struck value is gone.

## Restart (CUE-24, SHW)
- [ ] Wait 5 s (autosave), stop the daemon (Ctrl-C) and start it again with the same command:
      the app reconnects, the lists, cues, legends, tracking options, chase and masters are back;
      every list is off with Q1 next.

## Stay in Record (CUE-62)
- [ ] [Jij] Record, **Create List**, tap an empty handle: its first cue, "Building p.s: tap it or Append for the next cue, Exit ends.", Record still armed.
- [ ] [Jij] Tap a group and a palette, change a value on the line: they act as normal, nothing is recorded. Tap the handle, then **Append**: Q2, Q3. Type a number and tap: that cue.
- [ ] [Claude] Another handle and a cue row: refused, still armed. One Clear, then a second: still armed; Append with the programmer empty is refused and stays armed.
- [ ] [Claude] **Create Chase** on a list: refused, still armed. On an empty handle: a chase on the strip. Flash or Swop on the strip is the tap (UI-43).
- [ ] [Claude] **Exit**, Esc and opening a screen each end it; each cue was one undo step.
