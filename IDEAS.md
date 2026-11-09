# CMDMX — Ideas inbox

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

The table below is the index: every `### IDEA-nn` section has a row, and the row's Status moves
along in the same PR that moves the idea; a task status it names ("VS2-26 todo") must match
BACKLOG (`scripts/check-tracking.py` checks all of this in CI).

Scope rule: phase 1 is `specs/PRD.md`. A new idea lands in a later phase unless the owner
explicitly pulls it in. If an idea forces a decision *earlier* (because something being
specced now would otherwise have to be redone), Claude flags it and adds that decision to
the backlog, not the whole idea.

## Ideas

| Id | Idea | Status | Phase | Module |
| --- | --- | --- | --- | --- |
| IDEA-01 | More USB-DMX interfaces than the ENTTEC Pro family | triaged | 2 | OUT |
| IDEA-02 | Colour picking in several models: RGB, CMY, HSL/HSI, gel numbers (LEE first) | basic picker done (VS2-06); gels specified 2026-10-28 (P1-23, #164; VS2-48 and VS2-49 done, VS2-50 blocked) | 1 (owner 2026-10-28, gels too) | PRG, PAL, IPC, UI |
| IDEA-03 | Colour calibration so different fixtures match each other | triaged | 2 | LIB, PAT, core |
| IDEA-04 | Phase 1 = programming the building blocks of any show (robust programmer); cue lists and busking both run on them | accepted 2026-10-17 | 1 | PRD, PRG, new `cuelists.md` |
| IDEA-05 | Timeline synced to Ableton; drag presets onto it for easy timecoded shows | triaged | 3 | new `timeline.md` |
| IDEA-06 | Own web remote: anyone on the network with a password, from an iPad or the Mac's browser (replaces TouchOSC) | triaged | 2 (OSC bridge in 1) | IPC, CTL |
| IDEA-07 | Double-click a wheel to take that one attribute out of the programmer (release instead of recording 0) | done (VS2-05, #66) | 1 | PRG, IPC, app |
| IDEA-08 | Browse the library by manufacturer, then model | done (P1-14; VS2-23 #86, VS2-25 #88) | 1 | LIB, IPC, app |
| IDEA-09 | Multi-select in the patch table: every edit on a range | done (P1-15; VS2-27 #94, VS2-28 #96) | 1 | PAT, app |
| IDEA-10 | Output lamps on fixture buttons (what each fixture outputs now) | in IDEA-35 batch C (C3); spec P1-33 approved (UI-60, IPC-58) | 1, with batch C | IPC, UI |
| IDEA-11 | Tap a selected group again to deselect it | done (VS2-15) | 1 | PRG |
| IDEA-12 | Command history with the arrow keys | done (UX-02c, #33) | 1 | UI |
| IDEA-13 | Easy editing of wheel and slot names, and attribute defaults | done (P1-14; VS2-24 #87, VS2-26 #91) | 1 | LIB, app |
| IDEA-14 | Icon library for legends, plus a user folder with own drawings | done for phase 1 (library + picker, VS2-19/VS2-22); user folder triaged | 1 / 2 (user folder) | PAL, PBK, app |
| IDEA-15 | More selection orders and patterns: random, 1 in 3, …, broadened (owner 2026-10-29) | specified 2026-10-29 (P2-02, approved as proposed; `programmer.md` PRG-71..84, IPC-48, UI-33/34, FX-45; SEL-01..07 todo) | 2 (next, owner 2026-10-29) | PRG, FX, PAL, PBK, IPC, UI |
| IDEA-16 | BPM masters follow Ableton (Link or MIDI clock), with subdivisions and multiples | triaged | 2 | CUE, CTL |
| IDEA-17 | MCP server: build shows with an AI assistant | triaged | 2 | IPC |
| IDEA-18 | FX/shape generator as the best on the market: intuitive, fast, professional results | specified 2026-10-29 (P2-01, `effects.md` FX-01..44, ADR-011; FXB-01..04 done, FXB-05..06 todo) | 2 (flagship), now (owner 2026-10-29) | `effects.md`, ENG, PRG, CUE, PAL, IPC, UI |
| IDEA-19 | Rebrand to CMDMX by Colourmotion | specified 2026-10-27 (P1-21, `brand.md` BRD-01..25; VS8-01 gate3, VS8-02 todo) | 1 (after VS6) | docs, app, README |
| IDEA-20 | Website: the place to download CMDMX | specified 2026-10-27 (P1-21, `website.md` WEB-01..34; VS8-03 done) | 1 (owner 2026-10-27) | `website.md`, `release.md` |
| IDEA-21 | Paid: 1 universe free (full feature set), more universes cost a little; licence model open | triaged | 1 (decision now) / end of 1 (build) | new `licensing.md`, OUT |
| IDEA-22 | Elgato Stream Deck natively: link playbacks to keys intuitively, keys show legend, colour and state | specified 2026-10-25 (P1-19, CTL-44..58; VS7-05, VS7-06, VS7-07 and VS7-09 done) | late 1 (after VS7-04; owner 2026-10-23) | CTL, app |
| IDEA-23 | Open-hardware maker desk: free build plans for a real desk (2× 15" touch, playback faders, buttons) with flight case, show-proof | specified 2026-10-25 (`hardware.md` HW-01..71; HW-01 done) | 10 (hardware; parallel track beside phase 1) | `hardware.md`, CTL, licence |
| IDEA-24 | An info message type, so good news ("autosave works again") is not amber like a warning | triaged | 1 (later, proposed) | IPC, UI |
| IDEA-25 | 2D rig overview from above: fixtures where they hang, select and see output there | triaged | 2 (owner 2026-10-25) | PAT, UI, IPC |
| IDEA-26 | User pages (as in Onyx): own pages with a free layout of windows and widgets (show clock, …) next to fixed default workspaces | triaged | 2 (owner 2026-10-25) | UI, SHW, IPC |
| IDEA-27 | Fan in phase 1: spread a value over the selection order (moving heads above all) | specified 2026-10-25 (P1-20, PRG-56..62; VS2-35 done) | 1 | PRG, UI |
| IDEA-28 | Local AI assistant: a small model on the Mac, a chat window in the app that builds palettes, groups and more on request | triaged | 2, with IDEA-17 (owner 2026-10-25) | IPC, UI, new `assistant.md` |
| IDEA-29 | Serial fade steps: check the "fraction" more steps than Lightkey with an engine cue fade; cut frames to the highest patched channel only if they remain | new | 2 (proposed) | OUT |
| IDEA-30 | Cross-platform UI, so the reference desk runs on Linux (HW-02) | triaged | 10, after the surface (owner 2026-10-25; phase renumbered 2026-11-01) | UI, IPC, daemon on Linux |
| IDEA-31 | Stream Deck + and Neo extras (dials, touch strip, touch keys, info screen; was VS7-08), for a volunteer who owns one | triaged | later (owner 2026-10-27) | CTL, app |
| IDEA-32 | Automatic mixing of extra emitters (White, Warm White, Amber, Lime, …) from the target colour, plus a colour-temperature view (Kelvin, CTO/CTB steps) | done (2026-10-28, VS2-44..46: #159, #160, #161) | 1 (owner 2026-10-28) | LIB, ENG, PRG, IPC, UI |
| IDEA-33 | The FX engine is still sparse; much more should go in later (owner 2026-10-29) | triaged; phases confirmed 2026-10-29 | 2, in parts (owner 2026-10-29; part 7 after IDEA-25) | `effects.md`, ENG, PRG, CUE, PBK, IPC, UI |
| IDEA-34 | Multi-user support in the software | triaged | 3 (owner 2026-11-01) | ENG, PRG, IPC, UI, undo |
| IDEA-35 | Everyday gaps: actions an operator takes for granted that nobody wrote down (`docs/research/everyday-gaps.md`) | batch A done (v0.3.0); second review 2026-11-02 added A2, H, I, J; third review 2026-11-04 proposes K into the batches; batch C specified (P1-33); batch D specified (P1-36); batch E specified (P1-39) | 1, in order A2, H, B to G, I, J (owner 2026-11-02); H specified (P1-27) | PRG, CUE, PBK, PAT, LIB, SHW, CTL, IPC, UI |
| IDEA-36 | A refused number stays in its field: the field shows a value the engine does not hold (SPF-07 run, 2026-11-02) | REC-02 done (UI-52) | 1 (owner 2026-11-02) | UI |
| IDEA-37 | The app takes over when another engine goes away: today it stays offline (DEF-03 run, 2026-11-02) | triaged; awaiting phase confirmation | 1 proposed | REL |
| IDEA-38 | Workspaces that are not on show leave the accessibility tree (FUN-01 run, 2026-11-03) | triaged; awaiting phase confirmation | 1 proposed | UI |
| IDEA-39 | Strips are cut off on a window with the Dock (FUN-01 run, 2026-11-03) | spec UI-63, build FIT-01 | 1 (owner 2026-11-07) | UI |
| IDEA-40 | A function's hint stays after it is disarmed (FUN-03 run, 2026-11-03) | spec P1-30 proposed (UI-50) | 1 (owner 2026-11-04) | UI |
| IDEA-41 | Our own built-in 3D visualiser, inside the app (owner 2026-11-04) | triaged | 4 (owner 2026-11-04) | new `visualiser.md`, PAT, LIB, IPC, UI |
| IDEA-42 | A phone page the console serves on the network, with a login set in the console: walk the patch fixture by fixture, readdress, highlight to check | spec P1-34 approved (`remote.md` RMT-01..16); RMB-01, RMB-02 and RMB-03 done; parking on the phone with PRK-02 | 1 (owner 2026-11-06, before Christmas) | new `remote.md`, PAT, PRG, IPC, daemon |
| IDEA-43 | Park fixtures: the rig on paper is bigger than what hangs; a parked fixture leaves output, groups and selection, its programming kept | spec P1-35 proposed (`patch.md` PAT-29..36; PRK-01..02 todo) | 1 (owner 2026-11-06) | PAT, PRG, GRM, IPC, UI |
| IDEA-44 | The live stripes on the fixture tiles step visibly during a fade or a fader move | done (IPC-58, IPC-59 at 25 a second; SEE-11 done) | 1 (owner 2026-11-07) | IPC, UI |
| IDEA-45 | A tracked value's source names the cue running now, not the cue it came from (SEE-09 run, 2026-11-07) | triaged | 1 proposed, owner to confirm | IPC, ENG, UI |
| IDEA-46 | Unfold a cue list or chase onto the faders: each cue on its own fader, edited there (Titan's Unfold) | triaged; out of batch D (owner decision 3, P1-36) | 1, later, its own spec | CUE, PBK, UI |
| IDEA-47 | A linked playback copy: the same cues on another handle, with its own times and options (Titan's Link) | triaged; out of batch D (owner decision 4, P1-36) | later, phase to confirm | PBK, CUE, SHW |
| IDEA-48 | A BCF2000 behind a MIDI interface: fresh feedback after a power-cycle, the profile found without a name match, a way to reach slots 9 and up | new, from the VS1-07 run | later, phase to confirm | CTL |
| IDEA-49 | Titan's Exchange Mapping screen: see and change how old attributes map to new ones on an exchange | triaged; out of batch E (owner decision 3, P1-39) | later, phase to confirm | PAT, UI |
| IDEA-50 | Freeze an attribute of a fixture at the output | triaged; out of batch E (owner decision 7, P1-39) | 1, later | PAT, ENG |
| IDEA-51 | Output curves per fixture and attribute | triaged; out of batch E (owner decision 7, P1-39) | 1, later | PAT, ENG |
| IDEA-52 | Flip a moving head to its other yoke position | triaged; out of batch E (owner decision 7, P1-39) | 1, later | LIB, PRG |
| IDEA-53 | Lamp on/off and fixture macros through the control channel | triaged; out of batch E (owner decision 7, P1-39) | 1, later | LIB, PRG, UI |
| IDEA-54 | RDM patching: discover and address fixtures on an RDM-enabled Art-Net node | triaged; out of batch E (owner decision 7, P1-39) | 1, later | OUT, PAT |

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
**Outcome (2026-10-28):** owner picked the gel part for phase 1 now (ONYX-style), after the Kelvin/CTO/CTB view (IDEA-32). Specified as P1-23: PRG-67..70, IPC-47, UI-32, PAL-33, approved by owner 2026-10-28 with the four questions as proposed (tungsten 3200 K; the owner asks LEE; test rows only until LEE answers; `Lee n` with the L key and the gel legend). What the research found:
- **Data:** each filter page on leefilters.com publishes CIE x, y and transmission Y for two sources, Tungsten (3200 K) and Daylight (Source C), plus the nominal mired shift; the spectral curve is in the page as 5 nm points from 405 to 700 nm only. LEE's own x, y come from its full measurement, so CMDMX uses those instead of recomputing from the truncated curve: no spectral maths and no CIE tables in the engine. No open gel dataset with a clear licence exists (the R package photobiologyFilters has seven LEE gels, digitised from plots).
- **Source: tungsten 3200 K.** A gel number means the colour it gives on a theatre lamp; ETC Eos (575 W HPL in a Source Four) and Hog use a tungsten reference too. On tungsten LEE 201 is a 5600 K daylight white, as lighting techs know it; on a daylight source it would be a deep blue. D65 (ADR-005's white) was the other candidate: it is "the gel on the fixture's white", but no tech knows the numbers that way.
- **CT agreement:** with LEE's tungsten x, y, every CT gel lands within 6 mired of 3200 K plus the PRG-66 step, except 204 Full CTO (15 mired: LEE's data says 2190 K, nominal +159 says 2120 K). PRG-66's ½, ¼, ⅛ CTO values match what LEE 205, 206, 223 do on tungsten (+79, +41, +26), not the +109, +64, +26 LEE prints for daylight. PRG-66 stays as approved.
- **Licence:** the pages say "Copyright Panavision" (LEE's owner); there is no licence for reuse, and a whole table of LEE's values is a database extraction (UK/EU database right), whatever the copyright status of single numbers. Proposed: the owner asks LEE/Panavision for written permission to ship number, name and tungsten x, y with attribution; tests use a handful of rows meanwhile; the full table lands when LEE says yes (VS2-50). If LEE says no: a user-supplied gel file instead (later idea).
- **Palettes** store only the colour; the gel is read back from it (PRG-69), so a palette shows its gel without the show file changing.

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
**Research (2026-11-09):** the LG Lighting Smart 5 records running cues against an audio file, edits them on a waveform and plays shows from a list, with MIDI and LTC on the same recorder; a workflow to weigh when this idea is planned (`docs/research/smart5-console.md`). Phase unchanged.

### IDEA-06 — Own web remote with a password (2026-10-17)
**Owner's intent:** in the church a TouchOSC layout on an old iPad mini in another room is the remote today. Rather: our own web app, usable by anyone on the network who has the password, from an iPad or the browser on the Mac there. Hardware control (BCF2000 or similar) in the church is wanted too.
**Triage:** phase 2. It reuses the WebSocket contract (`ipc.md`); it needs a LAN listener, a password/session and a touch-friendly page. For the phase-1 finish line (replace Lightkey) the existing TouchOSC layout can keep working through the OSC schema that phase 1 already has (CTL, P1-05) — so the remote is not lost when Lightkey goes. The BCF2000 is phase 1 already (VS1-07, MCU driver).
**Open questions:** which controls does the TouchOSC layout have today (faders, cue Go, presets)? That defines the OSC schema in P1-05. (Asked again as question 8 of `control-surfaces.md`; custom-address Learn, CTL-38, keeps the layout unchanged. 2026-10-24: the owner chose a new layout made for ColourmotionDMX instead, VS4-04.)

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
**Specified (2026-10-23, P1-14):** LIB-19..22, IPC-35..36; build tasks VS2-23 and VS2-25.

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
**Specified (2026-10-24, P1-15):** PAT-19..25, IPC-40; build tasks VS2-27 and VS2-28.
**Done (2026-10-24):** engine VS2-27 (#94), app VS2-28 (#96); run `tests/manual/runs/2026-10-24-VS2-28.md`.

### IDEA-10 — Output lamps on fixture buttons (2026-10-17, UX-02.0)
**Owner's intent:** the sketch's coloured strip on each fixture cell, showing what the fixture
outputs right now (intensity × colour), so the screen shows the stage at a glance.
**Triage:**
- Proposed: later in phase 1, after UX-02d. Needs an IPC addition: each fixture's output colour
  and intensity, sent a few times a second (not every tick) and only for changed fixtures, so the
  tick stays cheap.
- Touches now: only a disabled stripe on the fixture cells (UI-11).
- FXB-06 run (2026-10-29): FX-41 (the fixture tiles show a chase run) waits for this; the run
  judged effects on a stage view Claude drew from the Art-Net frames.
**Open questions:** the owner confirms the phase. Batch C (P1-33, 2026-11-05) takes it as C3: UI-60 on the feed of IPC-58.

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
**Specified (2026-10-23, P1-14):** LIB-23..27, IPC-37..39 (slots per channel, not ONYX's linked
wheels; slots typed by hand, a Lightkey importer stays open); build tasks VS2-24 and VS2-26.

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
**Outcome (2026-10-29):** owner picked selection orders as the next feature after the FX generator,
"but more than I had in the idea". Researched in `docs/research/selection-orders.md` (Titan, grandMA3
MAtricks, Onyx grouping tools) and specified as P2-02 (approved by the owner 2026-10-29, the four decisions as proposed):
- **Order and grid are two things.** Order tools permute the selection: Reverse, Shuffle (a seeded
  permutation, new every press), Sort, Revert (Onyx, Titan's Random Order). The grid gives items
  shared positions with the FX generator's own function (FX-07): wings, groups, blocks, plus a flip
  for Inside out (MA3 MAtricks). Onyx's Every / Block / Divide / Mirror and Titan's `2 In 6` are
  shortcuts onto it: Odd/Even, 1 in *n*, *n* in *m*, Blocks *n*, Divide *n*, Outside in, Inside out.
- **One set of positions for everything that spreads:** Next/Previous step stage by stage; Fan,
  a new effect, palette overlap and cue overlap read the same positions. All ends the stage and keeps
  the grid, so Fan and effects run across the stages (odd against even; Onyx's Fan/FX toggle).
- **Stored order:** a group keeps its order and its grid; a cue keeps each item's position, so its
  overlap replays the recorded order (Titan); a fixture-order editor for cues, with shared step
  numbers (Titan *Fixture Order*); an effect's members keep their order, with Reverse/Shuffle/Sort
  on a running entry.
- **Later:** MA3's Width, Shift, X/Y/Z axes and From/To layers, a MAtricks pool; Titan's 2D Direction
  (with IDEA-25); a keypad pattern syntax; Select If.
Criteria: `programmer.md` PRG-71..84 (PRG-05/06 amended), `ipc.md` IPC-48, `ui.md` UI-33/34,
`effects.md` FX-45 (FX-06, 07, 14, 38 amended), PAL-14 and PBK-15 amended; build rows SEL-01..07.

### IDEA-16 — BPM masters follow Ableton, with subdivisions and multiples (2026-10-21, P1-08)
**Owner's intent:** besides Tap and typing, a BPM master can sync to Ableton and run chases at
subdivisions or multiples of Ableton's tempo ("isn't that a sick new feature?").
**Triage:**
- Proposed phase 2 (busking depth), before the Ableton timeline of IDEA-05 (phase 3), which
  needs the same link. Owner to confirm or pull it into phase 1.
- Touches now: only the shape of the BPM master. `cuelists.md` CUE-35 gives it a tempo set by
  Tap or typing plus a multiplier (/4 … ×4); a sync source would be a third way to set that
  tempo, so nothing has to be redone. No criteria until its phase is planned.
- Two ways in: **Ableton Link** (tempo *and* beat phase over the network, so chase steps can land
  on Ableton's beat; also used by Traktor, Rekordbox and many apps) or **MIDI clock** (24 pulses
  per beat over CoreMIDI/IAC, tempo only, no phase; the daemon gets MIDI anyway for CTL).
- Open questions: Link's SDK is C++ and dual-licensed GPLv2+ or proprietary (check before
  choosing), so with Link the open licence question (MIT or GPL-3.0, `BACKLOG.md`) leans to GPL;
  MIDI clock has no such tie. Should a chase step on the beat (phase), or only follow the tempo?
**Outcome (2026-10-21):** owner: phase 2.

### IDEA-17 — MCP server: build shows with an AI assistant (2026-10-21, VS3-05 run)
**Owner's intent:** an MCP (Model Context Protocol) server in ColourmotionDMX, so an assistant
such as Claude can build shows smartly: patch, groups, palettes, cues and cue lists by asking,
on the same engine the operator uses ("that is new, we must have it").
**Triage:**
- Proposed phase 2 (once the phase-1 programming model is stable); owner to confirm or pull it
  forward.
- Touches now: only the IPC contract. Every `ipc.md` command is already a natural MCP tool and
  the snapshot a natural resource, so the MCP server can be a small separate client of the same
  WebSocket JSON (ADR-002, like the remotes of IDEA-06), with no change to the engine. Until its
  phase the one requirement is to keep every command in `ipc.md` complete and documented, as
  now. No criteria until its phase is planned.
- Show-proof: it goes through the same refusals and warnings as the app; it must never stop the
  tick, and must not change live output during a show without the operator (a blind / prepare
  mode, or a confirm in the app); loopback only as IPC-01/02, authentication with IDEA-06.
- Open questions: what should it do first (build a service list from a running order, palettes
  for a patched rig, import a Lightkey or other show, explain what is on stage)? Blind only or
  also live? Its own binary (stdio MCP, Rust or Swift) or inside the daemon? Which clients
  (Claude Desktop, Claude Code, others)?
**Outcome (2026-10-21):** owner: phase 2; details later.

### IDEA-18 — The best FX generator on the market (2026-10-22)
**Owner's intent:** the effects (shape) generator must become the best in the market. Moving
from Titan to ONYX was a disappointment: getting a good result there was hard work. Here it has
to be intuitive and fast, and easily give a professional result. How is not clear yet, but it
must be taken along in everything we build. The owner wants to invest heavily in it.
**Triage:**
- Phase 2 already holds the "shape generator" (`PROJECT.md`); this idea makes it a **flagship**
  of phase 2, not a checkbox, and asks phase 1 not to block it.
- Before any spec: a design study. Compare Titan (shapes: pick a shape, size/speed/spread on
  the encoders, per attribute group), ONYX (FX engine, where the pain was), grandMA3 (phasers:
  steps as keyframes, powerful but abstract) and ChamSys/Hog (FX palettes). Write down what
  made Titan quick and ONYX slow, then sketch the UI before criteria (as with P1-14).
- Directions to weigh: start from a result, not a waveform (a gallery of ready looks with a
  live preview on the fixture buttons, IDEA-10); a few big controls (size, speed, spread,
  direction) on the encoders and BCF2000; spread follows the selection order (IDEA-15: odd/even,
  1 in 3, random, inside-out); speed locks to a BPM master (CUE-35, IDEA-16); effects as
  palettes and in cues; effects relative to the base value (circle around a position palette)
  or absolute.
- **What it touches now** (phase 1 must leave room, no FX code yet):
  - *Engine merge:* an effect is mostly a *relative offset* on top of the value from the
    programmer/cues, evaluated from the tick time (deterministic, ENG-11). The merge needs a
    place for that layer; adding it later should not mean redoing `engine-merge.md`.
  - *Cues and palettes:* they will have to store effect references next to attribute values.
    Show schema 4 just landed; an optional field later needs no migration, but the shape of a
    cue's contents should allow it.
  - *Fixture library:* the FX attribute group already exists (IPCGBES-FX); built-in fixture
    macros on a channel are a different thing from generated effects.
**Open questions:**
- What exactly made ONYX hard: too many parameters, unclear spread/phase, no preview, effects
  that would not combine with cues, something else? And what did Titan do right?
- Pull a small first version into phase 1 (a few shapes: dimmer chase, colour, pan/tilt circle;
  size/speed/spread), or keep it all in phase 2 and only guard the engine shape now?
- Fix the two engine points above (relative layer in the merge, room for effects in cues) now
  as a decision in `BACKLOG.md`, before cue playback is built on the merge?
**Outcome (2026-10-22):** owner: fix the two engine points now → `BACKLOG.md` P1-17 (ADR-007). Cue playback (VS3-03..05)
was already built by then, so the ADR also checks it against the relative layer.
The phase and a possible small first version in phase 1 are still open.
ADR-007 approved (owner, 2026-10-22): `engine-merge.md` ENG-14..19 and an optional effect reference in cue
and palette contents.
**Outcome (2026-10-29):** owner picked the FX generator as the next feature (phase 2 starts
here). Specified as P2-01: ADR-011 (an effect is an *entry*: shape + parameters + ordered members,
owned by the programmer, a cue or a palette; it amends where ADR-007's effect reference lives)
and `effects.md` FX-01..44, with build rows FXB-01..06. What the design study settled:
- **Start from a result.** A gallery of fourteen looks (Dimmer chase, Circle, Rainbow wave, …)
  with good defaults runs at once on the selection; Size, Speed and Spread are the three big
  controls on screen and encoders (Titan's speed); raw shapes are behind a *Looks | Shapes*
  switch. ONYX's page of parameters before anything moves is what to avoid.
- **Relative by default** (a circle around the cue's position, Titan), **absolute for dimmer
  waves and colour shapes** (a dimmer chase on a dark rig still runs: Titan's beginner trap).
- **One effect per attribute** (ENG-15); a Path covers Pan + Tilt together.
- **Spread over the selection order** with grandMA3's wings, groups and blocks; "1 in 3" from
  IDEA-15 is groups 3. Random member order stays with IDEA-15.
- **Tempo** from the chase speed sources (Free, BPM masters with Tap, Rate masters); Ableton
  Link / MIDI clock (IDEA-16) later as a BPM master source.
- **Colour** on the one target colour (ADR-005): Rainbow, Hue shift, Colour mix over 2–8
  colours; auto-mix (ADR-010) runs after the effect, so it needs nothing new.
- **Later:** user key-frame shapes, layered effects, size masters, the pixel mapper.

Approved by the owner 2026-10-29 with the four decisions as proposed. The owner: "not like ONYX,
which I find not user-friendly"; which ONYX pain mattered most is still to hear, and each FXB
run checks the result against it.

### IDEA-19 — Rebrand to CMDMX (2026-10-22)
**Owner's intent:** rename the product to CMDMX, written in all caps.
**Triage:**
- The crates are already `cmdmx-*`, so the code barely changes. What changes: the app name and
  bundle (`scripts/bundle-app.sh`, `Info.plist`, window titles), README, PROJECT.md, the docs,
  the GitHub repo name (`Aottens/ColourmotionDMX` → e.g. `Aottens/CMDMX`; GitHub redirects the
  old URL) and the local folder.
- Cheap now, more expensive with every PR. One small task, no criteria needed beyond "no user-
  visible ColourmotionDMX left".
**Open questions:**
- Does "Colourmotion" stay as the maker's name ("CMDMX by Colourmotion"), or go away completely?
- Rename the GitHub repo and the bundle id (`…colourmotiondmx` → `…cmdmx`) too? An existing show
  file / settings folder then moves: fine now, there are no users yet.
- Check the name is free: a quick search for existing "CMDMX" products and domains
  (cmdmx.com / .app / .nl) before the website is built on it.
**Owner's answer (2026-10-22):** the brand is **"CMDMX by Colourmotion"**. CMDMX is the product
name; Colourmotion stays as the maker.
**Timing (owner, 2026-10-22):** the rebrand runs as its own task right **after VS6** (multi-universe
output, VS6-01..03) is done, not before.

### IDEA-20 — Website to download CMDMX (2026-10-22)
**Owner's intent:** a website that becomes the place where you download the tool.
**Triage:**
- Needs a release to download: a signed and **notarized** `.app` (or `.dmg`). Without
  notarization macOS Gatekeeper blocks the download for normal users. That takes an Apple
  Developer account (€99/year) and a signing step in `scripts/bundle-app.sh` / CI.
- Small static site (download button, screenshots, what it does, price, docs link). Hosting can
  be GitHub Pages or similar; downloads from GitHub Releases. Updates in the app (Sparkle)
  come later.
- Fits at the end of phase 1, when there is a first release worth downloading. The site itself
  can be a separate subproject (`subprojects/website/`).
**Open questions:**
- Domain name (follows IDEA-19).
- Dutch, English or both? (The audience beyond the church is international.)
- When: placeholder page now ("coming soon") or only with the first release?

### IDEA-21 — Paid: 1 universe free, more costs a little (2026-10-22)
**Owner's intent:** the tool is paid after all. One universe is free; more universes cost
money. Not much, but something.
**Triage:**
- **Conflicts with what is written now:** `PROJECT.md` lists "any licensing dongle or paywall"
  as out of scope, and README/PROJECT call the project open-source. Both change if this is
  accepted.
- **Forces a decision now (licence).** The open question "MIT or GPL-3.0" is no longer the right
  question. With MIT or GPL anyone may remove the universe limit and redistribute it. Options:
  1. *Source-available* (e.g. PolyForm Noncommercial / Business Source Licence / Fair Source):
     code stays public on GitHub, but using more than one universe needs a paid licence.
  2. *Open source + paid builds*: GPL code, the limit only in the official download; whoever
     builds it themselves gets everything. Honest, but the limit is a courtesy.
  3. *Closed source*: the repo goes private.
- **Forces a decision now (engine shape).** Multi-universe output (VS6) is being built right now.
  The limit should sit in **one** place: the daemon counts active output universes and refuses
  more than the licence allows, as a warning in the UI. Core stays pure (licence state comes in
  as a value).
- **Show-proof:** a licence check may never stop output during a show. A missing, broken or
  expired licence only blocks *patching/enabling* an extra universe; universes that are running
  keep running. The check works offline (a signed licence key checked locally, Ed25519); no
  internet needed at the venue.
- Selling: a "merchant of record" (Paddle, Lemon Squeezy) handles payment, EU VAT and licence
  keys; the site from IDEA-20 links to it. Pricing: a one-off price per universe count, or tiers
  (e.g. 1 free / 4 / unlimited).
**Open questions:**
- Licence model: 1, 2 or 3 above?
- Tiers and price (per extra universe, or fixed tiers; one-off or with paid major updates)?
- Does the church get a free full licence?
- Is "universe" the counter: what about Art-Net/sACN output the engine sends to several nodes
  (does the same universe sent to two nodes count once)?
**Owner's answers (2026-10-22):**
- Licence model: still open, the owner is thinking about it.
- The **only** limit is the universe count. No features are held back: the free version is the
  full pro tool on one universe. Many churches and small venues are fully served by that; you pay
  only when you really need more.
- Avolites: CMDMX follows Titan's *behaviour* with its own code, which is not infringement.
  Still to keep in mind for the website: "Avolites" and "Titan" are trademarks. A descriptive
  mention ("works the way Titan users expect") is fine; no logos, no look-alike branding, and
  no copied manual text or screenshots.

### IDEA-22 — Stream Deck natively compatible (2026-10-23)
**Owner's intent:** make Elgato Stream Decks work natively with CMDMX: link a Stream Deck to
playbacks in an intuitive way, straight from the software.
**Triage:** late phase 1, after VS7-04 (owner, 2026-10-23; spec task P1-19), as a surface profile on the general control layer
(`control-surfaces.md`, CTL). A Stream Deck is not MIDI: it is a USB HID device with an LCD per
key (Stream Deck+ also has dials and a touch strip). Two routes:
1. **Native HID in the daemon** (`hidapi`): CMDMX drives the keys itself, draws each key image
   (playback legend, icon, halo colour, level bar, active/flash state), follows pages; no Elgato
   app needed, but the Elgato app must not hold the device at the same time.
2. **An Elgato Stream Deck plugin** (Elgato SDK) that talks to CMDMX over OSC (CTL-36) or IPC:
   lives next to other Stream Deck actions, needs the Elgato app running.
"Intuitive": in the app, drag a playback (or Go, page, a palette) onto a key of an on-screen
Stream Deck, or Learn by pressing the key (CTL-25); a key follows its handle like a fader does
(CTL-21).
**What it touches now:** nothing forced. The CTL action catalogue already fits (`upper`/`lower`,
`go`, `page`, `palette` …); route 1 adds a feedback kind "key image" (legend, colour, state) to
CTL-17 and one dependency (`hidapi`) in the daemon when its phase comes.
**Open questions:**
- Route 1 (native, no Elgato app), route 2 (plugin), or both?
- Which models: Stream Deck (15), Mini, XL, Stream Deck+ (dials for wheels/levels), Neo?
- What a playback key does by default: Go, Flash or Swop (as the handle's buttons, CUE-25)?

**Specified 2026-10-25 (P1-19):** route 1, native HID from the daemon (ADR-009); all models as a
table; a playback key is its handle's `upper` button; CTL-44..58, build tasks VS7-05..08.

### IDEA-23 — Open-hardware desk for the maker community (2026-10-23)
**Owner's intent:** CMDMX becomes a lighting desk for the maker community. The software stays
1 universe free, more for a small fee (IDEA-21). Next to it, open build plans so anyone can
build a real, full-size desk: two 15" touch screens, playback faders under them, the needed
keys, a flight case design, and show-proof. A ~€1000–2500 build would beat desks that cost
many times more.
**Triage:**
- **The engine does not change.** Under "control is general MIDI + OSC" (CTL, VS7) the desk
  surface is one more controller profile, like the BCF2000: a microcontroller (Teensy 4.1 /
  RP2040) reads faders, keys and encoders and shows up as a USB-MIDI device. Fader count stays
  configuration (`faders_per_page`).
- **Build path in steps:** (1) the surface as a USB-MIDI box on today's Mac app; (2) Mac mini
  plus two screens in an enclosure = a full desk on the current software; (3) a Linux/ARM build
  (Pi 5, RK3588, N100) for a cheaper, Apple-free desk.
- **Step 3 needs a cross-platform UI.** The engine already builds on Linux; the UI is SwiftUI.
  The UI talks to the daemon over WebSocket and holds no lighting logic, so it is a rewrite of
  the screens, not of the console. `PROJECT.md` lists "Windows/Linux UI" out of scope; that
  changes if step 3 is accepted. For makers a Linux build is the natural target.
- **Licence gets a hardware axis.** Plans (PCB, firmware, CAD, BOM, build guide) need their
  own licence, e.g. CERN-OHL-S/W or CC BY-SA, separate from the software choice in IDEA-21.
  Not forced now.
- **Show-proof hardware:** no SD card (NVMe), read-only root filesystem, locking power
  (PowerCON/IEC lock) and a small UPS or supercap, locking USB/Ethernet (etherCON), a firmware
  watchdog, and the surface failing safe: if the UI crashes, the daemon keeps output alive
  (already the rule), and a lost surface never changes levels.
- **Flight case:** parametric design around the desk (standard case hardware, birch ply with
  aluminium extrusion), published as drawings a case maker or a maker can build from.
- **Repo shape:** a `subprojects/hardware/` (KiCad PCB, firmware, enclosure and case CAD, BOM,
  build guide), hosted on the IDEA-20 website. Hardware tasks ship with owner-only checklists
  (gate 3).
**Open questions:**
- Step 3 at all: Mac mini only, or a Linux/ARM build with a cross-platform UI?
- Layout: how many playback faders and keys per fader, encoders, keypad (closer to Titan
  Mobile or Tiger Touch)?
- Motorised faders (clean page changes, ~€15–25 each) or pickup/catch on plain faders?
- DMX XLR outputs built in (RS485 on the surface MCU) or Ethernet/Art-Net only plus a USB
  interface?
- Plans only, or also sell kits/PCBs later?
- Hardware licence (CERN-OHL-S, CERN-OHL-W, CC BY-SA)?
**Owner's answer (2026-10-23):** phase 3 becomes the hardware phase; IDEA-23 goes there.
**Research (2026-10-23):** candidate parts, sample BOMs and open questions in `docs/research/hardware-desk.md`; §5a records the owner's modular fader-strip boards (2 faders per JLCPCB board on a bus to one main controller).
**Owner's answer (2026-10-25):** phase 3 starts now as a parallel track beside phase 1 (DECISIONS); `subprojects/hardware/`, first task HW-01 (the open questions, then `hardware.md`).
**Specified (2026-10-25, HW-01):** the owner answered the eleven questions (`hardware.md`, *Owner decisions*): a Tiger Touch II-size desk with one touch screen, Linux reference compute (IDEA-30), 10 motorised playback + 10 preset faders, four optical wheels, four isolated DMX ports, CERN-OHL-S v2. Build tasks HWB-01..09, the USB-MIDI box first.
**Owner's answer (2026-11-01):** the hardware phase moves from 3 to 10, which leaves phases up to 9 for other work (DECISIONS).
**Research (2026-11-09):** the finished LG Lighting Smart 5 (26 faders, 8 DMX ports) lists at about $910–950, below the ~€1000–2500 build, so the "beats desks that cost many times more" argument does not hold against it. The case rests on openness, repair, show-proof behaviour and updates that do not depend on one maker (`docs/research/smart5-console.md`). Scope unchanged.

### IDEA-24 — An info message type for good news (2026-10-18, SHW-c run; recorded 2026-10-24)
**Owner's intent (from the SHW-c run, finding 1):** "autosave of <name> works again" is shown in
amber, as every engine message is, so good news looks like a warning.
**Triage:**
- Proposed later in phase 1: an `info` level next to the warning level on engine messages
  (`ipc.md`), shown in a calm colour in the message zone (`ui.md`).
- Touches now: nothing; the SHW-13 text stays as it is.
**Open questions:** the owner confirms the phase.

### IDEA-25 — 2D rig overview from above (2026-10-25)
**Owner's intent:** a 2D overview of the rig, seen from above.
**Triage:**
- Phase 2 (the PRD keeps the visualiser out of phase 1; a 2D plan is the light part of
  it). A window that draws every patched fixture at its spot on a top-down plan; tap or
  lasso to select (PRG), and each fixture shows its live output colour and intensity (the same
  feed IDEA-10 needs).
- Titan reference: the Fixtures window's layout view (fixture buttons placed freely). Onyx has a
  similar fixture layout.
- Patch gets a position per fixture (x, y, and later rotation; metres or grid units), set by
  dragging on the plan. Patching from the plan stays out of scope (`patch.md`).
- It is a natural window type for a user page (IDEA-26).
**What it touches now:** nothing forced. The position is an extra optional field on a patched
fixture in the show file (SHW), added when its phase comes; old shows load without it.
**Open questions:**
- Free placement only, or also helpers (grid snap, "line up selected", truss bars as drawing)?
- A background image (the venue's floor plan) under the fixtures?
- Does a fixture show beam direction (pan/tilt), or only colour and intensity?
**Owner's answer (2026-10-25):** phase 2.

### IDEA-26 — User pages, as in Onyx (2026-10-25)
**Owner's intent:** in Onyx you can make a page and lay it out yourself; that worked well. Best
of both worlds: we steer with default pages (the workspaces we think are right, not editable),
and the user can add own pages next to them, with things like a show clock.
**Triage:**
- Phase 2. The fixed workspaces of UI-07/UI-08 stay as they are: they are the default
  pages and cannot be edited. User pages are added below them in the rail (F5 onward), are
  stored in the show file and never change show state (UI-08's rule).
- A user page is a free grid: the user places and sizes windows from a catalogue — the existing
  windows (Fixtures, Groups, palettes, Programmer, Cue list, Active playbacks, …) plus widgets
  that only make sense here: show clock, countdown/timer to a time, the rig overview (IDEA-25),
  a few playback faders or keys, macro buttons, a text note.
- Show clock: the UI has a wall clock in the status bar (UI-06); a show clock adds a big
  clock, a stopwatch since show start or since a cue, and a countdown. Time comes from the
  daemon (core stays time-as-parameter), so a remote (IDEA-06) shows the same.
- Duplicate a default page as a start for an own page, so the defaults teach the layout.
**What it touches now:** nothing forced. UI-08 keeps fixed workspaces; windows are already
self-contained (UI-09), which is what a free grid needs.
**Open questions:**
- Grid (cells that snap) or fully free placement? A grid is quicker and stays tidy on other
  screen sizes.
- Per show (in the show file) or per user/Mac (so own pages travel between shows)?
- Which widgets first besides the show clock?
- Does a user page also replace the side column and attribute bar, or only the 2×2 grid?
**Owner's answer (2026-10-25):** phase 2.

### IDEA-27 — Fan belongs to basic programming (2026-10-25)
**Owner's intent:** plain fanning is part of basic programming, moving heads above all, so it
goes in phase 1 instead of phase 2.
**Owner's decision (2026-10-25):** Fan moves to phase 1; Align stays in phase 2. Spec task
P1-20 in `BACKLOG.md`.
**Triage:**
- Titan: Fan mode on the Fan key; while it is on, a wheel spreads its change over the selected
  fixtures in selection order (the middle fixture stays, the ends move most for Line), with fan
  curves on the softkeys. UI v0.1 already sketches this: Fan key, curves Line, Mirror, Wings,
  Arrow on the softkeys, and a FAN tag on the attribute bar (`specs/ux/main-screen.html`).
- It is programmer logic (PRG): the core computes the per-fixture values; the app only sends
  the wheel delta with "fan on" and the curve, so the UI holds no lighting logic.
- Fanned values are plain programmer values per fixture, so record, palettes and cues need
  nothing new.
- Fan follows the selection order of PRG; the extra orders of IDEA-15 (random, 1 in 3) stay in
  phase 2 and plug into the same rule later.
**Open questions (for P1-20):**
- Which curves in phase 1: Line only, or Line, Mirror, Wings, Arrow as sketched?
- Fan from the middle (Titan: symmetric around the centre) or from the first fixture?
- Does Fan stay on until switched off (latched) or only while the key is held?
- Fan on groups: does a group's own order count, and how do sub-selections (Next/Prev) fan?
- Fan from a fader or encoder on the BCF2000 (CTL action "fan") in phase 1?

**Specified 2026-10-25 (P1-20):** the four curves of UI v0.1 with exact factors (Mirror and Arrow
redefined, the sketch had them as one curve), Fan latched and off on a new selection, one list in
selection order over items, only wheels fan, a `fan` control action; PRG-56..62, UI-30, IPC-43,
CTL-59, build tasks VS2-35..36. Phase 2 remainder: Titan's Pull curves, Fan parts, the group
options, press-and-hold Fan, a keypad fan.

### IDEA-28 — A local AI assistant with a chat window (2026-10-25)
**Owner's intent:** next to the MCP server for a cloud assistant (IDEA-17), run a model locally.
Everything is JSON and the whole context (patch, profiles, groups, palettes) is already there,
so a relatively small model should manage. A chat window in the app takes requests like "make
the default colours for all fixtures", "build basic groups", "do this, do that".
**Triage:**
- Phase 2, together with IDEA-17: both talk to CMDMX through the same tool layer (the
  IPC commands, `ipc.md`), so the work is shared. The app holds no lighting logic, so the
  chat window only shows the conversation and a preview; the model and its tool calls live
  outside the UI.
- Show-proof as IDEA-17: what the assistant wants to change is shown first and applied as one
  undoable step (`undo.md`); never live output without the operator; the model runs in its own
  process so it can never slow the DMX tick.
- Feasibility study: `docs/research/local-llm-assistant.md` (2026-10-25). Verdict: feasible
  on a 16 GB Apple Silicon Mac (8 GB with care) with a ~4B model at 4-bit, when the model only
  chooses and fills parameters of built-in deterministic "wizards" (colour palettes from wheel
  slots and colour mixing, groups by type, odd/even); the lighting maths stays code. Aiming
  ("movers at the stage") needs fixture positions, so it waits for IDEA-25.
- Proposed shape: `llama-server` (llama.cpp, MIT) as its own process on loopback, output
  constrained to a JSON schema; candidate models Qwen3.5-4B, Gemma 4 E4B, Phi-4-mini, downloaded
  on first use, never in the app bundle; Apple Foundation Models (macOS 26) as a zero-download
  option. A separate `cmdmx-assistant` process, a client of the same IPC and tool catalogue as
  the IDEA-17 MCP server; the chat window shows the diff and an Apply button.
- Engine work it needs (useful without a model too): blind store commands (`palette.store`,
  `group.store`) — today `palette.record` records through the programmer, which is live, so
  building palettes would light the rig — and `plan.preview` / `plan.apply` as one undo step.
- Spike before any spec (1–2 days, no engine change): the church demo show on Art-Net only,
  three 4B models and Apple FM, 20 Dutch and English prompts for a schema-constrained plan of
  today's IPC commands; pass bar ≥ 18/20 correct, < 10 s, no tick overrun.
**What it touches now:** nothing forced; like IDEA-17 it needs every IPC command complete and
documented, as now.
**Open questions:**
- Which church laptop (chip, RAM, macOS)? Not recorded in the repo; it decides 8 vs. 16 GB.
- Wizards first as plain commands in the app (no model), with the assistant on top later?
- Locked while playbacks are live, or always allowed as a planning tool (nothing changes until
  Apply)?
**Owner's answer (2026-10-25):** phase 2, together with IDEA-17.

### IDEA-29 — Serial fade steps after VS4-05 (2026-10-25)
**Found in:** the church run (`tests/hardware/runs/2026-10-25-church.md`, section 2). At 33 Hz the fade on P37 was
"comparable" with Lightkey, but the owner saw "a fraction" more steps in ColourmotionDMX.
**Note:** that fade was Claude's IPC script (30 `programmer.set` per second, ~2 DMX per step), coarser than an
engine cue fade, which the engine computes every tick.
**Proposed:** phase 2 (owner to confirm). First look again with a cue fade on the Eurolite; only if steps remain,
a criterion in `output-protocols.md` for serial frames cut to the highest patched channel (shorter frames, a higher
rate). Touches now: nothing.
**Open questions:** does the Eurolite MK2 accept short label-6 frames; what rate does a 373-channel frame allow?

### IDEA-30 — Cross-platform UI for the Linux desk (2026-10-25, HW-01)
**Owner's intent:** the reference desk runs Linux (macOS needs a paid touch driver, and a desk
without touch is not acceptable); development stays on the Mac. "At some point we have to go to
the cross-platform UI."
**Triage:**
- Phase 10, after the surface (owner 2026-10-25; the hardware phase was 3 until 2026-11-01). The UI holds no lighting logic and talks to the
  daemon over WebSocket, so it is a rewrite of the screens, not of the console.
- The daemon needs Linux too: MIDI through ALSA instead of CoreMIDI, serial ports under
  `/dev/tty*` instead of `/dev/cu.*` (OUT-09/35), paths outside `~/Library`.
- `PROJECT.md` lists "Windows/Linux UI" out of scope for this milestone; that stays until this
  idea's phase is planned.
**What it touches now:** nothing; HW-02 keeps the Mac mini as a variant and HW-35 waits for it.
**Open questions:** which UI toolkit (web UI in a kiosk browser, Qt, Slint, Flutter, …); does
the Mac keep the SwiftUI app or move to the same UI?

### IDEA-31 — Stream Deck + and Neo extras, for a volunteer (2026-10-27, was VS7-08)
**Owner's intent:** the owner will not buy a Stream Deck + or Neo for this, so VS7-08 leaves the
backlog. It stays open for a volunteer who owns one.
**Triage:**
- Later phase, no backlog row (owner 2026-10-27). CTL-55 and the model table rows (CTL-47) stay
  in `control-surfaces.md` as approved; only the build is parked.
- A volunteer needs the deck itself: the gate-3 check runs on real hardware.
**What it touches now:** nothing; the Original keeps working (VS7-05..07, 09).
**Open questions:** none until someone with a + or Neo offers to build it.

### IDEA-32 — Automatic emitter mixing and a colour-temperature view (2026-10-28)
**Owner's intent:** with the target colour as the one colour value (ADR-005), a fixture with extra
emitters should use them by itself: a warm white on an RGBW par comes mostly from its White, an
amber on an RGB+Lime par uses the Lime. Next to that, the operator wants to pick a white by colour
temperature (Kelvin) and to warm or cool a colour the way a CTO or CTB gel does.
**Owner decisions (2026-10-28):**
1. Auto-mix is opt-in per fixture type (per mode), default off, so existing shows do not change.
2. The direct emitter wheels stay live and add on top of the auto-mixed level, so the operator
   keeps direct control.
3. The colour of an emitter comes from GDTF `cie` (LIB-32), otherwise a name-based default
   (White, Warm White, Cold White, Amber, Lime), editable in the profile editor.
4. Colour temperature as a target colour: a Kelvin view and CTO/CTB steps. LEE gel numbers stay
   in IDEA-02.
**Triage:**
- Pulls ADR-005's "automatic white/lime extraction" forward ahead of calibration (IDEA-03), on the
  nominal model: ADR-010 amends ADR-005 decisions 2 and 4. Stored colours do not change; it is
  output only, like the nominal mapping it extends.
- Criteria: LIB-36..39 (opt-in, emitter colour, output factor, profile editor), ENG-20..24
  (the mix in the frame writer), PRG-63..66 (Kelvin view, CTO/CTB steps), IPC-46, UI-31.
  Task row P1-22; build rows after approval.
- UV (and any emitter without a colour) is never auto-mixed; CMY fixtures and colour wheels have
  no auto-mix.
**What it touches now:** nothing until the spec is approved; then the frame writer (`set_mixing`
in `cmdmx-core::engine`), the profile model and editor, the programmer's colour models.
**Owner's answers (2026-10-28):** ADR-010's four questions as proposed: direct adds on top (clamped),
profile order for two whites, 1800–10000 K with the nominal mired values, Kelvin for every colour fixture.

### IDEA-33 — The FX engine is still sparse; much more should go in later (2026-10-29)
**Owner's intent:** (when picking IDEA-15) "I also found the FX engine rather sparse; more must go in
there too in the future." The FX generator of IDEA-18 (FX-01..44, v0.2.4) is a good start, not the end.
**Triage:** what Titan, grandMA3 and Onyx offer that CMDMX lacks (`docs/research/selection-orders.md`,
second part), in parts, each with a proposed phase. Every part builds on ADR-011 (entries on a
shape library), so none has to be redone first.
1. **More forms and looks** — waves (Saw, Pulse, half-sine, Flicker/Candle, Lightning, Strobe burst),
   paths (Ballyhoo, Spiral, Star, Random position, Fly in/out), colour (Two-colour bounce, Fire,
   Water); more looks in the gallery (FX-30). Each form is one row and one test (ADR-011). *Phase 2,
   right after IDEA-15.*
2. **Speed, Size and Rate masters on playbacks** — a fader that sets an effect's size or speed (Titan
   shape size/speed on a playback, MA3 speed/rate masters, Onyx FX masters), *Speed on fader*, size
   masters per group. *Phase 2, after part 1.*
3. **Cycles and one-shots** — an effect that stops after *n* cycles (Titan Cycles, part cycles
   allowed), a one-shot chase on Go, effects on chase steps. *Phase 2.*
4. **Layered effects per attribute** — several entries on one attribute adding up or overriding, with
   Titan's Overlay/LTP choice per playback; lifts ENG-15's one-effect rule (ADR-011 amendment).
   *Phase 2, later.*
5. **Step / key-frame FX** — user effects made of steps (values or palettes per step, fade or snap,
   per-step time and acceleration): Titan Key Frame Shapes, MA3 phasers, Onyx step FX. Also user
   shape files. *Phase 2, later; the biggest part.*
6. **Spread layers over the grid** — MA3's From/To for phase, size, speed and fade over the selection
   positions (PRG-73), Titan's Offset between shapes and *Match to Spread*; buddying is already
   blocks. *Phase 2, with or after part 4.*
7. **Pixel / matrix effects** — images, gradients and text over a 2D layout of fixtures and cells
   (Titan Pixel Mapper, MA3 bitmap fixtures, Onyx Matrix FX). Needs the rig overview (IDEA-25).
   *Phase 2 late, after IDEA-25, else phase 3.*
8. **Tempo from outside** — Ableton Link / MIDI clock (IDEA-16) and audio beat detection as BPM master
   sources. *With IDEA-16 (phase 2).*
**What it touches now:** nothing; IDEA-15 (P2-02) keeps FX-07 as the one position function, which
parts 6 and 7 build on.
**Open questions:** which part matters most for the church (the owner's playground, not its limit);
which ONYX pain from IDEA-18 each part must avoid.
**Outcome (2026-10-29):** owner confirmed the phases and the order as proposed (parts 1–6 phase 2 in
this order after IDEA-15, part 7 after IDEA-25, part 8 with IDEA-16). No criteria until each part is planned.

### IDEA-34 — Multi-user support in the software (2026-11-01)
**Owner's intent:** "Multi-user support in the software", for phase 3.
**Triage:**
- Phase 3 (owner 2026-11-01). Spec in a new module when the phase is planned.
- The daemon already takes several clients and sends each change to all of them (IPC-01), so the
  transport is there. The state is not: phase 1 has one programmer (`programmer.md`, out of scope)
  and one shared undo history (UNDO-15). Titan gives each user their own Undo History column
  (`undo.md`), and ADR-008 expects a per-user programmer part, which its design does not block.
- Loopback only today (IPC-01), so users on other machines need the LAN listener and password of
  IDEA-06 (phase 2).
**What it touches now:** nothing; no decision is forced before phase 3.
**Open questions:**
- What is a user: several operators working at once, each with an own programmer and selection, or
  also named profiles with own preferences and rights?
- Several devices on one daemon only, or also several consoles sharing a show (TitanNet, out of
  scope in `PROJECT.md`)?
- Which parts are per user (programmer, selection, undo, pages (IDEA-26)), and what happens when two
  users change the same fixture?

### IDEA-35 — Everyday gaps in the console (2026-11-01)
**Owner's intent:** surprised that a group could not be deleted (P1-24), the owner asked for a search
for more such gaps. "The church Mac has nothing to do with the app: we test there, but we just build
the app."
**Triage:**
- `docs/research/everyday-gaps.md` lists about 40 candidates in batches A to G, found by reading code,
  specs and the Titan manual. Ordered by what a general console needs, not by church dates.
- Phase 1, batch by batch; each batch gets its own spec with criteria, checked against Titan.
**What it touches now:** batch A is specified (P1-25, SPF-01..11); the other batches nothing until specified.
**Outcome (2026-11-01):** owner: all of it in phase 1, worked through in order, A to G.
**Owner's answer (2026-11-01):** when batch A is done, the version goes straight to 0.3.0, followed by a new gap review.
**Second review (2026-11-02):** batches A2 (defects), H, I and J in `docs/research/everyday-gaps.md`; owner: A2 and H first, then B to G, I and J; I1 and I3 go with B as advised.
**Third review (2026-11-04):** one operator's evening with the app running: batch K in `docs/research/everyday-gaps.md`, proposed into B to G, I and J; awaits the owner's approval of the order.
**A2 (2026-11-02):** spec P1-26 approved by the owner, every recommendation accepted; build tasks DEF-01..05.
**C (2026-11-05):** spec P1-33 approved by the owner, every recommendation accepted: L1 and J8 first, then L2, L3, L5, C1..C3 (IDEA-10) and K8; build tasks SEE-01..10.

### IDEA-36 — A refused number stays in its field (2026-11-02)
**Found in:** the SPF-07 run; not from the owner.
**What happens:** type fade in 602 in Playback Options: the engine refuses it and keeps 3, the warning shows, but the field goes on
showing 602 until the tab is reopened. `NumberField` shows what was sent (VS3-05 run: the old value looked like a jump back), and
the file's header says a refused value stays as it was. Every `NumberField` in Playback Options does this, also a cue's own times.
**Triage:** phase 1 (owner, 2026-11-02); `ui.md`; its place in the order is still open. Nothing recorded wrong: the show holds the old value.
**Third review (2026-11-04):** proposed in batch B, which changes times anyway.
**Open question:** how the field learns of the refusal, since warnings carry no field; and whether it snaps back at once or on leaving the field. P1-29, approved by the owner 2026-11-04: it need not learn, half a second after sending it shows what the engine holds (UI-52).

### IDEA-37 — The app takes over when another engine goes away (2026-11-02)
**Found in:** the DEF-03 run; not from the owner.
**What happens:** an app that found another engine on port 7319 (REL-11) connects to it and owns nothing. When that engine goes, the app says "Engine offline" and stays
offline until it is restarted; it does not start its own bundled engine. Only reachable with a second engine by hand, so not in an ordinary show.
**Triage:** proposed phase 1, after A2; `release.md`. Owner to confirm the phase. Nothing touched now.
**Third review (2026-11-04):** proposed phase 2 instead of 1: only reachable with a second engine by hand. Owner to confirm.
**Open question:** take over at once or ask, and with the same show?

### IDEA-38 — Workspaces that are not on show leave the accessibility tree (2026-11-03)
**Found in:** the FUN-01 run; not from the owner.
**What happens:** the workspaces stay built while hidden, and their buttons stay in the accessibility tree at their own positions. An accessibility click on a cue row in Playbacks landed on the Looks tile "Dimmer wave" of the hidden Shapes workspace and applied the effect; a real mouse does not. It reaches VoiceOver and anything that drives the app by accessibility, such as background runs.
**Triage:** proposed phase 1, with the gap review of 0.4.0; `ui.md`. Owner to confirm the phase. Nothing touched now.
**Third review (2026-11-04):** proposed batch F; not met with a real mouse. Owner to confirm.
**Open question:** hide them from accessibility, or stop keeping hidden workspaces built?

### IDEA-39 — Strips are cut off on a window with the Dock (2026-11-03)
**Found in:** the FUN-01 run; not from the owner.
**What happens:** on the owner's MacBook with the Dock showing the window is 832 pt high. The workspace layout then cuts off the strips below the fader readout, so the handle buttons are out of reach; empty bands stay above and below the window grid. On the screens that replace the grid (Patch, Playback Options) the upper button shows, the lower is cut off.
**Triage:** proposed phase 1, with the gap review of 0.4.0; `ui.md`. Owner to confirm the phase. Nothing touched now. Console mode may not have this; not tried in the run.
**Third review (2026-11-04):** seen on the first screen, cleared by full screen; proposed batch F. Owner to confirm.
**Open question:** the minimum height, and whether the bands can give their room to the strips.
**Outcome (2026-11-07):** seen in console mode on every workspace in the SEE-08 run; the owner asked for the fix now
(phase 1). Cause: the rail's items (at least 44 pt each) set the height of the middle row, so the stack grew past the
screen and the playback area fell off. It grew with every rail item; the Remote screen (RMT-15, #259, 2026-11-06)
took it past a MacBook's full screen, a window with the Dock was past it before. UI-63: the minimum is UI-01's
1280 × 800 pt of content (832 pt with the title bar); the middle row takes what is left, the rail scrolls and the side
column keys shrink to 34 pt. FIT-01 builds it.

### IDEA-40 — A function's hint stays after it is disarmed (2026-11-03)
**Found in:** the FUN-03 run; not from the owner.
**What happens:** the hint said on arming ("Unfold: tap a fixture to unfold or fold its cells.") stays in the message zone after Esc, Exit or a screen disarms the function, until the next message. Patch and Control show it too, with nothing armed. A delete's message does the same since UI-47, with its count and "Confirm on softkey A" (FUN-05 + FUN-06 run).
**Triage:** proposed phase 1, with the gap review of 0.4.0; `ui.md`. Owner to confirm the phase. Nothing touched now.
**Third review (2026-11-04):** seen again after Esc and through an engine restart. Owner: in a patch before B, so phase 1 (P1-30).
**Open question:** clear only hints (`say`) on a disarm, so a warning from the engine stays?

### IDEA-41 — Our own built-in 3D visualiser (2026-11-04)
**Owner's intent:** phase 4 is our own 3D visualiser, built into CMDMX, not an outside program such as Capture.
**Triage:**
- Phase 4, confirmed by the owner. Module spec to write when the phase is planned (`visualiser.md`).
- It draws the rig in 3D from the live output, so a show can be programmed without lamps. The engine holds
  no 3D: the app draws what the engine sends, on the same feed as IDEA-10 and IDEA-25.
- Builds on IDEA-25 (2D rig overview, phase 2): the same fixture positions, selecting in the view.
**What it touches now:** nothing forced. When IDEA-25 is specced, the position on a patched fixture should
leave room for height and orientation, so the 3D view does not need a second position model.
**Open questions:**
- What it must show: beams with zoom and gobo, or light on surfaces as well?
- Where the stage comes from: simple shapes drawn in the app, or an imported model?
- Where a fixture's size and beam angle come from: the profile (GDTF has them), or set per fixture?
- A window in the app, on a user page (IDEA-26), or also a second screen?
- On the Linux desk (IDEA-30), or macOS only? This weighs on the choice of rendering.

### IDEA-42 — A phone page for walking the patch (2026-11-06)
**Owner's intent:** alone on a stage with the console somewhere at the back and no printed patch. A
mobile web page that the console serves on the network as soon as the show is up: click through the
fixtures, patch them and go on to the next. Ideally a highlight, or something basic to see whether
the address was entered right. Behind a username and password set in the console. Wanted in phase 1:
at Christmas the church runs on this console.
**Titan reference:** the Titan Remote app (phone/tablet over Wi-Fi) shows the DMX address patch but does
not edit it; Titan's Web API is HTTP on the console. Patch editing from a phone goes past Titan.
**Triage:**
- Phase 1 (owner 2026-11-06). A first slice of IDEA-06 (web remote, phase 2): the LAN listener, the
  login and one page. IDEA-06 later adds faders and cues to the same listener.
- The daemon serves it (the only crate with I/O): one page built into the binary, over the WebSocket
  it already speaks (`tungstenite`), no web framework. The app's own port stays as it is: loopback only
  and `Origin` refused (IPC-01, IPC-02). The page gets its own port on the LAN, off until a username
  and password are set in the console.
- The page: fixtures in number order (number, name, type, mode, universe.address, footprint), jump by
  number; a fixture's card edits universe and address through the existing patch commands (PAT-04,
  clashes refused as today); Previous / Next walks on. Highlight follows the card: the shown fixture
  is highlighted (PRG Highlight, `programmer.highlight`), Next moves it on, so a wrong lamp lighting up
  shows a wrong address.
**What it touches now:** IPC-01/02 stay; a new module spec (`remote.md`, RMT) that IDEA-06 extends later.
**Open questions:**
- Only readdress existing fixtures, or also name, renumber, add and remove from the phone?
- Highlight through the console's own selection and programmer (simple, as a remote does), or apart
  from it, so someone programming at the console is not disturbed?
- Plain HTTP on the LAN (the password crosses the Wi-Fi readable) or HTTPS with a self-signed
  certificate (the phone warns once)? Where the login lives: the console's settings, not the show.
- Does the page also show the DMX values going out on the fixture's channels, as the address check?
**Owner's answers (2026-11-06):** universe, address and name of existing fixtures (adding and removing
stay on the console); highlight through the console's own selection and programmer.

### IDEA-43 — Park fixtures (2026-11-06)
**Owner's intent:** the church rig is bigger on paper than what really hangs, which makes programming
hard. Parking a fixture should take it out of groups and the selection as well. How does Titan do it?
**Titan reference** (*Changing the Patch*): Park (Patch › Repatch Fixtures › Park) takes the fixture off
the DMX output map; all its programming is kept and its old line and address are remembered, so
Unpark puts it back. Patch View can show parked fixtures greyed out where they were patched. A repatch
onto a used address offers *Park Conflicting Fixtures* (and a user setting to always do so). Titan's
Park is not the Park of other consoles (holding an output value): that is Titan's Freeze. The manual
says nothing about groups or the selection.
**Triage:**
- Proposed phase 1: programming the church for Christmas needs it, and the church notes already list
  what is parked in Lightkey (`tests/hardware/church.md`).
- Proposed behaviour, Titan's park plus the owner's wish: a parked fixture outputs nothing, cannot be
  selected, and is skipped wherever fixtures are taken live (a group's selection, All, selection order,
  Fan, effect spread, recording). The show still holds it in its groups, palettes and cues, so Unpark
  gives everything back. Parked fixtures show greyed in the patch and are left out of the fixture window.
- With park, a range clash in the patch (patch.md, P1-15: "CMDMX has no parking, so every clash
  refuses") can offer Park Conflicting, as Titan does.
**What it touches now:** nothing forced; PAT-05 (remove) stays as it is.
**Open questions:**
- The owner said "out of the group": skipped while parked but kept for Unpark (proposed), or really
  removed from the groups?
- Park only from the console, or also on the phone page (IDEA-42) while walking the rig?
- Does Unpark go back to the old address, or refuse when that address is taken by now?
**Owner's answers (2026-11-06):** phase 1, spec P1-35 beside P1-34; skipped while parked and kept for Unpark.

### IDEA-44 — The live stripes step during a fade (2026-11-07)
**Found in:** the SEE-07 + SEE-08 run; the owner: "the playback in the fixtures reacts a bit jerkily".
**What happens:** the stripe on a fixture tile follows the engine at most 10 times a second (IPC-58's rate cap), and
the app adds nothing in between (FX-41), so a fade or a fader move shows in steps of 0.1 s. Art-Net itself is smooth.
**Triage:** proposed phase 1, `ipc.md`; owner to confirm. Proposed: raise the cap to 25 a second (the live feed and
`output.watch` alike), profiled with the Demo show first, since every look runs `Engine::explain` on the daemon loop.
**What it touches now:** nothing forced; the cap is one constant (`live::EVERY`).
**Open question:** 25 a second enough, or should the app ease between two values after all (FX-41 says no)?
**Owner's answer (2026-11-07):** phase 1, the cap at 25 a second (SEE-11, in the SEE-07 + SEE-08 build).

### IDEA-45 — A tracked value names the cue it came from (2026-11-07)
**Found in:** the SEE-09 + SEE-10 run.
**What happens:** after GO from Q1 to Q2, the pars that track their Q1 values show "3.1 · Q2" as source in the
Intensity view and the Grid's *Sources*, with the magenta `=` for tracked. The label names the playback's current cue
(IPC-58, ENG-25's base), so the operator cannot see that Q1 set the value.
**Triage:** proposed phase 1, `ipc.md` (IPC-58, IPC-59) and `engine.md` (ENG-25); owner to confirm. Proposed: a
tracked value's label names the cue that stored it ("3.1 · Q1"), as Titan's tracking display points back to the
source cue; `why` (IPC-60) already carries a `cue`.
**What it touches now:** nothing forced; the label is made by the daemon outside the tick.
**Open question:** the cue that stored it, or both ("3.1 · Q1 → Q2")?

### IDEA-46 — Unfold a cue list onto the faders (2026-11-07)
**Found in:** IDEA-35 batch D (D4), from Titan's *Editing Cue Lists* and *Editing a Chase*.
**Intent:** Unfold, then a list's handle, puts each cue on its own fader, page by page; raising a fader runs that cue
with its times, Record Step changes it, Unfold again leaves. An editing view, not copies.
**Triage:** phase 1, later, with a spec of its own (owner decision 3, P1-36, 2026-11-07): it is as large as the rest of
batch D, and the Cue list window already edits every cue.
**What it touches now:** nothing; the app's Unfold folds a fixture's cells only (UI-29).
**Open questions:** what the faders' own playbacks do meanwhile; whether it runs on the surfaces' faders.

### IDEA-47 — A linked playback copy (2026-11-07)
**Found in:** IDEA-35 batch D (D4), from Titan's *Copying, Moving, Linking and Deleting a Cue List*.
**Intent:** Copy twice (or Link) makes a playback on another handle that shows the same cues, with its own times and
playback options; a change to a cue shows on both.
**Triage:** later (owner decision 4, P1-36, 2026-11-07); the phase is to be confirmed. Cues shared between playbacks
change the show file and every cue edit.
**What it touches now:** nothing; `copy_playback` clones the cues.
**Open question:** which phase.

### IDEA-48 — A BCF2000 behind a MIDI interface (2026-11-08)
**Found in:** the VS1-07 run, where the BCF2000 had to run over DIN MIDI because its USB output does not work on the
owner's M4 Mac.
**Intent:** the BCF works as smoothly behind a MIDI interface as it would over USB.
- A power-cycled BCF keeps the wrong motor and LED state until it is unticked and ticked in Control. It sends the
  Mackie sysex `F0 00 00 66 14 01 …` at power-on, which could trigger the full feedback of CTL-21.
- The profile matches by device name, so a surface behind `H4MIDI-WC Port 1` needs the mapping chosen by hand.
- The BCF2000 has no Channel buttons, so with more than 8 faders per page slots 9 and up cannot be reached from it.
**Triage:** later, phase to confirm; the owner's desk works without it (Control re-sends everything at once).
**What it touches now:** nothing; `control-surfaces.md` CTL-16, 21, 30 and 32 when it is planned.
**Open questions:** which phase; which button could stand in for Channel on a BCF2000.

### IDEA-49 — Titan's Exchange Mapping screen (2026-11-08)
**Found in:** IDEA-35 batch E (M2), from Titan's *Changing the Patch*.
**Intent:** when a fixture type is exchanged, the operator sees how the old attributes map to the new ones and can change it.
**Triage:** later, phase to confirm (owner decision 3, P1-39, 2026-11-08). The screen is as large as the exchange itself, and names
are already normalised on import, so the automatic match of PAT-38 comes first.
**What it touches now:** nothing; PAT-38 matches by name, kind and slot name and reports what drops.
**Open question:** which phase.

### IDEA-50 — Freeze an attribute of a fixture (2026-11-08)
**Found in:** IDEA-35 batch E (E1), from Titan's *Fixture Personality Options*.
**Intent:** an attribute of a fixture stays at its value at the output while the Freeze option is on.
**Triage:** phase 1, later (owner decision 7, P1-39, 2026-11-08). It needs a per-attribute record of the last output, which is not there.
**What it touches now:** nothing; PAT-44..45 hold the other options.
**Open question:** what Titan does when Freeze is switched off during a fade.

### IDEA-51 — Output curves per attribute (2026-11-08)
**Found in:** IDEA-35 batch E (E1), from Titan's *Fixture Personality Options* and *Curves*.
**Intent:** a curve between the programmed value and the output, chosen per fixture and attribute.
**Triage:** phase 1, later (owner decision 7, P1-39, 2026-11-08). Titan's manual describes only Linear, so the shapes need defining first.
**What it touches now:** nothing; PAT-45 is where the output write would apply it.
**Open question:** which shapes, and from where.

### IDEA-52 — Flip a moving head (2026-11-08)
**Found in:** IDEA-35 batch E (E1), from Titan's *Fixture Personality Options*.
**Intent:** Flip moves a selected head to its other yoke position, with the same beam direction.
**Triage:** phase 1, later (owner decision 7, P1-39, 2026-11-08). Titan defines its settings in the personality; our profiles do not hold them.
**What it touches now:** nothing; `fixture-library.md` when it is planned.
**Open question:** where the flip data comes from for QLC+ and GDTF profiles.

### IDEA-53 — Lamp on/off and fixture macros (2026-11-08)
**Found in:** IDEA-35 batch E (E4), from Titan's *Fixture Personalities*.
**Intent:** Lamp On, Lamp Off and Reset run through a fixture's control channel with its timing, one press for a selection.
**Triage:** phase 1, later (owner decision 7, P1-39, 2026-11-08). Profiles hold no macro data. A control channel's named slots can
already be set by hand (LIB-15).
**What it touches now:** nothing; LIB-15 is the nearest thing.
**Open question:** where macro data comes from for QLC+ and GDTF profiles.

### IDEA-54 — RDM patching (2026-11-08)
**Found in:** IDEA-35 batch E (E4), from Titan's manual.
**Intent:** the console discovers RDM fixtures on an Art-Net node and patches or addresses them from there.
**Triage:** phase 1, later (owner decision 7, P1-39, 2026-11-08). It needs a discovery path in the protocols crate and an
RDM-capable node to test with. Titan needs an RDM-enabled Art-Net node as well; XLR does not carry it.
**What it touches now:** nothing; `cmdmx-protocols` has output only.
**Open question:** which RDM-capable node to test with.
