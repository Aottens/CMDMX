# Module spec: control-surfaces (CTL)

Owner: `cmdmx-control` (pure: MIDI and OSC decode/encode, mappings, pickup, feedback, learn) and
`cmdmx-daemon` (CoreMIDI ports, the OSC socket, mapping files, IPC). Status: **CTL-05..43
approved by owner 2026-10-23** (P1-05; the ten questions answered, see Owner decisions). CTL-01..04 (the Mackie Control wire format) are kept from slice 1
and already tested in `cmdmx-control/src/mcu.rs`; they now belong to the Mackie Control profile.
Scope set by the owner on 2026-10-23 (DECISIONS): a **general MIDI and OSC control layer**; named
hardware (the BCF2000) and named clients (the church TouchOSC remote) are profiles and clients on
it, never the design.

References: Avolites Titan v19 manual, *Triggers* (mappings of MIDI/DMX/audio triggers to hardware
controls or items, Learn, channel/command/value ranges; [v12 page](https://manual.avolites.com/docs/12.0/running-the-show/midi-dmx-or-audio-triggering/))
and *Playback controls*; ADR-003 (Mackie Control, amended 2026-10-23); MIDI 1.0 specification;
OSC 1.0 specification; ETC Eos *OSC* chapter as reference for an OSC address schema (Titan has no
OSC); TouchOSC's default ports (send 8000, receive 9000).

## Purpose
Any MIDI device and any OSC client can play the console: faders, handle buttons, Go/Back, page,
Grand Master, programmer keys, keypad and wheels. A control does nothing a screen button cannot:
every input becomes one of the engine's existing commands (the IPC command set), so undo, refusals
and live/edit classes stay the engine's (UNDO-04). The console answers back: motor faders follow a
page change, LEDs show what is active, OSC layouts show levels and legends. Mappings are made by
Learn, stored as readable files and travel between Macs.

## Definitions
- **Device:** a MIDI input (CoreMIDI source) together with the output (destination) of the same
  name, if there is one. Identified by its name; a second device with the same name is `name #2`.
- **Input:** one control on a device: `{kind, channel, number}` with kind `note`, `cc`, `cc14`
  (MSB on CC 0..31 with its LSB on CC +32), `pitchbend` or `program`; channel 1..16; `number` the
  note, CC or program (none for pitchbend). For OSC the input is an address.
- **Action:** what an input does, from the action catalogue (CTL-05), e.g. `fader 3`, `go`,
  `key clear`, `wheel 2`.
- **Binding:** input → action, with options (range, invert, button mode, encoder encoding, pickup)
  and feedback (CTL-19).
- **Mapping:** a named list of bindings for one device or for OSC, stored as one JSON file.
- **Profile:** a mapping shipped with the app, read-only, optionally with a protocol driver
  (Mackie Control). Editing a profile saves a user copy that hides it (as LIB user profiles do).
- **Feedback:** messages the console sends back to the element of a binding: a fader or CC value,
  an LED, an encoder ring, an OSC value or text.
- **Handle-relative / absolute:** `fader 3` is slot 3 of the current page (follows page changes);
  `fader 2.3` is page 2 slot 3; `playback 12` is a playback by id. Default handle-relative.

## Acceptance criteria

### Actions (one vocabulary for MIDI and OSC)
- **CTL-05** Every action sends one engine command; there is no lighting logic in the control layer
  (ARCHITECTURE). The catalogue for phase 1:

  | Action | Value | Command |
  | --- | --- | --- |
  | `fader <slot \| page.slot>`, `playback <id> level` | continuous | `fader.set` (the slot's playback; an empty slot does nothing) |
  | `upper <slot>`, `lower <slot>` | button | `playback.button` (CUE-55) |
  | `go`, `back`, `stop`, `release` (optional `<slot>` or `playback <id>`; none = connected list, CUE-27) | button (on press) | `cuelist.go/back/stop`, `playback.release` |
  | `tap` (same targets) | button | `playback.tap` |
  | `page next`, `page prev`, `page <n>` | button | `page.set` (refused at 1 and at the last page: nothing happens) |
  | `channel next`, `channel prev` (CTL-32) | button | none: moves the Mackie Control fader window |
  | `gm` | continuous | `grand_master.set` |
  | `dbo` | button (held) | `dbo.set` on press, off on release |
  | `key <name>`: `clear`, `locate`, `highlight`, `rem_dim`, `undo`, `redo`, `all`, `step_next`, `step_prev` (Fix ±1) | button | the command the screen key sends (`programmer.*`, `undo`, `redo`) |
  | `keypad <token>`: `0`–`9`, `.`, `@`, `Thro`, `And`, `Not`, `Fixture`, `Group`, `Palette`, `Full`, `Enter`, `Backspace` | button | the token into the command line as the screen keypad does (UI-17) |
  | `wheel <n>` | relative | `programmer.wheel` on the n-th wheel of the active attribute group (`wheels`, IPC-11) |
  | `attribute_group <I\|P\|C\|G\|B\|E\|S\|FX>` | button | `programmer.attribute_group` |
  | `group <id>`, `palette <id>` | button | select the group / apply the palette as its window button does |

  New actions are added to this table by later criteria, never as free-form commands.
- **CTL-06** An action runs with the class of its command (UNDO-04): a fader move is live and never
  in the history; `keypad Enter` that records is an edit and undoable. A refusal is a warning in
  the message zone with the device name (`BCF2000: page 11 does not exist`), as for any client.
- **CTL-07** Value conversions when an input's kind differs from the action's value:
  - continuous input on a button action: press when it rises to ≥ 0.5, release when it falls
    below 0.5 (a fader as a Flash);
  - button input on a continuous action: press = the binding's max, release = its min
    (default 1.0 and 0.0; a Flash-like GM bump);
  - absolute input on a wheel: the difference to its previous value, full travel = one full turn of
    the wheel's range (PRG-19);
  - relative input on a continuous action: each step moves the level by 1/256 (Fine 1/1024,
    Coarse 1/64), clamped.

### Generic MIDI in
- **CTL-08** The daemon lists every CoreMIDI device (CTL definitions) with its state `Ok`,
  `Missing` (seen before, not connected now) or `Disabled`. Any number of devices run at once, each
  with its own mapping; a mapping may also say `"device": "*"` (any enabled device).
- **CTL-09** Decoded messages: note on/off (note on with velocity 0 is off), 7-bit CC, 14-bit CC
  (only where a binding says `cc14`: the value is applied when the LSB arrives, or alone after the
  MSB when no LSB follows within the same packet), pitch bend (14-bit), program change. One CoreMIDI
  packet may carry several messages; running status is honoured. Poly and channel aftertouch,
  sysex (outside a protocol driver), clock, MTC and active sensing are ignored.
- **CTL-10** Scaling: 7-bit 0..127 → 0.0..1.0 with 127 exactly 1.0; 14-bit 0..16383 likewise;
  a binding may set `min`/`max` (input range, values outside are clamped) and `invert`. Example:
  CC 7 value 64 → level 0.504; with `invert` → 0.496.
- **CTL-11** Relative encoders, encoding per binding: `sign_magnitude` (bit 6 = down, MCU),
  `twos_complement` (1..63 up, 127..65 down) and `binary_offset` (64 = 0, 65 up 1, 63 down 1).
  Example: `twos_complement` 0x7D → −3; `binary_offset` 0x43 → +3.
- **CTL-12** Buttons: a note, or a CC/program where value > 0 is press and 0 is release (program
  change is press only). Button mode per binding: `momentary` (press and release passed on,
  default) or `toggle` (each press flips on/off, for held actions such as `dbo`, `upper` on a
  Flash handle).
- **CTL-13** Faders without a motor, per binding `pickup` (default) or `jump`. With `pickup` a fader
  whose position differs from its target's level by more than 2 % does nothing until it crosses
  or comes within 2 % of that level, then it follows. Pickup re-arms whenever the target changes
  under it: a page change, another client moving the level, a Go. With `jump` every move sets the
  level. Motor faders (the profile says so) never use pickup.
- **CTL-14** Input never blocks the tick: decoded actions reach the tick through the IPC command
  channel (IPC-07); several values for one input within one tick are coalesced to the last (button
  presses and releases are never coalesced away). A flood of 2000 messages per second keeps 40 Hz
  output (test with a virtual port).
- **CTL-15** Malformed or truncated messages are ignored and counted per device; the first one per
  device and per minute is logged. Nothing a device sends can stop the engine.
- **CTL-16** A device that disappears goes `Missing` with one warning (`BCF2000 disconnected`);
  its bindings are kept. When it comes back it is attached again within 2 s with the same mapping,
  and its full feedback is sent (CTL-21).

### Feedback out
- **CTL-17** Every binding on a device with an output, and every OSC binding, has feedback on by
  default to the same element (per binding `feedback: false` turns it off; per binding another
  element may be named, for devices whose LED note differs from the button note):
  - continuous actions: the target's current level (fader motor, CC value, OSC float);
  - buttons: LED on while the target is on: `upper`/`lower` per the handle button's state
    (Flash held, Swop held, Latch on, chase step; CUE-25), `go` while the connected list runs a
    transition, `page <n>` on the current page, `dbo` while on, `key` keys while their state is on
    (Highlight, Locate held), `attribute_group` on the active group, `group` while all its fixtures
    are selected;
  - `wheel`: the wheel's value as CC (0..127) or ring (Mackie Control), off when the wheel does
    not exist.
- **CTL-18** LED values per binding `on_value`/`off_value` (default 127/0), so pad controllers can
  show colours by velocity. Example: an APC-style pad with `on_value 5` lights red.
- **CTL-19** Feedback is sent after the tick's frame, only when the value changed, at most once
  per element per tick.
- **CTL-20** No echo of an input's own move: a value set by an input in this tick is not sent back
  to that input; a touched motor fader receives no position until it is released; at release it
  receives the engine level once (so a refused or clamped move snaps back).
- **CTL-21** After a page change every handle-relative binding sends its new target's state in the
  next tick: motor faders move to the new page's levels, LEDs follow; an empty slot sends level 0
  and LEDs off. On attach, reattach, mapping change and a new OSC client the full feedback state
  of that device or client is sent.

### Mappings and Learn
- **CTL-22** A mapping file is readable JSON:
  `{"name":"nanoKONTROL2","device":"nanoKONTROL2*","bindings":[{"input":{"kind":"cc","channel":1,"number":0},"action":"fader 1","pickup":true},{"input":{"kind":"note","channel":1,"number":41},"action":"go","on_value":127}]}`.
  `device` is a name or a pattern with `*`; several patterns are separated by `|` (`*BCF2000*|*X-Touch*`). A binding that does not parse is left out with one
  warning naming it; the rest loads. Unknown fields are kept on save.
- **CTL-23** Mappings live per Mac in `~/Library/Application Support/ColourmotionDMX/control/`
  (with `--show-dir`, `<show-dir>/control/`), one file per mapping, written atomically on every
  change. They are not show data: opening another show keeps them. Which mapping each device uses,
  and whether it is enabled, is stored in the daemon's `state.json` (SHW-09). Export is the file;
  `control.mapping_import` accepts a file's JSON and refuses a name that exists unless `replace`.
- **CTL-24** A device seen for the first time is listed `Disabled` and does nothing until it is
  enabled; when enabled it gets the profile whose `device` pattern matches its name, else an empty
  mapping. The choice is remembered for that device name.
- **CTL-25** Learn: `control.learn {action}` arms Learn; the next qualifying input from any enabled
  device (or OSC client) is bound to that action in that device's mapping, and Learn ends with
  `Learned: BCF2000 CC 7 ch 1 → fader 3`. Qualifying: a note or program press; a CC or pitch bend
  that moved by at least 3 steps (jitter does not learn); fader touch and release never. Arming
  again replaces the armed action; `control.learn_cancel` or Esc in the app ends it.
- **CTL-26** Learn guesses the binding: note → button; pitch bend → continuous 14-bit; CC n with
  CC n+32 arriving in the same packet → `cc14`; a CC whose values stay within 1..15 or 113..127
  → relative `twos_complement`, around 64 → `binary_offset`; any other CC → continuous. The guess
  is shown and can be changed in the bindings table.
- **CTL-27** One input does one action within a mapping: learning an input that is already bound
  replaces the old binding and says so (`CC 7 was fader 2`). An action may have several inputs.
- **CTL-28** For learning and diagnosis the snapshot and a `control` delta carry `last_input`
  (device, input text, value), at most 10 updates per second.
- **CTL-29** Control configuration (mappings, enabled devices, OSC settings) is not undoable and
  not in the show; its commands are class neutral (UNDO-04).

### Profiles, Mackie Control the first
- **CTL-30** Profiles ship with the app as mapping files, read-only, and are listed with the user
  mappings. Phase 1 ships one: **Mackie Control** (`device` patterns for the BCF2000, X-Touch,
  X-Touch Extender, iCON), with the Mackie Control driver (ADR-003). Other devices are learned;
  more profiles are added when someone owns the device to verify them.
- **CTL-01..04** (unchanged, tested) Mackie Control wire format: CTL-01 fader pitch bend per
  channel decodes to a 10-bit level, master on channel 9; CTL-02 fader position encodes back and
  round-trips; CTL-03 button notes map to the four channel rows, V-Pot push, Bank ◀▶, Channel ◀▶
  and fader touch; CTL-04 V-Pot relative CC decodes to a signed delta, ring LEDs encode on CC 48+n.
- **CTL-31** Fader window: a Mackie Control surface of W faders (8 per unit; extenders listed in
  the device order the user sets, adding 8 each) shows slots `offset + 1 … offset + W` of the
  current page, offset 0 at start. `faders_per_page` is read from the settings, never assumed 8:
  - 8 per page, one BCF2000: faders 1–8 = slots 1–8;
  - 10 per page, 8 faders: slots 1–8, Channel ▶ shows slots 9–10 on faders 1–2 and faders 3–8 are
    unbound;
  - 16 per page, BCF2000 + one extender: slots 1–16 at once;
  - 6 per page, 8 faders: faders 7–8 unbound.
  An unbound fader is driven to 0, its LEDs are off and its input does nothing.
  The order is set by `control.order {devices: [name, …]}` (VS7-04): units in the list come
  first in that order, the others after them by name; kept in `state.json` under `control.order`
  and shown as the snapshot's `control.order`.
- **CTL-32** Mackie Control navigation: Bank ◀/▶ = `page prev`/`page next` and reset the offset to
  0; Channel ◀/▶ move the offset by W within the page (stopping at 0 and at the last window).
  Channel ◀/▶ are the actions `channel prev`/`channel next` (added to the CTL-05 catalogue; they
  move the window of the Mackie Control surfaces, send no engine command and do nothing on a mapping
  without the driver). Any page change, from the surface or elsewhere, resets the offset to 0.
- **CTL-33** Mackie Control default bindings: fader n → `fader` of window slot n (motor, CTL-20/21);
  master fader → `gm`; REC row → `upper`, SELECT row → `lower` of window slot n; V-Pot n →
  `wheel n` with the ring showing the wheel's value (single-dot mode, CTL-04); V-Pot push n →
  `key step_next` on n = 1, unbound otherwise; transport Play (note 94) → `go`, Stop (93) → `stop`, Rewind (91) → `back`. Faders read and move
  as 10-bit values (CTL-01/02): full travel is exactly 1.0. Fader touch (notes 104..112) only holds
  the motor (CTL-20) and is never an input or learned. The profile is the file
  `subprojects/engine/profiles/mackie-control.json` (`"driver": "mackie_control"`), built into the
  daemon. Extenders: until the app sets the order (VS7-04), the units are ordered by device name.
  Every binding can be relearned in a user copy. What the BCF2000's four right-hand buttons and
  encoder-group buttons send in MC mode is filled in by `tests/hardware/bcf2000.md` before VS1-07
  closes.

### OSC
- **CTL-34** OSC over UDP: OSC 1.0 messages and bundles (bundle elements run in order at once;
  time tags are ignored); argument types `i`, `f`, `d`, `h`, `T`, `F`, `s` (numbers accepted as any
  numeric type). Malformed packets are ignored and counted (CTL-15).
- **CTL-35** OSC is off by default. Settings: `enabled`, `port` (default 8000) on all interfaces,
  `feedback_port` (default 9000), `allow` (empty = any sender; else a list of IPv4 addresses or
  subnets, other senders ignored and counted), `clients` (`IP:port` feedback clients, CTL-37) and
  `mapping` (the OSC mapping for custom addresses, CTL-38; default `OSC`, created by the first
  Learn). Kept in `state.json` under `control.osc` (CTL-23). A port that cannot be opened is one
  warning and the snapshot's `osc.error`; the console keeps running (VS7-03).
- **CTL-36** The built-in address schema, under `/cmdmx`, each address an action of CTL-05:

  | Address | Args | Action |
  | --- | --- | --- |
  | `/cmdmx/fader/<slot>` | f 0..1 | `fader <slot>` |
  | `/cmdmx/fader/<page>/<slot>` | f | `fader <page>.<slot>` |
  | `/cmdmx/playback/<id>/level` | f | `playback <id> level` |
  | `/cmdmx/fader/<slot>/upper`, `/lower` | 1/0 | `upper`/`lower` |
  | `/cmdmx/go`, `/back`, `/stop`, `/release`, `/tap` (and `/cmdmx/playback/<id>/go` …) | none or 1 | the button action |
  | `/cmdmx/page` | i | `page <n>` |
  | `/cmdmx/page/next`, `/cmdmx/page/prev` | none or 1 | `page next/prev` |
  | `/cmdmx/gm` | f | `gm` |
  | `/cmdmx/dbo` | 1/0 | `dbo` |
  | `/cmdmx/key/<name>` | none or 1/0 | `key <name>` |
  | `/cmdmx/keypad/<token>` | none or 1 | `keypad <token>` |
  | `/cmdmx/wheel/<n>` | i or f steps | `wheel <n>` |
  | `/cmdmx/attribute_group/<g>`, `/cmdmx/group/<id>`, `/cmdmx/palette/<id>` | none or 1 | the button action |

  Buttons: a non-zero argument is press, 0 is release (TouchOSC sends 1.0 and 0.0); no argument
  is press and release at once. An unknown `/cmdmx` address is ignored and counted.
- **CTL-37** OSC feedback goes to every sender heard in the last 10 minutes, at its IP on
  `feedback_port`, and to the clients listed in the settings (`host:port`). It uses the same
  addresses with the current value (CTL-17), plus `/cmdmx/fader/<slot>/legend` s,
  `/cmdmx/page` i, `/cmdmx/page/legend` s, `/cmdmx/cue/current` s and `/cmdmx/cue/next` s
  (number and legend of the connected list) and `/cmdmx/message` s (the last engine warning).
  CTL-19..21 apply; a new client gets the full state. Echo (CTL-20) is per sender IP: the client
  at that IP does not get the value back, the other clients do. The built-in feedback covers the
  slots of the current page (`fader/<slot>` f, its `upper`/`lower` 1.0/0.0 and `legend`), `page`,
  `page/legend`, `gm`, `dbo`, `go`, `key/highlight`, `attribute_group/<g>`, the cue texts
  (`"<number> <legend>"`, the number alone without a legend, `""` without a list) and `message`
  (VS7-03). OSC moves (built-in and custom addresses) never pick up (OSC controls follow their feedback like
  motor faders, CTL-13).
- **CTL-38** Custom addresses: an OSC mapping binds any address (for example an existing TouchOSC
  layout's `/1/fader1` or `/2/push3`) to an action, learned like MIDI (CTL-25: the next address
  received binds), with feedback to the same address. The built-in schema always works next to it.
  So a layout made for another program works without editing the layout. Learn takes the next
  address outside `/cmdmx` (those keep acting while Learn is armed); the message is
  `Learned: OSC /1/fader1 → fader 1` and a learned OSC fader is saved as `jump`. Button
  feedback is 1.0/0.0 (VS7-03).

### IPC and the app
- **CTL-39** Snapshot gains `control`: `devices` (`[{"name":"BCF2000","input":true,"output":true,
  "state":"Ok","enabled":true,"mapping":"Mackie Control","errors":0}]`), `mappings` (names, with
  `profile: true` for shipped ones), `osc` (the CTL-35 settings, `clients` with address and last
  heard, `errors`; VS7-03: `clients` lists every feedback client as
  `{"address":"192.168.1.20:9000","last_heard":12.5,"configured":false}`, `last_heard` in daemon
  seconds or `null` for a configured client not heard, beside `mapping` and `error`), `learn` (the armed action or `null`) and `last_input` (CTL-28). A `control`
  delta follows every change, `last_input` at most 10 per second.
- **CTL-40** Commands (refusals are warnings, IPC-06; all class neutral, CTL-29):
  `control.enable {device, on}` · `control.assign {device, mapping}` ·
  `control.learn {action, mapping?}` · `control.learn_cancel` · `control.mapping {name}` (returns
  its bindings to the asking client) · `control.binding {mapping, input, action?, …options}`
  (edit or add) · `control.unbind {mapping, input}` · `control.mapping_new {name, device}` ·
  `control.mapping_rename` · `control.mapping_delete` (not a profile) · `control.mapping_import
  {json, replace?}` · `control.osc {enabled?, port?, feedback_port?, allow?, clients?, mapping?}` ·
  `control.order {devices}` (CTL-31). An action
  string outside the catalogue is refused (`unknown action "fader x"`).
- **CTL-41** App: a **Control** screen on the workspace rail next to Output: a device table
  (name, state dot, Enabled, mapping dropdown), an OSC panel (CTL-35 settings, clients heard, error
  count), and the bindings table of the selected mapping (input, action, options editable in
  place, Learn and Remove per row, Add, Export/Import). The last input is shown live at the top.
- **CTL-42** Learn from the main screen: the Control screen's **Learn** button (and softkey) puts
  the app in learn mode; then clicking a fader, handle button, Go block key, page button, GM or
  programmer key arms `control.learn` with that control's action instead of using it, and the
  message zone says `Move a control for fader 3 … (Esc cancels)`. Learned, the app stays in learn
  mode for the next control until Esc or the Learn button again.
- **CTL-43** Status bar: a `Control` dot with the worst state of the enabled devices and OSC
  (Missing = amber); hover lists those not Ok.

## Where the code lives
- `cmdmx-control` (pure, no ports, no clock): MIDI decode (CTL-09..12), mapping model and JSON
  (CTL-22), action catalogue and parsing (CTL-05), conversion and pickup (CTL-07, 13), feedback
  diffing (CTL-17..21), the learn guess (CTL-26), the Mackie Control driver (CTL-01..04, 31..33)
  and an own OSC 1.0 codec (CTL-34; small, byte-exact tested like the Art-Net framing).
- `cmdmx-daemon`: CoreMIDI through `midir` (planned in ARCHITECTURE), hotplug polling, the UDP
  socket, mapping files and `state.json`, turning actions into commands on the command channel,
  sending feedback after each tick, the `control.*` commands.

## Tests
Unit tests in `cmdmx-control` carry the criterion ids (`ctl_11_twos_complement_decodes_minus_3`);
the daemon gets a test with a virtual CoreMIDI port and a loopback OSC socket (CTL-14, 16, 36, 37);
hardware lines stay in `tests/hardware/bcf2000.md` (gate 3) and a new `tests/hardware/touchosc.md`
for the church remote (VS4-04).

## Out of scope (phase 1)
MIDI Show Control, MIDI clock and MTC (cue triggers and sync: IDEA-16 and timecode, later phases,
owner decision 7); MIDI or OSC sent by cues (macros); NRPN; Mackie
Control scribble strips and the 7-segment display (X-Touch legends, later task); HUI; the own web
remote (IDEA-06, phase 2); DMX-in and audio triggers (Titan has them; later phase); OSC over TCP.

## Owner decisions (P1-05, 2026-10-23: 1–7, 9, 10 as proposed; 8 answered)
1. Mappings live per Mac in `Application Support/…/control/`, exportable as a file; not in the show (CTL-23).
2. A new device is listed disabled until it is enabled once (CTL-24).
3. Faders without motors use Pickup by default, Jump per binding (CTL-13).
4. OSC is off by default; when on, any sender unless `allow` is set (CTL-35).
5. Prefix `/cmdmx`, listen port 8000, feedback port 9000 (CTL-35/36).
6. Phase 1 ships only the Mackie Control profile; other devices are learned (CTL-30).
7. MIDI Show Control is phase 2 (with IDEA-16 and timecode).
8. The church TouchOSC layout runs on a very old iPad mini; the owner does not know yet how to
   get the `.tosc` out. VS4-04 tries to export it and bind its addresses (CTL-38); if that fails,
   a new TouchOSC layout is built on the `/cmdmx` schema (CTL-36).
9. Mackie Control defaults as in CTL-33.
10. `midir` for CoreMIDI and an own OSC 1.0 codec in `cmdmx-control` (no `rosc`).

## Build tasks (`BACKLOG.md`, sized after approval)
- **VS7-01** `cmdmx-control`: generic MIDI decode, mapping model and JSON, action catalogue,
  conversions, pickup, feedback diffing, learn guess, OSC codec (CTL-05, 07, 09..13, 17..20, 22,
  26, 27, 34, 36).
- **VS7-02** Daemon: CoreMIDI devices through `midir`, hotplug, mapping files and `state.json`,
  actions on the command channel, feedback after the tick, Learn, `control.*` over IPC (CTL-06, 08,
  14..16, 21, 23..25, 28, 29, 39, 40). Needs VS7-01.
- **VS1-07** (re-scoped) Mackie Control profile on the generic layer: window and banks from
  `faders_per_page`, default bindings, motor and touch rules; BCF2000 checklist (gate 3) (CTL-01..04,
  30..33). Needs VS7-02.
- **VS7-03** Daemon: OSC socket, built-in schema, feedback clients, custom addresses and their
  Learn (CTL-35..38). Needs VS7-02.
- **VS7-04** App: Control screen, learn mode on the main screen, status-bar dot; manual run
  (CTL-41..43). Needs VS7-02.
- **VS4-04** (re-scoped) The church TouchOSC layout as an OSC client: its addresses bound with
  CTL-38, `tests/hardware/touchosc.md` run in the church (gate 3). Needs VS7-03 and VS7-04.
