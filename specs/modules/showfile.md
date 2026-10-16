# Module spec: showfile (SHW)

Owner: `cmdmx-core::show` (schema + (de)serialisation) and `cmdmx-daemon` (autosave). Status: **draft for owner approval**.

## Purpose
One human-readable file holds everything a show needs; it survives crashes, diffs cleanly in git, and loads on a newer version of the software.

## Acceptance criteria
- **SHW-01** The show is one JSON document `<name>.cmdmx.json` with top-level keys: `schema_version` (integer), `meta` (name, created, modified, app_version), `settings` (incl. `faders_per_page`, release defaults), `profiles`, `patch`, `groups`, `palettes`, `cues`, `chases`, `playbacks`, `pages`, `output` (lines → nodes), `control` (surface mappings).
- **SHW-02** `Show::to_json` is stable: keys are emitted in a fixed order and collections are sorted by id, so saving an unchanged show produces a byte-identical file (git-friendly).
- **SHW-03** `Show::from_json` of a file with an older `schema_version` runs migrations up to the current version and reports which ran; a newer version than the software knows is rejected with a clear error and nothing is overwritten.
- **SHW-04** A round-trip (`from_json(to_json(show)) == show`) holds for every scenario file's derived show and for a fixture show checked into `tests/fixtures/shows/`.
- **SHW-05** Autosave: the daemon writes `<name>.cmdmx.json.tmp` every 5 s if dirty, `fsync`s, then atomically renames over the show file; a crash mid-write never leaves a truncated show file.
- **SHW-06** On load, if `<name>.cmdmx.json.tmp` exists and is newer and valid, the daemon offers it as the recovery copy (reported over IPC; the app asks the user).
- **SHW-07** The file never contains absolute paths or machine-specific identifiers except in `control` (MIDI port names) and `output` (serial port, IPs), which are grouped so they can be ignored on another machine.
- **SHW-08** Palettes in cues are stored as references (`palette_id`), never as copied values; the loader verifies referential integrity and reports dangling references as warnings without refusing to load.

## Schema sketch (v1)

```json
{
  "schema_version": 1,
  "meta": { "name": "Demo", "created": "2026-10-16T08:00:00Z", "modified": "…", "app_version": "0.1.0" },
  "settings": { "faders_per_page": 8, "release_time_s": 0.0, "ltp_trigger_level": 0.02 },
  "profiles": { "dimmer": { "attributes": [ { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 } ] } },
  "patch": [ { "id": 1, "profile": "dimmer", "universe": 1, "address": 1, "user_number": 1 } ],
  "groups": [], "palettes": [], "cues": [], "chases": [],
  "playbacks": [ { "id": 1, "content": { "cue": 1 }, "priority": "Normal", "fader_mode": 0, "release_mask": "Global", "release_time_s": null, "kill_point": "FadeOutComplete" } ],
  "pages": [ { "id": 1, "legend": "Page 1", "slots": [ { "index": 0, "playback": 1, "lock": "Unlocked" } ] } ],
  "output": { "lines": [ { "line": 1, "nodes": [ { "sacn": { "universe": 1, "priority": 100 } } ] } ] },
  "control": { "surfaces": [ { "mcu": { "midi_in": "BCF2000", "midi_out": "BCF2000" } } ] }
}
```
