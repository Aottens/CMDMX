# Module spec: playbacks (PBK)

Owner: `cmdmx-core::playback` (pure: storage, recording, fader modes, release, flash/swop),
`cmdmx-daemon` (IPC), the playback area and Playback Options in `subprojects/app`.
Status: **PBK-01..05 approved by owner 2026-10-18 (VS1-06 subset); PBK-06..45 approved by owner
2026-10-18 (P1-04), owner decisions at the end.**
Behavioural reference: Avolites Titan v19 manual, *Cues* (https://manual.avolites.com/docs/cues:
Creating a Cue, Cue Playback, Cue Timing, Playback Options, Copying/Moving/Linking/Deleting) and
the Titan analysis doc linked in `PROJECT.md`. UI reference: UI v0.1 (`specs/ux/main-screen.html`:
pages column, playback strips, Active playbacks window, GM/DBO; `research.md` §4 button states,
"Halos", design brief items 6 and 10). Builds on `engine-merge.md` (ENG-01..09), `programmer.md`
(PRG-15, 20, 23, 26), `palettes.md` (references with fallback values, PAL-11, 13, 14, 18, 21, 22),
`showfile.md` (SHW-04, 07) and IDEA-14 (icons, phase-1 part).

## Purpose
Playbacks are what the operator runs: a handle on a page with content, a priority and a fader
mode. P1-04 makes them recordable from the programmer, gives them Titan's options (fader modes
0/1/2, priority, release mask and time, kill point, paging locks, playback groups), Flash and
Swop, stored handles on pages, legends with icons and halos, and DBO. A playback holds **cues**;
in phase 1 a playback recorded here holds exactly one cue (Titan's *memory*). P1-08 (cue lists)
lets a playback hold more than one cue and adds Go/Back, so everything in this spec is per
playback unless it says "per cue".

## Definitions
- **Page:** a bank of `faders_per_page` handles (Settings, default 8, never hard-coded). There
  are 10 pages (PBK-01). A page has a legend.
- **Handle:** a slot on a page, 1-based, 1 … `faders_per_page`. Written `page.slot` ("2.3").
- **Playback:** an internal **id** (assigned by the engine, never reused, like fixture and palette
  ids), its **home handle** (page and slot, unique), a legend, an icon, a halo, options and a
  list of cues. Commands and state name playbacks by id; the operator reaches them by handle.
- **Cue:** values (`fixture id → attribute → value or palette reference`, PAL definitions) and
  times: `delay`, `fade_in`, `fade_out` (seconds, 0–600) and `overlap` (0–100 %).
- **Fader level** *f*: the handle's fader, 0.0–1.0. **Effective level**: *f*, or 1.0 while Flash
  or Swop is held or Flash is latched (PBK-26..28).
- **Fire:** the effective level rises above 0 while the playback is not active: it becomes
  active (enters the merge). **Trigger:** the effective level reaches `ltp_trigger_level`
  (default 2 %, ENG-05): its LTP values are asserted from then on.
- **Intensity master** *m*: per active playback, 0.0–1.0; the playback's HTP values are scaled
  by *m* (PBK-15..17).
- **Kill:** the playback leaves the merge (PBK-19). **Release:** after a kill, the attributes in
  its release mask fade back to what is underneath (PBK-20).
- **Underneath:** for one fixture and attribute, what the merge outputs without this playback:
  the other active sources by ENG-01..04, else the value the attribute output in the tick before
  this playback first asserted it (Titan: its previous state; the profile default, LIB-03, when
  nothing had set it). Clarified 2026-10-18 (VS2-21): the held value is the playback's own last
  output, so it cannot be what a release returns to.
- **Timing:** every time is counted from the tick in which the action (a fader move, Flash,
  Release) reaches the engine, as for palette fades (PAL-13); *m* and fades are evaluated per
  tick from the tick's timestamp, so they are deterministic (ENG-11).
- Church examples use the demo rig of `programmer.md`: washes 101–104, RGBW pars 201–208,
  RGB+Lime pars 211–216, RGB pars 221–226, front spots 301–306.

## Acceptance criteria

### Pages (VS1-06 subset, approved; PBK-01 superseded by PBK-06)
- **PBK-01** Until P1-04 stores playbacks on handles, slot *s* of page *p* holds playback
  (p − 1) · `faders_per_page` + *s* when that playback exists, otherwise nothing. With 8 per page:
  page 1 holds 1–8, page 2 holds 9–16. There are 10 pages. *(Superseded by PBK-06; the schema 3
  migration, PBK-45, keeps exactly this layout for existing shows.)*
- **PBK-02** `Engine::set_page(p)` changes the current page; 0 or a page above 10 is refused with
  a warning and nothing changes. A page change never changes a playback's level or whether it
  is active: a playback up on page 1 keeps running while page 2 is shown, and shows its level
  again when page 1 comes back (Titan, unlocked handles; locks are PBK-07).
- **PBK-03** IPC (extends `ipc.md`): the snapshot carries `"page":{"number":1,"count":10,
  "slots":[{"slot":1,"playback":1},{"slot":2,"playback":null},…]}` with `faders_per_page`
  entries; `page.set {"page":2}` changes it; a `page` delta with the same object follows every
  change. `fader.set {playback, level}` is unchanged: the app sends the playback of the slot.
- **PBK-04** The demo show (IPC-14) gains two playbacks on page 2: 9 = the washes 101–104 at full
  Dimmer, 10 = the front spots 301–306 at full, both priority Normal, fader mode 0.
- **PBK-05** App (UI-21, UI-22): the pages column shows page buttons 1–10 with the current one
  highlighted; a click sends `page.set`. The strips show the current page's slots: the legend is
  the playback number, the level and green active strip come from that playback; a slot without
  a playback is an empty, disabled strip without a cap. Checked in
  `tests/manual/app-main-screen.md` (UX-02d).

### Stored handles and paging locks
- **PBK-06** A playback lives on its home handle; slot *s* of page *p* shows the playback whose
  home is `p.s`, unless a lock from another page covers it (PBK-07). Handles with a number above
  `faders_per_page` keep their playbacks (a smaller setting hides them, it never deletes them);
  they are listed in Active playbacks while active (PBK-40) and reachable again when the setting
  grows. An empty legend shows the home handle ("2.3"), replacing PBK-05's playback number.
- **PBK-07** Handle paging, per playback (Titan *Handle Paging*), default `Unlocked`:
  - `Unlocked`: shown only on its home page.
  - `Locked`: shown at its slot on every page; a playback stored at that slot on another page is
    not reachable while the lock exists.
  - `TransparentLock`: shown at its slot on every page whose own slot is empty.
  Two `Locked` playbacks on the same slot: setting the second is refused with a warning. Record,
  Copy and Move onto a slot address what it shows: a slot covered by a `Locked` playback is used
  (that playback); a slot showing a `TransparentLock` one from another page is empty, and a
  playback recorded there covers it on that page. Church:
  the front spots 301–306 on handle 1.8, `Locked`, so they can go out for a video from any page.
- **PBK-08** Page legends: 0–32 characters, empty by default; an empty page legend shows nothing
  next to the number (UI-21). Live state that stays out of the show: the current page (PBK-02).

### Recording (`Engine::record_playback`)
- **PBK-09** Recording onto an empty handle creates a playback there (new id, home = that handle,
  options at their defaults, one cue) holding the programmer's contents in the record mask
  (PRG-15) for **every fixture in the programmer**, as on Titan (the programmer, not the stage).
  The mask defaults to every group (I P C G B E S FX). Located and highlight values are never
  recorded (PRG-23, 26). With nothing in the mask, the record is refused with a warning
  ("Nothing in the programmer to record") and nothing changes. The programmer is left as it is.
  Programmer 201–208 Dimmer 1.0 + palette 3, record onto 1.1 → playback on 1.1 holds Dimmer and
  the palette references for 201–208.
- **PBK-10** *Record By* for playbacks: setting `cue_record_by`, default `Channel` (only touched
  attributes, so tracking can follow in phase 2, `PROJECT.md`), overridable per record; `Mixed`
  behaves as PAL-02 (whole P and C groups of a touched fixture). (Owner decision 1.)
- **PBK-11** Palette references stay references: a programmer value `{"palette": id, "value": …}`
  is stored as a reference to the same palette, its fallback `value` set to what the reference
  resolves to at the moment of recording (PAL definitions, PAL-11, PAL-18). Manual values are
  stored as values.
- **PBK-12** The new cue's times come from the settings `cue_delay`, `cue_fade_in`, `cue_fade_out`
  (default 0 s) and `cue_overlap` (default 100 %); out-of-range settings are refused as in
  PAL-15.
- **PBK-13** Recording onto a used handle needs a mode, else it is refused with a warning
  ("Handle 1.1 is used: Replace or Merge"). `Replace` stores the new contents into the playback's
  cue, keeping id, home, legend, icon, halo, options and times. `Merge` writes the new contents
  over the cue's values, keeping everything else. `Update` (the Update function button) merges
  only attributes the cue already holds for fixtures it already holds (as PAL-06 Quick Merge).
  P1-08 adds `Append` (the cue list case). A recorded or merged playback that is active changes
  its output on the next tick without a fade, as a palette does (PAL-11).
- **PBK-14** Copy, Move and Delete: Copy to an empty handle makes a new playback there (new id)
  with the same cues, legend, icon, halo and options; Move to an empty handle changes only the
  home handle (same id), so a running playback keeps running. Copy or Move onto a used handle is
  refused with a warning. Delete removes the playback; if it was active it is killed first
  (PBK-19, without release time), its id is never reused. Deleting a palette turns this
  playback's references into their fallback values (PAL-18); removing a fixture removes its
  values from every cue (as PAL-19), and a playback left without values stays.

### Fader modes (`fader_mode`, default 0; mode 3 is phase 2)
- **PBK-15** **Mode 0.** On fire, after the cue's `delay`, *m* rises from 0 to the effective
  level over `fade_in` (linearly; at once when 0) and follows the effective level from then on,
  that is *m* = *f* · *p*, *p* going from 0 to 1 over `fade_in` once `delay` has passed;
  a falling level lowers *m* at once (Titan: fade out is ignored). On trigger, the LTP values
  fade from their current output to the cue's values over `fade_in` after `delay`, staggered by
  `overlap` as PAL-14 (fixtures in user-number order), `Colour` in linear target RGB; with
  `fade_in` 0 they snap (ENG-05). They are not scaled by the fader. Washes on 1.1, `fade_in` 3 s,
  fader pushed to full at t = 0 → Dimmer 50 % at 1.5 s, full at 3 s; pulled to 0 → out at once.
- **PBK-16** **Mode 1.** As mode 0, and a falling level lowers *m* no faster than full scale per
  `fade_out` (from 100 % to 0 in `fade_out`; at once when 0), so pulling the fader down fades the
  intensity out over the fade-out time. LTP values stay as the cue set them. Washes at full,
  `fade_out` 4 s, fader pulled to 0 at t = 0 → Dimmer 50 % at 2 s, 0 at 4 s. (Owner decision 2.)
- **PBK-17** **Mode 2.** Times are ignored. *m* is the effective level, and every LTP value
  crossfades with the fader from the value the attribute output in the tick before the playback
  fired (captured per fixture and attribute) to the cue's value: output = from + *f* · (cue −
  from), `Colour` in linear target RGB. At fader 0 the attribute is back at its captured value
  (Titan: LTP reverts). Pan from 0.2 to a cue at 0.8, fader at 50 % → Pan 0.5.
- **PBK-18** A palette reference in a cue is resolved every tick (PAL-11), also while fading: a
  fade or crossfade runs towards the reference's current value.

### Kill and release
- **PBK-19** **Kill point** (`kill_point`, default `FadeOutComplete`): `FadeOutComplete` kills the
  playback when the effective level is 0 **and** *m* has reached 0 (in mode 1 after the
  fade-out); `FaderAtZero` kills it as soon as the effective level is 0, cutting a mode-1
  fade-out. On kill the playback leaves the merge: its HTP values are gone at once (they were
  at 0 already unless cut), its LTP values not in the release mask are held at their last output
  (ENG-07), and the masked ones release (PBK-20). To fire again the effective level has to rise
  from 0.
- **PBK-20** **Release mask:** per playback `release_mask` is `null` (use the global setting
  `release_mask`, default empty: nothing releases, Titan's default) or a list of attribute
  groups (a local mask). On kill, every attribute of the playback whose group is in the mask
  fades from its current output to what is underneath, linearly over the playback's
  `release_time`, or the global setting `release_time` (default 2 s, Titan) when it is `null`.
  While releasing, the playback is shown as releasing (PBK-35, amber in PBK-37). Strobe cue on 201
  (Shutter strobing, group I) with local mask I, release time 0 → fader to 0 puts Shutter back to
  what is underneath at once (ENG-08).
- **PBK-21** A playback that fires again while it is releasing stops its release at the current
  values and fades or crossfades from there as its mode says.
- **PBK-22** **Release + handle** (the Release function button, then the handle's legend) kills
  the playback at once (ignoring `fade_out` and the kill point) and releases **every** attribute
  it holds, over its release time (Titan's temporary mask). A fader that is still up has to go
  to 0 and back up to fire it again. Release on an inactive playback does nothing.
- **PBK-23** Tapping a row of the Active playbacks window (PBK-40) does the same as PBK-22.

### Priority and playback groups
- **PBK-24** `priority` (default `Normal`) is one of Titan's five levels and merges as ENG-01..04
  (unchanged); a changed priority applies from the next tick.
- **PBK-25** **Playback groups:** `group` is `null` (default) or 1–99. When a playback with a
  group fires, every other active playback with the same group is killed (PBK-19, with its own
  release rules) as if its kill point were reached; its fader stays where it is and has to go to
  0 and back up to fire again. Church: walk-in, worship and sermon looks in group 1, so raising
  one takes the other out.

### Flash, Swop and Latch
- **PBK-26** **Flash** held: the effective level is 1.0 and *m* jumps to 1.0, ignoring
  `fade_in` (LTP values fire with the cue's times, as on a fire). Released: the effective level
  is the fader again and *m* follows it at once, ignoring `fade_out`; at fader 0 the kill point
  rules apply. Flash on a playback whose fader is already up only lifts it to full while held.
- **PBK-27** **Swop** held: as Flash on this playback, and every other active playback's HTP
  contribution is 0 (LTP values unchanged, the programmer unaffected). Released: the others come
  back at their own levels and this one goes back as PBK-26. Swop on two handles at once: the
  later one wins.
- **PBK-28** **Latch** (function button) then Flash on a handle latches that Flash on: the
  playback stays at full as if Flash were held, until the next Flash press on it unlatches it.
  Latch is armed for one press; Clear or Exit disarms it.
- **PBK-29** Flash and Swop are the two fixed buttons per handle in phase 1, the same for every
  playback (upper Swop, lower Flash, UI-22). P1-08 decides what they do on a playback with more
  than one cue (Titan key profiles: Go, Flash and Go). (Owner decision 3.)

### Grand Master and DBO
- **PBK-30** **DBO** (Dead Black Out) is a toggle: while on, every HTP output is 0 after the merge,
  after the Grand Master (ENG-06), including the programmer and virtual-dimmer emitters (LIB-04);
  LTP output is untouched. Off restores the output at once. Live state: not in the show, off
  when the daemon starts. The GM and DBO are never paged or locked.

### Legends, icons and halos (IDEA-14, phase-1 part)
- **PBK-31** A playback has `legend` (0–32 characters), `icon` (an icon id from the built-in
  library, PAL-22, or `null` for the automatic icon) and `halo` (an sRGB colour or `null`).
  `playback.legend` refuses a longer legend or an icon id that breaks PAL-22's rules.
- **PBK-32** The engine computes each playback's automatic icon from its first cue's values with
  PAL-21's rules (colours, else positions, else the first stored attribute), so a red wash cue
  shows a red swatch on every client; a picked icon still reports the automatic one ("Auto").
- **PBK-33** Halo: `null` means **automatic**: the first colour of the automatic icon when it has
  colours, else no halo; a picked colour is used as is. The halo is drawn as the strip's top bar
  (UI-22, sketch) and is separate from every state colour (UI-03). It is the colour later sent to
  a control surface's fader LEDs where one has them (CTL). (Owner decision 4.)

### Show file (extends `showfile.md`)
- **PBK-34** Schema version 3 (SHW-04 migration from 2): `settings` gains `cue_record_by`,
  `cue_delay`, `cue_fade_in`, `cue_fade_out`, `cue_overlap`, `release_mask`, `release_time`; a key
  `pages` after `palettes` holds `[{"number": 2, "legend": "Service"}]` for pages with a legend,
  sorted by number; `playbacks` becomes `{"next_id": n, "playbacks": [{id, page, slot, legend,
  icon, halo, priority, fader_mode, kill_point, release_mask, release_time, paging, group,
  cues: [{delay, fade_in, fade_out, overlap, values}]}]}`, sorted by id (SHW-02). Live state
  (levels, active, releasing, Flash/Latch, DBO, current page) is not saved (SHW decision 3).

### Active playbacks and IPC (extends `ipc.md`)
- **PBK-35** Each snapshot `playbacks` entry (IPC-03) gains `page`, `slot`, `legend`, `icon`,
  `auto_icon` (PAL-21 shape), `halo` (the effective colour or `null`), `state` (`"off"`,
  `"active"` or `"releasing"`, replacing `active`), `flash` (held or latched), `latched`, and
  `options {priority, fader_mode, kill_point, release_mask, release_time, paging, group}` and
  `times {delay, fade_in, fade_out, overlap}` (of the first cue). The `playback` delta (IPC-05)
  carries the same object after any change to it; level-only changes stay as small as today
  (`id, level, state`). The snapshot gains `"pages":[{"number":1,"legend":""},…]` (10 entries,
  a `pages` delta after a legend change), `"dbo": false` (and a `dbo` delta), and
  `"playback_settings"` with the PBK-12 and PBK-20 settings (and a delta). The PBK-03 `page`
  object's slots carry the playback the slot shows, locks included (PBK-07).
- **PBK-36** Commands (refusals are warnings, IPC-06):
  `playback.record {"page":1,"slot":1,"mask":["I","C"],"record_by":"Channel","mode":"Merge"}`
  (all but the handle optional; `mode` `Replace`, `Merge` or `Update`, only for a used handle) ·
  `playback.flash {"id":1,"on":true}` · `playback.swop {"id":1,"on":true}` ·
  `playback.latch {"id":1}` (PBK-28) · `playback.release {"id":1}` (PBK-22) ·
  `playback.options {"id":1,"priority":"High","fader_mode":1,…}` and
  `playback.times {"id":1,"fade_in":3.0,…}` (each field optional, out of range refused) ·
  `playback.legend {"id":1,"legend":"Walk-in","icon":null,"halo":{"r":1,"g":0.5,"b":0}}` ·
  `playback.copy {"id":1,"to":{"page":2,"slot":4}}` · `playback.move` (same fields) ·
  `playback.delete {"id":1}` · `page.legend {"page":2,"legend":"Service"}` ·
  `dbo.set {"on":true}` · `settings.playback {…}` (PBK-12 and PBK-20 settings, each optional).

### App (UI v0.1; amends `ui.md` UI-13, 16, 18, 21, 22, 24 when approved)
- **PBK-37** Strips (UI-22): the legend area shows the halo as a 3 pt top bar, the icon (picked
  or automatic) and the legend (else `page.slot`), and a lock mark for `Locked`/`TransparentLock`
  with the home page when it is not the current one. Swop and Flash are live: press and hold
  sends `on: true`, release `on: false`; a latched Flash is shown inverted. Running = green
  bottom strip, releasing = amber bottom strip (UI-03).
- **PBK-38** Function buttons come alive on handles (the target is the strip's legend area):
  **Record** + tap an empty handle records it (PBK-09); + tap a used one shows the Record softkeys
  Replace, Merge and Cancel, and the Record context has Mask and Record By for this record.
  **Update** + tap = PBK-13 Update. **Copy** / **Move**: tap the source, then an empty handle.
  **Delete** + tap, confirmed on softkey A (two-step, PAT-10). **Release** + tap = PBK-22.
  **Latch** + Flash = PBK-28. **Set Legend** + tap a handle opens the legend editor of PAL-29
  (text, icon library, "Auto") with a halo row (Auto, a few colours, a picker); Set Legend + tap
  a page button edits the page legend.
- **PBK-39** **Options** + tap a handle (also the main context's Playback Options softkey after
  tapping a handle, and a right-click on a strip's legend, UI-18) opens a Playback Options screen
  in place of the window grid, side column and attribute bar (as UI-10; status bar and playback
  area stay): Fader (mode, kill point), Times (delay, fade in, fade out, overlap; the Edit Times
  softkey opens this tab), Release (Global/Local mask with IPCGBES toggles, release time),
  Playback (priority, group), Handle (paging). Each change is sent at once; Exit closes it.
- **PBK-40** The Active playbacks window (workspace 4, UI-13) lists every playback that is active
  or releasing, in fire order: home handle, legend, level %, state, attribute groups; a tap on a
  row releases it (PBK-23). Its title info shows "n running".
- **PBK-41** DBO under the GM (UI-24) is live: a click toggles `dbo.set`; while on it is amber and
  the status bar shows the DBO tag (UI-06).
- **PBK-42** `tests/manual/app-playbacks.md` walks recording onto handles with palette references,
  Replace/Merge/Update, the three fader modes with times (read on Art-Net), release mask and
  time, kill point, Release + tap, Flash/Swop/Latch, a locked front-spots handle across pages,
  a playback group, legends with icons and halos, page legends, DBO, and a daemon restart with
  the playbacks back from the show.

### Safety and determinism
- **PBK-43** Every refused playback action returns a warning and changes nothing (PRG-36); no
  playback action, option or content can stop the tick or the output (ENG-12). Same patch and
  same timestamped actions → identical playback state and frames (ENG-11). Fades take their time
  from the tick's timestamp only (the core has no clock).
- **PBK-44** Lenient load (SHW-07): each of these is left out with one warning naming it: a
  playback whose id or home handle is used by an earlier one, a page outside 1–10 or a slot
  below 1, an option or time out of range (set to its default instead, with the warning), a
  second `Locked` playback on a slot (loaded as `Unlocked`), a cue value for a fixture not in
  the patch. A reference to a missing palette becomes its fallback value (PAL-24).
- **PBK-45** The schema 2 → 3 migration (SHW-04) keeps every existing playback where PBK-01 put
  it: id *n* gets home page ⌈n / `faders_per_page`⌉ and slot ((n − 1) mod `faders_per_page`) + 1;
  its `values` become its only cue with times 0 and overlap 100; options at their defaults;
  `next_id` = highest id + 1. An id beyond page 10 is left out with a warning.

## Scenario test format
Scenario playback entries (`engine-merge.md`) gain the optional PBK-34 fields (`page`, `slot`,
options, and `cues`; a plain `values` stays allowed and means one cue with zero times). New
actions: `flash`/`swop {playback, on}`, `latch {playback}`, `release {playback}`,
`record {page, slot, …}` (the `playback.record` fields), `dbo {on}`:

```json
"playbacks": [ { "id": 1, "page": 1, "slot": 1, "fader_mode": 1,
  "cues": [ { "fade_in": 3.0, "fade_out": 4.0, "values": { "1": { "Dimmer": 1.0 } } } ] } ]
{ "t": 0.0, "fader": { "playback": 1, "level": 1.0 } }
{ "t": 2.0, "fader": { "playback": 1, "level": 0.0 } }
"expect": [ { "t": 1.5, "universe": 1, "channels": { "1": 128 } },
            { "t": 4.0, "universe": 1, "channels": { "1": 128 } } ]
```

Recording, Copy/Move/Delete, paging locks, groups, the show file and its migration are unit tests
on `cmdmx-core` (`tests/playbacks.rs`, `tests/showfile.rs`); everything that reaches the frame
(PBK-15..23, 25..28, 30) is a scenario file `tests/scenarios/PBK-nn-*.json`.

## What P1-08 (cue lists) builds on
- A playback already holds a list of cues with their own times (PBK-34), so a cue list is a
  playback with more than one cue: no new storage kind and no second migration.
- Options, kill, release, priority, groups, legends and halos are per playback and apply to a cue
  list unchanged; P1-08 adds Go/Back, the current cue, per-cue link/follow, Append (PBK-13) and
  what Flash/Swop do on a cue list (PBK-29).
- The fade machinery (from current output to target over time with delay and overlap, PBK-15,
  PAL-13/14) is the one a Go uses between cues.
- Cues store touched attributes only by default (PBK-10), so tracking can follow in phase 2.

## Build tasks (`BACKLOG.md`)
1. **VS2-20** core storage: stored handles and locks, record/Replace/Merge/Update,
   Copy/Move/Delete, palette and patch changes, legends, automatic icon and halo, settings, show
   schema 3 with migration and lenient load (PBK-06..14, 24, 31..34, 43..45). Tests:
   `cmdmx-core/tests/playbacks.rs`, `tests/showfile.rs`.
2. **VS2-21** core behaviour on the frame: fader modes with times, references while fading, kill
   point, release, Release, groups, Flash/Swop/Latch, DBO (PBK-15..23, 25..30). Tests: scenarios
   `tests/scenarios/PBK-*.json`, `cmdmx-core/tests/playback_modes.rs`.
3. **VS2-22** IPC and the app: strips, function buttons on handles, Playback Options screen,
   Active playbacks, DBO, `tests/manual/app-playbacks.md` (PBK-35..42).

## Out of scope (phase 1 unless noted)
Fader mode 3 (crossfade cue) and Cross Fade HTP (phase 2); fade curves; per attribute-group and
per-attribute times (Titan *Set Attribute Times*; with P1-08, owner decision 5);
Fixture Order (overlap follows user-number order); key profiles beyond the fixed Swop/Flash;
Preload, Timed Flash, Go on a single-cue playback; Release all (double Release) and the Release
Priority setting; Release to Home; Off Playback with mask; Run On Startup; shapes and effect
options (phase 2); Blind; chases with the BPM and Rate masters (P1-08) and group masters (P1-16; owner decision 6); Size
masters (with shapes, phase 2); virtual faders and executor buttons; physical fader pickup and motor behaviour on page change
(VS1-07, CTL); undo (P1-10).

## Owner decisions (2026-10-18)
All seven proposals accepted by the owner at gate 1.

1. Record By for playbacks defaults to `Channel` (touched attributes only), `Mixed` as option
   per record and as setting (PBK-10).
2. Modes 0/1 use the time model of PBK-15/16 (fade-in on fire scaled by the fader; mode 1 falls
   no faster than full scale per `fade_out`). Reversal trigger: a Titan behaves differently.
3. Flash and Swop (plus Latch) are fixed buttons in phase 1; P1-08 adds Go for cue lists; key
   profiles later (PBK-29).
4. The halo is automatic from the cue's colour by default; a picked colour or none on top (PBK-33).
5. Attribute-group times come with P1-08 (cue lists).
6. Chases with the BPM and Rate masters go into P1-08 (`cuelists.md`); group masters become
   their own small spec, P1-16, later in phase 1.
7. Release defaults as Titan: release time 2 s, empty global release mask (PBK-20).
