# ADR-004: GDTF as the fixture-profile format, with a thin overlay

**Status:** accepted
**Date:** 2026-10-16

## Context
A console is only as useful as its fixture library. Avolites maintains its own `.d4` format with a staffed library service; a small open-source project cannot. GDTF (General Device Type Format) is the open industry standard backed by MA Lighting, Robe and Vectorworks, with a public library (GDTF-share) and import support in Titan's Personality Builder since v17. The Open Fixture Library (OFL) is open source and exports GDTF.

## Decision
- Fixture profiles are **GDTF** archives (zip containing `description.xml` plus resources).
- Titan-specific behaviour that GDTF does not express is stored in a small **overlay JSON** next to the profile, keyed by GDTF fixture type id and mode: attribute → IPCGBES-FX group mapping where the GDTF attribute name does not map automatically, locate values, highlight/lowlight values, virtual dimmer flag, flip definition, cell layout hints.
- A default group mapping table (GDTF attribute names → I/P/C/G/B/E/S) lives in `cmdmx-core::library` so most fixtures need no overlay.
- Import from OFL/QLC+ `.qxf` is a later convenience, implemented as conversion to GDTF, not as a second native format.
- A **hand-written minimal profile format** (JSON) exists only for tests and vertical slice 1, is documented as such, and is removed from user-facing paths once GDTF import works.

## Alternatives
- Own JSON format: full control, empty library.
- Avolites `.d4`: proprietary XML, no licence to redistribute the library.
- QLC+ `.qxf`: open and large, but less expressive (no cells/geometry) and not an industry standard.

## Consequences
- Need a GDTF parser in Rust: zip + XML (`quick-xml`), DMX modes, channel functions with ranges and physical units, geometry for cells. Scope it as its own module spec.
- Physical-unit values (degrees, percent, colour) make Global palettes and cross-fixture colour mixing straightforward later.
- Overlay files are versioned alongside the show file.
