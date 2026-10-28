# ColourmotionDMX — Decision Log

Append-only. Newest at top. Each entry: date, decision, alternatives, why, reversal trigger.
Long-form records live in `specs/ADRs/`; this log is the index.

---

## 2026-10-28 — The website ships before the logo and the domain: a placeholder mark and github.io first (owner, VS8-03)

**Decision:** VS8-03 no longer waits for VS8-02 or the domain name. The site uses a placeholder lockup and favicon, clearly marked, in `subprojects/website/brand/` under the BRD-21/23 file names; VS8-02 replaces those files and no page changes. Until the owner names the domain the site is served at `https://aottens.github.io/CMDMX/`; the domain is then one line in `subprojects/website/CNAME` plus the Pages and DNS settings (WEB-20, `tests/hardware/website.md`).

**Alternatives considered:** keep VS8-03 blocked on the logo (the owner has not found it yet) and on the domain.

**Reasoning:** the real Colourmotion logo comes later (owner, 2026-10-28); the build, the release hook and the download block do not depend on it, and all pages use relative links, so the move to an own domain changes no page.

**Reversal trigger:** the placeholder is still there when the site is announced (VS8-08), or the own domain needs more than the CNAME line.

## 2026-10-27 — CMDMX gets its own website and domain; it goes live as a preview before notarisation (owner, P1-21)

**Decision:** the rename (`brand.md`) keeps the bundle id and moves the data folders once; the GitHub repo becomes `Aottens/CMDMX`, the local folder keeps its name. The website (`website.md`) is static on GitHub Pages under an own domain, rebuilt after every release so it always offers the latest DMG, with the manual of that release (mdBook). It may go live as a preview with the DMG not notarised; notarisation (VS8-08) comes before the site is announced. No price on the site until IDEA-21. English only for now.

**Alternatives considered:** github.io address first; notarise before the site goes live; a JavaScript widget that asks GitHub for the latest release at view time; a site framework (Astro, Jekyll).

**Reasoning:** baking the release in at build time needs no JavaScript and no API limits; mdBook is one binary with search and a print page; keeping the bundle id keeps the macOS permissions on the church Mac.

**Reversal trigger:** the site is announced before VS8-08 is done, or the manual outgrows mdBook (versions side by side, translations).

## 2026-10-27 — The installed app starts and stops its own engine; Quit stops output (owner, VS4-07)

**Decision:** the DMG's app carries the daemon and starts it at launch (REL-10); ⌘Q stops it after it has written the show (REL-13, REL-14). If the app crashes the daemon keeps output and the next launch adopts it (REL-11). Builds are universal; the version comes only from the `v*` tag (REL-01). First tag `v0.1.0`.

**Alternatives considered:** a daemon that keeps running after Quit with a separate *Stop engine* item; a launchd agent; Apple Silicon-only builds.

**Reasoning:** the Lightkey fallback needs the Eurolite free, and nobody at the church should need Terminal; a universal build makes the church Mac's chip irrelevant.

**Reversal trigger:** an operator needs output to survive a deliberate Quit (e.g. a second UI on another Mac): then add a keep-running choice.

## 2026-10-27 — The church Mac runs ColourmotionDMX from a versioned DMG, not notarised yet (owner, VS4-01)

**Decision:** on Sunday ColourmotionDMX runs on the church Mac beside Lightkey (fallback: stop the daemon, start Lightkey). It gets there as a DMG with a version number, built by GitHub (VS4-07). The DMG is not notarised for now: the first open on the church Mac is right-click › Open once.

**Alternatives considered:** the owner's Mac with the Eurolite moved over USB on fallback; building from source on the church Mac; notarising now (Apple Developer account).

**Reasoning:** the fallback is quickest when both programs sit on one Mac; the owner wants plain DMGs with versions, not a build tree on the church Mac; one right-click is acceptable until there are other users.

**Reversal trigger:** the DMG goes to anyone besides the owner (website, IDEA-20) or Gatekeeper stops allowing the right-click open: then notarise.

## 2026-10-27 — GDTF import: file in, user profile out; `gdtf` crate in core, bytes in (owner, P1-12)

**Decision:** a `.gdtf` file is imported once by Import GDTF… and saved as a user profile (LIB-34); importing a model that already is a user profile replaces it. `cmdmx-core::library::gdtf` parses with the `gdtf` crate (MIT) and takes the archive's bytes, read as a zip in memory, which amends ADR-006 decision 4 ("strings in") for GDTF only; core gains `gdtf`, `zip`, `uuid` and a second `quick-xml` version. Emitter chromaticity (`cie`) is stored now for IDEA-03. Tests use GDTF files written for this project plus the crate's MIT test file; a manufacturer file is committed only if its licence allows.

**Alternatives considered:** a GDTF folder indexed like the QLC+ set and converted on pick (a third library source); refusing an import over an existing user profile; our own GDTF parser on `quick-xml`.

**Reasoning:** ADR-006 converts every format once into the own model; the crate follows the GDTF 1.2 spec closely, which is less risk than a parser of our own; emitter colours are why GDTF is in phase 1.

**Reversal trigger:** the crate fails on real manufacturer files or stops being maintained; then parse `description.xml` ourselves as the QLC+ importer does.

## 2026-10-26 — Protocol v1: items on the wire, the show keeps strings (VS2-33)

**Decision:** on the IPC wire a whole-fixture item stays a JSON integer and a cell is a string `"901.2"` (IPC-42), so every whole-fixture message reads as before; the show file keeps writing every item as a string (schema 7). The core gets a serde helper for the wire form and `Item`'s own `Serialize` stays the show's. IPC-42's "sub-selection" is the existing `targets` key, not a new key; `contents` entries, `cue.view` fixtures and `located` keys carry items too. `v` goes to 1 for every message; the app's `protocolVersion` goes to 1 in the same PR.

**Alternatives considered:** strings everywhere on the wire (a needless change to every whole-fixture message); a `cell` field next to each `fixture` (two keys for one item, and lists of objects instead of ids).

**Reasoning:** IPC-42 asks for integer-or-string; keeping the two encodings apart lets the show schema and the protocol move independently.

**Reversal trigger:** a remote client that cannot cope with mixed-type arrays.

## 2026-10-25 — Box firmware details: palette in blocks of 8, end-snap, settled position per connection (HWB-01)

**Decision:** the HW-20 feedback palette has 16 colours, one per block of 8 velocities (0–7 off, 120–127 white; `wire-map.md`). A fader value within the deadband of an end stop snaps to 0 or 16383, so full travel is reachable before HW-15 calibration exists. HW-18 is read as: no value until the fader has settled, then its settled position once, again on every new USB connection, so a new host learns where the faders are. Prototypes use the pid.codes test PID `0x1209:0x0001`. The firmware workspace gets its own CI job (host tests and an RP2350 build).

**Alternatives considered:** a hue wheel over 1–126 (no named colours to set); sending nothing until the fader moves (the host would not know the position after a reconnect).

**Reasoning:** named colours at fixed velocities are easy to set as `on_value`; the settled position per connection matches what a motor strip does at boot.

**Reversal trigger:** the `CMDMX Desk` profile (HW-21) needs more colours or a brightness per key.

---

## 2026-10-25 — The reference desk: Tiger Touch II size, one screen, Linux compute (owner, HW-01)

**Decision:** the open-hardware reference desk is a Tiger Touch II-size console with one 15.6" PCAP touch screen, 10 motorised playback faders (two keys each), 10 plain preset faders (one key), a plain grand master, four optical wheels and a keypad, four isolated DMX ports on an own ENTTEC-Pro-compatible board and the rest on Art-Net/sACN. Its compute is Linux on an N100-class mini-PC; development and proofs of concept run on the Mac, and the cross-platform UI is IDEA-30 (phase 3, after the surface). The surface is smart two-fader strips (own RP2350, RS485) behind one USB-MIDI main controller; firmware in Rust. Hardware licence CERN-OHL-S v2, build guide CC BY-SA 4.0. Quality before price. All in `specs/modules/hardware.md`.

**Alternatives considered:** Quartz size (compact, 12" screen) or Sapphire Touch (two screens, 45 faders); Mac mini as the reference with a paid touch driver (UPDD), or without touch; plain faders everywhere; AS5600 magnetic wheels; CERN-OHL-W.

**Reasoning:** macOS has no native touch support and a desk without touch is not acceptable; Linux touch is native. The DMX board and USB-MIDI surface reuse OUT-31 and CTL, so the engine does not change. Professional parts (motor faders, optical encoders, isolated DMX) because the owner wants pro quality.

**Reversal trigger:** the cross-platform UI (IDEA-30) turns out not to be feasible, or a touch driver for macOS becomes free and native.

## 2026-10-25 — Cell values stored by item, show schema 7 (VS2-32)

**Decision:** an item is written as a string, `"901"` for a whole fixture and `"901.2"` for a cell, both as a map key (cue values, palette per-fixture values, programmer contents) and in a list (groups). A whole fixture's key is byte-identical to the old fixture-id key, so cue and palette keys need no migration; groups become `{id, items}` (schema 7, migration 6 → 7 turns `fixtures: [901]` into `items: ["901"]`). A palette's shared value for a type's cell lives in a separate optional `shared_cells` (`type → cell → attribute → value`), because profile keys may contain dots (`"Acme/Wash 2.0"`).
**Alternatives:** keys like `"<type>.2"` inside `shared`; numbers for whole fixtures and strings for cells in group lists; cells in side maps next to the fixture-keyed ones (the VS2-30/31 stopgap).
**Why:** one key type everywhere removes the twin maps; strings keep JSON map keys and list entries the same shape; a separate field cannot be misread.
**Reversal trigger:** none expected.

## 2026-10-25 — Phase 3 (hardware) runs as a parallel track beside phase 1 (owner)

**Decision:** the open-hardware desk (IDEA-23, phase 3) starts now as a parallel track instead of after phase 1. It lives in `subprojects/hardware/` with its own `STATE.md`, spec `specs/modules/hardware.md` (`HW-nn` criteria) and the `## Phase 3 track — hardware` section of `BACKLOG.md`. Its only contract with the software is the general control layer (CTL): the surface is a USB-MIDI (later possibly HID) device mapped like any other controller. Hardware tasks run in their own sessions and PRs, never touch `subprojects/engine` or `subprojects/app`, and never move a phase-1 task. A hardware need that would change the software becomes an IDEA or a backlog row for the software, not a change in a hardware PR. First task HW-01: the owner answers the open questions of `docs/research/hardware-desk.md` and approves `hardware.md`.

**Alternatives considered:** wait until phase 1 is done; build the surface inside the engine tracks.

**Reasoning:** the CTL layer already decouples the surface, so firmware can be tested against today's app through Learn, and long lead items (PCB rounds, parts, case drawings) no longer wait for the software.

**Reversal trigger:** hardware work delays a phase-1 task, or the surface needs an engine change before phase 1 is done.

## 2026-10-25 — Cells in the merge: the virtual cell Dimmer and per-item Off/Locate (VS2-31)

**Decision:** the virtual cell Dimmer is full until a cell value proposes it; a fixture's Dimmer is the master and does not count as a value for it, and the Grand Master and group masters act on the master only. In the programmer a cell's entry (value, else located) beats the fixture's; Off and Locate act on the items they target only. Cell values stay in the programmer until VS2-32 stores them (PRG-47, 51 refined).

**Alternatives considered:** the fixture Dimmer as the cell Dimmer's fallback (squares the level under a master); HTP-nobody-is-0 for the cell Dimmer (every existing show would go dark on cell fixtures); Off on a fixture also clearing its cells.

**Reasoning:** output of every show without cell values stays byte-identical, a cell is never mastered twice, and each item behaves through its own attributes as PRG-47 says.

**Reversal trigger:** the owner expects Off or Locate on a whole fixture to reset its cells as well.

## 2026-10-25 — Busy check with a second daemon stands; BCF2000 run postponed (owner)

**Decision:** the Stream Deck `Busy` line (CTL-44) counts as passed with a second daemon holding the deck; the Elgato app is not installed, so its own line is waived and VS7-07 is done. The BCF2000 run (VS1-07) waits until the owner finds the controller; it stays `gate3`. Order of work: VS2-35..36 (Fan), then VS2-31..34 (cells); the church service (VS4-03) the Sunday after.

**Alternatives considered:** install the Elgato app only for this check.

**Reasoning:** another program holding the HID device is the case the line tests; which program does not matter to the daemon.

**Reversal trigger:** a user reports the Elgato app is not detected as Busy.

## 2026-10-25 — Deck keys: level on top, a lock on fixed keys, levels only shown (VS7-07 run, owner)

**Decision:** a key image has one bar, on top: the level fills it in the halo colour (white without a halo), the rest is the halo at 25 %; nothing along the bottom. A key whose action does not follow the console page (`<page>.<slot>`, `playback <id>`) shows a lock, on the deck and on screen. A continuous action (`gm`, `fader`, `playback <id> level`) on a key only shows its level; a press sends nothing. The app sets a deck's page (`control.deck {page}`), and `control.binding`/`unbind` pick a key's binding by deck page (CTL-46, 47, 50, 57, 58, 67 amended).

**Alternatives considered:** level bar on top and halo bar at the bottom; GM key as flash-to-full or not allowed; a lock only in the app.

**Reasoning:** the owner's deck stands tilted, so the bottom edge of the keys is hard to see; a key sends 1 then 0, so a GM key pulled the grand master to 0 in the run, a blackout from one press.

**Reversal trigger:** a model whose keys read better with the bar at the bottom, or a wish to drive a level from a key (then as its own action, not `gm`).

## 2026-10-25 — Stream Deck renderer crates and font confirmed (VS7-05, ADR-009)

**Decision:** the key image renderer in `cmdmx-control` uses `fontdue` 0.9 (MIT/Apache-2.0/Zlib) and `jpeg-encoder` 0.7 ((MIT/Apache-2.0) and IJG), both with `default-features = false`, and the built-in font Atkinson Hyperlegible Bold (SIL OFL 1.1, `cmdmx-control/fonts/`, licence beside it). JPEG quality 90 without chroma subsampling, so small text stays crisp.

**Alternatives considered:** the crates with their default `simd`/`std` features; Inter or Roboto as the font.

**Reasoning:** without SIMD and with fontdue on `libm` the bytes are the same on the arm64 Mac and the x86-64 CI, so the golden images (CTL-51) hold on both. Atkinson Hyperlegible is made for legibility at small sizes, is 55 KB and has the `…` glyph.

**Reversal trigger:** the golden images differ between Mac and CI, or the owner finds the keys hard to read in the VS7-07 run.

## 2026-10-25 — Fan: four curves, latched, wheels only (P1-20, owner)

**Decision:** Fan is live engine state toggled by the Fan key, off on a new selection; a fanned wheel turn changes each target by `d · k` with *k* from the curve over the targets that have the attribute, in selection order (items, cells included). Phase 1 has Line, Mirror, Wings and Arrow with the factors of PRG-58; Mirror and Arrow are redefined because the UI v0.1 sketch had them as each other's negative. Only wheels fan; fanned values are plain manual values. Spec PRG-56..62, UI-30, IPC-43, CTL-59; build tasks VS2-35..36.

**Alternatives considered:** press-and-hold Fan; Titan's seven curves, Fan parts and group options now; a keypad fan; the sketch's formulas.

**Reasoning:** the four curves cover moving-head spreads, V shapes and rainbows; the rest is Titan polish that plugs into the same factor rule later. Engine state means every surface fans alike and the UI holds no lighting logic.

**Owner answers (2026-10-25):** all seven recommendations of P1-20 (see the spec's Owner decisions).

**Reversal trigger:** the owner finds a curve shape wrong against Titan on a real rig.

## 2026-10-25 — Stream Decks over native USB HID from the daemon (ADR-009, P1-19, owner)

**Decision:** Elgato Stream Decks are devices on the general control layer, driven by the daemon over USB HID (`hidapi`, own `deck` thread); no Elgato plugin. A deck is opened only while enabled, then exclusively (`Busy` while the Elgato app holds it); models are a table, not code; the daemon renders the key images (halo bar, legend, function, level bar, state) with a pure renderer in `cmdmx-control`; decks have their own pages besides the console page; links by drag onto an on-screen deck and by Learn. Spec CTL-44..58, ADR-009; build tasks VS7-05..08.

**Alternatives considered:** an Elgato Stream Deck plugin talking OSC/IPC; both routes; a shared open; the app rendering the images.

**Reasoning:** a show must not depend on a second program (the Elgato app, a Node runtime) running; the daemon keeps the deck alive with the app closed. The Elgato app and Bitfocus Companion can already send OSC to `/cmdmx` for whoever wants that route.

**Owner answers (2026-10-25):** all nine recommendations of P1-19 (see the spec's Owner decisions).

**Reversal trigger:** raw HID on macOS proves unreliable in the gate-3 run, or Elgato changes the protocol beyond the open references.

## 2026-10-25 — Fan is phase 1, Align stays phase 2 (IDEA-27, owner)

**Decision:** plain Fan (spread a value over the selection order) moves from phase 2 to phase 1; spec task P1-20. Align stays in phase 2.

**Alternatives considered:** Fan and Align together in phase 2 (`PROJECT.md` until now).

**Reasoning:** fanning positions of moving heads is basic programming; a phase-1 console without it is not a general console.

**Reversal trigger:** none expected.

## 2026-10-25 — Per-cell selection (P1-13, owner)

**Decision:** `programmer.md` PRG-46..55, `ui.md` UI-29, `ipc.md` IPC-42. A selection item is a whole fixture or one cell (`901.2`); cell ranges follow Titan's keypad table (`901 . 2 Thro 4` = cells 2–4 of 901, `901 Thro 904 . 2` = cell 2 of each); a cell without a Dimmer gets a virtual Dimmer under the master Dimmer; within one source a cell value beats the fixture value, between sources the ENG rules decide per cell; cues, groups and palettes key cell values by item; Unfold opens cells in place in the Fixtures window; IPC `v` goes to 1.
**Alternatives:** grandMA's running cell count; cells with colour only (`@` refused on a cell); Titan's Unfold replacing the whole page; cells as a separate list next to the selection.
**Why:** Titan is the behavioural reference for the keypad; a virtual cell dimmer makes cell chases on the Shehds possible; unfolding in place keeps the other fixtures in view.
**Reversal trigger:** pixel fixtures with dozens of cells make in-place Unfold unusable (then a page per fixture, as Titan).

## 2026-10-25 — IPC protocol versioning (P1-06, owner)

**Decision:** `ipc.md` IPC-41: `v` is one integer for the whole contract and goes up only on a change an older peer would misread (key removed or renamed, type/unit/meaning changed, command reshaped); new types, commands and keys do not raise it. Daemon and app each speak one version, no negotiation. The app checks the snapshot's `v` and on a mismatch closes, renders and sends nothing, shows both versions in place of the offline banner and keeps retrying; the daemon answers a command with another `v` with an `error` naming both versions. Tick, DMX output and other clients never notice. Build task VS1-09.
**Alternatives:** version negotiation (a daemon speaking several versions); bumping `v` on every addition.
**Why:** app and engine ship together from one repo, so one version each is enough; the snapshot arrives first, so the app sees a mismatch in both directions and the operator gets a clear message instead of a misread console.
**Reversal trigger:** remotes from other releases (IDEA-06) must keep working across an engine update.

## 2026-10-24 — The church remote gets a TouchOSC layout made for ColourmotionDMX (VS4-04, owner)

**Decision:** VS4-04 builds a new TouchOSC layout on the `/cmdmx` schema (CTL-36/37), shipped in the repo, instead of exporting the old church layout from the iPad mini and binding its addresses with custom-address Learn (CTL-38). CTL-38 stays for other people's layouts. Amends owner decision 8 of `control-surfaces.md`.

**Alternatives considered:** export the old `.tosc` and learn its addresses (the 2026-10-23 plan).

**Reasoning:** a layout tailored to the console uses its feedback (legends, page, cue texts, messages) that the old layout cannot show, and VS4-04 no longer waits for the old iPad.

**Reversal trigger:** the church operators need the old layout's look and placement unchanged.

## 2026-10-24 — Range edits in the patch table (P1-15, owner)

**Decision:** Move, Renumber, Name, Change Mode and Remove work on a multi-row selection in the patch table. Each range command is checked as a whole and refused naming the first clash (no parking), worked in number order, one undo step. Move places the range back to back from one start; Renumber keeps the order and closes gaps; a name ending in a number counts up ("Par 1…6"), another name is given to all; Change Mode needs the mode name on every fixture and refuses instead of re-packing when a bigger footprint no longer fits. Wire: `ids` on the existing `patch.*` commands. Moving several handles at once is a later task. Criteria PAT-19..25, IPC-40.

**Alternatives considered:** Titan's repatch that keeps the spacing between fixtures; re-packing the range on a mode change; one profile per range; click order; new `*_range` commands.

**Reasoning:** back to back is what a rig of identical pars needs and what the owner asked; silent re-packing would move fixtures nobody asked to move; number order matches the sorted table.

**Reversal trigger:** the owner misses Titan's keep-spacing repatch in a real patch session.

## 2026-10-24 — Merged = done; tracking checked in CI (owner)

**Decision:** A task PR sets its own `BACKLOG.md` row to `done (date, #PR)`, or `gate3: <checklist path>` when only the owner's hardware run is left. Statuses are `todo · doing · done · gate3 · blocked` (first word of the cell); `review` and `merged` are gone. Open points from a run become their own backlog row or IDEA in that PR. Root `STATE.md` is the handoff (focus, next, gate 3, blocked, owner decisions), not a log. `scripts/check-tracking.py` runs in CI: status vocabulary, gate3 paths exist, every IDEA section has an index row, and a task id in the PR title is not left `todo`.

**Alternatives considered:** a catch-up "sync planning" PR now and then (#60, #83); a notes column next to Status; GitHub Issues as the task list.

**Reasoning:** the rows were written before the merge ("in review") and nobody flipped them after, so drift was built in; a check in CI catches it in the same PR instead of relying on memory.

**Reversal trigger:** the check blocks real work more often than it catches drift.

**Amended 2026-10-24 (owner: "ik ga het in een review niet zien"):** the owner cannot catch drift by reading diffs, so the PR shows it. Every PR description carries `## Tracking`, the output of `scripts/check-tracking.py --summary origin/main` (new tasks, status moves, ideas, open points), also written to the CI job summary; CI fails without it. Run files end with `## Open points`, each point `→` the row, IDEA or checklist it went to.

## 2026-10-23 — Library browsing and the profile editor (P1-14, owner)

**Decision:** The Patch screen's Add panel becomes a full-width strip with one search field over Manufacturer, Model and Mode columns (search narrows the columns; no separate result list), with an "In this show" entry and channel counts per mode from the index. A full-screen profile editor edits one mode at a time: per channel name, group, HTP/LTP, Default/Locate/Highlight and slots (From/To/Name, colour on C channels, Split/Merge/Remove). Slots stay per channel, not ONYX's separate wheels linked to channels. Save writes both the show's copy (one undo step, all or nothing: used modes keep their footprint, an attribute used by stored content may not disappear) and the user profile. Test uses a new absolute `programmer.set`, so it is ordinary programmer content. Gobo pictures wait for the icon library (IDEA-14); Lightkey slots are typed by hand. Criteria LIB-19..27, IPC-35..39; sketch `specs/ux/library-and-profile-editor.html`.

**Alternatives considered:** keeping the 320 px side panel with a result list; ONYX's wheel editor plus channel linking; saving to the library only (the show keeps its own copy, SHW-05, so fixtures would not change); carrying values across an attribute rename.

**Reasoning:** one interaction for search and browse; slots are already per channel in the model (LIB-15), so linking wheels would add a step and a concept; a profile edit is mostly made to change the fixtures in front of you.

**Reversal trigger:** profiles that need modes added in the editor, or renames that must keep recorded values.

## 2026-10-23 — Mackie Control profile built into the daemon; Channel ◀▶ as actions (VS1-07)

**Decision:** The shipped profile is a mapping file in `subprojects/engine/profiles/`, compiled into the daemon (`include_str!`), so it ships with the app and cannot be edited on disk; an edit writes a user copy of the same name that hides it, deleting the copy brings it back. A mapping's `device` may list several patterns separated by `|`. Channel ◀▶ are catalogue actions `channel next/prev` that move the surface's fader window and send no engine command. Any page change resets the window. Extender units are ordered by device name until the Control screen (VS7-04) lets the user set the order.

**Alternatives considered:** a profile file in the app bundle read at start (one more path to find and keep read-only); a fixed Mackie driver outside the mapping (then its buttons could not be relearned, CTL-33).

**Reasoning:** smallest change on the VS7-02 layer; every Mackie binding stays a normal, relearnable binding, and the driver only adds the window, 10-bit faders, touch and rings.

**Reversal trigger:** profiles that users should update without a new build, or a surface whose units must be ordered before VS7-04.

## 2026-10-23 — Control is a general MIDI and OSC layer; named hardware are profiles on it (P1-05, owner)
**Decision:** `cmdmx-control` is a general control layer: any MIDI device in and out (CoreMIDI in the daemon), notes/CC/14-bit CC/pitch bend/program change mapped to one action catalogue (faders, handle buttons, Go/Back, page, GM, programmer keys, keypad, wheels) that sends the engine's existing commands; Learn, feedback (motor faders, LEDs, value echo), mappings as portable JSON files, several devices at once; OSC in and out with a `/cmdmx` schema and custom addresses. The Behringer BCF2000 in Mackie Control mode is one surface profile on it (ADR-003 amended), the church TouchOSC remote one OSC client. Spec `specs/modules/control-surfaces.md` (CTL-05..43 draft; CTL-01..04 kept as the Mackie Control wire format). VS1-07 and VS4-04 re-scoped under it; new tasks VS7-01..04.
**Alternatives considered:** a BCF2000/MCU driver first with generic MIDI later (ADR-003 as written); a driver per device; OSC only for the TouchOSC layout.
**Reasoning:** the owner corrected that the work talked only about the BCF2000; the church is a playground, not the limit. One action vocabulary keeps lighting logic out of the control layer and makes every input obey undo and refusals like a screen button.
**Owner answers (2026-10-23):** mappings per Mac, not in the show; new devices disabled until enabled once; Pickup by default; OSC off by default and any sender when on unless an allow list is set; `/cmdmx` on ports 8000/9000; only the Mackie Control profile ships in phase 1; MSC in phase 2; Mackie defaults as CTL-33; `midir` plus an own OSC codec. The church TouchOSC layout (old iPad mini) is exported in VS4-04 if possible, else rebuilt on `/cmdmx`.
**Reversal trigger:** a surface needs behaviour the action catalogue cannot express without lighting logic in `cmdmx-control`.

## 2026-10-22 — Multi-universe output: nodes in the show (P1-18, owner)
**Decision:** output nodes are show data (`output` key, schema 6; OUT-13). Each universe gets a default Art-Net node (port-address n−1, broadcast) and sACN node (universe n, multicast), both enabled, added automatically when the universe gets its first fixture; the serial port is stored in the show (missing port = Degraded); several serial nodes allowed, `auto` at most once; `--artnet-target`, `--sacn-unicast`, `--sacn-priority` and `--serial` are removed, `--no-*` stay as run-time kill switches; output edits are undoable edits (OUT-25..37).
**Alternatives considered:** Titan's defaults (16 fixed lines, Art-Net off); the serial port per Mac in the daemon's state file; keeping the flags as overrides.
**Reasoning:** send only what is patched; Art-Net on keeps today's behaviour and the owner's Art-Net check at home; one place (the show) for routing like Titan; flags that fight the show's config confuse.
**Reversal trigger:** a show moved between Macs keeps failing on its stored serial port, or a venue's network suffers from broadcast Art-Net.

## 2026-10-22 — Undo lets go of LTP attributes as Clear does (PRG-45, owner)

**Decision:** when undo or redo leaves an LTP attribute out of the programmer or the located set, it follows `clear_ltp` like Clear and Off (Release → playback value or profile default; Freeze → keep). Build task VS5-04.

**Alternatives considered:** keep held LTP output as live state untouched by undo (UNDO-09 as built), so Pan/Tilt stay where Locate put them after Undo.

**Reasoning:** one rule for whatever lets go of a value is easier to learn; the output matches programmer and playbacks after an undo; grandMA's Oops and Titan's Clear behaviour let go the same way. Freeze stays available for those who want values to stay.

**Reversal trigger:** operators expect undo never to move a light that undo did not explicitly touch.

## 2026-10-22 — Undo: snapshots of the edit state, edits only, one shared history (ADR-008, P1-10, owner)

**Decision (owner approved the seven recommendations):** one engine-wide undo (`specs/modules/undo.md`, UNDO-01..20, IPC-30/31). A step is one accepted command that changes show or programmer; wheel turns, colour-picker drags and selection taps in a row merge into one step, by command order, never by time. Live actions (Go, faders, Flash, GM, page) are not undoable and not in the history. Linear redo, 100 steps, memory only (emptied by Open/New and a restart), autosave writes undone state like any edit, one history shared by all clients. Steps save the edit state before the command (ADR-008), not inverse commands.

**Alternatives considered:** inverse commands per module; undo of live actions; history in the show file; per-client histories; time-based grouping.

**Reasoning:** snapshots make every module undoable without per-module code, so undo stays complete as modules are added; Titan undoes fixture and cue changes, not playback; time-based grouping would break the deterministic core.

**Reversal trigger:** memory of 100 snapshots is too large on a real rig, or operators expect Go to be undoable.

## 2026-10-22 — Phase 1 is driven by what a general console needs; undo first (owner)

**Decision:** the focus is a well-working general DMX lighting application; the church stays the reference venue and first proof, not what orders the work. Next, in this order: (1) undo — spec P1-10, then build it; (2) VS2-05 Locate, Highlight, Off, Rem Dim and Clear; (3) multi-universe output (OUT-13, P1-18); (4) control surfaces (P1-05, then VS1-07 and VS4-04). Church hardware runs (VS2-13 gate 3, VS4-05 gate 3) and the specs P1-14/P1-15 come after.

**Alternatives considered:** multi-universe output first (biggest gap: only line 1 leaves the daemon); undo last, or only its spec early.

**Reasoning:** undo touches every module, like the effect room of ADR-007: the more is built without it, the more has to be rebuilt; building it first makes VS2-05, output config and control surfaces undoable from the start. Locate/Highlight/Clear are what an operator uses every minute. More than one universe is a must, but the owner has not needed it yet.

**Reversal trigger:** a rig larger than one universe needs to run before undo is done.

## 2026-10-22 — A relative effect layer in the merge, effect references in contents (ADR-007, P1-17, owner)

**Decision:** ADR-007 accepted, `engine-merge.md` ENG-14..19 approved: per fixture, attribute and tick the output is base value → effect → clamp → group masters → Grand Master/DBO → frame. A relative effect adds an offset evaluated from the tick time, an absolute one replaces the base. At most one effect per attribute (highest priority, then most recently activated, also on HTP), chosen independently of the base owner. The held LTP value stays the base. Cue and palette contents (and the programmer's) may hold an optional effect reference next to a value; absent until effects exist, so show schema 5 is unchanged. No effect code in phase 1. See `specs/ADRs/ADR-007-relative-effect-layer.md`.

**Alternatives considered:** effects as ordinary absolute sources; effects after the group masters; several effects adding up on one attribute (left open for `effects.md`); inline effect definitions in cues.

**Reasoning:** a circle must run around the position a cue gives (what made Titan shapes quick); a Limit master must stay a hard limit; references match palettes. Fixing the order now means the phase-2 FX generator (IDEA-18) does not redo the merge cue lists and group masters are built on.

**Reversal trigger:** the phase-2 FX design study shows effects must act after the masters or live inline in cues.

---

## 2026-10-21 — Group masters as on Titan, mastering everything after the merge (P1-16, owner)

**Decision:** `specs/modules/group-masters.md` GRM-01..19 approved with its six recommendations: the master level is saved in the show (schema 5), so a restart never changes the stage; the programmer is mastered too, as by the Grand Master; Disabled locks the last effect and re-enabling jumps to the fader; masters of different modes on one fixture apply Scale → HTP → Limit, then the Grand Master/DBO; default buttons none/Flash with Swop as a `Custom` choice; several masters per group. Build tasks VS3-06 (core) and VS3-07 (IPC + app).

**Alternatives considered:** every master at its no-effect level after a restart; mastering only playbacks; Disabled = no effect, or a pickup on re-enable; one master per group.

**Reasoning:** the church's front spots on 1.8 must stay where they were after a daemon crash; a Limit has to be a hard limit; Titan allows a group button on any number of handles.

**Reversal trigger:** operators find a dark programmed fixture under a master at 0 confusing despite the amber level (then master only playbacks).

## 2026-10-21 — Cue lists over IPC and in the app (VS3-05, owner)

**Decision:** `ipc.md` IPC-24..28 and the `ui.md` amendments of UI-03, 13, 18, 19, 22, 23: `cue.view` is answered by the tick to the asking client only (never waiting); taps and handle buttons carry their receive time; `master.record` without a number takes the lowest free one; `playback.options` list/chase/master fields are each optional; entries carry the handle's button functions, so the app never applies `handle_buttons` itself. Magenta (#E54BB4 / #B8288A) means a tracked value. Show-wide playback settings sit in a Settings tab of Playback Options; BPM/Rate masters are recorded from Record-context softkeys.

**Alternatives considered:** the app choosing master numbers and button labels itself; a separate settings screen; broadcasting the Cue view to every client.

**Reasoning:** the app stays a thin client (CLAUDE.md); a tap must not be rounded to the 25 ms tick (CUE-33).

**Reversal trigger:** the Cue view needs live refresh on edits from other clients (then a revision counter on entries).

## 2026-10-21 — Serial output on its own 30 ms timer (VS4-05, OUT-24, owner)

**Decision:** The serial node's thread keeps the latest frame handed over and writes it every 30 ms, counted from the start of the previous write (~33 Hz), instead of skipping every second tick (20 Hz). Art-Net and sACN stay on the 40 Hz tick.

**Alternatives considered:** keep 20 Hz; a slower 25 ms tick for everything; frames cut to the highest patched channel (shorter writes, kept as the next step if fades still step).

**Reasoning:** the upgrade path named on 2026-10-18: a dragged fade on the RGBL par showed visible steps at 20 Hz, and 33 Hz was steady on the MK2 in the C-sender test. The tick stays 40 Hz and never waits on the serial port (OUT-11).

**Reversal trigger:** the par flickers at 33 Hz on the real rig, or steps remain (then frames cut to the highest patched channel).

## 2026-10-21 — Chase details for VS3-04 (CUE-30..34, owner)

**Decision:** Go on a chase whose fader is at 0 runs it with its HTP out; the kill point only acts when the level falls to 0 (as cue lists with `KillAtZero`). On a paused chase Back and Go to cue step with the crossfade and the chase stays paused; Stop again goes back; only Go resumes. Tap and a typed tempo on a chase following a BPM master go to the master, and a tap on a master starts the next step of every running chase on it at the tap (beat sync). Back goes one step against the direction (Random: the step shown before) and wraps with `Loop`; `StopOnFinal` runs one round per direction (Bounce 1 2 3 2 1, Random as many steps as the chase has).

**Alternatives considered:** killing a chase at once at fader 0; Back resuming a paused chase as on a cue list; a master tap that only sets the tempo; Back in number order without wrapping, Bounce and Random always looping.

**Reasoning:** common practice on Titan and grandMA (the fader is the intensity master, Prev/Next Step while stopped); a BPM master exists so several chases tap in time.

**Reversal trigger:** operators at the church find a paused chase that does not resume on Back confusing.

## 2026-10-21 — Cue lists track in phase 1; one Go block on the connected list plus Go per handle (P1-08, owner)

**Decision:** Answers to the first `specs/modules/cuelists.md` gate-1 questions. (1) A cue list lives on any handle with its own Go on a handle button; the Go block, Space and the Cue list window act on the connected list (last fired or tapped, else the first list in the show), as on Titan. (2) Handle buttons follow what is on the handle (memory Swop/Flash, cue list Go/Flash, chase Tap/Flash, BPM master Tap), configurable per kind for the show. (3) The cue-list fader at 0 keeps the list on its cue by default (`KillWithOff`), configurable. (4) **Tracking moves from phase 2 into phase 1** (Track/Block/Cue Only/Solo, Update modes, Cue view); this reverses that part of the 2026-10-17 phase-1 decision. (5) BPM masters by Tap/typing by default, Titan's fader modes as options; Rate masters 0–200 %. Ableton sync becomes IDEA-16 (phase 2). Follow-up the same day: a list or chase on a handle takes these best-practice buttons by default; the setting `handle_buttons` switches to Titan's factory Swop/Flash or a custom pair per kind. The spec (CUE-01..55) is approved.

**Alternatives considered:** a Go block pinned to one list, or no Go block; fixed Swop/Flash everywhere; Titan's Kill At 0 as default; cue lists without tracking until phase 2 (each cue a whole intensity look); BPM on the fader only.

**Reasoning:** Titan practice; the owner programs big shows, where a list without tracking is not workable; cues already store touched attributes only, so tracking needs no change to stored cues.

**Reversal trigger:** tracking makes the church service harder to program than Lightkey, or the connected-list model confuses operators at the church.

## 2026-10-18 — Playbacks follow Titan; a playback holds a list of cues (P1-04, owner)

**Decision:** `specs/modules/playbacks.md` PBK-06..45. Playbacks live on stored handles (page.slot) with Titan's paging locks; recording stores the programmer (touched attributes only by default, Record By Channel) with palette references and fallback values; fader modes 0/1/2 with delay/fade in/fade out/overlap per cue (mode 1 falls no faster than full scale per fade-out time); Titan kill point, release mask (empty global default) and release time (2 s); fixed Swop/Flash plus Latch; automatic halo from the cue's colour; DBO as live toggle. A playback stores a list of cues (one in phase 1), so P1-08 cue lists need no second migration. Chases with BPM/Rate masters move to P1-08; group masters get their own spec (P1-16); attribute-group times come with P1-08.

**Alternatives considered:** Mixed or By Fixture as record default; Titan key profiles now; halo only as a user choice; a separate cue-list storage kind; chases and masters inside `playbacks.md`.

**Reasoning:** market-conform Titan behaviour; touched-only cues keep phase-2 tracking open; one storage shape for memories and cue lists keeps the show file stable.

**Reversal trigger:** a Titan behaves differently in modes 0/1, or operators miss key profiles in the church.

## 2026-10-18 — Palettes follow Titan, market practice over the church (P1-03, owner)

**Decision:** `specs/modules/palettes.md` PAL-01..32. A palette is a reference: programmer and playbacks store `{"palette": id, "value": fallback}`; the merge resolves it every tick and uses the fallback when the palette resolves nothing or is deleted (Titan's delete behaviour). Global covers intensity, Pan, Tilt and Colour as on Titan, with Pan/Tilt stored in degrees via the profile's physical ranges. Record By defaults to Mixed (Channel as option). A palette lives on the cell it was recorded on and gets the next free number; its window only proposes the record mask (not ONYX's place-decides model). No Edit in phase 1 (Include + Record → Merge); icons via SF Symbols plus own glyphs, the show stores only our own icon ids; overlap time is per fixture.

**Alternatives considered:** freezing current values on delete; Pan/Tilt shared only (church-sized); Channel as default; fixed number ranges per window (the sketch); an open SVG icon set (Lucide).

**Reasoning:** the owner wants ColourmotionDMX to be market-conform, not church-sized; Titan is the behavioural reference; a place-decides model is what the owner finds annoying in ONYX.

**Reversal trigger:** Avolites users find a deviation in practice, or a browser remote needs icon files (then map our ids to an open set).

## 2026-10-18 — Eurolite MK2: 250000 baud over Apple's VCP driver, not auto-detected

**Decision:** The serial node opens the port at 250000 8N1 (OUT-20, was 115200). An FTDI port with a generic product string ("FT232R USB UART") is never opened automatically; the "no interface" log line names it with a `--serial <path>` hint (OUT-09/23). The owner chose this over treating every FTDI 0403:6001 port as ENTTEC Pro and over a serial-number allowlist.

**Alternatives considered:** detaching Apple's FTDI driver and using D2XX/libusb (as Lightkey's admin prompt suggests); probing ports with the ENTTEC "get widget parameters" request; auto-using every FTDI port; a USB-serial-number allowlist.

**Reasoning:** On the owner's Mac the MK2 followed a full/off pattern at 250000 baud through the stock VCP driver, so no admin rights or driver change are needed. At 115200 a 518-byte frame took ~54 ms per 25 ms tick and the unit ignored it. The MK2 does not answer ENTTEC requests and shares its product string with Open DMX clones, so neither probing nor the string can identify it, and sending Pro frames to an unknown FTDI device (an Open DMX clone, an Arduino) is not show-proof. Once the show stores node config (OUT-13), the chosen port is stored there. Same day, OUT-24: serial writes at most every 30 ms (20 Hz with 25 ms ticks), because the MK2 flickered an LED par at 40 Hz and was steady at ≤ 33 Hz; the owner chose this over a 33 Hz own-timer serial thread (upgrade path if fades step visibly).

**Reversal trigger:** an MK2 unit that answers label 3 or reports a Pro product string (then detection covers it), or the owner wants plug-and-play before OUT-13 lands.

## 2026-10-18 — The church patch: numbers in blocks of 100 per type, profiles from QLC+ where exact (VS2-13, owner)

**Decision:** The church rig gets user numbers in blocks of 100 per fixture type (101–120 Showtec pars, 201–204 Phantom 25, …, 1201), names are the Lightkey short names with duplicates moved to the next free one (P45, MH9, MH10). Types QLC+ has with Lightkey's channel order come from QLC+; the other eight are channel lists from the Lightkey show; the Shehds zoom channel is called Zoom (user profile). Made once by `cargo run -p cmdmx-daemon --example church` (PAT-15..18).

**Alternatives considered:** numbers equal to the Lightkey short names (P27 → 27); 1..56 in address order; all twelve types as channel lists; a Lightkey profile importer first.

**Reasoning:** Titan-style blocks keep each type together on the command line and in the Fixtures window; QLC+ profiles carry slots and ranges the channel lists lack; an importer can come later (IDEA-13 asks for slot editing anyway).

**Reversal trigger:** the owner finds the block numbers harder to use in a service than the Lightkey names.

## 2026-10-17 — Work towards the real UI early, zone by zone

**Decision:** UX-02 (the SwiftUI main screen as UI v0.1) is split into a UI spec (UX-02.0) and four zones (UX-02a shell, b attribute bar, c side column, d playback area), built now on top of the existing IPC instead of after the engine features. A zone whose engine part is not built yet is shown disabled and labelled with the spec it waits for; new engine features land straight in the real layout.

**Alternatives considered:** keep the thin working layout until palettes, playbacks and cue lists exist, then build the main screen in one go.

**Reasoning:** owner: the sooner we work in the real UI, the more feel we get for it, and the project becomes interesting to follow for outsiders earlier. Building each later feature into its final place also avoids reworking a temporary layout.

**Reversal trigger:** the real layout slows engine work down (e.g. placeholders need rework each time a spec lands).

## 2026-10-17 — The QLC+ library is fetched at a pinned commit, not committed (VS2-11)
**Decision:** `subprojects/engine/scripts/fetch-qlcplus.sh` downloads QLC+'s `resources/fixtures` (commit 3d832ce, 1781 files, 18 MB) with its Apache-2.0 licence and a NOTICE into the git-ignored `vendor/qlcplus`; the daemon indexes it in the background. Only the three church files live in the repo (tests, LIB-09).
**Alternatives:** committing the whole set (18 MB of third-party files in every clone and diff); a cargo build script with network access (breaks offline and reproducible builds).
**Why:** small repo, reproducible version, clear licence boundary. The app bundle will copy the folder in when the daemon ships inside it.
**Reversal trigger:** CI or packaging needs the files without network access (then vendor a compressed archive).

## 2026-10-17 — Colour defaults to open white; the virtual dimmer also scales emitters (owner)
**Decision:** The `Colour` attribute defaults to open white (1, 1, 1), so a fixture brought up on its dimmer alone shows white (LIB-03). On a mode without a dimmer channel the virtual Dimmer (Grand Master included) scales every colour channel **and** every emitter (LIB-04). Hue/Saturation are HSV of the target colour and keep max(r, g, b) (PRG-30). Colour in the app is its own task (VS2-14).
**Alternatives:** black as the default colour (dimmer up shows nothing until a colour is picked); scaling only RGB (a Grand Master blackout would leave Lime/White burning on virtual-dimmer pars).
**Why:** no "I see nothing" surprise on LED fixtures; a blackout must darken everything.
**Reversal trigger:** an operator finds white-at-dimmer-up gets in the way of programming (then make the default a profile/show setting).

## 2026-10-17 — Extra emitters are recognised by a flag, not by name (LIB-17, owner)
**Decision:** Every profile attribute carries an `emitter` flag; the engine treats an attribute as an extra emitter (direct wheel PRG-34, ADR-005 palette zeroing, defaults) only by that flag, so any number of emitters with any name works (Amber, Warm/Cold White, UV, Royal Blue, …). Duplicate names take the channel name on import; C/M/Y next to RGB are emitters, CTO/CTB are not.
**Alternatives:** a fixed list of emitter names (breaks on Warm/Cold White, which QLC+ has no preset for, and on unknown LED colours).
**Why:** same principle as GDTF's `ColorAdd_*` family; the owner wants every deviating emitter under direct control, not just White and Lime.
**Reversal trigger:** none foreseen; per-emitter colour data (IDEA-03) is added alongside the flag, not instead of it.

## 2026-10-17 — Own profile model filled by importers, QLC+ first (ADR-006, accepted)

**Decision:** The engine's own profile model is the only internal fixture format; importers convert QLC+ `.qxf` (bundled, Apache-2.0), typed channel lists and GDTF (phase 1), later OFL, into it. Patch: internal id + user number as on Titan; no church-driven cuts (owner). Supersedes ADR-004 in part. See `specs/ADRs/ADR-006-own-profile-model-with-importers.md`.

**Alternatives considered:** GDTF internal (ADR-004); QLC+ format internal; own curated library.

**Reasoning:** The church's fixtures are in the QLC+ library, not in GDTF or OFL; OFL does not export GDTF; the owner does not want to reinvent the library.

**Reversal trigger:** QLC+ profiles prove unreliable for the church fixtures, or GDTF-share gains the budget brands.

---

## 2026-10-17 — Colour as a target colour plus direct emitters (ADR-005, accepted)

**Decision:** One `Colour` attribute per fixture holds a linear RGB target colour in the BT.2020 reference space; White, Lime and other extra emitters are ordinary attributes the operator drives directly; phase 1 maps the target colour nominally onto each fixture's RGB or CMY channels; a colour palette with a target colour zeroes the emitters it does not mention. See `specs/ADRs/ADR-005-target-colour-and-direct-emitters.md`.

**Alternatives considered:** raw DMX per emitter; CIE xyY storage; sRGB reference space; automatic white/lime extraction.

**Reasoning:** Palettes must work across the church's three LED colour systems now and accept gels and calibration later without rewriting shows; the owner wants White and Lime under direct control.

**Reversal trigger:** nominal mapping makes the mixed pars unusable side by side, or direct-only emitters are too much work for everyday whites.

---

## 2026-10-17 — Programmer behaviour choices (P1-02)

**Decision:** Clear defaults to selection-then-programmer (Titan's one-press Clear is a setting); Locate removes the located attributes from the programmer; a typed command-line selection replaces the selection while tapping adds; Fan and Blind stay out of phase 1 (greyed in UI v0.1); undo gets its own item (P1-10). Spec: `specs/modules/programmer.md`.

**Alternatives considered:** Titan's defaults for Clear and Locate; Fan/Blind in phase 1; undo inside the programmer spec.

**Reasoning:** A first Clear that keeps your work is safer during a service; with Locate, what is on stage is what gets recorded. Fan is phase 2 in `PROJECT.md`; undo spans programmer and show edits.

**Reversal trigger:** Avolites muscle memory at the church makes the Clear or Locate deviation trip the operator.

## 2026-10-17 — Main-screen sketch is UI v0.1

**Decision:** `specs/ux/main-screen.html` (UX-01, iteration 2 with owner tweaks) is UI v0.1: the reference for the SwiftUI main screen and for the programmer, palette and playback specs. Its layout (workspace rail, fixed 2×2 window grid, function buttons, keypad + command line, softkeys, attribute bar, pages, playback strips, Go block, GM/DBO) and visual rules (flat, dense, hairlines, colour only for state) hold until the owner changes them. Background: `specs/ux/research.md`.

**Alternatives considered:** iteration 1 (generic dark dashboard, rejected by the owner); a 1:1 copy of the Titan look (dated).

**Reasoning:** The owner judged it right ("this becomes v0.1, we build on this first"). It keeps the Titan layout an Avolites user knows and gives it a modern, strict look.

**Reversal trigger:** the SwiftUI build or a church test shows a zone that does not work in practice; change the sketch first, then the app.

---

## 2026-10-17 — Phase-1 finish line: replace Lightkey in the church

**Decision:** Phase 1 is done when a real Sunday morning service in the church is programmed and run on ColourmotionDMX instead of Lightkey. Phase 1 is cut into milestones VS2 (programming), VS3 (running: cue list + busking), VS4 (the church).

**Alternatives considered:** an abstract feature list as the end of phase 1.

**Reasoning:** A real venue, rig and user keep the scope honest and force the hardware track to happen. The church is the owner's own playground, so what works there can move to other setups.

**Reversal trigger:** the church rig stops being representative of the shows the owner wants to run.

## 2026-10-17 — Phase 1 is programming the building blocks of any show

**Decision:** Phase 1 changes from "one evening of busking" to "program the building blocks of any show": a robust programmer at the core, palettes, cues on playbacks and in cue lists, and enough playback to run them. Cue-list tracking moves to phase 2; timelines (IDEA-05) to phase 3.

**Alternatives considered:** keep busking as the phase-1 goal; put tracking in phase 1.

**Reasoning:** The owner programs big shows; busking is one way of running them, not the goal. Cues store only touched attributes (Titan programmer behaviour), so tracking can be added without changing the show file.

**Reversal trigger:** the cue data model turns out to block tracking.

## 2026-10-17 — IPC transport: WebSocket

**Decision:** App and daemon talk JSON over a WebSocket on loopback. The same contract serves remote clients (iPad/phone/browser, like Titan Remote) later.

**Alternatives considered:** plain TCP with length-prefixed JSON (no extra crate).

**Reasoning:** The owner wants a remote later; browsers only speak WebSocket. Swift has WebSocket support built in; the daemon needs one small crate (choice in the IPC spec).

**Reversal trigger:** the WebSocket crate adds measurable latency to fader feel (> ~10 ms, see ADR-002 trigger).

## 2026-10-17 — `serialport` crate for the ENTTEC Pro output

**Decision:** The daemon uses `serialport` 4.x with default features off for USB product-string detection and raw serial I/O (VS1-04, OUT-09/20).

**Alternatives considered:** `libc` termios plus IOKit or `ioreg` parsing for the product string.

**Reasoning:** std cannot read USB descriptors or set a tty raw; doing both by hand is much more unsafe code. Default features off drops `libudev`, so the Linux CI needs no system package.

**Reversal trigger:** the crate misbehaves with the Eurolite MK2 at the church, or blocks the tick despite its own thread.

## 2026-10-16 — Public GitHub repo, back to one PR per task

**Decision:** The repo is public at github.com/Aottens/ColourmotionDMX from now on (answers the "public from day one?" question). Work goes through one PR per task again; this reverses the local-review entry below.

**Alternatives considered:** keep local review; private repo until vertical slice 1.

**Reasoning:** The owner created a public remote; PRs give CI on every task.

**Reversal trigger:** none expected.

## 2026-10-16 — Local review instead of PRs while there is no remote

**Decision:** One branch per task; the owner reviews `git diff main...<branch>` and approves; merge with `--no-ff` so the merge commit marks the review.

**Alternatives considered:** push to GitHub and use PRs now.

**Reasoning:** The repo is local only; a PR adds nothing until there is a remote and CI to run.

**Reversal trigger:** a GitHub remote is added — switch back to one PR per task.

## 2026-10-16 — Fixture format: GDTF as internal format, OFL/QLC+ import later

**Decision:** Fixture profiles are GDTF files plus a small JSON overlay for Titan-specific fields (attribute group, locate values, virtual dimmer, highlight). See ADR-004.

**Alternatives considered:** own JSON format; Avolites `.d4` XML; QLC+ `.qxf`.

**Reasoning:** GDTF is the open industry standard with the largest library (GDTF-share); Avolites itself imports GDTF since Titan v17. Writing our own format means writing our own library, which is the one thing a two-person project cannot do.

**Reversal trigger:** GDTF parsing turns out to be unreliable for common entry-level fixtures, or the overlay grows into a full format of its own.

## 2026-10-16 — Controllers speak Mackie Control first

**Decision:** The first control-surface driver implements Mackie Control Universal (MCU). BCF2000 runs in MC mode. A generic MIDI note/CC mapping is the second driver. See ADR-003.

**Alternatives considered:** custom CC mapping per controller; HUI.

**Reasoning:** MCU gives 10-bit faders, motor feedback, button LEDs and encoder rings under one protocol, and works unchanged with BCF2000, X-Touch, X-Touch Compact and iCON. One driver, whole family.

**Reversal trigger:** MCU's fixed 8-channel bank model gets in the way of configurable fader counts in a way that cannot be solved with bank offsets.

## 2026-10-16 — Engine runs as a separate headless process

**Decision:** The engine is a daemon; the macOS app is a thin client over local IPC. See ADR-002.

**Alternatives considered:** single process with engine thread.

**Reasoning:** Show-proof requirement: a UI crash or hang must not touch DMX output. Also enables remote UIs later.

**Reversal trigger:** IPC latency measurably hurts fader feel (> ~10 ms end-to-end) and cannot be fixed with a lighter transport.

## 2026-10-16 — Rust engine, Swift/SwiftUI app

**Decision:** Engine and protocols in Rust; native macOS UI in Swift/SwiftUI built with SwiftPM and Command Line Tools only. See ADR-001.

**Alternatives considered:** all-Swift; C++/Qt (industry norm); C#/.NET (Titan, Onyx); Tauri/web UI.

**Reasoning:** The owner does not write the code himself, so a second language costs nothing in learning; the primary requirement is "must not crash", which favours Rust's error model over Swift's trapping unwraps; Rust has mature crates for sACN, Art-Net, serial, MIDI and GDTF. UI stays native for macOS feel.

**Reversal trigger:** FFI/IPC friction between the two languages dominates feature work.

## 2026-10-16 — 8 faders per page by default, count is configuration

**Decision:** `faders_per_page` defaults to 8 (BCF2000, X-Touch, nanoKONTROL all have 8); 10 or 16 are settings, not forks.

**Alternatives considered:** fixed 10 (Titan T3/Titan Mobile layout).

**Reasoning:** No affordable MIDI controller has 10 faders; two on-screen-only faders are the two you cannot find while busking.

**Reversal trigger:** none expected; the setting covers it.

## 2026-10-16 — Repo language English, conversation Dutch

**Decision:** All files in the repo are English (open-source audience). Conversations with the owner are Dutch; CLAUDE.md says so.

**Alternatives considered:** all Dutch.

**Reasoning:** Contributors and fixture-library ecosystem are international.

**Reversal trigger:** project stays private indefinitely.
