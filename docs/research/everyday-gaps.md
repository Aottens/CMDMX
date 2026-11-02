# Research: everyday gaps in the console (IDEA-35)

Status: research only, no acceptance criteria. Written 2026-11-01, after the first church try showed
a group could not be deleted (P1-24). Each point is a candidate; check it against the Titan manual
when its batch is specified.

How it was found: an action matrix per object over core, IPC and app; a walk from an empty show to a
finished service; the everyday chapters of the Titan manual against specs and code. All by reading:
the app was not run. Only A1, A2 and B1 were checked by hand in the code. The Titan comparison used
page summaries, and some pages only by title.

## A. Show-proof (done, v0.3.0: SPF-01..11; SPF-03 waits for its sleep run)
Specified 2026-11-01 (P1-25; build tasks SPF-01..11): OUT-38..41, UI-35..40, IPC-50..52, REL-24, REL-25,
PBK-46. Checked against the code: all six hold, with two corrections. A3: UI-22 already said "draggable cap",
so the jump is a difference from the spec. A6: the engine, show file and IPC have the settings (PBK-12);
only the screen is missing.
1. Nothing keeps the Mac awake: no power assertion in app or daemon, nothing in the checklists.
   Whether idle sleep stops the output was not tried.
2. Command-Q asks only in console mode (`ConsoleMode.swift`); leaving full screen turns that off. The
   manual says it always asks.
3. One click makes a fader or the Grand Master jump to that level (`PlaybackArea.swift`), and Lock is
   disabled (`ui.md`, later).
4. Warnings are one truncated amber line without a history; output detail is on hover only (IDEA-24
   covers the colour only).
5. An engine the app adopted after an app crash is not restarted when it dies (`DaemonLauncher.swift`).
6. Times default to 0 s, with no show-wide default in the Settings tab.

## B. Changing what is recorded
1. A playback with one cue cannot be opened: View Cue exists on cue list rows only, and such a
   playback cannot be connected. The way out is rebuild and Replace.
2. Include on a cue or playback: palettes only (PRG-16 names PBK and CUE, neither has a criterion).
3. An effect in a cue can be neither seen nor removed in the Cue view.
4. Update cannot choose which palettes of a fired cue to store into; PBK-13 merges into the cue.
5. Cue view Remove is per attribute: no whole-fixture remove, no dormant values (Titan's Off/On).
6. Group Merge or Update: Update armed plus a tap selects the group. A group master cannot be
   retargeted. Record over a used group asks nothing, unlike palettes.
7. Blind and Edit are disabled, so every fix happens on stage (`programmer.md`, Blind has no phase).

## C. Seeing what the rig does
1. Rig-wide value views: Channel Grid and Intensity View.
2. DMX View, per-channel output.
3. Live output on fixture tiles (IDEA-10).

## D. Working on many at once
1. Copy, Move and Delete take one handle, cell or cue; Retain Layout and Bunch Up; Menu Latch.
2. No clear-all: page, pool, or a wipe that keeps the patch. No copy of a page.
3. Release a page of playbacks. Release All (`playbacks`, later).
4. Unfold a cue list or chase onto faders. Linked playback copy. Swap Items on a used handle.

## E. Patch and profiles
1. Invert attribute, Swap Pan/Tilt, Fixture Offset, Attribute Limits, output curves, Freeze, Flip.
2. Copy a patched fixture, Fixture Exchange, Swap Addresses, Update Personality, Auto Groups.
3. Delete or duplicate a fixture profile: a bad import stays in the list.
4. Lamp on/off and fixture control macros. RDM patching.

## F. Controls that exist in the engine, or small ones in the app
1. Rename a control mapping: IPC is there, the app has no caller.
2. Move a fixture button: `patch.handle` is there (PAT-14), the app never sends it.
3. Effect palette pool ignores armed functions: Delete armed plus a tap seems to apply the palette.
   Inferred from the code; try it first.
4. Mapping: duplicate, change the device pattern. Deck keys: copy, move to another page, clear a page,
   confirm on Remove. Output nodes have no name.
5. Keypad hidden below 1440 pt with Keys off, so no Thro or Enter there and the manual is silent.
6. Dimmer wheel: 1/255 per 4 pt, about 1020 pt from 0 to full without Shift. Relative level (`@ +10`).
7. Help key is dead. Manual: `running.md` says Command-1 to 4 (there are five); it calls
   `faders_per_page` a setting, but no control or IPC command was found.

## G. Show files
1. No reveal, delete or rename in the app ("Finder", `showfile.md`), and the app cannot reveal the file.
2. Loading a show brings its output nodes: no way to keep this desk's settings (Art-Net at home,
   Eurolite elsewhere).
3. Save As moves you into the copy, and going back needs Open, which changes the stage.
4. Copying a file over the open show: Open is refused without a hint to quit first, and the next
   autosave overwrites the copy. A new show has no Serial node while Art-Net dots stay green.
5. Reports of fixtures, cues, palettes and groups.

Tracked already, not repeated here: Macros, Select If, Align, Scene Master, virtual faders, nested and
time palettes, Merge from another show, Parking, Undo History, set list, Move In Dark (their specs'
out-of-scope lists, mostly without a backlog row).

# Second review, 2026-11-02 (at v0.3.0)

Same method and limits: code, specs and summaries of the Titan manual, the app not run. It looked at
what the first round skipped: paths between features, failures, and the Titan pages read by title only.
Checked by hand in the code: A2.1, A2.2, A2.3, H1, H3 (Patch and other screens), I1 (the fields), I5, J1.
A claim about Titan rests on a page summary; I2 is an inference from one.

## A2. Defects in what exists, proposed before B
Specified 2026-11-02 (P1-26; build tasks DEF-01..05, criteria PAT-26..28, IPC-53, UI-41, REL-26..28), each point checked
(`tests/manual/runs/2026-11-02-A2-checks.md`). Point 3 happens, also from a client and for a momentary DBO. Point 5 does not happen. Point 2 is worse:
the Messages sheet gets two lines every 2 s. Points 1 and 4 also leave palette effects, running cues and programmer effects.
1. Fixture remove leaves its members in cue effects: `patch_remove` cleans cue values and order, the
   load drops the members (`engine.rs`), so a running cue and a reopened show can differ. A gone first
   member also defeats the FX-42 dark check.
2. The restart note says "the show is back" straight after the process starts, before anything is
   loaded (`DaemonLauncher.swift`). It is never cleared, and an engine that dies on start seems to repeat it
   every 2 s with no backoff.
3. A held surface Flash or Swop can stick through Lock: the release is dropped and `set_locked` releases
   nothing (`control.rs`). Inferred; IPC-50 covers touch only.
4. A mode change drops programmer and cue values with a warning, but leaves palette values and cue
   effects without one (`engine.rs` `apply_mode`).
5. Whether Command-1 to 5 still switch workspaces behind the Lock cover: `running.md` says shortcuts
   stop. Needs a run.

## H. Functions and consequences behave the same everywhere
1. An armed function does the normal action on some targets, without a refusal: Delete, Update, Copy,
   Move or Set Legend plus a cue row sets Next (`CueLists.swift`); any function plus a fixture tile selects.
2. Flash and Swop stay live while a function is armed (`PlaybackViews.swift`).
3. Esc differs: with Keys off it does not disarm, with Keys on it does nothing on Patch, Show, Output
   and Control, and Record stays armed into those screens, where its keys are hidden.
4. Armed functions hear mouse taps only: Record plus `Group 5 Enter` just selects.
5. Delete confirmations state no consequence: how many cues use a palette, a playback that is running,
   the groups and cues a fixture is in. Empty groups and cues stay behind. Undo does cover these.
6. Lock is only in the Main context, with no Key action or Mac key.

## I. Behaviour that differs from Titan, no reason recorded
1. Fade Out and Delay Out follow Fade In until set; here they are separate and 0, so a cue with only a
   fade in snaps HTP down. Daily.
2. Keypad level: two digits as normal, a single-digit mode, `@ @` = 100 %; here `@ v` is v %. Daily.
3. Building a list: Titan stays in Record (Record, fader, fader appends); here Record disarms each time
   and needs a tap and the Append softkey. A chase is made by changing the kind in Options. Daily.
4. A connected chase's crossfade on the wheels; here only shown, set in Options. Daily.
5. Playback View scrolls to the active cue; here it does not. Daily on long lists.
6. Times edited in the grid over several cues; here one cue at a time through a tab. Daily.
7. Prev and Next Step set the chase direction, with bounce and random on keys; here Options only.
8. Shape size on the fader per playback (Fixed, Fader, HTP Fader); here always multiplied (FX-25).
9. Block Shapes and Solo Excluding Shapes (FX-27).

Absent and untracked (sometimes): Record By Fixture on playbacks; Group in the palette mask; an automatic
link for new cues; time in the programmer for the next cue; Options on several playbacks; Tempo Units;
Temporary Speed; per-playback Key Profile; effect Speed Multiplier; Referencing Playbacks list; Unassign to
the Show Library; gobo image as palette legend. Rarely: Copy Legends, Add Channels on Update, Minimum Palette
Mode, Auto Legend off, Always Merge, Attribute Overlap, Prev + Next cancels a skip, Bunch Up With Offset.

## J. Input paths and what you can see
1. Attribute values cannot be typed: the command grammar has level, palette and gel only (`command.rs`).
2. No range selection by mouse: Thro on the keypad only (a lasso only on the rig plan, IDEA-25).
3. The Mac keyboard covers the command line only: no Clear, Record, Update, Delete, Locate, Highlight or
   page key, and the app has no menu commands.
4. Surfaces cannot program: no Record, Update, Delete, Copy, Move, Set Legend or Exit in the vocabulary.
5. The surface keypad line is invisible and has no Exit, so a half-typed line is prefixed to the next.
6. Mappings and deck keys address by number or slot, so Move or Delete leaves them on the wrong thing,
   with no warning.
7. Running playbacks on other pages have no mark on the page buttons.
8. No "why is this lit": no view or IPC field gives the owner of an attribute or channel.
9. The app cannot show why the engine died: a panic goes to `daemon.log`, and the app cannot reveal it.
