import Foundation

/// Connection state to cmdmx-daemon.
enum EngineConnection: Equatable {
    case offline
    case connecting
    case online
}

/// What the UI knows about one slot on the current page. The engine says which playback a slot
/// holds (PBK-03); level and active state mirror its `PlaybackState`. The app never computes these.
struct PlaybackSlot: Identifiable, Equatable {
    let id: Int          // 0-based slot index on the page
    var playback: Int?   // nil: an empty slot (PBK-05)
    var level: Double    // 0.0...1.0, as last reported by the engine
    var isActive: Bool
    /// UI-22: the legend is the playback number; names and halos come with P1-04.
    var legend: String { playback.map(String.init) ?? "" }
}

/// The current page of playbacks (PBK-03): snapshot object or `page` delta (flattened).
/// Fields missing from a message fall back to a single empty page.
struct PageState: Decodable, Equatable {
    struct Slot: Decodable, Equatable {
        let slot: Int          // 1-based
        let playback: Int?
    }

    var number = 1
    var count = 1
    var slots: [Slot] = []

    init() {}

    private enum CodingKeys: String, CodingKey { case number, count, slots }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        number = (try? c.decode(Int.self, forKey: .number)) ?? 1
        count = (try? c.decode(Int.self, forKey: .count)) ?? 1
        slots = (try? c.decode([Slot].self, forKey: .slots)) ?? []
    }
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
    /// Attribute name → group letter (IPC-21); places programmer values in the I/P/C/B columns.
    let attributes: [String: String]

    private enum CodingKeys: String, CodingKey {
        case id, number, name, profile, mode, universe, address, footprint, handle, attributes
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
        attributes = (try? c.decode([String: String].self, forKey: .attributes)) ?? [:]
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

/// One programmer value (IPC-21): a level 0...1 or an sRGB colour 0...1.
enum ProgrammerValue: Decodable, Equatable {
    case level(Double)
    case colour(r: Double, g: Double, b: Double)

    private struct RGB: Decodable { let r, g, b: Double }

    init(from decoder: Decoder) throws {
        if let level = try? decoder.singleValueContainer().decode(Double.self) {
            self = .level(level)
        } else {
            let c = try RGB(from: decoder)
            self = .colour(r: c.r, g: c.g, b: c.b)
        }
    }
}

/// The programmer values of one fixture, by attribute name (IPC-21).
struct ProgrammerContent: Decodable, Identifiable, Equatable {
    let fixture: Int
    let values: [String: ProgrammerValue]
    var id: Int { fixture }

    private enum CodingKeys: String, CodingKey { case fixture, values }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        fixture = try c.decode(Int.self, forKey: .fixture)
        let raw = (try? c.decode([String: Tolerant<ProgrammerValue>].self, forKey: .values)) ?? [:]
        values = raw.compactMapValues(\.value)
    }
}

/// The first target's colour as HSI hue and saturation plus sRGB, all 0...1 (IPC-22).
struct TargetColour: Decodable, Equatable {
    let hue: Double
    let saturation: Double
    let r: Double
    let g: Double
    let b: Double
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
    /// IPC-21: programmer values per fixture; IPC-22: the first target's colour, nil without one.
    var contents: [ProgrammerContent] = []
    var colour: TargetColour?

    init() {}

    enum CodingKeys: String, CodingKey {
        case selection, targets, wheels, contents, colour
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
        contents = ((try? c.decode([Tolerant<ProgrammerContent>].self, forKey: .contents)) ?? []).compactMap(\.value)
        colour = try? c.decode(TargetColour.self, forKey: .colour)
    }
}

/// `programmer.wheel` resolution (IPC-13); raw values are the wire strings.
enum WheelResolution: String {
    case normal = "Normal", fine = "Fine", coarse = "Coarse"
}

/// ISO-8601 UTC as the daemon writes it, with or without fractional seconds (SHW-15).
private func parseISODate(_ text: String) -> Date? {
    (try? Date(text, strategy: .iso8601)) ?? (try? Date(text, strategy: Date.ISO8601FormatStyle(includingFractionalSeconds: true)))
}

/// The open show (SHW-15): name, last successful autosave and whether autosave works. An empty
/// name means no snapshot has arrived yet.
struct ShowState: Decodable, Equatable {
    var name = ""
    var savedAt: Date?
    var autosaveOk = true

    init() {}

    private enum CodingKeys: String, CodingKey {
        case name
        case savedAt = "saved_at"
        case autosaveOk = "autosave_ok"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        name = (try? c.decode(String.self, forKey: .name)) ?? ""
        savedAt = (try? c.decode(String.self, forKey: .savedAt)).flatMap(parseISODate)
        autosaveOk = (try? c.decode(Bool.self, forKey: .autosaveOk)) ?? true
    }

    /// SHW-16: "not saved yet", "saved 4 s ago", above 59 s "saved 2 min ago".
    func ageText(now: Date) -> String {
        guard let savedAt else { return "not saved yet" }
        let s = max(0, Int(now.timeIntervalSince(savedAt)))
        return s < 60 ? "saved \(s) s ago" : "saved \(s / 60) min ago"
    }
}

/// One saved show in the shows folder (SHW-15 `show.list`).
struct ShowEntry: Decodable, Identifiable, Equatable {
    let name: String
    let modified: Date?
    var id: String { name }

    private enum CodingKeys: String, CodingKey { case name, modified }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        name = try c.decode(String.self, forKey: .name)
        modified = (try? c.decode(String.self, forKey: .modified)).flatMap(parseISODate)
    }
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
    /// PBK-03: the current page and the playback each slot holds; `slots` is derived from it.
    @Published private(set) var page = PageState()
    @Published private(set) var slots: [PlaybackSlot] = []
    /// Level and active state per playback id, so a page switch shows the right values at once.
    private var playbackStates: [Int: (level: Double, active: Bool)] = [:]
    @Published private(set) var grandMaster: Double = 1.0
    /// Latest `warning` / `error` message from the engine; cleared when a
    /// `programmer.command` is sent (IPC-17).
    @Published private(set) var lastMessage: String?
    /// IPC-10..12: all straight from the snapshot / deltas.
    @Published private(set) var fixtures: [FixtureInfo] = []
    @Published private(set) var groups: [GroupInfo] = []
    /// Reply to the last `library.search` (this client only).
    @Published private(set) var libraryResults: [LibraryItem] = []
    /// Last query sent; a reply that arrives after the field was emptied is ignored.
    private var lastLibraryQuery = ""
    @Published private(set) var programmer = ProgrammerState()
    /// IPC-20: one entry per output node; empty while offline (the state is unknown then).
    @Published private(set) var outputs: [OutputStatus] = []
    /// SHW-15: the open show; keeps the last known value while offline. `shows` is the reply to the
    /// last `show.list` (this client only).
    @Published private(set) var show = ShowState()
    @Published private(set) var shows: [ShowEntry] = []

    /// UI-22 / IPC-05 "touch": playback ids and the GM held by this client's pointer. While held,
    /// engine levels are parked here (not shown) so the bar follows the finger. Release is delayed by
    /// `releaseGrace` so the engine's echo of the final value lands first; then the parked level is shown.
    private var heldFaders: [Int: Double?] = [:]
    private var gmHeld = false
    private var gmEngineLevel: Double?
    private var releaseTasks: [Int: Task<Void, Never>] = [:]  // key 0 = GM, else playback id
    private let releaseGrace: UInt64 = 150_000_000

    private let url = URL(string: "ws://127.0.0.1:7319")
    private var socket: URLSessionWebSocketTask?

    init() {
        rebuildSlots()
        Task { await connectForever() }
    }

    // MARK: - Commands (app → engine)

    /// `page.set` (PBK-03). No local update: the engine's `page` delta changes the strips.
    func setPage(_ number: Int) { sendJSON("page.set", ["page": number]) }

    /// `fader.set` for the playback a slot holds (PBK-03) — optimistic local update; the engine's
    /// delta is authoritative.
    func setFader(playback id: Int, level: Double) {
        playbackStates[id] = (level.clamped01, level > 0)
        rebuildSlots()
        send(FaderSet(playback: id, level: level.clamped01))
    }

    /// UI-22: a scroll step is a short touch, so the engine's echo of the previous step cannot
    /// pull the bar back while the wheel is still turning.
    func nudgeFader(playback id: Int, by delta: Double) {
        touchFader(playback: id, held: true)
        setFader(playback: id, level: (playbackStates[id]?.level ?? 0) + delta)
        touchFader(playback: id, held: false)
    }

    /// UI-22 / IPC-05: pointer down/up on a fader. Up starts the grace period; down cancels it.
    func touchFader(playback id: Int, held: Bool) {
        releaseTasks.removeValue(forKey: id)?.cancel()
        if held {
            if heldFaders[id] == nil { heldFaders[id] = .some(nil) }
            return
        }
        releaseTasks[id] = Task {
            try? await Task.sleep(nanoseconds: releaseGrace)
            guard !Task.isCancelled else { return }
            releaseTasks[id] = nil
            guard let parked = heldFaders.removeValue(forKey: id) else { return }
            if let level = parked, playbackStates[id] != nil {
                playbackStates[id]?.level = level
                rebuildSlots()
            }
        }
    }

    /// Same for the Grand Master.
    func touchGrandMaster(held: Bool) {
        releaseTasks.removeValue(forKey: 0)?.cancel()
        if held { gmHeld = true; return }
        releaseTasks[0] = Task {
            try? await Task.sleep(nanoseconds: releaseGrace)
            guard !Task.isCancelled else { return }
            releaseTasks[0] = nil
            gmHeld = false
            if let level = gmEngineLevel { grandMaster = level }
            gmEngineLevel = nil
        }
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
    /// IPC-23: hue and saturation 0...1 under the pointer; the engine maps them to every colour target.
    func setColour(hue: Double, saturation: Double) {
        sendJSON("programmer.colour", ["hue": hue.clamped01, "saturation": saturation.clamped01])
    }
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

    func searchLibrary(_ query: String) {
        lastLibraryQuery = query
        if query.trimmingCharacters(in: .whitespaces).isEmpty { libraryResults = []; return }
        sendJSON("library.search", ["query": query])
    }

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

    // Show commands (SHW-15). The engine checks names and refuses with a warning.

    func listShows() { sendJSON("show.list") }
    func newShow(_ name: String) { patch("show.new", ["name": name]) }
    func openShow(_ name: String) { patch("show.open", ["name": name]) }
    func saveShowAs(_ name: String) { patch("show.save_as", ["name": name]) }

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
        fileprivate let page: Tolerant<PageState>?
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
        fileprivate let show: Tolerant<ShowState>?
        fileprivate let shows: Tolerant<[Tolerant<ShowEntry>]>?

        enum CodingKeys: String, CodingKey {
            case type, page, playbacks, id, level, active, message, fixtures, groups, programmer, items, outputs, show, shows
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
            releaseTasks.values.forEach { $0.cancel() }; releaseTasks = [:]
            heldFaders = [:]; gmHeld = false; gmEngineLevel = nil
            fadersPerPage = max(1, incoming.fadersPerPage ?? fadersPerPage)
            page = incoming.page?.value ?? PageState()
            playbackStates = [:]
            for pb in incoming.playbacks ?? [] {
                storePlayback(id: pb.id, level: pb.level, active: pb.active)
            }
            rebuildSlots()
            grandMaster = (incoming.grandMaster ?? grandMaster).clamped01
            fixtures = incoming.fixtures ?? []
            groups = incoming.groups ?? []
            programmer = incoming.programmer ?? ProgrammerState()
            outputs = incoming.outputs?.value ?? []
            if let show = incoming.show?.value { self.show = show }
            connection = .online
            listShows()
        case "page":
            // PBK-03: the delta carries the page fields at top level.
            if let state = try? JSONDecoder().decode(PageState.self, from: data) { page = state; rebuildSlots() }
        case "programmer":
            // IPC-12: the delta carries the IPC-11 fields at top level.
            if let state = try? JSONDecoder().decode(ProgrammerState.self, from: data) { programmer = state }
        case "fixtures":
            if let fixtures = incoming.fixtures { self.fixtures = fixtures }
        case "outputs":
            if let outputs = incoming.outputs?.value { self.outputs = outputs }
        case "show":
            // SHW-15: the delta carries the show fields at top level. A switch or save changes the
            // list too, so ask again.
            if let state = try? JSONDecoder().decode(ShowState.self, from: data) { show = state }
            listShows()
        case "show.list":
            if let shows = incoming.shows?.value { self.shows = shows.compactMap(\.value) }
        case "library.results":
            if !lastLibraryQuery.trimmingCharacters(in: .whitespaces).isEmpty { libraryResults = incoming.items ?? [] }
        case "groups":
            if let groups = incoming.groups { self.groups = groups }
        case "playback":
            if let id = incoming.id, let level = incoming.level, let active = incoming.active {
                storePlayback(id: id, level: level, active: active)
                rebuildSlots()
            }
        case "grand_master":
            if let level = incoming.level {
                if gmHeld { gmEngineLevel = level.clamped01 } else { grandMaster = level.clamped01 }
            }
        case "warning", "error":
            lastMessage = incoming.message
            print("engine \(incoming.type): \(incoming.message ?? "")")
        default:
            break
        }
    }

    /// Keeps the engine's level and active state per playback id. A held fader parks the level (UI-22).
    private func storePlayback(id: Int, level: Double, active: Bool) {
        var state = playbackStates[id] ?? (level: 0, active: false)
        state.active = active
        if heldFaders[id] != nil { heldFaders[id] = .some(level.clamped01) } else { state.level = level.clamped01 }
        playbackStates[id] = state
    }

    /// PBK-01/05: the strips show the current page's slots; the engine decides which playback each holds.
    private func rebuildSlots() {
        let byNumber = Dictionary(page.slots.map { ($0.slot, $0.playback) }, uniquingKeysWith: { first, _ in first })
        slots = (0..<fadersPerPage).map { index in
            let playback = byNumber[index + 1].flatMap { $0 }
            let state = playback.flatMap { playbackStates[$0] }
            return PlaybackSlot(id: index, playback: playback, level: state?.level ?? 0, isActive: state?.active ?? false)
        }
    }
}

extension Double {
    var clamped01: Double { Swift.min(1, Swift.max(0, self)) }
}
