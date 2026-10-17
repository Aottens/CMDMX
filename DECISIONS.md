# ColourmotionDMX — Decision Log

Append-only. Newest at top. Each entry: date, decision, alternatives, why, reversal trigger.
Long-form records live in `specs/ADRs/`; this log is the index.

---

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
