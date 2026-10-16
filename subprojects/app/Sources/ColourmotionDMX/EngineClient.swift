import Foundation

/// Connection state to cmdmx-daemon.
enum EngineConnection: Equatable {
    case offline
    case connecting
    case online
}

/// What the UI knows about one playback slot on the current page.
/// Mirrors the engine's `PlaybackState`; the app never computes these.
struct PlaybackSlot: Identifiable, Equatable {
    let id: Int          // 0-based index on the page
    var legend: String
    var level: Double    // 0.0...1.0, as last reported by the engine
    var isActive: Bool
}

/// Thin IPC client. Scaffold: holds a local model and reports `.offline`.
/// VS1-05 replaces the stubs with the real socket (see specs/modules/ipc.md).
@MainActor
final class EngineClient: ObservableObject {
    @Published private(set) var connection: EngineConnection = .offline
    /// From the engine's settings snapshot. Never hard-code 8 or 10.
    @Published private(set) var fadersPerPage: Int = 8
    @Published private(set) var pageLegend: String = "Page 1"
    @Published private(set) var slots: [PlaybackSlot] = []
    @Published private(set) var grandMaster: Double = 1.0

    init() {
        rebuildSlots()
    }

    // MARK: - Commands (app → engine)

    /// `fader.set` — optimistic local update; the engine's delta is authoritative.
    func setFader(index: Int, level: Double) {
        guard slots.indices.contains(index) else { return }
        slots[index].level = level.clamped01
        slots[index].isActive = level > 0
        // TODO(VS1-05): send {"v":0,"cmd":"fader.set","playback":…,"level":…}
    }

    /// `grand_master.set`
    func setGrandMaster(_ level: Double) {
        grandMaster = level.clamped01
        // TODO(VS1-05): send {"v":0,"cmd":"grand_master.set","level":…}
    }

    // MARK: - Snapshot handling (engine → app)

    /// Applied when a snapshot arrives; also used by tests.
    func apply(fadersPerPage: Int, legend: String) {
        self.fadersPerPage = max(1, fadersPerPage)
        self.pageLegend = legend
        rebuildSlots()
    }

    private func rebuildSlots() {
        slots = (0..<fadersPerPage).map {
            PlaybackSlot(id: $0, legend: "PB \($0 + 1)", level: 0, isActive: false)
        }
    }
}

extension Double {
    var clamped01: Double { Swift.min(1, Swift.max(0, self)) }
}
