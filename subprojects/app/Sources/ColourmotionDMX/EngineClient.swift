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

/// One patched fixture (IPC-10, PAT-09). `id` is the internal id, `number` the user number.
/// Every field but `id` falls back to a default so a malformed entry cannot drop a snapshot.
struct FixtureInfo: Decodable, Identifiable, Equatable {
    struct Handle: Decodable, Equatable {
        let page: Int
        let slot: Int
    }

    let id: Int
    let number: Int
    let name: String
    let profile: String
    let mode: String
    let universe: Int
    let address: Int
    let footprint: Int
    let virtualDimmer: Bool
    let handle: Handle?

    private enum CodingKeys: String, CodingKey {
        case id, number, name, profile, mode, universe, address, footprint, handle
        case virtualDimmer = "virtual_dimmer"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(Int.self, forKey: .id)
        number = (try? c.decode(Int.self, forKey: .number)) ?? id
        name = (try? c.decode(String.self, forKey: .name)) ?? ""
        profile = (try? c.decode(String.self, forKey: .profile)) ?? ""
        mode = (try? c.decode(String.self, forKey: .mode)) ?? ""
        universe = (try? c.decode(Int.self, forKey: .universe)) ?? 0
        address = (try? c.decode(Int.self, forKey: .address)) ?? 0
        footprint = (try? c.decode(Int.self, forKey: .footprint)) ?? 0
        virtualDimmer = (try? c.decode(Bool.self, forKey: .virtualDimmer)) ?? false
        handle = try? c.decode(Handle.self, forKey: .handle)
    }
}

/// One fixture type found by `library.search` (LIB-14). The patch profile key is `manufacturer/model`.
struct LibraryItem: Decodable, Identifiable, Equatable {
    let manufacturer: String
    let model: String
    let modes: [String]
    let source: String
    var id: String { "\(source)/\(manufacturer)/\(model)" }
    var profileKey: String { "\(manufacturer)/\(model)" }

    private enum CodingKeys: String, CodingKey { case manufacturer, model, modes, source }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        manufacturer = try c.decode(String.self, forKey: .manufacturer)
        model = try c.decode(String.self, forKey: .model)
        modes = (try? c.decode([String].self, forKey: .modes)) ?? []
        source = (try? c.decode(String.self, forKey: .source)) ?? ""
    }
}

/// One output node as reported by the daemon (IPC-20). Every field falls back to a default,
/// so an odd entry cannot drop the snapshot it came in.
struct OutputStatus: Decodable, Identifiable, Equatable {
    enum State { case ok, degraded, stopped }

    let name: String
    let state: State
    let detail: String?
    var id: String { name }

    private enum CodingKeys: String, CodingKey { case name, state, detail }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        name = (try? c.decode(String.self, forKey: .name)) ?? "?"
        switch try? c.decode(String.self, forKey: .state) {
        case "Ok": state = .ok
        case "Stopped": state = .stopped
        default: state = .degraded   // "Degraded" or something newer: show it as a warning
        }
        detail = try? c.decode(String.self, forKey: .detail)
    }
}

/// Decodes to nil instead of throwing, for fields that must never drop a whole message.
private struct Tolerant<T: Decodable>: Decodable {
    let value: T?
    init(from decoder: Decoder) throws { value = try? T(from: decoder) }
}

/// One recorded group; `fixtures` are in group order (IPC-10).
struct GroupInfo: Decodable, Identifiable, Equatable {
    let id: Int
    let fixtures: [Int]
}

/// One attribute-group tab I P C G B E S FX (IPC-11).
struct AttributeGroupState: Decodable, Identifiable, Equatable {
    let group: String
    let available: Bool
    let hasValues: Bool
    var id: String { group }

    enum CodingKeys: String, CodingKey {
        case group, available
        case hasValues = "has_values"
    }
}

/// One wheel of the current layout; `value` is 0.0...1.0 (IPC-11).
struct WheelState: Decodable, Identifiable, Equatable {
    let attribute: String
    let value: Double
    let touched: Bool
    let sixteenBit: Bool
    var id: String { attribute }

    private enum CodingKeys: String, CodingKey {
        case attribute, value, touched
        case sixteenBit = "sixteen_bit"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        attribute = try c.decode(String.self, forKey: .attribute)
        value = try c.decode(Double.self, forKey: .value)
        touched = try c.decode(Bool.self, forKey: .touched)
        sixteenBit = (try? c.decode(Bool.self, forKey: .sixteenBit)) ?? false
    }
}

/// Mirror of the engine's programmer state (IPC-11). Fields missing from a
/// message fall back to empty so a malformed message cannot drop a snapshot.
struct ProgrammerState: Decodable, Equatable {
    var selection: [Int] = []
    var targets: [Int] = []
    var activeGroup: String = ""
    var attributeGroups: [AttributeGroupState] = []
    var wheels: [WheelState] = []
    var inProgrammer: [Int] = []
    /// `Hsi`, `Rgb` or `Cmy` (IPC-11, PRG-30).
    var colourModel: String = "Hsi"

    init() {}

    enum CodingKeys: String, CodingKey {
        case selection, targets, wheels
        case activeGroup = "active_group"
        case attributeGroups = "attribute_groups"
        case inProgrammer = "in_programmer"
        case colourModel = "colour_model"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        selection = (try? c.decode([Int].self, forKey: .selection)) ?? []
        targets = (try? c.decode([Int].self, forKey: .targets)) ?? []
        activeGroup = (try? c.decode(String.self, forKey: .activeGroup)) ?? ""
        attributeGroups = (try? c.decode([AttributeGroupState].self, forKey: .attributeGroups)) ?? []
        wheels = (try? c.decode([WheelState].self, forKey: .wheels)) ?? []
        inProgrammer = (try? c.decode([Int].self, forKey: .inProgrammer)) ?? []
        colourModel = (try? c.decode(String.self, forKey: .colourModel)) ?? "Hsi"
    }
}

/// `programmer.wheel` resolution (IPC-13); raw values are the wire strings.
enum WheelResolution: String {
    case normal = "Normal", fine = "Fine", coarse = "Coarse"
}

/// Thin IPC client: WebSocket to cmdmx-daemon (specs/modules/ipc.md, IPC-03..09).
/// Holds no lighting logic; local state is a mirror of the engine's snapshot and deltas.
@MainActor
final class EngineClient: ObservableObject {
    @Published private(set) var connection: EngineConnection = .offline
    /// Attributes a Fine step was sent on since the app started (IPC-16: one decimal).
    @Published private(set) var fineUsed: Set<String> = []
    /// From the engine's settings snapshot. Never hard-code 8 or 10.
    @Published private(set) var fadersPerPage: Int = 8
    @Published private(set) var pageLegend: String = "Page 1"
    @Published private(set) var slots: [PlaybackSlot] = []
    @Published private(set) var grandMaster: Double = 1.0
    /// Latest `warning` / `error` message from the engine; cleared when a
    /// `programmer.command` is sent (IPC-17).
    @Published private(set) var lastMessage: String?
    /// IPC-10..12: all straight from the snapshot / deltas.
    @Published private(set) var fixtures: [FixtureInfo] = []
    @Published private(set) var groups: [GroupInfo] = []
    /// Reply to the last `library.search` (this client only).
    @Published private(set) var libraryResults: [LibraryItem] = []
    @Published private(set) var programmer = ProgrammerState()
    /// IPC-20: one entry per output node; empty while offline (the state is unknown then).
    @Published private(set) var outputs: [OutputStatus] = []

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

    // Programmer commands (IPC-13). One-liners; the engine decides what they mean.

    func selectFixture(_ id: Int) { sendJSON("programmer.select", ["fixture": id]) }
    func selectGroup(_ id: Int) { sendJSON("programmer.select_group", ["group": id]) }
    func step(_ direction: Int) { sendJSON("programmer.step", ["direction": direction]) }
    func pattern(_ stages: Int) { sendJSON("programmer.pattern", ["stages": stages]) }
    func selectAll() { sendJSON("programmer.all") }
    func clearProgrammer() { sendJSON("programmer.clear") }
    func setAttributeGroup(_ group: String) { sendJSON("programmer.attribute_group", ["group": group]) }
    func setColourModel(_ model: String) { sendJSON("programmer.colour_model", ["model": model]) }
    func recordGroup(_ id: Int) { sendJSON("group.record", ["group": id]) }

    func wheel(attribute: String, steps: Int, resolution: WheelResolution) {
        guard steps != 0 else { return }
        if resolution == .fine { fineUsed.insert(attribute) }
        sendJSON("programmer.wheel", ["attribute": attribute, "steps": steps, "resolution": resolution.rawValue])
    }

    /// Sends the words plus `Enter` and clears the previous message (IPC-17).
    func command(words: [String]) {
        lastMessage = nil
        sendJSON("programmer.command", ["tokens": words + ["Enter"]])
    }

    // Patch commands (PAT-09, LIB-14). All take internal ids; the engine checks and may refuse
    // with a warning/error, which lands in `lastMessage`.

    func searchLibrary(_ query: String) { sendJSON("library.search", ["query": query]) }

    /// `address` / `number` nil = omitted, the engine picks (PAT-03).
    func patchAdd(profile: String, mode: String, count: Int, universe: Int, address: Int?, number: Int?) {
        var fields: [String: Any] = ["profile": profile, "mode": mode, "count": count, "universe": universe]
        if let address { fields["address"] = address }
        if let number { fields["number"] = number }
        patch("patch.add", fields)
    }
    func patchMove(id: Int, universe: Int, address: Int) { patch("patch.move", ["id": id, "universe": universe, "address": address]) }
    func patchRemove(ids: [Int]) { patch("patch.remove", ["ids": ids]) }
    func patchRenumber(id: Int, number: Int) { patch("patch.renumber", ["id": id, "number": number]) }
    func patchName(id: Int, name: String) { patch("patch.name", ["id": id, "name": name]) }
    func patchChangeMode(id: Int, mode: String) { patch("patch.change_mode", ["id": id, "mode": mode]) }

    /// Hides the last engine message (e.g. when the operator selects another row).
    func clearMessage() { lastMessage = nil }

    private func patch(_ cmd: String, _ fields: [String: Any]) {
        lastMessage = nil
        sendJSON(cmd, fields)
    }

    private func sendJSON(_ cmd: String, _ fields: [String: Any] = [:]) {
        guard let socket,
              let data = try? JSONSerialization.data(withJSONObject: fields.merging(["v": 0, "cmd": cmd]) { a, _ in a }),
              let text = String(data: data, encoding: .utf8) else { return }
        socket.send(.string(text)) { error in
            if error != nil { socket.cancel(with: .goingAway, reason: nil) }
        }
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
            outputs = []
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
        let fixtures: [FixtureInfo]?
        let groups: [GroupInfo]?
        let programmer: ProgrammerState?
        let items: [LibraryItem]?
        fileprivate let outputs: Tolerant<[OutputStatus]>?

        enum CodingKeys: String, CodingKey {
            case type, playbacks, id, level, active, message, fixtures, groups, programmer, items, outputs
            case fadersPerPage = "faders_per_page"
            case grandMaster = "grand_master"
        }
    }

    private func handle(_ message: URLSessionWebSocketTask.Message) {
        guard case .string(let text) = message,
              case let data = Data(text.utf8),
              let incoming = try? JSONDecoder().decode(Incoming.self, from: data)
        else { return }

        switch incoming.type {
        case "snapshot":
            fadersPerPage = max(1, incoming.fadersPerPage ?? fadersPerPage)
            rebuildSlots()
            for pb in incoming.playbacks ?? [] {
                applyPlayback(id: pb.id, level: pb.level, active: pb.active)
            }
            grandMaster = (incoming.grandMaster ?? grandMaster).clamped01
            fixtures = incoming.fixtures ?? []
            groups = incoming.groups ?? []
            programmer = incoming.programmer ?? ProgrammerState()
            outputs = incoming.outputs?.value ?? []
            connection = .online
        case "programmer":
            // IPC-12: the delta carries the IPC-11 fields at top level.
            if let state = try? JSONDecoder().decode(ProgrammerState.self, from: data) { programmer = state }
        case "fixtures":
            if let fixtures = incoming.fixtures { self.fixtures = fixtures }
        case "outputs":
            if let outputs = incoming.outputs?.value { self.outputs = outputs }
        case "library.results":
            libraryResults = incoming.items ?? []
        case "groups":
            if let groups = incoming.groups { self.groups = groups }
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
