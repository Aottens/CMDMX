# Module spec: patch (PAT)

Owner: `cmdmx-core::patch` (pure) and `cmdmx-daemon` (IPC), patch screen in `subprojects/app`.
Status: **PAT-01..14 approved by owner 2026-10-17 (P1-11); PAT-09 wire details approved 2026-10-17 (VS2-12)**; PAT-15..18 (the church one-off, VS2-13) approved by owner 2026-10-18; PAT-09/10 amendment (modes dropdown, VS2-16) approved by owner 2026-10-18; PAT-19..25 (range edits, P1-15) approved by owner 2026-10-24. Builds on `fixture-library.md` (profile + mode).
Behavioural reference: Titan *Patching* (fixtures from the library, user numbers, auto address).

## Purpose
Put the rig into the show: pick a fixture type and mode from the library, say how many, where
they start in DMX and which numbers the operator will type, and the engine places them without
overlaps. The patch is what the programmer, palettes and cues refer to, so it must be impossible
to end up with two fixtures on the same channel by accident.

## Definitions
- **Fixture:** `id` (internal, assigned by the engine at patch time, never shown, never reused;
  every reference in the show — selection, programmer, groups, palettes, cues — uses it),
  `number` (the user number the operator types and sees; unique; changeable), `name` (optional
  legend), `profile` (`manufacturer/model`), `mode`, `universe` (1-based), `address` (1–512),
  `handle` (its button position in the Fixtures window: page and slot).
- **Footprint:** the mode's channel count; a fixture occupies `address … address + footprint − 1`.

## Acceptance criteria
- **PAT-01** `patch.add` places `count` fixtures of one profile and mode: numbers from `number`
  upwards, addresses from `address` upwards, each starting right after the previous fixture's
  footprint. 6 × Showtec LED Par 56 "6 Channel" from number 221, universe 1 address 37 → 221–226
  at 37, 43, 49, 55, 61, 67.
- **PAT-02** A patch that would overlap an existing fixture's channels, run past slot 512, or reuse
  an existing number is refused as a whole (no partial patch), with a warning naming the first
  clash ("226 would overlap 301 at 1.67"). Nothing changes.
- **PAT-03** Without an `address`, the patch takes the first free run of channels big enough for
  all `count` fixtures in the given universe; without a `number`, the first free run of numbers
  after the highest used. Refused if no such run exists.
- **PAT-04** `patch.move` gives one fixture a new universe/address under the same checks as PAT-02
  (its own old channels do not count as a clash). Programmer values, groups and recorded data
  follow the fixture because they refer to its id.
- **PAT-05** `patch.remove` removes fixtures. They leave the selection, the programmer and
  every group; the frame channels they used drop to 0 at the next tick. Removal is always allowed
  in phase 1; recorded data referring to a removed number is ignored with a warning on load
  (SHW-08 rule for dangling references).
- **PAT-06** `patch.renumber` changes a fixture's user number to an unused one; nothing else
  changes, because every reference uses the id. Renumbering 201–208 to 1201–1208 leaves groups,
  programmer values and recorded data untouched; `1201 Enter` now selects what `201` did.
- **PAT-12** User numbers are resolved to ids at the edges: the command line (PRG-08) and remote
  surfaces take user numbers; IPC messages and the show file carry ids, and the snapshot's
  `fixtures` list maps id → number for display. Internal ids are never reused after removal, so a
  stale reference can never point at a different fixture.
- **PAT-13** `patch.change_mode` gives a fixture another mode of its profile. Attributes are matched
  by name; programmer values and recorded data for attributes the new mode lacks are dropped with
  a warning; a bigger footprint is checked as in PAT-02 and refused on a clash.
- **PAT-14** Handles (Titan fixture buttons): a patch assigns each new fixture the next free slot in
  the Fixtures window in patch order (pages of `handles_per_page` slots, a setting, default 40); `patch.handle {id, page, slot}` moves a button to a free slot
  (or swaps with the occupant). The Fixtures window (IPC-15) shows fixtures by handle.
- **PAT-07** A fixture patched with a mode that has `virtual_dimmer` gets a Dimmer attribute
  without a channel (LIB-04); the patch list shows it as "virtual dimmer".
- **PAT-08** The patch is part of the show file (`patch` key, SHW-01) together with the profiles
  it uses (ADR-006 decision 5); loading a show re-runs the PAT-02 checks and reports clashes as
  warnings instead of refusing the show.

### IPC and app
- **PAT-09** IPC (extends `ipc.md`): the snapshot's `fixtures` entries become `{id, number, name,
  profile, mode, universe, address, footprint, virtual_dimmer, handle}`; a `fixtures` delta is
  sent after any patch change. Commands: `patch.add {profile, mode, count, universe, address?,
  number?}`, `patch.move {id, universe, address}`, `patch.remove {ids}`, `patch.renumber {id,
  number}`, `patch.name {id, name}`, `patch.change_mode {id, mode}`, `patch.handle {id, page,
  slot}`. Refusals are warnings (IPC-06).
  Wire details (VS2-12, within the approved criterion): `profile` is the engine's profile key; a
  fixture type from the library is patched under the key `"<manufacturer>/<model>"` (split at the
  first `/`), and the daemon fetches and converts it from the library in the client thread, never
  in the tick. A key already in the show keeps the copy the show has (ADR-006 decision 5), so
  fixtures patched earlier never change under the operator. `mode` is required, `address` and
  `number` are optional (PAT-03). In the snapshot `mode` is the mode's name (the first mode when
  none was given), `virtual_dimmer` is LIB-04's virtual dimmer, `handle` is `{page, slot}` or
  `null`, and the delta is `{"v":0,"type":"fixtures","fixtures":[…]}` (full list, at most one per
  tick, only when the list changed).
  Amended (VS2-16, owner 2026-10-18): each entry also carries `modes`, the names of every mode of
  the fixture's profile as the show holds it (ADR-006 decision 5), in profile order, e.g.
  `["16 Channel","24 Channel"]` for the Shehds; the client never asks the library for them.
- **PAT-10** A Patch screen (full screen from the workspace rail, UI v0.1): a table of the patch
  (number, name, manufacturer/model, mode, universe.address, footprint), sorted by number; an Add
  panel with library search (LIB-14), mode, count, universe, start address and start number
  (both optional, PAT-03); Move, Renumber, Name, Change Mode and Remove on a selected row, Remove
  and Change Mode with a two-step confirm. A refused action shows the warning in the panel. Verified by
  `tests/manual/app-patch.md`.
  Amended (VS2-16, owner 2026-10-18): Change Mode is a dropdown of the row's `modes` (PAT-09) with
  the current mode preselected, not a text field; Change stays disabled while the current mode is
  picked, so a one-mode fixture cannot be changed.
  Amended (P1-14, approved by owner 2026-10-23): the Add panel is LIB-22's (search over manufacturer,
  model and mode columns, a full-width strip above the table); the Selected panel gains Edit
  profile… (LIB-23).
  Amended (P1-15, approved by owner 2026-10-24): the table takes a multi-row selection (PAT-25).
- **PAT-25** Multi-select on the Patch screen (P1-15, approved by owner 2026-10-24): click selects one row,
  Shift-click a run of rows, Cmd-click adds or drops a row, Cmd-A selects all (macOS table
  conventions). With more than one row selected the Selected panel is titled "Selected · 6
  fixtures" and its actions send the range forms (IPC-40) for the whole selection: Move (start
  universe and address, PAT-20), Renumber (start number, PAT-21), Name (prefilled with the first
  row's name, hint "Par 1 numbers them Par 1, Par 2, …", PAT-23), Change Mode (a dropdown of the
  modes every selected row has, in the first row's order, the shared current mode preselected or
  none when they differ; Change disabled while nothing new is picked, PAT-22) and Remove ("Remove 6
  fixtures", two-step confirm). Edit profile… is enabled only when every selected row has the
  same profile. The selection stays on the same fixtures (ids) after an edit, so the table shows
  where they went; a refused range edit shows its warning in the panel and changes nothing.
  Verified by `tests/manual/app-patch.md` (amended).

### Range edits (P1-15, IDEA-09; approved by owner 2026-10-24)
Titan reference (*Changing the Patch*, *Copying, moving and deleting fixtures*): fixtures selected
together are repatched from one start address, get user numbers counting up from the one typed,
can share one legend, and are deleted in one go; a clash is refused or the clashing fixtures are
parked. Titan exchanges fixture types one fixture at a time and has no multi-fixture mode change.
ColourmotionDMX has no parking (a removed fixture is gone, PAT-05), so every clash refuses.

- **Range:** the fixtures one range command acts on, given as a list of ids (`ids`). A range is
  worked in **number order** (ascending user number), whatever order the list or the clicks came
  in. A range of one fixture behaves exactly as the single-fixture command (PAT-04, 06, 13).

- **PAT-19** A range command is checked as a whole before anything changes: if one fixture of the
  range would be refused, the command is refused with one warning naming the first clash in
  number order ("224 would overlap 301 at 1.67"), and nothing changes (PAT-02). An empty `ids`,
  an id not in the patch or an id listed twice refuses the command. An accepted range command is
  one undo step (UNDO-01, UNDO-03), labelled with the count when it holds more than one fixture
  ("Patch move 6 fixtures").
- **PAT-20** Move a range back to back: `patch.move` with `ids` places the range in number order
  from the given universe and address, each fixture right after the previous one's footprint as
  in PAT-01, whatever gaps or order the fixtures had before. The range's own old channels do not
  count as a clash (PAT-04). 221–226 (6 Channel) at 1.37–1.67 moved to 2.101 → 2.101, 107, 113,
  119, 125, 131. A range that would run past slot 512 or overlap a fixture outside the range is
  refused (PAT-19).
- **PAT-21** Renumber a range, order kept: `patch.renumber` with `ids` gives the range consecutive
  user numbers from the given number, in number order. 1221–1226 from 1301 → 1301–1306 (1221 →
  1301 … 1226 → 1306); 1221, 1223, 1225 from 1301 → 1301, 1302, 1303 (gaps close). The range's own
  old numbers do not count as used, so 201–206 from 203 is allowed (→ 203–208) unless 207 or 208
  belongs to a fixture outside the range, which refuses the command naming it. Nothing but the
  numbers changes (PAT-06).
- **PAT-22** Change the mode of a range: `patch.change_mode` with `ids` gives every fixture of the
  range the named mode; every fixture's profile must have a mode of that name, otherwise the
  command is refused naming the first fixture without it. Fixtures keep their addresses; each
  new footprint is checked against fixtures outside the range and against the range's own new
  footprints, so six 6-channel pars back to back cannot grow to 7 channels in place (refused,
  "221 would overlap 222 at 1.43"; the operator moves the range first, or after a smaller mode,
  PAT-20). Attributes are matched by name per fixture as in PAT-13; dropped programmer values and
  recorded data give one warning for the range ("Dropped Strobe on 6 fixtures").
- **PAT-23** Numbered names: `patch.name` with `ids` names the range in number order. A name that
  ends in a number counts up from it, keeping the number of digits: "Par 1" → Par 1 … Par 6;
  "P40" → P40 … P45; "Par 01" → Par 01 … Par 09, Par 10, …. A name
  without a trailing number is given to every fixture as it is (Titan Set Legend on a group); an
  empty name clears every name. Names need not be unique, so a name never clashes.
- **PAT-24** Remove a range: `patch.remove {ids}` (PAT-05) is a range command under PAT-19: an
  unknown or repeated id refuses the whole removal; an accepted one is one undo step that brings
  every removed fixture back with its groups, palettes and cue data (UNDO-03).

### The church
- **PAT-11** The church rig from the owner's Lightkey export is patched once through these commands
  (or a one-off script that calls them) into the church show; the result is listed in
  `tests/hardware/church.md` (VS4-01) so the patch can be checked against the real fixtures.

The one-off is `cargo run -p cmdmx-daemon --example church -- <export.tsv>` (VS2-13, owner
decisions 2026-10-18). It reads the owner's Lightkey patch export from disk; the export and the
Lightkey show stay out of git. Options: `--show-dir` (default `~/Documents/ColourmotionDMX/Shows`),
`--profiles-dir` (default `~/Library/Application Support/ColourmotionDMX/profiles`) and
`--qlcplus-dir` (default the bundled `vendor/qlcplus/fixtures`).

- **PAT-15** The export is Lightkey's tab-separated patch list with the header `Universe, Address,
  Count, Manufacturer, Model, Manufacturer & model, Mode, Channels, Short name, Comments`; each
  row is one fixture. A different header, a row with `Count` other than 1, or a universe,
  address or channel count that is not a number refuses the run with the line number; nothing
  is written.
- **PAT-16** Each Lightkey fixture type (manufacturer + model) maps to one profile and mode
  (table below). Types QLC+ has with the same channel order come from QLC+; the others are
  channel lists (LIB-11) written from the channel layout in the Lightkey show. The Shehds is the
  QLC+ profile with its `Focus` attribute renamed `Zoom` (Lightkey: Zoom, 10–60°; closes
  fixture-library open question 1), saved as a user profile (LIB-12). On the Phantom 25 and Fuze
  75S, DMX 0 closes the shutter (0–3 and 0–31), so their Shutter defaults to open (1.0, LIB-03).
  A type not in the table, or a mode whose footprint differs from the export's `Channels`,
  refuses the run naming the type.
- **PAT-17** Numbers come in blocks of 100 per type (Titan style): the types in order of their
  first address get 101.., 201.., 301.., …, and within a block the fixtures are numbered in
  address order; a type with more than 99 fixtures refuses the run. The name is the Lightkey
  short name; a short name an earlier fixture (by address) already has becomes the next one of
  its letter prefix (highest number with that prefix + 1): the RGBL par at 373 (`P40`, also the
  BTF50Z at 234) becomes `P45`, the Fuze 75S at 464 and 477 (`MH6`, `MH7`, also Shehds) become
  `MH9` and `MH10`. Fixtures are patched in number order, so their handles (PAT-14) group by type.
- **PAT-18** Every fixture goes through `patch_add` with count 1, its address and its number, so
  each one passes the PAT-02 checks; a refusal stops the run before anything is written. The run
  writes the show `Kerk` (SHW-01, carrying the profiles it uses, PAT-08) into the shows folder
  only when no show of that name exists (never overwrites), writes each channel-list profile
  and the Shehds profile into the user profiles folder unless a file of that name is already
  there (kept and reported), and prints the patch as a Markdown table (number, name, profile,
  mode, universe.address, footprint) for `tests/hardware/church.md`.

| Lightkey type | Profile key (mode) | Source | Fixtures | Numbers |
| --- | --- | --- | --- | --- |
| Showtec LED PAR 56 Short DMX | `Showtec/LED Par 56` (6 Channel) | QLC+ | 20 | 101–120 |
| Showtec Phantom 25 LED Spot | `Showtec/Phantom 25 LED Spot` (11 Channel): Pan, Tilt, Pan fine, Tilt fine, Pan/Tilt Speed (S), Colour Wheel (C), Shutter (I, default 1.0), Dimmer, Gobo (G), Control (S), Programs (S) | channel list | 4 | 201–204 |
| Prolights Lumipar6UQ | `Pro-Lights/Lumipar6UQ` (4 Channel): Red, Green, Blue, White | channel list | 8 | 301–308 |
| Generic PAR Spot | `Generic/PAR Spot` (1 Channel): Dimmer | channel list | 6 | 401–406 |
| Showtec Compact Par 7 Tri | `Showtec/Compact Par 7 Tri` (3 Channel) | QLC+ | 2 | 501–502 |
| Generic PAR Wash (RGBW) | `Generic/PAR Wash (RGBW)` (4 Channel): Red, Green, Blue, White | channel list | 2 | 601–602 |
| Stairville Hz-200 DMX | `Stairville/Hz-200 DMX` (Default) | QLC+ | 1 | 701 |
| beamZ BTF50Z Mini Fresnel | `beamZ/BTF50Z Mini Fresnel` (3 Channel): Dimmer, Warm White, Cold White | channel list | 2 | 801–802 |
| Shehds LED Beam+Wash 19x15W RGBW Zoom Light | `Shehds/LED Beam+Wash 19x15W RGBW Zoom Lighting` (24 Channel) | QLC+, Focus → Zoom, user profile | 4 | 901–904 |
| Generic PAR Spot (RGBL) | `Generic/PAR Spot (RGBL)` (4 Channel): Red, Green, Blue, Lime | channel list | 4 | 1001–1004 |
| beamZ Fuze 75S Spot | `beamZ/Fuze 75S Spot` (16 Bit): Pan, Pan fine, Tilt, Tilt fine, Colour Wheel (C), Gobo (G), Shutter (I, default 1.0), Dimmer, Focus, Prism (E), Gobo/Prism Rotation (G), Pan/Tilt Speed (S), Programs (S) | channel list | 2 | 1101–1102 |
| Generic PAR Spot (RGB) | `Generic/PAR Spot (RGB)` (3 Channel): Red, Green, Blue | channel list | 1 | 1201 |

Colour wheel, gobo and prism channels of the Phantom 25 and Fuze 75S are plain channels without
slot names for now (the Lightkey show has them; adding them is a LIB-12 edit later).

## Out of scope (this spec)
Per-cell selection and Unfold (cells exist in the model, LIB-07; selecting them is a PRG task),
a fixture spanning several universes (pixel products, with the pixel mapper), patching from a
visual plot (with the visualiser).

## Owner decisions (P1-15, 2026-10-24: every recommendation accepted)
1. **Move: back to back or keep the spacing?** Titan's repatch keeps the address gaps between the
   selected fixtures; the backlog row says back to back. *Recommend back to back* (PAT-20): it is
   what you asked for and what a church rig of identical pars needs. Keeping gaps can be a later
   option if you miss it.
2. **Change mode that no longer fits in place: refuse or re-pack?** *Recommend refuse* (PAT-22)
   with the clash in the warning: re-packing silently moves fixtures the operator did not ask to
   move. Change mode then Move is two undo steps, each one visible.
3. **Change mode on mixed fixture types:** *recommend allowed* when every type has a mode of that
   name (PAT-22; the dropdown shows only shared modes), instead of only one profile per range.
4. **Name without a trailing number:** *recommend the same name for every fixture* (Titan), so
   "Front" names all six "Front" and "Front 1" numbers them (PAT-23).
5. **Order:** *recommend number order*, not click order (the table is sorted by number, so what
   you see is what you get).
6. **Wire:** *recommend `ids` on the existing commands* (IPC-40) instead of new `*_range`
   commands: one command per edit, and a range of one is the old command.
7. **Handles of a range** (moving several Fixtures-window buttons at once, Titan Move with Retain
   Layout / Bunch Up) stay out of this spec: *recommend* a later PRG/UI task when the Fixtures
   window gets its own Move.

## Owner decisions (2026-10-17)
1. Internal id and user number are separate, as on Titan (PAT-06, PAT-12): the church is the
   playground, not the limit.
2. PAT-05: removal is always allowed and dangling data is ignored with a warning; a confirm
   for fixtures used in cues comes with cue lists (P1-08).
