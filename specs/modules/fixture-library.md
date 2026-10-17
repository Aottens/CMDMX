# Module spec: fixture-library (LIB)

Owner: `cmdmx-core::library` (model, parsing, conversion; pure) and `cmdmx-daemon` (reading files,
the bundled QLC+ library). Status: **draft for owner approval (P1-01)**. Decisions: ADR-006 (own
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
  and `virtual_dimmer` (true when the mode mixes colour but has no dimmer channel).
- **Attribute:** `name` (Dimmer, Pan, Tilt, Strobe, Zoom, Focus, White, Lime, …), `group`
  (I P C G B E S FX), `kind` (HTP/LTP), `channels` (1-based offsets; more than one when several
  heads share it, phase 1), optional `fine` offsets, `default` (0.0–1.0, default 0), optional
  `locate` and `highlight` values.
- **Colour mixing system (ADR-005):** `RGB` or `CMY`, with the channel offsets of each component
  (several per component when several heads mix). It is the fixture's one `Colour` attribute
  (group C, LTP). Extra emitters (White, Amber, Lime, UV, Indigo) are ordinary C attributes.
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

### QLC+ import (`.qxf`)
- **LIB-05** Preset mapping: `IntensityMasterDimmer`/`IntensityDimmer` → Dimmer (I, HTP);
  `IntensityRed/Green/Blue` → RGB system; `IntensityCyan/Magenta/Yellow` → CMY system;
  `IntensityWhite/Amber/UV/Indigo/Lime` → emitter attributes White/Amber/UV/Indigo/Lime (C, LTP);
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
- **LIB-07** Heads: channels with the same role in several heads of one mode become one attribute
  (or one colour component) driving all of them. Shehds 19x15 "24 Channel" (three RGBW heads) →
  one Colour with three channels per component and one White with three channels; whole-fixture
  control only in phase 1.
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
  channel types: Dimmer, Dimmer fine, Red, Green, Blue, White, Amber, Lime, UV, Cyan, Magenta,
  Yellow, Pan, Pan fine, Tilt, Tilt fine, Strobe, Zoom, Focus, Colour Macro, or Other (with a
  name, group and default). Same mapping rules as LIB-05/08: U'King RGBL as `[Red, Green, Blue,
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

## Out of scope (phase 1)
GDTF and OFL importers (ADR-006 order), Lightkey profile import (checked once a file is available),
per-cell control of multi-head fixtures (heads are merged, LIB-07), colour-wheel slot selection by
colour, physical units (degrees, beam angle) on the wheels, a graphical profile editor beyond
LIB-12's fields.

## Open questions for the owner
1. Shehds 19x15: QLC+ calls the zoom channel **Focus** (`BeamFocusNearFar`). If Lightkey calls it
   Zoom and it zooms, we rename it in the user profile after the church visit (LIB-12).
2. Church check tomorrow: which QLC+ mode matches each fixture's DIP/menu setting (e.g. Shehds 16 or
   24 channels)? The Lightkey export will tell.
