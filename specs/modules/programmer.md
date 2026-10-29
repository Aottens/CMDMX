# Module spec: programmer (PRG)

Owner: `cmdmx-core::programmer`. Status: **PRG-01..37 approved by owner 2026-10-17 (P1-02); PRG-38..44 approved 2026-10-22 (VS2-05), PRG-45 approved 2026-10-22 (VS5-04); PRG-46..55 approved 2026-10-25 (P1-13); PRG-56..62 approved 2026-10-25 (P1-20); PRG-63..66 approved 2026-10-28 (P1-22); PRG-67..70 approved 2026-10-28 (P1-23)**; **PRG-71..84 (selection orders, P2-02) and the PRG-05/06 amendments: draft for owner approval**.
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
- **Select order:** the order in which the selection's items were selected, kept beside the
  selection for Revert (PRG-71).
- **Grid:** engine state `{wings, groups, blocks, flip}` that gives items shared positions
  (PRG-73); `{1, 0, 1, false}` is *off*.
- **Positions of a list:** for *n* items in order and a grid, each item's position *k* of *N*:
  FX-07's steps 1–3 (wings, groups, blocks), then with `flip` *k* ← *N* − 1 − *k*. Grid off →
  *k* = *i*, *N* = *n*. One function in core, shared with the FX generator.
- **Target positions:** with a sub-selection active, the targets in order with *k* = *i* (the grid
  is not applied inside a stage); otherwise the positions of the list under the grid.
- **Programmer contents:** per fixture, per attribute, a *touched* value: either a manual value
  or a palette reference (`palette_id`). An attribute that was never touched is absent.
- **Located set / highlight:** output overrides that are *not* programmer contents.
- **Active attribute group** and **colour model** (HSI / RGB / CMY / Kelvin): engine state shared by
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
  *Amended (P2-02, for owner approval):* a pattern of `n` stages is the grid `{groups: n}`
  (PRG-73, 74); the behaviour above is unchanged.
- **PRG-06** All ends the sub-selection or pattern (targets = whole selection again). With no
  sub-selection active, All selects every fixture in the programmer, in user-number order.
  *Amended (P2-02, for owner approval):* All ends the sub-selection but keeps the grid (PRG-74), so
  Fan, effects and overlap then run across the stages; `programmer.grid {}` switches the grid off.
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
- **PRG-45** Undo and redo let go of an LTP attribute as Clear does (PRG-43): an LTP value or
  located value that the restored programmer no longer holds follows `clear_ltp` (`Release` → the
  playback value, else the profile default; `Freeze` → it keeps its output). Locate 101, Undo
  → Pan and Tilt return to their defaults (`Release`) instead of staying centred. A value the
  restored programmer still holds, or brings back, outputs as before. Approved by owner 2026-10-22
  (VS2-05 run, finding 3), as grandMA's Oops and Titan's Clear behaviour do.

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

### Colour temperature (IDEA-32, ADR-010; approved by owner 2026-10-28)
Kelvin is a fourth view of the same target colour (PRG-30): it stores nothing new. Temperatures
are points on the Planckian locus (CIE 1931 xy from the Kim et al. cubic spline, valid
1667–25000 K), converted to linear BT.2020 (ADR-005).

- **PRG-63** The colour models are HSI, RGB, CMY and **Kelvin**. Kelvin's wheels are Kelvin, then
  the direct emitters (PRG-17). Switching to Kelvin changes no value and no output.
- **PRG-64** The Kelvin wheel reads the temperature of the nearest locus point (in CIE 1960 uv) to
  the target colour, clamped to 1800–10000 K and shown in K rounded to 10 K; black has no
  temperature ("—"). White (1, 1, 1) reads 6500 K; setting 3200 K and reading it back gives
  3200 K (round trip within 5 K). The wheel moves in mired (10⁶ / K) so warm and cool steps feel
  even: its 0.0–1.0 value is (555.6 − mired) / 455.6, so one `Normal` step is about 1.8 mired.
- **PRG-65** Turning Kelvin sets each target's colour to the locus point at its own new
  temperature (relative per target, PRG-19, clamped to 1800–10000 K), scaled so max(r, g, b)
  keeps its value (1.0 for black). A colour off the locus lands on it at the temperature it
  reads. 201 at full white, Kelvin to 3200 K → (1.0, 0.68, 0.29); 202 at half-level red reads
  1800 K (clamped); one step down → (0.5, 0.19, 0.02) at 1800 K.
- **PRG-66** CTO and CTB steps shift every target's temperature by a nominal gel's mired value,
  as that gel would on a source at that temperature: Full CTO +159, ½ CTO +81, ¼ CTO +42,
  ⅛ CTO +20; Full CTB −137, ½ CTB −78, ¼ CTB −35, ⅛ CTB −18; then PRG-65 (clamp, keep max). They
  work in any colour model and touch Colour. 201 at 5600 K, ½ CTO → 3850 K; 202 at 3200 K,
  Full CTB → 5700 K; 203 at 5600 K, Full CTB → 10000 K (clamped). Gel numbers: PRG-67..70.

### Gel numbers (IDEA-02, P1-23; approved by owner 2026-10-28)
A gel number is one more way to *set* the target colour, next to the wheels, the picker field and
the CT steps; nothing new is stored (ADR-005 decision 5, ADR-010 decision 5). A gel's colour is
the chromaticity its maker publishes for the filter on a **3200 K tungsten source** (LEE's
"Tungsten" x, y), converted to linear BT.2020 (ADR-005), negative components set to 0, scaled so
the largest component is 1: the colour the number has always meant on a theatre lamp (ETC Eos
and Hog pick gels on a tungsten reference too). Brightness is not part of it (PRG-33): a gel's
transmission is not applied. Phase 1 has the LEE range; Rosco and GAM follow as data allows.

- **PRG-67** The engine carries a compiled-in gel table per range (LEE first:
  `subprojects/engine/gels/lee.json`, rows `{"number":201,"name":"Full C.T. Blue","x":0.330,
  "y":0.339}`, its source and permission in a `NOTICE` beside it). Colour filters only:
  diffusion and neutral-density filters are not in it. A row whose x, y does not convert (y ≤ 0,
  off the chromaticity diagram, no positive component; amended in VS2-48) or a duplicate number fails a test, never the engine. LEE 201 →
  (1.0, 0.920, 0.837); LEE 106 Primary Red → (1.0, 0.029, 0.001); LEE 181 Congo Blue (0.187,
  0.058) → (0.153, 0.0, 1.0) (green below 0, set to 0).
- **PRG-68** `Lee n Enter` sets every target's Colour to LEE n's colour, scaled so max(r, g, b)
  keeps its value (1.0 for black, as PRG-65), and touches it. It has the place of `Palette n` in
  a line (PRG-10): `201 Thro 208 Lee 106 Enter` selects and applies. Leading zeros do not count
  (`Lee 79` is 079 Just Blue). It works in any colour model and does not change the model; a
  colour-wheel fixture takes the nearest slot (ADR-005 decision 3); fixtures without Colour are
  skipped. A number not in the table → warning "LEE 999 is not in the gel table", no targets
  with Colour → warning; nothing changes (PRG-36). 201 at full white, `Lee 106` →
  (1.0, 0.029, 0.001); 202 at half-level red, `Lee 201` → (0.50, 0.46, 0.42).
- **PRG-69** A colour reads as a gel when, divided by its max(r, g, b), it equals that gel's
  colour within 0.002 per component (the lowest number when two match; black reads none). The
  engine state (PRG-35) carries this reading for the first target with Colour, as it carries
  Kelvin (PRG-64). 201 after `Lee 201` reads LEE 201, also at half level; after one Hue step it
  reads none.
- **PRG-70** CT gels and CT steps agree: picked on its own, each LEE CT gel reads (PRG-64) within
  8 mired of 3200 K plus the matching PRG-66 step: 223/206/205 ↔ ⅛/¼/½ CTO, 218/203/202/201 ↔
  ⅛/¼/½/Full CTB (LEE 201 reads 5600 K, 3200 K + Full CTB is 5700 K). LEE 204 ↔ Full CTO within
  16 mired: LEE's own data puts 204 on tungsten at 2190 K, the nominal +159 at 2120 K; both kept.
  (PRG-66's ½, ¼, ⅛ CTO values +81, +42, +20 are what LEE 205, 206, 223 measure on tungsten,
  not the +109, +64, +26 LEE prints for them, which hold on daylight.)

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

### Cells of multi-cell fixtures (P1-13, approved by owner 2026-10-25)
Builds on LIB-07: a multi-cell mode has cells 1..n in profile order, each with its own Colour,
emitters and optionally its own Dimmer; the mode's own attributes (Pan, Tilt, master Dimmer,
Strobe, Zoom) belong to the fixture as a whole. Church example: Shehds 901–904, three RGBW cells
each, a master Dimmer and Pan/Tilt on the fixture. Titan reference: *Controlling Fixtures*,
Multi-Cell Fixtures and Sub-Fixtures (keypad table) and Unfold.
- **PRG-46** Selection items. A selection item is a whole fixture (`901`) or one cell
  (`901.2`, cell numbers 1-based). Every selection rule (PRG-01..07) works on items: the selection
  is an ordered list of items without duplicates, so `901` and `901.2` are different items and may
  both be selected. Tap 901.2, 901.3, 901.2 → `[901.3]`. A cell item of a fixture without cells, or
  a cell number above the mode's cell count, does not exist (skipped on the command line as
  PRG-08 skips unpatched numbers).
- **PRG-47** Item attributes. A whole-fixture item has the fixture's attributes as today
  (LIB-07: its Colour and emitters drive every cell). A cell item has only its cell's attributes
  (Colour, emitters, its own Dimmer) and, when the cell has no Dimmer channel, a virtual Dimmer
  that scales that cell's colour and emitters (as `virtual_dimmer` does for a mode, PRG-09); the
  fixture's master Dimmer still scales all cells on top. Wheels (PRG-17..19), keypad levels
  (PRG-09), Locate (PRG-38), Highlight (PRG-39), Off (PRG-27, 40) and palettes act on an item
  through exactly these attributes. Targets 901.2 only → active group C or I; Pan is not on the
  wheels. `901.2 @ 50 Enter` → cell 2's Dimmer 0.50, master Dimmer untouched. The virtual cell
  Dimmer is proposed only by cell values (the fixture's Dimmer is the master, not a value for it);
  nobody proposing it means full, and the Grand Master and group masters act on the master Dimmer
  only, so a cell is never mastered twice (VS2-31, 2026-10-25).
- **PRG-48** Command line (Titan keypad syntax; `.` is its own token, `901.2` typed on the
  keypad is the tokens `901` `.` `2`). In fixture order, then cell order:
  `n . m` cell m of n · `n .` all cells of n · `n . m Thro i` cells m..i of n ·
  `n . m Thro . j` cells m..j of n · `n Thro i . j` cell j of each of n..i ·
  `n Thro i .` all cells of n..i · `n . m Thro i . j` cells m..j of each of n..i ·
  `. m` cell m of every selected fixture · `.` all cells of the selected fixtures ·
  `. m Thro . j` cells m..j of the selected fixtures. `And`, `Not` and `@` combine as in PRG-08/09.
  `901 . 2 Thro 3 Enter` → `[901.2, 901.3]`; `901 Thro 904 . 2 Enter` →
  `[901.2, 902.2, 903.2, 904.2]`; `901 Thro 902 . Enter` → `[901.1, 901.2, 901.3, 902.1, …, 902.3]`;
  `Not` removes the items it names: `901 . Not 901 . 2 Enter` → `[901.1, 901.3]`. A fixture in a cell range that has no
  cells is skipped; nothing left → refused (PRG-08). "Selected fixtures" for the `.`-first forms
  are the fixtures that are in the selection or have a cell in it, in selection order.
- **PRG-49** Group selection and recording store items: a group recorded with `[901.1, 902.1]`
  selects those two cells again (Titan stores the cell state with the group). PRG-02's toggle
  compares items.
- **PRG-50** Fix ±1 and patterns step through items by position (PRG-04/05): selection
  `[901.1, 901.2, 901.3]`, Odd/Even → `[901.1, 901.3]`. All (PRG-06) with no sub-selection
  selects every item in the programmer, in fixture order then whole fixture before its cells.
- **PRG-51** Whole fixture and cell in the merge. Within one source a cell's own value beats the
  fixture value for that cell: programmer 901 Colour red and 901.2 Colour blue → cells 1 and 3
  red, cell 2 blue; Off on 901.2 → cell 2 red again. Between sources the fixture value counts as
  a value for every cell, so ENG rules (priority, HTP, latest LTP) decide per cell attribute:
  playback 1 holds 901 red, programmer touches 901.2 blue → cell 2 blue, cells 1 and 3 red.
  In the programmer an item's entry is its value, else its located value, and the cell's entry
  beats the fixture's, so Locate on 901.2 shows on cell 2 while 901 holds red. Off and Locate act
  on the items they target only: Off on 901 leaves 901.2's own values (VS2-31, 2026-10-25).
  A cell value a cue list stops holding is released to its fixture, not faded to 0 as the cell's
  own value (CUE-13 amendment, VS2-38, approved 2026-10-26).
- **PRG-52** Programmer contents, cues, groups and palettes key cell values by item
  (`"901.2"`): `contents(mask)` (PRG-15) returns them under their item, Record stores them so,
  Include (PRG-16) loads them back. A palette reference on a cell resolves as PAL-07 with the cell
  first: per-item value for `901.2`; else the per-fixture value for 901 of that attribute; else
  the shared value for the type's cell 2; else the shared value for the type; else global. So a
  colour palette recorded on whole fixtures also colours one cell it is applied to.
- **PRG-53** A cell item whose cell no longer exists (Change Mode to a mode with fewer cells) is kept
  in groups, cues and palettes but outputs nothing and is skipped on select, as PAL-19 does for an
  attribute the new mode lacks. Removing the fixture removes its cell items as it removes the
  fixture (PAT-12 / undo).
- **PRG-54** Rem Dim (PRG-28, 41): a fixture with any selected item keeps its intensity; every
  other fixture gets master Dimmer 0 (its cells untouched). Highlight lowlight (PRG-26) applies
  per item: the other selected items get the lowlight level on their own intensity.
- **PRG-55** Engine state (PRG-35) lists items: selection and sub-selection are item lists,
  per fixture "selected" means the whole fixture is selected and "cells selected" lists its
  selected cell numbers; "in programmer" holds per fixture whether the whole fixture and which
  cells have programmer values.

### Fan (P1-20, IDEA-27, approved by owner 2026-10-25)
Titan reference: *Controlling Fixtures*, Changing Fixture Attributes, Fan Mode. Fan spreads a
wheel change over the targets in their order, so the ends move most and the middle least (Line),
or in one of the other curves. It is engine state like the active attribute group, so the
screen wheels and every control surface fan alike; the client only says "fan on, curve X".
- **PRG-56** Fan mode is engine state: off, or on with a curve `Line`, `Mirror`, `Wings` or
  `Arrow`; the engine also keeps the last curve (default `Line`). The Fan key toggles: off → on
  with the last curve, on → off. Choosing a curve turns Fan on with that curve and makes it the
  last curve. Fan stays on (latched, Titan's default) until the Fan key is pressed again or the
  selection changes (PRG-01..03, 08, a group select, Clear of the selection); Fix ±1, patterns
  and All (PRG-04..06) change only the targets and keep Fan on. Fan mode and the last curve are
  live state: not undoable (UNDO-04), not saved in the show, off when the engine starts.
- **PRG-57** Fan positions. For a wheel on attribute *a*, the fan list is the targets that have
  *a*, in target order: selection order, a group in its stored order (PRG-02), cells as items
  (PRG-46) — the whole selection is one list (Titan's "Ignore Groups"). With *n* items in the
  list, item *i* (0-based) sits at `x = i / (n − 1)`. Items without *a* take no position, so a
  spot between two washes does not break a Pan fan. With *n* < 2 the wheel acts as without Fan
  (PRG-19), so it never seems dead. Targets 901–904 unfolded to `[901.1, …, 904.3]` on Hue → a
  fan over 12 cells.
- **PRG-58** Fan curves give each position a factor *k* in −1..1:
  - **Line** `k = 2x − 1`: 5 items → −1, −½, 0, ½, 1; 6 → −1, −0.6, −0.2, 0.2, 0.6, 1. The
    middle stays; the ends move most, in opposite directions (Pan spread).
  - **Mirror**: the first ⌈n/2⌉ and the last ⌈n/2⌉ positions are two halves (an odd middle item is
    in both); each half is a Line of its own, the second mirrored (`k_i = k_{n−1−i}`): 5 → −1, 0, 1,
    0, −1; 6 → −1, 0, 1, 1, 0, −1. A half of one item has `k = 0`.
  - **Wings**: `s = ⌊n/3⌋` items in each outer wing, the rest is the middle with `k = 0`; the left
    wing has `k = −(s − j)/s` for its item *j* (0-based from the outer end), the right wing the
    same with `+`: 6 → −1, −½, 0, 0, ½, 1; 9 → −1, −⅔, −⅓, 0, 0, 0, ⅓, ⅔, 1; 5 → −1, 0, 0, 0, 1.
    With *n* < 3 every `k` is 0.
  - **Arrow** `k = |2x − 1|`: 5 → 1, ½, 0, ½, 1; 6 → 1, 0.6, 0.2, 0.2, 0.6, 1. The ends move
    together, the middle stays (a V on Tilt or Dimmer).
  These replace the formulas of the UI v0.1 sketch, where Mirror and Arrow were each other's
  negative and so one curve (the wheel's direction flips the sign). Titan's Pull curves and Fan
  parts are phase 2 (Out of scope).
- **PRG-59** A fanned wheel turn by *d* (the step size of PRG-21 times `steps`) changes item *i* by
  `d · k_i` from its own current value (its programmer value, or its output value when untouched,
  PRG-20), clamped per item (PRG-19); Hue wraps (PRG-32). Every item in the fan list becomes a
  touched value, also those with `k = 0`, so recording stores the whole fan. Washes 101–105 at
  Pan 0.5, Fan Line, +10 `Coarse` (+0.40) → 0.10, 0.30, 0.50, 0.70, 0.90; a −10 turn → all 0.50
  again. Fan Arrow on Dimmer, 201–205 at 0.20, +10 `Coarse` → 0.60, 0.40, 0.20, 0.40, 0.60.
  Changing a palette reference this way makes it manual per item (PRG-22).
- **PRG-60** Fan works on every wheel of the layout (PRG-17): any attribute group, the colour
  model's wheels and direct emitters. A colour wheel's fan list is the targets with a colour
  attribute (PRG-31): 201–208 at red, Fan Line on Hue, +0.25 → a rainbow from 270° to 90° through
  red. Only wheel turns fan: keypad levels (`@`, PRG-09), palettes, Locate, Highlight, Off and the
  absolute `programmer.set` (colour picker) act as without Fan.
- **PRG-61** Fanned values are plain manual values per item: `contents()` (PRG-15), Record,
  Update, cues and palettes need nothing new (a fanned palette is detected per fixture, PAL-04).
  A fanned wheel turn is a wheel command for undo: one step, coalesced with further turns on the
  same attribute (UNDO-05); the live Fan commands end that run.
- **PRG-62** Engine state (PRG-35) carries `fan` (`null` or the curve) and `fan_curve` (the last
  curve).

### Selection orders and the grid (P2-02, IDEA-15; draft for owner approval)
Titan reference: *Controlling Fixtures* (Fix ±1, pattern select from All, custom patterns, Random
Order on a held group key), *Fixture Groups* (a group stores its order) and *Cue Timing* (fixture
overlap follows the order the cue was recorded in; Fixture Order with shared step numbers). The
grid is grandMA3's MAtricks (wings, groups, blocks, a shuffle seed); Onyx's grouping tools (Every,
Block, Divide, Mirror; Random, Reverse, Sort, Revert) map onto it. Research and comparison:
`docs/research/selection-orders.md`.

Two separate things: the **order** says which item comes first (a permutation of the selection);
the **grid** says which items count as the same position. Everything that spreads over a
selection reads the same positions (Definitions): Next/Previous, Fan, a new effect, palette
overlap and cue overlap. Examples use RGBW pars 201–208 and spots 301–306; positions are written
in selection order.

- **PRG-71** Order tools. `Reverse`, `Shuffle` (PRG-72), `Sort` and `Revert` reorder the selection
  and change nothing else: not the programmer contents, not the grid, not Fan. They are no
  programmer change for PRG-03 (the next tap still adds). Selection `[203, 201, 202]`: Reverse →
  `[202, 201, 203]`; Sort → `[201, 202, 203]` (user number, a whole fixture before its cells, cells
  in cell order, as PRG-50); Revert → the select order `[203, 201, 202]`. An item selected after a
  reorder goes to the end of the selection and of the select order. With a stage selected
  (PRG-74) the stage keeps its number and is recomputed on the new order. Fewer than two items →
  refused ("Order: select at least two fixtures"). Undo: edit commands that coalesce with the
  selection commands (UNDO-04, 05).
- **PRG-72** Shuffle gives a new order from a fixed permutation (Fisher–Yates driven by SplitMix64)
  seeded by the engine's shuffle counter, which is 1 when the engine starts and goes up by one per
  Shuffle; same actions → same order (PRG-37, ENG-11). Every press gives a new order (owner
  2026-10-18): a seed that gives back the current order is skipped for the next one, so two items
  always swap. The counter is live state: not saved, not undone. Shuffle twice, Revert → the select
  order again.
- **PRG-73** The grid. `programmer.grid` sets the whole grid: fields left out take their defaults
  (`wings` 1–8, `groups` 0 = off or 2–32, `blocks` 1–32, `flip`; FX-06's ranges), out of range is
  refused and changes nothing. 201–208:

  | Grid | Positions | *N* | Name (PRG-75) |
  | --- | --- | --- | --- |
  | groups 2 | 0 1 0 1 0 1 0 1 | 2 | Odd/Even, 1 in 2 |
  | groups 3 | 0 1 2 0 1 2 0 1 | 3 | 1 in 3 |
  | blocks 2 | 0 0 1 1 2 2 3 3 | 4 | Blocks 2 |
  | blocks 4 | 0 0 0 0 1 1 1 1 | 2 | Halves |
  | wings 2 | 0 1 2 3 3 2 1 0 | 4 | Outside in |
  | wings 2, flip | 3 2 1 0 0 1 2 3 | 4 | Inside out |
  | groups 4, blocks 2 | 0 0 1 1 0 0 1 1 | 2 | 2 in 4 |

  The grid stays while items are tapped in or out (positions are recomputed) and through order
  tools, Fix ±1 and All; it ends with `programmer.grid {}` and when a new selection starts (PRG-03,
  a typed selection PRG-08, All selecting the programmer PRG-06, Clear of the selection), unless a
  group brings its own (PRG-79). Undo as PRG-71; not saved in the show except with a group.
- **PRG-74** Stepping by position. With the grid on, the sub-selection is a **stage**: the items at
  position *s*. Setting a grid selects stage 0 (as PRG-05 did); Fix +1 / Fix −1 (Next / Previous)
  move *s* through 0…*N* − 1, wrapping. All ends the stage and keeps the grid; the next Fix +1
  selects stage 0 (Fix −1 stage *N* − 1). Highlight follows the stage (PRG-39). Grid off: PRG-04
  unchanged. 201–208, Outside in → `[201, 208]`, Fix +1 → `[202, 207]`; Inside out →
  `[204, 205]`; 2 in 4 → `[201, 202, 205, 206]`, Fix +1 → `[203, 204, 207, 208]`.
- **PRG-75** Shortcuts, in the engine so every surface gives the same grid; each replaces the whole
  grid and selects stage 0: **Odd/Even** = groups 2 · **1 in n** = groups *n* (2–32) · **n in m** =
  groups *m*, blocks *n* (1 ≤ *n* < *m* ≤ 32; Titan's `2 In 6`) · **Blocks n** = blocks *n* ·
  **Divide d** (2–32: halves, thirds, quarters …) = blocks ⌈*len*/*d*⌉ for the selection's length at
  that moment · **Outside in** = wings 2 · **Inside out** = wings 2, flip · **Grid off**. With an
  empty selection each is refused ("Select fixtures first"); a grid on one item is allowed
  (*N* = 1). Twelve pars `2 in 6` → 0 0 1 1 2 2 0 0 1 1 2 2.
- **PRG-76** Fan on positions. PRG-57's fan list keeps its rule (the targets that have attribute
  *a*, in target order); each item takes its target position over that list, and
  *x* = *k* / (*N* − 1); with *N* < 2 the wheel acts as without Fan. Grid off → PRG-57 unchanged.
  201–208 at red, Odd/Even, All, Fan Line on Hue +0.25 → odd pars 270°, even pars 90° (odd against
  even). Spots 301–306 at Pan 0.5, Outside in, All, Fan Line +0.40 → 0.10, 0.50, 0.90, 0.90, 0.50,
  0.10. With a stage selected the fan runs over the stage's items plainly.
- **PRG-77** Effects take the order. FX-14's members are the targets in target order, so a reversed
  or shuffled selection runs in that order (a shuffled Dimmer chase is Titan's random chase). With
  no stage selected and the grid on, a new entry takes `wings`, `groups`, `blocks` and `flip` from
  the grid instead of the look's defaults; the look's other defaults (spread too) stay. 201–208,
  Outside in, All, *Dimmer chase* → the chase runs from both ends to the middle; 1 in 3, All,
  *Rainbow wave* → three colours repeating over the pars. Reordering a running entry: FX-45.
- **PRG-78** Palette overlap on positions. PAL-14's *i* is the target's position *k* over the
  targets the palette touches; items with the same position start together. 201–208, Outside in,
  All, a colour palette with *t* = 2 s, overlap 0 % → 201 and 208 start at 0 s, 202 and 207 at 2 s,
  …, all done at 8 s. Grid off, and a quick palette (user-number order), → PAL-14 unchanged.
- **PRG-79** Groups keep the order and the grid. PRG-07 already stores the selection in its order,
  so a reversed or shuffled order recorded into a group comes back. Recording a group while the
  grid is on also stores the grid (group `grid`, absent when off). Selecting that group when it
  starts a new selection (PRG-03, a typed `Group n`, an empty selection) restores its grid with no
  stage selected (as after All), so Fan, effects and overlap use it at once and Fix +1 starts at
  stage 0; selected into an existing selection, the group appends its items and leaves the grid.
  Group "Pars outside in" (201–208, wings 2) → `Group 5 Enter`, *Dimmer chase* runs inwards.
- **PRG-80** Cues keep the order. Recording a cue (PBK-09..13, CUE-43..46: Record, Update, Merge)
  stores per recorded item its position, `order` (`{"201":0,"208":0,"202":1,…}`): the selected
  items their positions under the grid over the whole selection (a selected stage does not
  matter), then recorded items outside the selection in user-number order at *N*, *N* + 1, …. On a
  timed Go, `overlap` (PBK-15) staggers by `order` as PRG-78 does; a cue without `order` (older
  shows, a cue made without a selection such as the PBK-45 migration) keeps user-number order.
  Update and Merge keep the positions of items already in the cue and add new items after the
  cue's highest position, in their recorded order. 201–208 Outside in, Record cue 2 with fade 2 s,
  overlap 0 % → on Go 201 and 208 first, 204 and 205 last.
- **PRG-81** A cue's fixture order can be edited afterwards (Titan *Fixture Order*):
  `cue.order {"list":1,"cue":2,"order":{"205":0,"204":0,"206":1,…}}` replaces it (an edit step,
  UNDO-04); items of the cue left out of the message follow after the given ones in their old
  order; an item not in the cue, or a negative position, is refused. The app's editor: UI-34.
- **PRG-82** Engine state (PRG-35) carries `grid` (`null` when off, else `{wings, groups, blocks,
  flip}`), `stage` (*s* or `null`), `positions` (per selection item its *k*, in selection order)
  and `position_count` (*N*); a cue in the cue view carries its `order` (absent when none).
- **PRG-83** Show schema 9 (SHW-04): groups gain the optional `grid`, cues the optional `order`,
  effect parameters the optional `flip` (FX-06, absent when false). Schema 8 shows load unchanged;
  a show without them saves byte-identical apart from the version number.
- **PRG-84** Positions are computed when the selection, the grid or an entry's members change,
  never per tick (ENG-13, FX-13); no selection length, grid or order makes the engine panic (0, 1
  or more items than positions included), and refused actions change nothing (PRG-36).

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
- Fan beyond PRG-56..62 (phase 2): Titan's Pull Middle, Pull Ends and Pull End curves, Fan parts
  (hold Fan + a number), the group options Fan Within Group and Fan Group as Fixture, press-and-hold
  Fan as a setting, and a keypad fan (`@ 0 Thro 100`). Align
  (phase 2, `PROJECT.md`) and Blind (no phase yet): greyed out in UI v0.1.
- Selection orders beyond PRG-71..84 (later): grandMA3's Width (several positions per step), Shift
  and X/Y/Z axes, From/To layers of fade, delay and phase over the grid, a MAtricks pool; Titan's
  2D Direction from a layout (needs IDEA-25); a keypad syntax for patterns (`2 In 6`), Invert
  (swap selected and deselected). Select If (phase 2). Cells: per-cell group masters, Titan's cell buttons in an attribute
  editor, and Shapes or pixel mapping over cells (phase 2); cells are selected by `.` and Unfold
  (PRG-46..55, UI-29).
- Gel ranges beyond LEE (Rosco, GAM: as data allows), a daylight-source option for gels, a gel's transmission as dimming, and colour calibration (phase 2, IDEA-03).
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
3. Fan and Blind stay out of phase 1; their buttons are greyed out in UI v0.1. (Fan: reversed
   2026-10-25, IDEA-27; PRG-56..62.)
4. Undo is a separate backlog item (P1-10), not part of PRG.
5. A typed selection replaces the current selection; tapping adds (PRG-08 vs PRG-01).

## Owner decisions (2026-10-25, P1-13)
1. Cell ranges read as Titan's: `901 . 2 Thro 4` is cells 2–4 of 901, `901 Thro 904 . 2` is cell 2
   of each (PRG-48), not grandMA's running count.
2. A cell without its own Dimmer gets a virtual Dimmer; the master Dimmer scales on top (PRG-47).
3. Unfold opens cells in place behind their fixture instead of replacing the page (UI-29).
4. Within one source a cell value beats the fixture value; between sources the ENG rules decide
   per cell (PRG-51).

## Owner decisions (2026-10-25, P1-20: all seven as recommended)
1. Phase 1 has the four curves of UI v0.1 with the factors of PRG-58; Pull curves and Fan parts
   are phase 2.
2. The Fan key toggles (latched); a new selection turns Fan off, Fix ±1, patterns and All keep it
   (PRG-56). Press-and-hold is a phase-2 setting.
3. Fan follows the selection order, a group its stored order, the whole selection as one list
   (PRG-57); the group options are phase 2.
4. Fan works over items, cells included; only items with the attribute take a position (PRG-57).
5. Only wheel turns fan; no keypad fan in phase 1 (PRG-60).
6. The encoders fan through the engine state; a `fan` control action for a button is phase 1
   (CTL-59).
7. An item with factor 0 still becomes a touched value, so Record stores the whole fan (PRG-59).

## Owner decisions (P2-02, proposed 2026-10-29)
1. Order and grid are separate: order tools permute the selection, the grid gives shared positions
   with the FX-07 function; all spreading features read the same positions (PRG-71..78).
2. All ends a stage but keeps the grid, so Fan and effects run across the stages after All
   (Onyx's Fan/FX toggle, MA3's MAtricks); `Grid off` or a new selection ends it (PRG-06, 73, 74).
3. A group stores its grid next to its order and restores it when it starts a selection (PRG-79).
4. Cues store each item's position, so overlap replays the recorded order (Titan); older cues keep
   user-number order (PRG-80); a fixture-order editor follows in its own slice (PRG-81, UI-34).
