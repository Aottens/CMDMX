# Module spec: fixture-library (LIB)

Owner: `cmdmx-core::library` (model, parsing, conversion; pure) and `cmdmx-daemon` (reading files,
the bundled QLC+ library). Status: **LIB-01..16 approved by owner 2026-10-17 (P1-01); LIB-17 approved by owner 2026-10-17 (generic emitters)**. Decisions: ADR-006 (own
model, importers, QLC+ first), ADR-005 (target colour, direct emitters). Reference: QLC+ fixture
definition format (`.qxf`, `engine/src/qlcchannel.h` presets), Titan *Personality Builder*.

## Purpose
Every fixture in a show has a profile that says which channel does what. ColourmotionDMX does not
build its own library: it imports the open QLC+ library and lets the operator type a profile from
a channel list for anything missing. Whatever the source, the engine only ever sees one small
model, and a show carries the profiles it uses.

## Definitions
- **Profile:** one fixture type: `manufacturer`, `model`, `source` (e.g. `qlcplus:Shehds/…qxf`,
  `channel-list`), and one or more **modes**. Identified by `manufacturer/model`.
- **Mode:** `name`, `footprint` (channels), **attributes**, an optional **colour mixing system**,
  `virtual_dimmer` (true when the mode mixes colour but has no dimmer channel), optional
  **cells** and optional physical ranges (`pan_degrees`, `tilt_degrees`, `zoom_degrees`).
- **Cell:** one head of a multi-head mode, with its own colour components, emitters and
  optionally its own dimmer channel.
- **Attribute:** `name` (Dimmer, Pan, Tilt, Strobe, Zoom, Focus, White, Lime, …), `group`
  (I P C G B E S FX), `kind` (HTP/LTP), `channel` (1-based offset), optional `fine` offset,
  optional `slots` for wheel-type channels (ranges with a name and, for colour wheels, a colour), `default` (0.0–1.0, default 0), optional
  `locate` and `highlight` values, and `emitter` (true for an extra emitter, LIB-17).
- **Colour mixing system (ADR-005):** `RGB` or `CMY`, with the channel offset of each component.
  It is the fixture's one `Colour` attribute (group C, LTP). Extra emitters (White, Warm White,
  Cold White, Amber, Lime, UV, Indigo, and Cyan/Magenta/Yellow next to RGB) are ordinary C
  attributes marked `emitter`. A mode can have any number of them; a multi-cell mode has them
  per cell.
- **Import report:** per source channel: what it became (attribute, colour component, emitter) or
  "kept as a plain channel", plus warnings.

## Acceptance criteria

### Model and storage
- **LIB-01** A profile round-trips through JSON unchanged (`from_json(to_json(p)) == p`), keys in a
  fixed order (git-friendly, as SHW-02). The minimal test profiles of the scenario files stay
  valid as single-mode profiles (no mode name = mode "default").
- **LIB-02** A mode is rejected on load with a clear error if an attribute or colour component
  points outside the footprint, two attributes claim the same channel, or a `default`, `locate`
  or `highlight` value is outside 0.0–1.0. A rejected profile never reaches the patch.
- **LIB-03** An attribute that no source has proposed since the fixture was patched outputs its
  `default` (Showtec LED Par 56 "Mode" channel default 0 = RGB control, so the par does not run its
  built-in program). HTP attributes default to 0 (ENG-07 unchanged).
- **LIB-04** A `virtual_dimmer` mode outputs each colour channel as component × Dimmer, where Dimmer
  is a normal HTP attribute of that fixture without a channel of its own. Showtec LED Par 56 at
  Colour (1, 0.5, 0) and Dimmer 0.5 → R 128, G 64, B 0.

- **LIB-17** Emitters are generic: the engine recognises an extra emitter only by its `emitter`
  flag, never by its name, so every rule for emitters (direct wheels PRG-34, ADR-005 palette
  zeroing, LIB-03 defaults) holds for any number of them with any name. Each emitter name is
  unique within a mode (cell). Importers name an emitter after its preset (White, Amber, …);
  when two channels of a mode would get the same name (QLC+ has no Warm/Cold White preset, so
  both are `IntensityWhite` or colour hint White), each takes its channel name ("Warm White",
  "Cold White"). Cyan/Magenta/Yellow channels in a mode that also has Red/Green/Blue are
  emitters, not a CMY system (RGBACL par: RGB system + emitters Amber, Cyan, Lime). Loading a
  mode where `emitter` is set on an attribute outside group C, or where two emitters share a
  name, is rejected as LIB-02.

### QLC+ import (`.qxf`)
- **LIB-05** Preset mapping: `IntensityMasterDimmer`/`IntensityDimmer` → Dimmer (I, HTP);
  `IntensityRed/Green/Blue` → RGB system; `IntensityCyan/Magenta/Yellow` → CMY system, but
  emitters when the mode also has Red/Green/Blue (LIB-17);
  `IntensityWhite/Amber/UV/Indigo/Lime` → emitter attributes White/Amber/UV/Indigo/Lime (C, LTP);
  `ColorCTOMixer/CTCMixer/CTBMixer` → CTO / CTC / CTB (C, LTP, not emitters);
  `PositionPan`/`PositionTilt` → Pan/Tilt (P, LTP); `ShutterStrobe*` → Strobe (I, LTP);
  `ShutterIris*` → Iris (B); `BeamZoom*` → Zoom (B); `BeamFocus*` → Focus (B); `ColorWheel` →
  Colour Wheel (C); `ColorMacro` → Colour Macro (C); `GoboWheel`/`GoboIndex` → Gobo / Gobo Index
  (G); `PrismRotation*` → Prism Rotation (E); `Speed*` → Pan/Tilt Speed (S). Every `…Fine` preset
  becomes the `fine` offset of its coarse attribute.
- **LIB-06** Channels without a preset map by their QLC+ group: Intensity → I, Colour → C, Gobo → G,
  Prism → E, Shutter → I, Beam → B, Pan/Tilt → P, Speed/Effect/Maintenance/Nothing → S; the
  attribute takes the channel's name, LTP, default 0, and the report marks it "kept as a plain
  channel". A channel whose QLC+ colour hint names a colour (Red, Green, …) is treated like the
  matching preset.
- **LIB-07** Heads become cells: Shehds 19x15 "24 Channel" (three RGBW heads) → a mode with three
  cells, each with its own RGB components and White. The fixture as a whole still has one Colour
  and one White: setting them drives every cell (the master), so the fixture works before
  per-cell control exists. Per-cell selection (`201.2`, Unfold) is a programmer task with its own
  PRG criteria; the model does not change for it.
- **LIB-15** Wheel-type channels keep their slots: for each QLC+ capability range its name, and for
  colour-wheel slots its colour (QLC+ `Res1`/colour hint) converted to a target colour (ADR-005),
  so a target colour can later pick the nearest slot. `ColorWheel` → Colour Wheel (C) with slots;
  `GoboWheel` → Gobo (G) with slot names.
- **LIB-16** Physical ranges are imported where the source has them (QLC+ `Focus PanMax/TiltMax`,
  `Lens DegreesMin/Max`): Shehds 19x15 → pan 540°, tilt 180°, zoom 8–55°. They are stored for
  degree display on the wheels and position palettes; nothing depends on them yet.
- **LIB-08** A mode that mixes colour but has no Dimmer gets `virtual_dimmer = true` (Showtec LED
  Par 56 "6 Channel"). A mode with both a master dimmer and colour channels does not.
- **LIB-09** Every mode of the file is imported with its QLC+ name and footprint. Scenario test:
  the QLC+ files for Shehds LED Beam+Wash 19x15W RGBW Zoom, Showtec LED Par 56 and Pro-Lights
  LumiPar 7QPro are checked into `tests/fixtures/qlcplus/` with their expected profiles; the
  importer reproduces them exactly.
- **LIB-10** Malformed XML, a mode referencing an unknown channel, or a channel number outside
  the mode → the import fails with an error naming the file and the problem; nothing is added to
  the library. Unknown presets are imported as plain channels with a warning, not a failure.

### Profile from a channel list
- **LIB-11** The engine builds a profile from `manufacturer`, `model`, `mode` and an ordered list of
  channel types: Dimmer, Dimmer fine, Red, Green, Blue, White, Warm White, Cold White, Amber,
  Lime, UV, Indigo, Cyan, Magenta, Yellow, Emitter (with a name, e.g. "Royal Blue"), Pan, Pan
  fine, Tilt, Tilt fine, Strobe, Zoom, Focus, Colour Macro, or Other (with a name, group and
  default). Same mapping rules as LIB-05/08: U'King RGBL as `[Red, Green, Blue,
  Lime]` → Colour RGB on 1–3, Lime on 4, `virtual_dimmer = true`.
- **LIB-12** A profile can be edited after import or creation (attribute names, groups, defaults,
  locate/highlight values) and saved as a user profile; a user profile with the same
  `manufacturer/model` wins over the bundled QLC+ one.

### Library
- **LIB-13** The daemon serves the library: the bundled QLC+ set (fetched at build time from a
  pinned QLC+ commit, Apache-2.0 notice included) plus user profiles from
  `~/Library/Application Support/ColourmotionDMX/profiles/`. A search by text returns
  manufacturer, model and mode names without converting every file; a profile is converted when
  it is picked.
- **LIB-14** IPC (extends `ipc.md`): `library.search {"query":"shehds"}` → to the asking client only
  `{"v":0,"type":"library.results","items":[{"manufacturer":"Shehds","model":"…","modes":["16 Channel","24 Channel"],"source":"qlcplus"}]}`
  (at most 50 items); `library.import_report {"manufacturer","model"}` → the report;
  `library.save_channel_list {...}` → creates a user profile (LIB-11). Errors as IPC-06.

## Out of scope (this spec)
The GDTF importer (its own spec section and task after QLC+, phase 1, ADR-006) and the OFL
importer (later: QLC+ already covers most of OFL's budget fixtures); Lightkey profile import
(checked once a file is available); per-cell selection (PRG task, LIB-07); picking a colour-wheel
slot from a target colour (VS2-06 follow-up, LIB-15 provides the data); showing degrees on the
wheels (UX); a graphical profile editor beyond LIB-12's fields.

## Open questions for the owner
1. Shehds 19x15: QLC+ calls the zoom channel **Focus** (`BeamFocusNearFar`). If Lightkey calls it
   Zoom and it zooms, we rename it in the user profile after the church visit (LIB-12).
2. Church check tomorrow: which QLC+ mode matches each fixture's DIP/menu setting (e.g. Shehds 16 or
   24 channels)? The Lightkey export will tell.
