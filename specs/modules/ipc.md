# Module spec: ipc (IPC)

Owner: `cmdmx-daemon` (server) and `subprojects/app` (client). Status: **IPC-01..09 approved by owner 2026-10-17 (VS1-05); IPC-10..17 approved by owner 2026-10-17 (VS2-07); IPC-14 amended and IPC-18..19 approved by owner 2026-10-17 (VS2-14); IPC-35..39 approved by owner 2026-10-23 (P1-14); IPC-41 approved by owner 2026-10-25 (P1-06); IPC-42 approved by owner 2026-10-25 (P1-13); IPC-43 approved by owner 2026-10-25 (P1-20); IPC-44 and the IPC-08 amendment approved by owner 2026-10-25 (VS1-08); IPC-45 approved by owner 2026-10-27 (P1-12); IPC-14 amended 2026-10-28 (VS2-47)**. References: ADR-002 (engine as separate process), DECISIONS 2026-10-17 (WebSocket), RFC 6455.

## Purpose
The app (and later remotes, IDEA-06) sees engine state and sends commands over one versioned JSON contract. The daemon owns all state; a client only renders what the daemon reports. No client can ever slow down or stop the tick.

## Definitions
- **Client:** any WebSocket connection to the daemon (the app now; browser/iPad remotes later).
- **Snapshot:** full state a client needs to render, sent once on connect.
- **Delta:** one changed piece of state, sent to every client.

## Acceptance criteria

### Transport
- **IPC-01** The daemon listens for WebSocket connections on `127.0.0.1:7319` (loopback only); `--ipc-port <n>` changes the port. Several clients may be connected at once.
- **IPC-02** A WebSocket handshake that carries an `Origin` header is refused. Browsers always send one, so a web page on the Mac cannot drive the lights; the native app sends none. Browser remotes come with authentication later (IDEA-06).

### Messages (JSON text frames, every message has `"v"`: 0 until IPC-42, 1 since; the examples below keep the `0` they were written with)
- **IPC-03** On connect the client first receives a snapshot:
  `{"v":0,"type":"snapshot","faders_per_page":8,"grand_master":1.0,"playbacks":[{"id":1,"level":0.0,"active":false}]}`. `playbacks` lists every playback in the engine, sorted by id; `active` means the playback is in the merge.
- **IPC-04** Commands from a client:
  `{"v":0,"cmd":"fader.set","playback":1,"level":0.5}` and `{"v":0,"cmd":"grand_master.set","level":0.8}`. Levels are clamped to 0.0..1.0. A command takes effect in the frame of the next tick.
- **IPC-05** Deltas to every client, at most one per changed item per tick, only when the value changed:
  `{"v":0,"type":"playback","id":1,"level":0.5,"active":true}`, `{"v":0,"type":"grand_master","level":0.8}`. A change made by one client therefore reaches all clients, including the sender (its optimistic local value is replaced by the engine's).
- **IPC-06** Errors: invalid JSON, an unknown `cmd`, missing fields, or `"v"` other than 0 → `{"v":0,"type":"error","message":"…"}` to that client only; the connection stays open. An engine refusal (e.g. unknown playback) → `{"v":0,"type":"warning","message":"…"}` to every client (engine warnings are shared state).

- **IPC-41** Versioning (P1-06). `v` is one integer for the whole contract. It goes up only on a
  change an older peer would misread: a key removed or renamed, a value's type, unit or meaning
  changed, a command that changes shape. A new message type, command or key does not raise it
  (the app ignores types and keys it does not know; the daemon answers an unknown `cmd` with an
  `error`, IPC-06). A daemon speaks exactly one version and the app speaks exactly one; there is
  no negotiation. When they differ:
  - The app checks `v` of the snapshot (it always arrives first, IPC-03, so the app notices in
    both directions). On another version it closes the connection, renders nothing from it and
    sends nothing, and shows in place of the offline banner (UI-06): "Engine speaks protocol
    v<n>, this app speaks v<m>: install the app and engine from the same release." It then
    retries as when offline (IPC-08), so a restarted engine of the right version is picked up.
  - The daemon answers a command with another `v` with an `error` to that client only, naming
    both versions ("protocol v<n> not supported, this engine speaks v<m>"); the connection
    stays open (IPC-06). This is the backstop for other clients (remotes, scripts).
  - Either way the tick, DMX output, the show and every other client carry on untouched.

### Isolation (show-proof)
- **IPC-07** Each client runs on its own thread. The tick never waits on a client: commands reach the tick through a non-blocking channel drained at the start of each tick; messages to a client go through a bounded queue (64 messages). A client whose queue is full is disconnected (it resyncs from a fresh snapshot when it reconnects). A panic in a client thread is caught and closes only that client.
- **IPC-44** Losing a client changes nothing but that connection (VS1-08, ADR-002; approved by owner 2026-10-25).
  When a client disappears without a close handshake (the app killed with `kill -9`, its TCP
  connection reset or closed mid-message), every enabled node keeps sending one frame per tick
  (OUT-03/06) with the same values: no frame is skipped (the Art-Net sequence runs on without a
  gap), and fader levels, running cues and programmer values hold. The daemon logs at most one
  line for it. A client that connects afterwards gets a snapshot (IPC-03) that shows that live
  state: playback levels and states, each cue list's current cue, and the programmer. Tested on
  the real binary with an Art-Net node to `127.0.0.1`; the app side is IPC-08 and the manual run.

### App behaviour (verified by `swift build` + `tests/manual/app-ipc.md`; no Swift test target)
- **IPC-08** The app connects to `ws://127.0.0.1:7319` on launch. While not connected it shows the "engine offline" banner and retries every second; on (re)connect it rebuilds its state from the snapshot.
  *(Amended 2026-10-25, VS1-08, approved by owner:)* `--ipc-port <n>` on the app's command line
  (`open CMDMX.app --args --ipc-port 7400`) points it at another port, as the daemon's
  IPC-01, so a test daemon runs beside the real one; default 7319. A value that is not a port
  number is ignored with a line on stderr (the app uses 7319).
- **IPC-09** Moving an on-screen fader sends `fader.set` (slot n → playback n for now; paging is VS1-06) and the grand master sends `grand_master.set`; the displayed levels follow the deltas. To make this testable the daemon's demo show has one dimmer per fader (playback n → channel n of universe 1), and the breathing demo on playback 1 only runs with `--demo` (so it does not fight the app's fader).

### Programmer (VS2-07; behaviour in `programmer.md`)
- **IPC-10** The snapshot gains three keys: `"fixtures":[{"id":101,"profile":"wash"}]` (every patched
  fixture, sorted by id), `"groups":[{"id":2,"fixtures":[101,102,103,104]}]` (sorted by id, members
  in group order) and `"programmer":{…}` (IPC-11).
- **IPC-11** The programmer state object:
  `{"selection":[203,201],"targets":[203],"active_group":"P","attribute_groups":[{"group":"I","available":true,"has_values":false},…],"wheels":[{"attribute":"Pan","value":0.5,"touched":true,"sixteen_bit":true}],"in_programmer":[201,203],"colour_model":"Hsi"}`.
  `attribute_groups` has eight entries in I P C G B E S FX order: `available` = some target has an
  attribute in that group, `has_values` = some target has a programmer value in it. `wheels` is
  the wheel layout (PRG-17) with, per attribute, the value of the first target that has it
  (programmer value if touched, else the current output, PRG-20), whether that value is
  touched, and whether that attribute is 16-bit in its profile (has a fine channel). `in_programmer` lists fixtures with programmer values, sorted. `colour_model` (`Hsi`/`Rgb`/`Cmy`, PRG-30) says which wheels Colour shows as (added with VS2-06; Hue and Saturation values are 0.0–1.0, Hue as a fraction of 360°).
- **IPC-12** Deltas: `{"v":0,"type":"programmer",…IPC-11 fields}` to every client after a tick in
  which any of those fields changed, and `{"v":0,"type":"groups","groups":[…]}` after a tick in
  which a group was recorded. At most one of each per tick.
- **IPC-13** Commands, each mapped 1:1 onto the engine (refusals are warnings, IPC-06):
  `programmer.select {"fixture":201}` (PRG-01) · `programmer.select_group {"group":3}` (PRG-02) ·
  `programmer.step {"direction":1}` / `-1` (Fix ±1, PRG-04/05) · `programmer.pattern {"stages":2}`
  (PRG-05) · `programmer.all` (PRG-06) · `programmer.clear` (PRG-29; options IPC-32) ·
  `programmer.attribute_group {"group":"P"}` (PRG-18) ·
  `programmer.wheel {"attribute":"Pan","steps":-3,"resolution":"Fine"}` (PRG-19..21; integer
  steps, `resolution` Normal/Fine/Coarse, default Normal) ·
  `programmer.command {"tokens":["201","Thro","205","Enter"]}` (PRG-08..11) ·
  `group.record {"group":3}` (PRG-07). Missing or ill-typed fields → `error` to the sender.
- **IPC-14** Demo show: after the eight demo dimmers (fixtures 1–8 on channels 1–8, IPC-09 unchanged)
  the daemon patches the church rig on universe 1, each fixture right after the previous one:
  washes 101–104 (Dimmer, Pan 16-bit, Tilt, Zoom, R, G, B, White; 9 channels from 9), RGBW pars
  201–208 (Dimmer, R, G, B, White; 5 channels from 45), RGB+Lime pars 211–216 (R, G, B, Lime,
  virtual dimmer; 4 channels from 85), RGB pars 221–226 (Showtec LED Par 56 "6 Channel": R, G, B,
  Mode, Speed, Strobe, virtual dimmer; from 109) and front spots 301–306 as one-channel dimmers
  (145–150); and records groups 1 All church fixtures, 2 Washes, 3 Pars (201–208, 211–216,
  221–226), 4 Front spots. White and Lime are emitters (LIB-17).
  *Amended 2026-10-28 (VS2-47):* every Demo profile is patched under its own
  `<manufacturer>/<model>` key (`Demo/Dimmer` for 1–8 and 301–306, `Demo/Wash`, `Demo/RGBW Par`,
  `Demo/RGB+Lime Par`, `Showtec/LED Par 56 6 Channel`), the key `profile.save` installs under
  (IPC-38), so Patch › Edit profile… opens a Demo fixture's type (IPC-37) and its Save changes
  every fixture of that type. A Demo show saved before this keeps its old short keys.

- **IPC-18** `programmer.colour_model {"model":"Rgb"}` (`Hsi`, `Rgb`, `Cmy`) sets the colour model
  (PRG-30); the change reaches every client as a `programmer` delta (IPC-12). Another model name
  or a missing field → `error` to the sender.

- **IPC-46** Colour temperature and auto-mix (IDEA-32, PRG-63..66, ENG-20..23; approved
  by owner 2026-10-28). `programmer.colour_model` (IPC-18) also takes `Kelvin`. The `colour` object
  (IPC-22) gains `"kelvin"`: the PRG-64 reading in K (rounded to 10) or `null` for black. A Kelvin
  `wheels` entry carries that value as its 0.0–1.0 wheel value (PRG-64). `programmer.ct_step
  {"step":"cto_half"}` (`cto_full`, `cto_half`, `cto_quarter`, `cto_eighth` and the same for
  `ctb_`) applies PRG-66 to the targets; another step name or no targets with Colour → `warning`.
  An emitter's `wheels` entry gains `"auto":true` when at least one target with that emitter
  auto-mixes it (ENG-21); absent otherwise. The profile JSON carries `automix` and `output` as
  LIB-36/38 (IPC-37, 38 unchanged). For the profile editor (LIB-39), `profile.emitter
  {"name":"Warm White","cie":[0.4366,0.4042]}` (`cie` optional, two numbers) answers the asking
  client only, without the tick: `{"v":0,"type":"profile.emitter","name":"Warm White","cie":[…]|null,
  "default_cie":[…]|null,"status":"auto"|"no_colour"|"outside_gamut"}` — `default_cie` is LIB-37's
  default for the name, `status` whether an emitter at `cie` can auto-mix (ENG-20). So the
  defaults and the gamut check stay in the engine (amended 2026-10-28, VS2-46).

### App programmer workspace (verified by `swift build` + `tests/manual/app-programmer.md`)
- **IPC-15** The palette placeholder above the faders becomes two windows. Fixtures: one button per
  fixture (number and profile); selected = blue with its selection-order number, target = bold
  outline, in programmer = red top strip. Groups: one button per group (number and fixture
  count); a tap sends `programmer.select_group`; a Record button arms, and the next group tap
  (empty or used) sends `group.record`. Every state shown comes from IPC-10..12.
- **IPC-16** An attribute bar between the windows and the faders: eight tabs I P C G B E S FX
  (active, available, has values; unavailable tabs disabled) sending
  `programmer.attribute_group`; one wheel per `wheels` entry with name and value in %, red when
  touched, with one decimal on a `sixteen_bit` wheel once a Fine step was sent on it (owner,
  VS2-07 run); dragging it vertically (1 step per 4 pt) or scrolling (1 step per tick) sends
  `programmer.wheel`, with Option = Fine and Shift = Coarse; buttons Fix −1, Fix +1, Odd/Even
  (pattern 2), All and Clear (Clear red while the selection or programmer is non-empty).
- **IPC-17** A command line in the attribute bar: one text field; Return sends its words plus
  `"Enter"` as `programmer.command` and empties the field. Next to it a message line shows the
  latest `warning` or `error` until the next command is sent. While the field has focus, keys
  never trigger anything else.
- **IPC-19** On the C tab the attribute bar shows three buttons HSI, RGB and CMY (and Kelvin, UI-31), the one in
  `colour_model` highlighted; a click sends `programmer.colour_model`. The Hue wheel shows its
  value in degrees (`value × 360`, no decimals); every other wheel stays in %.

### Patch (VS2-12)
The patch messages (snapshot `fixtures` entries, `fixtures` delta, `patch.*` commands) and the
Patch screen are specified in `patch.md` PAT-09 and PAT-10; IPC-10's `fixtures` entries grow to
the PAT-09 shape.

- **IPC-40** Range forms (P1-15, PAT-19..24; approved by owner 2026-10-24): `patch.move`, `patch.renumber`,
  `patch.name` and `patch.change_mode` take either `id` (one fixture, as today) or `ids` (a range);
  both or neither → `error` (IPC-06). `{"v":0,"cmd":"patch.move","ids":[3,4,5],"universe":2,"address":101}`,
  `{"v":0,"cmd":"patch.renumber","ids":[3,4,5],"number":1301}`,
  `{"v":0,"cmd":"patch.name","ids":[3,4,5],"name":"Par 1"}`,
  `{"v":0,"cmd":"patch.change_mode","ids":[3,4,5],"mode":"7 Channel"}`. `patch.remove {ids}` is
  unchanged. Class edit, one undo step each (UNDO-04); a refusal is one `warning` and no
  `fixtures` delta; an accepted command sends one `fixtures` delta (PAT-09).

- **IPC-42** Cells (P1-13, PRG-46..55; approved by owner 2026-10-25). A selection item on the wire is
  an integer for a whole fixture (`901`, as today) or a string `"<fixture>.<cell>"` for a cell
  (`"901.2"`). `selection`, `sub_selection` and group contents (IPC-11/12) are lists of items;
  `contents` entries' `fixture`, `cue.view` fixtures and `located` keys may be `"901.2"` (the
  sub-selection is the `targets` key of IPC-11); each `fixtures` entry gains `cells` (count,
  0 for none), and `in_programmer` entries become items. `programmer.select {"fixture":901,"cell":2}`
  selects a cell (`cell` optional). A list that holds a string is a type change an older app would
  misread, so `v` goes up to 1 with this criterion (IPC-41).
  Amendment (VS2-34, approved by owner 2026-10-26): a `fixtures` entry with cells also carries `cell_attributes`,
  attribute name to group over all its cells (the virtual cell Dimmer, Colour, the cells' own
  attributes), as `attributes` does for the fixture (IPC-21); absent for a fixture without cells.
  A new key, so `v` stays 1 (IPC-41).
  Amendment (VS2-37, approved by owner 2026-10-26): `cue.remove`'s `fixture` takes an
  item, `201` or `"901.2"` (CUE-54); a string that is not an item (`"901.0"`, `"x"`) → `error` to the
  sender. An integer still means the whole fixture, so `v` stays 1 (IPC-41).

- **IPC-43** Fan (P1-20, PRG-56..62; approved by owner 2026-10-25). `programmer.fan {}` toggles Fan
  (off → on with the last curve, on → off); `programmer.fan {"curve":"Wings"}` (`Line`, `Mirror`,
  `Wings`, `Arrow`) turns it on with that curve; `programmer.fan {"curve":null}` turns it off. An
  unknown curve → `error` to the sender. The `programmer` snapshot and delta (IPC-11/12) gain
  `"fan":null|"Line"|…` and `"fan_curve":"Line"`. `programmer.wheel` is unchanged: the engine fans.
  New optional fields an older app ignores: `v` stays (IPC-41).
### Library browsing and the profile editor (P1-14; behaviour in `fixture-library.md` LIB-19..27, approved 2026-10-23)
- **IPC-35** `library.manufacturers {"query":"phantom"}` (`query` optional) → to the asking client
  only `{"v":0,"type":"library.manufacturers","query":"phantom","items":[{"name":"Showtec","models":3}]}`
  (LIB-20). Answered in the client thread like `library.search` (class neutral, UNDO-04); while
  the library is loading, the LIB-14 error.
- **IPC-36** `library.models {"manufacturer":"Showtec","query":"phantom"}` (both optional, at
  least one) → to the asking client only `{"v":0,"type":"library.models","manufacturer":"Showtec",
  "query":"phantom","items":[{"manufacturer":"Showtec","model":"Phantom 25 LED Spot","modes":[{"name":"11 Channel","channels":11}],"source":"user"}],"truncated":false}`
  (LIB-19, LIB-21). Neither field → error (IPC-06). The reply echoes `manufacturer` and `query`
  so the app drops answers to an older query (LIB-22). Neutral, client thread.
- **IPC-37** `patch.profile {"profile":"Showtec/Phantom 25 LED Spot"}` → the show's own copy of
  that profile, to the asking client only: `{"v":0,"type":"patch.profile","profile_key":"Showtec/Phantom 25 LED Spot","profile":{…}}`.
  Answered by the tick like `cue.view` (IPC-24: put on that client's queue without waiting);
  a key the show does not carry → error to the asking client. Neutral.
- **IPC-38** `profile.save {"profile":{…}}` (LIB-25): class edit, one undo step labelled "Edit
  profile <model>". The tick checks and installs it in the show (a refusal is a `warning`,
  IPC-06, and nothing is written); the user profile file is then written off the tick, like the
  autosave, and the asking client gets `{"v":0,"type":"library.saved","manufacturer","model","fixtures":4}`
  (`fixtures`: patched fixtures now using it; 0 when the show does not carry it). The library
  index takes the new entry, so a later `library.*` shows it as `"source":"user"`.
  `library.save_profile` (LIB-14) stays: it writes the file only.
- **IPC-39** `programmer.set {"attribute":"Pan","value":0.5}` sets that attribute to an absolute
  level (0.0–1.0) on every selected fixture that has it, as a touched programmer value (like a
  wheel move, PRG-19); class edit, coalesced with other `programmer.set` of the same attribute
  like a wheel (UNDO-05). A value outside 0.0–1.0, or an attribute none of the selected fixtures
  has, is a `warning`. `Colour` is refused here (`programmer.colour` sets it).
- **IPC-45** `library.import_gdtf {"path":"/Users/…/Robe@Robin Esprite@….gdtf"}` (LIB-34; approved
  by owner 2026-10-27, P1-12): the daemon reads and imports the file off the tick and answers the asking
  client only `{"v":1,"type":"library.imported","manufacturer","model","modes":[{"name","channels"}],"replaced":false,"report":{"channels":[…],"warnings":[…]}}`
  (report as LIB-14). A missing or unreadable file, one over 100 MB, or a failed import (LIB-33)
  → error (IPC-06), nothing saved. Class neutral (it changes the library, not the show; UNDO-04).
  A new message type, so `v` stays 1 (IPC-41).

### Show file (SHW-b)
The show messages (snapshot `show`, `show` delta, `show.list/new/open/save_as`) are specified in
`showfile.md` SHW-15. Wire details (SHW-b, within the approved criterion): `show.list` is
answered by the client thread like `library.*`; `modified` is the file's modification time as
ISO-8601 UTC; names are checked (SHW-09) and an Open is read from disk in the client thread, so
the tick never waits on a show file; a switch (new, open, save as) reaches the tick as one event,
which flushes the current show (SHW-11), installs the new one and sends every client a snapshot.

### Pages (VS1-06)
The `page` object in the snapshot, the `page` delta and `page.set` are specified in
`playbacks.md` PBK-03 (approved by the owner 2026-10-18).

### Main screen (UX-02)
IPC-20..23 (output health, programmer contents, the first target's colour, absolute hue and
saturation for the colour picker) are specified in `ui.md`, approved by the owner 2026-10-17.

## Out of scope (this version)
Remote access over the LAN with a password (IDEA-06); a daemon or app that speaks more than one version (IPC-41); command acknowledgements (the result of a command is its delta, or a `warning`/`error`); binary encoding. Everything listed here before is now in: pages, playbacks, palettes, cue lists, group masters, Locate/Highlight/Off/Rem Dim/Clear, undo, output nodes, patch, library, show file, the main screen and its colour picker and keypad each have their own section in this file.

### Playbacks (VS2-22; behaviour in `playbacks.md`)
The playback messages are specified in `playbacks.md` as PBK-35 (snapshot `playbacks` entries
with handle, legend, icons, halo, `state`, Flash/Latch, options and times; the small `playback`
delta for live changes and the whole-list `playbacks` delta for stored ones; `pages`, `dbo`,
`playback_settings` with their deltas) and PBK-36 (the `playback.*`, `page.legend`, `dbo.set`
and `settings.playback` commands). IPC-03's `active` is replaced by `state`.

### Palettes (VS2-19; behaviour in `palettes.md`)
The palette messages are specified in `palettes.md` as PAL-25 (snapshot `palettes` and the
`palettes` delta), PAL-26 (programmer state: `palettes {in_use, relevant}` by palette id, a
`palette` id or `null` per wheel, a `palettes` map per contents entry) and PAL-27 (the
`palette.*` and `settings.palette` commands). Commands name palettes by number; state names
them by id, and the snapshot entries carry both.

### Cue lists, chases and speed masters (VS3-05; behaviour in `cuelists.md`)
The cue-list messages are specified in `cuelists.md` as CUE-40 (snapshot `playbacks` entries with
`kind`, `cues`, `list`, `chase`, `master` and live `cue`; `bpm` in the small `playback` delta;
`connected` in the snapshot with its delta; the CUE-38 settings in `playback_settings`), CUE-41
(the `cuelist.*`, `playback.button/tap/bpm`, `cue.*`, `master.record` commands and the new
`playback.record`, `playback.options` and `settings.playback` fields) and CUE-54 (`cue.view`,
`cue.remove`). Wire details (VS3-05, within the approved criteria; IPC-24..27 approved by owner 2026-10-21):
- **IPC-24** `cue.view` is answered by the tick, because the cue state lives in the engine: the
  client thread hands the request to the tick together with its own outgoing queue, and the tick
  puts the answer on that queue only, without waiting (IPC-07: a full queue drops the answer;
  the client asks again after the next `playbacks` delta). A refused view (unknown playback or
  cue) is a `warning` to every client, as other refusals (IPC-06). Each value is
  `{"value":…,"from":3.0}` with `value` a level 0.0–1.0 or sRGB `{"r","g","b"}` as in `contents`
  (IPC-21), plus `"palette":<id>` when it is a palette reference (as PAL-26).
- **IPC-25** `playback.tap` and `playback.button` are stamped with the time the client thread
  received them, on the tick's clock (seconds since the daemon started), and carry that time to
  the engine (CUE-33: a tap is never rounded to the tick).
- **IPC-26** `master.record {page, slot, kind, number?}`: without `number` the master gets the
  lowest free number of its kind (refused when there is none, as a copy, CUE-37), so the app
  never chooses numbers itself.
- **IPC-27** `playback.record` `mode` takes `Replace`, `Merge`, `Update` (PBK-36), `Append` and
  `Insert` (CUE-03, 05); `cue` is a number (CUE-04, not together with `Append`/`Insert`) and
  `update_mode` one of `Forwards`, `CueOnly`, `Backwards`, `Both` (CUE-52). `playback.options`
  takes `kind` (`Cues`/`Chase`), and `list`, `chase` and `master` as objects whose fields are each
  optional: a missing field keeps its stored value, and the merged whole is checked as CUE-29/35
  say before anything changes.
- **IPC-28** Entry shape (VS3-05): `list` and `chase` are always present, `master` is `null` except on
  masters; the live part gains `cue` (`null` except on lists) and `bpm` (the effective BPM of a
  chase or BPM master, else `null`), so CUE-40's `effective_bpm` is this live `bpm`, and
  `multiplier` (a BPM master's effective multiplier: the fader's zone with fader `Multiplier`,
  else the stored one; `null` on anything else; VS3-08); the stored
  part gains `buttons` (the two functions `handle_buttons` gives this handle now, e.g.
  `["Go","Flash"]`, CUE-25), so the app labels strips without knowing the setting's rules. The
  `connected` delta is `{"v":0,"type":"connected","id":1}` (`id` `null` when none).

### Group masters (VS3-07; behaviour in `group-masters.md`)
The group-master messages are specified in `group-masters.md` as GRM-15. Wire details (VS3-07,
within GRM-15; IPC-29 approved by the owner 2026-10-22):
- **IPC-29** A group master's `master.level` is its saved level (GRM-13). On an enabled master it
  follows the fader, and that alone is not a stored change: a fader move sends the small
  `playback` delta (live `level`) only, never the whole-list `playbacks` delta; `master.level` is
  current in the snapshot and in every whole-list delta sent for another reason. On a disabled
  master it is the locked level and does not move with the fader. The app draws the live `level` (the fader) and shows
  `master.level` as "LOCKED 50%" while `enabled` is false (amended by the owner 2026-10-22, VS3-07 run). `master.record` with `kind: "Group"`
  needs `group` and refuses a `number`; `playback.options` `master` refuses `mode`, `scale` and
  `enabled` on a speed master and the speed-master fields (`number`, `bpm`, …) on a group master,
  so a group master's `group` and `level` never change by `playback.options`.


### Locate, Highlight, Off, Rem Dim and Clear (VS2-05, approved by owner 2026-10-22)
- **IPC-32** Commands (refusals are warnings, IPC-06; a field of the wrong type → `error`):
  `programmer.locate {"groups"?:["P"]}` (PRG-23, 38; no `groups` = all) ·
  `programmer.highlight {"on":true}` (PRG-26, 39) ·
  `programmer.off {"groups":["C"]}` or `{"attribute":"Hue"}` (PRG-27, 40; neither = the active
  group, both → `error`) · `programmer.rem_dim` (PRG-28, 41) ·
  `programmer.clear {"groups"?:["C"],"time"?:3.0}` (no field = PRG-29 by `clear_precedence`;
  `groups` = the masked Clear; `time` = ENG-10, emptying the programmer and leaving the
  selection; both together = a masked timed Clear) ·
  `settings.programmer {"clear_precedence"?, "clear_ltp"?, "lowlight_level"?}` (PRG-42, all or
  nothing). The command line ends a selection with `Locate` (PRG-10).
- **IPC-33** The programmer view (IPC-11) gains `located` (fixture id → sorted attribute names)
  and `highlight` (bool); the snapshot gains `programmer_settings {clear_precedence, clear_ltp,
  lowlight_level}` with a `programmer_settings` delta when one changes (as `palette_settings`).
- **IPC-34** Command classes (UNDO-04, PRG-44): `programmer.highlight` is live; the other IPC-32
  commands are edits, labelled "Locate", "Off <group>" / "Off <attribute>", "Rem Dim", "Clear",
  "Settings".
### Undo (VS5-02; behaviour in `undo.md`)
IPC-30 (`undo` / `redo` commands) and IPC-31 (`undo` snapshot field and delta) are specified in
`undo.md`. Every command's class (edit / live / reset / neutral, UNDO-04) is one table in
`cmdmx-daemon` (`class` in `main.rs`).
### Output nodes (P1-18; behaviour in `output-protocols.md`, approved 2026-10-22)
OUT-33 (IPC-20 amended: one `outputs` entry per node with its config and state), OUT-34
(`output.add/edit/remove`, edits) and OUT-35 (`output.ports`, neutral) are specified in
`output-protocols.md`.
