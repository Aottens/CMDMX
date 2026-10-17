# ColourmotionDMX — State

_Last updated: 2026-10-16_

## Current focus
Vertical slice 1: one hand-written fixture → patch on universe 1 → one fader in the app → Art-Net + sACN out → Eurolite MK2 over serial → BCF2000 fader moves it with motor feedback.

## Working
- Repo scaffold: meta-layer, ADR-001..004, PRD, module specs (3 written, 3 skeleton)
- `cmdmx-core`: universe buffer, patch of a dimmer/generic fixture, HTP/LTP merge, playback level → DMX frame; scenario test runner over `tests/scenarios/*.json`
- `cmdmx-protocols`: Art-Net ArtDmx, sACN E1.31 data packet, ENTTEC Pro "send DMX" frame — all byte-exact tested
- `cmdmx-control`: Mackie Control decode/encode for fader pitchbend and button notes (unit tested)
- `cmdmx-daemon`: 40 Hz tick loop; sends line 1 as Art-Net (VS1-02 done) and sACN universe 1 multicast, priority 100 (VS1-03 done), each node on its own thread. No IPC yet
- `subprojects/app`: SwiftPM skeleton builds and runs on the owner's Mac (CLT only); window with offline banner, grid, 8 faders + GM. No Swift test target (XCTest/Testing absent from CLT)

## In progress
- VS1-04 (Eurolite MK2 serial) on branch `vs1-04-enttec-serial`: spec awaiting approval

## Blocked
- nothing

## Hardware available for testing
- Behringer BCF2000 (use in Mackie Control mode)
- Eurolite USB-DMX512 PRO Interface MK2 (FTDI FT232R, ENTTEC Pro protocol, product string "Eurolite DMX512 Pro")
- Mac (Apple Silicon), Command Line Tools instead of Xcode.app

## Repo
- Public: https://github.com/Aottens/ColourmotionDMX (one PR per task)

## Resume next session with
VS1-04: get owner approval for OUT-09/20..22 and the `serialport` crate, then failing tests, serial node via `output::spawn`, hardware checklist with the Eurolite.
