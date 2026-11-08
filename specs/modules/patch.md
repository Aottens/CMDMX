# Module spec: patch (PAT)

Owner: `cmdmx-core::patch` (pure) and `cmdmx-daemon` (IPC), patch screen in `subprojects/app`.
Status: **PAT-01..14 approved by owner 2026-10-17 (P1-11); PAT-09 wire details approved 2026-10-17 (VS2-12)**; PAT-15..18 (the church one-off, VS2-13) approved by owner 2026-10-18; PAT-09/10 amendment (modes dropdown, VS2-16) approved by owner 2026-10-18; PAT-19..25 (range edits, P1-15) approved by owner 2026-10-24; PAT-26..28: approved by owner 2026-11-02 (IDEA-35 batch A2, P1-26). PAT-29..36 (parking, IDEA-43, P1-35): approved by owner 2026-11-06. PAT-37..47 (batch E, P1-39): approved by owner 2026-11-08. Builds on `fixture-library.md` (profile + mode).
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
- **PAT-10** (The Remove confirmation says what it touches: UI-47.) A Patch screen (full screen from the workspace rail, UI v0.1): a table of the patch
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
CMDMX had no parking then (a removed fixture is gone, PAT-05), so every clash refuses; Park Conflicting
is PAT-32 (P1-35).

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

### Effects and palettes follow the patch (IDEA-35 batch A2; approved by owner 2026-11-02 (IDEA-35 batch A2, P1-26))
- **PAT-26** Removal reaches the effects. `patch.remove` (PAT-05) takes the fixture and its cells out of every
  cue's effects, every palette's effects and the effects of the cues running now, in the same command. Member
  positions are worked out again (FX-07); an entry left without members goes. So a running show gives the same
  frames as that show saved and reopened, which today it does not: the load drops these members
  (`from_show`) and `patch_remove` leaves them, also as the first member the FX-42 checks read. The load
  drops them from palette effects too (SHW-07), which it does not yet. No warning: removal is silent
  today, and what a delete should say is batch H5. Undo brings the members back, also in a running cue.
  Tests `pat_26_…` (frames equal to a reopen, palette effects, a gone first member and FX-42, undo while running).
- **PAT-27** A mode change reaches the effects as it reaches the values. A fixture whose new mode has none of the
  attributes an entry drives leaves that entry, in cue effects, the programmer's effects and the
  cues running now; an entry left without members goes. An entry that still has one of its attributes keeps the
  member. A cell member stays in every mode, as a cell's values do (PRG-53). It counts in the warnings of
  `apply_mode`: per value for one fixture, and the one line of PAT-22 for a range ("Dropped Strobe on 6
  fixtures"). Undo brings it back. Tests `pat_27_…`.
- **PAT-28** A mode change says what a palette can no longer do. Palettes keep what they store (PAL-19: the old
  mode can come back, and a shared value also serves other fixtures), but where a palette holds a
  value or effect for the fixture, per fixture or by type, on an attribute the new mode lacks, one warning per
  palette names it: `Palette 3 Fast strobe: no Strobe on 6 fixtures in this mode, the value stays`. A
  global value says nothing: it is meant for any fixture that has the attribute; an effect entry without members,
  as FX-22 records it, is the same (DEF-05, see `DECISIONS.md`). A change that loses nothing
  says nothing. Tests `pat_28_…`.

### Parking (P1-35, IDEA-43; approved by owner 2026-11-06)
Titan reference (*Changing the Patch*): Park "removes the fixture from the DMX output map, but all
programming is retained"; the old line and address are remembered and Unpark restores them; Patch View can
show parked fixtures greyed where they were patched; a repatch onto used channels offers *Park Conflicting
Fixtures*. Titan's Park is not the Park of other consoles (an output value held): that is Freeze. The manual
says nothing about groups or the selection; the owner wants a parked fixture out of them (IDEA-43), so
CMDMX skips it everywhere live and keeps it in the show for Unpark (owner decision 2026-11-06).

- **Parked:** a fixture that stays in the show (id, number, name, profile, mode, handle, its universe and
  address remembered, every group, palette and cue it is in) but is off the output map and out of every
  live list. Its channels are free for other fixtures.

- **PAT-29** `patch.park {ids}` parks fixtures; a range command under PAT-19 (an unknown or repeated id
  refuses all; a fixture already parked stays as it is). In the same command the fixtures leave the
  selection, the programmer, the located set and Highlight, as PAT-05 does for a removal, and their
  frame channels drop to 0 at the next tick. Nothing recorded changes. One undo step ("Park 4
  fixtures"), an edit command (UNDO-04), refused while locked (IPC-50).
- **PAT-30** Skipped live. A parked fixture cannot be selected: a tap on its tile or `201 Enter` is refused
  with "201 is parked"; a group (PRG-02), `Thro`, `And`, All and Rem Dim (PRG-28) leave it out without a word.
  A group whose fixtures are all parked selects nothing and warns "Group 3: every fixture is parked". So
  selection order, the grid, Fan and Fix ±1 count only what hangs. In effects (FX-07) member positions are
  worked out over the members that are not parked, so a chase over a group runs over the lamps that hang;
  the parked members stay in the entry.
- **PAT-31** `patch.unpark {ids}` puts fixtures back at their remembered universe and address under the
  PAT-02 checks as a range (PAT-19): when another fixture holds those channels by now, the whole command is
  refused naming the first clash ("201 would overlap 1201 at 1.37: give it a free address") and nothing
  changes. `patch.move` on a parked fixture (PAT-04, 20) unparks it at the new address (Titan: give it a new
  address). An unparked fixture is selectable again and plays its recorded data from the next tick.
  One undo step.
- **PAT-32** Park Conflicting. `patch.move` (one fixture or a range) and `patch.add` take
  `park_conflicting: true`: every fixture outside the move whose channels it would overlap is parked in
  the same command instead of refusing it, and the result says so ("Moved 201 to 1.37, parked 301, 302").
  A move past slot 512 or onto a used number is still refused. One undo step for the move and the parks.
- **PAT-33** What is kept. Recorded data for a parked fixture is kept as it is by every record, update and
  merge (cues, palettes, PBK-49): it outputs nothing and records nothing new while parked, because it
  cannot be in the programmer. Recording onto a used group (PRG-07) keeps the group's parked fixtures,
  after the recorded selection in their old order, so a group re-recorded while the stage is half hung
  still has them after Unpark. Removing a parked fixture (PAT-05) removes it as any other.
- **PAT-34** Wire and show. Snapshot `fixtures` entries (PAT-09) gain `parked` (bool); `universe` and
  `address` stay the remembered ones. IPC-58's `output` has no entry for a parked fixture. The show file's
  patch entry gains `parked: true` (absent = false), a schema step under SHW-04 with a migration that
  parks nothing. Loading a show leaves parked fixtures out of the PAT-08 clash checks.
- **PAT-35** In the app. The Patch screen shows a parked row greyed, its address column "parked (was
  1.37)"; the Selected panel has **Park** and **Unpark** for one row or a range (PAT-25), and Move with
  a clash offers **Park conflicting and move** under the warning. In the Fixtures window a parked
  fixture's tile stays at its handle, greyed with a `PARK` tag and not tappable (UI-11), so the layout does
  not shift and Unpark brings it back where it was. Verified by `tests/manual/app-patch.md` (amended).
- **PAT-36** The phone page (`remote.md` RMT-13) offers Park and Unpark on a fixture's card, and Park and
  move after a clash (RMT-11), through these same commands.

### Patch and profiles (IDEA-35 batch E, P1-39; approved by owner 2026-11-08)
From `docs/research/everyday-gaps.md` E, K10, M2 and M3; checked in `tests/manual/runs/2026-11-08-E-checks.md`. Titan
pages: *Changing the Patch*, *Copying, moving and deleting fixtures*, *Fixture Personality Options*, *Fixture Groups*.
Titan keeps programming through an exchange ("All programmed items including palettes will be preserved"), keeps Pan, Tilt
and Dimmer always, and matches the other functions, adjustable in Exchange Mapping. It exchanges one fixture at a time.
Here it takes a range, because M2 was seven washes. The Exchange Mapping screen is an owner question.

- **PAT-37** **Fixture Exchange** (M2). `patch.exchange {ids, profile, mode, universe?, address?}` gives fixtures another
  profile and mode, a range command under PAT-19. Each keeps its `id`, number, name and handle, so groups, the selection and
  everything recorded stay with it. The profile comes as in `patch.add` (PAT-09: a key the show has keeps the show's copy).
  An unknown profile or mode refuses the command, naming it. Without a place, each fixture keeps its universe and address and
  the new footprints are checked as in PAT-22. With `universe` and `address`, the range is placed back to back from there
  as in PAT-20, in the same command, because a block that grows cannot be moved apart first. A parked fixture has no channels
  to check and stays parked; with a place given it is unparked there (PAT-31). Old channels drop to 0 at the next tick where
  the new footprint no longer covers them, and the new type plays from that tick. One undo step ("Exchange 7 fixtures"), an edit
  command (UNDO-04), refused while locked (IPC-50). The seven washes of the M2 run stay in groups 5, 16 and 17 with
  all their cues. Tests `pat_37_…`.
- **PAT-38** **What carries over** (M2). In the programmer, in each cue's values and dormant values, an attribute is matched by
  name and kind of value as in PAT-13. Names come from LIB-05, so Pan, Tilt, Dimmer and Colour match across types; a
  virtual dimmer (LIB-04) is a Dimmer. A level inside a named slot (LIB-15), going to an attribute that has slots, moves to the
  middle of the slot with the same name; with no slot of that name it is dropped like a missing attribute, because the same
  level on another wheel is another gobo. Every other level stays as it is; position is not converted between pan
  ranges (LIB-16), so positions may need an Update, as on Titan. A palette reference stays a reference. Cell items
  follow PRG-53 and effects PAT-27. What is dropped gives one line for the range ("Dropped Gobo, Strobe on 7 fixtures").
  Running cues give the same frames as the show saved and reopened (PAT-26). Tests `pat_38_…`.
- **PAT-39** **Palettes carry over** (M2, and so no M5 refusal after an exchange). A palette's values for the fixture itself
  stay, with PAT-38's slot rule. A value on an attribute the new type lacks stays and is named as in PAT-28 ("in this
  type"), so an exchange back brings it into use. A palette with shared values (and shared cell values) for the old type and none
  for the new type gets a copy under the new type, by PAT-38's rule; the old type's values stay for fixtures that were not
  exchanged, and a palette that has its own values for the new type keeps them. Global values and effect entries without
  members do not change. Tests `pat_39_…`.
- **PAT-40** **Copy a patched fixture** (E2). `patch.copy {ids, number?}`, a range command under PAT-19, makes new
  fixtures of the same profile, mode and name, numbered upwards from `number` or after the highest in use (PAT-03), each on the
  next free handle (PAT-14). A copy starts parked at the original's place (Titan: "Parked", no DMX channel), so a Move gives it its
  address (PAT-31). It comes with everything recorded on the original, cells included: cue values, dormant values and order,
  effect membership right after the original, palette values for the fixture, and behaviour (PAT-44). It joins the original's
  groups right after it (owner question 5). The programmer and the selection do not change. One undo step ("Copy 4
  fixtures"); the message says "Copied 4 fixtures, parked: give them an address". Tests `pat_40_…`.
- **PAT-41** **Swap Addresses** (E2). `patch.swap_addresses {ids, with}` takes two ranges of equal size with no fixture in both;
  pairs in number order trade universe and address (Titan's *One For One*). Different sizes refuse the command, as does a
  parked fixture in either range ("201 is parked"). The new footprints are checked as a whole against every fixture outside
  both ranges and against each other (PAT-19). Nothing else changes, since recorded data follows the id. One undo step.
  Tests `pat_41_…`.
- **PAT-42** **Update from the library** (E2; Titan *Updating Patched Personalities*). `patch.update_profile {profile}` replaces the
  show's copy of one profile (PAT-09) with the library's current one, under LIB-25's checks: each used mode keeps its name and
  footprint and no stored attribute disappears, otherwise it is refused naming the mode or attribute, and nothing changes. Output
  follows on the next tick. One undo step ("Update profile Phantom 25 LED Spot"). No file is written. A key the library does not
  have refuses; an identical copy says "already up to date" and adds no step. Tests `pat_42_…`.
- **PAT-43** **Auto Groups** (E2, K run). `patch.add` takes `auto_groups: true`. A count of two or more makes a group of the
  new fixtures in number order, legend `<count> <model>` (Titan's "4 BB4"). A group legended `All <model>` (Titan's "All
  Robe Pointe") gets every fixture of the profile: appended to an existing group of exactly that legend, new otherwise, numbered in
  the first free group numbers with the type's group first. A count of one appends to an existing `All` group and makes
  no group. In the same undo step as the add. Tests `pat_43_…`.
- **Behaviour:** per fixture and attribute, what the output stage does to a level: `invert`, `low` and `high` (limits,
  0.0–1.0), `offset` (−1.0–1.0). All off by default. It lives on the fixture, not the profile.
- **PAT-44** **Setting behaviour** (E1). `patch.behaviour {ids, attribute, invert?, low?, high?, offset?}`, a range command under
  PAT-19, sets only the fields given. A fixture without the attribute refuses the command naming it. `low` above `high`, or
  a value out of range, refuses. Colour holds no level, so it refuses ("Colour cannot be inverted: use its channels"); a virtual
  Dimmer can take behaviour. `patch.behaviour_reset {ids}` puts every field back (Titan's Reset Behaviour; it leaves PAT-46's
  swap alone, as Titan does). Each is one undo step. Tests `pat_44_…`.
- **PAT-45** **At the output** (E1; Titan: applied "just before final output"). After merge and masters, before the DMX write, a
  level goes: invert (1 − v), then add the offset, then clamp to `low`..`high`. Every source goes through it: programmer,
  cues, effects, Locate, Highlight and a profile default. A cell's attribute takes the fixture's behaviour of that name. A virtual
  Dimmer's scale is the value after behaviour. What is stored never changes, and the programmer may hold values outside the limits
  (Titan). Pan 0.25 inverted outputs 0.75; Dimmer 1.0 with `high` 0.8 outputs DMX 204; offset +0.1 on 0.95 outputs 1.0. A
  fixture without behaviour costs one check per tick and nothing allocates. Tests `pat_45_…`, scenario on the frame.
- **PAT-46** **Swap Pan and Tilt** (E1). `patch.swap_pan_tilt {ids, swapped}`, a range command under PAT-19, sends the Pan
  value to Tilt's channels and the Tilt value to Pan's, coarse and fine, after each attribute's own behaviour. A fixture without both
  refuses the command naming it. One undo step. Tests `pat_46_…`.
- **PAT-47** **Wire and show.** Snapshot `fixtures` entries (PAT-09) gain `behaviour` (attribute → the fields that are not default)
  and `swap_pan_tilt`, both absent at their defaults. The show file's patch entry gains them too, as the next schema step
  under SHW-04, with a migration that changes nothing. A mode change or exchange keeps the behaviour of attributes that
  remain and drops the rest; a Copy takes it along. Tests `pat_47_…`.

### The church
- **PAT-11** The church rig from the owner's Lightkey export is patched once through these commands
  (or a one-off script that calls them) into the church show; the result is listed in
  `tests/hardware/church.md` (VS4-01) so the patch can be checked against the real fixtures.

The one-off is `cargo run -p cmdmx-daemon --example church -- <export.tsv>` (VS2-13, owner
decisions 2026-10-18). It reads the owner's Lightkey patch export from disk; the export and the
Lightkey show stay out of git. Options: `--show-dir` (default `~/Documents/CMDMX/Shows`),
`--profiles-dir` (default `~/Library/Application Support/CMDMX/profiles`) and
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
Per-cell selection and Unfold (cells exist in the model, LIB-07; selecting them is PRG-46..55),
a fixture spanning several universes (pixel products, with the pixel mapper), patching from a
visual plot (with the visualiser). Not in batch E (P1-39, owner questions 3 and 7): Titan's Exchange Mapping screen, Park Conflicting
on an exchange, Swap Addresses' Retain Layout, Update All, Freeze, Curves, Flip, Lamp on/off and fixture macros, RDM patching.

## Owner decisions (P1-39, batch E, 2026-11-08: every recommendation accepted)
1. **What is in batch E:** Fixture Exchange, find in Patch, Number and Address after Add, Copy, Swap Addresses, Update from library,
   Auto Groups, delete and duplicate a profile, Invert, Swap Pan/Tilt, Limits and Offset? Recommended: yes. Not chosen: Exchange and
   find alone now, the rest later, which leaves E's other points open for another spec round.
2. **The third review's proposal for E (K1 first, add K10):** K1 is already done (LIB-40, NEW-01), so only K10 remains. Approve K10
   in E? Recommended: yes, UI-74. Not chosen: leaving it, so each next type starts with a refusal.
3. **Exchange Mapping (PAT-38):** match by name, kind and slot name, and report what drops? Recommended: yes, the Exchange
   Mapping screen becomes an idea for later (IDEA-49). It is a screen as large as the exchange itself, and names are already normalised on
   import. Not chosen: Titan's screen now.
4. **A wheel value with no slot of that name (PAT-38):** drop it and say so? Recommended: yes, the fixture falls back to its default and
   the message names it. Not chosen: keep the level, which puts an unknown gobo or colour on stage without a word.
5. **Copy and groups (PAT-40):** the copy joins the original's groups? Titan's page names cues and palettes and says nothing about
   groups. Recommended: yes, it is parked until addressed, so it changes nothing live (PAT-30). Not chosen: no groups, so each
   needs adding by hand.
6. **Auto Groups by default (UI-74):** off, a toggle in the Add strip that the Mac remembers? Recommended: off, so nobody's group pool
   fills unasked. Titan's page does not give its default. Not chosen: on.
7. **Freeze, Curves and Flip (E1), Lamp on/off with fixture macros and RDM patching (E4):** out of batch E, each an idea for later in
   phase 1 (IDEA-50..54)? Recommended: yes. Each needs something that is not there: Freeze a per-attribute record of the last output, Curves shapes the
   manual does not define (only Linear is described), Flip and macros data the profiles do not hold, RDM a discovery path in
   the protocols crate and an RDM-capable node to test with. A control channel's named slots can already be set by hand. Not chosen:
   in E, which would double it.

## Owner decisions (P1-35, 2026-11-06, IDEA-43)
1. Phase 1, beside the phone page (P1-34).
2. A parked fixture is skipped live and kept in groups, palettes and cues for Unpark (PAT-30, 33). Not
   chosen: taking it out of its groups, which Unpark could not undo.
3. Both proposals approved with this spec on 2026-11-06: Unpark onto taken channels is refused, not re-placed
   elsewhere (PAT-31); the tile stays greyed in the Fixtures window rather than disappearing (PAT-35).

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

## Owner decision (P1-26, 2026-11-02: the recommendation accepted)
1. **PAT-27, an effect on a mode without its attribute:** the fixture leaves the entry, with a warning, as values do, so
   a cue's values and effects cannot disagree; undo brings it back. Not chosen: staying and warning, which would
   restore the effect if the mode comes back, but leaves a member in the cue that does nothing.
