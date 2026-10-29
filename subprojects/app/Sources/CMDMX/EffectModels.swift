import Foundation

// Wire mirrors of the effect state (FX-32, FX-33, FX-39). Decoding is tolerant per field, so an
// older daemon (no `effects`, no `runnable_looks`) leaves the Shapes workspace empty, not broken.

/// `{"Wave":"Square"}`, `{"Path":"Circle"}` or `{"Colour":"Rainbow"}`.
struct EffectShape: Codable, Hashable {
    enum Kind: String { case wave = "Wave", path = "Path", colour = "Colour" }
    let kind: Kind
    let name: String

    init(_ kind: Kind, _ name: String) { self.kind = kind; self.name = name }

    init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        let raw = try c.decode([String: String].self)
        guard let (key, name) = raw.first, let kind = Kind(rawValue: key) else {
            throw DecodingError.dataCorruptedError(in: c, debugDescription: "not a shape")
        }
        self.init(kind, name)
    }

    func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        try c.encode([kind.rawValue: name])
    }

    /// The command field form.
    var json: [String: String] { [kind.rawValue: name] }

    /// Display name: "RampUp" → "Ramp up".
    var title: String { Self.spaced(name) }

    static func spaced(_ text: String) -> String {
        var out = ""
        for ch in text {
            if ch.isUppercase, !out.isEmpty { out += " " + ch.lowercased() } else { out.append(ch) }
        }
        return out
    }
}

/// FX-32: one look of the gallery. `id` is 1-based (the `runnable_looks` ids).
struct EffectLook: Decodable, Identifiable, Equatable {
    let id: Int
    let name: String
    let group: String
    let shape: EffectShape
    let attribute: String?
}

/// FX-32: the look gallery and the raw shapes per kind; constant, so only in the snapshot.
struct EffectCatalogue: Decodable, Equatable {
    var looks: [EffectLook] = []
    var wave: [String] = []
    var path: [String] = []
    var colour: [String] = []

    init() {}

    private enum CodingKeys: String, CodingKey { case looks, shapes }
    private struct Shapes: Decodable { let wave, path, colour: [String] }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        looks = ((try? c.decode([Tolerant<EffectLook>].self, forKey: .looks)) ?? []).compactMap(\.value)
        if let s = try? c.decode(Shapes.self, forKey: .shapes) { wave = s.wave; path = s.path; colour = s.colour }
    }

    /// FX-39: the attribute group of an entry: its look's, else a look with the same shape, else by kind.
    func group(look: String?, shape: EffectShape) -> String {
        if let look, let l = looks.first(where: { $0.name == look }) { return l.group }
        if let l = looks.first(where: { $0.shape == shape }) { return l.group }
        switch shape.kind { case .wave: return "I"; case .path: return "P"; case .colour: return "C" }
    }
}

/// A colour of a `ColourMix` list, as the engine stores it (`{r, g, b}`, 0...1).
struct EffectColour: Codable, Equatable {
    var r: Double
    var g: Double
    var b: Double
    var json: [String: Double] { ["r": r, "g": g, "b": b] }
}

/// FX-06 / FX-09: where the tempo comes from.
enum EffectSpeedSource: Equatable {
    case free
    case bpm(Int)
    case rate(Int)

    /// The `effect.set` value.
    var json: Any {
        switch self {
        case .free: return "Free"
        case .bpm(let n): return ["Bpm": n]
        case .rate(let n): return ["Rate": n]
        }
    }
}

/// FX-06: the parameters of one entry; a missing field keeps the engine's default.
struct EffectParams: Decodable, Equatable {
    var size = 0.5, top = 1.0, bpm = 120.0, beats = 1.0
    var speedSource = EffectSpeedSource.free
    var spread = 0.0, phase = 0.0, width = 0.5, crossfade = 0.0
    /// `Forward`, `Backward` or `Bounce`.
    var direction = "Forward"
    var wings = 1, groups = 0, blocks = 1
    /// `Relative` or `Absolute`.
    var mode = "Relative"

    init() {}

    private enum CodingKeys: String, CodingKey {
        case size, top, bpm, beats, spread, phase, width, crossfade, direction, wings, groups, blocks, mode
        case speedSource = "speed_source"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        func num(_ k: CodingKeys, _ d: Double) -> Double { (try? c.decode(Double.self, forKey: k)) ?? d }
        func int(_ k: CodingKeys, _ d: Int) -> Int { (try? c.decode(Int.self, forKey: k)) ?? d }
        size = num(.size, size); top = num(.top, top); bpm = num(.bpm, bpm); beats = num(.beats, beats)
        spread = num(.spread, spread); phase = num(.phase, phase); width = num(.width, width)
        crossfade = num(.crossfade, crossfade)
        direction = (try? c.decode(String.self, forKey: .direction)) ?? direction
        mode = (try? c.decode(String.self, forKey: .mode)) ?? mode
        wings = int(.wings, wings); groups = int(.groups, groups); blocks = int(.blocks, blocks)
        if let text = try? c.decode(String.self, forKey: .speedSource), text == "Free" {
            speedSource = .free
        } else if let one = try? c.decode([String: Int].self, forKey: .speedSource), let (key, n) = one.first {
            speedSource = key == "Bpm" ? .bpm(n) : key == "Rate" ? .rate(n) : .free
        }
    }
}

/// FX-33: one programmer effect entry as the engine reports it.
struct EffectEntry: Decodable, Identifiable, Equatable {
    let id: Int
    let look: String?
    let shape: EffectShape
    let attributes: [String]
    let members: [Item]
    /// FX-07: k/N per member, 0...1.
    let positions: [Double]
    let params: EffectParams
    let palette: Int?
    let current: Bool
    /// FX-38: the `ColourMix` list, empty for other shapes.
    let colours: [EffectColour]
    /// FX-42: a Hue shift on a white base shows nothing.
    let whiteBase: Bool

    private enum CodingKeys: String, CodingKey {
        case id, look, shape, attributes, members, positions, params, palette, current, colours
        case whiteBase = "white_base"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(Int.self, forKey: .id)
        shape = try c.decode(EffectShape.self, forKey: .shape)
        look = try? c.decode(String.self, forKey: .look)
        attributes = (try? c.decode([String].self, forKey: .attributes)) ?? []
        members = (try? c.decode([Item].self, forKey: .members)) ?? []
        positions = (try? c.decode([Double].self, forKey: .positions)) ?? []
        params = (try? c.decode(EffectParams.self, forKey: .params)) ?? EffectParams()
        palette = try? c.decode(Int.self, forKey: .palette)
        current = (try? c.decode(Bool.self, forKey: .current)) ?? false
        colours = (try? c.decode([EffectColour].self, forKey: .colours)) ?? []
        whiteBase = (try? c.decode(Bool.self, forKey: .whiteBase)) ?? false
    }
}

/// FX-39: the effect a palette holds, for its pool tile.
struct PaletteEffect: Decodable, Equatable {
    let look: String?
    let shape: EffectShape

    private enum CodingKeys: String, CodingKey { case look, shape }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        look = try? c.decode(String.self, forKey: .look)
        shape = try c.decode(EffectShape.self, forKey: .shape)
    }
}
