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

/// Thin IPC client: WebSocket to cmdmx-daemon (specs/modules/ipc.md, IPC-03..09).
/// Holds no lighting logic; local state is a mirror of the engine's snapshot and deltas.
@MainActor
final class EngineClient: ObservableObject {
    @Published private(set) var connection: EngineConnection = .offline
    /// From the engine's settings snapshot. Never hard-code 8 or 10.
    @Published private(set) var fadersPerPage: Int = 8
    @Published private(set) var pageLegend: String = "Page 1"
    @Published private(set) var slots: [PlaybackSlot] = []
    @Published private(set) var grandMaster: Double = 1.0
    /// Latest `warning` / `error` message from the engine (no UI yet).
    @Published private(set) var lastMessage: String?

    private let url = URL(string: "ws://127.0.0.1:7319")
    private var socket: URLSessionWebSocketTask?

    init() {
        rebuildSlots()
        Task { await connectForever() }
    }

    // MARK: - Commands (app → engine)

    /// `fader.set` — optimistic local update; the engine's delta is authoritative.
    func setFader(index: Int, level: Double) {
        guard slots.indices.contains(index) else { return }
        slots[index].level = level.clamped01
        slots[index].isActive = level > 0
        send(FaderSet(playback: index + 1, level: level.clamped01))
    }

    /// `grand_master.set`
    func setGrandMaster(_ level: Double) {
        grandMaster = level.clamped01
        send(GrandMasterSet(level: level.clamped01))
    }

    private struct FaderSet: Encodable {
        let v = 0
        let cmd = "fader.set"
        let playback: Int
        let level: Double
    }

    private struct GrandMasterSet: Encodable {
        let v = 0
        let cmd = "grand_master.set"
        let level: Double
    }

    /// Fire and forget; a failed send cancels the socket, which triggers the reconnect path.
    private func send(_ command: some Encodable) {
        guard let socket,
              let data = try? JSONEncoder().encode(command),
              let text = String(data: data, encoding: .utf8) else { return }
        socket.send(.string(text)) { error in
            if error != nil { socket.cancel(with: .goingAway, reason: nil) }
        }
    }

    // MARK: - Connection (IPC-08)

    /// One connection attempt after another, 1 s apart, for the life of the app.
    private func connectForever() async {
        while !Task.isCancelled {
            await runOneConnection()
            socket = nil
            connection = .offline
            try? await Task.sleep(nanoseconds: 1_000_000_000)
        }
    }

    /// Returns when the socket fails or closes. No `Origin` header is set (IPC-02).
    private func runOneConnection() async {
        guard let url else { return }
        connection = .connecting
        let task = URLSession.shared.webSocketTask(with: url)
        socket = task
        task.resume()
        do {
            while true {
                handle(try await task.receive())
            }
        } catch {
            task.cancel(with: .goingAway, reason: nil)
        }
    }

    // MARK: - Messages (engine → app)

    /// Union of all server message shapes; unused fields stay nil.
    private struct Incoming: Decodable {
        struct Playback: Decodable {
            let id: Int
            let level: Double
            let active: Bool
        }
        let type: String
        let fadersPerPage: Int?
        let grandMaster: Double?
        let playbacks: [Playback]?
        let id: Int?
        let level: Double?
        let active: Bool?
        let message: String?

        enum CodingKeys: String, CodingKey {
            case type, playbacks, id, level, active, message
            case fadersPerPage = "faders_per_page"
            case grandMaster = "grand_master"
        }
    }

    private func handle(_ message: URLSessionWebSocketTask.Message) {
        guard case .string(let text) = message,
              let incoming = try? JSONDecoder().decode(Incoming.self, from: Data(text.utf8))
        else { return }

        switch incoming.type {
        case "snapshot":
            fadersPerPage = max(1, incoming.fadersPerPage ?? fadersPerPage)
            rebuildSlots()
            for pb in incoming.playbacks ?? [] {
                applyPlayback(id: pb.id, level: pb.level, active: pb.active)
            }
            grandMaster = (incoming.grandMaster ?? grandMaster).clamped01
            connection = .online
        case "playback":
            if let id = incoming.id, let level = incoming.level, let active = incoming.active {
                applyPlayback(id: id, level: level, active: active)
            }
        case "grand_master":
            if let level = incoming.level { grandMaster = level.clamped01 }
        case "warning", "error":
            lastMessage = incoming.message
            print("engine \(incoming.type): \(incoming.message ?? "")")
        default:
            break
        }
    }

    /// Slot n ↔ playback id n+1 (paging is VS1-06).
    private func applyPlayback(id: Int, level: Double, active: Bool) {
        let index = id - 1
        guard slots.indices.contains(index) else { return }
        slots[index].level = level.clamped01
        slots[index].isActive = active
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
