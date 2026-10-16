# ColourmotionDMX — State

_Last updated: 2026-10-16_

## Current focus
Vertical slice 1: one hand-written fixture → patch on universe 1 → one fader in the app → Art-Net + sACN out → Eurolite MK2 over serial → BCF2000 fader moves it with motor feedback.

## Working
- Repo scaffold: meta-layer, ADR-001..004, PRD, module specs (3 written, 3 skeleton)
- `cmdmx-core`: universe buffer, patch of a dimmer/generic fixture, HTP/LTP merge, playback level → DMX frame; scenario test runner over `tests/scenarios/*.json`
- `cmdmx-protocols`: Art-Net ArtDmx, sACN E1.31 data packet, ENTTEC Pro "send DMX" frame — all byte-exact tested
- `cmdmx-control`: Mackie Control decode/encode for fader pitchbend and button notes (unit tested)
- `cmdmx-daemon`: compiles, runs a 40 Hz tick loop, no IPC yet
- `subprojects/app`: SwiftPM skeleton builds and runs on the owner's Mac (CLT only); window with offline banner, grid, 8 faders + GM. No Swift test target (XCTest/Testing absent from CLT)

## In progress
- VS1-02 on branch `vs1-02-artnet-output`: spec approved (OUT-01..04, 11, 12, 15), no tests/code yet

## Blocked
- nothing

## Hardware available for testing
- Behringer BCF2000 (use in Mackie Control mode)
- Eurolite USB-DMX512 PRO Interface MK2 (FTDI FT232R, ENTTEC Pro protocol, product string "Eurolite DMX512 Pro")
- Mac (Apple Silicon), Command Line Tools instead of Xcode.app

## Resume next session with
On branch `vs1-02-artnet-output`: write failing tests `out_03`/`out_04` (loopback UDP, bytes + sequence), `out_11` (non-blocking hand-off), `out_12` (log once per state change), `out_15` (CLI flags), then the Art-Net output thread in `cmdmx-daemon` (std only, no new crates). Then STATE/BACKLOG update and PR.
