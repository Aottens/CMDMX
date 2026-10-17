# ADR-006: Own profile model, filled by importers (QLC+ first)

**Status:** accepted (owner, 2026-10-17)
**Date:** 2026-10-17
**Supersedes in part:** ADR-004 (GDTF is no longer the internal format; the overlay becomes fields of the own model)

## Context
ADR-004 made GDTF the internal fixture format, with OFL and QLC+ as later conversions to GDTF. A
check of the church rig against the open libraries (2026-10-17) changed the picture:
- Budget brands rarely publish GDTF; GDTF-share was not searched fixture by fixture, but the
  church's brands (Shehds, Showtec budget pars, Pro-Lights, U'King) are not GDTF publishers.
- The **QLC+ fixture library** (Apache-2.0, 1781 profiles) has the Shehds 19x15 RGBW zoom wash,
  Showtec LED Par 56, several Pro-Lights RGBW pars and Showtec compact RGB pars. The **Open Fixture
  Library** (MIT, 658 profiles) has none of the Shehds or Pro-Lights fixtures.
- ADR-004's premise that OFL exports GDTF is wrong: OFL imports GDTF but does not export it.
- The engine already works on a small own model (`cmdmx-core::profile`), described in ADR-004
  as "what GDTF import will produce". ADR-005 adds a colour mixing system and direct emitters to it.

The owner's view: do not reinvent the library.

## Decision
1. **The engine's profile model is the one internal format.** A profile is a fixture type
   (manufacturer, model) with one or more modes; a mode lists its attributes with IPCGBES group,
   HTP/LTP, channel offset(s), optional fine channel, **default value**, and optional locate and
   highlight values, plus the ADR-005 colour mixing system and emitters and a virtual-dimmer
   flag. Profiles are stored as JSON in this model. The fields ADR-004 put in an overlay live in
   the profile itself; there is no overlay file.
2. **Importers convert other formats into this model, once**, with an import report that lists
   how every source channel was mapped and what was not understood. The result is an ordinary
   profile the operator can correct. Import order:
   1. **QLC+ `.qxf`** (phase 1): covers almost the whole church rig.
   2. **Profile from a channel list** (phase 1): typed in for fixtures no library has (the church's
      U'King RGB+Lime pars) or where a library profile is wrong.
   3. **GDTF** (phase 1, after QLC+) via the `gdtf` crate (MIT): for brand-name fixtures and the
      per-emitter colour data calibration needs (IDEA-03). The church is the playground, not the
      limit: a Titan-class console must take manufacturer profiles.
   4. **OFL JSON** (later): QLC+ already covers most of OFL's budget fixtures.
   Importing the owner's own Lightkey custom profiles (`.lightkeyfxt`) is checked once a file is
   available; Lightkey's own library is theirs and is not imported.
3. **The QLC+ library ships with the app**, fetched at build time from a pinned commit of the
   QLC+ repository by a script and copied into the app bundle (works offline in the church), with
   its Apache-2.0 licence and attribution. Profiles are converted when the operator picks one,
   not up front.
4. **Parsing is pure.** XML and JSON parsing of profile text lives in `cmdmx-core::library`
   (strings in, profile and report out); reading files is the daemon's job. New dependency:
   `quick-xml` in `cmdmx-core`.
5. **A show embeds the profiles it uses** (SHW-01 `profiles`), so a show file opens on any machine
   without the library it was built from.

## Alternatives
- **GDTF as the internal format (ADR-004):** the format is rich, but the church's fixtures are not
  available in it and the planned conversions do not exist.
- **QLC+ `.qxf` as the internal format:** large library, but it couples the engine to another
  program's schema and has no notion of a target colour, emitters as separate attributes or
  IPCGBES groups.
- **Our own curated library:** impossible at our size.

## Consequences
- `fixture-library.md` (P1-01) specifies the model, the QLC+ mapping (QLC+ channel presets and
  groups → attributes, heads → whole-fixture attributes in phase 1), the channel-list profile and
  the library browser; `patch.md` (P1-11) builds on profile + mode.
- Default values become engine behaviour: an attribute that no source has ever proposed outputs
  its default (a fixture's "Mode" channel stays in DMX control, not in a built-in program).
- ADR-004's status changes to "superseded in part by ADR-006"; its GDTF reasoning stays valid for
  the later GDTF importer.
- Licence: the bundled QLC+ profiles are Apache-2.0, compatible with either licence still open
  for this project (MIT or GPL-3.0, BACKLOG).

## Reversal trigger
QLC+ profiles prove unreliable for the church fixtures in practice (then the channel-list profile
carries the church and the QLC+ importer drops in priority), or GDTF-share gains the budget brands.
