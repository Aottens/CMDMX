import AppKit
import SwiftUI

// Console mode (UI-26): native full screen with the Dock and menu bar auto-hidden, and a
// confirmation before the app quits. The choice lives in UserDefaults ("consoleMode").
// The engine is a separate process, so quitting the app never stops DMX output.

private let consoleKey = "consoleMode"

final class AppDelegate: NSObject, NSApplicationDelegate {
    /// Closing the only window quits (and so asks in console mode), like Cmd+Q.
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard UserDefaults.standard.bool(forKey: consoleKey) else { return .terminateNow }
        let alert = NSAlert()
        alert.messageText = "Quit the app?"
        alert.informativeText = "DMX keeps running in the engine."
        alert.addButton(withTitle: "Quit")
        alert.addButton(withTitle: "Cancel")
        // Return must not quit by accident during a show: Cancel is the default button.
        alert.buttons.first?.keyEquivalent = ""
        alert.buttons.last?.keyEquivalent = "\r"
        return alert.runModal() == .alertFirstButtonReturn ? .terminateNow : .terminateCancel
    }
}

/// Sits in front of SwiftUI's own window delegate: asks for the full-screen presentation options
/// and routes "close" through the quit confirmation; everything else is forwarded.
private final class ConsoleWindowDelegate: NSObject, NSWindowDelegate {
    weak var original: NSWindowDelegate?

    override func responds(to selector: Selector!) -> Bool {
        super.responds(to: selector) || (original?.responds(to: selector) ?? false)
    }

    override func forwardingTarget(for selector: Selector!) -> Any? {
        original?.responds(to: selector) == true ? original : nil
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        if UserDefaults.standard.bool(forKey: consoleKey) {
            NSApp.terminate(nil)   // asks via applicationShouldTerminate
            return false
        }
        return original?.windowShouldClose?(sender) ?? true
    }

    func window(_ window: NSWindow, willUseFullScreenPresentationOptions proposed: NSApplication.PresentationOptions)
        -> NSApplication.PresentationOptions {
        [.fullScreen, .autoHideDock, .autoHideMenuBar]
    }
}

/// Invisible view that finds the main window and makes it follow the console flag.
struct WindowConfigurator: NSViewRepresentable {
    let console: Bool

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeNSView(context: Context) -> NSView {
        let view = WindowObserverView()
        view.onWindow = { [weak coordinator = context.coordinator] window in coordinator?.attach(window, console: console) }
        return view
    }

    func updateNSView(_ view: NSView, context: Context) {
        context.coordinator.console = console
        if let window = view.window { context.coordinator.attach(window, console: console) }
    }

    private final class WindowObserverView: NSView {
        var onWindow: ((NSWindow) -> Void)?
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            if let window { DispatchQueue.main.async { self.onWindow?(window) } }
        }
    }

    @MainActor
    final class Coordinator {
        var console = true
        private let delegate = ConsoleWindowDelegate()
        private var observers: [NSObjectProtocol] = []

        func attach(_ window: NSWindow, console: Bool) {
            self.console = console
            window.collectionBehavior.insert(.fullScreenPrimary)
            if window.delegate !== delegate {
                delegate.original = window.delegate
                window.delegate = delegate
            }
            if observers.isEmpty {
                // Leaving or entering full screen by hand (green button, Ctrl-Cmd-F) flips the flag,
                // so "console on" always means "full screen".
                let center = NotificationCenter.default
                observers = [
                    center.addObserver(forName: NSWindow.didEnterFullScreenNotification, object: window, queue: .main) { _ in
                        UserDefaults.standard.set(true, forKey: consoleKey)
                    },
                    center.addObserver(forName: NSWindow.didExitFullScreenNotification, object: window, queue: .main) { _ in
                        UserDefaults.standard.set(false, forKey: consoleKey)
                    },
                ]
            }
            if console != window.styleMask.contains(.fullScreen) { window.toggleFullScreen(nil) }
        }
    }
}
