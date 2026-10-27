# ADR-001: Rust engine, Swift/SwiftUI app

**Status:** accepted
**Date:** 2026-10-16

## Context
CMDMX must be show-proof ("may not crash during a show") and the owner will not write the code himself; an AI agent writes it under spec/test gates. The UI must feel native on macOS. Industry consoles are C++ (MagicQ, grandMA, QLC+, OLA) or C# (Titan, Onyx); newer open-source lighting tools are increasingly Rust.

## Decision
- Engine, protocols and control-surface logic: **Rust** (stable, edition 2021), as a Cargo workspace.
- macOS app: **Swift/SwiftUI**, built with **SwiftPM and Xcode Command Line Tools only** (no Xcode.app, which the owner removed for disk space). A script produces the `.app` bundle.
- The two talk over local IPC (see ADR-002); there is no in-process FFI.
- No app unit tests for now: XCTest and Swift Testing are absent from the Command Line Tools. Behaviour is tested in the engine; `swift build` gates the app. Revisit with a swift.org toolchain if needed.

## Alternatives
- All-Swift: one language and the Apple frameworks are nearby, but a force-unwrap or out-of-bounds index is a hard crash with no recovery, and the ecosystem lacks sACN/Art-Net/GDTF libraries.
- C++/Qt: the industry norm, cross-platform, but the worst choice for an agent to write crash-free code in.
- C#/.NET (as Titan): mature, but non-native on macOS and heavier runtime.
- Tauri/web UI: compact and Xcode-free, but not native feel; could still be a secondary remote UI later.

## Consequences
- Two toolchains in CI (ubuntu `cargo test`, macos `swift build`).
- Rust's `Result`-based error handling plus clippy lints give us the crash-safety story; see CLAUDE.md show-proof rules.
- GDTF parsing must be written or adopted from a Rust crate; this is the one place all-Swift would have cost more.
- The app stays a thin client; any lighting logic in Swift is a review failure.
