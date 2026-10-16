# ColourmotionDMX

Open-source, native macOS lighting console built on the Avolites Titan operating
model (handles, programmer, IPCGBES palettes, fader modes, release masks, playback
groups), designed fader-first for busking with a MIDI controller.

## Client / Stakeholder
- Owner: Aernout Ottens (Colourmotion). Product owner, reviewer, hardware tester.
- Users (later): lighting operators who know Avolites and work from a Mac.

## Scope (in scope)
- Phase 1 "one evening of busking":
  - Fixture library via GDTF import (thin overlay for attribute group, locate values, virtual dimmer)
  - Patch, attribute groups (IPCGBES-FX), programmer, wheels (encoders)
  - Palettes (Global/Shared/Normal, reference model, quick palette, fade + overlap)
  - Cues, chases (BPM, tap), playbacks with fader modes 0/1/2, priority, release mask/time, kill point
  - Pages of N faders (8 default, configurable), Grand Master, BPM master, group masters, playback groups
  - HTP/LTP engine with 5 priority levels
  - Output: Art-Net, sACN, ENTTEC-Pro-compatible USB (Eurolite USB-DMX512 PRO MK2 as reference)
  - Input: Mackie Control driver (BCF2000 in MC mode, X-Touch), free MIDI note/CC mapping, OSC schema
  - Show file: one readable JSON file, autosave, git-friendly
- Phase 2: shape generator, fader mode 3, nested/time palettes, fan/align, virtual faders, scene master
- Phase 3+: cue lists with tracking, pixel mapper, key frame shapes, set list, sACN input, visualiser

## Out of scope (for this milestone)
- Media server, Synergy/Ai, Capture visualiser, TitanNet multi-console, timelines, NDI
- Windows/Linux UI (the engine is portable; the UI is macOS only)
- Any licensing dongle or paywall

## Stack
- Engine (`subprojects/engine/`): Rust, headless daemon, Cargo workspace
  - `cmdmx-core` – data model, programmer, palettes, playback engine, HTP/LTP merge
  - `cmdmx-protocols` – Art-Net, sACN (E1.31), ENTTEC Pro serial framing (byte-exact)
  - `cmdmx-control` – Mackie Control Universal mapping, generic MIDI mapping
  - `cmdmx-daemon` – process entry point, 40 Hz tick loop, local IPC server
- App (`subprojects/app/`): Swift/SwiftUI via SwiftPM, builds with Xcode Command Line Tools only (no Xcode.app). Thin client: renders engine state, sends commands over local IPC. No lighting logic in the UI.
- Tests: Rust unit tests, JSON scenario tests (`tests/scenarios/`), byte-exact protocol tests, manual hardware checklists (`tests/hardware/`)
- CI: GitHub Actions – `cargo test` on ubuntu, `swift build` on macos
- Fixture format: GDTF (zip + XML), overlay JSON for Titan-specific fields

## Key people
- Product owner / hardware tester: Aernout
- Engineering: Claude Code sessions, gated by spec approval, PR review and hardware checklists

## Reference material
- Analysis of Avolites Titan as basis for this console (Claude Doc, 2026-10-16):
  https://claude.ai/code/artifact/e1214e2e-fb3a-49eb-943f-b2b0355d4c22
  Its "Eerste speclist" section is the seed of `specs/PRD.md`.

## Status
See `STATE.md`.
