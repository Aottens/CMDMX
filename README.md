# ColourmotionDMX

An open-source, native macOS lighting console built on the Avolites Titan way of working:
handles, a programmer, IPCGBES attribute groups, palettes, playbacks with fader modes. You
program the building blocks of a show once and run them as a cue list, as busking pages on a
MIDI controller, or both.

- **Show-proof by design.** The engine is a separate Rust process; if the app hangs or crashes,
  DMX keeps flowing and the app reconnects.
- **Native Mac app** in SwiftUI, built with the Command Line Tools only.
- **Out:** Art-Net, sACN and ENTTEC-Pro-compatible USB interfaces. **In:** Mackie Control
  surfaces such as the Behringer BCF2000.
- **Fixtures:** the whole open QLC+ library (about 1800 fixtures), plus profiles typed from a
  plain channel list for fixtures nobody has a file for.

The first goal is to run a real Sunday service in the owner's church on it instead of Lightkey.
The rig there has about 30 fixtures: moving-head washes, mixed RGBW / RGB+Lime / RGB pars and
theatre spots.

![The main screen: fixtures, groups, palettes, programmer, function buttons, keypad, softkeys, attribute bar and playback faders](docs/screenshots/main-screen.png)

*The main screen, against a demo of the church rig. Controls that are not built yet are greyed
out.*

## Status

Early, under active development, not yet for real shows. What works today:

- The engine runs at 40 Hz and outputs Art-Net, sACN and USB (ENTTEC Pro protocol).
- Patch: pick a fixture from the library, set its mode, count, address and user number. Clashes
  are refused, and you can move, renumber, rename, change mode and remove.
- Programmer: select fixtures and groups (tap again to deselect), Fix ±1, odd/even, a Titan-style command line
  (`201 Thro 205 @ 50`) and wheels.
- Colour as one value per fixture. You set it with HSI, RGB or CMY wheels or a colour picker,
  and extra emitters (White, Lime, …) are driven directly.
- The app has its real main screen. It runs full screen, with a status bar that shows the
  engine and every output. There are four workspaces of fixtures, groups and other windows, and
  a dark and a day theme.

- Show file: the engine keeps the show in one readable JSON file in
  `~/Documents/ColourmotionDMX/Shows/`. It autosaves within 5 seconds of any change, survives a
  crash, and opens the last show again when it starts. The Show screen opens, creates and saves
  shows under a new name; the status bar shows the show and when it was last saved.

- The church rig (56 fixtures from the owner's Lightkey patch) is patched by a one-off
  (`cargo run -p cmdmx-daemon --example church -- <export.tsv>`) into the show *Kerk*.

- Console keypad, function buttons and softkeys work. The command line is built from tokens, and
  with Keys on the Mac keyboard drives it. Up and Down recall earlier commands.

Next: the playback area of the main screen, and checking *Kerk* on the real rig. The design is a clickable sketch in
[`specs/ux/main-screen.html`](specs/ux/main-screen.html). After that come palettes, playbacks
and cue lists. Progress: [`STATE.md`](STATE.md), plan: [`BACKLOG.md`](BACKLOG.md).

## Build and run

Requirements: macOS 14+, Rust (stable), Xcode Command Line Tools (`xcode-select --install`).

```sh
# once: fetch the QLC+ fixture library (pinned, Apache-2.0)
subprojects/engine/scripts/fetch-qlcplus.sh

# engine (keep it running)
cd subprojects/engine && cargo run -p cmdmx-daemon

# app, in a second terminal
cd subprojects/app && swift build && scripts/bundle-app.sh && open .build/ColourmotionDMX.app
```

Tests: `cargo test` in `subprojects/engine`.

## Repository

```
PROJECT.md ARCHITECTURE.md DECISIONS.md STATE.md BACKLOG.md IDEAS.md CLAUDE.md
specs/              PRD, ADRs, module specs with numbered acceptance criteria, UX sketch
subprojects/engine  Rust workspace: cmdmx-core, -protocols, -control, -daemon
subprojects/app     SwiftPM app (Command Line Tools only, no Xcode.app)
tests/              scenario tests, manual and hardware checklists
docs/screenshots/   README images
```

## How work happens

Every feature starts as a spec with numbered acceptance criteria, then come failing tests, then
code, and the owner reviews every pull request. Anything that touches hardware also gets a
checklist that the owner runs on the real gear. Details are in [`CLAUDE.md`](CLAUDE.md).

Licence: to be decided.
