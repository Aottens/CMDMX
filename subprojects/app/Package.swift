// swift-tools-version:5.9
// ColourmotionDMX macOS app — thin SwiftUI client for cmdmx-daemon.
// Builds with Xcode Command Line Tools only: `swift build`. No .xcodeproj (ADR-001).
import PackageDescription

let package = Package(
    name: "ColourmotionDMX",
    platforms: [.macOS(.v14)],
    targets: [
        .executableTarget(
            name: "ColourmotionDMX",
            path: "Sources/ColourmotionDMX"
        ),
        // No test target: neither XCTest nor Swift Testing ships with the Command
        // Line Tools. The app is a thin client; behaviour is tested in the engine.
        // Add a test target again only with a toolchain that provides one
        // (Xcode.app, or a swift.org toolchain) — see CLAUDE.md.
    ]
)
