# ColourmotionDMX — Architecture

## High-level data flow

```
                 ┌─────────────────────────────────────────────────────────┐
                 │  ColourmotionDMX.app (Swift/SwiftUI, macOS)             │
                 │  thin client: faders, preset grid, programmer view      │
                 └───────────────▲───────────────────────┬─────────────────┘
                 state snapshots │  local IPC (TCP/WS,   │ commands
                 + deltas        │  JSON, loopback only) │ (fader, palette, record…)
                 ┌───────────────┴───────────────────────▼─────────────────┐
                 │  cmdmx-daemon (Rust, headless, separate process)        │
                 │                                                         │
   MIDI in/out   │  cmdmx-control ──► cmdmx-core ──► cmdmx-protocols       │  DMX out
  (BCF2000 MCU, ─┼─►  MCU / MIDI map    programmer     Art-Net / sACN /  ──┼─► network,
   generic CC)   │  ◄─ motor feedback   palettes       ENTTEC Pro serial   │  USB serial
                 │                      playbacks                          │
                 │                      HTP/LTP merge                      │
                 │                      40 Hz tick, injectable clock       │
                 └──────────────────────────┬──────────────────────────────┘
                                            │ autosave (JSON show file)
                                            ▼
                                     ~/…/show.cmdmx.json
```

Why two processes: a UI hang or crash must never stop DMX output. The daemon keeps running, holds the last look, and the app reconnects and re-syncs state. A supervisor (later) restarts the daemon from the last autosave if the daemon itself dies.

## Subprojects
- `subprojects/engine/` — Rust Cargo workspace, four crates:
  - `cmdmx-core` — pure domain model and engine. No I/O, no threads, no clock of its own (the tick receives a timestamp). Everything here is deterministic and covered by scenario tests.
  - `cmdmx-protocols` — byte-exact packet/frame builders and parsers. No sockets; the daemon owns sockets and serial ports.
  - `cmdmx-control` — control-surface protocols (Mackie Control Universal, generic MIDI mapping) as pure encode/decode. No CoreMIDI; the daemon owns MIDI ports.
  - `cmdmx-daemon` — the only crate with I/O: sockets, serial, MIDI, timers, IPC server, autosave.
- `subprojects/app/` — SwiftPM package. Renders state, sends commands. Zero lighting logic.

## Key external dependencies
- Rust crates (kept minimal, each one a decision): `serde`/`serde_json` (show file, IPC, scenarios), `thiserror` (error types). Planned for the daemon: `midir` (MIDI), `serialport` (ENTTEC Pro), `tokio` or `mio` (sockets + IPC).
- macOS: Command Line Tools (Swift toolchain + SDK). No Xcode.app.
- GDTF: zip + XML; parser lives in `cmdmx-core::library` (planned).

## Cross-cutting concerns
- **Show-proof rules (see ADR-002 and CLAUDE.md):** no `unwrap`/`expect`/`panic!` in engine code outside tests (clippy `deny`); no allocation in the per-tick hot path once the slice is stable; panics caught at thread boundaries; last look held on any error; autosave every few seconds.
- **Determinism:** the core receives time as a parameter. Scenario tests drive it with a virtual clock.
- **Attribute groups** (I P C G B E S FX) are a property of the attribute in the fixture profile, used as the mask everywhere (record, clear, release, locate). Never a UI concept.
- **Palettes are references.** Cues store palette ids, resolved at playback time.
- **Fader count is configuration** (`faders_per_page`, default 8). Pages are multiples of it; nothing in the model assumes 8 or 10.
