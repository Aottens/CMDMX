# app — State

_Last updated: 2026-10-18_

## Current step
VS2-19 in review (PAL-28..30): palette windows, function buttons and softkeys, legend editor with the icon library, cyan palette values in the Programmer window and on the wheels. Checklist `tests/manual/app-palettes.md` (not yet run by the owner).

Before that:
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

- Palettes (VS2-19, PAL-28..30; `PaletteWindows.swift`, `PaletteIcons.swift`): `EngineClient` decodes `palettes` (snapshot + delta), `programmer.palettes`, the wheel `palette` and the contents `palettes` map, and sends the `palette.*` / `settings.palette` commands. Colours / Positions / Gobos & Beams windows: cells on handles, rows of 8, at least 32 cells and one empty row after the last used one; used cell = number, G/S/N tag, icon (picked from the library, else the auto icon: swatch / split swatch / position plot / "ATTR nn%"), legend ("Colour n" when empty), IPCGBES strip from `groups`; red top strip for in use, 38 % for not relevant. Tap applies (a lone number on the command line is the fade time); Record + empty cell records, Record + used cell asks Replace / Merge / Quick Merge on the softkeys (same cell again = Quick Merge; Mask, Record By, Record as in the same context; Record steps back out of the Mask softkeys); two plain taps on an empty cell = Quick Record; Update, Include, Copy, Move, Delete (confirm on softkey A) and Set Legend (softkey or keypad Set) are one-shot (`UIState.function`); the Palette key / `P` adds the token and, on a lone `Palette [n]` line, shows Master Time, Master Overlap, Set Overlap, Cancel. Icon library: 54 own ids in 8 categories, SF Symbols plus own glyphs (gobo x3, moving head, par, beam angle x2). No Swift tests (CLT): `swift build` is the gate.

## In progress
- UX-02d (playback area: pages bar, strips, Go block placeholder, GM) in review; run `tests/manual/runs/2026-10-18-UX-02d.md` passed

## Resume with
The owner's choice after UX-02d: P1-14 (library browsing + profile editor, IDEA-13) or the PBK/PAL specs. Checklist `tests/manual/app-main-screen.md` grows per zone.
