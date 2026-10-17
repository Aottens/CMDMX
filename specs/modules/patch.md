# Module spec: patch (PAT)

Owner: `cmdmx-core::patch` (pure) and `cmdmx-daemon` (IPC), patch screen in `subprojects/app`.
Status: **draft for owner approval (P1-11)**. Builds on `fixture-library.md` (profile + mode).
Behavioural reference: Titan *Patching* (fixtures from the library, user numbers, auto address).

## Purpose
Put the rig into the show: pick a fixture type and mode from the library, say how many, where
they start in DMX and which numbers the operator will type, and the engine places them without
overlaps. The patch is what the programmer, palettes and cues refer to, so it must be impossible
to end up with two fixtures on the same channel by accident.

## Definitions
- **Fixture:** `number` (the user number the operator types and every other part of the show
  refers to; unique), `name` (optional legend), `profile` (`manufacturer/model`), `mode`,
  `universe` (1-based), `address` (1–512).
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
  follow the fixture because they refer to its number.
- **PAT-05** `patch.remove` removes fixtures by number. They leave the selection, the programmer and
  every group; the frame channels they used drop to 0 at the next tick. Removal is always allowed
  in phase 1; recorded data referring to a removed number is ignored with a warning on load
  (SHW-08 rule for dangling references).
- **PAT-06** `patch.renumber` changes a fixture's number to an unused one and rewrites every
  reference in the engine (selection, programmer, groups; later palettes and cues) in the same
  step.
- **PAT-07** A fixture patched with a mode that has `virtual_dimmer` gets a Dimmer attribute
  without a channel (LIB-04); the patch list shows it as "virtual dimmer".
- **PAT-08** The patch is part of the show file (`patch` key, SHW-01) together with the profiles
  it uses (ADR-006 decision 5); loading a show re-runs the PAT-02 checks and reports clashes as
  warnings instead of refusing the show.

### IPC and app
- **PAT-09** IPC (extends `ipc.md`): the snapshot's `fixtures` entries gain `name`, `mode`,
  `universe`, `address`, `footprint` and `virtual_dimmer`; a `fixtures` delta is sent after any
  patch change. Commands: `patch.add {profile, mode, count, universe, address?, number?}`,
  `patch.move {number, universe, address}`, `patch.remove {numbers}`, `patch.renumber {number,
  to}`, `patch.name {number, name}`. Refusals are warnings (IPC-06).
- **PAT-10** A Patch screen (full screen from the workspace rail, UI v0.1): a table of the patch
  (number, name, manufacturer/model, mode, universe.address, footprint), sorted by number; an Add
  panel with library search (LIB-14), mode, count, universe, start address and start number
  (both optional, PAT-03); Move, Renumber, Name and Remove on a selected row, Remove with a
  two-step confirm. A refused action shows the warning in the panel. Verified by
  `tests/manual/app-patch.md`.

### The church
- **PAT-11** The church rig from the owner's Lightkey export is patched once through these commands
  (or a one-off script that calls them) into the church show; the result is listed in
  `tests/hardware/church.md` (VS4-01) so the patch can be checked against the real fixtures.

## Out of scope (phase 1)
Changing a patched fixture's mode (remove and re-add), multiple universes per fixture, cells/heads
as separate fixtures (LIB-07), DMX address offsets per line beyond OUT's line → node mapping,
patching from a visual plot.

## Open questions for the owner
1. Number and internal id are the same thing here (PAT-06 rewrites references when renumbering).
   Titan keeps an internal handle next to the user number; for a church-sized rig one number is
   simpler. OK?
2. PAT-05: removal is always allowed and dangling data is ignored with a warning. Titan asks before
   removing a fixture that is used in cues; we can add that once cues exist (P1-08).
