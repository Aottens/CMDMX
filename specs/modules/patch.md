# Module spec: patch (PAT)

Owner: `cmdmx-core::patch` (pure) and `cmdmx-daemon` (IPC), patch screen in `subprojects/app`.
Status: **draft for owner approval (P1-11)**; owner decisions folded in 2026-10-17. Builds on `fixture-library.md` (profile + mode).
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
  the Fixtures window in patch order; `patch.handle {id, page, slot}` moves a button to a free slot
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
- **PAT-10** A Patch screen (full screen from the workspace rail, UI v0.1): a table of the patch
  (number, name, manufacturer/model, mode, universe.address, footprint), sorted by number; an Add
  panel with library search (LIB-14), mode, count, universe, start address and start number
  (both optional, PAT-03); Move, Renumber, Name, Change Mode and Remove on a selected row, Remove
  and Change Mode with a two-step confirm. A refused action shows the warning in the panel. Verified by
  `tests/manual/app-patch.md`.

### The church
- **PAT-11** The church rig from the owner's Lightkey export is patched once through these commands
  (or a one-off script that calls them) into the church show; the result is listed in
  `tests/hardware/church.md` (VS4-01) so the patch can be checked against the real fixtures.

## Out of scope (this spec)
Per-cell selection and Unfold (cells exist in the model, LIB-07; selecting them is a PRG task),
a fixture spanning several universes (pixel products, with the pixel mapper), patching from a
visual plot (with the visualiser).

## Owner decisions (2026-10-17)
1. Internal id and user number are separate, as on Titan (PAT-06, PAT-12): the church is the
   playground, not the limit.
2. PAT-05: removal is always allowed and dangling data is ignored with a warning; a confirm
   for fixtures used in cues comes with cue lists (P1-08).
