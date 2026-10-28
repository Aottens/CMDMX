# ADR-005: Colour is a device-independent target colour, extra emitters are driven directly

**Status:** accepted (owner, 2026-10-17; automatic white/lime extraction stays a later opt-in). Decisions 2 and 4 amended by ADR-010 (proposed 2026-10-28): automatic emitter mixing as an opt-in per mode, ahead of calibration.
**Date:** 2026-10-17

## Context
The church has about 20 LED pars of three colour systems (RGBW, RGB+Lime, plain RGB with a
virtual dimmer) next to four RGBW washes. One colour choice has to look like one colour on all of
them, and later (IDEA-02, IDEA-03) a LEE gel number or a calibrated match must land on every
fixture type. That only works if palettes, cues and the programmer store a colour as *what the
operator wants to see*, not as raw DMX per emitter: raw values recorded on an RGBW par mean
nothing to an RGB+Lime par, and a show file full of raw values would have to be rewritten when
calibration arrives. The owner also wants the extra emitters under his own hand: Lime makes a fine
amber, White a real white (2026-10-17).

Titan does something similar: its colour mixing applies to all colour systems in a selection
(v17), with a per-emitter "Channel" view next to RGB/CMY/HSI.

## Decision
1. **One colour attribute per fixture.** A fixture that mixes colour has one logical attribute
   `Colour` (group C) whose value is a **target colour**: a linear-light RGB triple `{r, g, b}`,
   each 0.0–1.0, in a defined reference space: **ITU-R BT.2020 primaries, D65 white, linear (no
   gamma)**. HSI, RGB and CMY on the wheels are views of this one value (PRG-30); CMY is
   `1 − RGB`. Because the space is a standard one, every stored colour converts exactly to CIE
   XYZ/xy, which is what gels (IDEA-02) and calibration (IDEA-03) need later.
2. **Extra emitters are ordinary attributes.** White, Lime, and later Amber, UV, … are separate
   attributes in group C with their own names and plain 0.0–1.0 values (PRG-34). The target
   colour never drives them in phase 1: they are 0 unless the operator, a palette or a cue sets
   them. Output per emitter = its own value; nothing is added to or subtracted from RGB.
3. **Phase-1 mapping is nominal.** Each fixture maps the target colour onto its own mixing
   channels as if its emitters were the reference primaries: RGB and RGBW and RGB+Lime fixtures
   get `R = r, G = g, B = b`; CMY fixtures get `C = 1 − r, M = 1 − g, Y = 1 − b`. A fixture
   without a dimmer channel (virtual dimmer, LIB/PAT) multiplies its mixing channels by its
   Dimmer value. Colour wheels (slot-based colour): the target colour picks the nearest slot by
   the slot colours in the profile (amended 2026-10-17, owner: no church-driven cuts; data in
   LIB-15, behaviour after the mixing case).
4. **Phase 2 replaces only the mapping.** Calibration (IDEA-03) maps the same target colour onto
   each fixture's real emitters (CIE xy from GDTF or a measurement, plus per-unit trim) and may
   offer, as an opt-in, to use extra emitters for better whites and ambers. Stored colours do not
   change; the show file is not migrated.
5. **Storage.** Programmer, palettes and cues store `"Colour": {"r": 1.0, "g": 0.42, "b": 0.0}`
   and emitter values by name, `"White": 0.6`, `"Lime": 0.3`. A colour palette recorded with
   the C mask stores the target colour and the emitter values that were touched. **Applying a
   colour palette that contains a target colour also sets every extra emitter of that fixture
   that the palette does not mention to 0**, so recalling Blue after a warm white with White
   up gives blue, not pastel blue. A palette with only emitter values (say "Lime 100 %") leaves
   the target colour alone. Global / Shared / Normal detection for colour palettes compares
   these stored values (P1-03).

## Alternatives
- **Raw DMX per emitter in palettes and cues** (what most entry-level consoles do): simplest, but
  a palette recorded on one fixture type does not work on another, and IDEA-02/03 would mean
  rewriting every show file.
- **CIE xyY as the stored value:** truly device-independent, but every fixture already needs gamut
  mapping in phase 1, and the RGB picker becomes a conversion instead of a value. The chosen
  space converts to xyY exactly, so nothing is lost.
- **sRGB / Rec.709 linear as the reference space:** familiar, but LED pars have more saturated
  primaries than sRGB, so their deepest greens and blues could not be stored. BT.2020 covers them.
- **Automatic white/lime extraction from the target colour** (W = min(R, G, B) and similar):
  better whites without effort, but less predictable, and it fights direct emitter control. Kept
  as a phase-2 opt-in on top of calibration.

## Consequences
- A programmer value is no longer always one number: the core needs a value type with a scalar
  and a colour case (`Value::Level(f32) | Value::Colour { r, g, b }`), used by the programmer,
  `contents`, playbacks and the merge (`Colour` is LTP; each component is written to its channel).
  This lands with VS2-06.
- Profiles describe a colour mixing system (`RGB` or `CMY`, with the channel of each component)
  instead of separate Red/Green/Blue attributes; emitters stay attributes. Part of
  `fixture-library.md` (P1-01) and the minimal test profile.
- PRG-17 (colour-group wheels), PRG-30..34 are implemented against this model in VS2-06;
  palettes (P1-03) and the show file (P1-09) use the storage in decision 5.
- With nominal mapping, mixed fixture types will not match perfectly in phase 1. That is the
  known ceiling; IDEA-03 lifts it without touching shows.

## Reversal trigger
A church test shows that nominal mapping makes the mixed pars unusable side by side (then pull
calibration forward; storage stays), or operators find direct-only emitters too much work for
everyday whites (then make automatic extraction available earlier as an opt-in).
