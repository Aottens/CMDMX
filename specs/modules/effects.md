# Module spec: effects (FX)

Owner: `cmdmx-core::effect` (shapes, entries, clocks; pure), the merge in `cmdmx-core::engine`,
`cmdmx-daemon` (IPC) and `subprojects/app` (Shapes workspace).
Status: **FX-01..44 approved by owner 2026-10-29 (P2-01, IDEA-18), the four owner decisions as proposed**.
Structural choices: ADR-011 (on top of ADR-007). Merge order: `engine-merge.md` ENG-14..19.
Behavioural reference: Avolites Titan v19 manual, *Shapes and Effects* — Shape Generator
(https://manual.avolites.com/docs/effects/shape-generator) and Key Frame Shapes
(https://manual.avolites.com/docs/effects/key-frame-shapes); operators' praise for Eos/ChamSys
FX palettes and grandMA3's wings/groups/blocks. The behaviour below is CMDMX's own.

## Purpose
The effect generator is the phase-2 flagship (IDEA-18): **intuitive, fast, a professional result
in seconds.** The operator selects fixtures, taps a *look* ("Dimmer chase", "Circle", "Rainbow
wave"), and it runs at once around the current look; three big controls (Size, Speed, Spread)
on the screen and the encoders shape it; it records into cues and palettes like any value. What
made ONYX slow (a page of parameters before anything moves) is avoided by starting from a result
with good defaults; what made Titan quick (relative shapes, Size/Speed/Spread on the wheels) is kept.

## Definitions
- **Shape:** a built-in function of a phase θ (in cycles) to a value. Three kinds (ADR-011):
  - **Wave** on one scalar attribute: `Sine`, `Triangle`, `RampUp`, `RampDown`, `Square`,
    `Random`; gives *w* ∈ [0, 1].
  - **Path** on the Pan/Tilt pair: `Circle`, `Eight`, `Square`, `Triangle`, `LinePan`,
    `LineTilt`; gives an offset (*x*, *y*) ∈ [−½, ½]².
  - **Colour** on the `Colour` attribute (ADR-005): `Rainbow`, `HueShift`, `ColourMix`.
- **Look:** a named preset in the gallery: a shape, the attribute(s) it acts on and default
  parameters (FX-30). Looks are what the operator picks; they are not stored, the entry is.
- **Effect entry** (ADR-011): a shape, its attributes, its parameters (FX-06) and its
  **members**: the ordered fixtures (or cells, PRG-46) it runs on. Owned by exactly one of: the
  programmer, a cue, a palette. A palette's entries have no members. An entry may instead of its
  own parameters point at an **effect palette** (`palette`), resolved every tick (PAL-11).
- **Current entry:** the programmer entry the effect controls (screen, encoders) act on.
- **Base value:** as `engine-merge.md`: the value ENG-01..10 resolve, before the effect.
- **Effect clock:** per running entry, θ_total (cycles since the entry started), advanced every
  tick from tick timestamps (FX-12).
- Church examples: washes 101–104, RGBW pars 201–208, RGB+Lime pars 211–216, RGB pars 221–226,
  front spots 301–306 (moving heads with Pan/Tilt in the demo rig).

## Acceptance criteria

### Shapes (pure functions; θ is the fractional phase in [0, 1))
- **FX-01** Waves, *w*(θ): `Sine` ½ − ½·cos 2πθ (starts at 0, top at θ = ½); `Triangle`
  1 − |2θ − 1|; `RampUp` θ; `RampDown` 1 − θ; `Square` 1 while θ < `width`, else 0; `Random` a
  value in [0, 1] from a fixed hash of (entry seed, member position *k*, whole cycle number),
  held for the whole cycle, so members at the same position flash together and the same scenario
  gives the same sparkle (ENG-11). The entry seed is set from a counter when the entry is made
  and saved with it.
- **FX-02** Paths, (*x*, *y*)(θ): `Circle` (½ sin 2πθ, ½ cos 2πθ); `Eight` (½ sin 2πθ,
  ½ sin 4πθ); `Square` and `Triangle` run their outline at constant speed through their corners
  at equal parts of the cycle, starting at the first: `Square` (−½, −½), (½, −½), (½, ½), (−½, ½)
  at θ = 0, ¼, ½, ¾; `Triangle` (0, ½), (½, −½), (−½, −½) at θ = 0, ⅓, ⅔; `LinePan`
  (½ sin 2πθ, 0); `LineTilt` (0, ½ sin 2πθ). When the fixture's profile has both `pan_degrees`
  and `tilt_degrees` (LIB), *y* is multiplied by `pan_degrees / tilt_degrees` so a circle is
  round on stage, not an ellipse (spot with 540°/270° → *y* doubled).
- **FX-03** Colours: `Rainbow` the target colour of HSI (θ · 360°, saturation 1, intensity 1)
  with PRG-30's conversion; `HueShift` the base colour with its hue turned by size · (*w* − ½) ·
  360° (*w* from a `Sine`), saturation and intensity kept: on a white base it shows nothing (the
  operator sees why in the editor, FX-42); `ColourMix` runs through its list of 2–8 colours
  *c*₀…*c*ₖ₋₁ (values or colour-palette references, resolved every tick): step *s* = ⌊θ·*k*⌋,
  *f* = frac(θ·*k*), and the colour fades from *c*ₛ to *c*ₛ₊₁ (wrapping) by
  *t* = clamp((*f* − (1 − `crossfade`)) / `crossfade`, 0, 1) — over the step's last `crossfade`
  part (0 % snaps, 100 % blends all the time), linearly in linear target RGB as
  cue fades are (PBK-15). The colour entry list is the one thing a key frame was needed for.
- **FX-04** Every shape function is total: any finite θ gives a finite value in its range; a
  non-finite θ gives the value at θ = 0 and logs once (show-proof rules).

### Parameters
- **FX-05** Value on the base, per member and attribute, after FX-07's phase:
  - **Relative** (`mode: Relative`): scalar wave = base + size · (*w* − ½); path = base + size ·
    *x* on Pan and base + size · *y* on Tilt; `HueShift` as FX-03.
  - **Absolute** (`mode: Absolute`): scalar wave = `top` − size · (1 − *w*) (`top` default 1.0);
    a path is centred on `top_pan`, `top_tilt` (defaults: the base when the entry was made);
    `Rainbow` and `ColourMix` give a colour *e* and the value is lerp(base, *e*, size), so size
    100 % is the effect's colour and less mixes the base in.

  Then ENG-16's clamp. Defaults: `Absolute` for a wave on an `Htp` attribute, `Rainbow` and
  `ColourMix`; `Relative` for everything else. Washes 101 Pan 0.5, `Circle` size 0.2 → Pan runs
  0.4–0.6 around the cue's position; Dimmer `Square` absolute, size 1.0, top 1.0 → 0 or 100 %
  whatever the base; size 0.5 → 50 % or 100 %.
- **FX-06** Parameters and ranges (refused with a warning outside them, nothing changes):
  `size` 0–1 (in attribute units; for a path in Pan units); `top`; `bpm` 0–600 (0 = stopped;
  cycles per minute at `beats` 1); `beats` ¼, ½, 1, 2, 4, 8 or 16 (a cycle lasts `beats` beats);
  `speed_source` `Free` | `Bpm n` | `Rate n` (as CUE-29); `spread` 0–1440°; `phase` 0–360°;
  `width` 1–100 % (`Square`); `crossfade` 0–100 % (`ColourMix`); `direction` `Forward` |
  `Backward` | `Bounce`; `wings` 1–8; `groups` 0 (off) or 2–32; `blocks` 1–32; `mode`.
- **FX-07** Member positions. Members are numbered *i* = 0…*n*−1 in their order (the selection
  order when applied, PRG-57's list: members without the entry's attributes take no position).
  Each member gets a position *k* of *N*, in this order:
  1. **Wings** *W*: parts of *L* = ⌈*n*/*W*⌉ members; in every odd part (2nd, 4th, …) the order
     runs backwards: *j* = *i* mod *L*, mirrored to *L* − 1 − *j* in odd parts; *N* = *L*.
  2. **Groups** *G* (when on): *j* = *j* mod *G*, *N* = min(*G*, *N*) — every *G*-th member
     moves together ("1 in 3" is groups 3).
  3. **Blocks** *B*: *k* = ⌊*j*/*B*⌋, *N* = ⌈*N*/*B*⌉ — *B* neighbours move as one.

  The member's phase offset is φ = (`spread` / 360°) · *k*/*N*. Spread 0 → all together;
  360° → one wave over the members, a chase of 4 is offsets 0, ¼, ½, ¾; 720° → two waves. 8
  pars, spread 360°, wings 2 → positions 0 1 2 3 3 2 1 0: the chase runs in from both ends to
  the middle; with `Backward` from the middle out.
  Positions are computed when the entry or its members change, never per tick (FX-13).
- **FX-08** Phase per member: θᵢ = frac(θ_dir + `phase`/360° − φᵢ), where θ_dir follows
  `direction`: `Forward` θ_total; `Backward` −θ_total (a circle turns the other way, a chase runs
  from the last member to the first); `Bounce` u = θ_total mod 2, θ_dir = *u* when *u* < 1, else
  2 − *u* (the wave travels there and back). Forward chase over 201–204: 201 leads, 204 follows
  ¾ cycle later.

### Clock and tempo
- **FX-09** An entry's clock starts at 0 when the entry starts to run: when it is made in the
  programmer, when its cue fires (after the cue's `delay`), when its palette is applied. Every
  tick it advances by Δt · *rate*, *rate* = effective BPM / (60 · `beats`), with Δt from tick
  timestamps (ENG-17 as amended by ADR-011): a speed change never jumps the phase. Effective BPM:
  `Free` = `bpm`; `Bpm n` = BPM master *n*'s tempo with its multiplier (CUE-35); `Rate n` = `bpm`
  · Rate master *n*'s rate (CUE-36); a missing master runs as `Free` and is kept (as CUE-29).
- **FX-10** Tap on the current entry (`effect.tap`) times taps as CUE-33 (moment of receipt, 2 s
  series, mean of the last three intervals, 1–600 BPM, 0.1 BPM steps): on `Free` it sets the
  entry's `bpm`, on `Bpm n` it taps that master. Every tap from the second on beat-syncs: the
  clock is set to the nearest whole beat, θ_total ← round(θ_total · `beats`) / `beats`. A tap
  on a BPM master (CUE-33) beat-syncs every running entry that follows it the same way.
- **FX-11** Restart (`effect.restart`) sets the clocks of all programmer entries to 0 at once,
  so they run in step (Titan *Restart Shapes*); entries of playbacks are not touched.
- **FX-12** Determinism: same scenario (patch, timestamped actions, tick timestamps) → the same
  bytes at every tick on the same machine (ENG-11), with effects running; no system clock, no
  unseeded randomness.
- **FX-13** Per tick, effects allocate nothing (ENG-13): positions, offsets and resolved
  parameters live in the entry and are updated when the entry is edited; the tick evaluates one
  shape function per member and attribute. 500 members × 2 attributes stay within one tick's
  budget at 40 Hz (benchmark test, `#[ignore]` as ENG-13).

### The programmer
- **FX-14** `effect.apply {look}` (or `{shape, attribute?}`, or `{palette}`) makes a programmer
  entry from the targets (PRG definitions) that have the shape's attribute(s), in target order,
  with the look's defaults (FX-30) or the palette's parameters; targets without them are skipped,
  none left → refused with a warning ("No selected fixture has Pan/Tilt"). The new entry becomes
  the current entry and runs at once. Each target that already has a programmer effect on one of
  those attributes leaves that entry first (an entry with no members left goes away): one
  programmer effect per fixture and attribute.
- **FX-15** The entry's attributes are touched in the programmer (PRG-15 `contents()` lists them
  with the entry), at programmer priority (ENG-04). An attribute with an effect and no value of
  its own keeps showing the base from the playbacks underneath (ENG-15): a programmer circle on
  spots whose position comes from a running cue circles that position.
- **FX-16** `effect.set {param, value}` changes a parameter of the current entry from the next
  tick; positions are recomputed when spread, wings, groups, blocks or direction change. Changing
  a parameter of an entry that points at an effect palette copies the palette's parameters into
  the entry first (it becomes its own, as PRG-22 for values).
- **FX-17** The effect wheels: while the Shapes workspace is open or `effect.wheels {on: true}`
  is set, the wheel layout (PRG-17) is Size, Speed (BPM), Spread, Phase on page 1 and Width /
  Crossfade, Top, Beats, Wings on page 2, acting on the current entry; on the screen, the
  BCF2000 and OSC alike (`wheels` in engine state). A wheel turn is a relative step (PRG-21 step
  sizes: size 1 %, BPM 1, spread 15°, phase 15° per `Coarse` step).
- **FX-18** `effect.select {entry}` makes another programmer entry current. Selecting fixtures
  that are exactly the members of one entry makes it current too.
- **FX-19** `effect.stop` removes the targets from every programmer entry (all attributes), or
  only on the attributes of the active attribute group when one is chosen; `Off` on an
  attribute (PRG Off) removes the effect on it too. `Clear` empties all entries; Clear with a time
  *t* (ENG-10) fades every entry's size to 0 over *t* and then removes it, so an effect never
  stops with a jump from Clear.
- **FX-20** Undo (`undo.md`): apply, stop, select, restart and every `effect.set` are edit steps;
  wheel turns and repeated `effect.set` of the same parameter on the same entry coalesce (UNDO-05).
  The clock is live state: undo does not rewind it.

### Recording and palettes
- **FX-21** Recording a cue (PBK-09..13, Record By, masks) stores the programmer entries whose
  attributes are in the mask, members cut to the recorded fixtures; an entry with no members or
  attributes left is not stored. A palette-pointing entry stays a pointer (PBK-11). Update and
  Merge replace a cue's effect per member and attribute as they replace values.
- **FX-22** **Effect palette.** Recording into a palette stores, next to the values as today, the
  programmer entries in the palette's groups **without members**: shape, attributes, parameters
  (and `palette` pointers resolved to parameters). Applying such a palette to the targets
  applies its values (PAL-07) and makes an entry per stored entry with the targets as members
  (FX-14). "Circle around the pulpit" = a position palette with a circle in it. A palette with
  only entries and no values is shown in the pool with the effect's look icon.
- **FX-23** An entry pointing at an effect palette reads the palette's parameters every tick
  (PAL-11): editing the palette changes every cue that points at it, from the next tick. A
  deleted palette leaves its last parameters in the entry (as a value reference's fallback).

### Playback
- **FX-24** A cue's entries start when the cue fires (FX-09) and stop with their source (kill,
  release, the next cue that does not track them). They act through ENG-15 at the playback's
  priority.
- **FX-25** The playback's *m* (PBK-15..17, CUE-31) scales an entry: for an absolute effect on an
  `Htp` attribute the effect's value is scaled like any intensity (ENG-05); for every other effect
  the size is multiplied by *m*. So the fader shrinks a circle, a rising *m* fades an effect in
  over `fade_in`, and a release time fades it out; with *m* = 0 a relative effect leaves the base.
- **FX-26** On a cue list going from cue A to cue B, per member and attribute: if B holds (or
  tracks, CUE-50) an entry with the same shape, the clock continues and size, top and BPM fade
  from A's to B's values over the attribute's fade time (CUE-14), the rest changes at once; if
  not, the effect's output crossfades from A's effect to B's (or to the plain base) over that
  fade time. A cue list never jumps an effect on a timed Go.
- **FX-27** Tracking (CUE-50) tracks entries per member and attribute as it tracks values; a
  blocking cue stops a tracked effect only where it holds a value or an entry of its own.

### Safety
- **FX-28** No input combination makes an effect stop the tick: a malformed entry in a show is
  refused at load with a warning and the rest loads; a bad value at run time holds the last good
  value for that attribute and logs once (show-proof rules). No `unwrap`/indexing that can panic.
- **FX-29** Show schema 8 (SHW-04): cues, palettes and the saved programmer gain an optional
  `effects` list; schema 7 shows load unchanged (no entries); a show without effects saves
  byte-identical apart from the version number.

### The gallery of looks
- **FX-30** The built-in looks and their defaults (size in attribute units, BPM at `beats` 1):

  | Look | Group | Shape (attribute) | Mode | Size | BPM | Spread | Other |
  | --- | --- | --- | --- | --- | --- | --- | --- |
  | Dimmer chase | I | Square (Dimmer) | Abs | 1.0 | 120 | 360° | width 100 %/*N* |
  | Dimmer wave | I | Sine (Dimmer) | Abs | 1.0 | 60 | 360° | |
  | Breathe | I | Sine (Dimmer) | Abs | 0.5 | 20 | 0° | |
  | Sparkle | I | Random (Dimmer) | Abs | 1.0 | 240 | 360° | |
  | Comet | I | RampDown (Dimmer) | Abs | 1.0 | 90 | 360° | |
  | Circle | P | Circle | Rel | 0.2 | 20 | 0° | |
  | Eight | P | Eight | Rel | 0.25 | 15 | 0° | |
  | Fan sweep | P | LinePan | Rel | 0.3 | 15 | 180° | wings 2 |
  | Tilt wave | P | LineTilt | Rel | 0.2 | 20 | 360° | |
  | Rainbow | C | Rainbow | Abs | 1.0 | 6 | 0° | |
  | Rainbow wave | C | Rainbow | Abs | 1.0 | 6 | 360° | |
  | Colour chase | C | ColourMix | Abs | 1.0 | 60 | 360° | colours: base + its complement, crossfade 0 % |
  | Colour fade | C | ColourMix | Abs | 1.0 | 10 | 0° | as above, crossfade 100 % |
  | Zoom pulse | B | Sine (Zoom) | Rel | 0.3 | 30 | 0° | |

  Defaults that need the selection (*N*, base, complement) are taken once, when the look is
  applied. The table is code (ADR-011), so a later look is one row and one test.
- **FX-31** A look is offered only when some target has its attribute(s); `effect.apply {look}`
  on targets that have none is refused (FX-14).

### IPC (extends `ipc.md`; protocol `v` unchanged: new keys and commands only, IPC-02)
- **FX-32** The snapshot carries `effect_catalogue` once: the looks (id, name, group, shape,
  attribute kind) and the shapes by kind, so the app draws the gallery without lighting logic.
- **FX-33** Engine state carries `effects`: the programmer entries in order, each `{id, look,
  shape, attributes, members, positions, params, palette, current}`, with `positions` the *k*/*N*
  of FX-07 per member (for the spread strip, FX-42), and `wheels` (FX-17). A delta follows every
  change; the clock is never sent (the stage, and the fixture tiles, show it).
- **FX-34** Commands: `effect.apply`, `effect.set`, `effect.select`, `effect.stop`,
  `effect.restart`, `effect.tap`, `effect.wheels`; refusals come back as warnings (IPC). Each is
  in UNDO-04's table with the class of FX-20.
- **FX-35** The control layer's action catalogue (CTL) gains the effect wheels (through FX-17,
  nothing new on the device side), `effect.tap`, `effect.restart` and `effect.stop`, so a BCF2000
  button or a TouchOSC key can tap an effect's tempo.

### App: the Shapes workspace (amends `ui.md` UI-07, UI-08, UI-18)
- **FX-36** The rail's Shapes entry is enabled and opens a fifth workspace (F5) with four windows:
  **Looks** (the gallery), **Effect** (the editor of the current entry), **Fixtures** (live, so the
  operator sees the effect), and **Effects** (the programmer's entries, with a tab for the effect
  palettes in the pool). The main context's *Shapes & Effects* softkey opens it.
- **FX-37** Looks: tiles grouped Intensity / Position / Colour / Beam in IPCGBES order with the
  group hue, each with a small still drawing of its shape; looks no target can run are dimmed
  (FX-31). One tap applies the look to the targets (FX-14). A *Looks | Shapes* switch (as
  *Mix | Gels*, UI-32) shows the raw shapes per kind for operators who want them.
- **FX-38** Effect editor, for the current entry, built from a few big controls, top to bottom:
  **Size**, **Speed** (BPM field, Tap, the source chips `Free`, `BPM 1–8`, `Rate 1–4`, and
  `beats`), **Spread** (degrees, with chips 0°, 90°, 180°, 360° and *Every n*), then a fold-out
  *More* with Direction (→ ← ↔), Phase, Wings, Groups, Blocks, Mode (Rel/Abs), Top, Width or
  Crossfade, and for `ColourMix` the colour list (add from the colour picker or a Colours palette,
  reorder, remove). No control the shape does not use is shown.
- **FX-39** Effects window: one row per programmer entry (look icon, look name, attribute group
  hue, member count, BPM); a tap makes it current (FX-18); a swipe or the *Stop* button stops it
  (FX-19). Softkeys in the Shapes workspace: Stop FX, Restart, Tap, and Record as in the main
  context (an effect palette is a palette with entries, FX-22).
- **FX-40** The encoders (IPC-16 wheels on screen, BCF2000, OSC) show the effect wheels of FX-17
  while the Shapes workspace is open, with their names and values.
- **FX-41** The fixture tiles show output values with the effect in (as today), so the operator
  sees the chase run on the tiles; the app animates nothing itself.
- **FX-42** Under the Spread control a strip shows one cycle of the shape with a dot per member at
  its position from `positions` (FX-33), so spread, wings, groups and blocks are visible before the
  operator looks up at the rig. A `HueShift` on a base without saturation says so under Size
  ("The base colour is white: Hue shift shows nothing").
- **FX-43** The manual (`subprojects/website/manual/src/`) gains a *Shapes and effects* page in the
  same PR as the app (WEB-33).

### Run
- **FX-44** A home run on Art-Net (`tests/manual/effects.md`, Claude listens to the frames): a
  dimmer chase, a circle around a position palette, a rainbow wave on the pars, speed on a BPM
  master with Tap, a cue list with a timed Go from a circle to an eight, record an effect palette
  and recall it on another selection; the owner judges by eye on the tiles and in the run file.

## Scenario test format
Scenario files `tests/scenarios/FX-nn-*.json` (format of `engine-merge.md`) gain an `effect`
action (`apply`, `set`, `stop`, `restart`, `tap` with the IPC arguments) and sample frames at
given tick timestamps. Shape functions and positions (FX-01..08) are unit tests in
`cmdmx-core::effect` named `fx_nn_…`.

## Out of scope (later ideas)
- User key-frame shapes (Titan Key Frame Shapes, grandMA3 phaser steps) and user shape files.
- Several effects adding up on one attribute (ENG-15), Titan's Overlay/LTP shape behaviour choice.
- Size masters, *Speed on fader*, effects on chase steps.
- Effect direction from a 2D layout (needs IDEA-25), random member order (IDEA-15), the pixel
  mapper over cells (phase 2, own spec).
- Ableton Link / MIDI clock as tempo source (IDEA-16); it plugs in as a BPM master source.
- Gobo / prism / shutter "effects": these are fixture macros on FX-group channels (ordinary values).
- Cycle counts (Titan *Cycles*) and one-shot effects.

## Owner decisions (approved 2026-10-29, as proposed; the owner: not like ONYX, which is not user-friendly)
1. Relative by default; absolute for dimmer waves and colour shapes (FX-05).
2. One effect per attribute, no layering (ENG-15, ADR-011).
3. The playback fader scales effect size (FX-25), as Titan's default.
4. Key-frame shapes later; Colour mix over 2–8 colours covers the colour chase now (FX-03).
