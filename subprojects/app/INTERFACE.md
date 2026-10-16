# app — Interface

## Publishes
- IPC commands to the engine (see `subprojects/engine/INTERFACE.md`): `fader.set`, `grand_master.set`, later palette/record/page/patch commands.
- Nothing else. The app has no DMX, MIDI or file I/O of its own (except preferences such as window layout).

## Consumes
- IPC `snapshot` and `delta` messages from the engine.
- `faders_per_page` from the snapshot to lay out the fader row.

## Owns
- Window layout, workspace arrangement, legends' presentation (colour/halo rendering), keyboard shortcuts, the "engine offline / reconnecting" state.

## Does NOT handle
- Any lighting logic: HTP/LTP, fades, palette resolution, patch math, protocol framing. If it appears here, it is a review failure (CLAUDE.md).
- Starting the daemon in phase 1 is allowed (spawn `cmdmx-daemon` if none is listening), but supervising/restarting it is the daemon-side supervisor's job (later task).

## Assumes about environment
- macOS 14+ (SwiftUI), Apple Silicon or Intel.
- Built with Xcode Command Line Tools via SwiftPM; no Xcode.app, no `.xcodeproj`.
