import SwiftUI

// The Shapes workspace (FX-36..42): Looks, Effect, Effects. Pure rendering of the engine's effect
// state plus command sending. The curves drawn here are stills for the eye (a tile, the spread
// strip); no output is computed in the app, the engine owns every effect value.

// MARK: - Stills

/// Static pictures of the shapes. Only a drawing, never an output (the engine evaluates, FX-04).
enum ShapePicture {
    /// 0...1 over one cycle `x` in 0...1.
    static func wave(_ name: String, _ x: Double, width: Double = 0.5) -> Double {
        switch name {
        case "Sine": return 0.5 - 0.5 * cos(2 * .pi * x)
        case "Triangle": return x < 0.5 ? 2 * x : 2 - 2 * x
        case "RampUp": return x
        case "RampDown": return 1 - x
        case "Square": return x < width ? 1 : 0
        default:   // Random: a fixed scatter
            let steps = [0.3, 0.8, 0.1, 0.6, 0.95, 0.4, 0.2, 0.7]
            return steps[min(steps.count - 1, max(0, Int(x * Double(steps.count))))]
        }
    }

    static func wavePath(_ name: String, in r: CGRect, width: Double = 0.5) -> Path {
        var p = Path()
        let n = 120
        for i in 0...n {
            let x = Double(i) / Double(n)
            let pt = CGPoint(x: r.minX + x * r.width, y: r.maxY - wave(name, x, width: width) * r.height)
            if i == 0 { p.move(to: pt) } else { p.addLine(to: pt) }
        }
        return p
    }

    /// The outline of a position path, -1...1 on both axes.
    private static func points(_ name: String) -> [CGPoint] {
        switch name {
        case "Circle":
            return (0...48).map { let t = 2 * Double.pi * Double($0) / 48; return CGPoint(x: cos(t), y: sin(t)) }
        case "Eight":
            return (0...64).map { let t = 2 * Double.pi * Double($0) / 64; return CGPoint(x: cos(t), y: sin(2 * t)) }
        case "Square": return [CGPoint(x: -1, y: -1), CGPoint(x: 1, y: -1), CGPoint(x: 1, y: 1), CGPoint(x: -1, y: 1), CGPoint(x: -1, y: -1)]
        case "Triangle": return [CGPoint(x: -1, y: 1), CGPoint(x: 1, y: 1), CGPoint(x: 0, y: -1), CGPoint(x: -1, y: 1)]
        case "LinePan": return [CGPoint(x: -1, y: 0), CGPoint(x: 1, y: 0)]
        default: return [CGPoint(x: 0, y: -1), CGPoint(x: 0, y: 1)]   // LineTilt
        }
    }

    static func outline(_ name: String, in r: CGRect) -> Path {
        let scale = min(r.width, r.height) / 2
        var p = Path()
        for (i, q) in points(name).enumerated() {
            let pt = CGPoint(x: r.midX + q.x * scale, y: r.midY + q.y * scale)
            if i == 0 { p.move(to: pt) } else { p.addLine(to: pt) }
        }
        return p
    }

    static func rainbow(_ steps: Int = 6) -> [Color] {
        (0...steps).map { Color(hue: Double($0) / Double(steps), saturation: 0.7, brightness: 0.9) }
    }

    /// The colours a colour shape is drawn with: a Mix its own list, else a hue ramp.
    static func colours(_ shape: EffectShape, _ list: [EffectColour]) -> [Color] {
        switch shape.name {
        case "ColourMix":
            let own = list.map { Color(red: $0.r, green: $0.g, blue: $0.b) }
            return own.isEmpty ? [Color(hue: 0, saturation: 0.7, brightness: 0.9), Color(hue: 0.55, saturation: 0.7, brightness: 0.9)] : own
        case "HueShift": return (0...3).map { Color(hue: 0.55 + Double($0) * 0.08, saturation: 0.7, brightness: 0.9) }
        default: return rainbow()
        }
    }
}

/// A small still drawing of a shape: a curve for a wave, the outline for a path, a gradient (blocks
/// for Colour mix) for a colour shape.
struct ShapeIcon: View {
    @Environment(\.theme) private var t
    let shape: EffectShape
    var tint: Color?
    var colours: [EffectColour] = []

    var body: some View {
        let ink = tint ?? t.fg
        switch shape.kind {
        case .wave:
            Canvas { ctx, size in
                let r = CGRect(origin: .zero, size: size).insetBy(dx: 2, dy: 3)
                ctx.stroke(ShapePicture.wavePath(shape.name, in: r), with: .color(ink), lineWidth: 1.5)
            }
        case .path:
            Canvas { ctx, size in
                let r = CGRect(origin: .zero, size: size).insetBy(dx: 3, dy: 3)
                ctx.stroke(ShapePicture.outline(shape.name, in: r), with: .color(ink), lineWidth: 1.5)
            }
        case .colour:
            let list = ShapePicture.colours(shape, colours)
            RoundedRectangle(cornerRadius: 2)
                .fill(shape.name == "ColourMix" ? AnyShapeStyle(blocks(list)) : AnyShapeStyle(LinearGradient(colors: list, startPoint: .leading, endPoint: .trailing)))
                .padding(.vertical, 5).padding(.horizontal, 2)
        }
    }

    /// Hard-edged blocks, one per colour.
    private func blocks(_ list: [Color]) -> LinearGradient {
        let n = Double(list.count)
        let stops = list.enumerated().flatMap { i, c in
            [Gradient.Stop(color: c, location: Double(i) / n), Gradient.Stop(color: c, location: Double(i + 1) / n)]
        }
        return LinearGradient(stops: stops, startPoint: .leading, endPoint: .trailing)
    }
}

/// FX-40: "¼" … "16" for the beats values the engine allows.
func beatsLabel(_ beats: Double) -> String {
    switch beats {
    case 0.25: return "¼"
    case 0.5: return "½"
    default: return String(Int(beats.rounded()))
    }
}

/// FX-39 / FX-40: how an entry's speed reads, "120 BPM", "BPM 2" or "Rate 1".
func speedLabel(_ p: EffectParams) -> String {
    switch p.speedSource {
    case .free: return "\(Int(p.bpm.rounded())) BPM"
    case .bpm(let n): return "BPM \(n)"
    case .rate(let n): return "Rate \(n)"
    }
}

private let groupNames = ["I": "Intensity", "P": "Position", "C": "Colour", "G": "Gobo", "B": "Beam", "E": "Effect", "S": "Shape", "FX": "FX"]

// MARK: - Looks (FX-37)

struct LooksWindow: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    /// The Looks | Shapes switch (as Mix | Gels, UI-32): view state only.
    @State private var raw = false

    var body: some View {
        let catalogue = engine.effectCatalogue
        let runnable = Set(engine.programmer.runnableLooks)
        PanelWindow(
            title: "Looks",
            info: raw ? "raw shapes" : "\(runnable.count) of \(catalogue.looks.count) fit",
            accessory: {
                HStack(spacing: 2) {
                    Button("Looks") { raw = false }.buttonStyle(ModeButton(on: !raw))
                    Button("Shapes") { raw = true }.buttonStyle(ModeButton(on: raw))
                }
            }
        ) {
            ScrollView {
                VStack(alignment: .leading, spacing: 8) {
                    if raw {
                        rawRow("Wave", catalogue.wave.map { EffectShape(.wave, $0) }, "I")
                        rawRow("Path", catalogue.path.map { EffectShape(.path, $0) }, "P")
                        rawRow("Colour", catalogue.colour.map { EffectShape(.colour, $0) }, "C")
                    } else {
                        // IPCGBES order; only the groups that have looks.
                        ForEach(attributeGroupOrder.filter { g in catalogue.looks.contains { $0.group == g } }, id: \.self) { group in
                            section(groupNames[group] ?? group, group) {
                                ForEach(catalogue.looks.filter { $0.group == group }) { look in
                                    tile(look.name, look.shape, group, dimmed: !runnable.contains(look.id)) { engine.applyLook(look.name); engine.setEffectWheels(true) }
                                }
                            }
                        }
                    }
                }
                .padding(6)
            }
        }
    }

    private func rawRow(_ title: String, _ shapes: [EffectShape], _ group: String) -> some View {
        section(title, group) {
            // A wave on the raw switch drives Dimmer: no attribute is sent (FX-14).
            ForEach(shapes, id: \.self) { shape in
                tile(shape.title, shape, group, dimmed: false) { engine.applyShape(shape) }
            }
        }
    }

    private func section<C: View>(_ title: String, _ group: String, @ViewBuilder _ content: () -> C) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(title.uppercased()).font(Typo.legend(11, .semibold)).tracking(1.1).foregroundStyle(t.fg2)
            Rectangle().fill(Theme.groupHue(group)).frame(height: 1)
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 84), spacing: 2)], spacing: 2) { content() }
        }
    }

    private func tile(_ name: String, _ shape: EffectShape, _ group: String, dimmed: Bool, action: @escaping () -> Void) -> some View {
        let hue = Theme.groupHue(group)
        return Button(action: action) {
            VStack(spacing: 2) {
                ShapeIcon(shape: shape, tint: hue).frame(height: 30)
                Text(name).font(Typo.legend(11)).foregroundStyle(t.fg).lineLimit(1).minimumScaleFactor(0.7)
            }
            .padding(.horizontal, 4).padding(.top, 5).padding(.bottom, 4)
            .frame(maxWidth: .infinity)
            .background(t.surf)
            .overlay(alignment: .top) { Rectangle().fill(hue).frame(height: 2) }
            .overlay(Rectangle().strokeBorder(t.line, lineWidth: 1))
            .clipShape(RoundedRectangle(cornerRadius: radius))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .opacity(dimmed ? 0.35 : 1)   // FX-31: no target can run it; a tap still gets the engine's reason
    }
}

// MARK: - Controls of the editor

/// A larger variant of `ModeButton` for the editor's chips.
private struct ChipStyle: ButtonStyle {
    @Environment(\.theme) private var t
    let on: Bool
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.font(Typo.mono(11))
            .foregroundStyle(on ? t.bg : t.fg2)
            .padding(.horizontal, 7).frame(minWidth: 26, minHeight: 22)
            .background(RoundedRectangle(cornerRadius: radius).fill(on ? t.fg : Color.clear))
            .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(t.line, lineWidth: hairline))
            .opacity(configuration.isPressed ? 0.7 : 1)
    }
}

private struct Chips<T: Equatable>: View {
    let items: [(String, T)]
    let selected: T?
    let pick: (T) -> Void
    var body: some View {
        HStack(spacing: 2) {
            ForEach(Array(items.enumerated()), id: \.offset) { _, item in
                Button(item.0) { pick(item.1) }.buttonStyle(ChipStyle(on: selected == item.1))
            }
        }
    }
}

/// Title, readout and a slider. Sends on every change (the engine coalesces undo per parameter,
/// FX-34); while dragging, the slider shows its own value so the echo cannot make it jump.
private struct FxSlider: View {
    @Environment(\.theme) private var t
    let title: String
    let value: Double
    let range: ClosedRange<Double>
    let readout: (Double) -> String
    let set: (Double) -> Void
    @State private var drag: Double?

    var body: some View {
        let v = drag ?? value
        VStack(alignment: .leading, spacing: 1) {
            HStack {
                Text(title).font(Typo.legend(Typo.body, .semibold)).foregroundStyle(t.fg2)
                Spacer()
                Text(readout(v)).font(Typo.mono(Typo.body)).foregroundStyle(t.fg)
            }
            Slider(value: Binding(get: { v }, set: { drag = $0; set($0) }), in: range) { editing in
                if !editing { drag = nil }
            }
        }
    }
}

/// A typed number, committed with Return, clamped to `range`.
private struct NumberField: View {
    @Environment(\.theme) private var t
    let value: Double
    let range: ClosedRange<Double>
    let set: (Double) -> Void
    @State private var text = ""
    @FocusState private var focused: Bool

    private func format(_ v: Double) -> String { v.rounded() == v ? String(Int(v)) : String(format: "%.1f", v) }

    var body: some View {
        TextField("", text: $text)
            .textFieldStyle(.plain).font(Typo.mono(16, .medium)).multilineTextAlignment(.trailing)
            .focused($focused)
            .padding(.horizontal, 6).frame(width: 64, height: 26)
            .background(RoundedRectangle(cornerRadius: radius).fill(t.field))
            .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(focused ? t.blue : t.line2, lineWidth: 1))
            .onAppear { text = format(value) }
            .onChange(of: value) { _, v in if !focused { text = format(v) } }
            .onSubmit {
                if let v = Double(text.replacingOccurrences(of: ",", with: ".")) { set(min(range.upperBound, max(range.lowerBound, v))) }
                text = format(value)
            }
    }
}

private struct StepRow: View {
    @Environment(\.theme) private var t
    let title: String
    let text: String
    let dec: () -> Void
    let inc: () -> Void
    var body: some View {
        HStack(spacing: 4) {
            Text(title).font(Typo.legend(Typo.body, .semibold)).foregroundStyle(t.fg2)
            Spacer()
            Button("−", action: dec).buttonStyle(ChipStyle(on: false))
            Text(text).font(Typo.mono(Typo.body)).frame(minWidth: 40)
            Button("+", action: inc).buttonStyle(ChipStyle(on: false))
        }
    }
}

// MARK: - Effect editor (FX-38, FX-42)

struct EffectWindow: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState
    @State private var more = false

    var body: some View {
        let entry = engine.programmer.currentEffect
        PanelWindow(
            title: "Effect",
            info: entry.map { "\($0.members.count) fixtures" } ?? "",
            accessory: {
                if let entry {
                    HStack(spacing: 4) {
                        ShapeIcon(shape: entry.shape, tint: Theme.groupHue(engine.effectCatalogue.group(look: entry.look, shape: entry.shape)), colours: entry.colours)
                            .frame(width: 30, height: 16)
                        Text(entry.look ?? entry.shape.title).font(Typo.legend(Typo.body, .semibold)).foregroundStyle(t.fg)
                    }
                }
            }
        ) {
            if let entry {
                ScrollView { editor(entry).padding(10) }
            } else {
                Text("Tap a look to start an effect").font(Typo.ui(Typo.body)).foregroundStyle(t.fg3)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
    }

    private func editor(_ e: EffectEntry) -> some View {
        let p = e.params
        let mix = e.shape == EffectShape(.colour, "ColourMix")
        let square = e.shape == EffectShape(.wave, "Square")
        return VStack(alignment: .leading, spacing: 12) {
            FxSlider(title: "Size", value: p.size, range: 0...1, readout: percent) { engine.setEffect("size", $0) }
            if e.whiteBase {   // FX-42
                Text("The base colour is white: Hue shift shows nothing").font(Typo.ui(Typo.body)).foregroundStyle(t.amber)
            }
            if e.dark {   // FX-42
                Text("Intensity is 0: this effect shows nothing").font(Typo.ui(Typo.body)).foregroundStyle(t.amber)
            }

            VStack(alignment: .leading, spacing: 4) {   // Speed
                HStack(spacing: 8) {
                    Text("Speed").font(Typo.legend(Typo.body, .semibold)).foregroundStyle(t.fg2)
                    Spacer()
                    NumberField(value: p.bpm, range: 0...600) { engine.setEffect("bpm", $0) }
                    Text("BPM").font(Typo.mono(Typo.small)).foregroundStyle(t.fg2)
                    Button("Tap") { ui.learnOrRun("effect tap", engine) { engine.tapEffect() } }.buttonStyle(FlatButton())
                }
                FxSlider(title: "Tempo", value: p.bpm, range: 0...600, readout: { "\(Int($0.rounded())) BPM" }) { engine.setEffect("bpm", $0) }
                let sources: [(String, EffectSpeedSource)] = [("Free", .free)]
                    + (1...8).map { ("BPM \($0)", .bpm($0)) } + (1...4).map { ("Rate \($0)", .rate($0)) }
                FlowChips(items: sources, selected: p.speedSource) { engine.setEffect("speed_source", $0.json) }
                HStack(spacing: 6) {
                    Text("beats").font(Typo.mono(Typo.small)).foregroundStyle(t.fg2)
                    Chips(items: [0.25, 0.5, 1, 2, 4, 8, 16].map { (beatsLabel($0), $0) }, selected: p.beats) { engine.setEffect("beats", $0) }
                }
            }

            VStack(alignment: .leading, spacing: 4) {   // Spread
                FxSlider(title: "Spread", value: p.spread, range: 0...1440, readout: { "\(Int($0.rounded()))°" }) { engine.setEffect("spread", $0) }
                HStack(spacing: 6) {
                    Chips(items: [0, 90, 180, 360].map { ("\($0)°", Double($0)) }, selected: p.spread) { engine.setEffect("spread", $0) }
                    Spacer()
                    Text("Every n").font(Typo.mono(Typo.small)).foregroundStyle(t.fg2)
                    groupsStepper(p.groups)
                }
                SpreadStrip(entry: e).frame(height: 56)   // FX-42
            }

            Button { more.toggle() } label: {
                Text(more ? "▾ More" : "▸ More").font(Typo.legend(Typo.body, .semibold)).foregroundStyle(t.fg2)
            }
            .buttonStyle(.plain)
            if more { moreControls(e, mix: mix, square: square) }
        }
    }

    /// FX-06: groups is 0 (off) or 2...32.
    private func groupsStepper(_ g: Int) -> some View {
        HStack(spacing: 4) {
            Button("−") { engine.setEffect("groups", g <= 2 ? 0 : g - 1) }.buttonStyle(ChipStyle(on: false))
            Text(g == 0 ? "off" : String(g)).font(Typo.mono(Typo.body)).frame(minWidth: 30)
            Button("+") { engine.setEffect("groups", g == 0 ? 2 : min(32, g + 1)) }.buttonStyle(ChipStyle(on: false))
        }
    }

    /// "No control the shape does not use is shown" (FX-38).
    @ViewBuilder private func moreControls(_ e: EffectEntry, mix: Bool, square: Bool) -> some View {
        let p = e.params
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("Direction").font(Typo.legend(Typo.body, .semibold)).foregroundStyle(t.fg2)
                Spacer()
                Chips(items: [("→", "Forward"), ("←", "Backward"), ("↔", "Bounce")], selected: p.direction) { engine.setEffect("direction", $0) }
            }
            FxSlider(title: "Phase", value: p.phase, range: 0...360, readout: { "\(Int($0.rounded()))°" }) { engine.setEffect("phase", $0) }
            StepRow(title: "Wings", text: String(p.wings), dec: { engine.setEffect("wings", max(1, p.wings - 1)) },
                    inc: { engine.setEffect("wings", min(8, p.wings + 1)) })
            StepRow(title: "Groups", text: p.groups == 0 ? "off" : String(p.groups),
                    dec: { engine.setEffect("groups", p.groups <= 2 ? 0 : p.groups - 1) },
                    inc: { engine.setEffect("groups", p.groups == 0 ? 2 : min(32, p.groups + 1)) })
            StepRow(title: "Blocks", text: String(p.blocks), dec: { engine.setEffect("blocks", max(1, p.blocks - 1)) },
                    inc: { engine.setEffect("blocks", min(32, p.blocks + 1)) })
            HStack {
                Text("Mode").font(Typo.legend(Typo.body, .semibold)).foregroundStyle(t.fg2)
                Spacer()
                Chips(items: [("Rel", "Relative"), ("Abs", "Absolute")], selected: p.mode) { engine.setEffect("mode", $0) }
            }
            if p.mode == "Absolute" {
                FxSlider(title: "Top", value: p.top, range: 0...1, readout: percent) { engine.setEffect("top", $0) }
            }
            if square {
                FxSlider(title: "Width", value: p.width, range: 0.01...1, readout: percent) { engine.setEffect("width", $0) }
            }
            if mix {
                FxSlider(title: "Crossfade", value: p.crossfade, range: 0...1, readout: percent) { engine.setEffect("crossfade", $0) }
                colourList(e.colours)
            }
        }
    }

    /// FX-38: the ColourMix list (2...8, FX-03). Every change sends the whole list. Adding takes the
    /// programmer's current colour; adding from a Colours palette is not offered (the app does not
    /// hold the palettes' values).
    private func colourList(_ list: [EffectColour]) -> some View {
        let send = { (new: [EffectColour]) in engine.setEffect("colours", new.map(\.json)) }
        return VStack(alignment: .leading, spacing: 4) {
            Text("Colours").font(Typo.legend(Typo.body, .semibold)).foregroundStyle(t.fg2)
            ScrollView(.horizontal) {
                HStack(spacing: 6) {
                    ForEach(Array(list.enumerated()), id: \.offset) { i, c in
                        VStack(spacing: 2) {
                            RoundedRectangle(cornerRadius: radius).fill(Color(red: c.r, green: c.g, blue: c.b))
                                .frame(width: 34, height: 26)
                                .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(t.line2, lineWidth: 1))
                            HStack(spacing: 1) {
                                Button("◂") { var l = list; l.swapAt(i, i - 1); send(l) }
                                    .buttonStyle(ChipStyle(on: false)).disabled(i == 0)
                                Button("×") { var l = list; l.remove(at: i); send(l) }
                                    .buttonStyle(ChipStyle(on: false)).disabled(list.count <= 2)
                                Button("▸") { var l = list; l.swapAt(i, i + 1); send(l) }
                                    .buttonStyle(ChipStyle(on: false)).disabled(i == list.count - 1)
                            }
                        }
                    }
                }
            }
            Button("Add current colour") {
                if let c = engine.programmer.colour { send(list + [EffectColour(r: c.r, g: c.g, b: c.b)]) }
            }
            .buttonStyle(FlatButton())
            .disabled(engine.programmer.colour == nil || list.count >= 8)
        }
    }
}

/// Chips that wrap onto more rows when the window is narrow.
private struct FlowChips<T: Equatable>: View {
    let items: [(String, T)]
    let selected: T?
    let pick: (T) -> Void
    var body: some View {
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 52), spacing: 2, alignment: .leading)], alignment: .leading, spacing: 2) {
            ForEach(Array(items.enumerated()), id: \.offset) { _, item in
                Button(item.0) { pick(item.1) }.buttonStyle(ChipStyle(on: selected == item.1)).frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }
}

/// FX-42: one cycle of the shape with a dot per member at x = frac(spread/360 · k/N), its phase
/// offset (FX-07). Positions come from the engine; only the dot's place is derived here.
private struct SpreadStrip: View {
    @Environment(\.theme) private var t
    let entry: EffectEntry

    var body: some View {
        let (line, dot, back, border) = (t.fg3, t.fg, t.bg, t.surf)
        let shape = entry.shape, spread = entry.params.spread, width = entry.params.width
        Canvas { ctx, size in
            let r = CGRect(origin: .zero, size: size).insetBy(dx: 6, dy: 8)
            ctx.fill(Path(CGRect(origin: .zero, size: size)), with: .color(border))
            // A wave draws itself, a path as its sine, a colour shape as its colours.
            let curve: (Double) -> Double
            switch shape.kind {
            case .wave:
                curve = { ShapePicture.wave(shape.name, $0, width: width) }
                ctx.stroke(ShapePicture.wavePath(shape.name, in: r, width: width), with: .color(line), lineWidth: 1.5)
            case .path:
                curve = { ShapePicture.wave("Sine", $0) }
                ctx.stroke(ShapePicture.wavePath("Sine", in: r), with: .color(line), lineWidth: 1.5)
            case .colour:
                curve = { _ in 0.5 }
                let g = Gradient(colors: ShapePicture.colours(shape, entry.colours))
                ctx.fill(Path(CGRect(x: r.minX, y: r.midY - 3, width: r.width, height: 6)),
                         with: .linearGradient(g, startPoint: CGPoint(x: r.minX, y: 0), endPoint: CGPoint(x: r.maxX, y: 0)))
            }
            for k in entry.positions {
                let phase = spread / 360 * k
                let x = phase - phase.rounded(.down)
                let c = CGPoint(x: r.minX + x * r.width, y: r.maxY - curve(x) * r.height)
                let d = CGRect(x: c.x - 3.5, y: c.y - 3.5, width: 7, height: 7)
                ctx.fill(Path(ellipseIn: d), with: .color(dot))
                ctx.stroke(Path(ellipseIn: d), with: .color(back), lineWidth: 1)
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: radius))
        .accessibilityLabel("Spread strip, \(entry.positions.count) fixtures")
    }
}

// MARK: - Effects (FX-39)

struct EffectsWindow: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState
    @State private var palettes = false

    var body: some View {
        let entries = engine.programmer.effects
        let pool = engine.palettes.filter { !$0.effects.isEmpty }
        PanelWindow(
            title: "Effects",
            info: palettes ? "\(pool.count) palettes" : "\(entries.count) entries",
            accessory: {
                HStack(spacing: 2) {
                    Button("Entries") { palettes = false }.buttonStyle(ModeButton(on: !palettes))
                    Button("Palettes") { palettes = true }.buttonStyle(ModeButton(on: palettes))
                }
            }
        ) {
            if palettes {
                if pool.isEmpty {
                    hint("No effect palettes yet")
                } else {
                    ScrollView {
                        LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 2), count: 6), spacing: 2) {
                            ForEach(pool) { p in
                                let first = p.effects.first
                                Cell(number: p.number,
                                     legend: p.legend.isEmpty ? (first?.look ?? first?.shape.title ?? "") : p.legend,
                                     icon: first.map { f in
                                         AnyView(ShapeIcon(shape: f.shape, tint: Theme.groupHue(engine.effectCatalogue.group(look: f.look, shape: f.shape))).frame(height: 22))
                                     }) { engine.applyEffectPalette(p.number) }
                            }
                        }
                        .padding(3)
                    }
                }
            } else if entries.isEmpty {
                hint("No effects in the programmer")
            } else {
                ScrollView {
                    VStack(spacing: 2) { ForEach(entries) { row($0) } }.padding(3)
                }
            }
        }
    }

    private func hint(_ text: String) -> some View {
        Text(text).font(Typo.ui(Typo.body)).foregroundStyle(t.fg3).frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func row(_ e: EffectEntry) -> some View {
        let hue = Theme.groupHue(engine.effectCatalogue.group(look: e.look, shape: e.shape))
        return HStack(spacing: 8) {
            Rectangle().fill(hue).frame(width: 3)
            ShapeIcon(shape: e.shape, tint: hue, colours: e.colours).frame(width: 40, height: 24)
            Text(e.look ?? e.shape.title).font(Typo.legend(Typo.body, .semibold)).foregroundStyle(t.fg).lineLimit(1)
            Spacer(minLength: 4)
            Text("\(e.members.count)").font(Typo.mono(Typo.small)).foregroundStyle(t.fg2)
            Text(speedLabel(e.params)).font(Typo.mono(Typo.small)).foregroundStyle(t.fg2).frame(minWidth: 56, alignment: .trailing)
            // Stop (FX-19) removes the current targets from their entries, so it is offered on the
            // current row only: that entry is what the selection runs.
            if e.current {
                Button("Stop") { ui.learnOrRun("effect stop", engine) { engine.stopEffects() } }.buttonStyle(FlatButton(fill: .red, compact: true))
            }
        }
        .padding(.trailing, 6).frame(height: 34)
        .background(t.surf)
        .overlay(Rectangle().strokeBorder(e.current ? t.blue : t.line, lineWidth: e.current ? 2 : 1))
        .clipShape(RoundedRectangle(cornerRadius: radius))
        .contentShape(Rectangle())
        .onTapGesture { engine.selectEffect(e.id); engine.setEffectWheels(true) }   // FX-18, FX-17
    }
}
