# CLAUDE.md — how to work in this repo

ColourmotionDMX is a show-proof, open-source macOS lighting console modelled on Avolites Titan.
Read `PROJECT.md`, `STATE.md`, `ARCHITECTURE.md` and `DECISIONS.md` before doing anything.
Talk to the owner in **Dutch**; write code, comments, specs and commits in **English**.

## The loop: spec → test → code → review

1. **Spec first.** Every feature has a spec in `specs/modules/<module>.md` with numbered
   acceptance criteria (`ENG-01`, `OUT-03`, …). No code for a criterion that has no number.
   If the spec is missing or unclear, write or extend it and stop for owner approval (gate 1).
2. **Test second.** Each criterion gets a test carrying its id in the test name
   (`fn eng_01_htp_highest_wins()`), or a scenario file `tests/scenarios/<id>-*.json`.
   Write the failing test before the implementation.
3. **Code third.** Smallest change that turns the test green. Run `cargo test` in
   `subprojects/engine` and `cargo clippy --all-targets -- -D warnings`.
4. **Review.** Open a PR per task; the owner reviews (gate 2). Hardware-touching tasks ship with
   a checklist in `tests/hardware/` that only the owner can tick (gate 3).
5. **Close the loop.** Update the subproject `STATE.md`, root `STATE.md` and `BACKLOG.md`
   in the same PR. A decision made along the way goes in `DECISIONS.md` (and an ADR if it is
   structural).

Use the project-management conventions from the `project-manage` skill when available
(session start: read STATE files; session end: handoff in STATE.md). Use the
`project-orchestrator` agent in `.claude/agents/` for pre-flight and post-flight checks.

## Show-proof rules (non-negotiable in engine code)

- No `unwrap()`, `expect()`, `panic!`, `unreachable!`, `todo!` or indexing that can panic in
  `cmdmx-core`, `cmdmx-protocols`, `cmdmx-control` or the daemon's runtime paths.
  `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]`
  is set at crate level; tests may `#[allow]` locally.
- Errors are values (`Result`), surfaced to the UI as warnings; the engine never stops ticking.
- On any error in an output path: hold the last good frame, keep ticking, log once.
- The core is deterministic: time comes in as a parameter, never from `Instant::now()` inside core.
- No allocation in the per-tick hot path once vertical slice 1 is stable (profile before optimising).
- Panics at thread boundaries are caught (`catch_unwind`) and reported; the daemon keeps output alive.
- Autosave the show every few seconds to a temp file, then atomic rename.

## Architecture constraints

- UI contains no lighting logic. If you are writing HTP/LTP, fade, palette or patch logic in Swift, stop.
- The daemon is the only crate with I/O. Core, protocols and control are pure and unit-testable.
- Attribute group (I P C G B E S FX) is a property of the attribute in the fixture profile.
- Palettes are references; cues store palette ids.
- `faders_per_page` is configuration (default 8). Never hard-code 8 or 10.
- Titan is the behavioural reference. When in doubt about how something should feel, check the
  Titan manual (https://manual.avolites.com/docs/) and the analysis doc linked in `PROJECT.md`,
  then write the behaviour down as a numbered criterion before implementing it.

## Toolchain notes

- Rust: stable, edition 2021. `cargo test`, `cargo clippy`, `cargo fmt --check` must pass.
- Swift: the app builds with Xcode Command Line Tools only (`xcode-select --install`), via
  `swift build` in `subprojects/app`. Do not introduce an `.xcodeproj`. `scripts/bundle-app.sh`
  wraps the binary into a `.app`.
- Hardware on the owner's desk: Behringer BCF2000 (Mackie Control mode), Eurolite USB-DMX512 PRO
  Interface MK2 (ENTTEC Pro protocol over FTDI serial; identifies as "Eurolite DMX512 Pro",
  VID/PID 0403:6001 — detect by product string, never treat as Open DMX).

## Commit style

`<area>: <imperative summary>` — e.g. `core: implement HTP merge (ENG-01..03)`.
Reference criterion ids in the body. One task per PR.
