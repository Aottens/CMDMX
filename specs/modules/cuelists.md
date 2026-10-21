# Module spec: cue lists, chases and speed masters (CUE)

Owner: `cmdmx-core::playback` (cue lists, chases and masters are playbacks), `cmdmx-daemon`
(IPC, tap timestamps), the Go block, Cue list window, strips and Playback Options in
`subprojects/app`.
Status: **approved by owner 2026-10-21 (P1-08, gate 1); owner decisions at the end.**
Behavioural reference: Avolites Titan v19 manual, *Cue Lists* (https://manual.avolites.com/docs/cue-lists:
Creating, Playback, Editing, Timing, Options), *Chases* (https://manual.avolites.com/docs/chases:
Playback, Timing, Options) and *Running the Show → Playback controls* (speed masters, key
profiles), plus the Titan analysis doc linked in `PROJECT.md` ("Chases en cue lists", "Masters").
UI reference: UI v0.1 (`specs/ux/main-screen.html`: Go block, Cue list window, strip meta line
"Q3 › 4"; `research.md` design brief items 10 and 11, the Go row of the inventory). Builds on
`playbacks.md` ("What P1-08 builds on", PBK-09..23, 25..29, 34..40), `palettes.md` (PAL-13/14
fades and overlap, references), `ui.md` (UI-13, 19, 22, 23) and `showfile.md` (SHW-02, 04, 07).

## Purpose
A cue list is the theatre stack: numbered cues run one after the other with Go, each with its
own times, some following on by themselves. A chase steps through its cues by itself at a
tempo (BPM, Tap) with a crossfade. Both are **playbacks with more than one cue**: they live on
a handle like every playback (`playbacks.md`), keep its options (priority, release, groups,
paging, legend, halo), and add Go/Back/Stop, cue numbers, links and the chase clock. The church
runs a service as one cue list (walk-in, welcome, worship, prayer, sermon, video, walk-out)
next to the front spots on their own fader, with one big Go always in the same place.

Cue lists **track** in phase 1 (owner, 2026-10-21, pulled forward from phase 2): a cue stores
only touched attributes (PBK-10), and what it does not hold carries on from the cues before it
(CUE-50), as on Titan. Block, Cue Only and Solo per cue and the Update modes come with it
(CUE-51, 52). A list with tracking off, and every chase, behaves as CUE-13 says for a cue's own
values.

## Definitions
- **Memory, cue list, chase:** a playback of kind `Cues` with one cue is a memory and behaves
  exactly as `playbacks.md` says. A playback of kind `Cues` with two or more cues is a **cue
  list**. A playback of kind `Chase` is a **chase**; its cues are its **steps**. "List" below
  means a cue list or a chase.
- **Speed master:** a playback of kind `BpmMaster` or `RateMaster`: a handle without cues that
  sets the tempo of chases that follow it (CUE-35..37). It never enters the merge.
- **Cue number:** a decimal 0.001–9999.999 with at most three decimals, unique within its
  playback, written "Q2.5". Cues are kept in number order; that is the order Go runs them.
- **Current cue:** the cue a running list last started (by Go, a link, Go to cue or Back), `none`
  while the list is off. **Next cue:** the cue Go will start: the one after the current cue in
  number order, the first cue while the list is off, or the cue the operator set (CUE-19).
- **Cue state:** what a cue puts on stage, fixture by fixture and attribute by attribute (CUE-50):
  with tracking, its own (**hard**) values plus the values **tracked** from the cues before it;
  without tracking, its own values only.
- **Transition:** what starting a cue does to the output (CUE-13). Its **duration** is the
  longest delay + fade (overlap and group times included) over the attributes the transition
  fades, HTP fading out included (owner, 2026-10-21: a cue that changes nothing is complete at
  once); its **progress** runs from 0 to 1 over that duration (1 at once when it is 0).
- **Connected list:** the one list the Go block, Space and the Cue list window act on (CUE-26).
- **Effective BPM / step time:** a chase's tempo after its speed source (CUE-29); one step lasts
  *T* = 60 / effective BPM seconds.
- Timing is as in `playbacks.md`: counted from the tick in which the action reaches the engine,
  from the tick's timestamp only (ENG-11). Tap is the one exception (CUE-33).
- Church examples: the service list on handle 1.1 with the sketch's seven cues, front spots on
  1.8 `Locked` (PBK-07), demo rig of `programmer.md` (pars 201–226, washes 101–104, spots
  301–306).

## Acceptance criteria

### Kinds and cues
- **CUE-01** A playback has a `kind`: `Cues` (default), `Chase`, `BpmMaster` or `RateMaster`.
  `Cues` with one cue is a memory: nothing in this spec changes it. Appending or inserting a
  second cue (CUE-03..05) makes it a cue list; deleting down to one cue (CUE-07) makes it a
  memory again, its output unchanged until the next action. `playback.options {kind}` switches
  between `Cues` and `Chase` at any time, keeping the cues; a running list switched to the other
  kind keeps its output and continues by the new kind's rules from its current cue or step. A
  playback cannot be switched to or from a master kind (CUE-35).
- **CUE-02** A cue has `number` (CUE definitions), `legend` (0–32 characters, empty by default),
  the PBK times (`delay`, `fade_in`, `fade_out`, `overlap`), `link` (CUE-20), `track` (CUE-51)
  and `group_times` (CUE-14), and its values. A memory's single cue has the same fields; they only take effect
  once the playback holds more cues (its `group_times` also apply on fire, CUE-14).

### Recording into a list (extends PBK-09..13)
- **CUE-03** **Append** (`mode: "Append"` on a used handle, PBK-13): the programmer's contents in
  the record mask (PBK-09, PBK-10, palette references PBK-11) become a new cue at the end with
  number ⌊last⌋ + 1, legend empty, times from the PBK-12 settings and link `Wait`. On a chase it
  adds a step. A running list's output does not change; if the list stood on its last cue with
  nothing next (CUE-21), the new cue becomes the next cue. With nothing in the mask it is refused
  as PBK-09. Service list Q1..Q7, Append → Q8.
- **CUE-04** **Record to a cue number** (`cue` in `playback.record`): a number the list does not
  hold inserts a new cue there (times from the settings, link `Wait`), on any playback of kind
  `Cues` or `Chase`, a memory included (it becomes a list). A number it holds needs a mode, else
  it is refused ("Q3 exists: Replace, Merge or Update"): `Replace`, `Merge` and `Update` act on
  that cue as PBK-13 says, keeping its number, legend, times, link and group times. A live cue
  that changes is on stage on the next tick without a fade (PBK-13). A number out of range or
  with more than three decimals is refused.
- **CUE-05** **Insert** (`mode: "Insert"`): a new cue after the current cue (after the last one
  when the list is off), numbered halfway between the current and the next cue, cut to three
  decimals (between Q2 and Q3 → Q2.5; between Q2 and Q2.5 → Q2.25), or ⌊current⌋ + 1 when there
  is no next cue. Refused when no number fits ("No room between Q2.001 and Q2.002: renumber").
  It becomes the next cue, so Go runs it.
- **CUE-06** `Replace`, `Merge` and `Update` on a cue list or chase without a `cue` number act on
  its current cue (Titan: update the live cue); on a list that is off they are refused ("Name a cue:
  Service is not running"). On a memory they act on its cue as before (PBK-13).

### Editing cues
- **CUE-07** `cue.delete {id, cue}` removes the cue; the playback's last cue cannot be deleted
  ("Delete the playback instead", PBK-14). Deleting the current cue leaves the output as it is;
  the next Go starts the cue after the deleted number. The app asks for a two-step confirm
  (PAT-10).
- **CUE-08** `cue.renumber {id, cue, to}` gives a cue a new number and moves it to its place in
  the order (refused when `to` is used or invalid). `cue.renumber_all {id}` numbers every cue
  1, 2, 3 … in their order. `cue.copy {id, cue, to}` copies a cue (values, times, legend, link,
  group times) to a free number in the same list. The current and next cue follow their cue.
- **CUE-09** `cue.times {id, cue, delay?, fade_in?, fade_out?, overlap?, link?, group_times?}` and
  `cue.legend {id, cue, legend}` change one cue; out-of-range values are refused (times 0–600 s,
  overlap 0–100 %, link offset 0–600 s, as PBK-12). A change to a running transition applies from
  the next cue started; the running one keeps its times.

### Running a cue list
- **CUE-10** **Go** (`cuelist.go {id}`): on a list that is off it fires the list at its next cue
  (the playback becomes active, PBK-25 groups apply, CUE-26 connects it) and starts that cue's
  transition; on a running list it starts the next cue. At the end with nothing next (CUE-21) it
  is refused with a warning ("Q7 Walk-out is the last cue") and nothing changes. Go on a paused
  list resumes it instead (CUE-12).
- **CUE-11** **Back** (`cuelist.back {id}`): starts the cue before the current one in number
  order, with that cue's fades and overlap but no delay, and without following links. The next
  cue becomes the one after it. On the first cue, or on a list that is off, it is refused with a
  warning. Back never wraps to the last cue.
- **CUE-12** **Stop** (`cuelist.stop {id}`): pauses the list: every running fade, delay, overlap
  stagger and link timer of the list holds where it is and the list's output freezes. Go resumes
  them with the time they had left; it does not start the next cue. Stop on a paused list does
  Back (Titan: Stop while paused goes back to the previous cue). Stop on a list with nothing
  running does nothing. Paused is live state.
- **CUE-13** **The transition.** When cue B starts (by Go, a link, Go to cue or Back), for each
  attribute of each fixture:
  - B's state (CUE-50) holds it at another value than the one it has or is fading to: it fades
    after B's delay from its current value to that value, staggered by B's overlap as PBK-15
    (fixtures in user-number order), `Colour` in linear target RGB, palette references live
    (PBK-18). B's times are used for tracked values too, so a jump to a cue fades everything to
    that cue's state in that cue's times. The current value of an HTP attribute is the list's own
    level for it before the fader (not the merged output); of an LTP attribute its current
    output. HTP rises over `fade_in` and falls over `fade_out`; LTP fades over `fade_in`.
  - B's state holds it at the value it has or is fading to (a tracked value after a plain Go):
    nothing changes; a running fade of an earlier cue runs on to its end.
  - The list output it before and B's state does not hold it: HTP fades to 0 over B's
    `fade_out` after B's delay; LTP keeps its current value as the list's value (held, not
    asserted again, so it does not beat a later LTP source, ENG-02).

  Service list, Q1 pars 201–208 at 45 %, Q2 only spots 301–306 at 80 % with `fade_out` 2 s:
  with tracking, Go → the spots rise and the pars stay at 45 % (tracked from Q1); with tracking
  off, the pars fade to 0 in 2 s while the spots rise.
- **CUE-14** **Attribute-group times** (owner decision 5 of `playbacks.md`): a cue's
  `group_times` maps attribute groups (I P C G B E S FX) to `{delay, fade}`. For the attributes of
  a listed group the group's `delay` replaces the cue's delay and its `fade` replaces both
  `fade_in` and `fade_out`; overlap still applies. Groups not listed use the cue's times. On a
  memory they apply to the LTP attributes of the listed groups when it triggers (PBK-15/16); its
  HTP attributes follow *m* as before. Fader mode 2 ignores them (PBK-17); chases ignore them
  (CUE-29). Washes Q3 with fade 2 s and P `{delay 1, fade 5}` → colour and dimmer in 2 s, Pan and
  Tilt start after 1 s and arrive after 6 s.
- **CUE-15** **The fader on a cue list** is its intensity master: *m* = the effective level (fader,
  Flash) at once; the list's HTP values are scaled by *m*, its LTP values are not. Fader modes
  0/1/2 and the kill point (PBK-15..17, 19) do not apply to a cue list; the cues' times do the
  fading. The list option `fader` decides what 0 means (owner, 2026-10-21: configurable per list;
  the setting `cue_list_fader` is the value a playback gets when it becomes a cue list):
  - `KillWithOff` (default, Titan *Intensity Kill With Off*): at 0 the list stays active
    on its current cue (HTP out, LTP held, Go still works); only Release kills it (CUE-17).
  - `KillAtZero` (Titan *Intensity Kill At 0*): when the effective level falls to 0 the list is
    killed (CUE-17). Go with the fader already at 0 runs the list with its HTP out (owner,
    2026-10-21), as `KillWithOff`.
- **CUE-16** **Fire first cue** (list option `fire_first_cue`, default on): the effective level
  rising above 0 on a list that is off fires it at its next cue, as Go. Off: the list becomes
  active with nothing on stage until Go.
- **CUE-17** **Kill and reset.** A list is killed by `KillAtZero`, by Release (PBK-22, PBK-23, the
  Go block's Release) and by its playback group (PBK-25). Every fade, delay and link timer of the
  list stops; the attributes of all its cues release as PBK-20 says (the release mask covers what
  any cue of the list holds); the list is off and its next cue is its first cue again. Fired
  again, it starts from the top: Go fires it at once, the fader only after it has been at 0
  (PBK-22).
- **CUE-18** **Go to cue** (`cuelist.goto {id, cue}`): starts that cue now with its own times, as
  Go would, in any order; on a list that is off it fires the list there. An unknown number is
  refused with a warning.
- **CUE-19** **Set next** (`cuelist.next {id, cue}`): the next cue becomes that cue, on or off;
  nothing changes on stage. Go then starts it (Titan: picking the next cue with wheel A). An
  unknown number is refused.
- **CUE-20** **Links** (Titan *Link Wait For Go / After Previous / With Previous*; the owner's
  follow and wait). A cue's `link` is `{"type":"Wait"}` (default: it waits for Go),
  `{"type":"After","offset":s}` (it starts *s* seconds after the cue before it has completed its
  transition: follow) or `{"type":"With","offset":s}` (it starts *s* seconds after the cue
  before it started: wait time). Links follow a cue started by Go, Go to cue or another link,
  never one started by Back. With loop on (CUE-21) the first cue's link follows the last cue. A
  cue started by a link never starts another one in the same tick (a chain of zero-time links
  moves one cue per tick, so a looping chain cannot stall the tick). Stop pauses the link timers
  (CUE-12), a kill cancels them (CUE-17), and Go on the list while a link is pending starts the
  pending cue at once. Service Q5 Sermon → Q6 Video with `After` 0: Go on Q5, once Q5 is done
  Q6 starts by itself.
- **CUE-21** **End of list** (list option `loop`): `StopOnFinal` (default for cue lists) leaves the
  last cue on stage with nothing next; `Loop` makes the first cue the next one after the last.
- **CUE-22** **Flash on a cue list** works as PBK-26 on the effective level; on a list that is off
  it fires the list at its next cue (released at fader 0 with `KillAtZero`, the list is killed).
  Swop works on a list as PBK-27 says when `handle_buttons` puts it on a button (CUE-25).
- **CUE-23** Priority, playback groups, release mask and time, paging locks, legends, icons and
  halos are per playback and apply to a list unchanged (PBK-07, 20, 24, 25, 31..33). The
  automatic icon and halo come from the first cue (PBK-32). Copy, Move and Delete of a playback
  (PBK-14) carry all its cues and list options; Delete kills a running list first (CUE-17).
- **CUE-24** The live state of a list (current cue, next cue, paused, progress of the current
  transition, running fades and link timers) is not saved (SHW decision 3); after a daemon start
  every list is off with its first cue next.

### Tracking (owner, 2026-10-21: pulled forward from phase 2)
- **CUE-50** **Tracking** (list option `tracking`, default on, Titan; chases never track). With
  tracking on, the state of cue *n* holds, for every fixture and attribute, the value of the last
  cue at or before *n* in number order that holds it (hard in that cue, tracked in the cues after
  it), looking back no further than the nearest Block cue at or before *n* (or *n* itself when it
  is a Solo cue), and skipping Cue Only and Solo cues before *n* (CUE-51: a Solo cue before *n*
  is not there for *n*, as a Cue Only cue). With tracking off, the state of cue *n* is its own
  values. Switching tracking changes no cue and applies from the next cue started. Church: Q3
  Worship holds only the washes' colour and position; the pars' Q2 look tracks into Q3, so no
  cue has to repeat the pars.
- **CUE-51** Per cue `track` (Titan's cue tracking options): `Track` (default); `Block`: nothing
  tracks into this cue (its state is its own values) and later cues track from it; `CueOnly`: its
  values do not track into later cues, so the next cue's state is as if this cue were not there
  and what it changed comes back; `Solo`: Block and Cue Only together. Q6 Video `CueOnly` takes
  the pars to 15 % and the spots to 0; Go to Q7 Walk-out brings back the Q5 state for everything
  Q7 does not hold.
- **CUE-52** **Update modes** (Titan): with tracking on, Update and Merge on a cue (CUE-04, 06) store
  each changed attribute by the setting `update_mode` (default `Forwards`, overridable per record).
  On a tracking list Update takes the attributes of the cue's state, tracked ones included (owner,
  2026-10-21); with tracking off, those the cue holds (PBK-13).
  - `Forwards`: in the cue; later cues that do not hold it track the new value.
  - `CueOnly`: in the cue, and the cue right after it, when it neither holds it nor is a Block or
    Solo cue, gets the attribute's old state value as a hard value, so later cues keep what they had (an
    attribute the state did not hold before gets 0 there when it is HTP; an LTP one tracks on).
  - `Backwards`: in the cue its value tracks from (the cue itself when the value is hard there or
    new), so every cue in between changes too, and protected after the cue as `CueOnly`.
  - `Both`: in the cue its value tracks from, without protection, so it also changes forwards
    until a cue holds it.

  With tracking off every mode stores in the cue. `Replace` stores the cue's new contents; later
  cues track from them.
- **CUE-53** Tracking follows the stored order: a cue that is inserted, recorded to a new number or
  copied tracks into the cues after it; after a Delete the cues after it track from the cue
  before it (what only the deleted cue set is gone from them); Renumber changes the order tracking
  follows. An edit that changes the current cue's state is on stage on the next tick without a
  fade (PBK-13); the other cues show their new state when they start.
- **CUE-54** **Cue view.** `cue.view {id, cue}` → to the asking client only (as LIB-14)
  `{"v":0,"type":"cue.view","playback":1,"cue":3.0,"fixtures":[{"fixture":201,
  "values":{"Dimmer":{"value":0.45,"from":1.0}}}]}`: the cue's state sorted by fixture, `from` the
  cue the value is hard in (this cue when hard), references and colours as PAL-25. The app asks
  again after a `playbacks` delta while the view is open. `cue.remove {id, cue, fixture,
  attribute?}` takes a hard value (or all of a fixture's) out of a cue, so it tracks from earlier
  cues again. App: the Cue softkey **View Cue** shows the Cue view in place of the Cue list window:
  a row per fixture, columns as the Programmer window (UI-14), hard values in normal text,
  tracked values magenta, references cyan; a right-click on a hard value offers Remove (two-step).
  The Cue list window gains a Track column (`·`, B, CO, S). UI-03 gains magenta = tracked value
  (research: the cross-vendor rule).

### Handle buttons (answers PBK-29)
- **CUE-25** The two buttons of a handle follow what is on it (owner, 2026-10-21; Titan's key
  profiles have a section per kind for the same reason). Defaults:

  | Kind | Upper button | Lower button |
  | --- | --- | --- |
  | Memory | Swop (PBK-27) | Flash (PBK-26) |
  | Cue list | **Go** (CUE-10 on this list) | Flash (CUE-22) |
  | Chase | **Tap** (CUE-33) | Flash (CUE-31) |
  | BPM master | — | **Tap** (CUE-33, 35) |
  | Rate master | — | — |

  The setting `handle_buttons` (a small key profile for the whole show; key profiles per user
  stay later; owner, 2026-10-21) is one of:
  - `BestPractice` (default): the table above, so a playback takes the buttons of its kind the
    moment it becomes a cue list or chase (Append, Insert, a kind change) and loses them when it
    goes back to a memory.
  - `Titan`: Titan's factory key profile, Swop / Flash on every kind except masters (a BPM master
    keeps Tap); Go is then in the Go block and Space only.
  - `Custom`: a pair per kind (`Memory`, `CueList`, `Chase`, `BpmMaster`, `RateMaster`; a kind
    left out uses `BestPractice`). Each button is one of: Flash, Swop, Go, Back, Stop, Tap,
    Release or none, as far as the kind has it (Go/Back/Stop on lists, Tap on chases and BPM
    masters, Flash/Swop/Release on everything but masters). A handle's Go acts on its own list,
  wherever the Go block points.
- **CUE-55** `playback.button {id, button: "upper"|"lower", on}` presses or releases a handle
  button; the engine does what `handle_buttons` assigns (held functions, Flash and Swop, follow
  `on`; the others act on the press). The app's strips and later the control surfaces (CTL) send
  this, so the assignment lives in the engine. `playback.flash`/`playback.swop` stay (PBK-36)
  and work on lists too; `playback.flash` on a master is refused with a warning.

### The connected list and the Go block (owner decision 1)
- **CUE-26** The engine keeps one **connected** list (live state, not saved): a cue list or a
  chase, or none. It changes:
  - by `cuelist.connect {id}` (in the app: a plain tap on a list's strip legend, CUE-44);
  - while the setting `auto_connect` is on (default on, Titan *Auto Connect*): when a list fires,
    or gets Go, Go to cue or Set next from its own handle;
  - when there is none (daemon start, show opened, the connected list deleted, turned into a
    memory or a master): the first list in handle order (page, then slot) becomes connected, so
    the Go block always has a list when the show has one.

  Connecting never changes what is on stage. A `connected` delta follows every change.
- **CUE-27** `cuelist.go`, `cuelist.back`, `cuelist.stop`, `cuelist.goto`, `cuelist.next` and
  `playback.release` without an `id` act on the connected list; with none they are refused with a
  warning ("No cue list connected"). The Go block and Space send them without an id.

### Chases (owner decision 6 of `playbacks.md`)
- **CUE-28** A chase runs its steps by itself, in the order of CUE-30, each lasting *T* = 60 /
  effective BPM. Its clock advances each tick by Δt / *T* steps from the tick's timestamp, so a
  tempo change takes effect at once without a jump in the running step; it moves at most one step
  per tick. It starts with its first step (the last one when the direction is `Backward`) when it
  fires (fader, Flash or Go), and a kill (PBK-19 kill point, PBK-22, PBK-25) resets it to that
  step.
- **CUE-29** Chase options (`chase`): `bpm` 1–600 (default 120), `crossfade` 0–100 % (default 0),
  `direction`, `loop` (default `Loop`), `speed_source` (default `Free`). On each step the step's
  values fade from their current value over `crossfade` · *T* with CUE-13's rules (HTP not in the
  step fades to 0, LTP held); 0 snaps; at 50 % the step fades for half its time and holds for the
  other half (Titan). The steps' own times, overlap, group times and links are ignored in
  phase 1. Effective BPM: `Free` = `bpm`; `Bpm n` = BPM master *n*'s tempo (CUE-35); `Rate n` = `bpm` ·
  Rate master *n*'s rate (CUE-36). A source whose master does not exist runs as `Free` and is
  kept, so a master recorded later with that number takes over.
- **CUE-30** `direction`: `Forward` (default), `Backward`, `Bounce` (1 2 3 2 1 2 …) or `Random`
  (never the same step twice in a row, chosen by a fixed pseudo-random sequence seeded from the
  playback id and the step count, so it is deterministic, ENG-11). With `loop` `StopOnFinal` the
  chase stays on its last step, active.
- **CUE-31** The fader on a chase is its intensity master as on a memory in mode 0 without times:
  *m* = the effective level at once, HTP scaled, LTP not; the kill point applies (PBK-19), so at
  fader 0 a chase is killed and reset. Flash fires it and lifts it to full while held (PBK-26).
- **CUE-32** Go, Back and Stop on a chase (Go block, connected chase): Stop pauses its clock and
  the running crossfade; Go on a paused chase resumes it; Go on a running chase starts the next
  step now and restarts the step clock; Back starts the previous step (against the direction)
  now; Go to cue *n* starts step *n*. Set next sets the step Go starts.
- **CUE-33** **Tap** (`playback.tap {id}`, the chase's upper button, the Go block's Tap): taps are
  timed at the moment the daemon receives the command, passed to the core with the action (not
  the tick, which would round to 25 ms). The first tap, or a tap more than 2 s after the previous
  one, starts a new series and changes nothing; each further tap sets the BPM to 60 / the mean of
  the series' last three intervals at most, clamped to 1–600 and rounded to 0.1 BPM, and starts
  the next step at that moment (beat sync). On a chase following `Bpm n` the tap goes to that
  master; otherwise it sets the chase's own `bpm` (saved).
- **CUE-34** `playback.bpm {id, bpm}` types a tempo for a chase or a BPM master (1–600, else
  refused); the Go block's BPM field sends it.

### Speed masters (owner decision 6 of `playbacks.md`; owner decision 5)
- **CUE-35** **BPM masters 1–8** (Titan *BPM 1–8*). `master.record {page, slot, kind: "Bpm",
  number}` puts one on an empty handle (a used handle, a number in use or outside 1–8 is refused).
  It holds `bpm` (default 120, saved), set by Tap (CUE-33) and by `playback.bpm`, and a
  `multiplier` (/4, /2, ×1, ×2, ×4, default ×1) that scales the tempo it gives chases. Its
  `fader` option (owner, 2026-10-21: tap or type by default, Titan's behaviour possible):
  - `None` (default): the fader is not used; the strip shows the tempo instead of a level.
  - `Bpm` (Titan *BPM On Fader*): the fader sets the BPM, 0 % → 0 (stopped) to 100 % →
    `fader_max` (option, default 240 BPM); the last of a fader move, a tap or a typed tempo sets
    the tempo, so a tap is never undone until the fader moves again.
  - `Multiplier` (Titan *Multiplier on Fader*): the fader picks the multiplier in five equal
    zones /4, /2, ×1, ×2, ×4; the tempo comes from Tap or typing.

  Ableton sync (IDEA-16, phase 2) will be a third tempo source next to Tap and typing.
- **CUE-36** **Rate masters 1–4** (Titan *Rate 1–4*). `master.record {…, kind: "Rate", number}`
  as CUE-35. Its rate is fader · `rate_scale` (option 100 or 200 %, default 200 %: fader 50 % =
  ×1, full = ×2, 0 = stopped); the fader level is live state like every level. Rate masters
  scale chases in phase 1 (shapes in phase 2).
- **CUE-37** Masters never enter the merge, are never killed or released, and ignore priority,
  group and release options; they page, lock, Copy, Move and Delete as playbacks (PBK-07, 14);
  a copy gets the next free number of its kind (refused when there is none). Recording, Append or
  Insert onto a master's handle is refused ("Handle 1.3 holds BPM 1").
  Their Playback Options show only Handle and Master tabs. Deleting a master makes its chases run
  as CUE-29 says for a missing master.

### Show file (extends `showfile.md`, `playbacks.md` PBK-34)
- **CUE-38** Schema version 4 (SHW-04 migration from 3):
  - each cue gains `number`, `legend`, `link`, `track` and `group_times` (only the groups set, in
    I P C G B E S FX order), after `overlap` and before `values`;
  - each playback gains, after `group`, `kind`, `list` (`{"tracking":true,"fader":"KillWithOff",
    "fire_first_cue":true,"loop":"StopOnFinal"}`) and `chase` (`{"bpm":120.0,"crossfade":0.0,
    "direction":"Forward","loop":"Loop","speed_source":"Free"}`), always written, and `master`
    (`{"number":1,"bpm":120.0,"multiplier":1.0,"fader":"None","fader_max":240.0}` or
    `{"number":1,"rate_scale":200}`) on masters
    only; `speed_source` is `"Free"`, `{"Bpm":n}` or `{"Rate":n}`;
  - `settings` gains `auto_connect` (true), `cue_list_fader` (`KillWithOff`), `update_mode`
    (`Forwards`) and `handle_buttons` (`"BestPractice"`, `"Titan"` or
    `{"Custom":{"CueList":["Go","Back"]}}`, CUE-25).

  The 3 → 4 migration numbers each playback's cues 1, 2, 3 … in their order, with empty legends,
  link `Wait`, track `Track`, no group times, kind `Cues` and the default `list` and `chase`
  options.
- **CUE-39** Lenient load (SHW-07), each with one warning naming it: a cue whose number is out of
  range, has more than three decimals or repeats an earlier one is left out (a playback left
  without cues stays, PBK-44); a link, group time or list/chase option out of range is set to its
  default; a `Custom` button a kind cannot have is set to that kind's `BestPractice` button; a
  master whose number is out of range or used by an earlier master of its kind is left
  out; a master with cues loads without them.

### IPC (extends `ipc.md`, PBK-35/36)
- **CUE-40** Each snapshot `playbacks` entry gains `kind`; `cues` (`[{"number":2.5,"legend":"Welcome",
  "delay":0,"fade_in":2,"fade_out":2,"overlap":100,"link":{"type":"Wait"},"track":"Track","group_times":{},
  "groups":["I","C"]}]`, without values); `list`, `chase` and `master` as CUE-38, `chase` with
  `effective_bpm`; and live `cue` (`{"current":2.0,"next":3.0,"progress":0.4,"paused":false}`,
  `current`/`next` `null` when none; for a chase the step numbers). `groups` (PBK-35) becomes the
  groups of all cues. The small `playback` delta gains `cue` and, on chases and masters, `bpm`
  (effective); it is sent at most once per tick while a transition runs (IPC-05). Any stored cue
  change sends the PBK-35 `playbacks` delta. The snapshot gains `"connected": 1` (an id or `null`)
  and a `connected` delta; `playback_settings` gains the four CUE-38 settings.
- **CUE-41** Commands (refusals are warnings, IPC-06), each `id` optional where CUE-27 says so:
  `cuelist.go {id}` · `cuelist.back {id}` · `cuelist.stop {id}` · `cuelist.goto {id, cue}` ·
  `cuelist.next {id, cue}` · `cuelist.connect {id}` · `playback.button` (CUE-55) ·
  `playback.tap {id}` · `playback.bpm {id, bpm}`
  · `playback.record` gains `mode` `Append`/`Insert`, `cue` and `update_mode` (CUE-03..06, 52) ·
  `playback.options` gains `kind`, `list {…}`, `chase {…}`, `master {…}` (each field optional) ·
  `cue.times`, `cue.legend`, `cue.delete`, `cue.renumber`, `cue.renumber_all`, `cue.copy`
  (CUE-07..09) · `cue.track {id, cue, track}` (CUE-51) · `cue.view`, `cue.remove` (CUE-54) ·
  `master.record {page, slot, kind, number}` · `settings.playback` gains the CUE-38 settings.

### App (UI v0.1; amends `ui.md` UI-13, 18, 19, 22, 23 and PBK-37..39 when approved)
- **CUE-42** **Go block** (UI-23) shows the connected list:
  - header: its legend (else `page.slot`) and handle on the left; live and next fade time
    ("live 3s · next 2s", in/out as "2/4s") on the right; for a chase "120 BPM · xfade 50 %";
    an amber "fader 40 %" when the list's effective level is below 100 % (so a Go at fader 0 is
    explained);
  - the "now" line: current cue number in green and its legend, with the progress while a
    transition runs; amber "paused" while paused;
  - the GO button with the next cue's number and legend, a progress bar along its bottom, "end of
    list" and disabled when nothing is next;
  - the row Back · Stop · Release (Stop lit amber while paused); a chase adds Tap and the BPM
    field (scroll to adjust, type to set, CUE-34);
  - with no connected list, the block is disabled with "No cue list". It is never covered (UI-23).
- **CUE-43** **Cue list window** (UI-13) shows the connected list: title "Cue list · <legend>",
  info "1.1 · 7 cues"; columns Q, Legend, Delay, Fade (in/out), Link (`·`, "follow +1s", "with
  +2s"), Track (CUE-54), Groups; the current row marked green, the next row outlined; a running transition shows
  its progress in its row. A tap on a row sets it next (CUE-19), so a tap can never change the
  stage. A right-click on a row opens the Cue softkeys: Go to cue (CUE-18), View Cue (CUE-54),
  Edit Times (the
  Times tab for that cue, CUE-46), Set Legend, Renumber (type a number), Copy (type a number),
  Delete (two-step). With no list: the sketch's empty table and "No cue list connected".
- **CUE-44** **Strips** (UI-22, PBK-37): the two buttons are labelled with what `handle_buttons`
  assigns to the kind (GO, TAP, FLASH, …; CUE-25), send `playback.button` (CUE-55) and are live
  while pressed; the legend's second
  line shows "Q3 › 4" on a cue list, the step and BPM on a chase ("2/8 · 120"), "BPM 1 · 120 ×1"
  or "RATE 2 · ×1.0" on a master; the level bar of a running cue list shows the transition's
  progress (sketch); the connected list's strip has a 2 pt blue frame (UI-03: the selected
  playback). A plain tap on a list's legend (no function button armed) connects it (CUE-26).
- **CUE-45** **Record** on a used handle shows the softkeys Replace, Merge, **Append**, **Insert**
  and Cancel (PBK-38). Record + tap a row of the Cue list window shows Replace, Merge, Update and
  Cancel for that cue (CUE-04). Record, a number on the keypad, then a tap on a handle records to
  that cue number (Titan *Record, Cue n*). On a tracking list the Record context has an Update
  Mode softkey (Forwards, Cue Only, Backwards, Both; CUE-52) for this record.
- **CUE-46** **Playback Options** (PBK-39) gains, by kind: a **Cue list** tab (kind, tracking,
  fader, fire first cue, loop) and a **Chase** tab (kind, BPM, crossfade, direction, loop, speed source);
  the **Times** tab gets a cue picker (default the current, else the first cue), link type and
  offset, track (CUE-51), and an IPCGBES row of group times (delay, fade, or "—" for the cue's times); a master
  shows Handle and **Master** tabs (number, multiplier, fader mode and maximum, or rate scale).
  `Auto connect`, the cue-list fader default, the update mode and the handle buttons per kind
  sit in the Playback settings.
- **CUE-47** **Keys** (UI-19 amended): while Keys is live, Space = Go and Shift+Space = Back on the
  connected list; never while a text field has focus (UI-19). Space does nothing while Keys is
  off, so a stray key never fires a cue (research: "Space = Go only when armed").
- **CUE-48** `tests/manual/app-cuelists.md` walks the service list (VS3-01): Append Q1..Q7 without
  Clear between cues and with tracking, a Cue Only video cue, Update in each mode, the Cue view,
  Insert Q2.5, renumber all, link Q5 → Q6, group times on the washes, Go /
  Back / Stop / Go to cue / Set next from the Go block, Space and the handle's Go, a second list
  connected by a tap and by auto connect, the fader options, Release and reset, a chase with Tap,
  crossfade, Bounce and a BPM master in each fader mode and a Rate master, changed handle
  buttons, read on Art-Net; and a daemon restart with the
  lists back from the show.

### Safety and determinism
- **CUE-49** Every refused list action returns a warning and changes nothing (PBK-43); nothing a
  list, link chain, chase or master does can stop the tick or the output (ENG-12). Same patch and
  same timestamped actions (tap times included) → identical list state and frames (ENG-11). The
  core has no clock and no random source of its own (CUE-30, 33).

## Scenario test format
Scenario playback entries gain the CUE-38 fields (`kind`, `list`, `chase`, `master`, and per cue
`number`, `legend`, `link`, `track`, `group_times`; `number` may be left out and then counts 1, 2, 3 … in
file order). New actions: `go`, `back`, `stop` `{playback}` (no `playback` = the connected list),
`goto`/`next {playback, cue}`, `connect {playback}`, `tap {playback, at}` (`at` = the receive
time, CUE-33), `bpm {playback, bpm}`, `button {playback, button, on}` (CUE-55); scenarios may
set `settings` (`handle_buttons`, `update_mode`). Example, CUE-20 follow with fire first cue:

```json
"playbacks": [ { "id": 1, "page": 1, "slot": 1,
  "cues": [ { "number": 1, "fade_in": 2.0, "values": { "1": { "Dimmer": 1.0 } } },
            { "number": 2, "fade_in": 4.0, "fade_out": 4.0,
              "link": { "type": "After", "offset": 1.0 },
              "values": { "2": { "Dimmer": 1.0 } } } ] } ]
{ "t": 0.0, "fader": { "playback": 1, "level": 1.0 } }
"expect": [ { "t": 1.0, "universe": 1, "channels": { "1": 128, "2": 0 } },
            { "t": 5.0, "universe": 1, "channels": { "1": 128, "2": 128 } } ]
```

(Q1 fires with the fader and is complete at 2 s; Q2 follows at 3 s; at 5 s it is halfway.)
Recording, editing, numbering, kinds, masters, the connected list and the show file are unit
tests on `cmdmx-core` (`tests/cuelists.rs`, `tests/showfile.rs`); everything that reaches the frame
(CUE-10..22, 28..36) is a scenario file `tests/scenarios/CUE-nn-*.json`.

## Build tasks (`BACKLOG.md`, sized after approval)
1. **VS3-02** core storage: kinds, cue numbers and fields, Append / Insert / record to a cue,
   cue edits, masters on handles, list/chase/master options, schema 4 with migration and lenient
   load (CUE-01..09, 23, 35..39 storage, the CUE-51 field). Tests: `cmdmx-core/tests/cuelists.rs`, `tests/showfile.rs`.
2. **VS3-03** cue-list behaviour on the frame: transitions, group times, Go / Back / Stop / Go to
   / Set next, links, the fader options, fire first cue, kill and reset, Flash, the connected list,
   tracking with Block / Cue Only / Solo and the Update modes, handle buttons
   (CUE-10..27, 49..53, 55). Tests: scenarios `CUE-*.json`, `cmdmx-core/tests/cuelist_run.rs`.
3. **VS3-04** chases, Tap and the speed masters (CUE-28..37). Tests: scenarios, unit tests for tap
   and the random order.
4. **VS3-05** IPC and the app: Go block, Cue list window, strips, Record softkeys, Options tabs,
   Space, the Cue view, `tests/manual/app-cuelists.md` (CUE-40..48, 54); the owner's run is VS3-01's demo.

## Out of scope (phase 1 unless noted)
Editing in a tracking view (turning hard values into tracked ones and back), Update on several
cues at once, shape tracking (with shapes, phase 2); the manual crossfader fader mode (with fader
mode 3, phase 2); a typed time on Go ("3 Go"); Snap / Snap Back and Cut Next Cue To Live; Move In
Dark; Cue Release; disabling a cue; Cue Links Disabled; link offsets in %; Delay Out and
per-attribute or per-fixture times; per-step times, fixture overlap and speed multipliers (×/÷)
on chases; LocalClock and Edge Sync; Ableton or MIDI-clock sync (IDEA-16, phase 2); the Rate
Grand Master; Size masters and shapes following masters (phase 2); key profiles per user and
beyond the two handle buttons (CUE-25); a Cue / Connect key on the keypad; copying cues between
lists; Quick Build and Record Stage; autoloads, macros and timecode / MIDI triggers in cues
(phase 3 for timecode); Blind editing; the set list.

## Owner decisions (2026-10-21, answers to the first gate-1 questions)
1. **Go per fader and one Go block (A):** a cue list sits on any handle with its own Go
   (CUE-25); the Go block, Space and the Cue list window follow the connected list (CUE-26: a tap
   on its legend, auto connect when it fires, else the first list in the show). Not chosen: a Go
   block pinned to one list (B), no Go block (C).
2. **Handle buttons follow what is on the handle** (CUE-25, 55). Owner asked what is usual:
   Titan's key profiles assign the buttons per kind (cues, chases, cue lists, masters), with
   Flash/Swop as the factory default for all; operators commonly put Go on a cue list's button,
   and Go or Tap on a chase's. The CUE-25 table is that practice and the default (a list or chase
   on a handle gets it at once); `handle_buttons` switches to Titan's Swop/Flash or a custom pair
   per kind (owner follow-up, same day), and makes it
   configurable per kind for the whole show.
3. **Cue-list fader at 0:** `KillWithOff` by default, configurable per list and as the setting
   for new lists (CUE-15).
4. **Tracking comes into phase 1** (CUE-50..54): Track / Block / Cue Only / Solo per cue, Update
   modes, a Cue view with tracked values in magenta. Reverses part of DECISIONS 2026-10-17
   ("cue-list tracking moves to phase 2"); the cue data model was made for this (touched
   attributes only), so no stored format changes beyond schema 4.
5. **Speed masters:** BPM masters by Tap or typing by default, with Titan's BPM-on-fader and
   multiplier-on-fader as options (CUE-35); Rate masters 0–200 %, 50 % = normal speed (CUE-36).
   Ableton sync with subdivisions and multiples is a new idea, IDEA-16.
6. **Approval (same day):** tracking (CUE-50..54, `update_mode` default `Forwards`), the
   handle buttons (CUE-25 with `BestPractice` / `Titan` / `Custom`, CUE-55) and the BPM-master
   fader modes (CUE-35) approved; IDEA-16 (Ableton sync) is phase 2.

Smaller proposals, accepted with the approval: cue numbers with up to three decimals; Back uses
the previous cue's fades without delay; a tap on a Cue list row sets the next cue instead of
jumping; `auto_connect` on by default; chase defaults 120 BPM, crossfade 0 %, Forward, Loop;
Shift+Space = Back; Cue Only on an attribute the state did not hold puts HTP 0 in the next cue.
