# Module spec: undo (UNDO)

Owner: `cmdmx-core::undo` (the history, pure), `cmdmx-core::engine` (what a step captures and
restores), `cmdmx-daemon` (classifying commands, IPC), the Undo/Redo controls in
`subprojects/app`.
Status: **approved (owner, 2026-10-22; the seven questions at the end answered as recommended,
DECISIONS 2026-10-22).** Build tasks VS5-01..03.
Behavioural reference: Avolites Titan v19 manual, *Titan Basics → Other Parts of the Touch
Screen → Undo/Redo* (https://manual.avolites.com/docs/titan-basics/other-parts-of-the-touch-screen):
recent actions are listed in the prompt area; actions "including fixture and cue changes" can
be undone and redone; an Undo History window lists them, undoes everything up to a touched
step, greys out undone steps (which can be redone) and marks actions that cannot be undone; in
multi-user mode each user has their own column. UI v0.1 (`specs/ux/main-screen.html`): one Undo
key in the programmer summary and an Undo softkey, both labelled with the step they undo, a
history of 30 snapshots (programmer, selection, palettes, playbacks), "Undone: …" in the message
zone, and "Deleted …. Undo brings it back." after a confirmed Delete. Builds on
`programmer.md` (PRG-29 Clear), `palettes.md`, `playbacks.md` (PBK-14 Delete), `cuelists.md`,
`group-masters.md` (GRM-13 saved level), `patch.md` (PAT-12 ids never reused), `showfile.md`
(SHW-11 switching shows, SHW-18 live state not saved), `ipc.md` and `ui.md` (UI-18, UI-20).
ADR-008 records the structural choice (snapshots of edit state, not inverse commands).

## Purpose
Every edit an operator makes can be taken back with one key, and put back again, whatever
module it touched: a Clear that threw away ten minutes of programming, a Record over the wrong
palette, a Delete of the wrong playback, a patch move to the wrong address. Live playing (Go,
faders, Flash) is never undone: undo must not change what the audience sees except by putting an
edit back. Built first (owner, 2026-10-22) so every module built after it is undoable from the
start.

## Definitions
- **Edit state:** everything a show edit or a programmer command changes: the stored show
  (`Engine::to_show`: patch, user profiles, groups, palettes, playbacks with their cues and
  options, page legends, settings) plus the programmer (values, palette references, selection
  and its order, the located set: PRG-44).
- **Live state:** everything else in the engine: fader and Grand Master levels (including the
  saved level of a group master, GRM-13), DBO, Flash/Swop/Latch, cue-list and chase positions and
  fades, tap tempo, the current page, the connected list, held LTP values, the wheels' attribute
  group and colour model, output status.
- **Command class:** every IPC command is exactly one of
  - **edit:** may change the edit state; it is undoable;
  - **live:** never changes the edit state; it is not in the history;
  - **reset:** replaces the whole edit state (`show.open`, `show.new`); it empties the history;
  - **neutral:** answered outside the tick (`library.*`, `show.list`, `cue.view`).
- **Step:** one entry in the history: a label and the edit state from before it.
- **Coalescing key:** what makes consecutive edit commands one step (UNDO-05).

## Acceptance criteria

### What one step is
- **UNDO-01** Every edit command the engine accepts and that changes the edit state adds one
  step, labelled from the command in English ("Record palette 2.3", "Delete playback 1.4",
  "Clear", "Patch move 301", "Wheel Pan"). A command the engine refuses (`Err`) or that leaves
  the edit state equal (Clear on an empty programmer) adds no step and leaves the redo list
  alone.
- **UNDO-02** A command-line line sent with Enter is one step, whatever it does (select, set,
  record).
- **UNDO-03** One confirmed action that touches several modules is one step (a Delete of a
  fixture removes it from groups, palettes and cues: one undo brings all of it back).
- **UNDO-04** Every IPC command kind has a class (Definitions) in one table in
  `cmdmx-daemon`; a unit test fails for a command kind without one, so a new command is
  classified when it is added. Phase 1 classes: **live** `fader.set`, `grand_master.set`,
  `dbo.set`, `page.set`, `master.level`, `playback.button` (Go, Back, Stop, Flash, Swop, Latch,
  Release …), `playback.tap`, `playback.bpm`, Go to cue / Set next, `programmer.attribute_group`,
  `programmer.colour_model`, `programmer.fan` (PRG-56); **reset** `show.open`, `show.new`; **neutral** `library.*`,
  `show.list`, `show.save_as`, `cue.view`; **edit** everything else (programmer selection and
  values, Clear, palette apply, Record/Update/Include/Merge, Copy, Move, Delete, Renumber,
  legends, icons, options, settings, patch).
- **UNDO-05** Consecutive edit commands with the same coalescing key merge into the step
  before them (its label and saved state stay): wheel turns on the same attribute, colour-picker
  moves, and selection commands (tap a fixture or group, Next/Previous, Odd/Even, All). Any other
  command, live commands included, ends the run. No time is involved: two wheel turns a minute
  apart with nothing in between are one step.

### Undo and redo
- **UNDO-06** Undo puts back the edit state saved in the newest step and moves that step to the
  redo list; Redo does the reverse. Undo with an empty history and Redo with an empty redo list
  are refused with "Nothing to undo" / "Nothing to redo" and change nothing.
- **UNDO-07** A new step empties the redo list (linear history, no branches).
- **UNDO-08** The history keeps the newest 100 steps; the oldest is dropped when a 101st is
  added. The redo list is bounded by the same 100.
- **UNDO-09** Undo and redo never change the live state, except as UNDO-10..12 require.
- **UNDO-10** A playback that undo or redo removes is killed at once, as Delete does (PBK-14).
  A playback that undo or redo brings back comes back as Record onto that handle would leave it
  (not running, its fader where the handle's fader is). A playback that stays but whose contents
  change keeps running and outputs its new contents from the next tick, as an edit does on stage
  (CUE "edits on stage at once"). A group master keeps its current level (GRM-13), whatever level
  the step saved.
- **UNDO-11** Undo and redo never lower the never-reused id counters (fixtures PAT-12,
  playbacks, palettes): after undoing a Patch add, the next add gets a fresh id, never the
  undone one.
- **UNDO-12** Held LTP values and fixtures that undo removes or brings back follow the patch as
  a patch edit does: a removed fixture stops output, a returned one outputs its programmer,
  playback or default values from the next tick. An LTP attribute the restored programmer no
  longer holds is let go of as Clear does (`programmer.md` PRG-45).

### Lifetime, autosave, several clients
- **UNDO-13** The history lives in the daemon's memory only: it is not in the show file and
  starts empty after a daemon restart. A **reset** command (`show.open`, `show.new`) empties it,
  and so does the daemon's own switch to the Demo fallback. `show.save_as` keeps it.
- **UNDO-14** Undo and redo change the edit state like any edit, so autosave (SHW) writes the
  result in place at its next run; nothing else changes in autosave.
- **UNDO-15** There is one history for the daemon, shared by every client: an Undo from any
  client undoes the newest step, whoever made it (there is one programmer, `programmer.md`).

### Determinism and safety
- **UNDO-16** `cmdmx-core::undo` takes no time and no randomness: the same command sequence
  gives the same history, labels and restored state. Scenario tests drive it.
- **UNDO-17** No `unwrap`/`expect`/`panic!`/indexing in the history; an undo whose saved state
  cannot be applied is refused with a warning and the history is left as it was. Taking a step
  happens per command, never per tick: the tick's hot path does not allocate for undo.

### IPC (extends `ipc.md`)
- **IPC-30** Commands `{"cmd":"undo"}` and `{"cmd":"redo"}`; refused ones answer with the usual
  error (UNDO-06).
- **IPC-31** The snapshot has `"undo": {"undo": <label|null>, "redo": <label|null>}` (the step
  the next Undo and Redo would act on); a `undo` delta with the same shape is sent when either
  label changes. After an undo or redo the normal deltas of the changed state follow (programmer,
  palettes, playbacks, fixtures …).

### App (amends `ui.md` UI-18, UI-20)
- **UNDO-18** Undo is the Undo key in the programmer summary (UI-20) and softkey G "Undo" in the
  main context (UI-18), both labelled with the IPC-31 `undo` label as a second line or tooltip
  and disabled while it is null; a Redo key sits next to Undo in the programmer summary, the
  same way. ⌘Z sends `undo` and ⇧⌘Z sends `redo`, also in console mode (UI-26), but not while a
  text field (legend editor, Patch screen field) has focus: there they edit the text.
- **UNDO-19** After an Undo or Redo the message zone shows "Undone: <label>" or "Redone:
  <label>" (the label the key showed), or the daemon's refusal.
- **UNDO-20** `tests/manual/app-undo.md` walks: program, Clear twice, Undo twice (programmer
  and selection back); Record over a palette, Undo; Delete a playback, Undo (back on its
  handle); patch move, Undo; a Go and a fader move between edits are not undone; Redo; a second
  client's Undo; daemon restart empties the history.

## Scenario test format
`tests/scenarios/UNDO-*.json` steps gain `{"undo": {}}` and `{"redo": {}}`, and an expectation
`{"history": {"undo": "<label>|null", "redo": "<label>|null"}}`. Everything else (programmer
expectations, frames) as today. Command classes (UNDO-04) are a unit test in `cmdmx-daemon`.

## Build tasks (`BACKLOG.md`, sized after approval)
- **VS5-01** Core: `cmdmx-core::undo` history (UNDO-01..03, 05..12, 16, 17), edit-state
  capture/restore on `Engine`, scenario steps. M.
- **VS5-02** Daemon: command classes (UNDO-04), the history around `apply`, reset on show switch
  (UNDO-13..15), IPC-30/31. S.
- **VS5-03** App: Undo/Redo keys, softkey, ⌘Z/⇧⌘Z, messages, `tests/manual/app-undo.md` and an
  owner run (UNDO-18..20). M.

## Out of scope (phase 1 unless noted)
The Undo History window with undo-to-a-step (Titan; later), per-user histories
(multi-user, later), undo of library edits (user profile files outside the show, LIB-12), undo
across a daemon restart, undo of live actions.

## Owner decisions (2026-10-22: each as recommended)
1. **What is one step?** Recommended: one accepted command that changes show or programmer;
   wheel turns on one attribute, colour-picker drags and selection taps in a row are one step;
   a command-line line is one step (UNDO-01..05). Alternative: every message one step (a wheel
   spin becomes 40 steps), or time-based grouping (breaks determinism).
2. **Live actions (Go, faders, Flash, GM, page) undoable?** Recommended: no, only edits; live
   actions are not in the history and do not block undo (UNDO-04, 09). Titan undoes "fixture
   and cue changes", not playback. Alternative: undo Go too (risky during a service).
3. **Redo?** Recommended: yes, linear, emptied by a new edit (UNDO-06, 07), as Titan.
4. **How far back?** Recommended: 100 steps (UNDO-08); the sketch has 30. The Undo History
   window (undo up to a step) comes later.
5. **Survive a restart / in the show file?** Recommended: no; memory only, emptied by
   Open/New and restart (UNDO-13). The `.bak` and autosave cover a crash.
6. **Autosave?** Recommended: undo is an edit like any other, autosave writes it (UNDO-14).
7. **Several clients?** Recommended: one shared history (UNDO-15); per-user columns come with
   multi-user.
