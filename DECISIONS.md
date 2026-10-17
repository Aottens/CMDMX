# ColourmotionDMX — Decision Log

Append-only. Newest at top. Each entry: date, decision, alternatives, why, reversal trigger.
Long-form records live in `specs/ADRs/`; this log is the index.

---

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
