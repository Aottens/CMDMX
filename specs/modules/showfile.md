# Module spec: showfile (SHW)

Owner: `cmdmx-core::show` (schema, (de)serialisation, migrations; pure) and `cmdmx-daemon`
(shows folder, autosave, recovery, IPC), Show screen in `subprojects/app`.
Status: **SHW-01..17 approved by owner 2026-10-18 (P1-09)**. Builds on `patch.md` (PAT-08, PAT-12) and ADR-006
decision 5 (a show embeds its profiles).
Behavioural reference: Titan *Shows* (new/load/save, the show carries its personalities), macOS
autosave in place.

## Purpose
One human-readable file holds the show: it opens on another Mac without the fixture library,
survives a crash with at most a few seconds lost, diffs cleanly in git, and still opens after an
upgrade. The daemon always runs a show and saves it by itself; there is no Save button to forget.

## Definitions
- **Show data:** what the operator programmed and expects back after a restart: settings,
  profiles, patch, groups, playbacks (and later palettes, cues, pages, output and control
  configuration, each added by its own spec).
- **Live state:** what is happening right now and starts fresh after a restart: fader levels,
  Grand Master, programmer contents, selection, held LTP values, the wheels' attribute group.
  Never in the show file (owner decision 3).
- **Shows folder:** `~/Documents/ColourmotionDMX/Shows/`; a show named *Kerk* is the file
  `Kerk.cmdmx.json` in it.

## Acceptance criteria

### The file (`cmdmx-core::show`)
- **SHW-01** A show is one JSON document. Schema version 1 has the top-level keys, in this order:
  `schema_version` (1), `meta` (`name`, `created`, `modified`, `app_version`), `settings`
  (`faders_per_page`, `handles_per_page`, `ltp_trigger_level`, the three wheel steps,
  `colour_model`), `profiles` (engine profile key → profile), `patch` (`next_id` and `fixtures`:
  `{id, number, name, profile, mode, universe, address, handle}`), `groups` (`{id, fixtures}`,
  fixtures in their recorded order; from schema 7 `{id, items}` with items as `"901"` or
  `"901.2"`, PRG-52) and `playbacks` (`{id, priority, fader_mode, values}`).
  `Engine::to_show` and `Engine::from_show` convert between an engine and a `Show`; times in
  `meta` come in as parameters (the core never reads the clock).
- **SHW-02** The output is stable: pretty-printed with two-space indents and a trailing newline,
  keys in a fixed order, every collection sorted by id (profiles by key). Writing the same show
  twice gives byte-identical files, and loading a file the software wrote and writing it again
  changes nothing, so an unchanged show never shows up as a git diff.
- **SHW-03** Round trip: engine → show → engine → show gives the same show, and the loaded engine
  produces the same frames for the same fader moves. Checked for the demo show (IPC-14) and for
  `tests/fixtures/shows/church-demo.cmdmx.json`.
- **SHW-04** Every change to the schema, additive ones included, raises `schema_version` and comes
  with a migration from the previous version, so older software never re-saves a newer file and
  drops what it does not know. A file with an older version is migrated step by step and the load
  reports which migrations ran. A file with a newer version, a file that is not JSON, or one
  without `schema_version` is refused with a clear message ("Kerk was made by a newer
  ColourmotionDMX (schema 3; this one reads up to 1)"), and nothing changes.
- **SHW-05** The show carries exactly the profiles its fixtures use: saving drops profiles no
  fixture uses. Loading uses the embedded profile, never the library's, so a show opens unchanged
  on a Mac without the library or with a newer one (ADR-006 decision 5).
- **SHW-06** `next_id` is saved, so internal ids are never reused across a save and load (PAT-12):
  patch 1–3, remove 3, save, load, patch one more → it gets id 4.
- **SHW-07** A load that is not refused (SHW-04) always produces a running show and never stops
  at the first problem. Each of these is left out with one warning naming it: a profile that does
  not validate (LIB-02) and the fixtures that use it, a fixture whose profile or mode is missing,
  a fixture that clashes with an earlier one by id (PAT-02 checks, PAT-08), a group member or a
  playback value for a fixture that is not in the patch.
- **SHW-08** The file holds no absolute paths and nothing machine-specific (v1 has no `output` or
  `control` keys; their specs add them, grouped so another machine can ignore them).

### Daemon: folder, autosave, recovery
- **SHW-09** Shows live in the shows folder, created when missing (`--show-dir <path>` overrides
  it for tests). A show name is 1–64 characters, has no `/` or `:` and does not start with `.`;
  names are unique ignoring case (APFS). The daemon remembers the open show's name in
  `~/Library/Application Support/ColourmotionDMX/state.json`; with `--show-dir` that file is
  `<show-dir>/state.json`, so a test run never changes the owner's last show (SHW-b detail).
  The list holds only `*.cmdmx.json` files, sorted by name ignoring case.
- **SHW-10** At start the daemon opens the last show. With no last show, or when its file is gone,
  it opens the show *Demo*, creating it from the built-in demo rig (IPC-14) if it does not exist.
  When the last show is refused (SHW-04), the daemon opens *Demo* as well, warns, and never writes
  to the refused file.
- **SHW-11** Autosave in place (owner decision 1): within 5 s of any change to show data the
  daemon writes `<name>.cmdmx.json.tmp`, `fsync`s it and renames it over the show file, so the
  show file is always either the previous or the new complete version. Nothing is written while
  nothing changed (live state changes are not show changes). The tick only hands a copy of the
  show to a saver thread and never waits on the disk. Before a show switch (SHW-14) the current
  show is written at once.
- **SHW-12** When a load left something out (SHW-07) or ran a migration (SHW-04), the daemon first
  copies the original file to `<name>.cmdmx.json.bak` (overwriting an older `.bak`), before the
  first autosave can replace it, and says so in the warning.
- **SHW-13** When an autosave fails (disk full, no permission) the engine keeps running, the
  daemon warns once and retries every 5 s; the warning repeats only after a save has worked again
  and then fails anew. When a save works again after a failure, the daemon says so once ("autosave
  of Kerk works again"), so the failure message does not stay on screen (owner, SHW-c run). The show state (SHW-14) says the autosave is failing.
- **SHW-14** Crash recovery: when the daemon opens a show and finds a `.tmp` next to it that is
  newer than the show file and loads without being refused, the crash came between write and
  rename: the `.tmp` is renamed over the show file and a warning says the autosave of <time> was
  recovered. Any other `.tmp` is deleted with a warning.
- **SHW-18** The daemon's command line never reaches the shows folder by accident (an earlier
  `--help` opened and migrated the owner's show *Kerk*). `--help` or `-h` prints a usage text to
  stdout, one line per flag naming the flag, its value and its default, and exits 0. Any other
  argument the daemon does not know prints `cmdmx-daemon: unknown argument '<arg>'` plus the same
  usage to stderr and exits 2, and so does a `--ticks` value that is missing or not a whole
  number (it used to run forever). All of this happens before the daemon opens a show, a socket
  or a serial port.

### IPC and app
- **SHW-15** IPC (extends `ipc.md`): the snapshot carries `show: {name, saved_at, autosave_ok}`
  (`saved_at` ISO-8601 UTC of the last successful write, `null` before the first), and a
  `{"v":0,"type":"show",…}` delta follows any change of it. Commands: `show.list` → to the asking
  client only `{"v":0,"type":"show.list","shows":[{"name","modified"}]}` sorted by name;
  `show.new {name}` (empty patch, groups and playbacks, default settings); `show.open {name}`;
  `show.save_as {name}` (writes the current show under a new name and continues in it). New and
  Save As refuse an existing name, Open an unknown one, as warnings (IPC-06); a refused or failed
  switch leaves the current show running. After a switch every client gets a fresh snapshot.
- **SHW-16** The status bar shows the show's name (instead of "Demo show") and the autosave age
  ("saved 4 s ago"), amber while `autosave_ok` is false (UI-06).
- **SHW-17** A Show screen (from the workspace rail, UI-07): the open show's name, the list of
  shows (name, modified), Open on a selected row, and New and Save As each with a name field.
  New and Open change what is on stage, so they use the two-step confirm of PAT-10 ("Open Kerk —
  output changes now"). A refusal shows the warning on the screen. Verified by
  `tests/manual/app-show.md`.

## Schema sketch (v1)

```json
{
  "schema_version": 1,
  "meta": {
    "name": "Kerk",
    "created": "2026-10-18T09:00:00Z",
    "modified": "2026-10-18T09:12:41Z",
    "app_version": "0.1.0"
  },
  "settings": {
    "faders_per_page": 8,
    "handles_per_page": 40,
    "ltp_trigger_level": 0.02,
    "wheel_step_normal": 0.00392,
    "wheel_step_fine": 0.0000153,
    "wheel_step_coarse": 0.04,
    "colour_model": "Hsi"
  },
  "profiles": {
    "Showtec/LED Par 56 6 Channel": { "manufacturer": "Showtec", "model": "LED Par 56 6 Channel", "source": "qlcplus", "modes": [ … ] }
  },
  "patch": {
    "next_id": 7,
    "fixtures": [
      { "id": 1, "number": 221, "name": "", "profile": "Showtec/LED Par 56 6 Channel", "mode": "6 Channel", "universe": 1, "address": 37, "handle": { "page": 1, "slot": 1 } }
    ]
  },
  "groups": [ { "id": 3, "fixtures": [ 1, 2, 3, 4, 5, 6 ] } ],
  "playbacks": [ { "id": 1, "priority": "Normal", "fader_mode": 0, "values": { "1": { "Dimmer": 1.0 } } } ]
}
```

Playbacks are stored in their current engine shape; `playbacks.md` (P1-04) reshapes them with a
schema bump and migration (SHW-04). Palettes (P1-03), cues and cue lists (P1-08), pages, output
lines and control surfaces each add their key the same way.

## Out of scope (this spec)
Live state after a restart (owner decision 3; a later criterion if wanted), deleting and renaming
shows (Finder), importing other consoles' show files (the church patch comes in through the patch
commands, PAT-11), merging part of one show into another, undo of show edits (P1-10, `undo.md`), two daemons
on one shows folder.

## Owner decisions (2026-10-18)
1. Autosave in place, as macOS and Lightkey do: the show file is always current, there is no
   Save button; New, Open and Save As remain.
2. Shows live in `~/Documents/ColourmotionDMX/Shows/`, visible in Finder, so the owner can make it
   a git repository or a synced folder; the Show screen lists that folder.
3. A daemon restart brings back the show only; live state starts fresh, as on Titan.
