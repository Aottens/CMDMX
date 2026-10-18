# ColourmotionDMX — Ideas inbox

The owner drops ideas in chat, in any form. Claude records each one here, triages it and
guards the scope. An idea is **not** a commitment and gets no acceptance criteria until its
phase is planned.

## How an idea moves
1. **new** — recorded below in the owner's intent, with an id `IDEA-nn`.
2. **triaged** — Claude proposes a phase, the module spec it belongs to, what it touches now,
   and open questions. The owner confirms or moves the phase.
3. **specced** — when its phase is planned: numbered criteria in `specs/modules/<module>.md`.
4. **backlog** — a task row in `BACKLOG.md` pointing at those criteria. From here the normal
   loop (spec → test → code → review) applies.
5. **done** / **rejected** (with the reason, so it is not proposed again).

Scope rule: phase 1 is `specs/PRD.md`. A new idea lands in a later phase unless the owner
explicitly pulls it in. If an idea forces a decision *earlier* (because something being
specced now would otherwise have to be redone), Claude flags it and adds that decision to
the backlog, not the whole idea.

## Ideas

| Id | Idea | Status | Phase | Module |
| --- | --- | --- | --- | --- |
| IDEA-01 | More USB-DMX interfaces than the ENTTEC Pro family | triaged | 2 | OUT |
| IDEA-02 | Colour picking in several models: RGB, CMY, HSL/HSI, gel numbers (LEE first) | triaged | 1 (basic) / 2 (gels) | PRG, PAL |
| IDEA-03 | Colour calibration so different fixtures match each other | triaged | 2 | LIB, PAT, core |
| IDEA-04 | Phase 1 = programming the building blocks of any show (robust programmer); cue lists and busking both run on them | accepted 2026-10-17 | 1 | PRD, PRG, new `cuelists.md` |
| IDEA-05 | Timeline synced to Ableton; drag presets onto it for easy timecoded shows | triaged | 3 | new `timeline.md` |
| IDEA-06 | Own web remote: anyone on the network with a password, from an iPad or the Mac's browser (replaces TouchOSC) | triaged | 2 (OSC bridge in 1) | IPC, CTL |
| IDEA-07 | Double-click a wheel to take that one attribute out of the programmer (release instead of recording 0) | accepted 2026-10-17 | 1 (with VS2-05) | PRG, IPC, app |

### IDEA-01 — More USB-DMX interfaces (2026-10-17)
**Owner's intent:** the daemon now speaks only the ENTTEC Pro protocol; other interfaces must follow.
**Triage:**
- Phase 1 already covers every interface that speaks the ENTTEC Pro protocol (ENTTEC DMX USB Pro, Eurolite USB-DMX512 PRO, DMXking ultraDMX — see `enttec::KNOWN_PRODUCT_STRINGS`). Art-Net/sACN nodes cover network interfaces.
- The output model is already one node per interface, so a new driver is a new node type next to `serial_out.rs`; nothing to change now.
- Caution: "Open DMX"-style interfaces (a bare FTDI chip without its own processor) make the Mac generate the DMX timing itself. That is timing-sensitive on macOS and at odds with the show-proof rule; each such interface needs its own decision.
**Open questions:** which interfaces does the owner actually meet (church, rental, own stock)? Add them here by name and model.

### IDEA-02 — Colour picking in several colour models (2026-10-17)
**Owner's intent:** like ONYX, choose a colour as RGB, CMY(K), HSL, or by **LEE filter number** — older lighting techs know those numbers by heart.
**Triage:**
- Basic picker (RGB/CMY/HSI on the encoders) belongs in phase 1 with the programmer (P1-02).
- Gel numbers (LEE first; Rosco/GAM later) in phase 2: needs a gel library with a reliable colour per filter, and only looks right across fixture types with IDEA-03.
- Note: K (black) has no meaning in light; fixtures mix additively (RGB/RGBW/…) or subtractively (CMY). The picker offers CMY; "CMYK" is read as CMY.
**Open questions:** source and licence for the LEE colour data (LEE publishes transmission curves; ready-made RGB tables are often inaccurate). Which other gel ranges matter?

### IDEA-03 — Fixture colour calibration and matching (2026-10-17)
**Owner's intent:** with mixed (often Chinese) LED fixtures, one chosen colour (e.g. one LEE filter) must look the same on two different fixture types; fixtures must be calibratable against each other.
**Triage:** phase 2. Two layers:
- **Per fixture type:** the colour of each emitter (red, green, blue, white, …) as CIE coordinates. GDTF can describe emitters this way (ADR-004); for fixtures without good data a measurement or overlay value fills in.
- **Per fixture instance:** a trim per emitter, because two units of the same model also differ. Set by eye against a reference fixture, or measured with a colour meter.
- The engine then converts one target colour into the right emitter levels per fixture.
**Open questions:** does the owner have (or want) a colour meter, or is matching by eye enough? Is intensity matching (one fixture brighter than another) part of this too?

### IDEA-04 — Big programmed shows are the main use (2026-10-17)
**Owner's intent:** not only busking: cue lists and running whole shows; "very big shows" must be programmable. Busking must work but is not the main thing.
**Triage:** this changes the phase-1 goal in `PROJECT.md` and `specs/PRD.md` ("one evening of busking", cue lists with tracking in phase 3+). Proposal: phase 1 becomes "program and run a show": cue lists (Titan cue list behaviour: cues with fade/delay, go/back, link/follow) move into phase 1; busking (palettes on a grid, playbacks, fader modes) stays. Tracking is the open point — it is what makes big shows editable, and it is a large engine feature.
**What it touches now:** the playbacks spec (P1-04) and the show file; the engine merge already has priorities and playback states that cue lists build on.
**Outcome (2026-10-17):** owner: phase 1 is *programming* — set up the building blocks for any type of show; the focus moves to a robust programmer. `PROJECT.md` and `specs/PRD.md` updated: cue lists (no tracking) join phase 1, tracking goes to phase 2, cues store touched attributes only so tracking can follow. Provisional design target 16 universes / 500 fixtures / 1000 cues, owner to confirm.

### IDEA-05 — Timeline synced to Ableton (2026-10-17)
**Owner's intent:** a timeline that syncs to e.g. Ableton, so timecoded shows are easy to make by dragging presets onto it.
**Triage:** phase 3 (timelines are out of scope for now in `PROJECT.md`; "software first"). Sync options to choose later: MIDI Time Code or LTC (absolute time), MIDI clock or Ableton Link (tempo and beat, no absolute position). Builds on cue lists (IDEA-04) and palettes.
**Open questions:** sync to song position (timecode) or to the beat, or both?

### IDEA-06 — Own web remote with a password (2026-10-17)
**Owner's intent:** in the church a TouchOSC layout on an old iPad mini in another room is the remote today. Rather: our own web app, usable by anyone on the network who has the password, from an iPad or the browser on the Mac there. Hardware control (BCF2000 or similar) in the church is wanted too.
**Triage:** phase 2. It reuses the WebSocket contract (`ipc.md`); it needs a LAN listener, a password/session and a touch-friendly page. For the phase-1 finish line (replace Lightkey) the existing TouchOSC layout can keep working through the OSC schema that phase 1 already has (CTL, P1-05) — so the remote is not lost when Lightkey goes. The BCF2000 is phase 1 already (VS1-07, MCU driver).
**Open questions:** which controls does the TouchOSC layout have today (faders, cue Go, presets)? That defines the OSC schema in P1-05.

### Decision these ideas force now
IDEA-02 and IDEA-03 only work if palettes and cues store a colour as a **target colour** (device-independent) and not as raw DMX channel values. The palettes spec (P1-03) and the show file fix that format, so this decision must come **before** P1-03, otherwise phase 2 means rewriting palettes and show files. → `BACKLOG.md` P1-07.

### IDEA-07 — Double-click a wheel to clear that attribute (2026-10-17, VS2-14 run)
**Owner's intent:** turning White back to 0 % still leaves "White 0 %" in the programmer, so it
would record as a value. Often you want the attribute *out* of the programmer instead, so it
shows through from the playbacks again. Double-clicking the wheel (or the parameter) should do that.
**Triage:**
- The engine side already exists as a concept: PRG-27 **Off** removes touched values by attribute
  group mask (VS2-05, not built yet). This idea is the same action for one attribute of the
  targets, plus a quick gesture in the app.
- Proposed: a PRG criterion "Off for one attribute" next to PRG-27 (`programmer.off {attribute}`
  over IPC), built with VS2-05; double-click on a wheel in the app sends it. The BCF2000 and
  later remotes get their own gesture (e.g. Off + touch the encoder, as consoles do).
- Touches nothing built so far; the Colour wheels (Hue/Saturation/Red/…) would take Colour out
  as a whole, since it is one value (ADR-005).
**Owner decisions (2026-10-17):** phase 1, built with VS2-05. The double-click is on the wheel's
**name** only, not on the wheel itself, so a drag can never trigger it ("safe, but functional").
Clears the attribute for all targets, as wheels act on all targets. The PRG criterion is written
at the start of VS2-05.

### IDEA-08 — Browse the library by manufacturer, then model (2026-10-17, VS2-12 run)
**Owner's intent:** in the Patch screen's Add panel, picking a fixture by search alone is sometimes
awkward; browsing manufacturer → model must be possible too.
**Triage:**
- Proposed phase 1 (the patch screen is phase 1). Needs one LIB criterion: a manufacturer list
  and the models of one manufacturer over IPC (e.g. `library.manufacturers`,
  `library.models {manufacturer}`), answered in the client thread like `library.search`
  (LIB-14); the app shows two columns next to the search field.
- Touches now: `cmdmx-core::library::Index` (already keyed by manufacturer + model), the daemon's
  library requests, the Add panel.
**Outcome (2026-10-17):** owner: phase 1. Spec task P1-14 in `BACKLOG.md`.
**Note (UX-02a run, 2026-10-17):** the owner is not yet sure about the search field plus result list
itself; P1-14 should rethink the whole Add panel (browse columns, search, picking a mode), ideally
with a quick sketch first.

### IDEA-09 — Multi-select in the patch table (2026-10-17, VS2-12 run)
**Owner's intent:** select several rows in the patch table and edit them at once.
**Triage:**
- Proposed phase 1. `patch.remove {ids}` already takes many; move, renumber and change mode of a
  range need PAT criteria for what "at once" means (e.g. renumber 1221–1226 from 1301 keeps the
  order; move a range back to back from one start address, checked as a whole like PAT-02;
  change mode for all, refused as a whole if one does not fit).
- Touches now: the PAT-09 commands (new multi-fixture forms or a new command), the Patch screen's
  Selected panel.
**Outcome (2026-10-17):** owner: phase 1, every edit on a range — remove, move, renumber, change
mode and name (numbered, e.g. "Par 1…6"). Spec task P1-15 in `BACKLOG.md`.

### IDEA-10 — Output lamps on fixture buttons (2026-10-17, UX-02.0)
**Owner's intent:** the sketch's coloured strip on each fixture cell, showing what the fixture
outputs right now (intensity × colour), so the screen shows the stage at a glance.
**Triage:**
- Proposed: later in phase 1, after UX-02d. Needs an IPC addition: each fixture's output colour
  and intensity, sent a few times a second (not every tick) and only for changed fixtures, so the
  tick stays cheap.
- Touches now: only a disabled stripe on the fixture cells (UI-11).
**Open questions:** the owner confirms the phase.

### IDEA-11 — Tap a selected group again to deselect it (2026-10-17, UX-02a run)
**Owner's intent:** clicking the group you just selected a second time should deselect it, as a
fixture tap already does (PRG-01).
**Triage:**
- Proposed phase 1, a small engine task. Amend PRG-02: selecting a group whose fixtures are all
  in the selection removes them from it (the rest of the selection keeps its order). With only
  some of them selected, the tap appends the missing ones, as today. Titan's group buttons
  behave like this.
- Touches now: `cmdmx-core::programmer` (`select_group`), one scenario/unit test, no IPC change
  (`programmer.select_group` stays), and the Groups window shows it through the existing
  selection state.
**Outcome (2026-10-17):** owner: phase 1. Task VS2-15 in `BACKLOG.md`.

### IDEA-12 — Command history with the arrow keys (2026-10-17, UX-02b run)
**Owner's intent:** as in a terminal, Up recalls the previous command on the command line ("nice to
have").
**Triage:**
- Proposed phase 1, built with UX-02c (the token command line and Mac keys): Up and Down step
  through the commands sent in this session, app-side only (no engine change; history is not
  part of the show). Needs one UI criterion next to UI-19.
**Outcome (2026-10-17):** owner: phase 1, built with UX-02c. Open: keep the history across app restarts or
only per session (decided when UI-19 is amended at the start of UX-02c).
**Done (2026-10-18):** UI-27 with UX-02c; history kept across app restarts (owner).

### IDEA-13 — Easy editing of wheel and slot names (2026-10-18, VS2-13)
**Owner's intent:** slot names (colour wheel, gobo, prism, programs) must exist on every fixture,
also ones built from a channel list, and editing them has to be user-friendly; ONYX does this
well (owner does not remember how Avolites does it).
**Triage:**
- Proposed phase 1, with P1-14 (library browsing and the Add panel rethink): a slot editor in
  the profile editor (LIB-12): per channel a list of ranges (from–to DMX, name, optional colour
  or gobo picture), with a split/merge of ranges and a live DMX test on the selected fixture.
  Look at ONYX's fixture editor and Titan's Fixture Tools/Personality Builder before the spec.
- Touches now: nothing. The church Phantom 25 and Fuze 75S (PAT-16) are patched with plain
  wheel channels; their slot data is in the Lightkey show and can be filled in once this exists.
- Open questions: take the slots from the Lightkey show automatically (a Lightkey profile
  importer) or enter them by hand in the editor.
**Outcome (2026-10-18):** owner: phase 1, with P1-14. Same editor: attribute defaults must be
settable there too (owner, VS2-13 review: the church movers start at Pan/Tilt 0, the owner wants
to set e.g. 0.5 per fixture type; LIB-12 already allows editing defaults, the editor is missing).

### IDEA-14 — Icon library for legends, plus a user folder with own drawings (2026-10-18, VS2-13 run)
**Owner's intent:** playbacks, cues, palettes, groups and the like carry pictures, as on Titan
(legend pictures). The difference: ColourmotionDMX ships its own tight, consistent icon
library that can already show most things (colours, positions, gobos, effects, fixture types,
moments in a service), plus a user folder with icons the user draws themselves.
**Triage:**
- Proposed phase 1 for the library and picking it (an `icon` id on each legend, next to the
  text, in the show file); a built-in icon set drawn in one style (SF Symbols-like line icons,
  licence checked); assign from a picker in the app; palettes and playbacks show them on their
  buttons. Specified with P1-03 (palettes) and P1-04 (playbacks), as UI + SHW criteria.
- Proposed phase 2 for the user folder with an icon editor (draw in the app, stored as SVG in
  `~/Library/Application Support/ColourmotionDMX/icons/`, referenced by id and copied into the
  show like profiles, ADR-006 decision 5, so a show travels with its icons).
- Touches now: nothing; the show file schema gets an optional `icon` field when the first
  legend criterion lands (SHW-04 migration not needed for an optional field).
**Outcome (2026-10-18):** owner: phase split as proposed (library + picker in phase 1 with
P1-03/P1-04; user folder in phase 2). The user folder starts with importing SVG/PNG; drawing in
the app comes after that. Automatic icons as on Titan: a colour palette shows its colour, a
position palette its pan/tilt, unless an icon is picked.

### IDEA-15 — More selection orders and patterns: random, 1 in 3, … (2026-10-18, P1-03)
**Owner's intent:** more ways to order and split a selection than Odd/Even: random order, every
third fixture ("1 in 3"), and further options; ONYX has this, as far as the owner remembers.
**Triage:**
- Proposed phase 2, next to Fan/Align (`PROJECT.md`), where selection order matters most
  (fans, overlap, chases across fixtures).
- Touches now: little. The engine already splits a selection into *n* stages (PRG-05,
  `programmer.pattern {"stages": n}`), so "1 in 3" is only a UI control away; the UI shows
  Odd/Even alone. Palette overlap (PAL-14) follows the target order, so a reordered selection
  will stagger palettes too.
- Look at ONYX (selection tools) and Titan (*Select If*, *Shuffle Selection*, pattern select)
  before the spec. Candidate orders: reverse, random/shuffle (a seed kept in the selection, so
  the engine stays deterministic, ENG-11), inside-out, outside-in; patterns: 1 in *n* with an
  offset, blocks of *n*.
- Open questions: is "1 in 3" wanted earlier than phase 2 (it would be a small UI-only task);
  does random reshuffle on every press or stay until the selection changes?
**Outcome (2026-10-18):** owner: phase 2, "1 in 3" included. Random reshuffles on every press
(each press takes a new seed, so the same actions still give the same result, ENG-11).
