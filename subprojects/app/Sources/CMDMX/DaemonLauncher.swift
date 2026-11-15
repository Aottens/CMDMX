import AppKit
import Combine
import Darwin
import Foundation

/// REL-10..13: starts the daemon bundled in the app (if any), adopts one that is still running
/// after an app crash, restarts it when it dies (adopted too, REL-24), backing off when it keeps failing
/// (REL-28), says when it is back (REL-25..27), and stops it on quit.
/// No lighting logic here.
@MainActor
final class DaemonLauncher: ObservableObject {
    static let shared = DaemonLauncher()

    /// Shown in the status bar next to the engine state; nil when all is well.
    @Published private(set) var note: String?
    /// REL-30: one line for the Messages sheet when an engine ends without the app asking.
    let stopLine = PassthroughSubject<String, Never>()
    /// REL-30: size of daemon.log when the running engine started or was adopted.
    private var logOffset: UInt64 = 0

    private let daemonURL = Bundle.main.bundleURL.appendingPathComponent("Contents/MacOS/cmdmx-daemon")
    private let fm = FileManager.default
    private var child: Process?
    private var childExited: DispatchSemaphore?
    private var adopted: pid_t?
    private var adoptedWatch: DispatchSourceProcess?
    private var stopping = false
    /// REL-25, 26: set when a restart is pending; the first snapshot after it says the show is back.
    private var restarting = false
    /// REL-28: failures in a row, and when the running child started; 60 s of running starts the count again.
    private var failures = 0
    private var startedAt: Date?
    private static let waits: [Double] = [2, 4, 8, 16, 30]
    private static let otherEngine = "Another engine is running on port 7319"
    /// True while this app started or adopted the engine, so quitting the app stops it (REL-13).
    /// A flag, not `child != nil`: during the restart window `child` is nil but the app still owns it.
    private(set) var ownsEngine = false

    private var logDir: URL { fm.homeDirectoryForCurrentUser.appendingPathComponent("Library/Logs/CMDMX") }
    private var logURL: URL { logDir.appendingPathComponent("daemon.log") }

    /// REL-30: show daemon.log in Finder; the folder when there is no log yet.
    func revealLog() {
        if fm.fileExists(atPath: logURL.path) {
            NSWorkspace.shared.activateFileViewerSelecting([logURL])
        } else if fm.fileExists(atPath: logDir.path) {
            NSWorkspace.shared.open(logDir)
        }
    }

    private func logSize() -> UInt64 {
        ((try? fm.attributesOfItem(atPath: logURL.path))?[.size] as? NSNumber)?.uint64Value ?? 0
    }

    /// REL-30: what was added since the engine started, as one line; nil when nothing was added.
    private func stopSummary() -> String? {
        guard let h = try? FileHandle(forReadingFrom: logURL) else { return nil }
        defer { try? h.close() }
        guard let end = try? h.seekToEnd() else { return nil }
        let from = end < logOffset ? 0 : max(logOffset, end > 262_144 ? end - 262_144 : 0)   // rotated: from 0; cap 256 KB
        guard (try? h.seek(toOffset: from)) != nil, let data = try? h.readToEnd() else { return nil }
        let lines = String(decoding: data, as: UTF8.self).split(whereSeparator: \.isNewline)
            .map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
        // Rust's panic hook puts the message on the line after `panicked at <file>:<line>:<col>:`; it joins them.
        if let i = lines.firstIndex(where: { $0.contains("panicked at") }) {
            let next = lines[safe: i + 1].flatMap { lines[i].hasSuffix(":") ? " \($0)" : nil } ?? ""
            return String((lines[i] + next).prefix(200))
        }
        return lines.last.map { String($0.prefix(200)) }
    }

    private func reportStop(code: Int32?) {
        let c = code.map { "code \($0)" } ?? "code unknown"
        // ponytail: nothing added to the log gives the code alone.
        stopLine.send("Engine stopped (\(c))" + (stopSummary().map { ": \($0)" } ?? ""))
    }
    private var pidFile: URL {
        fm.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support/CMDMX/daemon.pid")
    }

    /// REL-10: only a bundled daemon is started; `swift run` and a debug bundle start nothing.
    func start() {
        guard fm.isExecutableFile(atPath: daemonURL.path) else { return }
        ownsEngine = true
        if let pid = adoptablePid() { adopted = pid; logOffset = logSize(); watch(pid); return }
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

    /// REL-24: an adopted engine is not our child, so watch its pid instead of a termination handler.
    private func watch(_ pid: pid_t) {
        let src = DispatchSource.makeProcessSource(identifier: pid, eventMask: .exit, queue: .main)
        src.setEventHandler { [weak self] in MainActor.assumeIsolated { self?.adoptedExited() } }
        src.resume()
        adoptedWatch = src
        // Died between the adopt check and the watch: kqueue may never report it.
        if kill(pid, 0) != 0 { adoptedExited() }
    }

    private func adoptedExited() {
        guard adopted != nil else { return }
        adopted = nil
        adoptedWatch?.cancel()
        adoptedWatch = nil
        guard !stopping else { return }
        reportStop(code: nil)
        failed("Engine exited, restarting", code: nil)
    }

    private func launch() {
        let log = openLog()
        logOffset = logSize()
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
            failed("Could not start the engine: \(error.localizedDescription)", code: nil)
            return
        }
        startedAt = Date()   // REL-26: the note stays until the first snapshot (engineConnected)
        child = p
        childExited = done
        try? fm.createDirectory(at: pidFile.deletingLastPathComponent(), withIntermediateDirectories: true)
        try? "\(p.processIdentifier)\n".write(to: pidFile, atomically: true, encoding: .utf8)
    }

    /// REL-12: append to daemon.log; a log over 10 MB is moved to daemon.log.1 at app launch.
    private func openLog() -> FileHandle? {
        try? fm.createDirectory(at: logDir, withIntermediateDirectories: true)
        let url = logURL
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
            restarting = false
            note = Self.otherEngine
            return
        }
        if let t = startedAt, Date().timeIntervalSince(t) >= 60 { failures = 0 }
        reportStop(code: code)
        failed("Engine exited (code \(code)), restarting", code: code)
    }

    /// REL-28: waits 2, 4, 8, 16, then 30 s; from the third failure in a row one steady line, so the sheet gets one.
    private func failed(_ text: String, code: Int32?) {
        restarting = true
        failures += 1
        note = failures >= 3
            ? "Engine does not start\(code.map { " (code \($0))" } ?? ""), trying every 30 s. Log: ~/Library/Logs/CMDMX/daemon.log"
            : text
        let wait = Self.waits[min(failures, Self.waits.count) - 1]
        DispatchQueue.main.asyncAfter(deadline: .now() + wait) { [weak self] in
            guard let self, !self.stopping, self.child == nil else { return }
            self.launch()
        }
    }

    /// REL-26: called at each snapshot; only the first one after a restart says the show is back.
    /// Does not reset `failures`: only 60 s of running does (REL-28).
    func engineConnected(show: String?) {
        guard restarting else { return }
        restarting = false
        note = "Engine restarted: \(show.flatMap { $0.isEmpty ? nil : $0 } ?? "an empty show") is back, nothing is running"
    }

    /// REL-27: the "back" line ends at the operator's first command.
    func commandGiven() {
        if note?.hasPrefix("Engine restarted") == true { note = nil }
    }

    /// REL-27: the other-engine line stays as long as it is so, so it goes when the connection drops.
    func engineLost() {
        if note == Self.otherEngine { note = nil }
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
