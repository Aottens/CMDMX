import AppKit
import SwiftUI

/// Entry point. The app is a thin client: it renders what the engine reports
/// and sends commands back. No lighting logic lives here (see
/// subprojects/app/INTERFACE.md and CLAUDE.md).
@main
struct ColourmotionDMXApp: App {
    @StateObject private var engine = EngineClient()

    init() {
        // `swift run` launches us without an app bundle; make sure we still get
        // a menu bar, a Dock icon and keyboard focus.
        NSApplication.shared.setActivationPolicy(.regular)
        NSApplication.shared.activate(ignoringOtherApps: true)
    }

    var body: some Scene {
        WindowGroup("ColourmotionDMX") {
            ContentView()
                .environmentObject(engine)
                .frame(minWidth: 960, minHeight: 600)
        }
        .windowResizability(.contentMinSize)
    }
}
