import Foundation

// Codable mirror of the engine's profile JSON (cmdmx-core profile.rs / attribute.rs), complete so a
// load -> save round trip drops nothing (LIB-23, IPC-37/38). Optionals are encoded only when present.
// The helpers at the bottom only reshuffle channel numbers in the document; the engine validates on Save.

struct TargetColourDoc: Codable, Equatable {
    var r: Double
    var g: Double
    var b: Double
}

struct SlotDoc: Codable, Equatable, Identifiable {
    var name: String
    var from: Int
    var to: Int
    var colour: TargetColourDoc?
    /// Editor-only identity (not on the wire), so a selected row survives re-sorting.
    var id = UUID()

    private enum CodingKeys: String, CodingKey { case name, from, to, colour }
}

struct AttributeDoc: Codable, Equatable {
    var name: String
    /// "I" "P" "C" "G" "B" "E" "S" "FX"
    var group: String
    /// "Htp" or "Ltp"
    var kind: String
    var channel: Int
    var fineChannel: Int?
    var defaultLevel: Double
    var locate: Double?
    var highlight: Double?
    var emitter: Bool
    var slots: [SlotDoc]

    private enum CodingKeys: String, CodingKey {
        case name, group, kind, channel, locate, highlight, emitter, slots
        case fineChannel = "fine_channel", defaultLevel = "default"
    }

    init(name: String, group: String, kind: String, channel: Int, fineChannel: Int? = nil, defaultLevel: Double = 0,
         locate: Double? = nil, highlight: Double? = nil, emitter: Bool = false, slots: [SlotDoc] = []) {
        self.name = name; self.group = group; self.kind = kind; self.channel = channel
        self.fineChannel = fineChannel; self.defaultLevel = defaultLevel
        self.locate = locate; self.highlight = highlight; self.emitter = emitter; self.slots = slots
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        name = try c.decode(String.self, forKey: .name)
        group = try c.decode(String.self, forKey: .group)
        kind = try c.decode(String.self, forKey: .kind)
        channel = try c.decode(Int.self, forKey: .channel)
        fineChannel = try c.decodeIfPresent(Int.self, forKey: .fineChannel)
        defaultLevel = try c.decodeIfPresent(Double.self, forKey: .defaultLevel) ?? 0
        locate = try c.decodeIfPresent(Double.self, forKey: .locate)
        highlight = try c.decodeIfPresent(Double.self, forKey: .highlight)
        emitter = try c.decodeIfPresent(Bool.self, forKey: .emitter) ?? false
        slots = try c.decodeIfPresent([SlotDoc].self, forKey: .slots) ?? []
    }
}

struct ComponentDoc: Codable, Equatable {
    var channel: Int
    var fineChannel: Int?
    private enum CodingKeys: String, CodingKey { case channel, fineChannel = "fine_channel" }
}

struct ColourMixingDoc: Codable, Equatable {
    /// "RGB" or "CMY"
    var system: String
    var components: [ComponentDoc]

    /// Red/green/blue or cyan/magenta/yellow, in the engine's component order.
    var roles: [String] { system == "CMY" ? ["Cyan", "Magenta", "Yellow"] : ["Red", "Green", "Blue"] }
}

struct CellDoc: Codable, Equatable {
    var colour: ColourMixingDoc?
    var attributes: [AttributeDoc]

    private enum CodingKeys: String, CodingKey { case colour, attributes }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        colour = try c.decodeIfPresent(ColourMixingDoc.self, forKey: .colour)
        attributes = try c.decodeIfPresent([AttributeDoc].self, forKey: .attributes) ?? []
    }
}

struct ModeDoc: Codable, Equatable {
    var name: String
    var footprint: Int
    var attributes: [AttributeDoc]
    var colour: ColourMixingDoc?
    var virtualDimmer: Bool
    var cells: [CellDoc]
    var panDegrees: Double?
    var tiltDegrees: Double?
    var zoomDegrees: [Double]?

    private enum CodingKeys: String, CodingKey {
        case name, footprint, attributes, colour, cells
        case virtualDimmer = "virtual_dimmer", panDegrees = "pan_degrees", tiltDegrees = "tilt_degrees"
        case zoomDegrees = "zoom_degrees"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        name = try c.decode(String.self, forKey: .name)
        footprint = try c.decode(Int.self, forKey: .footprint)
        attributes = try c.decodeIfPresent([AttributeDoc].self, forKey: .attributes) ?? []
        colour = try c.decodeIfPresent(ColourMixingDoc.self, forKey: .colour)
        virtualDimmer = try c.decodeIfPresent(Bool.self, forKey: .virtualDimmer) ?? false
        cells = try c.decodeIfPresent([CellDoc].self, forKey: .cells) ?? []
        panDegrees = try c.decodeIfPresent(Double.self, forKey: .panDegrees)
        tiltDegrees = try c.decodeIfPresent(Double.self, forKey: .tiltDegrees)
        zoomDegrees = try c.decodeIfPresent([Double].self, forKey: .zoomDegrees)
    }
}

struct ProfileDoc: Codable, Equatable {
    var manufacturer: String
    var model: String
    var source: String
    var modes: [ModeDoc]

    private enum CodingKeys: String, CodingKey { case manufacturer, model, source, modes }

    init(manufacturer: String, model: String, source: String = "", modes: [ModeDoc] = []) {
        self.manufacturer = manufacturer; self.model = model; self.source = source; self.modes = modes
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        manufacturer = try c.decodeIfPresent(String.self, forKey: .manufacturer) ?? ""
        model = try c.decodeIfPresent(String.self, forKey: .model) ?? ""
        source = try c.decodeIfPresent(String.self, forKey: .source) ?? ""
        modes = try c.decode([ModeDoc].self, forKey: .modes)
    }
}

// MARK: - Channel layout of a mode (document bookkeeping only)

/// What sits on one DMX channel of a mode.
struct ChannelEntry {
    enum Ref: Hashable {
        case attr(Int)
        case cellAttr(Int, Int)
        /// Colour mixing component: cell (nil = the mode's own mixing) and component index.
        case mixing(Int?, Int)
    }
    var channel: Int
    var ref: Ref
    var fine: Bool
}

extension ModeDoc {
    /// Every occupied channel (coarse and fine bytes), in channel order.
    var entries: [ChannelEntry] {
        var out: [ChannelEntry] = []
        func add(_ attrs: [AttributeDoc], _ ref: (Int) -> ChannelEntry.Ref) {
            for (i, a) in attrs.enumerated() {
                out.append(ChannelEntry(channel: a.channel, ref: ref(i), fine: false))
                if let f = a.fineChannel { out.append(ChannelEntry(channel: f, ref: ref(i), fine: true)) }
            }
        }
        func addMixing(_ m: ColourMixingDoc?, cell: Int?) {
            for (i, comp) in (m?.components ?? []).enumerated() {
                out.append(ChannelEntry(channel: comp.channel, ref: .mixing(cell, i), fine: false))
                if let f = comp.fineChannel { out.append(ChannelEntry(channel: f, ref: .mixing(cell, i), fine: true)) }
            }
        }
        add(attributes) { .attr($0) }
        addMixing(colour, cell: nil)
        for (ci, cell) in cells.enumerated() {
            add(cell.attributes) { .cellAttr(ci, $0) }
            addMixing(cell.colour, cell: ci)
        }
        return out.enumerated().sorted { ($0.element.channel, $0.offset) < ($1.element.channel, $1.offset) }.map(\.element)
    }

    /// Applies `f` to every channel number in the mode (attributes, fine bytes, mixing components, cells).
    mutating func mapChannels(_ f: (Int) -> Int) {
        func attrs(_ list: inout [AttributeDoc]) {
            for i in list.indices {
                list[i].channel = f(list[i].channel)
                list[i].fineChannel = list[i].fineChannel.map(f)
            }
        }
        func mixing(_ m: inout ColourMixingDoc?) {
            guard var value = m else { return }
            for i in value.components.indices {
                value.components[i].channel = f(value.components[i].channel)
                value.components[i].fineChannel = value.components[i].fineChannel.map(f)
            }
            m = value
        }
        attrs(&attributes)
        mixing(&colour)
        for i in cells.indices {
            attrs(&cells[i].attributes)
            mixing(&cells[i].colour)
        }
    }
}
