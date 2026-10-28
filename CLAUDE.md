# CLAUDE.md — how to work in this repo

CMDMX is a show-proof, open-source macOS lighting console modelled on Avolites Titan.
Read `PROJECT.md`, `STATE.md`, `ARCHITECTURE.md` and `DECISIONS.md` before doing anything.
Talk to the owner in **Dutch**; write code, comments, specs and commits in **English**.

## The loop: spec → test → code → review

1. **Spec first.** Every feature has a spec in `specs/modules/<module>.md` with numbered
   acceptance criteria (`ENG-01`, `OUT-03`, …). No code for a criterion that has no number.
   If the spec is missing or unclear, write or extend it and stop for owner approval (gate 1).
2. **Test second.** Each criterion gets a test carrying its id in the test name
   (`fn eng_01_htp_highest_wins()`), or a scenario file `tests/scenarios/<id>-*.json`.
   Write the failing test before the implementation.
3. **Code third, by a Sonnet subagent.** The main session writes spec and failing tests; a
   Sonnet subagent (`model: sonnet`) writes the smallest change that turns them green; the main
   session reviews the diff before it goes to the owner (owner preference, 2026-10-17). Run `cargo test` in
   `subprojects/engine` and `cargo clippy --all-targets -- -D warnings`.
4. **Review.** One branch and one PR per task on `github.com/Aottens/CMDMX`; the owner
   reviews and merges (gate 2). Hardware-touching tasks ship with
   a checklist in `tests/hardware/` that only the owner can tick (gate 3).
5. **Close the loop.** Update the subproject `STATE.md`, root `STATE.md`, `BACKLOG.md` and
   `README.md` in the same PR. In the README, a new feature goes into its group under
   *What works today*; `## Status` is rewritten, never appended to: where phase 1 stands and what
   is next, at most 8 lines (checked). Update the description whenever the project's direction
   has moved. A decision made along the way goes in `DECISIONS.md` (and an ADR if it is
   structural). **Merged = done:** the PR sets its own `BACKLOG.md` row to `done (date, #PR)`, or
   `gate3: <checklist path>` when only the owner's hardware run is left; never `review`/`merged`.
   Open points found in a run go under `## Open points` in the run file, each ending in `→` its
   backlog row, IDEA or checklist, made in that PR. Root `STATE.md` is the handoff (focus, next,
   gate 3, blocked, owner decisions), not a log. `scripts/check-tracking.py` checks this in CI.
   A PR that changes user-visible behaviour updates the manual section it touches
   (`subprojects/website/manual/src/`) in the same PR (WEB-33); `check-tracking.py` warns when
   `subprojects/app/Sources` changes without the manual.
   **Every PR description has a `## Tracking` section** with the output of
   `scripts/check-tracking.py --summary origin/main` (new tasks, status moves, ideas, open points),
   so the owner sees in plain lines what the PR records without reading the tables.
6. **Clean up after the merge.** GitHub deletes the merged branch; delete the local ones too, in
   the owner's checkout on `main`:
   `git fetch --prune && git branch -vv | awk '/: gone]/{print $1}' | xargs git branch -D`.
   Worktrees of archived sessions go via the app (Settings › Storage, or `clean_up_worktrees`).

Use the project-management conventions from the `project-manage` skill when available
(session start: read STATE files; session end: handoff in STATE.md). Use the
`project-orchestrator` agent in `.claude/agents/` for pre-flight and post-flight checks.

## Ideas intake

The owner drops ideas in chat. Record each one in `IDEAS.md` (id, intent, proposed phase,
module, what it touches now, open questions) and ask the owner to confirm the phase. No
criteria or code for an idea until its phase is planned. Phase 1 is `specs/PRD.md`; new ideas
default to a later phase. If an idea forces a decision earlier, add only that decision to
`BACKLOG.md` and say so.

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
- **No Swift unit tests for now.** Neither XCTest nor Swift Testing is available with the
  Command Line Tools alone (verified 2026-10-16 on the owner's Mac). The app is a thin client;
  all behaviour tests live in the Rust engine. `swift build` is the app's gate. If Swift tests
  become necessary, install a swift.org toolchain (ships XCTest + Testing, ~1.5 GB, no
  Xcode.app) and add a test target with `import Testing`.
- Hardware on the owner's desk: Behringer BCF2000 (Mackie Control mode).
- Hardware at the church (the venue; on the owner's desk since 2026-10-18 for testing): Eurolite
  USB-DMX512 PRO Interface MK2 (ENTTEC Pro label-6 frames at **250000 baud** over FTDI FT232R serial,
  Apple VCP driver, no admin rights needed). It reports the FTDI default product string
  "FT232R USB UART" (VID/PID 0403:6001) and does not answer ENTTEC requests, so it cannot be told
  from an Open DMX clone: never auto-detect it, use `--serial <path>` (OUT-09/23); never treat
  any interface as Open DMX.

## Commit style

`<area>: <imperative summary>` — e.g. `core: implement HTP merge (ENG-01..03)`.
Reference criterion ids in the body. One task per PR.
