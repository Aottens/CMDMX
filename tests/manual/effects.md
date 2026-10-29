# Effects checklist (FX-36..44) — template

The FX generator's home run (FXB-06, FX-44) and the Shapes workspace (FX-36..42), checked by hand
because there is no Swift test target (CLAUDE.md). Copy to `tests/manual/runs/<date>-FXB-06.md`
and tick it there. Claude drives the boring steps and listens to the Art-Net frames; the owner
judges by eye on the fixture tiles and says what feels wrong. Each step message shows done/total.

Setup: two terminals and a fresh shows folder so the real one stays untouched.
- `rm -rf /tmp/cmdmx-fx`
- Daemon: `cd subprojects/engine && cargo run -p cmdmx-daemon -- --no-sacn --show-dir /tmp/cmdmx-fx`.
- App: `cd subprojects/app && swift build && scripts/bundle-app.sh && open .build/CMDMX.app`.
- Art-Net listener on universe 1 (demo rig: washes 101–104 with Pan/Tilt/Zoom, RGBW pars 201–208,
  RGB+Lime pars 211–216, RGB pars 221–226, front spots 301–306). Groups: 2 washes, 3 pars,
  4 spots.

## The workspace (FX-36, 39, 40)
- [ ] The rail's **Shapes** entry is live with F5; F5, ⌘5 and the Main softkey **Shapes &
      Effects** all open it. It shows Looks, Effect, Fixtures and Effects.
- [ ] In the Shapes workspace the softkeys read Stop FX, Restart, Tap, Record (and Undo).
- [ ] With no effect, the Effect window says to tap a look; the encoders show the normal wheels.

## Looks (FX-37, 31)
- [ ] Nothing selected: every look is dimmed. Select group 3 (pars): the colour and dimmer looks
      light, Circle, Eight, Fan sweep, Tilt wave and Zoom pulse stay dimmed.
- [ ] Tiles are grouped Intensity / Position / Colour / Beam with their group hue and a small
      drawing of the shape. **Looks | Shapes** shows the raw shapes per kind and back.

## A dimmer chase (FX-44 a, FX-38, 41, 42)
- [ ] Select group 3, tap **Dimmer chase**: on the tiles and in the frames the pars run one after
      the other at 120 BPM (Claude: each of the 20 pars at full about 1/20 of a 0.5 s cycle in turn). The tiles show no
      output yet (IDEA-10): judge on a stage view of the frames.
- [ ] The Effect window shows Size, Speed (120, Free, beats 1), Spread 360° and the strip with a
      dot per par spread over one cycle. Spread chip 0°: all dots together, the pars flash
      together; 720°: two runs. *Every n* 3: every third par together.
- [ ] More: Direction ← runs the chase the other way; Wings 2 runs it in from both ends (the
      strip's dots mirror). Width is shown (square wave), Crossfade is not.
- [ ] In Shapes, tap I: the encoders show Dimmer; tap FX (or a look, or an Effects row): the
      effect wheels are back (FX-17, FX-40 amended).
- [ ] The encoders show Size, Speed, Spread, Phase with "100 %", "120 BPM", "360°", "0°"; turning
      Speed changes the tempo without a jump in the frames.

## A circle around a position palette (FX-44 b, FX-22)
- [ ] Select group 2, set Pan/Tilt somewhere off centre and record Positions cell 1. Clear, select
      group 2, apply the palette, then tap **Circle**: the washes circle around that position
      (Claude: Pan and Tilt run a sine a quarter cycle apart around the palette's values).
- [ ] Size 0 stops the movement on the palette's position; back to 20 %.

## A rainbow wave on the pars (FX-44 c)
- [ ] Select group 3, tap **Rainbow wave**: the colours run across the pars (Claude: hue offset
      per par, 6 BPM). The Effects window lists Dimmer chase, Circle and Rainbow wave with their
      member counts and BPM; tapping a row makes it current in the Effect window.
- [ ] Shapes ▸ Hue shift on the pars with no colour set: under Size, "The base colour is white:
      Hue shift shows nothing".

## Speed on a BPM master with Tap (FX-44 d, FX-09, 10)
- [ ] Make a BPM master on a handle (Record ▸ BPM Master). Current effect the dimmer chase:
      source chip **BPM 1**. Tap the master's tempo four times at about 100 BPM: the chase follows
      and lands on the beat (Claude: cycle length about 0.6 s).
- [ ] The **Tap** softkey on a Free effect sets its BPM; **Restart** starts all effects in step.

## A timed Go from a circle to an eight (FX-44 e, FX-26)
- [ ] Clear. Group 2, Pan/Tilt 50 % (until FXB-08 the Demo Wash rests at 0), Circle; record cue 1 on a cue list. Clear, group 2 **Eight**; record cue 2.
      Give cue 2 a 3 s fade. Go cue 1, then Go cue 2: the movement crossfades over 3 s without a
      jump (Claude: no Pan/Tilt step between frames over the fade).
- [ ] Pull the cue list's fader to half: the eight gets smaller.

## An effect palette on another selection (FX-44 f, FX-22, 39)
- [ ] Group 3 Rainbow wave; Record, tap an empty Colours cell: the palette shows the rainbow's
      drawing. The Effects window's **Palettes** tab lists it.
- [ ] Clear, select 211–216 only, tap the palette in the Palettes tab: the Effect window says
      "Intensity is 0: this effect shows nothing"; `@100`: the rainbow runs on those six pars only.

## Stop, Clear, Undo (FX-19, 20)
- [ ] **Stop FX** on the selected fixtures stops their effect; Undo brings it back.
- [ ] Clear with a time (`3` then Clear once FXB-07 is in; until then `programmer.clear {time: 3}`
      over IPC): the effects shrink to nothing over 3 s, no jump.

## Open points
