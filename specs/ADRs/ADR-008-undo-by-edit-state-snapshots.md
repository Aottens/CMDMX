# ADR-008: Undo by snapshots of the edit state, one history in the daemon

**Status:** proposed (owner approval pending)
**Date:** 2026-10-22
**Origin:** BACKLOG P1-10, `specs/modules/undo.md`

## Context
Undo has to cover programmer, palettes, playbacks, cue lists, group masters, patch and show
settings, and every module built later (owner, 2026-10-22: build undo first). Every IPC command
reaches the engine through one function, `apply` in `cmdmx-daemon`, on the tick thread. The
engine already separates what is stored (`Engine::to_show`) from live state (SHW-18), and the
programmer is a small extra part. A show like *Kerk* (56 fixtures) is small to clone; commands
arrive at human speed, not per tick.

## Decision
1. **A step saves the edit state before the command, not an inverse command.** The edit state
   is the stored show plus the programmer (`undo.md` Definitions). Undo swaps it back in; live
   state stays. No module writes undo code of its own: a new edit command is undoable as soon as
   it is classified `edit` (UNDO-04).
2. **One history in the daemon**, wrapped around `apply`: classify, save the edit state for an
   `edit` command that starts a new step, apply, drop the step if the command failed or changed
   nothing. The history itself (`cmdmx-core::undo`) is pure and has no clock; consecutive
   commands merge by a coalescing key, never by time (ENG-11).
3. **Restoring is not loading.** Undo swaps the edit-state fields in place and then reconciles
   the live state (UNDO-10..12): removed playbacks are killed, id counters never go down, group
   master levels stay. It does not go through `Engine::from_show`, which would reset live state.

## Alternatives
- **Inverse commands per module (command pattern):** small history, but every command needs a
  hand-written inverse in every module, built and tested forever; one forgotten inverse breaks
  undo silently. Snapshots get every module for free.
- **Diffs of the show JSON:** smaller memory, but slower and more code than a clone at command
  rate; worth it only if memory is measured to be a problem (profiles are the large part and can
  be shared, e.g. `Arc`, without changing this decision).
- **History in the show file:** survives restarts, but grows the file, writes on every edit, and
  undoing yesterday's edit is rarely wanted; the `.bak` and autosave already cover a crash.

## Consequences
- `Engine` gets an `EditState` type with capture/restore; adding a field to the show or
  programmer means deciding whether it is edit or live state (the type makes that explicit).
- Memory: up to 100 copies of the edit state (UNDO-08). Fine for phase 1 rigs; measure with a
  large show before phase 2.
- Multi-user (later) needs a history per user; with snapshots that means a per-user programmer
  part, which this design does not block.
