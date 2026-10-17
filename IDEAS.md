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

### Decision these ideas force now
IDEA-02 and IDEA-03 only work if palettes and cues store a colour as a **target colour** (device-independent) and not as raw DMX channel values. The palettes spec (P1-03) and the show file fix that format, so this decision must come **before** P1-03, otherwise phase 2 means rewriting palettes and show files. → `BACKLOG.md` P1-07.
