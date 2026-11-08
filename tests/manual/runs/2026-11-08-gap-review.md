# Gap review, the week after (REL-26, 2026-11-08, at v0.7.0)

An existing show is rebuilt for the next week, on a much larger rig than the church. Findings are in
`docs/research/everyday-gaps.md`, *Fifth review*. Claude drove it: the app under background control, then full-screen
control with a real mouse for the picks, a WebSocket client to program the show and for the patch checks, a listener on
port 6454. The owner chose the angle and took no step.

Setup: `main` at v0.7.0, debug daemon with `--ipc-port`, `--show-dir` and `--profiles-dir` in `/tmp/cmdmx-gap07`,
`--no-sacn --no-serial`. *Zondag 1*, built over IPC on the Demo profiles: 254 fixtures on universes 1 to 4 (38 + 48 washes,
96 pars, 72 dimmers), 17 groups, 12 colour and 8 position palettes, list *Dienst* on 1.1 with 30 cues, memories on 1.2 to
1.8, pages 2 and 3 with 8 each, four chases on page 4. Debug bundle with the same port, `-consoleMode NO -keysLive NO`,
removed after the run. No real show was opened; no lamps.

## Walk
- [x] Show screen, Save As *Zondag 2*: moves into the copy, *Zondag 1* stays. New with **Keep the patch** would empty the groups and palettes too (`SHW-19`, M1).
- [x] Copy, tap 2.1, ⇧-tap 2.8: "Copy 8 playbacks: tap the target". Page 5, tap 5.1: all eight there, one undo step. The page gets no legend (M6).
- [x] Delete, tap 3.1, All: "Delete 8 playbacks on page 3? Confirm on softkey A." Eight gone; the page keeps its legend *Pasen* (M6).
- [x] Delete, tap cue 3, scroll the list while armed, ⇧-tap: "Delete 7 cues of 1.1?", confirmed, 23 cues left, one undo step "Delete 7 cues".
- [x] Times on the list: the grid opens over all cues (I6).
- [x] Move plus a cue row: "Move cue 10: type the number, then Renumber". One cue at a time (M4). The row menu has no Move.
- [x] Patch: ten rows visible of 254, no find; 40 scroll ticks to reach 1005. ⇧-pick of seven rows works (M3).
- [x] Remove seven washes, first step: "In 3 groups, 62 cues on 28 playbacks and 11 palettes. Undo brings it all back."
- [x] Over IPC, those seven removed and seven of another type added at the same numbers and address: group 5 from 24 to 17, 16 from 24 to 21, 17 from 24 to 20. Two undos restored it (M2).
- [x] Over IPC, 12 new pars on universe 5: the universe is added by itself. Group 8 *Merge*: 60. `palette.apply` of palettes 1 and 9 on them: "does not fit the selection" (M5).
- [x] Over IPC, Open *Zondag 1* and back to *Zondag 2*: 1.6 s and 0.6 s to the snapshot. In the app afterwards Undo reads "nothing" (UNDO-13, M1).
- [x] Views: Intensity with 266 tiles. Art-Net arrived on all five universes while the fader was up.
- [x] Engine load, debug build: 78 % of a core on *Zondag 2* (266 fixtures), 10 % on *Demo* (38 fixtures) (M7).

## Not run
- Go through the list on the large rig: the fader went up, no Go was pressed, so fades on 266 fixtures were not seen.
- Release All and Move with swap: both ran in `2026-11-08-MUL-04.md` and `2026-11-08-MUL-03.md`, on a small show.
- A surface or deck after Move or Delete (J6): no hardware in this run.
- A release build: the engine load above is a debug build only.
- A real show over more than one week: the two shows here are a day's work by script.

## Not concluded
- Palettes 1 to 11 came out Normal because the build script left other values in the programmer. Palette 12 came out Global. So the refusal in M5 is real for a Normal palette, and whether new fixtures usually meet one depends on how a show is programmed.
- One click on *Fixtures + Groups* while *Confirm remove* was showing left the Patch screen open. The next click on another workspace worked. Seen once, not repeated.

## Open points
- The findings M1 to M7, with their proposals → `docs/research/everyday-gaps.md`
