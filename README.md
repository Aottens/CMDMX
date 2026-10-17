# ColourmotionDMX

Open-source, native macOS lighting console built on the Avolites Titan operating model
(handles, programmer, IPCGBES palettes, fader modes, release masks, playback groups). The core
is a robust programmer: program the building blocks of any show once, then run them as a cue
list, as busking pages on a MIDI controller, or both. Engine in Rust as a separate show-proof
process, UI in Swift/SwiftUI, Art-Net / sACN / ENTTEC-Pro USB out, Mackie Control in.

Phase-1 finish line: run a real Sunday service in the owner's church (~30 fixtures: moving-head
washes, mixed LED pars, theatre spots) on ColourmotionDMX instead of Lightkey.

**Status (2026-10-17):** vertical slice 1 runs: the daemon sends Art-Net, sACN and ENTTEC-Pro
serial at 40 Hz, and the app's faders drive DMX over a local WebSocket. The programmer is being
built: selection, groups, stepping and patterns are done; the command line, wheels and output
follow. The main-screen design is a clickable sketch (`specs/ux/main-screen.html`, UI v0.1).
Details in `STATE.md` and `BACKLOG.md`.

## Layout

```
PROJECT.md ARCHITECTURE.md DECISIONS.md STATE.md BACKLOG.md IDEAS.md CLAUDE.md
specs/            PRD, ADRs, module specs with numbered acceptance criteria, UX sketch
subprojects/engine  Rust workspace: cmdmx-core, -protocols, -control, -daemon
subprojects/app     SwiftPM app (Command Line Tools only, no Xcode.app)
tests/scenarios     JSON scenario tests replayed against the engine
tests/hardware      manual checklists for BCF2000, Eurolite MK2, Art-Net/sACN
```

## Build

```sh
# engine
cd subprojects/engine && cargo test && cargo run -p cmdmx-daemon
# app (needs: xcode-select --install)
cd subprojects/app && swift build && scripts/bundle-app.sh && open .build/ColourmotionDMX.app
```

## How work happens

Spec → failing test → code → review, with hardware checklists for anything that touches the
desk. Details in `CLAUDE.md`. Licence: to be decided (see `BACKLOG.md`).
