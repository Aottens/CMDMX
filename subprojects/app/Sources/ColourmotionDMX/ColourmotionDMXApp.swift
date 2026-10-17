import AppKit
import SwiftUI

/// Entry point. The app is a thin client: it renders what the engine reports
/// and sends commands back. No lighting logic lives here (see
/// subprojects/app/INTERFACE.md and CLAUDE.md).
@main
struct ColourmotionDMXApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @StateObject private var engine = EngineClient()

    init() {
        // Console mode is on by default (UI-26); the theme starts dark (UI-02).
        UserDefaults.standard.register(defaults: ["consoleMode": true, "dayTheme": false, "keypadVisible": true])
        // `swift run` launches us without an app bundle; make sure we still get
        // a menu bar, a Dock icon and keyboard focus.
        NSApplication.shared.setActivationPolicy(.regular)
        NSApplication.shared.activate(ignoringOtherApps: true)
    }

    var body: some Scene {
        // One window only: the main screen.
        Window("ColourmotionDMX", id: "main") {
            MainScreen()
                .environmentObject(engine)
                .frame(minWidth: 1280, minHeight: 800)
        }
        .defaultSize(width: 1440, height: 900)
        .windowResizability(.contentMinSize)
    }
}
