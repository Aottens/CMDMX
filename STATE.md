# ColourmotionDMX — State

_Last updated: 2026-10-16_

## Current focus
Vertical slice 1: one hand-written fixture → patch on universe 1 → one fader in the app → Art-Net + sACN out → Eurolite MK2 over serial → BCF2000 fader moves it with motor feedback.

## Working
- Repo scaffold: meta-layer, ADR-001..004, PRD, module specs (3 written, 3 skeleton)
- `cmdmx-core`: universe buffer, patch of a dimmer/generic fixture, HTP/LTP merge, playback level → DMX frame; scenario test runner over `tests/scenarios/*.json`
- `cmdmx-protocols`: Art-Net ArtDmx, sACN E1.31 data packet, ENTTEC Pro "send DMX" frame — all byte-exact tested
- `cmdmx-control`: Mackie Control decode/encode for fader pitchbend and button notes (unit tested)
- `cmdmx-daemon`: 40 Hz tick loop; sends line 1 as Art-Net (port-address 0, default 255.255.255.255) from its own thread, `--artnet-target`/`--no-artnet` (VS1-02 done). No IPC yet
- `subprojects/app`: SwiftPM skeleton builds and runs on the owner's Mac (CLT only); window with offline banner, grid, 8 faders + GM. No Swift test target (XCTest/Testing absent from CLT)

## In progress
- nothing

## Blocked
- nothing

## Hardware available for testing
- Behringer BCF2000 (use in Mackie Control mode)
- Eurolite USB-DMX512 PRO Interface MK2 (FTDI FT232R, ENTTEC Pro protocol, product string "Eurolite DMX512 Pro")
- Mac (Apple Silicon), Command Line Tools instead of Xcode.app

## Repo
- Public: https://github.com/Aottens/ColourmotionDMX (one PR per task)

## Resume next session with
VS1-03 (sACN, OUT-05..07) on a new branch `vs1-03-sacn-output`: spec check, failing tests, sACN node next to `artnet_out.rs`.
