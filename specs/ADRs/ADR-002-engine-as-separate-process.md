# ADR-002: Engine runs as a separate headless process

**Status:** accepted
**Date:** 2026-10-16

## Context
The non-negotiable requirement is that DMX output keeps running during a show. UI code is the most likely place for a crash or a hang (rendering, window management, user input edge cases). Avolites Titan itself runs its engine separately from its UI for the same reason.

## Decision
- `cmdmx-daemon` is a standalone process owning the tick loop, DMX output, MIDI and the show file.
- `CMDMX.app` is a client. It connects over a loopback-only local socket (JSON messages over WebSocket, decided 2026-10-17 so browser/iPad remotes can use the same contract later), receives state snapshots and deltas, and sends commands.
- If the app dies, the daemon keeps outputting the current look. If the daemon dies, a supervisor (phase 1, later task) restarts it from the last autosave; until then the fixtures hold their last received DMX values by their own design.
- The app starts the daemon if none is running and shows a clear "engine offline" state otherwise.

## Alternatives
- Single process with the engine on a realtime thread: simpler IPC story, but one crash takes everything down.
- Engine in-process with a watchdog: a watchdog cannot save a process that has already aborted.

## Consequences
- An IPC contract must be defined and versioned (`subprojects/engine/INTERFACE.md`).
- Fader feel depends on IPC latency; target < 10 ms app→daemon→MIDI feedback. Measure early in vertical slice 1.
- Enables remote clients later (iPad, second Mac) at no extra architectural cost.
- Two processes to debug; the daemon gets a `--log` flag and a `cmdmx-cli` (later) for scripting.
