import Foundation

// The `control` section of the snapshot and its delta (CTL-39), and the reply to `control.mapping`
// (CTL-40). Every field falls back to a default, so an odd entry cannot drop the message it came in.

/// One key of a Stream Deck as the engine draws it (CTL-57); the app only paints it.
struct DeckKeyLook: Decodable, Equatable {
    enum State { case off, inactive, active, held }

    let key: Int
    let label: String
    let sub: String
    /// `#rrggbb` or nil.
    let halo: String?
    let level: Double?
    let state: State
    /// The key's action does not follow the console page (CTL-50): drawn with a lock.
    let fixed: Bool

    private enum CodingKeys: String, CodingKey { case key, label, sub, halo, level, state, fixed }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        key = (try? c.decode(Int.self, forKey: .key)) ?? 0
        label = (try? c.decode(String.self, forKey: .label)) ?? ""
        sub = (try? c.decode(String.self, forKey: .sub)) ?? ""
        halo = try? c.decode(String.self, forKey: .halo)
        fixed = (try? c.decode(Bool.self, forKey: .fixed)) ?? false
        level = try? c.decode(Double.self, forKey: .level)
        switch try? c.decode(String.self, forKey: .state) {
        case "inactive": state = .inactive
        case "active": state = .active
        case "held": state = .held
        default: state = .off
        }
    }
}

struct ControlDevice: Decodable, Identifiable, Equatable {
    enum State {
        case ok, missing, disabled, busy, unsupported

        var word: String {
            switch self {
            case .ok: return "Ok"
            case .missing: return "Missing"
            case .disabled: return "Disabled"
            case .busy: return "Busy"
            case .unsupported: return "Unsupported"
            }
        }
    }

    let name: String
    let input: Bool
    let output: Bool
    let state: State
    let enabled: Bool
    let mapping: String?
    let errors: Int
    /// "midi" or "streamdeck"; the rest is a deck's (CTL-57).
    let kind: String
    let model: String?
    let cols: Int
    let rows: Int
    let deckPage: Int
    let deckPages: Int
    let brightness: Int
    let keys: [DeckKeyLook]
    var id: String { name }
    var isDeck: Bool { kind == "streamdeck" }

    private enum CodingKeys: String, CodingKey {
        case name, input, output, state, enabled, mapping, errors, kind, model, cols, rows, brightness, keys
        case deckPage = "deck_page"
        case deckPages = "deck_pages"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        name = (try? c.decode(String.self, forKey: .name)) ?? "?"
        input = (try? c.decode(Bool.self, forKey: .input)) ?? false
        output = (try? c.decode(Bool.self, forKey: .output)) ?? false
        switch try? c.decode(String.self, forKey: .state) {
        case "Ok": state = .ok
        case "Disabled": state = .disabled
        case "Busy": state = .busy
        case "Unsupported": state = .unsupported
        default: state = .missing   // "Missing" or something newer: show it as a warning
        }
        enabled = (try? c.decode(Bool.self, forKey: .enabled)) ?? false
        mapping = try? c.decode(String.self, forKey: .mapping)
        errors = (try? c.decode(Int.self, forKey: .errors)) ?? 0
        kind = (try? c.decode(String.self, forKey: .kind)) ?? "midi"
        model = try? c.decode(String.self, forKey: .model)
        cols = (try? c.decode(Int.self, forKey: .cols)) ?? 0
        rows = (try? c.decode(Int.self, forKey: .rows)) ?? 0
        deckPage = (try? c.decode(Int.self, forKey: .deckPage)) ?? 1
        deckPages = (try? c.decode(Int.self, forKey: .deckPages)) ?? 1
        brightness = (try? c.decode(Int.self, forKey: .brightness)) ?? 100
        keys = ((try? c.decode([Tolerant<DeckKeyLook>].self, forKey: .keys)) ?? []).compactMap(\.value)
    }
}

struct ControlMappingInfo: Decodable, Equatable {
    let name: String
    let profile: Bool

    private enum CodingKeys: String, CodingKey { case name, profile }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        name = (try? c.decode(String.self, forKey: .name)) ?? "?"
        profile = (try? c.decode(Bool.self, forKey: .profile)) ?? false
    }
}

/// The last input seen, `value` already as text: a number, on/off, or an OSC string.
struct ControlLastInput: Decodable, Equatable {
    let device: String
    let input: String
    let value: String

    private enum CodingKeys: String, CodingKey { case device, input, value }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        device = (try? c.decode(String.self, forKey: .device)) ?? "?"
        input = (try? c.decode(String.self, forKey: .input)) ?? "?"
        if let n = try? c.decode(Double.self, forKey: .value) {
            value = n == n.rounded() ? "\(Int(n))" : String(format: "%.2f", n)
        } else if let b = try? c.decode(Bool.self, forKey: .value) {
            value = b ? "on" : "off"
        } else {
            value = (try? c.decode(String.self, forKey: .value)) ?? ""
        }
    }
}

struct OSCClientInfo: Decodable, Identifiable, Equatable {
    let address: String
    /// Daemon seconds of the last message; nil for a configured client not heard.
    let lastHeard: Double?
    let configured: Bool
    var id: String { address }

    private enum CodingKeys: String, CodingKey {
        case address, configured
        case lastHeard = "last_heard"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        address = (try? c.decode(String.self, forKey: .address)) ?? "?"
        lastHeard = try? c.decode(Double.self, forKey: .lastHeard)
        configured = (try? c.decode(Bool.self, forKey: .configured)) ?? false
    }
}

struct OSCState: Decodable, Equatable {
    var enabled = false
    var port = 8000
    var feedbackPort = 9000
    var allow: [String] = []
    var clients: [OSCClientInfo] = []
    var mapping: String?
    var errors = 0
    var error: String?

    private enum CodingKeys: String, CodingKey {
        case enabled, port, allow, clients, mapping, errors, error
        case feedbackPort = "feedback_port"
    }

    init() {}

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        enabled = (try? c.decode(Bool.self, forKey: .enabled)) ?? false
        port = (try? c.decode(Int.self, forKey: .port)) ?? 8000
        feedbackPort = (try? c.decode(Int.self, forKey: .feedbackPort)) ?? 9000
        allow = (try? c.decode([String].self, forKey: .allow)) ?? []
        clients = ((try? c.decode([Tolerant<OSCClientInfo>].self, forKey: .clients)) ?? []).compactMap(\.value)
        mapping = try? c.decode(String.self, forKey: .mapping)
        errors = (try? c.decode(Int.self, forKey: .errors)) ?? 0
        error = try? c.decode(String.self, forKey: .error)
    }
}

struct ControlState: Decodable, Equatable {
    var devices: [ControlDevice] = []
    var mappings: [ControlMappingInfo] = []
    var learn: String?
    var lastInput: ControlLastInput?
    var osc = OSCState()
    var order: [String] = []

    private enum CodingKeys: String, CodingKey {
        case devices, mappings, learn, osc, order
        case lastInput = "last_input"
    }

    init() {}

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        devices = ((try? c.decode([Tolerant<ControlDevice>].self, forKey: .devices)) ?? []).compactMap(\.value)
        mappings = ((try? c.decode([Tolerant<ControlMappingInfo>].self, forKey: .mappings)) ?? []).compactMap(\.value)
        learn = try? c.decode(String.self, forKey: .learn)
        lastInput = try? c.decode(ControlLastInput.self, forKey: .lastInput)
        osc = (try? c.decode(OSCState.self, forKey: .osc)) ?? OSCState()
        order = (try? c.decode([String].self, forKey: .order)) ?? []
    }

    /// The device rows as the unit order shows them: listed names first, then by name (CTL-31).
    var orderedDevices: [ControlDevice] {
        let sorted = devices.sorted { $0.name < $1.name }
        let listed = order.compactMap { n in sorted.first { $0.name == n } }
        return listed + sorted.filter { d in !order.contains(d.name) }
    }
}

/// One binding of the shown mapping. `input` stays the daemon's own JSON, so it goes back
/// unchanged in `control.binding` and `control.unbind`.
struct ControlBinding: Identifiable {
    /// The binding's own JSON, kept whole so a key can be moved or swapped with all its options.
    let raw: [String: Any]
    let input: Any
    /// A deck key's page (CTL-48); 1 otherwise.
    let deckPage: Int
    private(set) var isDeck = false
    /// The key number of a deck `key` input (CTL-58).
    private(set) var deckKey: Int?
    let inputText: String
    let action: String
    let min: Double
    let max: Double
    let invert: Bool
    let buttonMode: String
    let encoding: String?
    let pickup: Bool
    let feedback: Bool
    let onValue: Int
    let offValue: Int
    var id: String { "\(deckPage)|\(inputText)" }

    init(_ json: [String: Any]) {
        raw = json
        deckPage = (json["deck_page"] as? NSNumber)?.intValue ?? 1
        input = json["input"] ?? NSNull()
        if let address = json["input"] as? String {
            inputText = address
        } else if let i = json["input"] as? [String: Any] {
            let kind = i["kind"] as? String ?? "?", channel = i["channel"] as? Int ?? 0, number = i["number"] as? Int
            let deck = ["key": "Key", "touch": "Touch", "dial": "Dial", "dial_push": "Dial push", "strip": "Strip"][kind]
            let label = ["note": "Note", "cc": "CC", "cc14": "CC14", "pitchbend": "Pitch bend", "program": "Program"][kind] ?? kind
            if let deck {
                inputText = [deck, number.map(String.init)].compactMap { $0 }.joined(separator: " ")
            } else {
                inputText = [label, number.map(String.init)].compactMap { $0 }.joined(separator: " ") + " ch \(channel)"
            }
            isDeck = deck != nil
            if kind == "key" { deckKey = number }
        } else {
            inputText = "?"
        }
        action = json["action"] as? String ?? ""
        min = (json["min"] as? NSNumber)?.doubleValue ?? 0
        max = (json["max"] as? NSNumber)?.doubleValue ?? 1
        invert = json["invert"] as? Bool ?? false
        buttonMode = json["button_mode"] as? String ?? "momentary"
        encoding = json["encoding"] as? String
        pickup = json["pickup"] as? Bool ?? true
        feedback = json["feedback"] as? Bool ?? true
        onValue = (json["on_value"] as? NSNumber)?.intValue ?? 127
        offValue = (json["off_value"] as? NSNumber)?.intValue ?? 0
    }
}

/// The reply to `control.mapping {name}`: the mapping's JSON object, kept whole for Export.
struct ControlMappingDoc {
    let name: String
    let raw: [String: Any]
    let bindings: [ControlBinding]

    init?(_ data: Data) {
        guard let reply = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
              let name = reply["name"] as? String, let mapping = reply["mapping"] as? [String: Any] else { return nil }
        self.name = name
        raw = mapping
        bindings = ((mapping["bindings"] as? [[String: Any]]) ?? []).map(ControlBinding.init)
    }

    /// Pretty JSON for a file.
    var exportData: Data? {
        try? JSONSerialization.data(withJSONObject: raw, options: [.prettyPrinted, .sortedKeys])
    }
}
