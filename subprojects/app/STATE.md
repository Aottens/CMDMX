# app — State

_Last updated: 2026-10-17_

## Current step
The app connects to the daemon over WebSocket (`EngineClient.swift`, IPC-08/09): offline banner + 1 s retry, snapshot rebuilds state, faders and grand master send commands and follow deltas. Builds with `swift build`; no test target (CLT), checked with `tests/manual/app-ipc.md`.

## Working
- `swift build`, `scripts/bundle-app.sh`, `open .build/ColourmotionDMX.app` → window with offline banner, placeholder grid, 8 faders + GM.
- Programmer workspace (VS2-07): fixtures + groups windows, attribute bar with wheels, command line.
- Patch (VS2-12, PAT-10): workspace rail Program / Patch; Patch screen with the patch table, Add panel (library search, mode, count, universe, optional address and number) and Move / Renumber / Name / Change Mode / Remove on a row (two-step confirm for Remove and Change Mode); fixture buttons show user numbers in handle order. Checklist `tests/manual/app-patch.md`.
- Colour (VS2-14, IPC-19): HSI/RGB/CMY buttons on the C tab send `programmer.colour_model`; the Hue wheel shows degrees. Checklist `tests/manual/app-programmer.md` → Colour section.

## In progress
- VS2-12 in review; owner run passed (`tests/manual/runs/2026-10-17-VS2-12.md`)

## Resume with
VS1-06 (faders bound to page-1 playbacks) from `BACKLOG.md`. The main screen follows UI v0.1, `specs/ux/main-screen.html` (DECISIONS 2026-10-17): the current placeholder grid + fader row is replaced zone by zone as specs land.
