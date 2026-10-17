# engine — State

_Last updated: 2026-10-17_

## Current step
IPC-21..23 (UX-02b) in review; next VS2-13 (church patch).

## Working
- `cmdmx-core`: attribute model (IPCGBES-FX, HTP/LTP, 8/16-bit), minimal profiles, patch with range checks, playbacks (priority, fader mode 0/1/2, activation + trigger counters), merge engine (ENG-01/02/03/05/06/12), scenario runner. 5 scenario files green.
- `cmdmx-protocols`: Art-Net ArtDmx, sACN E1.31 data packet, ENTTEC Pro label-6 frame + widget-params request + product-string detection. Golden-byte tests (OUT-01/02/05/06/07/08/09).
- `cmdmx-control`: MCU decode/encode for faders (10-bit pitchbend), buttons, V-Pots, ring LEDs, fader touch (CTL-01..04 provisional ids).
- `cmdmx-daemon`: 40 Hz loop over a demo patch; line 1 to an Art-Net node (`artnet_out.rs`, OUT-03/04/15) and an sACN node (`sacn_out.rs`, multicast + loopback, fixed CID, OUT-06/07/16/17), each on its own thread via `output.rs` (non-blocking hand-off, Ok/Degraded health, socket reopen, panic caught; OUT-11/12). ENTTEC Pro serial node (`serial_out.rs`, `serialport` crate: product-string detection, raw 115200 8N1, rescan ≤ 1/s; OUT-09/20/21/22) — unverified on hardware. IPC v0 (`ipc.rs`): WebSocket server on 127.0.0.1:7319, refuses browser Origins, snapshot + per-tick deltas, one thread per client, bounded queues (IPC-01..07). Demo show: one dimmer per fader; `--demo` for the breathing fader 1. Flags: `--ticks N`, `--ipc-port N`, `--demo`, `--artnet-target`, `--no-artnet`, `--sacn-unicast`, `--sacn-priority`, `--no-sacn`, `--serial PATH`, `--no-serial`, `--qlcplus-dir PATH`, `--profiles-dir PATH`. No MIDI yet.
- `cmdmx-core::programmer` (VS2-02): ordered selection with toggle, groups (record/select, order kept), new selection after a change (PRG-03), Fix ±1 stepping, n-stage patterns, All; `Engine::set_programmer_value` stores manual values (not in the merge yet). 21 tests in `cmdmx-core/tests/programmer.rs`.
- `cmdmx-core::command` (VS2-03): Titan keypad parser (`Fixture`/`Group`/`Thro`/`And`/`Not`/`@`/`Enter`) + `Engine::execute_command`: typed selection replaces, `@ v` sets I/HTP attributes, parse-then-apply. 16 tests in `cmdmx-core/tests/command_line.rs`; shared rig in `tests/common/mod.rs`.
- Programmer in the merge (VS2-04): manual values are a candidate at `Priority::Programmer` (ENG-04); `turn_wheel` (relative, clamped, start from programmer → merged → frame → 0; steps from `Settings::wheel_step_*`), `wheel_layout`, stored `active_group` (PRG-18), `contents(mask)`. Scenario runner has a `prg` action (select / wheel / command). 16 tests in `tests/wheels.rs`, scenarios PRG-12/13.
- VS2-07: `Engine::programmer_view()` (PRG-35) and `clear()` (PRG-29 default); daemon IPC-10..13 (snapshot fixtures/groups/programmer, programmer + groups deltas, ten programmer commands mapped by `apply` in `main.rs`) and the IPC-14 church demo rig with groups 1–4.
- Profile model v2 (VS2-08): `profile.rs` — `Profile{manufacturer, model, source, modes}`, `Mode{footprint, attributes, colour, virtual_dimmer, cells, pan/tilt/zoom degrees}`, `AttributeDef` + `default/locate/highlight/emitter/slots`; `from_json` validates (LIB-02, 17), legacy `{attributes}` = mode "default"; `Fixture.mode` (None = first); `Engine::add_profile` returns `Result`; `patch` writes LTP defaults (LIB-03). Colour components and cells are model-only until VS2-06. 19 tests in `tests/profile.rs`.
- Colour (VS2-06): `colour.rs` (`TargetColour`, `ColourModel`, HSV), `value.rs` (`Value`, untagged serde); `Mode::logical_attributes()` = virtual Dimmer, Colour, own attributes, cell-only names; engine `held` LTP values (patch defaults, Colour white) replace frame decoding; `write_value` maps Colour nominally to mode + cell mixing channels, levels to mode + same-named cell attributes, virtual dimmer scales colour and emitters. 18 tests in `tests/colour.rs`, scenarios LIB-04 and PRG-12 colour.
- VS2-14: IPC-18 `programmer.colour_model`; demo rig (IPC-14) built from JSON profiles in `main.rs`: RGBW washes 101–104, RGBW pars 201–208, RGB+Lime 211–216 and Showtec RGB 221–226 (virtual dimmer), spots 301–306.
- Patch core (VS2-10): `Fixture{id, number, name, profile, mode, universe, address, handle}`; `Engine::patch_add/move/remove/renumber/change_mode/handle`, `fixture`, `fixture_by_number`; number index + `next_id` (ids never reused); low-level `patch` (scenarios, rigs) checks overlap too, `number: 0` = same as id; command line resolves user numbers. 18 tests in `tests/patch.rs`.
- QLC+ importer (VS2-09): `library/qlcplus.rs` (quick-xml serde) → `Import { profile, report }`, `CoreError::ImportFailed`; church files + expected profiles in `tests/fixtures/qlcplus/`. 15 tests in `tests/qlcplus.rs`.
- Library (VS2-11): core `library::{Entry, Index}` (word search, user hides QLC+), `library::channel_list` (LIB-11, same rules as the importer), `qlcplus::header`; daemon `library.rs` (`Library::load/search/import_report/profile/save_profile/save_channel_list`, atomic user files) loaded on its own thread; `library.*` IPC answered in the client thread. `scripts/fetch-qlcplus.sh` fetches the QLC+ set (commit 3d832ce) into the git-ignored `vendor/qlcplus`; `tests/qlcplus_library.rs` imports all of it: 1779 of 1781 (two Eurolite files name a head channel outside their mode).
- Patch over IPC (VS2-12, PAT-09): snapshot/delta `fixtures` entries `{id, number, name, profile, mode, universe, address, footprint, virtual_dimmer, handle}`, `patch.*` commands mapped 1:1 in `apply`; `ipc::attach_profile` fetches a `Manufacturer/Model` profile from the library in the client thread, the tick installs it only when the show has no profile under that key (`Engine::has_profile`); `Engine::patch_name`.
- Output health over IPC (IPC-20, UX-02a): `output::OutputState` (Ok/Degraded/NoInterface/Stopped) and a lock-free `Status` per node thread; nodes report `state()`; snapshot `outputs` + `outputs` delta; the tick marks a dead node thread Stopped.
- Programmer window and picker data (IPC-21..23, UX-02b): `ProgrammerView.contents` (`FixtureValues`), `colour` (`ColourView`, first target's colour as hue/saturation + rgb), `Engine::set_colour(hue, saturation)` keeping each target's brightness; fixture entries carry `attributes` {name: group}. VS2-15: a tap on a fully selected group deselects it (PRG-02 amended).
- Workspace lints deny unwrap/expect/panic/indexing/todo; `cargo clippy -D warnings` and `cargo fmt --check` pass.

## In progress
- nothing

## Known gaps (by design, each waits for its spec)
- Release masks/times (ENG-07..10), allocation-free tick (ENG-13), chases, masters other than GM, show file (SHW-*), IPC (IPC-*), GDTF (LIB-*).

## Resume with
`cargo test && cargo clippy --all-targets -- -D warnings`, then VS2-13 (the church patch from the owner's Lightkey export). Fetch the QLC+ set once with `scripts/fetch-qlcplus.sh`.
