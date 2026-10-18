# app — State

_Last updated: 2026-10-18_

## Current step
The app connects to the daemon over WebSocket (`EngineClient.swift`, IPC-08/09): offline banner + 1 s retry, snapshot rebuilds state, faders and grand master send commands and follow deltas. Builds with `swift build`; no test target (CLT), checked with `tests/manual/app-ipc.md`.

## Working
- UX-02b (UI-14, 15, 20): attribute bar as in the sketch (tabs with group hues, rolling encoders, command-line zone, programmer summary with Clear), Programmer window (IPC-21), colour picker with hue/saturation field and model + emitter bars (IPC-22/23).
- Main-screen shell (UX-02a, `specs/modules/ui.md` UI-01..13, 26): sketch theme (dark/day), console mode (full screen, ⌘Q confirm), status bar with engine link and output health (IPC-20), rail with four workspaces (F1–F4) and screens, 2×2 window grid with Fixtures (handle order) and Groups live and labelled placeholders, side column drawn disabled, existing attribute bar and faders in their zones. Checklist `tests/manual/app-main-screen.md`.
- `swift build`, `scripts/bundle-app.sh`, `open .build/ColourmotionDMX.app` → window with offline banner, placeholder grid, 8 faders + GM.
- Programmer workspace (VS2-07): fixtures + groups windows, attribute bar with wheels, command line.
- Patch (VS2-12, PAT-10): workspace rail Program / Patch; Patch screen with the patch table, Add panel (library search, mode, count, universe, optional address and number) and Move / Renumber / Name / Change Mode / Remove on a row (two-step confirm for Remove and Change Mode); fixture buttons show user numbers in handle order. Checklist `tests/manual/app-patch.md`.
- Side column (UX-02c, UI-16..19, 27, 28): `TokenLine.swift` (shared token command line + history of 100 lines in UserDefaults `commandHistory`; `MacKeys` key-down monitor while Keys is on, never while a text field has focus); function buttons Record (arms, next group tap records; Clear or softkey Cancel disarms), Patch, Disk (Show screen); keypad live; softkeys Main / Selection / Record; ⌘1–⌘4 always, F1–F4 with Keys; `t.disabled` is the one disabled dim.
- Show (SHW-c, SHW-16/17): status bar shows the show name and the autosave age (amber while autosave fails); `UIState.screen` (Patch / Show) replaces `showPatch`; `ShowView.swift`: open show and age, list of shows (refreshed after every snapshot and `show` delta), Open and New with two-step confirm, Save As. Checklist `tests/manual/app-show.md`.
- Colour (VS2-14, IPC-19): HSI/RGB/CMY buttons on the C tab send `programmer.colour_model`; the Hue wheel shows degrees. Checklist `tests/manual/app-programmer.md` → Colour section.

## In progress
- UX-02d (playback area: pages bar, strips, Go block placeholder, GM) in review; run `tests/manual/runs/2026-10-18-UX-02d.md` passed

## Resume with
The owner's choice after UX-02d: P1-14 (library browsing + profile editor, IDEA-13) or the PBK/PAL specs. Checklist `tests/manual/app-main-screen.md` grows per zone.
