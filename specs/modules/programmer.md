# Module spec: programmer (PRG)

Owner: `cmdmx-core::programmer`. Status: **PRG-01..37 approved by owner 2026-10-17 (P1-02); PRG-38..44 approved 2026-10-22 (VS2-05)**.
Behavioural reference: Avolites Titan v19 manual, *Controlling Fixtures* (Selecting Fixtures,
Changing Fixture Attributes) — https://manual.avolites.com/docs/controlling-fixtures. UI
reference: UI v0.1 (`specs/ux/main-screen.html`: fixtures and groups windows, attribute bar,
keypad + command line, programmer window, colour picker). Merge rules: `engine-merge.md`.

## Purpose
The programmer is where every look is built. The operator selects fixtures in an order, changes
attributes with the wheels, the keypad or palettes, and sees the result live; only what was
touched is held, so recording stores exactly the operator's intent and lets every untouched
attribute show through from the playbacks. All of it lives in the engine: the app, the BCF2000
and later remotes are views and inputs of the same single programmer.

## Definitions
- **Fixture:** a patched fixture, addressed by its user number (`patch.md`). Church examples:
  washes 101–104, RGBW pars 201–208, RGB+Lime pars 211–216, RGB pars 221–226 (virtual
  dimmer), front spots 301–306.
- **Attribute:** a controllable parameter of a fixture's profile (Dimmer, Pan, Colour, Zoom, …),
  with its attribute group I P C G B E S FX from the profile (`fixture-library.md`).
  Values are normalised 0.0–1.0 and held with at least 16-bit resolution.
- **Selection:** an ordered list of fixtures without duplicates. Order is the order of selecting.
- **Sub-selection:** the part of the selection the wheels act on while stepping (Fix ±1) or in a
  pattern (Odd/Even). No sub-selection = the whole selection.
- **Targets:** the sub-selection if there is one, otherwise the selection.
- **Programmer contents:** per fixture, per attribute, a *touched* value: either a manual value
  or a palette reference (`palette_id`). An attribute that was never touched is absent.
- **Located set / highlight:** output overrides that are *not* programmer contents.
- **Active attribute group** and **colour model** (HSI / RGB / CMY): engine state shared by
  the screen and every control surface, so the on-screen wheels and the encoders always agree.
- **Colour:** per fixture one colour attribute holding a device-independent target colour, plus
  direct emitter attributes (White, Lime, …) in group C. Representation and how emitters
  combine with the target colour: ADR-005 (BACKLOG P1-07). Criteria PRG-30..34 are written
  against behaviour only and hold whichever representation ADR-005 picks.

## Acceptance criteria

### Selection
- **PRG-01** Selecting a fixture appends it to the selection; selecting a fixture that is already
  selected removes it. Select 203, 201, 202 → selection `[203, 201, 202]`; select 201 again →
  `[203, 202]`.
- **PRG-02** Selecting a group appends its fixtures in the group's stored order, skipping
  fixtures already selected (they keep their place). Selection `[205]`, group "Pars RGBW"
  (201–208) → `[205, 201, 202, 203, 204, 206, 207, 208]`.
  Selecting a group whose fixtures are **all** in the selection removes them from it instead,
  as PRG-01 does for one fixture; the rest keeps its order. Selection `[205, 201, …, 208]`,
  group "Pars RGBW" again → `[]`; selection `[301, 201, …, 208]` → `[301]`. When PRG-03 starts a
  new selection, the group is selected as usual. A typed `Group n` on the command line always
  selects (typed selections replace). Amended 2026-10-17 (IDEA-11, VS2-15).
- **PRG-03** After any change to the programmer contents (wheel, keypad level, palette, Rem Dim),
  the next select action first deselects everything and starts a new selection; the earlier
  changes stay in the programmer. Select 201, set dimmer 50 %, select 202 → selection `[202]`,
  201 still at 50 % in the programmer.
- **PRG-04** Fix +1 / Fix −1 with no pattern step a sub-selection of one fixture through the
  selection in order, wrapping at both ends. Selection `[101, 102, 103]`: Fix +1 → `[101]`,
  Fix +1 → `[102]`, Fix −1 → `[101]`, Fix −1 → `[103]`.
- **PRG-05** Pattern select splits the selection into `n` stages by position (stage `k` =
  positions `i` with `i mod n = k`); the sub-selection is stage 0, Fix ±1 moves between stages.
  Odd/Even is `n = 2`. Selection 201–206: Odd/Even → `[201, 203, 205]`, Fix +1 →
  `[202, 204, 206]`.
- **PRG-06** All ends the sub-selection or pattern (targets = whole selection again). With no
  sub-selection active, All selects every fixture in the programmer, in user-number order.
- **PRG-07** Recording a group stores the current selection in its order; recording with an
  empty selection is refused with a warning and leaves the group unchanged. Recording onto a
  used group replaces its contents.

### Command line (Titan keypad syntax, parsed and executed in the engine)
The client sends the token list on Enter (`["Fixture","201","Thro","205","@","50","Enter"]`);
editing the line (Back, Exit) stays in the client.
- **PRG-08** A selection expression replaces the selection. `Fixture` is optional (numbers default
  to fixtures); `Group n` selects group n; `And` adds; `Thro` is a range in the typed direction;
  `Not` removes. `201 Thro 205 Not 203 And 301 Enter` → `[201, 202, 204, 205, 301]`;
  `205 Thro 201 Enter` → `[205, 204, 203, 202, 201]`. Numbers without a patched fixture are
  skipped; if nothing is left, the line is refused with a warning.
- **PRG-09** `@ v` sets the intensity of the targets (or of the expression's fixtures) to v %
  (0–100, clamped) as touched values. `221 Thro 226 @ 40 Enter` → Dimmer 0.40 on six fixtures
  (the RGB pars' virtual dimmer included). `@ 50 Enter` with an empty selection is refused.
- **PRG-10** `Palette n Enter` applies palette n to the targets (rules in `palettes.md`); ending a
  selection expression with `Locate` instead of `Enter` selects and locates in one step (PRG-24).
- **PRG-11** An unknown token or malformed line is refused with a warning naming the line;
  nothing in the selection or programmer changes.

### Touched attributes only
- **PRG-12** Only touched attributes enter the programmer; untouched attributes of a fixture in
  the programmer keep coming from the playbacks. Playback 1 has 201 at Dimmer 80 % red; in the
  programmer only 201's colour is set to blue → output 201 = Dimmer 80 % (from the playback),
  blue (from the programmer).
- **PRG-13** The programmer is a merge source at `Programmer` priority (ENG-04): its touched
  values beat every playback except `VeryHigh`.
- **PRG-14** Selecting a fixture puts nothing in the programmer.
- **PRG-15** `contents(mask)` returns the touched values whose attribute group is in the mask,
  with their source (manual value or palette id), and nothing else: no located, highlight or
  playback values. This is what Record, Update and Quick Record read (`palettes.md`, cue lists).
- **PRG-16** Include loads given values into the programmer as touched values, keeping palette
  references as references. It is the one entry point PAL/PBK/CUE use for Include and Edit.

### Wheels
- **PRG-17** The wheel layout for the active group lists the attributes of that group that at
  least one target has, in the profile's attribute order (first target fixture first, then
  attributes only later targets add). Colour group: the model's wheels (HSI: Hue, Saturation;
  RGB: Red, Green, Blue; CMY: Cyan, Magenta, Yellow), then the direct emitters any target has.
  Targets 201 (RGBW) + 211 (RGB+Lime), group C, model HSI → `[Hue, Saturation, White, Lime]`.
- **PRG-18** If the active group has no attribute in the new targets, the engine switches to the
  first of I, P, C, G, B, E, S that has one. Select only 301 (spot) while in C → active group I.
- **PRG-19** Turning a wheel by `steps` changes the attribute relatively on every target that has
  it: each target keeps its offset, each value clamps to 0.0–1.0 on its own. 201 at 0.20 and
  202 at 0.70 on Dimmer, +10 `Coarse` steps (+0.40) → 0.60 and 1.00 (clamped).
- **PRG-20** A wheel touch on an attribute that is not yet in the programmer starts from the
  fixture's current output value, not from 0. Playback holds 201 at Dimmer 60 %; one step up →
  programmer 201 Dimmer 60 % + 1 step.
- **PRG-21** Step size per resolution, from settings: `Normal` = 1/255, `Fine` = 1/65535,
  `Coarse` = 1/25. On a 16-bit attribute one `Fine` step changes the fine channel by 1 and leaves
  the coarse channel (frame writer, `patch.md`). Speed acceleration and which key or gesture
  picks the resolution (Option/Shift, Avo) are input-side: `control-surfaces.md` and the app.
- **PRG-22** Changing an attribute that holds a palette reference replaces that attribute's
  reference with a manual value; other attributes from the same palette keep their reference.
  Position palette on 101 (pan + tilt), turn Pan → Pan manual, Tilt still the palette.

### Locate, Highlight, Off, Rem Dim
- **PRG-23** Locate sets the attributes of the targets that are in the locate mask (default all
  groups) to the profile's locate values (open white, centred, full). Located values are not
  programmer contents: `contents()` does not return them.
- **PRG-24** Locate removes the located attributes of those fixtures from the programmer (output
  and programmer agree). A later touch on a located attribute enters the programmer starting
  from the locate value (PRG-20), and only that attribute; the rest stays located.
- **PRG-25** In the merge, per attribute: programmer value over locate value, both at
  `Programmer` priority. Located 101, then Pan touched → Pan from the programmer, rest from locate.
- **PRG-26** Highlight on: targets output the profile's highlight values (default intensity full,
  colour open white); while a sub-selection is active, the other selected fixtures output the
  lowlight intensity (setting `lowlight_level`, default 0.25). Highlight wins over programmer
  values for those attributes, is never programmer contents, and stops when switched off or the
  selection is cleared; programmer edits made meanwhile stay in the programmer.
- **PRG-27** Off removes from the programmer the touched values of the targets whose attribute
  group is in the given mask (UI v0.1 default: the active group). 201 with Dimmer and Colour in
  the programmer, Off with mask C → only Dimmer stays. A fixture with nothing left leaves the
  programmer.
- **PRG-28** Rem Dim puts Dimmer 0 as a touched value into the programmer for every patched
  fixture with an intensity attribute that is not selected (so it records into cues). Selection
  301–306 → the other 24 fixtures get Dimmer 0 in the programmer.

### Clear
- **PRG-29** Clear follows the setting `clear_precedence`:
  `SelectionThenProgrammer` (first Clear deselects, the next empties the programmer),
  `SelectionWithProgrammer` (one Clear does both) or `ProgrammerThenSelection`. Emptying the
  programmer also empties the located set; deselecting also switches Highlight off. Default
  `SelectionThenProgrammer` (UI v0.1; Titan's default is a setting). A masked Clear (Set Mask) removes only the touched values in the mask, for all
  fixtures, and leaves the selection. A timed Clear behaves as ENG-10.

### Locate, Highlight, Off, Rem Dim and Clear in detail (VS2-05, approved by owner 2026-10-22)
- **PRG-38** Locate values. Per attribute in the locate mask: the profile's `locate` value;
  without one, an intensity attribute (I group and HTP, the virtual Dimmer included) 1.0, Pan and Tilt 0.5,
  Colour open white (1, 1, 1), an extra emitter 0 (white comes from the target colour,
  ADR-005), any other attribute its `default` (LIB-03). Located values stay until a Clear that
  empties the programmer, an Off (PRG-27, 40), a touch (PRG-24) or the fixture's removal; Locate
  again overwrites them. Locate with no targets is refused; Locate counts as a programmer
  change for PRG-03.
- **PRG-39** Highlight values. Per target: an intensity attribute its `highlight` value, else 1.0;
  Colour open white; an extra emitter 0; any other attribute with a `highlight` value that
  value; every other attribute is left alone. Lowlight sets only the intensity attributes of the
  other selected fixtures. Highlight replaces the base value after the merge (ENG-14), so group
  masters, the Grand Master and DBO still act on it. It follows the targets as Fix ±1 steps.
  Highlight is live state: not undone, not saved. Switching it on with no selection is refused.
- **PRG-40** Off for one attribute (IDEA-07): removes that attribute from every target, both its
  touched value and its located value. A colour-model wheel name (Hue, Saturation, Red, Green,
  Blue, Cyan, Magenta, Yellow) means the Colour attribute as a whole; an emitter name only that
  emitter. Targets 201 + 202 with Colour and White in the programmer, Off "Hue" → only White
  stays. PRG-27 (Off by mask) also removes located values in the mask. An Off that removes
  nothing, or has no targets, is refused ("Off: nothing of the selection is in the programmer").
- **PRG-41** Rem Dim with no selection is refused (it would black out the whole rig). "Intensity
  attribute" is every I-group HTP attribute, the virtual Dimmer included. Rem Dim counts as a
  programmer change for PRG-03.
  An LTP attribute of the I group (Strobe, Shutter) is not an intensity attribute for PRG-38,
  39 and 41: Locate puts it at its `locate` value, else its `default`; Highlight changes it only
  through its own `highlight` value; Rem Dim leaves it alone (so neither ever starts a strobe).
  Added 2026-10-22 in review (the Showtec RGB par's Strobe channel is I-group LTP).
- **PRG-42** Programmer settings in the show (`settings`, lenient load per field):
  `clear_precedence` (PRG-29), `clear_ltp` = `Release` (default) or `Freeze` (ENG-10) and
  `lowlight_level` 0.0–1.0 (PRG-26, default 0.25); a value out of range is refused.
  `ProgrammerThenSelection`: the first Clear empties the programmer and the located set, the
  next deselects; with an empty programmer the first Clear deselects.
- **PRG-43** When the programmer gives up an LTP attribute (Clear in any form, Off), `clear_ltp`
  decides what it outputs next: `Release` → the value the playbacks give, or the profile
  `default` (LIB-03, Colour open white) when no playback proposes it; `Freeze` → it keeps its
  current output while no playback proposes it (ENG-07's held value); a playback that proposes
  it takes it at once in both settings. Playback 1 holds 201 Pan 0.2: programmer Pan 0.8, Clear →
  0.2 in both settings. No playback, profile default Pan 0.5: Clear → `Release` 0.5, `Freeze` 0.8.
- **PRG-44** Undo classes (UNDO-04): Locate, Off (mask and attribute), Rem Dim, every Clear and
  `settings.programmer` are **edit** commands; Highlight is **live**. The located set is part of
  the programmer's edit state (undo.md *Definitions*): undoing a Locate takes the located values
  away again, undoing a Clear brings them back. A timed Clear's fade is live state: undo during
  the fade puts the programmer back at once.

### Colour
- **PRG-30** HSI, RGB and CMY are views of the same colour value: switching the model never
  changes a value or the output. 201 at red: RGB reads (1, 0, 0), CMY (0, 1, 1), HSI (0°, 100 %).
  Hue and Saturation are the HSV hue and saturation of the target colour (Saturation =
  (max − min) / max, 0 for black); turning them keeps max(r, g, b). Wheel values are 0.0–1.0,
  Hue as a fraction of 360°. The colour model is engine state (default HSI), in the programmer
  view.
- **PRG-31** A colour wheel applies to every target with a colour attribute, whatever its colour
  system, and regardless of selection order (Titan v17). Targets 201 (RGBW), 211 (RGB+Lime),
  221 (RGB): Hue to 240° → all three blue; the per-type mapping to emitters is ADR-005 / LIB.
- **PRG-32** Hue turns relatively per target (offsets kept, wraps at 360°); Saturation and the
  RGB/CMY wheels clamp per target (PRG-19).
- **PRG-33** HSI has no intensity wheel of its own: brightness is the fixture's Dimmer
  (group I), so a colour never carries a second, hidden intensity.
- **PRG-34** A direct emitter wheel (White, Lime, any emitter of LIB-17) changes only fixtures that have that emitter and
  is a touched attribute of its own, separate from the target colour. Targets 201 + 221, White
  +50 % → 201 White 0.50 in the programmer, 221 unchanged and not in the programmer.

### Engine state and safety
- **PRG-35** The engine exposes the programmer state for rendering: selection (ordered),
  sub-selection, pattern, active attribute group, colour model, wheel layout with current values
  and source (manual / palette / playback), programmer contents, located set, Highlight on/off,
  plus per fixture "selected", "in programmer" and per attribute group "available for targets"
  and "has programmer values". The IPC messages for this are added to `ipc.md` with the first
  PRG task.
- **PRG-36** Every refused action returns a `Warning` (shown on the command-line message line) and
  changes nothing; no programmer action can stop the tick or the output (ENG-12).
- **PRG-37** Same patch, same timestamped programmer actions → identical programmer state and
  byte-identical frames (ENG-11).

## Scenario test format
Scenario files (`tests/scenarios/PRG-nn-*.json`, format of `engine-merge.md`) gain a `prg` action
and a `programmer` expectation:

```json
{ "t": 0.0, "prg": { "select": [201, 202] } }
{ "t": 0.1, "prg": { "wheel": { "attribute": "Dimmer", "steps": 10, "resolution": "Normal" } } }
{ "t": 0.2, "prg": { "command": ["221", "Thro", "226", "@", "40", "Enter"] } }
{ "t": 0.3, "prg": "clear" }
"expect": [ { "t": 0.4, "programmer": { "selection": [202], "contents": { "201": { "Dimmer": 0.4 } } } } ]
```

Selection, sub-selection and contents rules (PRG-01..11, 14..22, 27..29) are unit tests on
`Programmer`; anything that reaches the frame (PRG-12, 13, 20, 21, 23..26) is a scenario file.

## Out of scope (phase 1 unless noted)
- Fan and align (phase 2, `PROJECT.md`) and Blind (no phase yet): greyed out in UI v0.1.
- Select If, Unfold, per-cell selection of multi-cell fixtures (phase 2; cells as a whole only).
- Gel book / filter numbers (phase 2, IDEA-02); colour calibration (phase 2, IDEA-03).
- Channel mode of the colour picker (one slider per raw emitter) beyond W/Lime: ADR-005 decides.
- Shapes and effects (phase 2). Multiple programmers (multi-user, remotes): one programmer.
- Undo: one engine-wide undo for programmer and show edits, its own item (BACKLOG P1-10, `undo.md`).
- Handle actions on groups other than Record (Set Legend, Copy, Move, Delete): shared with
  palettes and playbacks, specified with them.
- Wheel acceleration, encoder banks and their mapping: `control-surfaces.md`.

## Owner decisions (2026-10-17)
1. Clear defaults to `SelectionThenProgrammer` (UI v0.1); Titan's one-press Clear is a setting.
2. Locate removes existing programmer values of the located attributes (PRG-24), a deliberate
   deviation from Titan's default, so what you see is what you record.
3. Fan and Blind stay out of phase 1; their buttons are greyed out in UI v0.1.
4. Undo is a separate backlog item (P1-10), not part of PRG.
5. A typed selection replaces the current selection; tapping adds (PRG-08 vs PRG-01).
