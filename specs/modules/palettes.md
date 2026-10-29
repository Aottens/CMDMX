# Module spec: palettes (PAL)

Owner: `cmdmx-core::palette` (pure: storage, recording, resolving, fades), `cmdmx-daemon` (IPC),
the palette windows in `subprojects/app`.
Status: **PAL-01..32 approved by owner 2026-10-18 (P1-03); PAL-33 approved by owner 2026-10-28 (P1-23)**; PAL-14 amended for P2-02 (approved by owner 2026-10-29); owner decisions at the end.
Behavioural reference: Avolites Titan v19 manual, *Palettes*
(https://manual.avolites.com/docs/palettes: Creating, Using, Editing, Copying/Moving/Deleting,
Timing with Palettes) and the Titan analysis doc linked in `PROJECT.md`. UI reference: UI v0.1
(`specs/ux/main-screen.html`: the Colours, Positions and Gobos & Beams windows; `research.md`
"Palette iconography"). Builds on `programmer.md` (PRG-10, 15, 16, 22), ADR-005 (colour storage,
emitter zeroing), `showfile.md` (SHW-04, 07) and IDEA-14 (icons, phase-1 part).

## Purpose
A palette is a named look for part of a fixture (a colour, a position, a zoom) that the operator
records once and then recalls by one tap. A palette is a **reference**: the programmer, playbacks
and later cues store the palette's id, not its values, so changing the palette changes every
place it is used on the next tick. That is what lets a church service be re-focused by updating
six position palettes instead of forty cues. Palettes are what P1-04 (playbacks) and P1-08 (cue
lists) store, so they come first.

## Definitions
- **Palette:** a store of attribute values recorded from the programmer, with an internal **id**
  (assigned by the engine, never reused, like fixture ids in PAT-12), a user **number** 1–9999
  (unique) and a **cell**: its window and its handle (1-based position) in that window.
  Commands and the command line use numbers or cells; references use ids.
- **Window:** `Colours`, `Positions` or `Beams` (shown as "Gobos & Beams"). A palette lives in
  the window and cell it was recorded on, as on Titan; the window gives only the default record
  mask (Titan Quick Record): Colours C, Positions P, Beams I G B E S. What a palette holds comes
  from its values, never from where it sits (owner decision 6). Church examples: 1 "Warm"
  (Colours, cell 1), 2 "Pulpit" (Positions, cell 1), 3 "Zoom wide" (Beams, cell 1).
- **Fixture type:** a fixture's profile key (`patch.md`); all modes of one profile are one type.
- **Levels:** a palette holds values on three levels: **global** (`attribute → value`, for any
  fixture that has the attribute), **shared** (`fixture type → attribute → value`) and **per
  fixture** (`fixture id → attribute → value`). Its **kind** tag is `G` when the global level is
  not empty, else `S` when the shared level is not empty, else `N`.
- **Global attributes:** as on Titan, the intensity attribute (group I, HTP), Pan, Tilt and
  `Colour` (the target colour of ADR-005). Everything else is at most shared. Global Pan and
  Tilt are stored in **degrees from centre**, using each fixture's physical range (LIB-16), so
  one global position means the same angle on a 540° and a 360° head (owner decision 3).
- **Reference:** a stored value `{"palette": <id>, "value": <level or colour>}` in place of a
  level or colour. `value` is the **fallback**: the value the palette gave when the reference
  was made (applied in the programmer, recorded into a playback or cue).
- **Effects in a palette:** next to its values a palette may hold effect entries **without
  members** (shape and parameters; ADR-007, ADR-011, approved 2026-10-29, `effects.md` FX-22),
  applied to the targets, so a look such as "circle around the pulpit" can be one palette. The
  list is absent until used; a show without effects keeps its contents.
- **Resolve:** turn a reference for one fixture and attribute into a value (PAL-07).
- Church examples use the demo rig of `programmer.md`: washes 101–104, RGBW pars 201–208,
  RGB+Lime pars 211–216, RGB pars 221–226 (virtual dimmer), front spots 301–306.

## Acceptance criteria

### Recording (`Engine::record_palette`)
- **PAL-01** Recording onto an empty cell creates a palette there with the next free number
  (the lowest unused one, or a number the operator gives that is free), storing the programmer's
  contents in the record mask (PRG-15) for **every fixture in the programmer**, not only the
  selected ones. The mask defaults to the cell's window mask. Located and highlight values are never recorded (PRG-23, 26). With
  nothing in the mask, the record is refused with a warning and nothing changes ("Nothing in the
  programmer for Colours (mask C)"). Programmer 201 Dimmer 0.8 + Colour red, record 3 → palette
  3 holds only the Colour.
- **PAL-02** *Record By* (setting `record_by`, default `Mixed`, overridable per record; owner
  decision 2): `Channel` stores only the
  touched attributes. `Mixed` stores Position and Colour as a whole group: a fixture
  with any touched attribute in group P (or C) also stores every other attribute of that group
  it has, untouched ones at their current output value (PRG-20); other groups stay per channel.
  101 with only Pan touched, record 41 in `Mixed` → Pan and Tilt; in `Channel` → Pan only.
- **PAL-03** A programmer value that is itself a palette reference is stored as the value it
  resolves to at that moment (nested palettes are phase 2).
- **PAL-04** Kind detection (*Record as* `Auto`, the default). The palette is **shared** when,
  for each fixture type in it, all its fixtures stored the same attributes with the same values
  (compared at 16-bit, `round(v · 65535)`, each colour component on its own); otherwise it is
  **per fixture**. In a shared palette, every global attribute that every type stored with the
  same value moves to the global level. Pan and Tilt are compared in degrees from centre
  (`(v − 0.5) · range`, equal within 0.01°) and only become global when every fixture in the
  palette has a physical range for them; otherwise they stay shared. Values recorded on one
  fixture count as shared (Titan). Examples:
  - Red on 201–226, `Channel` (only Colour touched) → `G`: global Colour red.
  - Red on 201–208 with White 0.3 on the RGBW pars → `G`: global Colour red, shared White 0.3
    for the RGBW type.
  - Four different wash positions on 101–104 → `N`: four fixture entries.
  - Zoom 40 % on 101–104 → `S`: shared Zoom for the wash type.
  - "Straight down": Tilt at −90° on the washes (tilt range 270°) and on a second head type
    (tilt range 180°), Pan centred on both → `G`: global Pan 0° and Tilt −90°.
- **PAL-05** *Record as* `Normal` or `Shared` can only narrow the detected kind: `Normal`
  stores every value per fixture; `Shared` keeps the global level empty, and is refused with a
  warning when the values differ inside a type.
- **PAL-06** Recording onto a used cell needs a mode, else it is refused with a warning
  ("Palette 3 is used: Replace, Merge or Quick Merge"). `Replace` stores the new contents as on
  an empty cell. `Merge` detects the new contents on their own (PAL-04), never wider than the
  palette's current kind (a shared palette stays without a global level, a per-fixture one stays
  per fixture), and writes them over the existing values at their own level, keeping everything
  else. `QuickMerge` does the same
  but only for attributes the palette already holds on any level. Id, number, cell, legend and
  icon stay. Shared Red for the RGBW type, then RGB+Lime pars set to orange-red and merged → the
  palette also holds a shared entry for the RGB+Lime type; the RGBW entry is unchanged.

### Resolving and applying
- **PAL-07** A reference to palette *p* for fixture *f* and attribute *a* resolves, first hit
  wins: *p*'s per-fixture value for *f*; *p*'s shared value for *f*'s type; *p*'s global value,
  if *f* has *a*; `0` when *a* is an extra emitter of *f* (LIB-17) and *p* resolves `Colour` for
  *f* (ADR-005 decision 5); otherwise **nothing**. A global Pan or Tilt in degrees becomes
  `0.5 + degrees / range` with *f*'s own range, clamped to 0.0–1.0, and nothing for a fixture
  without a range. Global red applied to 221 (RGB) → red; to 211 (RGB+Lime) → red plus Lime 0;
  to 301 (spot, no Colour) → nothing.
- **PAL-08** Applying a palette to the targets (selection or sub-selection) puts a reference
  into the programmer for every attribute the palette resolves for each target, with the resolved
  value as its fallback. Attributes it
  does not resolve, and targets it resolves nothing for, are left as they are. If it resolves
  nothing for any target, it is refused with a warning ("Palette 3 Warm does not fit the
  selection") and nothing changes. Applying counts as a programmer change (PRG-03). Targets 201
  and 301, palette 3 (global Colour) → 201 Colour and White reference palette 3; 301 unchanged.
- **PAL-09** **Quick palette:** applying with an empty selection applies to every patched
  fixture the palette resolves anything for, in user-number order; the selection stays empty.
  Palette 3 with nothing selected → every colour-mixing fixture gets it.
- **PAL-10** `Palette n Enter` on the command line applies palette number *n* as PAL-08/09, to
  the typed selection when the line has one (PRG-08, PRG-10). An empty or unknown number is
  refused with a warning.
- **PAL-11** References are live. In the merge, a programmer or playback reference is resolved
  every tick, so a recorded or merged palette changes the output of every source that references
  it on the next tick, without a fade. A reference that resolves nothing (the palette no longer
  holds that fixture or attribute) proposes its fallback `value`. Playback 1 holds 201 Colour
  `{"palette": 1, "value": red}` at full; palette 1 re-recorded blue → 201 turns blue in the next
  frame; palette 1 re-recorded on 202 only → 201 shows red again.
- **PAL-12** **Include** applies like PAL-08/09 but puts the resolved values into the programmer
  as manual values instead of references, so the operator can change them and record them back
  (Record with Replace or Merge). PRG-22 holds: turning a wheel on a referenced attribute makes
  only that attribute manual.

### Timing
- **PAL-13** Applying with a fade time *t* > 0 fades each attribute from its current output value
  to the resolved value, linearly over *t*; `Colour` fades in linear target RGB; HTP and LTP
  attributes alike. The programmer holds the reference from the start (a Record during the fade
  stores the reference). The fade starts with the first tick after the apply (the core has no
  clock of its own; at most one tick, 25 ms, later). 201 at Dimmer 0, dimmer palette at 100 %
  applied with 2 s → 50 % at 1 s, 100 % at 2 s.
- **PAL-14** Overlap *o* (0–100 %) staggers the fades: the *i*-th fixture (0-based, in target
  order; for a quick palette in user-number order) starts after *i* · *t* · (1 − *o*/100), each
  fading over *t*. 100 % = all together. Washes 101–104, *t* = 2 s, *o* = 50 % → they start at
  0, 1, 2 and 3 s, so the run takes 5 s. The typed time is per fixture, as Titan's fixture
  overlap (owner decision 7). Without a fade time overlap has no effect.
  *Amended (P2-02, approved by owner 2026-10-29):* *i* is the target's position over the targets the palette
  touches (PRG-78), so items with the same position start together; grid off → as above.
- **PAL-15** Master time and master overlap: the settings `palette_time` (default 0 s) and
  `palette_overlap` (default 100 %) apply when an apply carries no time or overlap; a typed time
  or overlap overrides them for that apply only. Times outside 0–600 s and overlaps outside
  0–100 % are refused with a warning.
- **PAL-16** A fade never blocks anything. A wheel turn, another palette, Off or Clear on a fading
  attribute takes over from its current faded output value (PRG-20); the palette applied later
  starts its own fade from there.

### Managing palettes
- **PAL-17** Copy to an empty cell (any window) makes a new palette there (new id, next free
  number) with the same values, legend and icon; references keep pointing at the original. Move
  to an empty cell changes only the cell (same id and number), so every reference follows.
  Renumber gives a palette a free number 1–9999; references follow (they use the id). Copy or
  Move onto a used cell, or Renumber onto a used or out-of-range number, is refused with a
  warning.
- **PAL-18** Deleting a palette turns every reference to it, in the programmer and in every
  playback (later cue), into its fallback `value`, as on Titan: a playback goes back to the
  values it had when it was recorded (owner decision 1). Its number and cell are free again; its
  id is never reused. Palette 1 was red when playback 1 was recorded and is blue now; delete palette
  1 → playback 1 holds 201 Colour red.
- **PAL-19** Patch changes: removing fixtures (PAT-05) removes their per-fixture entries from
  every palette; a palette left without values stays (with its legend) and resolves nothing.
  Changing a fixture's mode keeps the stored values; an attribute the new mode lacks resolves
  nothing for that fixture.
- **PAL-20** Legend and icon: `legend` is 0–32 characters; an empty legend shows the window name
  and number ("Colour 1", "Position 2", "Beam 3"). `icon` is an icon id from the built-in
  library (PAL-22) or `null` for the automatic icon (PAL-21).

### Icons (IDEA-14, phase-1 part)
- **PAL-21** The engine computes each palette's automatic icon from its values, so every client
  draws the same thing:
  - any `Colour` value → `{"colours":[…]}`: up to 4 distinct colours as sRGB 0.0–1.0, in the
    order global, shared (by type key), per fixture (by id); a split swatch when there are more
    than one (Titan's bar per colour);
  - else any Pan or Tilt → `{"position":[[pan, tilt], …]}`: up to 8 distinct points, 0.0–1.0, a
    missing Pan or Tilt counting as 0.5, a global value in degrees drawn as
    `0.5 + degrees / 540` for Pan and `0.5 + degrees / 270` for Tilt (an icon only);
  - else `{"value":{"attribute":"Zoom","value":0.4}}`: the first stored attribute in I P C G B
    E S FX order, then by name.
  A palette with a picked icon still reports its automatic icon (the picker shows it as "Auto").
- **PAL-22** Icon ids are CMDMX's own names, `category.name` (`colour.drop`,
  `position.stage-left`, `beam.zoom`, `service.prayer`), lower case, `[a-z0-9.-]`, at most 48
  characters; `palette.legend` refuses any other string. The engine never needs the list; the
  app owns the built-in library: one line style (research.md: 1.25 pt, monochrome, so the only
  colour on a button is a colour palette's own), categories colour, position, beam and gobo,
  effect, fixture, service moments and generic, drawn with SF Symbols where one fits and as own
  glyphs for what SF Symbols lacks (gobo, moving head, par, beam angle; owner decision 5). An id the app does not know is drawn as the automatic
  icon. The user folder with imported SVG/PNG icons is phase 2 (IDEA-14).

### Show file (extends `showfile.md`)
- **PAL-23** Schema version 2 (SHW-04 migration from 1, which adds the defaults): `settings`
  gains `palette_time`, `palette_overlap` and `record_by`; a new key `palettes` after `groups`
  holds `{"next_id": n, "palettes": [{id, number, window, handle, legend, icon, global, shared,
  fixtures}]}`, with `fixtures` keyed by item (`"901"`, `"901.2"`) and an optional
  `shared_cells` (`type → cell → attribute → value`, PRG-52) from schema 7,
  sorted by id, maps sorted by key (SHW-02); a global Pan or Tilt is `{"degrees": -90.0}`; a
  playback value may be a reference `{"palette": id, "value": …}`. Live
  state (programmer references, running fades) is not saved (SHW decision 3).
- **PAL-24** Lenient load (SHW-07): each of these is left out with one warning naming it: a
  palette with an id, number or cell used by an earlier one, a number outside 1–9999 or an
  unknown window, a per-fixture
  entry for a fixture not in the patch, and a value that is not a level, colour, degrees (global
  Pan/Tilt only) or reference. A playback reference to a palette that does not exist becomes
  its fallback value, with a warning. Shared entries for a type no fixture uses are kept
  without a warning.

### IPC (extends `ipc.md`)
- **PAL-25** The snapshot gains `"palettes":[{"id":1,"number":1,"window":"Colours","handle":1,
  "legend":"Warm","icon":null,
  "auto_icon":{"colours":[{"r":1.0,"g":0.62,"b":0.3}]},"kind":"G","groups":["C"]}]`, sorted by
  number, `groups` being the attribute groups the palette holds in I P C G B E S FX order. A
  `{"v":0,"type":"palettes","palettes":[…]}` delta with the whole list follows any tick in which
  a palette was recorded, merged, renamed, copied, moved or deleted. The snapshot also carries
  `"palette_settings":{"time":0.0,"overlap":100.0,"record_by":"Mixed"}` (PAL-15, PAL-02), and a
  `{"v":0,"type":"palette_settings",…}` delta follows a change, so the Palette softkeys can show
  the current values (added 2026-10-18, VS2-19).
- **PAL-26** The programmer state (IPC-11) gains `"palettes":{"in_use":[1],"relevant":[1,2,7]}`
  (`in_use`: palettes referenced in the programmer; `relevant`: palettes that resolve something
  for a target, or all palettes when nothing is selected), each `wheels` entry gains
  `"palette": id` or `null`, and each `contents` entry (IPC-21) gains `"palettes":{"Colour":1}`
  for its referenced attributes and `"fading":{"Colour":0.4}` for its attributes with a palette
  fade still running (PAL-13/14): the progress 0.0–1.0 at the last tick, 0.0 while the fixture
  waits for its overlap turn; the entry disappears when the fade is done (amended 2026-10-18,
  owner, VS2-19 run).
- **PAL-27** Commands (numbers and cells, not ids; refusals are warnings, IPC-06):
  `palette.record {"window":"Colours","handle":4,"number":12,"mask":["C"],"record_by":"Mixed",
  "record_as":"Auto","mode":"Merge"}` (all but the cell optional: `number` only for a new
  palette, mask from the window, `record_by` from the setting, `record_as` Auto, `mode` only for
  a used cell) · `palette.apply {"number":3,"time":2.0,"overlap":50}` (`time` s and `overlap` %
  optional, PAL-15) · `palette.include {"number":3}` ·
  `palette.copy {"number":3,"to":{"window":"Colours","handle":5}}` · `palette.move` (same
  fields) · `palette.renumber {"number":3,"to":30}` · `palette.delete {"number":3}` ·
  `palette.legend {"number":3,"legend":"Warm","icon":"colour.drop"}` (`icon: null` = automatic) ·
  `settings.palette {"time":0.0,"overlap":100,"record_by":"Mixed"}` (each field optional).

### App (UI v0.1; amends `ui.md` UI-13, 16, 17, 18, 19 when approved)
- **PAL-28** The Colours, Positions and Gobos & Beams windows show their palettes on their
  handles, rows of 6 (UI-05), at least 36 cells and always one empty row after the last used cell,
  scrolling inside the window (UI-13 amended: no fixed number ranges). A used cell follows
  UI-05: number top-left, kind tag (G/S/N) top-right, icon (picked or automatic) and legend, and
  the IPCGBES mask strip from `groups`. In use in the programmer = red top strip; not relevant
  to the selection = 38 % opacity (Titan *Filter Relevant Palettes*). A tap applies the palette;
  when the command line holds only a number, that number is the fade time in seconds (Titan:
  type the time, touch the palette) and the line is cleared. The window title shows
  "mask C · 7 palettes".
- **PAL-29** Function buttons and softkeys come alive:
  - **Record** + tap an empty cell records it (PAL-01). Record + tap a used cell shows the Record
    softkeys Replace, Merge, Quick Merge and Cancel; tapping the same cell again is Quick Merge
    (Titan). The Record context also has Mask (IPCGBES toggles, starting from the window's
    mask), Record By (Channel / Mixed) and Record as (Auto / Normal / Shared), for this record
    only.
  - A double-click on an empty cell without Record armed records it too (Titan Quick Record).
  - **Update** + tap = Quick Merge; **Include** + tap = PAL-12; **Copy** / **Move**: tap the
    source, then an empty cell; **Delete** + tap, confirmed on softkey A (two-step, PAT-10).
  - **Set Legend** + tap a palette opens a legend editor: a text field, the number (Renumber,
    PAL-17), the icon library as a grid by category, and "Auto" with the automatic icon.
  - The keypad **Palette** key and the P key (UI-19) add the `Palette` token. Pressed with an
    empty command line, Palette opens the Palette softkeys: Master Time and Master Overlap (take
    the number on the command line; without one they show the current value) and Set Overlap
    (the number on the line is the overlap for the next tap only).
- **PAL-30** The Programmer window (UI-14) shows referenced values in cyan with the palette's
  legend; a wheel whose value comes from a palette shows its value in cyan (UI-03). A value
  that is still fading keeps showing its target, with an amber marker and its progress (UI-03:
  amber = fading) until the fade is done; the live output itself is for IDEA-10 (owner,
  VS2-19 run).
- **PAL-31** `tests/manual/app-palettes.md` walks recording, Quick Record, G/S/N on the mixed
  pars, applying with and without a selection, a timed apply with overlap, Merge, Copy/Move,
  Delete, legends and icons, and a restart of the daemon with the palettes back from the show.

### Safety
- **PAL-32** Every refused palette action returns a warning and changes nothing (PRG-36); no
  palette action or palette content can stop the tick or the output (ENG-12). Same patch and
  same timestamped actions → identical palettes, programmer state and frames (PRG-37, ENG-11).

### Gel numbers (IDEA-02; approved by owner 2026-10-28)
A palette stores only the colour (ADR-005 decision 5), never a gel number: the number is read
back from the colour (PRG-69), so it survives without being stored and the show file is unchanged.
- **PAL-33** Recording a new Colours palette whose stored Colour values all read as the same gel
  (PRG-69) gives it the legend of that gel, "L201 Full C.T. Blue" (cut at 32 characters), instead
  of an empty one; any other palette, and an Update or Include, leaves the legend as it is.
  201–208 after `Lee 201`, record Colours 4 → palette 4 legend "L201 Full C.T. Blue"; 201 at
  LEE 201 and 202 at LEE 106 → empty legend ("Colour 4").

## Scenario test format
Scenario files (`engine-merge.md`) gain an optional `palettes` list (palette entries as in
PAL-23) and a `pal` action (`record` with the `palette.record` fields, `apply {number}`,
`delete <number>`):

```json
{ "t": 0.0, "prg": { "select": [201, 211, 221] } }
{ "t": 0.1, "pal": { "record": { "window": "Colours", "handle": 1, "number": 3 } } }
{ "t": 0.2, "pal": { "apply": { "number": 3, "time": 2.0, "overlap": 50 } } }
"expect": [ { "t": 1.2, "universe": 1, "channels": { "37": 128 } } ]
```

Recording, detection, resolving, Copy/Move/Delete and the show file are unit tests on
`cmdmx-core` (`tests/palettes.rs`, `tests/showfile.rs`); anything that reaches the frame
(PAL-07 emitter zeroing, 11, 13, 14, 16) is a scenario file.

## Build tasks (`BACKLOG.md`)
1. **VS2-17** core: storage, record (PAL-01..06), resolve and apply (07..12), references in the
   merge, Copy/Move/Renumber/Delete, patch changes, legends, automatic icons, show schema 2
   (17..24, 32). Tests: `cmdmx-core/tests/palettes.rs`, scenarios PAL-07 and PAL-11.
2. **VS2-18** palette fades and overlap (PAL-13..16).
3. **VS2-19** IPC and the app: windows, Record/Update/Include/Copy/Move/Delete, legend editor
   with the icon library, Palette softkeys (PAL-25..31).

## Out of scope (phase 1 unless noted)
Nested palettes and *Fire Nested Palettes*, time palettes and palettes fired with their own
times, effect and shape palettes (phase 2, `PROJECT.md`); *Blind to Live* (with Blind);
*Release to quick palette* (Titan v19, with P1-04 release); applying a palette to all fixtures of
a playback; the Palette View window with per-value Off (phase 2); halos (with P1-04, for every
handle kind at once); the user icon folder (phase 2, IDEA-14); recording a palette from the command line (`Record Palette n`); **Edit** (Titan: set the
programmer aside, edit, Update, get the programmer back; owner decision 4: Include + Record →
Merge covers it, the Edit button stays disabled); undo (P1-10, `undo.md`).

## Owner decisions (2026-10-18)
None open; all seven answered on 2026-10-18.

1. Deleting a used palette behaves as on Titan: references fall back to the values they had when
   they were made (PAL-18). Hence the fallback `value` in every reference.
2. Record By defaults to `Mixed`, with `Channel` as the option per record and as a setting.
3. Market practice over the church: Global covers intensity, Pan, Tilt and Colour as on Titan;
   Pan/Tilt global in degrees via the physical ranges (PAL-04, PAL-07).
4. No Edit in phase 1; Include + Record → Merge.
5. Icons: SF Symbols in the app, plus own glyphs where the lighting world needs them; the show
   stores only CMDMX ids (PAL-22).
6. No number ranges per window: a palette lives where it was recorded and gets the next free
   number, as on Titan; its window never decides what it is (owner: the ONYX way, where the
   place decides, is the annoying one).
7. Overlap as Titan's fixture overlap: the typed time is per fixture (PAL-14).
