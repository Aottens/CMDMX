# ADR-010: Extra emitters can be mixed automatically from the target colour (opt-in, nominal)

**Status:** proposed (owner decisions 1–4 of IDEA-32 taken 2026-10-28; criteria wait for gate 1)
**Date:** 2026-10-28
**Amends:** ADR-005 decision 2 (emitters only direct) and decision 4 (extraction only with calibration).

## Context
ADR-005 keeps the extra emitters (White, Lime, Amber, …) under the operator's hand only and puts
automatic white/lime extraction behind calibration (IDEA-03, phase 2). The owner wants it now
(IDEA-32): a warm white on an RGBW par should come mostly from its White LED, an amber on an
RGB+Lime par should use the Lime, without the operator balancing four wheels per look. Calibration
is still far away, so the mix must work on what profiles already know: GDTF emitter chromaticity
(LIB-32) or a sensible default per emitter name. The owner also wants whites by colour temperature
(Kelvin) and CTO/CTB-style warming and cooling.

## Decision
1. **Opt-in per mode.** A profile mode gets `automix: true|false` (default false, LIB-36). Only an
   RGB mode can opt in; CMY modes, colour wheels and modes without a colour system cannot. Off =
   exactly today's output, so no existing show changes.
2. **Emitter colour is data in the profile.** Each extra emitter that takes part has a
   chromaticity `cie: [x, y]` (LIB-32; GDTF fills it). When the operator switches auto-mix on in
   the profile editor, emitters without `cie` get a default by name (White, Cold White, Warm
   White, Amber, Lime; LIB-37) that the operator can change. The engine never reads names
   (LIB-17 holds): an emitter without `cie`, and every UV emitter, is never auto-mixed.
3. **Nominal model, greedy extraction.** The fixture's R, G, B stay the BT.2020 reference
   primaries (ADR-005 decision 3), so the target colour *is* the fixture's nominal RGB drive.
   Each auto emitter is assumed, at full, to give exactly the light of the brightest RGB mix of its
   own chromaticity (its *RGB equivalent*, max component 1), divided by a per-emitter output
   factor (default 1.0, LIB-38) that the operator tunes by eye. In profile order, each emitter
   takes as much of the remaining RGB as its RGB equivalent fits under (the generalised
   `W = min(R, G, B)`), and leaves the rest in RGB (ENG-20, 21). This is a non-negative solve:
   no channel goes below 0, the colour and the brightness stay what the target colour says in the
   nominal model, and it is a fixed number of multiply-adds per emitter, with no solver, no
   iteration and no allocation per tick.
4. **Direct wheels add on top.** A direct emitter value (PRG-34) is added to the auto level and
   clamped at 1.0 (ENG-22). Palettes, cues and the programmer store exactly what they store today
   (ADR-005 decision 5); palette zeroing zeroes the direct part only.
5. **Colour temperature is a view, not a new value.** A fourth colour model, Kelvin, shows and sets
   the target colour as a point on the Planckian locus (1800–10000 K) at the colour's current
   brightness; CTO and CTB steps shift that point by their nominal mired values (PRG-63..66). The
   result is an ordinary target colour: palettes, cues and the show file are untouched. LEE gel
   numbers stay in IDEA-02.
6. **Calibration still replaces the mapping later.** IDEA-03 can swap the nominal primaries and
   output factors for measured ones without changing stored colours; this ADR only fills the gap.

## Alternatives
- **Wait for calibration (ADR-005 as is):** no new risk, but whites and ambers stay a
  four-wheel job for months, and the owner wants it now.
- **Least-squares or linear-programming solve over all emitters:** handles three or more
  overlapping emitters "optimally", but needs an objective nobody can tune by eye, iterates per
  tick, and is hard to test. The greedy pass is predictable: the order is the profile order the
  operator sees and can change.
- **Maximum brightness (RGB stays, emitters add light):** brighter, but a colour would change level
  when a fixture type opts in and would no longer match the RGB-only pars beside it.
- **Name-based behaviour in the engine:** simpler profiles, but breaks LIB-17 and every emitter
  that a manufacturer names differently. Names only seed editable data.
- **Kelvin as a stored value (`{"kelvin": 3200}`):** would need a new value type, migration and
  merge rules; a target colour already holds every white exactly.
- **Direct value as the max of auto and direct (HTP):** never over-drives, but a direct wheel then
  does nothing until it passes the auto level (a dead zone). Kept as an owner question.

## Consequences
- The frame writer (`set_mixing` in `cmdmx-core::engine`) learns one more step for opted-in
  modes; each emitter's RGB equivalent is computed once when the profile is loaded or edited, not
  per tick.
- Profiles gain `automix` per mode and `output` per emitter; both default so old profiles and shows
  load unchanged (LIB-01 round-trip).
- The nominal model is wrong in absolute terms (a real White LED is often brighter than RGB white):
  the output factor is the knob, the profile editor the place, IDEA-03 the real fix.
- The colour models become HSI / RGB / CMY / Kelvin (IPC-19 grows a fourth button).

## Reversal trigger
On the rig the auto-mixed whites or ambers look visibly different from the RGB-only pars and the
output factor cannot fix it (then pull IDEA-03 forward), or operators find direct-plus-auto
confusing (then switch to HTP, the alternative above).

## Open questions for the owner
1. Direct on top of auto: **sum, clamped at 100 %** (proposed, ENG-22) or the highest of the two
   (HTP)?
2. Order with two whites (Warm + Cold White): profile order (proposed), so a warm target fills the
   first white listed. Enough, or should the closest-in-colour emitter go first?
3. Kelvin range 1800–10000 K and the CTO/CTB mired values of PRG-66: right for you?
4. Should the Kelvin view also exist for fixtures without auto-mix (it then mixes whites from RGB
   only)? Proposed: yes, it is a view of the target colour like HSI.
