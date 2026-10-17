# app — State

_Last updated: 2026-10-17_

## Current step
The app connects to the daemon over WebSocket (`EngineClient.swift`, IPC-08/09): offline banner + 1 s retry, snapshot rebuilds state, faders and grand master send commands and follow deltas. Builds with `swift build`; no test target (CLT), checked with `tests/manual/app-ipc.md`.

## Working
- Main-screen shell (UX-02a, `specs/modules/ui.md` UI-01..13, 26): sketch theme (dark/day), console mode (full screen, ⌘Q confirm), status bar with engine link and output health (IPC-20), rail with four workspaces (F1–F4) and screens, 2×2 window grid with Fixtures (handle order) and Groups live and labelled placeholders, side column drawn disabled, existing attribute bar and faders in their zones. Checklist `tests/manual/app-main-screen.md`.
- `swift build`, `scripts/bundle-app.sh`, `open .build/ColourmotionDMX.app` → window with offline banner, placeholder grid, 8 faders + GM.
- Programmer workspace (VS2-07): fixtures + groups windows, attribute bar with wheels, command line.
- Patch (VS2-12, PAT-10): workspace rail Program / Patch; Patch screen with the patch table, Add panel (library search, mode, count, universe, optional address and number) and Move / Renumber / Name / Change Mode / Remove on a row (two-step confirm for Remove and Change Mode); fixture buttons show user numbers in handle order. Checklist `tests/manual/app-patch.md`.
- Colour (VS2-14, IPC-19): HSI/RGB/CMY buttons on the C tab send `programmer.colour_model`; the Hue wheel shows degrees. Checklist `tests/manual/app-programmer.md` → Colour section.

## In progress
- UX-02a in review; owner runs `tests/manual/app-main-screen.md`

## Resume with
VS1-06 (faders bound to page-1 playbacks) from `BACKLOG.md`. The main screen follows UI v0.1, `specs/ux/main-screen.html` (DECISIONS 2026-10-17): the current placeholder grid + fader row is replaced zone by zone as specs land.
