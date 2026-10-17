# app — State

_Last updated: 2026-10-17_

## Current step
The app connects to the daemon over WebSocket (`EngineClient.swift`, IPC-08/09): offline banner + 1 s retry, snapshot rebuilds state, faders and grand master send commands and follow deltas. Builds with `swift build`; no test target (CLT), checked with `tests/manual/app-ipc.md`.

## Working
- `swift build`, `scripts/bundle-app.sh`, `open .build/ColourmotionDMX.app` → window with offline banner, placeholder grid, 8 faders + GM.

## In progress
- nothing

## Resume with
VS1-06 (faders bound to page-1 playbacks) from `BACKLOG.md`. The main screen follows UI v0.1, `specs/ux/main-screen.html` (DECISIONS 2026-10-17): the current placeholder grid + fader row is replaced zone by zone as specs land.
