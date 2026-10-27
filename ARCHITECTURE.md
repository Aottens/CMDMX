# CMDMX — Architecture

## High-level data flow

```
                 ┌─────────────────────────────────────────────────────────┐
                 │  CMDMX.app (Swift/SwiftUI, macOS)             │
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
  - `cmdmx-control` — the general MIDI and OSC control layer as pure code: MIDI and OSC decode/encode, mappings, pickup, feedback, learn, and surface profiles on top (Mackie Control first). No CoreMIDI or sockets; the daemon owns MIDI ports and the OSC socket (`control-surfaces.md`).
  - `cmdmx-daemon` — the only crate with I/O: sockets, serial, MIDI, timers, IPC server, autosave.
- `subprojects/app/` — SwiftPM package. Renders state, sends commands. Zero lighting logic.

## Key external dependencies
- Rust crates (kept minimal, each one a decision): `serde`/`serde_json` (show file, IPC, scenarios), `thiserror` (error types); daemon: `serialport` (ENTTEC Pro: USB product string + raw port, default features off), `tungstenite` (IPC WebSocket, sync, one thread per client — no async runtime). `midir` (MIDI, macOS only, VS7-02). `hidapi` (Stream Decks, macOS only, default features off, VS7-06) and `libc` (SIGINT/SIGTERM blank the decks, unix). Control: `fontdue` and `jpeg-encoder` (key images, VS7-05).
- macOS: Command Line Tools (Swift toolchain + SDK). No Xcode.app.
- GDTF: zip + XML; parser lives in `cmdmx-core::library` (planned).

## Cross-cutting concerns
- **Show-proof rules (see ADR-002 and CLAUDE.md):** no `unwrap`/`expect`/`panic!` in engine code outside tests (clippy `deny`); no allocation in the per-tick hot path once the slice is stable; panics caught at thread boundaries; last look held on any error; autosave every few seconds.
- **Determinism:** the core receives time as a parameter. Scenario tests drive it with a virtual clock.
- **Attribute groups** (I P C G B E S FX) are a property of the attribute in the fixture profile, used as the mask everywhere (record, clear, release, locate). Never a UI concept.
- **Palettes are references.** Cues store palette ids, resolved at playback time.
- **Fader count is configuration** (`faders_per_page`, default 8). Pages are multiples of it; nothing in the model assumes 8 or 10.
