# Research: selection orders and the FX engine (2026-10-29, P2-02)

Input for IDEA-15 (selection orders, `programmer.md` PRG-71..84) and IDEA-33 (a richer FX engine).
Sources are the vendors' online manuals, read 2026-10-29; what a manual does not say is marked.

## Selection order and sub-selection

### Avolites Titan (behavioural reference)
Manual: *Controlling Fixtures* (https://manual.avolites.com/docs/controlling-fixtures), *Fixture
Groups* (…/controlling-fixtures/fixture-groups), *Cue Timing* (…/docs/cues/cue-timing),
*Shape Generator* (…/docs/effects/shape-generator).
- **Order is the order of selecting.** Fix +1 / Fix −1 (Next/Prev) step one fixture at a time in
  that order. Fan, Shapes and Fixture Overlap all read the same order.
- **Patterns:** All opens pattern softkeys (Odd/Even and others; the tips page names "odd, even,
  random"); Fix ±1 steps between the parts; All twice ends it. Custom patterns are typed
  (`2 In 6 Enter`). Avo + Fix ±1 jumps in blocks. Holding a group key gives the same options.
- **Groups store the order** they were recorded in, and a 2D layout whose X order *is* the fixture
  order (Fixture Order: tap fixtures in sequence, Auto Increment off gives several fixtures the same
  number). Holding a group key offers **Random Order** (temporary).
- **Direction** sets the order from the group layout (left to right, inside out, outside in, also
  in 2D); it returns to None on Clear unless *Maintain Direction* is set.
- **Cues keep the order:** "The order of the fixtures is set by the order you selected them", the
  group's stored order when a group was used. A cue's *Fixture Order* can be edited afterwards in
  Edit Times, again with shared step numbers (fixtures with the same number move together).
- **Shapes** run in the order the fixtures were selected when the shape was made (Fixture Order in
  Shape Edit changes it); cells run as one block, linear, or by the group layout.
- Not found in the v19 manual pages read: the exact list of pattern softkeys, a Reverse key, a
  Mirror order. Titan v11 notes mention "mirror selection / selection direction" options.

### grandMA3 (MAtricks)
Manual: https://help.malighting.com/grandMA3/2.5/HTML/matricks.html and its sub-pages.
- MAtricks divides the selection into sub-selections on a **selection grid** (X, Y, Z axes; the
  selection order fills X). Properties: **Blocks** (neighbours act as one), **Groups** (fixtures
  dealt out in turn: every *n*-th together), **Wings** (the selection mirrored from opposite ends),
  **Width** (how many positions one step selects), **Shuffle** (a seed 0–32767 per axis; the same
  seed gives the same order), **Shift** (moves the selection over the grid), Invert, and layers
  Fade / Delay / Speed / Phase with From/To values (the spread of timing and phase over the grid).
- Next / Prev step through the parts; **Set** shows the whole selection and switches MAtricks off,
  again switches it back on; settings stay until changed.
- Stored in the **MAtricks pool**, and used by recipes in cues; Shuffle is stored with the rest.
- Transform *Mirror* mirrors values over blocks, groups and wings.

### Obsidian Onyx (grouping tools)
Manual: *Using the Grouping Tools* (support.obsidiancontrol.com, Onyx manual).
- Modes with a **Mask Value**: **Every** *n* (every *n*-th, *n* sets to step through), **Block** *n*
  (consecutive segments of *n*), **Divide** *n* (halves, thirds, …), **Mirror** *n* (the *n*
  outermost, moving in with Next), **Group** (one group at a time). Next / Last step.
- Actions on the order: **Random**, **Reverse** (combinable with Random), **Sort** (by fixture
  number), **Revert** (back to the order of selecting); also Invert (swap selected/deselected).
- A **Fan/FX** toggle: off → changes act on the active subset only; on → Fan and FX run across the
  subsets (odd against even).
- The manual does not say whether the order is stored with groups or cues.

### What CMDMX takes (PRG-71..84)
- Two separate things: **order** (a permutation: Reverse, Shuffle, Sort, Revert; Onyx and Titan) and
  a **grid** that gives items shared positions (wings, groups, blocks, flip; MA3), computed by the
  one position function the FX generator already uses (FX-07). Onyx's Every = groups, Block =
  blocks, Divide = blocks of ⌈n/d⌉, Mirror = wings 2; Titan's custom `2 In 6` = groups 6, blocks 2.
- Every consumer reads the same positions: Next/Prev, Fan, new effects, palette overlap, cue overlap.
- Onyx's Fan/FX toggle is All: with a stage selected things act on that stage, after All they run
  across the stages.
- Titan's stored order: groups keep order (and, new, the grid); cues keep each item's position, so
  overlap replays the recorded order; a Fixture Order editor with shared numbers.
- Later: MA3's Width, Shift, X/Y/Z axes and From/To layers; Titan's 2D Direction (needs the rig
  overview, IDEA-25); a MAtricks pool; Select If.

## FX engines: what others have that CMDMX lacks (IDEA-33)
CMDMX now: 6 waves, 6 paths, 3 colour shapes, 14 looks; size, speed (Free, BPM, Rate), spread,
phase, direction, wings, groups, blocks; relative/absolute; one effect per attribute; effects in
cues and palettes (`effects.md` FX-01..44).
- **Titan:** many built-in shapes per attribute group (dimmer, position such as ballyhoo and
  spiral, colour, beam), Key Frame Shapes (user steps, each step a value or palette, with fade or
  snap), Cycles (a shape stops after *n* cycles, part cycles allowed), Overlay/LTP shape behaviour
  (layering with cues), Mask FX, Offset between shapes, Beats with *Match to Spread*, cells as
  block/linear/layout, the **Pixel Mapper** (images and effects over a layout of cells), and shape
  size and speed on playback faders.
- **grandMA3:** phasers: every effect is steps of values or presets with per-step transitions and
  acceleration, absolute or relative, in any attribute; MAtricks layers spread phase, fade and delay
  From/To over the grid; speed masters and **size/rate masters** on executors; bitmap fixtures and
  generators for pixel/matrix effects; several phasers layering through presets.
- **Onyx:** an FX engine with step FX (a chase of presets inside one FX), many waveforms, FX on
  groups with the grouping tools' Fan/FX toggle, a **Matrix FX** / pixel engine over 2D layouts,
  and FX speed/size masters on playbacks.
- Gaps for CMDMX, in the order proposed in IDEA-33: more forms and looks; Speed/Size masters and
  speed on a fader; cycles and one-shots; layered effects on one attribute; step (key-frame) FX;
  per-member From/To spreads; pixel/matrix effects over a 2D layout.
