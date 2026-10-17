# Module spec: ipc (IPC)

Owner: `cmdmx-daemon` (server) and `subprojects/app` (client). Status: **IPC-01..09 approved by owner 2026-10-17 (VS1-05); IPC-10..17 approved by owner 2026-10-17 (VS2-07)**. References: ADR-002 (engine as separate process), DECISIONS 2026-10-17 (WebSocket), RFC 6455.

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

### Messages (JSON text frames, every message has `"v": 0`)
- **IPC-03** On connect the client first receives a snapshot:
  `{"v":0,"type":"snapshot","faders_per_page":8,"grand_master":1.0,"playbacks":[{"id":1,"level":0.0,"active":false}]}`. `playbacks` lists every playback in the engine, sorted by id; `active` means the playback is in the merge.
- **IPC-04** Commands from a client:
  `{"v":0,"cmd":"fader.set","playback":1,"level":0.5}` and `{"v":0,"cmd":"grand_master.set","level":0.8}`. Levels are clamped to 0.0..1.0. A command takes effect in the frame of the next tick.
- **IPC-05** Deltas to every client, at most one per changed item per tick, only when the value changed:
  `{"v":0,"type":"playback","id":1,"level":0.5,"active":true}`, `{"v":0,"type":"grand_master","level":0.8}`. A change made by one client therefore reaches all clients, including the sender (its optimistic local value is replaced by the engine's).
- **IPC-06** Errors: invalid JSON, an unknown `cmd`, missing fields, or `"v"` other than 0 → `{"v":0,"type":"error","message":"…"}` to that client only; the connection stays open. An engine refusal (e.g. unknown playback) → `{"v":0,"type":"warning","message":"…"}` to every client (engine warnings are shared state).

### Isolation (show-proof)
- **IPC-07** Each client runs on its own thread. The tick never waits on a client: commands reach the tick through a non-blocking channel drained at the start of each tick; messages to a client go through a bounded queue (64 messages). A client whose queue is full is disconnected (it resyncs from a fresh snapshot when it reconnects). A panic in a client thread is caught and closes only that client.

### App behaviour (verified by `swift build` + `tests/manual/app-ipc.md`; no Swift test target)
- **IPC-08** The app connects to `ws://127.0.0.1:7319` on launch. While not connected it shows the "engine offline" banner and retries every second; on (re)connect it rebuilds its state from the snapshot.
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
  (PRG-05) · `programmer.all` (PRG-06) · `programmer.clear` (PRG-29, default behaviour only) ·
  `programmer.attribute_group {"group":"P"}` (PRG-18) ·
  `programmer.wheel {"attribute":"Pan","steps":-3,"resolution":"Fine"}` (PRG-19..21; integer
  steps, `resolution` Normal/Fine/Coarse, default Normal) ·
  `programmer.command {"tokens":["201","Thro","205","Enter"]}` (PRG-08..11) ·
  `group.record {"group":3}` (PRG-07). Missing or ill-typed fields → `error` to the sender.
- **IPC-14** Demo show: after the eight demo dimmers (fixtures 1–8 on channels 1–8, IPC-09 unchanged)
  the daemon patches the church rig on universe 1: washes 101–104 (Dimmer, Pan 16-bit, Tilt, Zoom;
  5 channels each from address 9), pars 201–208 and 221–226 and spots 301–306 as one-channel
  dimmers from address 29 (201 = 29 … 306 = 48), and records groups 1 All church fixtures,
  2 Washes, 3 Pars, 4 Front spots. Colour attributes follow with ADR-005 (VS2-06).

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

## Out of scope (this version)
Remote access over the LAN with a password (IDEA-06), pages and paging (VS1-06/PBK), palette messages, Locate/Highlight/Off/Rem Dim and Clear options (VS2-05), colour (VS2-06), keypad buttons (the Mac keyboard covers the command line for now), command acknowledgements, binary encoding.
