# engine — Interface

## Publishes
- **DMX frames** per universe to configured nodes: Art-Net (UDP 6454), sACN (UDP 5568 multicast/unicast), ENTTEC Pro serial. Framing in `cmdmx-protocols`; scheduling in the daemon.
- **IPC state** to clients (contract v0, to be specified in `specs/modules/ipc.md`):
  - `snapshot` on connect: settings (incl. `faders_per_page`), patch summary, pages, playback states (level, active), warnings.
  - `palettes` (PAL-25) in the snapshot and as a whole-list delta; palette use in the programmer state (PAL-26).
  - `undo` (IPC-31) in the snapshot and as a delta when a label changes: `{"undo": label|null, "redo": label|null}`; commands `undo`/`redo` (IPC-30), refused with "Nothing to undo"/"Nothing to redo". One history in the daemon for all clients (UNDO-15), emptied by `show.open`/`show.new` and a restart (UNDO-13).
  - `outputs` (IPC-20, OUT-33) in the snapshot and as a delta: one entry per node of the show, sorted by line then id: its fields (`id, line, enabled, kind` + `port_address, target` | `universe, priority, unicast` | `port`) plus `state` `Ok | Degraded | Stopped | Disabled` and an optional `detail`.
  - `output.add {line, kind, …}` / `output.edit {id, …}` / `output.remove {ids}` (OUT-34, undoable edits) and `output.ports` → `{"type":"ports","ports":[{path, product, vid, pid, enttec}]}` to the asking client (OUT-35).
  - `control` (CTL-39, VS7-02) in the snapshot and as a delta: `devices` `[{name, input, output, state: Ok|Missing|Disabled, enabled, mapping, errors}]`, `mappings` `[{name, profile}]`, `learn` (armed action or null), `last_input` `{device, input, value}` (≤ 10/s). Commands (class neutral, refusals are warnings): `control.enable {device, on}`, `control.assign {device, mapping}`, `control.learn {action, mapping?}`, `control.learn_cancel`, `control.binding {mapping, input, action?, …options}`, `control.unbind {mapping, input}`, `control.mapping_new {name, device}`, `control.mapping_rename {name, to}`, `control.mapping_delete {name}`, `control.mapping_import {json, replace?}`; `control.mapping {name}` → `{"type":"control.mapping","name","mapping"}` to the asking client. Messages `Learned: …`, `… was …`, `<device> disconnected` arrive as warnings.
  - `delta` events: playback level/active changes, programmer, groups, `fixtures` after a patch change (PAT-09), warnings.
  - transport: WebSocket, JSON messages versioned with `"v": 0`; loopback-only for the app (DECISIONS 2026-10-17). Remote clients (iPad/browser) later need a LAN listener with authentication.
- **MIDI out** to control surfaces: feedback after each tick per binding (fader/CC values, LEDs; `cmdmx-control::feedback`, VS7-02); Mackie Control encoding with VS1-07.
- **Show file** `<name>.cmdmx.json` (SHW-01), autosaved by the daemon.

## Consumes
- Library requests from clients, answered to the asker (LIB-14): `library.search`, `library.import_report`, `library.profile`, `library.save_profile`, `library.save_channel_list`.
- IPC commands from clients: `fader.set {playback, level}`, `grand_master.set {level}`, the programmer commands (IPC-13, 18, 23 `programmer.colour`), `patch.add/move/remove/renumber/name/change_mode/handle` (PAT-09), `palette.record/apply/include/copy/move/renumber/delete/legend` and `settings.palette` (PAL-27); later `record.*`.
- MIDI in from any CoreMIDI device (raw packets → `cmdmx-control::midi::Decoder` → mapping → engine commands, VS7-02); mapping files in `~/Library/Application Support/ColourmotionDMX/control/` (or `<show-dir>/control/`), device choices in `state.json` under `control`.
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
- MIDI via CoreMIDI through `midir` 0.11 in the daemon (macOS only; elsewhere no ports).
