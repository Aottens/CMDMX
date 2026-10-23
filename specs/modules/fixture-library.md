# Module spec: fixture-library (LIB)

Owner: `cmdmx-core::library` (model, parsing, conversion; pure) and `cmdmx-daemon` (reading files,
the bundled QLC+ library). Status: **LIB-01..16 approved by owner 2026-10-17 (P1-01); LIB-17 approved by owner 2026-10-17 (generic emitters); LIB-18 approved by owner 2026-10-17 (import conventions); LIB-14 detail and LIB-18 tolerance approved by owner 2026-10-17 (VS2-11); LIB-19..27 approved by owner 2026-10-23 (P1-14)**. Decisions: ADR-006 (own
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

## Out of scope (this spec)
The GDTF importer (its own spec section and task after QLC+, phase 1, ADR-006) and the OFL
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
