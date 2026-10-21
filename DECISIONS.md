# ColourmotionDMX — Decision Log

Append-only. Newest at top. Each entry: date, decision, alternatives, why, reversal trigger.
Long-form records live in `specs/ADRs/`; this log is the index.

---

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
