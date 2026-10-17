# ColourmotionDMX — State

_Last updated: 2026-10-16_

## Current focus
Vertical slice 1: one hand-written fixture → patch on universe 1 → one fader in the app → Art-Net + sACN out → Eurolite MK2 over serial → BCF2000 fader moves it with motor feedback.

## Working
- Repo scaffold: meta-layer, ADR-001..004, PRD, module specs (3 written, 3 skeleton)
- `cmdmx-core`: universe buffer, patch of a dimmer/generic fixture, HTP/LTP merge, playback level → DMX frame; scenario test runner over `tests/scenarios/*.json`
- `cmdmx-protocols`: Art-Net ArtDmx, sACN E1.31 data packet, ENTTEC Pro "send DMX" frame — all byte-exact tested
- `cmdmx-control`: Mackie Control decode/encode for fader pitchbend and button notes (unit tested)
- `cmdmx-daemon`: 40 Hz tick loop; sends line 1 as Art-Net (VS1-02 done) and sACN universe 1 multicast, priority 100 (VS1-03 done), and ENTTEC Pro serial with product-string detection and hotplug (VS1-04, hardware unverified), each node on its own thread. IPC v0 over WebSocket on 127.0.0.1:7319 (VS1-05 done); the app's faders drive DMX
- `subprojects/app`: SwiftPM skeleton builds and runs on the owner's Mac (CLT only); window with offline banner, grid, 8 faders + GM. No Swift test target (XCTest/Testing absent from CLT)

## In progress
- nothing

## Blocked
- VS1-04 gate 3: the Eurolite MK2 is at the church; run the serial lines of `tests/hardware/output.md` on the next visit

## Hardware available for testing
- Behringer BCF2000 (use in Mackie Control mode)
- Eurolite USB-DMX512 PRO Interface MK2 (FTDI FT232R, ENTTEC Pro protocol, product string "Eurolite DMX512 Pro") — **at the church, not on the desk**; its hardware checks wait for a visit
- Mac (Apple Silicon), Command Line Tools instead of Xcode.app

## Repo
- Public: https://github.com/Aottens/ColourmotionDMX (one PR per task)

## Resume next session with
Start a fresh session (context hygiene). Next: UX-01 (clickable Titan-style main-screen sketch, recommended — it shapes the programmer specs) or VS1-06 (on-screen faders bound to playbacks of page 1 — needs the PBK paging subset). Code is written by Sonnet subagents from failing tests (CLAUDE.md).
