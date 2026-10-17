# engine — Interface

## Publishes
- **DMX frames** per universe to configured nodes: Art-Net (UDP 6454), sACN (UDP 5568 multicast/unicast), ENTTEC Pro serial. Framing in `cmdmx-protocols`; scheduling in the daemon.
- **IPC state** to clients (contract v0, to be specified in `specs/modules/ipc.md`):
  - `snapshot` on connect: settings (incl. `faders_per_page`), patch summary, pages, playback states (level, active), warnings.
  - `outputs` (IPC-20) in the snapshot and as a delta: per output node `Ok | Degraded | Stopped` (+ detail).
  - `delta` events: playback level/active changes, programmer, groups, `fixtures` after a patch change (PAT-09), warnings.
  - transport: WebSocket, JSON messages versioned with `"v": 0`; loopback-only for the app (DECISIONS 2026-10-17). Remote clients (iPad/browser) later need a LAN listener with authentication.
- **MIDI out** to control surfaces: motor fader positions, button LEDs, V-Pot rings (MCU encoding in `cmdmx-control`).
- **Show file** `<name>.cmdmx.json` (SHW-01), autosaved by the daemon.

## Consumes
- Library requests from clients, answered to the asker (LIB-14): `library.search`, `library.import_report`, `library.profile`, `library.save_profile`, `library.save_channel_list`.
- IPC commands from clients: `fader.set {playback, level}`, `grand_master.set {level}`, the programmer commands (IPC-13, 18), `patch.add/move/remove/renumber/name/change_mode/handle` (PAT-09); later `palette.apply`, `record.*`, `page.select`.
- MIDI in from control surfaces (raw bytes → `cmdmx-control::mcu::decode`).
- The QLC+ fixture set in `vendor/qlcplus/fixtures` (`scripts/fetch-qlcplus.sh`, pinned commit, Apache-2.0) and user profiles in `~/Library/Application Support/ColourmotionDMX/profiles/`.
- Fixture profiles in the own model (ADR-006, `fixture-library.md`): `cmdmx_core::profile::Profile::from_json` (validated, LIB-02); importers (QLC+, channel list, GDTF) convert into it. Scenario files may use the minimal `{ "attributes": [...] }` shape (one mode "default").

## Owns
- All lighting state: patch, programmer, palettes, cues, playbacks, pages, masters, output mapping, control mapping.
- The clock for the engine tick and all timing (fades, chases).
- The show file on disk.

## Does NOT handle
- Any rendering or window management.
- User-facing text/legends beyond storing them.
- Fixture library downloads (a client or separate tool fetches GDTF files; the engine loads from a folder).

## Assumes about environment
- macOS host (phase 1); the crates themselves are portable.
- Network interfaces reachable for UDP broadcast/multicast; serial device nodes under `/dev/cu.*`.
- MIDI via CoreMIDI through a Rust binding in the daemon (crate choice pending: `midir`).
