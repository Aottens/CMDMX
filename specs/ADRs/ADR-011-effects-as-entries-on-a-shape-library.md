# ADR-011: Effects are entries (shape + parameters + members) on a built-in shape library

**Status:** accepted (owner, 2026-10-29, the four decisions of `effects.md` as proposed)
**Date:** 2026-10-29
**Origin:** IDEA-18 (the FX generator, owner picked it as the next feature 2026-10-29), ADR-007
**Amends:** ADR-007 decision 2 (where an effect reference lives); `engine-merge.md` ENG-17 (wording)

## Context
ADR-007 fixed the order in the merge (base → effect → clamp → masters) and that stored contents
refer to effects instead of copying them per fixture. It left open what an effect *is*. The owner
wants the best effect generator on the market: Titan was quick (pick a shape, Size / Speed /
Spread on three wheels, relative to the current look); ONYX was hard work (many parameters, no
clear result). grandMA3 phasers are powerful but abstract; Eos and ChamSys put effects in
palettes ("FX palettes") so a look is one tap.

Four things have to be decided before criteria:
1. the unit that is stored and edited;
2. what a shape is (a waveform per attribute or a path over several attributes);
3. how a value comes out of it on a base (relative / absolute) and how a fader acts on it;
4. how time and tempo drive it, deterministically and without allocation per tick.

## Decision
1. **An effect entry is the unit.** One entry = a **shape** (from the built-in library), its
   **parameters** (size, speed and tempo source, spread, phase, width, direction, wings / groups /
   blocks, mode) and its **members**: the ordered list of fixtures (or cells) it runs on, in
   selection order. The members' order is what spread works over. An entry lives in exactly one
   owner: the programmer, a cue, or a palette (an **effect palette** holds entries *without
   members*: shape + parameters, applied to whatever is selected, as Titan / Eos FX palettes).
   A cue or programmer entry may point at an effect palette instead of holding its own parameters
   (`{"palette": id}`, resolved every tick as PAL-11 does for values), so editing the palette
   changes every cue that uses it.
   *This amends ADR-007 decision 2:* instead of an effect field per fixture and attribute, the
   owner holds a list of entries and "source *s* holds an effect for fixture *f*, attribute *a*"
   means *f* is a member and *a* is one of the entry's attributes. Nothing was ever stored in the
   per-attribute field (no effect code exists), so nothing migrates. ENG-14..19 stay as written.
2. **Shapes are built in, by kind, and few.** Three kinds, each with a short list:
   - *Wave* on one scalar attribute (Dimmer, Pan, Tilt, Zoom, Iris, Focus, an emitter, …):
     Sine, Triangle, Ramp up, Ramp down, Square, Random.
   - *Path* on the Pan/Tilt pair: Circle, Eight, Square, Triangle, Line (Pan), Line (Tilt).
   - *Colour* on the one target colour (ADR-005): Rainbow (hue around the circle), Hue shift
     (relative to the base colour), Colour mix (blend through a list of 2–8 colours, which may be
     palette references).
   On top sits a **gallery of looks**: named presets (Dimmer chase, Dimmer wave, Sparkle, Circle,
   Fan sweep, Rainbow wave, Colour chase, …) that are just a shape plus default parameters. The
   operator starts from a look, not from a waveform; the waveform is one tap away for those who
   want it. User-defined key-frame shapes (Titan) are later (see *Alternatives*).
3. **Relative by default, absolute by choice, the fader scales the size.**
   - *Relative* (default for Path, Hue shift and Wave on any `Ltp` attribute): value = base +
     size · (w − ½), *w* ∈ [0, 1] from the waveform. A circle runs around the position the cue
     gives; a Pan wave sweeps around it.
   - *Absolute* (default for Wave on `Htp`, Rainbow and Colour mix): value = top − size · (1 − *w*)
     for a scalar (top defaults to 100 %, so a dimmer chase runs 0–100 % at size 100 % and 50–100 %
     at size 50 %, whatever the base is); the colour itself for a colour shape.
   - In a playback the playback's *m* (PBK-15..17, CUE-31) scales the size for `Ltp` and Path
     effects (the circle grows with the fader, Titan's *fader controls size*), and scales the
     result like any intensity for an absolute `Htp` effect (ENG-05). A rising *m* is how an effect
     fades in; a falling *m* (release, kill time) is how it fades out, so ENG-18's jump never
     happens on a playback with times.
   - At most one effect per attribute (ENG-15 unchanged); a Path counts on both Pan and Tilt.
4. **An effect clock per entry, advanced from tick timestamps.** Each running entry keeps a phase
   clock θ (in cycles) that every tick advances by Δt · rate, rate = effective BPM / (60 · beats),
   exactly as a chase's step clock (CUE-28): a tempo change never jumps the phase. The effective
   BPM comes from the same speed sources as chases: `Free`, `Bpm n`, `Rate n` (CUE-29, 35, 36);
   a tap on a BPM master beat-syncs every entry that follows it. Per member the phase offset is
   computed **when the entry changes**, not per tick, into a fixed-size slot owned by the entry;
   per tick the engine only evaluates the waveform (a sine, a hash for Random). ENG-17's "from the
   tick's timestamp and its own start time only" is reworded to "from tick timestamps only (its
   clock advances by Δt per tick, CUE-28)": still no system clock, same scenario → same bytes.

## Alternatives
- **Effects as copies inline per fixture and attribute** (ADR-007's original field): simple to
  merge, but spread and member order then live nowhere, and a 24-fixture circle is 48 copies.
- **One global effect table, everything by id** (ONYX-like FX library): every busked effect
  becomes a named object; the pool fills with "FX 37"; editing one in the programmer changes
  recorded cues by surprise. Entries owned by their programmer / cue / palette avoid that; effect
  palettes give sharing where the operator asks for it.
- **Several effects adding up on one attribute** (Titan multiple shapes, grandMA3 phasers): rich,
  but the result is hard to predict and to undo by ear. Kept out (ENG-15); a Path covers the
  common "pan + tilt" pair.
- **User key-frame shapes now** (Titan Key Frame Shapes, grandMA3 phaser steps): the most powerful
  tool, also the one that made ONYX slow. Colour mix over 2–8 colours covers the common key-frame
  use (colour chase); free key frames become a later idea.
- **Multiplicative dimmer effects** (output = base · w): nothing happens on a dimmer at 0, the most
  common beginner's trap on Titan. Absolute-by-default for `Htp` avoids it.

## Consequences
- New spec `effects.md` (FX-01..), with ENG-14..19 tests landing in the first core task.
- `palettes.md`: an effect palette (palette with entries, no values); `cuelists.md`: a cue holds
  an entry list; tracking (CUE-50) tracks entries per member and attribute as values.
- Show schema 7 → 8 (`effects` lists in cues, palettes and the saved programmer; SHW-04).
- `engine-merge.md` ENG-17 reworded as above (no behaviour change for chases).
- The app gets the Shapes workspace (UI-07's disabled entry), Shapes & Effects softkey (UI-18).
- Built-in shapes are code, not data files: a short list of pure functions, unit-tested.

## Reversal trigger
Operators in the owner's runs keep wanting two effects on one attribute (then allow adding layers
with a fixed order), or keep wanting their own waveforms (then key-frame shapes before more looks).
