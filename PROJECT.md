# CMDMX

Open-source, native macOS lighting console built on the Avolites Titan operating
model (handles, programmer, IPCGBES palettes, fader modes, release masks, playback
groups), built around a robust programmer: program the building blocks once, then run
them as cue lists, busking pages or (later) a timeline.

## Client / Stakeholder
- Owner: Aernout Ottens (Colourmotion). Product owner, reviewer, hardware tester.
- Users (later): lighting operators who know Avolites and work from a Mac.

## Scope (in scope)
- Phase 1 "program the building blocks of any show" (decided 2026-10-17, see DECISIONS):
  - Fixture library via GDTF import (thin overlay for attribute group, locate values, virtual dimmer)
  - Patch, groups, attribute groups (IPCGBES-FX)
  - **Programmer as the core:** ordered selection, touched attributes only, wheels (encoders), colour picker in RGB/CMY/HSI, Locate/Highlight, Clear options, 16-bit, Fan over the selection order (IDEA-27)
  - Palettes (Global/Shared/Normal, reference model, quick palette, fade + overlap)
  - Recording cues onto playbacks and into cue lists; chases (BPM, tap)
  - Running what you programmed: playbacks with fader modes 0/1/2, priority, release mask/time, kill point; cue lists with Go/Back, fade/delay per cue, attribute-group times, link/follow and tracking (Track/Block/Cue Only/Solo, Update modes; pulled into phase 1 by the owner 2026-10-21)
  - Pages of N faders (8 default, configurable), Grand Master, BPM master, group masters, playback groups
  - HTP/LTP engine with 5 priority levels
  - Output: Art-Net, sACN, ENTTEC-Pro-compatible USB (Eurolite USB-DMX512 PRO MK2 as reference)
  - Input: a general MIDI and OSC control layer (any device, Learn, feedback, portable mappings); Mackie Control (BCF2000 in MC mode, X-Touch) and the church TouchOSC remote are a profile and a client on it
  - Show file: one readable JSON file, autosave, git-friendly
- Phase 2: show-running depth (tracking views, Move In Dark, …), shape generator, fader mode 3, nested/time palettes, align (fan moved to phase 1, IDEA-27), virtual faders, scene master, gel picker + colour calibration (IDEA-02/03), more USB interfaces (IDEA-01)
- Phase 3: hardware — the open-hardware maker desk (IDEA-23); runs as a parallel track beside phase 1 since 2026-10-25 (`subprojects/hardware/`)
- Phase 3+: timeline synced to Ableton/timecode (IDEA-05), pixel mapper, key frame shapes, set list, sACN input, visualiser
- Ideas not yet placed: `IDEAS.md`

## Reference venue: the church
The owner built and rigged the church; it is the playground where ideas are tested before they
go to other setups. **Phase 1 delivers a well-working general console; its first proof is replacing Lightkey (paid)
there with CMDMX** (owner, 2026-10-22: the church is the playground, not what orders the work).
- Shows: mostly Sunday morning services — static looks, little movement; the 6 front spots sit on
  their own fader so they can go out for a video. Sometimes bigger shows.
- Rig (~30 fixtures): 4 moving-head washes with zoom (19×15 W), ~20 LED pars of mixed types
  (RGBW, RGB+Lime, classic RGB), 6 single-channel theatre spots (dimmers).
- Output: Eurolite USB-DMX512 PRO MK2 (kept at the church).
- Consequences: mixed LED types make colour mapping per fixture type a phase-1 need (P1-07; full
  calibration stays phase 2, IDEA-03); cheap pars often lack GDTF, so profiles from a plain channel
  list are needed (P1-01).

## Out of scope (for this milestone)
- Media server, Synergy/Ai, Capture visualiser, TitanNet multi-console, NDI
- Windows/Linux UI (the engine is portable; the UI is macOS only)
- Any licensing dongle or paywall

## Stack
- Engine (`subprojects/engine/`): Rust, headless daemon, Cargo workspace
  - `cmdmx-core` – data model, programmer, palettes, playback engine, HTP/LTP merge
  - `cmdmx-protocols` – Art-Net, sACN (E1.31), ENTTEC Pro serial framing (byte-exact)
  - `cmdmx-control` – general MIDI + OSC mapping layer, Mackie Control profile
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
