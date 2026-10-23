import Foundation

// The `control` section of the snapshot and its delta (CTL-39), and the reply to `control.mapping`
// (CTL-40). Every field falls back to a default, so an odd entry cannot drop the message it came in.

struct ControlDevice: Decodable, Identifiable, Equatable {
    enum State { case ok, missing, disabled }

    let name: String
    let input: Bool
    let output: Bool
    let state: State
    let enabled: Bool
    let mapping: String?
    let errors: Int
    var id: String { name }

    private enum CodingKeys: String, CodingKey { case name, input, output, state, enabled, mapping, errors }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        name = (try? c.decode(String.self, forKey: .name)) ?? "?"
        input = (try? c.decode(Bool.self, forKey: .input)) ?? false
        output = (try? c.decode(Bool.self, forKey: .output)) ?? false
        switch try? c.decode(String.self, forKey: .state) {
        case "Ok": state = .ok
        case "Disabled": state = .disabled
        default: state = .missing   // "Missing" or something newer: show it as a warning
        }
        enabled = (try? c.decode(Bool.self, forKey: .enabled)) ?? false
        mapping = try? c.decode(String.self, forKey: .mapping)
        errors = (try? c.decode(Int.self, forKey: .errors)) ?? 0
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
    let input: Any
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
    var id: String { inputText }

    init(_ json: [String: Any]) {
        input = json["input"] ?? NSNull()
        if let address = json["input"] as? String {
            inputText = address
        } else if let i = json["input"] as? [String: Any] {
            let kind = i["kind"] as? String ?? "?", channel = i["channel"] as? Int ?? 0, number = i["number"] as? Int
            let label = ["note": "Note", "cc": "CC", "cc14": "CC14", "pitchbend": "Pitch bend", "program": "Program"][kind] ?? kind
            inputText = [label, number.map(String.init)].compactMap { $0 }.joined(separator: " ") + " ch \(channel)"
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
