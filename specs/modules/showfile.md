# Module spec: showfile (SHW)

Owner: `cmdmx-core::show` (schema, (de)serialisation, migrations; pure) and `cmdmx-daemon`
(shows folder, autosave, recovery, IPC), Show screen in `subprojects/app`.
Status: **SHW-01..17 approved by owner 2026-10-18 (P1-09)**; SHW-20..25 approved by owner 2026-11-12 (P1-41). Builds on `patch.md` (PAT-08, PAT-12) and ADR-006
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
- **Shows folder:** `~/Documents/CMDMX/Shows/`; a show named *Kerk* is the file
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
  CMDMX (schema 3; this one reads up to 1)"), and nothing changes.
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
  `~/Library/Application Support/CMDMX/state.json`; with `--show-dir` that file is
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

### Working on many at once (IDEA-35 batch D, P1-36; approved by owner 2026-11-07)
- **SHW-19** **A new show from this patch** (D2; amends SHW-15; Titan's New Show can only keep the DMX settings, the rest is our own choice, owner decision 2). `show.new {"name":"Kerk kerst","keep":"patch"}` saves the current show, then starts a show under the new name with the current patch: the fixtures with their numbers, names, addresses, modes and parked state, the show's own profiles, the output lines and the show settings. Groups, palettes, effect palettes, playbacks, cue lists, masters and the programmer start empty, the page is 1, and the undo history empties (UNDO-13). The old show's file is not touched, so nothing is lost. An existing name is refused as SHW-15. Without `keep` it stays SHW-15. Test `shw_19_…`.

### Show files (IDEA-35 batch G, P1-41; approved by owner 2026-11-12)
Checks behind these criteria: `tests/manual/runs/2026-11-12-G-checks.md`. Titan reference: *Loading and saving shows*,
*Clearing the console* and *Creating reports* (manual.avolites.com/docs/titan-basics/…). None of these changes the schema.

- **SHW-20** **Next week's show** (M1, first; amends SHW-19, P1-41 decision 2). `show.new {"name":"Kerk 16 nov","keep":"looks"}`
  does what `keep: "patch"` does and also keeps the groups (with their grids and legends) and the palettes (effect palettes
  included, with their numbers and legends). Playbacks, cue lists, masters, page legends and the programmer start empty, the page
  is 1, the undo history empties (UNDO-13), and the old show's file is not touched. Any other `keep` value is refused as now. In the
  app, New's *Keep the patch* checkbox becomes a choice of three: *Empty*, *The patch*, *Patch, groups and palettes*; the last is
  the default. The two-step confirm (SHW-17) names what goes: "New Kerk 16 nov — playbacks and cue lists start empty". Test
  `shw_20_…`.
- **SHW-21** **A copy that leaves you where you are** (G3, P1-41 decision 3). `show.save_copy {name}` writes the current show
  under a new name, as `show.save_as` would, and the open show stays open: its name, autosave, undo history and output do not
  change. An existing or invalid name is refused (SHW-09, SHW-15). When the copy is written, every client gets the notice "Saved a
  copy as <name>" (IPC-49) and the new `show.list`, so the Show screen lists it at once (found in the FIL-02 run, 2026-11-14). The
  Show screen gains *Save Copy* beside *Save As*, on the same name field; neither changes what is on stage, so neither asks twice.
  Test `shw_21_…`.
- **SHW-22** **Rename, delete and reveal a show** (G1; amends the out-of-scope list, P1-41 decision 4).
  `show.rename {name, to}` renames the file and its `.bak` (SHW-12); `to` follows SHW-09 and an existing name is refused.
  Renaming the open show first writes it (SHW-11), then renames, updates `state.json` and sends the `show` delta, so the open
  show carries on under its new name. `show.delete {name}` moves `<name>.cmdmx.json` and its `.bak` to the user's Trash
  (`~/.Trash`, or `<show-dir>/Trash/` with `--show-dir`, SHW-09), so Finder can still bring it back; the open show is refused
  ("Kerk is open: open another show first"). `show.list` also answers `dir`, the shows folder's absolute path. On the Show screen
  a selected row has *Rename* (with the name field), *Delete* (two-step, "Delete Kerk oud — it goes to the Trash") and
  *Show in Finder* (the app selects the file in Finder). Tests `shw_22_…`.
- **SHW-23** **A file changed outside CMDMX is never overwritten** (G4, P1-41 decision 5). The daemon remembers the size and
  modified time of the open show's file after each write and each open. When they differ (someone copied a file over it):
  1. an autosave first renames that file to `<name> outside <yyyy-mm-dd hhmm>.cmdmx.json` (a valid name, SHW-09; ` 2`, ` 3` … when
     taken), then writes as usual, and warns once: "Kerk.cmdmx.json was changed outside CMDMX: kept as Kerk outside 2026-11-12
     1430";
  2. `show.open` of the open show's own name loads the file from disk instead of being refused, after writing the show that is
     running to `<name> before open <yyyy-mm-dd hhmm>.cmdmx.json`, and warns which file holds it. While the file is unchanged, Open of
     the open show is refused as now, with the hint "its file has not changed outside CMDMX".
  The check runs before each autosave and on `show.list` (one `stat`, never on the tick). On `show.list` it only tells, once
  per change: "Kerk.cmdmx.json was changed outside CMDMX: Open Kerk to load it, or the next save keeps it aside" (made
  concrete in the FIL-05 build, 2026-11-14). Tests `shw_23_…`.
- **SHW-24** **This Mac's output stays** (G2, P1-41 decision 6; Titan asks the same when loading a show). `show.open {name,
  "output":"keep"}` opens the show with the output nodes that are running now (OUT-13: kinds, lines, names, ports, enabled) in
  place of the nodes in its file; the next autosave saves them into that show. Without `output` the show's own nodes are used, as
  now. `show.new` always takes the running nodes, with every `keep` value, so a new show at the church sends to the Eurolite at
  once; a line without fixtures sends zeros (OUT-29). In the app, Open's confirm gains a checkbox *Keep this Mac's output*, on by
  default and remembered by the app. Test `shw_24_…`.
- **SHW-25** **A report of the show** (G5, P1-41 decision 7; Titan: *Disk › Reports*). `show.report` writes one HTML file to
  `<shows folder>/Reports/<name> <yyyy-mm-dd hhmm>.html` and answers its absolute path to the asking client. It holds four tables,
  in this order, from the show as it is now: *Fixtures* (number, name, profile, mode, universe.address, parked), *Groups*
  (number, legend, member fixture numbers in order), *Palettes* (number, legend, kind, fixture count, attribute groups) and
  *Playbacks* (page, handle, legend, kind, and for a cue list or chase each cue: number, legend, fade in, fade out, delay). The
  show's name and the time are its title; it has no scripts and no outside links, and prints on A4 from a browser (PDF via
  the macOS print dialog). `cmdmx-core` renders the HTML from the show (pure, test `shw_25_…`); the daemon only
  writes the file. On the Show screen *Report* writes it and the app opens it in the default browser. A failed write is a warning
  (IPC-06). Made concrete in the FIL-06 build (2026-11-14): `cmdmx_core::report::render(show, time)`, the title
  `<name> — <yyyy-mm-dd hhmm>`; each table follows an `<h2>` with its name; fixtures sort by number, groups and palettes by
  number, playbacks by page then handle; mode is the profile's first mode when none is set, parked is `yes` or empty; members
  are fixture numbers (a cell as `31.2`) joined by `, `; a palette's kind is its window (*Colour*, *Position*, *Beam*), its
  fixture count says `global` or `by type` for a global or shared palette, and its groups are the letters of the attributes
  it holds (`I P C G B E S FX` order, an attribute no profile has is FX); a playback's kind is *Memory*, *Cue list*, *Chase*,
  *BPM master*, *Rate master* or *Group master*, and each cue of a cue list or chase is a row of its own below it, with fade
  out As In shown as the time it takes. Names and legends are HTML-escaped. `show.report` is information, so the lock does
  not refuse it (IPC-50).

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
Live state after a restart (owner decision 3; a later criterion if wanted), versions of a show (Titan keeps one per save;
here a `.bak` and the Trash), importing other consoles' show files (the church patch comes in through the patch
commands, PAT-11), merging part of one show into another (its own spec, P1-37), undo of show edits (P1-10, `undo.md`), two daemons
on one shows folder.

## Owner decisions (2026-10-18)
1. Autosave in place, as macOS and Lightkey do: the show file is always current, there is no
   Save button; New, Open and Save As remain.
2. Shows live in `~/Documents/CMDMX/Shows/`, visible in Finder, so the owner can make it
   a git repository or a synced folder; the Show screen lists that folder.
3. A daemon restart brings back the show only; live state starts fresh, as on Titan.

## Owner decisions (P1-41, batch G, 2026-11-12: every recommendation accepted)
1. **What is in batch G:** M1 (SHW-20) first, then G1 (SHW-22), G2 (SHW-24), G3 (SHW-21), G4 (SHW-23) and G5 (SHW-25)? Recommended:
   yes. G4's second half holds already: a new show has a Serial node on line 1 (`port: "auto"`, OUT-25), and its dot turns amber, not
   green, when no interface is found. *Merge from another show* stays its own spec (P1-37). Not chosen: leaving the report (G5) for
   later; it is the one M task and can be its own build.
2. **What next week's show keeps (SHW-20):** the patch, the groups and the palettes, with playbacks and cue lists empty, and that as
   New's default? Recommended: yes; groups and palettes are most of the programming, the cues are what changes each week. Not
   chosen: keeping playbacks too (that is Save As, then clearing pages), or a checkbox per kind of object (six boxes for one choice).
3. **Going back after a copy (SHW-21):** add *Save Copy*, which stays in the open show, beside *Save As*? Recommended: yes; it is
   what you want before trying something, and it never changes the stage. Not chosen: making Save As stay (the Mac's Save As moves
   you, and SHW-20 covers next week's show), or a *Back to Kerk* button (one more state to remember).
4. **Deleting a show (SHW-22):** to the Trash, never the open show? Recommended: yes; a deleted show can come back from Finder. Not
   chosen: deleting the file for good, or Finder only (it is in no app on the church Mac's screen).
5. **A file copied over the open show (SHW-23):** keep both versions by renaming, and let Open of the open show reload a file that
   changed outside? Recommended: yes; nothing is ever lost, and copying a show from home to the church Mac then works with one Open.
   Not chosen: stopping the autosave until someone decides (the engine never stops saving, SHW-13), or watching the folder live
   (`FSEvents`, more code for the same result).
6. **This Mac's output (SHW-24):** a checkbox on Open, on by default, and New always keeping the running output? Recommended: yes;
   the output belongs to the place the Mac stands, as Titan's *keep current DMX settings*. Not chosen: output on this Mac only
   (`state.json`, never in the show), which would undo OUT-13 and lose the nodes when a show moves to the church Mac on purpose.
7. **The report's form (SHW-25):** one printable HTML file with four tables, opened in the browser? Recommended: yes; it prints and
   saves as PDF from there, with no PDF code in the engine. Not chosen: CSV per table (for a spreadsheet; a later idea if wanted), or
   Titan's choice of HTML, PDF, CSV and XML.
