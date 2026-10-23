# ColourmotionDMX

An open-source, native macOS lighting console built on the Avolites Titan way of working:
handles, a programmer, IPCGBES attribute groups, palettes, playbacks with fader modes. You
program the building blocks of a show once and run them as a cue list, as busking pages on a
MIDI controller, or both.

- **Show-proof by design.** The engine is a separate Rust process; if the app hangs or crashes,
  DMX keeps flowing and the app reconnects.
- **Native Mac app** in SwiftUI, built with the Command Line Tools only.
- **Out:** Art-Net, sACN and ENTTEC-Pro-compatible USB interfaces. **In (planned):** any MIDI
  controller and any OSC app, mapped by Learn, with motor-fader and LED feedback; Mackie Control
  surfaces such as the Behringer BCF2000 come with a ready profile.
- **Fixtures:** the whole open QLC+ library (about 1800 fixtures), plus profiles typed from a
  plain channel list for fixtures nobody has a file for.

The goal is a well-working general DMX console. Its first proof is a real Sunday service in the
owner's church run on it instead of Lightkey.
The rig there has about 30 fixtures: moving-head washes, mixed RGBW / RGB+Lime / RGB pars and
theatre spots.

![The main screen: groups, colour palettes, function buttons, keypad, softkeys, attribute bar, the Undo and Redo keys and playback faders](docs/screenshots/main-screen.png)

*The main screen, against a demo of the church rig: twelve colour palettes, ten pars in the
programmer (red) on palette 5, three faders up, and softkey G ready to undo "Apply palette 5".
Controls that are not built yet are greyed out.*

![The Colour workspace: fixtures, colour picker, colour palettes and the Programmer window](docs/screenshots/colour-workspace.png)

*The Colour workspace (F3): the selected pars in blue, the colour picker, and the Programmer
window with each fixture's values; cyan means the value comes from a palette.*

## Status

Early, under active development, not yet for real shows. The engine's merge already has a place
for the phase-2 effects generator (effects run around the cue's values, before the masters). Next: a general MIDI and OSC control layer (spec `specs/modules/control-surfaces.md`, approved), with Mackie Control as its first profile; its pure core (MIDI decode, mappings, OSC codec) is built, and the engine now takes any MIDI device: enable it, learn a fader or button onto an action, and motor faders and LEDs follow (no Control screen in the app yet). A Mackie Control surface (BCF2000, X-Touch, iCON) works out of the box: its motor faders follow the page, Bank changes the page and Channel scrolls when a page has more faders than the surface. OSC works too, once switched on: a tablet (TouchOSC or any OSC app) sends to the console on port 8000 and gets faders, page, cue and messages back on port 9000; an existing layout's own addresses can be learned onto actions, so it needs no editing (no OSC panel in the app yet). What works today:

- The engine runs at 40 Hz and outputs every patched universe over Art-Net, sACN and USB (ENTTEC
  Pro protocol). Where each universe goes is part of the show: a universe that gets its first
  fixture goes out on Art-Net and sACN by itself, and outputs can be added, changed and removed
  (on the app's Output screen, undoable; a value out of range is refused with the reason) without the other outputs dropping a frame. The Eurolite
  USB-DMX512 PRO MK2 drives real dimmers and an LED par (250000 baud, about 33 Hz on its own
  timer); its port is stored in the show. `--no-artnet`, `--no-sacn` and `--no-serial` silence a
  protocol for one run.
- Patch: pick a fixture from the library, set its mode, count, address and user number. Clashes
  are refused, and you can move, renumber, rename, change mode (from a list of the fixture's modes)
  and remove.
- Programmer: select fixtures and groups (tap again to deselect), Fix ±1, odd/even, a Titan-style command line
  (`201 Thro 205 @ 50`) and wheels.
- Colour as one value per fixture. You set it with HSI, RGB or CMY wheels or a colour picker,
  and extra emitters (White, Lime, …) are driven directly.
- The app has its real main screen. It runs full screen, with a status bar that shows the
  engine and one dot per output protocol, and an Output screen to see and set where each
  universe goes. There are four workspaces of fixtures, groups and other windows, and
  a dark and a day theme.

- Show file: the engine keeps the show in one readable JSON file in
  `~/Documents/ColourmotionDMX/Shows/`. It autosaves within 5 seconds of any change, survives a
  crash, and opens the last show again when it starts. The Show screen opens, creates and saves
  shows under a new name; the status bar shows the show and when it was last saved.

- The church rig (56 fixtures from the owner's Lightkey patch) is patched by a one-off
  (`cargo run -p cmdmx-daemon --example church -- <export.tsv>`) into the show *Kerk*.

- Console keypad, function buttons and softkeys work. The command line is built from tokens, and
  with Keys on the Mac keyboard drives it. Up and Down recall earlier commands.

- Playback area: pages of playbacks with a pages bar, one strip per fader (legend, level,
  active edge), the Go block and the Grand Master.
- Playbacks: record the programmer onto a handle (palette
  references stay references), Replace/Merge/Update, Copy/Move/Delete, Titan paging locks
  (a front-spots fader on every page), options and times per playback, legends with automatic
  icons and halos. On the output: fader modes 0/1/2 with delay, fade in/out and overlap, kill
  point, release masks and times, Release, playback groups, Flash/Swop/Latch and DBO. In the app:
  strips with halo, icon and lock mark, live Swop/Flash, the function buttons on handles, a
  Playback Options screen, the Active playbacks window and DBO.

- Palettes in the engine, as on Titan: record colours, positions and beams from the programmer
  (Global/Shared/Normal decided automatically, also across fixture types; positions in degrees),
  apply them by tap or `Palette 3 Enter`, with or without a selection, snapping or fading over a
  typed or master time with fixture overlap. Cues and playbacks keep a reference, so changing a
  palette changes every place that uses it. In the app: Colours, Positions and Gobos & Beams
  windows with automatic icons (colour swatch, pan/tilt plot) or one from a built-in library,
  Record/Update/Include/Copy/Move/Delete and legends, as on Titan.

- Cue lists (engine): numbered cues (Append, Insert, Q2.5) run with Go, Back, Stop, Go to cue
  and Set next, follow on by themselves (after or with the cue before), loop, and track as on
  Titan (Block, Cue Only, Solo, Update modes). Times per attribute group, the fader as intensity
  master, Release resets to the top, and a handle's buttons become Go / Flash on a list. In the
  app: the Go block, a Cue list window, a Cue view with tracked values in magenta, Record
  Append/Insert, cue softkeys, list and chase options, and Space = Go with Keys on.
- Chases (engine): steps at a BPM with crossfade, Forward/Backward/Bounce/Random, loop or one
  round, Go/Back/Stop on the chase, Tap tempo that also puts the steps on the beat, and BPM
  masters (tap, typed, or on the fader) and Rate masters that drive several chases at once, in
  the app as well (Tap and BPM in the Go block).
- Group masters: a group saved onto a fader masters its fixtures' intensity, whatever puts it
  there (programmer, cue lists, chases): Scale (100–1000 %), HTP (a submaster for the group),
  Limit (a hard ceiling) or Take Over (picks the fixtures up into the programmer), Disabled to
  lock it, Flash on the lower button, several masters per group, the level saved in the show.
  In the app: the Group Master softkey under Record, strips with "GRP·SCALE 100%", and a Master
  tab. The church's use: the front spots on a Locked fader next to the service cue list.
- Undo across the whole engine: every edit (programmer, palettes, playbacks, patch, show
  settings) can be undone and redone, 100 steps back, while live playing (Go, faders, Flash) is
  never touched; what an undo takes out of the programmer is let go as Clear does. In the
  app: Undo and Redo keys labelled with the step, softkey G, ⌘Z and ⇧⌘Z.
- Locate (open white, centred, full, outside the programmer; `201 Thro 204 Locate`), Highlight
  with lowlight on the rest of the selection while you step with Fix ±1, Off for the active
  attribute group or, by double-clicking a wheel's name, for one attribute, Rem Dim, and Clear
  with a setting for its order and for what LTP attributes do (back to the playback or the
  fixture's default, or freeze). A timed Clear fades the output (IPC only for now).

Next: MIDI, Mackie Control and OSC surfaces. The
design is a clickable sketch in [`specs/ux/main-screen.html`](specs/ux/main-screen.html). Progress: [`STATE.md`](STATE.md), plan: [`BACKLOG.md`](BACKLOG.md).

## Build and run

Requirements: macOS 14+, Rust (stable), Xcode Command Line Tools (`xcode-select --install`).

```sh
# once: fetch the QLC+ fixture library (pinned, Apache-2.0)
subprojects/engine/scripts/fetch-qlcplus.sh

# engine (keep it running); `-- --help` lists the flags, `-- --show-dir <folder>` keeps a test run away from your shows
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
