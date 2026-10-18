# Module spec: fixture-library (LIB)

Owner: `cmdmx-core::library` (model, parsing, conversion; pure) and `cmdmx-daemon` (reading files,
the bundled QLC+ library). Status: **LIB-01..16 approved by owner 2026-10-17 (P1-01); LIB-17 approved by owner 2026-10-17 (generic emitters); LIB-18 approved by owner 2026-10-17 (import conventions); LIB-14 detail and LIB-18 tolerance approved by owner 2026-10-17 (VS2-11)**. Decisions: ADR-006 (own
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
  built-in program). HTP attributes default to 0 (ENG-07 unchanged). The `Colour` attribute
  defaults to open white (1, 1, 1), so a fixture brought up on its dimmer alone shows white.
  A released LTP attribute (Colour included) holds its last value (ENG-07).
- **LIB-04** A `virtual_dimmer` mode outputs each colour channel as component × Dimmer, where Dimmer
  is a normal HTP attribute of that fixture without a channel of its own (Grand Master included).
  Emitters are scaled the same way, so Dimmer 0 or a Grand Master blackout darkens the whole
  fixture. Showtec LED Par 56 at Colour (1, 0.5, 0) and Dimmer 0.5 → R 128, G 64, B 0; U'King
  RGBL at Lime 1.0, Dimmer 0.5 → Lime 128.

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
  per-cell control exists. In the engine: a fixture attribute drives the mode's attribute of that
  name and the same-named attribute of every cell; Colour drives the mode's and every cell's
  mixing channels. Per-cell selection (`201.2`, Unfold) is a programmer task with its own
  PRG criteria; the model does not change for it.
- **LIB-15** Wheel-type channels keep their slots (and, by LIB-18, every channel with two or more ranges): for each QLC+ capability range its name, and for
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
- **LIB-18** Import conventions (VS2-09, approved by owner 2026-10-17):
  - *Names.* A preset gives the attribute name of LIB-05; a channel without one keeps its QLC+
    name. When two attributes of one mode (or cell) would share a name, each takes its channel
    name; if that still clashes, ` 2`, ` 3`, … is appended.
  - *Slots.* Every channel with two or more capability ranges keeps them as slots (Mode,
    Program, Strobe ranges, not only wheels), so the operator sees "RGB Control" on the Showtec
    Mode channel. A slot gets a colour only from a single colour (`Res1="#rrggbb"`, or the older
    `Color` attribute); two-colour ranges (`ColorDoubleMacro`) get none.
  - *Colours.* QLC+ hex colours are sRGB: decoded to linear and taken as nominal primaries, as
    ADR-005 decision 3 does for fixtures (`#ff0000` → (1, 0, 0), `#808080` → 0.216 grey).
    Calibration (IDEA-03) replaces this, not the stored data.
  - *Defaults.* A channel's QLC+ `Default` value becomes the attribute default (n / 255);
    otherwise 0 (LIB-03).
  - *Heads.* A head's colour channels and emitters form a cell; any other channel of a head
    (its own dimmer, strobe) is a cell attribute too. Colour channels outside heads form the
    mode's colour system.
  - *Colour systems.* Red, Green and Blue together form an RGB system; Cyan, Magenta and Yellow
    form a CMY system only without RGB (LIB-17); an incomplete set becomes emitters named by
    colour.
  - *Physical.* A range of 0 (QLC+ "unknown") is not stored; a fixed lens stores min = max.
  - *Footprint.* A mode's footprint is its number of channels; channel numbers must be unique
    and below it (LIB-10).
  - *Tolerance* (VS2-11, from importing the whole QLC+ library): channel names are compared and
    stored without surrounding spaces; a channel listed in more than one head (a master dimmer
    in every head) belongs to the mode, not to a cell.
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
  Detail (VS2-11, approved by owner 2026-10-17): a search matches when every word of the query occurs,
  case-insensitively, in "manufacturer model"; results are sorted by manufacturer then model, a
  user profile is listed as `"source":"user"` and hides the QLC+ entry with the same
  manufacturer/model (LIB-12). Replies go to the asking client only:
  `{"type":"library.import_report","manufacturer","model","report":{"channels":[{"mode","channel","source_name","became"}],"warnings":[…]}}`
  (a QLC+ entry; a user profile has no import report → error);
  `library.profile {"manufacturer","model"}` → `{"type":"library.profile","profile":{…}}` (the
  converted profile, for editing (LIB-12) and for the patch (VS2-12));
  `library.save_profile {"profile":{…}}` (LIB-12) and `library.save_channel_list {"manufacturer",
  "model","mode","channels":["Red","Green","Blue",{"Emitter":"Royal Blue"},{"Other":{"name":"Mode","group":"S","default":0.0}}]}`
  → validated (LIB-02), written atomically to the user profiles folder as
  `<manufacturer> - <model>.json`, answered `{"type":"library.saved","manufacturer","model"}`.
  The library is indexed in the background at daemon start; until then a search is answered
  with an error ("library is loading"). Output never waits for the library.

## Out of scope (this spec)
The GDTF importer (its own spec section and task after QLC+, phase 1, ADR-006) and the OFL
importer (later: QLC+ already covers most of OFL's budget fixtures); Lightkey profile import
(checked once a file is available); per-cell selection (PRG task, LIB-07); picking a colour-wheel
slot from a target colour (VS2-06 follow-up, LIB-15 provides the data); showing degrees on the
wheels (UX); a graphical profile editor beyond LIB-12's fields.

## Open questions for the owner
1. ~~Shehds 19x15 Focus or Zoom?~~ Closed 2026-10-18 (VS2-13, owner): Lightkey calls it Zoom
   (10–60°); the church user profile renames it (PAT-16).
2. ~~Which QLC+ mode per church fixture?~~ Closed 2026-10-18: the Lightkey export gives them
   (`patch.md` PAT-16 table; Shehds 24 Channel).
