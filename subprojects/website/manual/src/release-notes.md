# Release notes

## 0.2.4

Published 29 October 2026 as "CMDMX 0.2.4".
[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/v0.2.4)

The FX generator: fixtures that move by themselves (see [Shapes and effects](effects.md)).

- A **Shapes** workspace (F5): select fixtures, tap a look such as a dimmer chase, a circle or a
  rainbow wave, and it runs at once around what the fixtures are already doing. **Size**, **Speed**
  and **Spread** shape it, on the screen or on the encoders.
- An effect's tempo can come from its own BPM, Tap, or a BPM or Rate master.
- Effects record into cues and into palettes. A timed Go between two circles carries on smoothly;
  the playback fader makes the effect smaller. An effect palette ("circle around the pulpit")
  runs on whatever is selected.
- A number then **Clear** (`3` Clear) clears the programmer over that time; effects shrink to
  nothing instead of stopping with a jump.
- Record ▸ **BPM Master** and **Rate Master** put the master on the first empty handle of the
  page, or say which page has room.
- The washes of the Demo show start pointing at the middle, so a circle runs around the centre.
- A cue that holds only an effect lists its attribute group in the Cue list window.

## 0.2.3

Published 29 October 2026 as "CMDMX 0.2.3".
[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/v0.2.3)

- Groundwork for the FX generator: the engine runs effects in the programmer, cues and palettes,
  and the app can talk to it about them. There are no controls for effects in the app yet; they
  come in 0.2.4.

## 0.2.2

Published 29 October 2026 as "CMDMX 0.2.2".
[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/v0.2.2)

- Gel numbers: the colour picker has a **Gels** side with the LEE gel book. Search by number or
  name and tap a swatch, or type `Lee 201` on the command line (see
  [The gel book](programming.md#the-gel-book)).
- The fixture types of the Demo show can be opened in the profile editor (Patch ›
  **Edit profile…**), and a change there reaches every fixture of that type.

## 0.2.1

Published 28 October 2026 as "CMDMX 0.2.1".
[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/v0.2.1)

- Extra emitters (White, Warm White, Amber, Lime) can be mixed automatically from the colour you
  pick: switch **Auto-mix emitters** on for a fixture type in the profile editor, with each
  emitter's colour and output. A white on an RGBW par then comes from its White LED; the emitter
  wheels show **+AUTO** and what you set on them comes on top.
- A **K** colour model: one Kelvin wheel from 1800 K to 10000 K.
- Colour-correction steps in the colour picker: CTO and CTB in ⅛, ¼, ½ and Full, working like real
  correction gels.
- When the engine stops with the app, the quit question now says so.

## 0.2.0

Published 28 October 2026 as "CMDMX 0.2.0".
[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/v0.2.0)

- The app is now called CMDMX. Shows and settings move over from the old folders by themselves
  on the first start.
- A website with the latest download and this manual.
- The app tells macOS why it asks for Local Network access (Art-Net and sACN).
- The release build can be signed and notarised by Apple; this release is not yet notarised, so
  the first open still needs the extra step described in [Getting started](getting-started.md).

## 0.1.0

Published 27 October 2026 as "ColourmotionDMX 0.1.0", before the rename. <!-- brand: old-name -->
[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/v0.1.0)

The first installable version. It contains:

- A DMX engine at 40 Hz with Art-Net, sACN and ENTTEC Pro compatible USB output.
- Patching from a large fixture library, a profile editor and GDTF import.
- The programmer, with a command line, wheels, a colour picker and palettes.
- Playbacks, cue lists and chases, with pages and fader modes.
- MIDI, OSC and Stream Deck control.
- Shows saved automatically, and Undo and Redo.
- The engine keeps output running if the app crashes.

It is installed from a DMG that is not notarised, so the first open needs the extra step described
in [Getting started](getting-started.md).
