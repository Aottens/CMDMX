# Research: everyday gaps in the console (IDEA-35)

Status: research only, no acceptance criteria. Written 2026-11-01, after the first church try showed
a group could not be deleted (P1-24). Each point is a candidate; check it against the Titan manual
when its batch is specified.

How it was found: an action matrix per object over core, IPC and app; a walk from an empty show to a
finished service; the everyday chapters of the Titan manual against specs and code. All by reading:
the app was not run. Only A1, A2 and B1 were checked by hand in the code. The Titan comparison used
page summaries, and some pages only by title.

## A. Show-proof
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
