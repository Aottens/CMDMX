# app — State

_Last updated: 2026-10-16_

## Current step
Skeleton only: a SwiftPM executable with one window, `faders_per_page` placeholder faders and an "engine offline" banner. **Not yet compiled** — the scaffold environment had no Swift toolchain. First job on the owner's Mac: `swift build` (Command Line Tools only).

## Working
- `Package.swift`, `Sources/ColourmotionDMX/*.swift`, `scripts/bundle-app.sh`.

## In progress
- nothing

## Resume with
`cd subprojects/app && swift build && scripts/bundle-app.sh && open .build/ColourmotionDMX.app`, then VS1-05/VS1-06 from `BACKLOG.md`.
