# Getting started

## What you need

- A Mac with macOS 14 or later, Apple Silicon or Intel.
- For lights: a network connection to an Art-Net or sACN node, or a USB DMX interface that speaks
  the ENTTEC Pro protocol (for example the Eurolite USB-DMX512 PRO MK2).

## Install

1. Download `CMDMX-<version>.dmg` from the
   [latest release](https://github.com/Aottens/CMDMX/releases/latest) on GitHub.
2. Open the DMG, drag **CMDMX** onto **Applications**, then eject the DMG.
3. Open CMDMX from Applications. macOS asks once to confirm an app downloaded from the internet:
   click **Open**.

The release page for v0.1.0 says "ColourmotionDMX" in its install text. That was the name before <!-- brand: old-name -->
the rename; the app is now called CMDMX.

### Questions macOS may ask

- **Local Network:** allow. It is needed for Art-Net and sACN.
- **Incoming connections** (firewall): allow. It is needed for OSC and TouchOSC.

## First launch

The app opens full screen. You will see a status bar at the top, a rail of workspaces on the left,
a grid of windows, and the playback faders at the bottom. The first time, CMDMX opens a show called
*Demo*, a built-in rig for trying things out. Do not run the Demo show on a real rig: its fixtures
sit at addresses where real lamps may be connected.

Your shows are saved as files in `~/Documents/CMDMX/Shows/`. They are saved automatically a few
seconds after each change.

## The engine and the app

You do not start the engine yourself. The app starts it when it opens and stops it when you quit.

- If the app crashes or hangs, the engine keeps sending DMX. Open the app again and it reconnects.
- Pressing Command-Q asks first and reminds you that DMX keeps running in the engine until the app
  is really closed.
- When you quit, the engine stops and lets go of the USB interface, so another program can use it.
  Quitting also saves your last changes to the show.

The status bar shows whether the engine is connected and, per output, whether it is working.

## Connect output

Open **Output** in the rail. It lists the output nodes of the show, one row each, with a coloured
dot for their state.

- **Art-Net and sACN** are added by themselves for a universe as soon as you patch the first
  fixture on it. You can edit, switch off, add or remove nodes on this screen.
- **USB DMX** is a *Serial* row. Its dropdown lists the serial ports it can find. Choose your
  interface and the dot turns green. The choice is remembered when you restart.

CMDMX talks to interfaces that use the ENTTEC Pro protocol. The Eurolite USB-DMX512 PRO MK2 is the
one it has been tested with. It does not announce itself clearly, so CMDMX cannot recognise it by
itself: choose its port yourself in the Serial row (it appears as a `/dev/cu.usbserial-...` entry).

## Your first show in ten minutes

This walkthrough uses a few steps that exist in the app today. Use a new show, or the Demo show
with nothing connected.

1. **Patch a fixture.** Click **Patch** in the rail. In the *Add* panel, pick a manufacturer and
   fixture, choose a mode, set the count, universe and address, and click **Add**. A clash with an
   existing fixture is refused.
2. **Select it.** Go back to a workspace with fixtures (for example the Colour workspace) and tap
   the fixture. It turns blue.
3. **Set a look.** Open the **I** group in the attribute bar and turn the Dimmer wheel up, or type
   a command such as `1 @ 50` on the command line. Pick a colour with the colour picker or a
   colour palette. Values you set sit in the *programmer* and show in red.
4. **Record it.** Press **Record**, then tap an empty handle in the playback area. The look is now
   a playback.
5. **Play it back.** Clear the programmer with **Clear** (twice empties it), then raise the
   playback's fader. The look comes back from the saved playback.

![The Colour workspace: fixtures, the colour picker, colour palettes and the Programmer window](images/colour-workspace.png)

*The Colour workspace: selected fixtures, the colour picker, palettes and the Programmer window.*

You can **Undo** (Command-Z) any edit, but not live playing such as pressing Go or moving faders.

More on each step: [Patching](patching.md), [Programming](programming.md),
[Palettes](palettes.md) and [Playbacks and cues](playbacks.md). For the ideas behind them, read
[Concepts](concepts.md).
