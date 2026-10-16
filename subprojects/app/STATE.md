# app — State

_Last updated: 2026-10-16_

## Current step
Skeleton: a SwiftPM executable with one window, `faders_per_page` placeholder faders and an "engine offline" banner. **Builds and runs on the owner's Mac** (2026-10-16, Command Line Tools only). No test target: XCTest/Swift Testing are not in the CLT; the engine holds the tests.

## Working
- `swift build`, `scripts/bundle-app.sh`, `open .build/ColourmotionDMX.app` → window with offline banner, placeholder grid, 8 faders + GM.

## In progress
- nothing

## Resume with
VS1-05 (IPC client) and VS1-06 (faders bound to engine playbacks) from `BACKLOG.md`; verify with `swift build` + `scripts/bundle-app.sh`.
