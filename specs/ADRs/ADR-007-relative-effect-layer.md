# ADR-007: A relative effect layer in the merge, effect references in stored contents

**Status:** proposed (owner approval pending)
**Date:** 2026-10-22
**Origin:** IDEA-18 (the FX generator as a phase-2 flagship), BACKLOG P1-17

## Context
The phase-2 effect (shape) generator must become the best on the market (IDEA-18). Its design is
open, but two things in phase 1 decide whether it can be added without rework:
- **The merge.** On Titan, ONYX, grandMA3 and Hog most effects are *relative*: a circle around the
  position a cue or palette gives, a dimmer wave around the current level. The merge of
  `engine-merge.md` (ENG-01..13) resolves one value per attribute and has no place for a second
  contribution on top of it. Cue playback (VS3-03..05) and group masters (VS3-06/07) are already
  built on that merge.
- **Stored contents.** Programmer, cue and palette contents map fixture → attribute → value or
  palette reference. An effect has to be recordable next to those values, in the same cue or
  palette, without a show schema bump before effects exist.

Today's tick (`Engine::tick`) per fixture and attribute: merge → for `Htp`: group masters
(GRM-06, Take Over passes GRM-10) → Grand Master/DBO (ENG-06, PBK-30) → frame writer, which
applies the virtual dimmer (LIB-04) and the coarse/fine split. `Ltp`: merge → held value → frame
writer.

## Decision
1. **The merge gets an effect layer between the base value and the masters.** Per fixture and
   attribute and per tick:
   1. **Base:** the value ENG-01..10 resolve today (priority, HTP/LTP, fades, release, the held
      `Ltp` value, 0 for an `Htp` attribute nobody proposes).
   2. **Effect:** at most one effect acts on the attribute, chosen among the active sources that
      hold an effect for it: highest priority, then most recently activated (LTP, also on `Htp`
      attributes), *independently of which source owns the base*. A **relative** effect adds an offset evaluated from the tick
      time; an **absolute** effect replaces the base.
   3. **Clamp** to the attribute's range (0.0–1.0, per component for a colour).
   4. **Group masters** (GRM-06) and **Grand Master/DBO** (ENG-06), `Htp` only, as now.
   So a programmer circle runs around the pan/tilt the cue list gives, and a group master or the
   Grand Master scales a dimmer effect like any other intensity.
2. **Effects are referenced, not stored inline.** Programmer, cue and palette contents may hold
   an optional **effect reference** per fixture and attribute next to its value, in a field that
   is absent until used. A show without effects is byte-identical to today's and keeps schema 5;
   the first build task that stores an effect raises the schema. What an effect is (shape, size,
   speed, spread, phase per fixture, BPM lock) belongs to a new `effects.md` in phase 2.
3. **Phase 1 builds no effect code.** The criteria added now (ENG-14..19) are satisfied by the
   current engine, which has no effect sources: base = output. They fix the order so the FX
   generator plugs in without redoing `engine-merge.md`, and a test for each lands with the first
   effect task.

## Alternatives
- **Effects as ordinary sources that win the merge (absolute only):** simple, but a circle could
  never follow a cue's position, which is exactly what makes shapes quick on Titan.
- **Effects after the group masters:** a dimmer effect would escape a Limit master and the Grand
  Master; a limit must stay a hard limit (GRM-09).
- **Several effects adding up on one attribute:** powerful (grandMA3 phasers), but makes the
  output hard to predict; left to `effects.md` as an open question, the order here allows it.
- **Inline effect definitions in cues:** duplicates shapes across cues and blocks editing one look
  in one place; references match how palettes already work (PAL references).

## Consequences
- `engine-merge.md` gains the definitions of base value and effect source and ENG-14..19.
- `cuelists.md` (cue contents) and `palettes.md` (palette contents) mention the optional effect
  reference; programmer contents get it in the same way when effects are specified.
- The held `Ltp` value stays the base, never base + offset, so stopping an effect returns the
  attribute to where it was.
- Take Over (GRM-10) compares against the mastered output, effect included: what the operator
  sees on stage.
- No change to show schema, IPC or app now.

## Reversal trigger
The phase-2 FX design study (IDEA-18) shows that effects must act after the masters or that
effect parameters must live inline in cues; then this ADR is superseded before any effect code.
