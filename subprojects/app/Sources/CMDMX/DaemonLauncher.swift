import AppKit
import Darwin
import Foundation

/// REL-10..13: starts the daemon bundled in the app (if any), adopts one that is still running
/// after an app crash, restarts it when it dies, and stops it on quit. No lighting logic here.
@MainActor
final class DaemonLauncher: ObservableObject {
    static let shared = DaemonLauncher()

    /// Shown in the status bar next to the engine state; nil when all is well.
    @Published private(set) var note: String?

    private let daemonURL = Bundle.main.bundleURL.appendingPathComponent("Contents/MacOS/cmdmx-daemon")
    private let fm = FileManager.default
    private var child: Process?
    private var childExited: DispatchSemaphore?
    private var adopted: pid_t?
    private var stopping = false
    /// True while this app started or adopted the engine, so quitting the app stops it (REL-13).
    /// A flag, not `child != nil`: during the restart window `child` is nil but the app still owns it.
    private(set) var ownsEngine = false

    private var logDir: URL { fm.homeDirectoryForCurrentUser.appendingPathComponent("Library/Logs/CMDMX") }
    private var pidFile: URL {
        fm.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support/CMDMX/daemon.pid")
    }

    /// REL-10: only a bundled daemon is started; `swift run` and a debug bundle start nothing.
    func start() {
        guard fm.isExecutableFile(atPath: daemonURL.path) else { return }
        ownsEngine = true
        if let pid = adoptablePid() { adopted = pid; return }
        launch()
    }

    /// REL-11: the pid file names a live process whose executable is this bundle's daemon.
    private func adoptablePid() -> pid_t? {
        guard let text = try? String(contentsOf: pidFile, encoding: .utf8),
              let pid = pid_t(text.trimmingCharacters(in: .whitespacesAndNewlines)), pid > 1,
              kill(pid, 0) == 0 else { return nil }
        var buf = [CChar](repeating: 0, count: 4 * Int(MAXPATHLEN))
        guard proc_pidpath(pid, &buf, UInt32(buf.count)) > 0 else { return nil }
        return String(cString: buf) == daemonURL.path ? pid : nil
    }

    private func launch() {
        let log = openLog()
        let p = Process()
        p.executableURL = daemonURL
        p.arguments = ["--qlcplus-dir", Bundle.main.bundleURL
            .appendingPathComponent("Contents/Resources/qlcplus/fixtures").path]
        p.standardOutput = log
        p.standardError = log
        let done = DispatchSemaphore(value: 0)
        p.terminationHandler = { proc in
            done.signal()
            let code = proc.terminationStatus
            Task { @MainActor in DaemonLauncher.shared.exited(code: code) }
        }
        do {
            try p.run()
        } catch {
            note = "Could not start the engine: \(error.localizedDescription)"
            retryLater()
            return
        }
        note = nil
        child = p
        childExited = done
        try? fm.createDirectory(at: pidFile.deletingLastPathComponent(), withIntermediateDirectories: true)
        try? "\(p.processIdentifier)\n".write(to: pidFile, atomically: true, encoding: .utf8)
    }

    /// REL-12: append to daemon.log; a log over 10 MB is moved to daemon.log.1 at app launch.
    private func openLog() -> FileHandle? {
        try? fm.createDirectory(at: logDir, withIntermediateDirectories: true)
        let url = logDir.appendingPathComponent("daemon.log")
        if child == nil, adopted == nil,
           let size = (try? fm.attributesOfItem(atPath: url.path))?[.size] as? Int, size > 10_000_000 {
            let old = logDir.appendingPathComponent("daemon.log.1")
            try? fm.removeItem(at: old)
            try? fm.moveItem(at: url, to: old)
        }
        if !fm.fileExists(atPath: url.path) { fm.createFile(atPath: url.path, contents: nil) }
        guard let h = try? FileHandle(forWritingTo: url) else { return nil }
        _ = try? h.seekToEnd()
        return h
    }

    private func exited(code: Int32) {
        child = nil
        guard !stopping else { return }
        if code == 2 {
            ownsEngine = false
            // REL-11: the port is taken by another program; the app connects to that one.
            note = "Another engine is running on port 7319"
            return
        }
        note = "Engine exited (code \(code)), restarting"
        retryLater()
    }

    private func retryLater() {
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) { [weak self] in
            guard let self, !self.stopping, self.child == nil else { return }
            self.launch()
        }
    }

    /// REL-13: SIGTERM, wait up to 5 s, then SIGKILL; remove the pid file. Leaves a developer's daemon alone.
    func stop() {
        stopping = true
        let pid = child?.processIdentifier ?? adopted
        guard let pid else { return }
        kill(pid, SIGTERM)
        if let done = childExited, child != nil {
            if done.wait(timeout: .now() + 5) == .timedOut { kill(pid, SIGKILL); _ = done.wait(timeout: .now() + 2) }
        } else {
            for _ in 0..<100 where kill(pid, 0) == 0 { usleep(50_000) }
            if kill(pid, 0) == 0 { kill(pid, SIGKILL) }
        }
        try? fm.removeItem(at: pidFile)
    }
}
