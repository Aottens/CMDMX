# Module spec: control-surfaces (CTL)

Owner: `cmdmx-control` (pure: MIDI and OSC decode/encode, mappings, pickup, feedback, learn) and
`cmdmx-daemon` (CoreMIDI ports, the OSC socket, Stream Decks over USB HID, mapping files, IPC). Status: **CTL-05..43
approved by owner 2026-10-23** (P1-05; the ten questions answered, see Owner decisions); **CTL-44..58 (Stream Deck) approved
by owner 2026-10-25** (P1-19; the nine questions as recommended); **CTL-59 (Fan) approved by owner 2026-10-25** (P1-20); **CTL-60..66 (TouchOSC layout) approved by owner 2026-10-25** (VS4-04); **CTL-58 details and CTL-67 approved by owner 2026-10-25** (VS7-07); **CTL-58a/b approved by owner 2026-10-25** (VS7-09). CTL-01..04 (the Mackie Control wire format) are kept from slice 1
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
  A Stream Deck is a device too (CTL-44).
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
  | `fan`, `fan <line\|mirror\|wings\|arrow>`, `fan off` (CTL-59) | button | `programmer.fan` |
  | `group <id>`, `palette <id>` | button | select the group / apply the palette as its window button does |

  New actions are added to this table by later criteria, never as free-form commands.
- **CTL-59** Fan (P1-20, PRG-56). `fan` toggles Fan as the Fan key does, `fan <curve>` turns it on
  with that curve, `fan off` turns it off (IPC-43). A `wheel <n>` binding needs nothing new: Fan
  is engine state, so encoder turns fan while it is on. Its feedback is on while `fan` is not
  `null` (`fan <curve>`: while that curve is on), like the `highlight` key LED.
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

### The shipped TouchOSC layout (VS4-04; approved by owner 2026-10-25)
A remote for the church iPad mini (1st gen, iOS 9.3.5, TouchOSC Mk1 1.9.8, held landscape),
made for ColourmotionDMX on the built-in schema (CTL-36/37). It is a client like any other: no
mapping, no Learn, nothing in the daemon changes.
- **CTL-60** The repo ships `touchosc/ColourmotionDMX.touchosc`: a TouchOSC Mk1 layout (a zip
  holding `index.xml`) for the iPad size (1024×768) in landscape, less a 48 px strip on the left
  for TouchOSC's own toolbar (amended 2026-10-25: the VS4-04 run found the right column cut
  off without it), one tab, every control on a
  `/cmdmx` address (TouchOSC's automatic `/<tab>/<name>` addresses off). It is written by
  `touchosc/generate.py` (Python 3, standard library only) and checked in; the same input gives
  the same bytes.
- **CTL-61** Fader strips: the generator takes the fader count (`--faders N`, default 8 =
  the default `faders_per_page`; never a fixed 8 in the layout code) and draws N strips of equal
  width, slot n top to bottom: the legend label (`/cmdmx/fader/<n>/legend`), the upper button
  (`/cmdmx/fader/<n>/upper`), a vertical fader 0..1 (`/cmdmx/fader/<n>`) and the lower button
  (`/cmdmx/fader/<n>/lower`). Buttons are momentary (1.0 press, 0.0 release) and light from the
  feedback (CTL-37). A show with another `faders_per_page` gets its layout by running the
  generator with that number (README line).
- **CTL-62** Top bar, left to right: `Page ◀` (`/cmdmx/page/prev`), the page number label
  (`/cmdmx/page`), the page legend label (`/cmdmx/page/legend`), `Page ▶` (`/cmdmx/page/next`),
  then the message line (`/cmdmx/message`) over the rest of the width.
- **CTL-63** Right column, top to bottom: the current cue label (`/cmdmx/cue/current`), the next
  cue label (`/cmdmx/cue/next`), a large `GO` (`/cmdmx/go`, lit by its feedback), `BACK`
  (`/cmdmx/back`) and `STOP` (`/cmdmx/stop`) side by side, `DBO` (`/cmdmx/dbo`) and a vertical
  `GM` fader (`/cmdmx/gm`). DBO is a toggle button (tap on, tap off; it sends 1.0/0.0, which the
  held `dbo` action of CTL-05 reads as press and release) because holding a glass button through
  a blackout is impractical; the feedback keeps it in step with the console.
  **Amended 2026-10-25 (VS4-06), approved by owner 2026-10-25:** the two cue labels must not be
  mistaken for each other at arm's length (the VS4-04 run mixed them up). The current cue label
  is large and marked: 70 px high, text size 28, green, with background and outline. The next cue
  line is small and grey: 30 px high, a static `Next:` label (no address, size 14, grey, no
  background or outline) followed on the same line by the next cue label (size 14, grey, no
  background or outline). The 20 px this adds comes off the GM fader. The texts stay the daemon's
  CTL-37 strings unchanged; `Next:` lives in the layout, because a Mk1 label shows either its own
  text or the received one, and other OSC clients should not get a prefix they did not ask for.
  A legend too long for the label is cut off by TouchOSC; that is accepted. Checked by eye in
  `tests/hardware/touchosc.md` (CTL-66); CTL-64 is unaffected, since the `Next:` label has no
  address.
  Titan reference (manual.avolites.com, *Cue list playback*, v13 and v18): the current cue is
  highlighted and the next cue has a box round it; the Connected Cue List view shows the current
  cue on top in light blue and the next cue below it in dark blue. Same idea here: the live cue
  stands out, the next cue is there but quieter.
- **CTL-64** One test fails when the layout and the schema drift: every address the layout sends
  decodes with the engine's CTL-36 decoder (`osc::schema`), and every address it shows (labels,
  lit buttons, faders) is one the daemon's CTL-37 feedback sends for a page of N slots. The
  generator also has `--check`, which fails when the checked-in `.touchosc` differs from a fresh
  run; CI runs it.
- **CTL-65** `touchosc/serve.sh` loads the layout on the iPad without the TouchOSC Editor: it
  announces the Mac over Bonjour as the editor's Sync does (`dns-sd -R`) and answers HTTP GETs
  with the bare `index.xml` from the layout until Ctrl-C (amended 2026-10-25: Mk1 1.9.8 accepts
  the zip's download but silently keeps the old layout), so TouchOSC → Layout → Add lists the Mac under Found Hosts. If Mk1 does not
  accept it, the fallback is the TouchOSC Editor (Java): open the file, press Sync. Both ways are
  in `tests/hardware/touchosc.md`.
- **CTL-66** `tests/hardware/touchosc.md` (gate 3, church iPad): load the layout, point TouchOSC
  at the Mac (host, port 8000, receive port 9000), work every control and see it on the console,
  check the feedback (a page change moves the faders and rewrites the legends; a cue Go rewrites
  the cue texts, and from the operator's seat the current cue reads as the large green one and the
  next as the small grey `Next:` line, VS4-06; a warning shows on the message line), and at the end restore the iPad's Lightkey
  settings (MIDI Bridge, OSC host 192.168.178.36:21600) and its old layout.

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

### Stream Deck (native USB HID; IDEA-22, P1-19, ADR-009)
An Elgato Stream Deck is a device on this layer like a MIDI device: its keys, dials and touch
areas are inputs, bound to CTL-05 actions in a mapping, learned (CTL-25) and given feedback. Its
feedback is a **key image** instead of an LED or a motor.

- **CTL-44** Route: the daemon drives Stream Decks itself over USB HID (`hidapi`, ADR-009) on its
  own `deck` thread, as MIDI runs on the `midi` thread (CTL-14). No Elgato app, plugin or driver,
  no admin rights. A deck is listed in the CTL-08 device table with `kind: "streamdeck"`; its name
  is the model name and the last 4 characters of its USB serial number
  (`Stream Deck XL (A1B2)`), so two decks of one model keep their own mapping whichever port they
  are in. A deck without a serial number falls back to CTL-08's `name #2` numbering.
- **CTL-45** Models are configuration, never code: a model table
  `subprojects/engine/profiles/streamdeck-models.json`, built into the daemon, holds per USB
  product id (vendor `0x0fd9`): name, `cols`, `rows`, key image size in pixels, image format
  (`jpeg` or `bmp`), rotation and flip, the report family (the packet layout), `verified`, and
  extras: `dials`, `touch_keys`, `strip` (w × h) and `info` (w × h). Nothing in the code assumes
  15 keys, 72 px or one layout. The phase-1 table: Original (15 keys, 72 px BMP), Original V2 and
  MK.2 (15, 72 px JPEG), Mini and Mini MK.2 (6, 80 px BMP), XL and XL V2 (32, 96 px JPEG),
  Neo (8, 96 px JPEG; 2 touch keys; info screen 248 × 58), Stream Deck + (8, 120 px JPEG; 4 dials;
  touch strip 800 × 100), Pedal (3 pedals, no images). A model passed in a gate-3 run is
  `verified: true`; the others are listed as `untested` in the device table but work the same. An
  Elgato product id not in the table is listed `Unsupported` with one warning and is not opened.
- **CTL-46** The Elgato app and other programs: a deck seen for the first time is `Disabled`
  (CTL-24) and is **not opened**, so another program can keep it. Enabled, the daemon opens it
  exclusively. When another program holds it, the deck is `Busy` with one warning
  (`Stream Deck XL (A1B2) is in use by another app; quit the Elgato Stream Deck app`), the daemon
  tries again every 2 s, and the deck becomes `Ok` once it is free. Disabling a deck blanks its
  keys and closes it at once. `Busy` and `Unsupported` count as not Ok for the status dot (CTL-43,
  amber). **Amended 2026-10-25 (VS7-07 run):** the warning comes once per Busy spell: the deck
  going `Ok` or being enabled again re-arms it.
- **CTL-47** Inputs, numbered as printed on the model, left to right and top to bottom from 1:
  `{"kind":"key","number":n}` per key or pedal (button, CTL-12 modes apply);
  `{"kind":"touch","number":n}` per Neo touch key (button); `{"kind":"dial","number":n}` per
  Stream Deck + dial (relative, signed steps per report, CTL-07 relative rules) and
  `{"kind":"dial_push","number":n}` (button); `{"kind":"strip","number":n}`, a short tap on the
  n-th of `dials` equal segments of the touch strip (press and release at once). Long press and
  drag on the strip are ignored in phase 1. Input text in messages: `Stream Deck XL (A1B2) key 5`.
  Reports that do not parse are ignored and counted (CTL-15).
  **Amended 2026-10-25 (owner, VS7-07 run):** a continuous action (`gm`, `fader <t>`,
  `playback <id> level`) on a key, touch key, dial push or strip segment only shows (CTL-50: the
  level in the top bar); a press sends no command, so a key can never pull the GM to 0.
- **CTL-48** Deck pages: a deck mapping holds bindings in the CTL-22 format; a key binding may
  carry `"deck_page": n` (default 1). New actions in the CTL-05 catalogue: `deck page next`,
  `deck page prev`, `deck page <n>` (button; no engine command; they switch the pages of the deck
  that sent them and do nothing from any other device). A deck has as many pages as the highest
  `deck_page` in its mapping; `next` on the last and `prev` on page 1 do nothing. The deck page is
  per deck, starts at 1 and is not saved. Changing the deck page sends that page's full feedback
  (CTL-21). A "folder" is a key bound to `deck page <n>` with a key on page n bound back.
  Handle-relative actions (`upper 3`, `fader 3`) follow the console page as on every device
  (CTL-21); the deck page only chooses which bindings the keys show.
- **CTL-49** Default deck mapping: a deck enabled without a mapping gets a generated read-only
  profile `Stream Deck <model>` (CTL-30 rules: editing saves a user copy). With K keys: when
  K ≥ 8 the last three keys are `page prev`, `page next`, `go`, else the last key is `go`; the
  keys from 1 get `upper 1 … upper n`, n = the smaller of the keys left and `faders_per_page`
  (never assumed 8); other keys stay unbound. Pedal: 1 `back`, 2 `go`, 3 `stop`. Neo touch keys:
  `deck page prev`, `deck page next`. Stream Deck + dials: `wheel 1..4`, dial pushes unbound.
  Examples: MK.2 (15) with 10 per page: keys 1–10 `upper 1–10`, 11–12 unbound, 13–15 page prev,
  page next, go; Mini (6) with 8 per page: keys 1–5 `upper 1–5`, key 6 `go`.
- **CTL-50** Key image, the feedback kind of a key binding (on by default, `feedback: false` makes
  the key black). The daemon builds a **key look** per key from the state CTL-17 already knows:
  - **playback actions** (`upper`, `lower`, `fader`, `go`, `back`, `stop`, `release`, `tap` on a
    slot or playback): a top bar in the playback's halo (PBK-33; no halo, no bar), the legend
    (PBK-31; empty legend: `PB <id>`), under it the button's function in small text (`Flash`,
    `Swop`, `Go`, … as CUE-25 sets the handle button, or the action word), a level bar along the
    bottom with the playback's level, and the state: **active** (level > 0 or running) at full
    brightness, inactive dimmed to 40 %, **held** (Flash or Swop held through this key) inverted.
    An empty slot is a black key.
  - **other actions**: a label (`Go`, `Page 2`, `Clear`, `GM`, `Deck 2`; a group or palette its
    name) and the CTL-17 LED state as active/inactive; continuous actions (`gm`, `playback <id>
    level`) also show the level bar.
  - per binding `label` replaces the legend and `colour` (sRGB) replaces the halo.
  - an unbound key is black.
  The legend wraps over at most 3 lines and the text shrinks to fit the model's key size; what
  still does not fit ends in `…`.
  **Amended 2026-10-25 (owner, VS7-07 run; a tilted deck hides the bottom edge):** there is one
  bar, on top: with a level it fills from the left to the level in the halo colour (white without
  a halo) and the rest of the bar is the halo colour at 25 % (no halo: grey 40); without a level
  it is the halo as before. Nothing is drawn along the bottom. A key whose action does not follow
  the console page (a `<page>.<slot>` target or `playback <id>`) is **fixed** and shows a small
  lock in its top-right corner under the bar; the look carries `fixed` (CTL-57 adds
  `"fixed": true|false` per key).
- **CTL-51** Rendering is pure: `cmdmx-control` turns a key look and a model entry into the image
  bytes the model wants (size, rotation, flip, JPEG or BMP), with one font built in (open licence,
  ADR-009). Same look and model, same bytes (golden tests per image size). The tick only compares
  looks; rendering and USB writes happen on the deck thread.
- **CTL-52** Image feedback timing: CTL-19..21 apply with the key look as the value. The level in
  a look is rounded to the level bar's pixel width, so a slow fade changes a key only when the bar
  moves. The deck thread keeps only the newest look per key: a deck that cannot keep up skips
  intermediate images, never queues them, and the tick never waits for USB. Full feedback on
  attach, reattach, mapping change, deck page change and console page change (CTL-21).
- **CTL-53** Brightness per deck, 0–100 (default 70), kept in `state.json` under the deck's
  `control.devices` entry and set by `control.deck {device, brightness}` (added to CTL-40, class
  neutral). Disabling a deck or quitting the daemon blanks its keys first.
- **CTL-54** Show-proof: an unplugged deck goes `Missing` (CTL-16), its mapping kept; plugged back
  in it is `Ok` within 2 s with its full images. A failed write counts as unplugged. A panic in the
  deck thread is caught, logged once, the decks go `Missing` and the thread starts again; MIDI,
  OSC and output keep running.
- **CTL-55** Stream Deck + and Neo extras: the touch strip shows one segment per dial with the
  label and value of what that dial is bound to (a wheel: its attribute name and value; a level:
  the legend and level); the Neo info screen shows the current page (`2 Service`) and the next cue
  of the connected list (`"<number> <legend>"`, as CTL-37). Both follow CTL-52's timing.
- **CTL-56** Learn: a deck key, touch key, dial push or strip tap qualifies at once, a dial after
  one step (CTL-25). It binds on the deck's current deck page:
  `Learned: Stream Deck XL (A1B2) key 5 → upper 3`. CTL-27 applies per deck page.
- **CTL-57** IPC: a deck in the snapshot's `control.devices` adds `kind`, `model`, `verified`,
  `cols`, `rows`, `extras` (`dials`, `touch_keys`, `strip`, `info`), `deck_page`, `deck_pages`,
  `brightness` and `keys`: the looks of the current deck page
  (`[{"key":5,"label":"Wash","sub":"Flash","halo":"#ff3000","level":0.5,"state":"active"}]`), so
  the app draws the deck itself. States may be `Busy` and `Unsupported`. A `control` delta carries
  changed looks, at most 10 per second per deck (CTL-39).
- **CTL-58** App: selecting a deck in the Control screen shows it as the model draws it (`cols` ×
  `rows` keys, the Neo touch keys and info screen, the + dials and strip), each key drawn from its
  look, with a deck page selector (`+` adds a page) and the brightness. Linking:
  - **drag** onto a key: a playback handle → `upper <slot>` (follows the page; with ⌥ held
    `upper <page>.<slot>`, fixed); a Go block key → `go`/`back`/`stop`/`release`; a page button →
    `page <n>`; a group or palette button → `group <id>`/`palette <id>`; the GM → `gm`;
    a key onto another key moves the binding (swaps when the other key is bound). Dropping on a
    bound key replaces it and says so (`key 5 was go`, CTL-27);
  - **right-click** a key: the action menu of the bindings table, Label, Colour, Remove;
  - **Learn** (CTL-42): click a console control, then press the deck key — or click the key on
    the on-screen deck (amended 2026-10-25, owner, VS7-07 run): that binds the armed action to
    that key on the deck page shown, cancels the armed `control.learn` and says
    `Learned: <deck> key n → <action>`; Learn mode stays on as CTL-42.
  The bindings table (CTL-41) lists deck bindings too, with their deck page.
  Details (VS7-07, approved by owner 2026-10-25): the deck page selector, labelled `Deck page`, shows pages 1..`deck_pages`;
  choosing one sets the deck's own page (CTL-67), so the screen and the deck always show the same
  page and Learn binds on it (CTL-56). `+` shows an empty page `deck_pages + 1` in the app only
  (all keys black, the deck stays where it is); the first binding dropped or learned there makes
  it a real page. Keys are drawn only from `keys` of the deck's current page, never computed in
  the app. Group and palette buttons are not on screen while the Control screen is shown, so
  they are linked through the action menu (Group ▸ / Palette ▸ with the show's groups and
  palettes); playback handles, Go block keys, page buttons and the GM below the Control screen
  are drag sources. A drop on a bound key and a move are sent as `control.unbind` then
  `control.binding` with the whole binding, so no option of the old binding stays behind; a swap
  unbinds both keys first. The Neo touch keys and info screen and the + dials and strip are drawn
  in VS7-08 (with CTL-55). A deck's `Busy` and `Unsupported` show as those words, amber.
  **Amended 2026-10-25 (owner, VS7-09; the VS7-07 run found the screen cramped and a label hiding
  a new action):**
  - **CTL-58a Focus.** Selecting a deck shows only what is being worked on: the Devices and OSC
    panels are hidden, the left column holds the Deck panel alone (keys up to 96 pt instead of
    72) and the bindings table beside it shows that deck's mapping. A `‹ Devices` button in the
    Deck panel's title bar, and Esc, deselect the deck and bring the Devices and OSC panels back.
    Selecting a MIDI device or OSC works as before (no focus).
  - **CTL-58b New action clears the look.** Choosing a different action for a bound key from the
    right-click menu (Action ▸, Group ▸, Palette ▸, Edit action…) or by Learn on the on-screen
    key also removes the binding's `label` and `colour`, so the key shows the new action's name
    and halo, as a drop already does. Choosing the action the key already has changes nothing.
- **CTL-67** IPC for the on-screen deck (VS7-07, approved by owner 2026-10-25):
  - `control.deck {device, brightness?, page?}`: `page` sets the deck's current deck page as a
    `deck page <n>` key would (CTL-48: full feedback, not saved); a page outside
    1..=`deck_pages` is refused (`deck page 4 outside 1..=3`). At least one field is given.
  - `control.binding` and `control.unbind` pick the binding by `input` and `deck_page`
    (default 1), so a key's binding on one deck page is edited or removed without touching the
    same key on another page. `control.binding` with a `deck_page` and no stored binding on that
    page adds one (it needs an `action`).

## Where the code lives
- `cmdmx-control` (pure, no ports, no clock): MIDI decode (CTL-09..12), mapping model and JSON
  (CTL-22), action catalogue and parsing (CTL-05), conversion and pickup (CTL-07, 13), feedback
  diffing (CTL-17..21), the learn guess (CTL-26), the Mackie Control driver (CTL-01..04, 31..33)
  and an own OSC 1.0 codec (CTL-34; small, byte-exact tested like the Art-Net framing); for
  Stream Decks the model table, the HID report codec (inputs, image packets, brightness), key
  looks, the key image renderer and the default deck mapping (CTL-45, 47..52, 55).
- `cmdmx-daemon`: CoreMIDI through `midir` (planned in ARCHITECTURE), hotplug polling, the UDP
  socket, mapping files and `state.json`, turning actions into commands on the command channel,
  sending feedback after each tick, the `control.*` commands; the `deck` thread through `hidapi`
  (open, Busy retry, hotplug, writes; CTL-44, 46, 53, 54).

## Tests
Unit tests in `cmdmx-control` carry the criterion ids (`ctl_11_twos_complement_decodes_minus_3`);
the daemon gets a test with a virtual CoreMIDI port and a loopback OSC socket (CTL-14, 16, 36, 37);
hardware lines stay in `tests/hardware/bcf2000.md` (gate 3) and a new `tests/hardware/touchosc.md`
for the church remote (VS4-04). Stream Deck: report decode and image packets byte-exact against
reference captures, golden images per key size (CTL-51), the deck thread against a fake HID
device (Busy, unplug, slow writes; CTL-46, 52, 54); `tests/hardware/streamdeck.md` on the
owner's deck (gate 3), which also records whether macOS asks for Input Monitoring.

## Out of scope (phase 1)
MIDI Show Control, MIDI clock and MTC (cue triggers and sync: IDEA-16 and timecode, later phases,
owner decision 7); MIDI or OSC sent by cues (macros); NRPN; Mackie
Control scribble strips and the 7-segment display (X-Touch legends, later task); HUI; the own web
remote (IDEA-06, phase 2); DMX-in and audio triggers (Titan has them; later phase); OSC over TCP;
an Elgato Stream Deck plugin (ADR-009; the Elgato app or Bitfocus Companion can already send OSC
to `/cmdmx`, CTL-36); playback icons on deck keys (later task); long press and drag on the
Stream Deck + strip; Stream Deck Studio and + XL (added to the model table when someone verifies
them).

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
   a new TouchOSC layout is built on the `/cmdmx` schema (CTL-36). **Amended 2026-10-24
   (owner):** VS4-04 builds a new layout tailored to ColourmotionDMX on `/cmdmx`; the old layout
   is not exported. CTL-38 stays for other people's layouts.
9. Mackie Control defaults as in CTL-33.
10. `midir` for CoreMIDI and an own OSC 1.0 codec in `cmdmx-control` (no `rosc`).

## Owner decisions (P1-19, Stream Deck, 2026-10-25: all nine as recommended)
1. Native USB HID from the daemon, no Elgato plugin (CTL-44, ADR-009).
2. A deck stays closed until it is enabled; enabled, it is opened exclusively and shows `Busy`
   while the Elgato app holds it (CTL-46).
3. Models as a configuration table; only the owner's model is verified on hardware, the others
   are `untested` (CTL-45). Which model the owner has is named in `tests/hardware/streamdeck.md`.
4. A dragged playback becomes its handle's `upper` button, so the key does what the handle's
   button does (Flash, Swop, Go; CTL-58).
5. Key image: halo bar, legend, level bar and state; icons later (CTL-50).
6. Handle-bound keys follow the console page; decks have their own pages, a folder is a key to a
   deck page (CTL-48).
7. Link by drag and by Learn (CTL-56, 58).
8. Stream Deck + dials and strip and the Neo extras in a separate, last build task (CTL-55).
9. The daemon renders the images (decks work without the app), with `hidapi`, a font rasterizer
   and a JPEG encoder as new dependencies (CTL-51, ADR-009).

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
- **VS4-04** (re-scoped 2026-10-24) A TouchOSC layout made for ColourmotionDMX on the `/cmdmx`
  schema (CTL-36/37), shipped in the repo, `tests/hardware/touchosc.md` run on the church iPad
  mini (gate 3). Needs VS7-03 and VS7-04.
- **VS7-05** `cmdmx-control`: Stream Deck model table, HID report codec, key inputs and deck
  pages, key looks, the image renderer, the default deck mapping (CTL-45, 47..51). Needs P1-19.
- **VS7-06** Daemon: the `deck` thread through `hidapi` (enumerate, open, Busy retry, Unsupported,
  hotplug, newest-look writes, brightness), deck inputs to commands, image feedback, Learn, the
  snapshot and `control.deck` (CTL-44, 46, 52..54, 56, 57). Needs VS7-05.
- **VS7-07** App: the on-screen deck in the Control screen, drag, right-click menu, Learn, deck
  pages; owner run and `tests/hardware/streamdeck.md` on the owner's deck (gate 3) (CTL-58, 67).
  Needs VS7-06.
- **VS7-08** Stream Deck + dials and touch strip, Neo touch keys and info screen (CTL-47 extras,
  55). Needs VS7-06; gate 3 only on a model someone owns.
