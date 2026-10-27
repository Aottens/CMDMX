# Module spec: fixture-library (LIB)

Owner: `cmdmx-core::library` (model, parsing, conversion; pure) and `cmdmx-daemon` (reading files,
the bundled QLC+ library). Status: **LIB-01..16 approved by owner 2026-10-17 (P1-01); LIB-17 approved by owner 2026-10-17 (generic emitters); LIB-18 approved by owner 2026-10-17 (import conventions); LIB-14 detail and LIB-18 tolerance approved by owner 2026-10-17 (VS2-11); LIB-19..27 approved by owner 2026-10-23 (P1-14); LIB-28..35 approved by owner 2026-10-27 (P1-12); LIB-35 amendment approved by owner 2026-10-27 (VS2-43)**. Decisions: ADR-006 (own
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
  PRG criteria (PRG-46..55); the model does not change for it.
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

### Browsing the library (P1-14, IDEA-08; sketch `specs/ux/library-and-profile-editor.html`)
- **LIB-19** The index knows each mode's channel count without converting a profile: while
  indexing, a QLC+ mode counts its `<Channel>` entries and a user profile takes the mode's
  footprint. An index entry's modes are `{name, channels}`. Showtec LED Par 56 (QLC+) → "6 Channel"
  6, a user profile with a 3-channel mode → "3 Channel" 3; Shehds 19x15 → "16 Channel" 16,
  "24 Channel" 24. `library.search` (LIB-14) is
  unchanged.
- **LIB-20** Manufacturers: the index lists every manufacturer once, sorted case-insensitively,
  with its number of models (a user profile that hides a QLC+ one counts once, LIB-12). With a
  query, only manufacturers with at least one model matching it (the word rule of LIB-14), and
  the count is the number of matches. Wire: IPC-35.
- **LIB-21** Models: the models of one manufacturer, sorted case-insensitively, each with its
  modes (LIB-19) and source; with a query only the matches. With a query and no manufacturer,
  the matches across all manufacturers, sorted by manufacturer then model, at most 500 with a
  `truncated` flag. Without both it is an error. Wire: IPC-36.
- **LIB-22** The Add panel (app; replaces PAT-10's Add panel) is a full-width strip above the
  patch table, the Selected panel stays at the right. One search field over three columns:
  - *Manufacturer:* LIB-20's list for the current query, with counts. A selected manufacturer
    that no longer matches is deselected. On top, when it has matches, an entry **In this show**:
    the profiles of the patched fixtures (from the snapshot), whose modes come from the show's
    own copy (IPC-37), because a patch of a type the show already carries uses that copy.
  - *Model:* the selected manufacturer's models matching the query (LIB-21); with no
    manufacturer selected, every match, each prefixed with its manufacturer. A user profile
    carries a `USER` tag; each row shows its number of modes.
  - *Mode:* the picked model's modes with their channel counts; the first is preselected.
  Beside the columns: the pick ("Showtec Phantom 25 LED Spot · 11 Channel · 11 channels · user
  profile"), Count, Universe, Address, Number and Add as PAT-10, plus **Edit profile…** (LIB-23,
  disabled without a pick) and **New from channel list…** (LIB-26). Arrow keys move within and
  between the columns; Return adds when Add is enabled. Replies to an older query are dropped.
  Verified by `tests/manual/app-patch.md` (amended).

### Profile editor (P1-14, IDEA-13; ONYX's fixture editor as reference)
- **LIB-23** A full-screen profile editor ("Patch › Profile editor") opens from the Add panel's
  Edit profile… (on the picked model: the show's copy when the show carries it, IPC-37, else
  `library.profile`) or from the Selected panel's Edit profile… (the show's copy of that
  fixture's profile). It edits one mode at a time, chosen in a mode picker, opening on the
  picked or patched mode. Manufacturer and model are read-only. Left, the channels of the mode:
  channel, attribute name, group tag, kind, default (%), number of slots. Right, the selected
  channel: name, group (I P C G B E S FX), kind HTP/LTP, and Default, Locate and Highlight as a
  level in % with its DMX value. A fine channel reads "fine of Pan" and has no fields of its own;
  a colour-system component or emitter shows its role and keeps it (only its name and, for an
  emitter, its defaults change). + adds a plain channel at the end (name "Channel n", group S,
  LTP, default 0), − removes the selected one, ▲▼ move it; channel offsets follow the order.
  Cancel leaves without saving; unsaved changes ask once before leaving.
- **LIB-24** Slots: the selected channel's slots in a table (From, To, Name, and on group C
  channels a colour, entered as sRGB and stored as a target colour like LIB-18) over a bar from
  0 to 255 that shows each range; clicking a range in the bar selects its row. Editing:
  + Slot (on a channel without slots one slot 0–255, otherwise a Split), **Split** (the selected
  range in two halves; the lower half keeps name and colour), **Merge with next** (the selected
  range grows over the next; its name and colour stay), **Remove** (the neighbour below takes
  the freed values; when the first range is removed, the next one does). Gaps typed into From/To
  are allowed. Slots are stored sorted by From. Slots belong to the coarse channel; a fine channel
  has none.
- **LIB-25** Save sends the edited profile to the engine (IPC-38), all or nothing:
  - validated as LIB-02, plus: a slot with From above To, or two slots of one channel that
    overlap, is refused (only on this path; loading existing profiles is unchanged);
  - when the show carries a profile with this key, every mode a patched fixture uses must still
    exist with the same footprint, and no attribute that stored content (palettes, cues, playbacks, the
    programmer) holds for fixtures of this type may disappear;
    otherwise it is refused with a warning naming the mode or the attribute and where it is used
    ("Gobo is used by cue 1.3 of list 2");
  - on success the show's copy is replaced as one undo step "Edit profile Phantom 25 LED Spot",
    output follows on the next tick (an unproposed attribute outputs its new default, LIB-03),
    and then the user profile is written atomically (LIB-12), so it hides the QLC+ entry from
    then on. A failed file write after the show was updated is a warning ("Saved in the show,
    not in the library: …"). Undo restores the show's copy only, not the file.
  Scenario: the church Phantom 25 with Pan and Tilt defaults 0.5 saved → fixtures 201–204 with
  no Pan/Tilt in any source output Pan and Tilt 0.5 (16-bit with their fine channels) on the next
  tick.
- **LIB-26** New from channel list… opens the same editor on an empty profile with manufacturer,
  model and mode name editable; + adds a channel of a LIB-11 type (picked from the list, with a
  name for Emitter and Other). Save sends `library.save_channel_list` (LIB-11, LIB-14); the new
  profile is then picked in the Add panel.
- **LIB-27** Test: a **Test** toggle in the editor's header (red while on), enabled when the
  current selection holds fixtures of this profile ("Test on 201–204"). While on, clicking a slot
  sets the channel's attribute to the slot's middle DMX value ((From + To) / 2 / 255), and moving
  Default, Locate or Highlight sets the attribute to that level, both with `programmer.set`
  (IPC-39) for the selected fixtures of this profile: ordinary programmer values, which Clear
  removes. Leaving the editor does not clear them.

### GDTF import (P1-12; ADR-006 import order 3; approved by owner 2026-10-27)
GDTF (DIN SPEC 15800) is the manufacturers' own format: a `.gdtf` file is a zip archive with a
`description.xml`. It brings brand-name fixtures the QLC+ library lacks, and it can describe the
real colour of each emitter, which colour calibration (IDEA-03, phase 2) needs. Reference: the
GDTF 1.2 specification (`github.com/mvrdevelopment/spec`, Annex A for attribute names).

- **LIB-28** Pure import. `cmdmx-core::library::gdtf::import(bytes, source)` takes the bytes of
  a `.gdtf` archive (no file system; the zip is read from memory) and returns a profile and an
  import report, like the QLC+ importer. Parsing uses the `gdtf` crate (MIT, 0.3). The first
  fixture type of the file becomes the profile: `manufacturer` from its `Manufacturer`, `model`
  from its `LongName` (its `Name` when that is empty), `source` `gdtf:<file name>`. Every DMX mode
  is imported with its GDTF name; its footprint is the highest channel offset it uses.
- **LIB-29** Attribute mapping. The logical channel's attribute decides, `(n)` meaning any
  number (Annex A):
  - `Dimmer` → Dimmer (I, HTP).
  - `ColorAdd_R/G/B` and `ColorRGB_Red/Green/Blue` → the RGB system; `ColorSub_C/M/Y` and
    `ColorRGB_Cyan/Magenta/Yellow` → the CMY system, but emitters Cyan/Magenta/Yellow when the
    mode (cell) also has red, green and blue (LIB-17). `ColorAdd_C/M/Y` are always emitters.
  - Emitters (C, LTP, `emitter`): `ColorAdd_W` White, `ColorAdd_WW` Warm White, `ColorAdd_CW`
    Cold White, `ColorAdd_RY` Amber, `ColorAdd_GY` Lime, `ColorAdd_UV` UV, `ColorAdd_BM` Indigo,
    `ColorAdd_GC` Blue-Green, `ColorAdd_BC` Light Blue, `ColorAdd_RM` Pink.
  - `CTO`, `CTC`, `CTB` → CTO / CTC / CTB (C, LTP, not emitters); `Color(n)` → Colour Wheel (C);
    `ColorMacro(n)` → Colour Macro (C).
  - `Pan`, `Tilt` → Pan / Tilt (P, LTP); `Shutter(n)` and `Shutter(n)Strobe` → Strobe (I, LTP);
    `Iris` → Iris, `Zoom` → Zoom, `Focus(n)` → Focus, `Frost(n)` → Frost (B);
    `Gobo(n)` → Gobo, `Gobo(n)Pos` → Gobo Index, `Gobo(n)PosRotate` → Gobo Rotation (G);
    `Prism(n)` → Prism, `Prism(n)PosRotate` → Prism Rotation (E);
    `PanTiltSpeed`, `PositionMSpeed` → Pan/Tilt Speed (S).
    For `(n)` above 1 the number is appended ("Gobo 2", "Colour Wheel 2", "Strobe 2").
  - Any other attribute is kept as a plain channel (LTP, named by its `Pretty` name, else its
    `Name`), with its group from its feature group: Dimmer → I, Position → P, Color → C,
    Gobo → G, Beam, Focus and Shapers → B, anything else (Control, Video, …) → S. The report says
    "kept as a plain channel". LIB-18's naming rule (channel name, then ` 2`, ` 3`) settles clashes.
- **LIB-30** Channels and values.
  - *Fine bytes.* A DMX channel's `Offset` list gives coarse first: `1,2` → channel 1 with fine
    channel 2. A third byte (24-bit) is not used; the report warns, and it outputs 0.
  - *Virtual channels* (`Offset` "None") get no attribute and are listed in the report. A mode that
    mixes colour without a Dimmer channel gets `virtual_dimmer` by LIB-08, whatever GDTF
    relations say.
  - *Default and Highlight.* The default is the `Default` of the channel's initial function
    (GDTF 1.0/1.1: of the DMX channel); `Highlight` becomes `highlight`. A GDTF value `v/n`
    becomes v / (2^(8n) − 1): `128/1` → 0.502, `32768/2` → 0.500. GDTF has no locate value.
  - *Slots* (LIB-15, LIB-18). A channel's named channel sets become its slots; without named
    sets, a channel with two or more channel functions gets one slot per function, named after it.
    From and To are on the coarse byte; a range ends where the next begins. A slot on a colour
    wheel takes the wheel slot's `Color` (CIE xyY), converted to a target colour: xy to linear
    sRGB (D65), negative components clipped to 0, scaled so the largest is 1 (`0.3127,0.3290`
    → (1, 1, 1)).
  - *Physical* (LIB-16). Pan and Tilt store the span of their function's `PhysicalFrom`..`PhysicalTo`
    (−270..270 → 540°); Zoom stores min and max. GDTF's own default 0..1 counts as unknown and is
    not stored.
- **LIB-31** Cells from geometry. Every DMX channel names a geometry. When a mode's colour
  channels sit on two or more geometries (or geometry references), each of those is a cell, in
  order of its first channel, and every channel of that geometry belongs to it (LIB-18 heads);
  channels on any other geometry (a master dimmer on the body) belong to the mode. Channels
  instanced through a geometry reference get its DMX offset (`Break` offset of the reference,
  `DMXBreak="Overwrite"`). LIB-07 holds unchanged: the fixture's Colour drives every cell.
- **LIB-32** Emitter colours (for IDEA-03). When a colour component or emitter's channel
  function links an `Emitter` of the physical description that has a CIE `Color`, the profile
  stores its chromaticity as `cie: [x, y]` on that component or emitter attribute. An emitter
  with only a dominant wavelength, or no link, stores nothing (the report says so). Nothing reads
  `cie` yet; it round-trips (LIB-01), and LIB-02 rejects `cie` on an attribute that is not an
  emitter or with x, y outside 0..1 or x + y > 1.
- **LIB-33** Failure. Not a zip, no `description.xml`, malformed XML, no fixture type, or no mode
  left to import → the import fails with an error naming the file and the problem; nothing is
  added (LIB-10). A mode whose channels use a DMX break other than 1 is skipped with a warning
  (one fixture = one footprint, PAT). Unknown attributes are plain channels with a warning, not a
  failure. The daemon runs the import off the tick inside `catch_unwind`: a panic in the parser
  becomes an import error and output never waits.
- **LIB-34** Library. `library.import_gdtf {"path":…}` (IPC-45) makes the daemon read the file
  (at most 100 MB), import it, and save the profile as a user profile (LIB-12: atomic, in the
  user profiles folder, listed as `"source":"user"`, hides a QLC+ entry of the same
  manufacturer/model). An existing user profile with that manufacturer/model is replaced. The
  show's own copies of profiles do not change (SHW-01); profile edits stay with LIB-23..25.
- **LIB-35** App. The Add panel gets **Import GDTF…** beside New from channel list… (LIB-22). It
  opens a file panel for `.gdtf` files and sends IPC-45. On success a sheet shows the import
  report (mode, channel, GDTF name, what it became; then the warnings) and the new profile is
  picked in the Add panel; on failure the error is shown and nothing changes. Every report cell
  can be read in full: the Became text wraps onto more lines instead of being cut off with "…",
  and hovering a cell shows its full text. Escape closes the sheet like Done (Return still does
  too) (amended 2026-10-27, VS2-43). Verified by `tests/manual/app-patch.md` (amended).

Scenario tests (VS2-41): GDTF files written for this project, as `description.xml` sources in
`tests/fixtures/gdtf/<name>/` that the test zips in memory, each with its expected profile: an
RGBW par (a 4-channel mode with virtual dimmer and emitter colours, a 6-channel mode with Dimmer
and a Strobe channel with three functions), a moving-head spot (16-bit Pan −270..270 and Tilt
−135..135 with defaults `32768/2`, colour wheel with four coloured slots, gobo wheel, Zoom 10–40°,
Highlight on the Dimmer), a four-pixel bar (geometry references, master dimmer → four cells), and
one broken file per LIB-33 case. The `gdtf` crate's own `Generic@RGBW8@test.gdtf` (MIT) is
imported too, as a file not written by us. Found in VS2-41: the `gdtf` crate 0.3.0 rejects a
file with an emitter that has no `DominantWaveLength` or a wheel slot without `Color`, and does
not read a GDTF 1.0 `Default` on the DMX channel (the channel function's is used); VS2-42's
manual run checks a real GDTF-share file.

## Out of scope (this spec)
Downloading from GDTF-share inside the app (it needs an account); GDTF's 3D models, gobo images,
DMX profiles (dimmer curves), mode masters and relations; modes over more than one DMX break
(LIB-33). The OFL
importer (later: QLC+ already covers most of OFL's budget fixtures); Lightkey profile import
(checked once a file is available); per-cell selection (PRG task, LIB-07); picking a colour-wheel
slot from a target colour (VS2-06 follow-up, LIB-15 provides the data); showing degrees on the
wheels (UX). In the profile editor (LIB-23..27): gobo pictures on slots (with the icon library,
IDEA-14), adding, copying or removing modes, renaming manufacturer or model, editing cells and the
colour system of multi-head modes, and ONYX's separate wheels linked to channels (slots stay per
channel, owner 2026-10-23).

## Open questions for the owner
1. ~~Shehds 19x15 Focus or Zoom?~~ Closed 2026-10-18 (VS2-13, owner): Lightkey calls it Zoom
   (10–60°); the church user profile renames it (PAT-16).
2. ~~Which QLC+ mode per church fixture?~~ Closed 2026-10-18: the Lightkey export gives them
   (`patch.md` PAT-16 table; Shehds 24 Channel).
3. ~~*(P1-14)* An edit that removes or renames an attribute that palettes, cues or the programmer
   hold for fixtures of this type: refuse the Save and name where it is used (LIB-25), or carry
   the values across a rename? **Recommendation:** refuse for now; a rename that carries values
   is a later task once someone needs it (the church renames were done at import, PAT-16).~~ Closed 2026-10-23 (owner): refuse.
4. ~~*(P1-14)* Test needs an absolute programmer value, which IPC does not have yet (only wheel
   steps and the command line). **Recommendation:** add `programmer.set` (IPC-39), the same as a
   wheel move to an absolute level, so Test is ordinary programmer content.~~ Closed 2026-10-23 (owner): `programmer.set`.
5. ~~*(P1-14)* One mode at a time in the editor, without adding or copying modes. **Recommendation:**
   yes; a profile with a missing mode is made from a channel list (LIB-26) for now.~~ Closed 2026-10-23 (owner): one mode at a time.
6. ~~*(P1-12)* Where do GDTF files come in? **Recommendation:** Import GDTF… picks a file, which is
   converted once and saved as a user profile (LIB-34), as ADR-006 says. The alternative, a GDTF
   folder indexed like the QLC+ set and converted on pick, keeps the original file but adds a
   third library source.~~ Closed 2026-10-27 (owner): as recommended.
7. ~~*(P1-12)* Importing a model that is already a user profile (perhaps one you edited)?
   **Recommendation:** replace it; the reply says so. Refusing would need a delete step first.~~ Closed 2026-10-27 (owner): as recommended.
8. ~~*(P1-12)* Test files. **Recommendation:** files we write ourselves (no licence question) plus
   the `gdtf` crate's MIT test file. A real manufacturer file (say a Robe or Showtec one you
   download from GDTF-share) is checked once in a manual run, and only committed if its licence
   allows.~~ Closed 2026-10-27 (owner): as recommended.
9. ~~*(P1-12)* Store emitter colours (LIB-32) now, although only calibration (IDEA-03, phase 2)
   uses them? **Recommendation:** yes: it is the reason GDTF is in phase 1, and an imported profile
   that lacks them would have to be imported again.~~ Closed 2026-10-27 (owner): as recommended.
10. ~~*(P1-12)* `cmdmx-core` gets bytes, not text, for GDTF (the zip is read in memory, still no file
    system), and the `gdtf` crate brings `zip`, `uuid` and a second `quick-xml` version into core.
    **Recommendation:** accept; writing our own GDTF parser is the larger risk. Recorded in
    `DECISIONS.md` once you agree.~~ Closed 2026-10-27 (owner): as recommended.
