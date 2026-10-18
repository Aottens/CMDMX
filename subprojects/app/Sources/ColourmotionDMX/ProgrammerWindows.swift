import AppKit
import SwiftUI

// The Programmer table (UI-14) and the Colour picker (UI-15). Pure rendering of the engine's
// programmer state (IPC-21, IPC-22) plus command sending: the picker sends the hue and saturation
// under the pointer (IPC-23) and wheel steps (IPC-13), and draws what the engine reports.

/// A window's one-line note on an empty state (the sketch's .note).
struct NoteText: View {
    @Environment(\.theme) private var t
    let note: String

    var body: some View {
        ScrollView {
            Text(note).font(Typo.ui()).foregroundStyle(t.fg2)
                .frame(maxWidth: 330, alignment: .leading).padding(8)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

// MARK: - Programmer (UI-14)

struct ProgrammerWindow: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient

    private static let columns = ["I", "P", "C", "B"]

    var body: some View {
        let rows = engine.programmer.contents
        let fixtures = Dictionary(engine.fixtures.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })

        PanelWindow(title: "Programmer", info: rows.isEmpty ? "empty" : "\(rows.count) fixtures", accessory: { EmptyView() }) {
            if rows.isEmpty {
                NoteText(note: "Only attributes you touch land here, in red. Values from a palette show the palette name in cyan. Locate and Highlight never land here.")
            } else {
                ScrollView {
                    LazyVStack(spacing: 0, pinnedViews: .sectionHeaders) {
                        Section {
                            ForEach(rows) { row in
                                let fixture = fixtures[row.fixture]
                                line(number: Text("\(fixture?.number ?? row.fixture)").foregroundStyle(t.fg2),
                                     name: Text(fixture.map(Self.title) ?? "").foregroundStyle(t.fg)) { group in
                                    cell(row, fixture, group)
                                }
                                HLine()
                            }
                        } header: {
                            VStack(spacing: 0) {
                                line(number: header("#"), name: header("Fixture")) { header($0) }
                                HLine()
                            }
                            .background(t.bg)
                        }
                    }
                }
            }
        }
    }

    private func header(_ s: String) -> some View {
        Text(s.uppercased()).font(Typo.legend(Typo.small)).tracking(0.8).foregroundStyle(t.fg3)
    }

    /// One table line: # and Fixture, then the I, P, C and B columns.
    private func line(number: some View, name: some View, column: @escaping (String) -> some View) -> some View {
        HStack(alignment: .top, spacing: 0) {
            number.frame(width: 34, alignment: .leading)
            name.lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
            ForEach(Self.columns, id: \.self) { group in
                column(group).frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .font(Typo.mono(11))
        .padding(.horizontal, 7).padding(.vertical, 3)
    }

    /// The fixture's name, else the model part of `manufacturer/model`.
    private static func title(_ f: FixtureInfo) -> String {
        if !f.name.isEmpty { return f.name }
        return f.profile.split(separator: "/").last.map(String.init) ?? f.profile
    }

    /// The values of one row that the fixture's `attributes` map places in `group`; "·" if none.
    @ViewBuilder private func cell(_ row: ProgrammerContent, _ fixture: FixtureInfo?, _ group: String) -> some View {
        let mine = row.values.filter { fixture?.attributes[$0.key] == group }.sorted { $0.key < $1.key }
        if mine.isEmpty {
            Text("·").foregroundStyle(t.fg3)
        } else {
            // Red = touched by hand; cyan + the palette's legend = referenced from a palette (PAL-30).
            let items = mine.map { name, value -> Text in
                let text: Text
                switch value {
                case .level(let v):
                    text = Text(mine.count > 1 ? "\(name) \(percent(v))" : percent(v))
                case .colour(let r, let g, let b):
                    text = Text("\(Text(Image(systemName: "square.fill")).font(.system(size: 8)).foregroundStyle(Color(red: r, green: g, blue: b))) \(Self.hex(r, g, b))")
                }
                guard let id = row.palettes[name] else { return text.foregroundStyle(t.red) }
                let legend = engine.palettes.first { $0.id == id }?.shownLegend ?? "palette"
                let target = Text("\(text) \(legend)").foregroundStyle(t.cyan)
                // A fade still running keeps its target and adds an amber bar filling up (PAL-30).
                guard let progress = row.fading[name] else { return target }
                let filled = Int((progress.clamped01 * 5).rounded(.down))
                let bar = String(repeating: "▰", count: filled) + String(repeating: "▱", count: 5 - filled)
                return Text("\(target) \(Text(bar).foregroundStyle(t.amber))")
            }
            items.dropFirst().reduce(items[0]) { Text("\($0) · \($1)") }
        }
    }

    /// #RRGGBB from sRGB 0...1; formatting only.
    static func hex(_ r: Double, _ g: Double, _ b: Double) -> String {
        func byte(_ v: Double) -> Int { Int((v.clamped01 * 255).rounded()) }
        return String(format: "#%02X%02X%02X", byte(r), byte(g), byte(b))
    }
}

// MARK: - Colour picker (UI-15)

struct ColourPickerWindow: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient

    var body: some View {
        let programmer = engine.programmer
        PanelWindow(title: "Colour picker",
                    info: programmer.colour == nil ? "select colour fixtures" : programmer.colourModel.uppercased(),
                    accessory: { EmptyView() }) {
            if let colour = programmer.colour {
                HStack(spacing: 6) {
                    HueSaturationField(colour: colour)
                    ScrollView {
                        VStack(alignment: .leading, spacing: 3) {
                            if programmer.activeGroup == "C" {
                                ForEach(programmer.wheels) { MixBar(wheel: $0) }
                            } else {
                                Text("pick the C tab for the bars").font(Typo.ui(11)).foregroundStyle(t.fg3)
                            }
                            gelRow
                        }
                    }
                    .frame(width: 150)
                }
                .padding(6)
            } else {
                NoteText(note: "Select fixtures that mix colour. The picker sets one target colour; each fixture type maps it to its own emitters (RGBW, RGB+Lime, RGB). Gel book: LEE first.")
            }
        }
    }

    /// Placeholders until the gel book (IDEA-02): 8 disabled swatches.
    private var gelRow: some View {
        VStack(alignment: .leading, spacing: 3) {
            Text("GELS · LATER").font(Typo.legend(Typo.small)).tracking(0.8).foregroundStyle(t.fg3)
            gelSwatches
        }
        .padding(.top, 4)
        .help("Gel book · IDEA-02")
    }

    private var gelSwatches: some View {
        LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 2), count: 4), spacing: 2) {
            ForEach(0..<8, id: \.self) { _ in
                RoundedRectangle(cornerRadius: radius).fill(t.surf)
                    .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(t.line, lineWidth: hairline))
                    .frame(height: 28).opacity(t.disabled)
            }
        }
    }
}

/// The hue/saturation field of the sketch's drawPicker(): hue left to right, white fading in
/// towards the bottom. The marker sits at the colour the engine reports (IPC-22).
private struct HueSaturationField: View {
    let colour: TargetColour
    @EnvironmentObject private var engine: EngineClient
    @State private var lastSend = Date.distantPast

    var body: some View {
        GeometryReader { geo in
            ZStack {
                LinearGradient(colors: (0...6).map { Color(hue: Double($0) / 6, saturation: 1, brightness: 1) },
                               startPoint: .leading, endPoint: .trailing)
                LinearGradient(colors: [.white.opacity(0), .white], startPoint: .top, endPoint: .bottom)
                ZStack {
                    Circle().strokeBorder(.black, lineWidth: 3)
                    Circle().strokeBorder(.white, lineWidth: 1.5).padding(0.75)
                }
                .frame(width: 18, height: 18)
                .position(x: colour.hue.clamped01 * geo.size.width, y: (1 - colour.saturation.clamped01) * geo.size.height)
            }
            .clipShape(RoundedRectangle(cornerRadius: radius))
            .contentShape(Rectangle())
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { send($0.location, in: geo.size, throttled: true) }
                    .onEnded { send($0.location, in: geo.size, throttled: false) }
            )
        }
        .frame(minHeight: 80)
    }

    /// At most one send per 25 ms while dragging; the final position always goes out.
    private func send(_ p: CGPoint, in size: CGSize, throttled: Bool) {
        let now = Date()
        guard size.width > 0, size.height > 0,
              !throttled || now.timeIntervalSince(lastSend) >= 0.025 else { return }
        lastSend = now
        engine.setColour(hue: p.x / size.width, saturation: 1 - p.y / size.height)
    }
}

/// One bar of the picker's mix column: letter, draggable bar, value. Drag steps like a wheel.
private struct MixBar: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    let wheel: WheelState
    @State private var sentSteps = 0

    /// Bar colour is drawing only (the sketch's per-channel hues).
    private var tint: Color {
        switch wheel.attribute {
        case "Red": return t.red
        case "Green": return t.green
        case "Blue": return t.blue
        case "Cyan": return t.cyan
        case "Magenta": return Color(hex: 0xD6409F)
        case "Yellow": return Color(hex: 0xE5C20A)
        case "White": return t.fg
        case "Lime": return Color(hex: 0xA6D608)
        case "Amber": return t.amber
        default: return t.fg2
        }
    }

    var body: some View {
        HStack(spacing: 4) {
            Text(String(wheel.attribute.prefix(1)).uppercased()).frame(width: 14, alignment: .leading)
            GeometryReader { geo in
                ZStack(alignment: .leading) {
                    Rectangle().fill(t.surf)
                    Rectangle().fill(tint).frame(width: geo.size.width * wheel.value.clamped01)
                }
                .overlay(Rectangle().strokeBorder(t.line, lineWidth: hairline))
                .contentShape(Rectangle())
                .gesture(
                    DragGesture(minimumDistance: 0)
                        .onChanged { drag in
                            // Right = positive; same 1 step per 4 pt as the wheels.
                            let total = Int(drag.translation.width / 4)
                            engine.wheel(attribute: wheel.attribute, steps: total - sentSteps,
                                         resolution: resolution(NSEvent.modifierFlags))
                            sentSteps = total
                        }
                        .onEnded { _ in sentSteps = 0 }
                )
            }
            .frame(height: 14)
            Text(wheel.attribute == "Hue" ? String(format: "%.0f", wheel.value * 360) : "\(Int((wheel.value * 100).rounded()))")
                .frame(width: 26, alignment: .trailing)
        }
        .font(Typo.mono(Typo.small))
        .foregroundStyle(t.fg2)
        .background(ScrollCatcher { steps, flags in
            engine.wheel(attribute: wheel.attribute, steps: steps, resolution: resolution(flags))
        })
        .help(wheel.attribute)
    }
}
