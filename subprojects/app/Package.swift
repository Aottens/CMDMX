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
        .testTarget(
            name: "ColourmotionDMXTests",
            dependencies: ["ColourmotionDMX"],
            path: "Tests/ColourmotionDMXTests"
        ),
    ]
)
