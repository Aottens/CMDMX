# ColourmotionDMX

An open-source, native macOS lighting console built on the Avolites Titan way of working:
handles, a programmer, IPCGBES attribute groups, palettes, playbacks with fader modes. You
program the building blocks of a show once and run them as a cue list, as busking pages on a
MIDI controller, or both.

- **Show-proof by design.** The engine is a separate Rust process; if the app hangs or crashes,
  DMX keeps flowing and the app reconnects.
- **Native Mac app** in SwiftUI, built with the Command Line Tools only.
- **Out:** Art-Net, sACN and ENTTEC-Pro-compatible USB interfaces. **In:** any MIDI controller
  and any OSC app, mapped by Learn, with motor-fader and LED feedback; Mackie Control surfaces
  such as the Behringer BCF2000 work out of the box.
- **Fixtures:** the whole open QLC+ library (about 1800 fixtures), plus profiles typed from a
  plain channel list for fixtures nobody has a file for.

The goal is a well-working general DMX console. Its first proof is a real Sunday service in the
owner's church run on it instead of Lightkey: moving-head washes, mixed RGBW / RGB+Lime / RGB
pars and theatre spots.

![The main screen: groups, colour palettes, function buttons, keypad, softkeys, attribute bar, the Undo and Redo keys and playback faders](docs/screenshots/main-screen.png)

*The main screen, against a demo of the church rig: twelve colour palettes, ten pars in the
programmer (red) on palette 5, three faders up, and softkey G ready to undo "Apply palette 5".
Controls that are not built yet are greyed out.*

![The Colour workspace: fixtures, colour picker, colour palettes and the Programmer window](docs/screenshots/colour-workspace.png)

*The Colour workspace (F3): the selected pars in blue, the colour picker, and the Programmer
window with each fixture's values; cyan means the value comes from a palette.*

## Status

Early, under active development, not yet for real shows. Next: browsing the fixture library by
manufacturer, then the church's TouchOSC remote on the OSC layer. Progress is in
[`STATE.md`](STATE.md), the plan in [`BACKLOG.md`](BACKLOG.md), the design in a clickable
sketch: [`specs/ux/main-screen.html`](specs/ux/main-screen.html).

## What works today

**Output**
- The engine runs at 40 Hz and sends every patched universe over Art-Net, sACN and USB (ENTTEC
  Pro protocol). A universe gets Art-Net and sACN by itself when its first fixture is patched;
  outputs are part of the show and can be added, changed and removed on the Output screen
  without the others dropping a frame.
- The Eurolite USB-DMX512 PRO MK2 drives real dimmers and LED pars (250000 baud, about 33 Hz).
  `--no-artnet`, `--no-sacn` and `--no-serial` silence a protocol for one run.

**Patch and fixtures**
- Pick a fixture from the library, set mode, count, address and user number; clashes are
  refused. Move, renumber, rename, change mode and remove.
- The church rig (56 fixtures from the owner's Lightkey patch) is patched by a one-off into the
  show *Kerk*.

**Programmer**
- Select fixtures and groups, Fix ±1, odd/even, a Titan-style command line
  (`201 Thro 205 @ 50`), wheels, and the console keypad with command history.
- Colour as one value per fixture: HSI, RGB or CMY wheels or a colour picker, with extra
  emitters (White, Lime, …) driven directly.
- Locate, Highlight with lowlight, Off (per attribute group or one attribute), Rem Dim, and
  Clear with a choice of what LTP attributes do.

**Palettes**
- Colours, positions and beams recorded as on Titan (Global/Shared/Normal decided
  automatically), applied by tap or `Palette 3 Enter`, snapping or fading with fixture overlap.
- Cues and playbacks keep a reference, so changing a palette changes every place that uses it.

**Playbacks, cue lists and chases**
- Record the programmer onto a handle; Replace/Merge/Update, Copy/Move/Delete, Titan paging
  locks, fader modes with fades and kill point, Flash/Swop/Latch, legends with icons, and DBO.
- Cue lists with Go, Back, Go to cue, links, loop and tracking (Block, Cue Only, Solo), times per
  attribute group, a Go block, a Cue list window and a Cue view.
- Chases at a BPM with crossfade and direction, Tap tempo, and BPM and Rate masters that drive
  several chases at once.
- Group masters: a group on a fader masters its fixtures' intensity (Scale, HTP, Limit or Take
  Over), whatever puts them there.

**Control**
- Any MIDI device: enable it, press Learn, click a fader or key on screen, move the hardware
  control. Motor faders and LEDs follow.
- Mackie Control surfaces (BCF2000, X-Touch, iCON): motor faders follow the page, Bank changes
  the page, Channel scrolls when a page has more faders than the surface.
- OSC, once switched on: a tablet sends to port 8000 and gets faders, page, cue and messages
  back on 9000; an existing layout's own addresses can be learned, so it needs no editing.

**Show, undo and app**
- The show is one readable JSON file in `~/Documents/ColourmotionDMX/Shows/`, autosaved within
  5 seconds of a change, crash-safe, and opened again at start.
- Undo and redo of every edit, 100 steps, while live playing (Go, faders, Flash) is never
  touched; ⌘Z and ⇧⌘Z.
- The app runs full screen with a status bar (engine, outputs, control devices, show), four
  workspaces, Patch, Output, Control and Show screens, and a dark and a day theme.

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
