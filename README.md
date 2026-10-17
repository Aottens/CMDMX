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
built: selection, groups, stepping, patterns, the keypad command line, wheels and programmer
output work, and the app drives them (fixtures and groups windows, attribute bar, command line)
against a demo of the church rig. Colour model: ADR-005 (one target colour per
fixture; every extra emitter — White, Lime, Amber, warm/cold white, UV, … — driven directly, LIB-17).
Fixture library: the open QLC+ library is imported (ADR-006), plus profiles typed from a channel
list; the engine's own profile model (modes, cells, defaults, slots, emitters) is in place (VS2-08), and colour runs
through the engine as one value with HSI/RGB/CMY wheels, virtual dimmers and multi-cell fixtures (VS2-06); the app switches the colour wheels between HSI, RGB and CMY on a
demo of the church rig with its RGBW, RGB+Lime and RGB pars (VS2-14). The engine patches fixtures
by type, mode, count and user number with clash checks (VS2-10) and imports QLC+ fixture files
into its own profile model (VS2-09); the daemon serves the whole QLC+ library (about 1800
fixtures) plus your own profiles, searchable over IPC (VS2-11). The Patch screen follows. The main-screen design is a clickable sketch (`specs/ux/main-screen.html`, UI v0.1).
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
