# Shapes and effects

An effect makes fixtures move by themselves: a dimmer chase over the pars, a circle on the
moving heads, a rainbow running across a row. In CMDMX you start from a result: select the
fixtures, tap a **look**, and it runs at once around what the fixtures are already doing. Three big
controls, **Size**, **Speed** and **Spread**, then shape it. Effects record into cues and palettes
like any other value.

## The Shapes workspace

Open it with **Shapes** on the rail, **F5** (or ⌘5), or the **Shapes & Effects** softkey in the
Main softkeys. It has four windows:

- **Looks**, the gallery you start effects from;
- **Effect**, the editor of the current effect;
- **Fixtures**, so you see the effect run on the fixture tiles;
- **Effects**, the effects in the programmer, with a tab for the effect palettes.

While the workspace is open, the encoders are the **effect wheels** (see below).

## Starting an effect: Looks

1. Select fixtures, for example the washes.
2. Tap a look in the **Looks** window.

The effect runs straight away on the selected fixtures, in the order you selected them. The looks
are grouped as Intensity, Position, Colour and Beam, each with a small drawing of its shape:

| Look | What it does |
| --- | --- |
| Dimmer chase | one fixture after the other at full, the rest dark |
| Dimmer wave | a soft wave of brightness across the fixtures |
| Breathe | all fixtures slowly up and down together |
| Sparkle | random fixtures flash |
| Comet | a flash that fades out, running across the fixtures |
| Circle, Eight | the beams draw a circle or a figure eight around their position |
| Fan sweep | the beams sweep out from the middle and back |
| Tilt wave | a wave of tilt across the fixtures |
| Rainbow | all fixtures run through the colours together |
| Rainbow wave | the rainbow runs across the fixtures |
| Colour chase, Colour fade | steps or fades between the current colour and its opposite |
| Zoom pulse | the zoom opens and closes |

A **dimmed** look does not fit the selection: none of the fixtures has the attribute it moves (a
circle needs Pan and Tilt). Tapping it does nothing but show a message.

The **Looks | Shapes** switch at the top shows the plain shapes instead (sine, triangle, ramps,
square, random; circle, eight, square, triangle and lines for position; rainbow, hue shift and
colour mix). A wave started from there runs on the dimmer.

Position and colour effects move *around* the current value: a circle on spots that a cue points
at the pulpit circles the pulpit. Dimmer waves and the colour looks set the value themselves.

**One effect per attribute:** a new effect on fixtures that already have one on the same
attribute replaces it for those fixtures.

## Shaping it: the Effect window

The Effect window edits the **current** effect: the one you started last, or the one you tapped in
the Effects window. From top to bottom:

- **Size**: how big the effect is. For a dimmer chase it is how far the dimmer goes; for a circle,
  how wide the circle is.
- **Speed**: the tempo in BPM (cycles per minute), a **Tap** button to tap it in, and where the
  tempo comes from: **Free** (its own BPM), **BPM 1** to **BPM 8** (follow a BPM master) or
  **Rate 1** to **Rate 4** (its own BPM times a rate master). The **beats** chips make one cycle
  last ¼ to 16 beats.
- **Spread**: how the effect is spread over the fixtures. 0° means all together; 360° is one wave
  over all fixtures (a chase); 720° two waves. **Every n** makes every n-th fixture move together
  ("1 in 3").

Under Spread a **strip** shows one cycle of the shape with a dot for every fixture where it sits
in the cycle, so you see what spread, wings and groups do before you look up at the rig.

**More** folds out the rest:

- **Direction**: → forward, ← backward, ↔ there and back.
- **Phase**: shifts the whole effect in its cycle.
- **Wings**: splits the fixtures in mirrored parts, so a chase runs in from both ends.
- **Groups** and **Blocks**: every n-th fixture together, or n neighbours as one.
- **Mode**: **Rel** moves around the current value, **Abs** sets the value itself; with Abs,
  **Top** is the highest value.
- **Width** (square waves only): how long each fixture is on.
- **Crossfade** and the **colour list** (colour mix only): the colours it steps through, and how
  much it fades between them. Add the current colour, move colours left or right, or remove them.

Only the controls the shape uses are shown. A hue shift on a white base shows nothing, and the
window says so under Size.

## The effect wheels

While the Shapes workspace is open, the encoders on the screen, on the BCF2000 and in TouchOSC act
on the current effect:

| Page | Wheels |
| --- | --- |
| 1 | Size, Speed, Spread, Phase |
| 2 | Width (Crossfade for a colour mix), Top, Beats, Wings |

The wheel shows the value in its own unit: % for size, BPM for speed, degrees for spread and phase.

## The Effects window and the softkeys

The **Effects** window lists every effect in the programmer: its look, the attribute group colour,
how many fixtures and its tempo. Tap a row to make it the current effect. The **Palettes** tab
shows the effect palettes (see below); tap one to run it on the selected fixtures.

In the Shapes workspace the softkeys are:

- **Stop FX**: stops the effects on the selected fixtures.
- **Restart**: starts all programmer effects from the beginning of their cycle at once, so they run
  in step.
- **Tap**: taps the tempo of the current effect. On a BPM master it taps the master.
- **Record**: as in the Main softkeys.

**Clear** empties the programmer, effects included. With a Clear time the effects shrink to
nothing over that time instead of stopping with a jump. **Undo** takes back every effect change.

## Effects in cues

Recording a cue stores the effects in the programmer together with the values (only for the
recorded fixtures and the attribute groups in the mask). When the cue runs, the effect starts with
it. On a cue list, a timed Go from a circle to another circle carries on smoothly with the new
size and speed; from one effect to another, or to no effect, it crossfades. The playback fader
scales the effect: pulling it down makes a circle smaller and a chase dimmer.

## Effect palettes

Recording a palette while an effect runs in its attribute groups stores the effect too, without
the fixtures. Applying the palette later runs the effect on whatever is selected. "Circle around
the pulpit" is a position palette with a circle in it. A palette with only an effect shows the
look's drawing in the pool. A cue that uses an effect palette follows it: change the palette and
every cue that uses it changes.
