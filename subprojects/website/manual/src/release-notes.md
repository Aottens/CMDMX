# Release notes

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
