# ColourmotionDMX

Open-source, native macOS lighting console built on the Avolites Titan operating model
(handles, programmer, IPCGBES palettes, fader modes, release masks, playback groups), designed
fader-first for busking with a MIDI controller. Engine in Rust as a separate show-proof process,
UI in Swift/SwiftUI, Art-Net / sACN / ENTTEC-Pro USB out, Mackie Control in.

**Status:** scaffold (2026-10-16). Engine core, protocol framing and MCU decode are tested;
no sockets, serial or MIDI yet. See `STATE.md` and `BACKLOG.md`.

## Layout

```
PROJECT.md ARCHITECTURE.md DECISIONS.md STATE.md BACKLOG.md CLAUDE.md
specs/            PRD, ADRs, module specs with numbered acceptance criteria
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
