import SwiftUI

// The Colours, Positions and Gobos & Beams windows (UI-13, PAL-28) and the legend editor
// (PAL-29). Pure rendering of the engine's `palettes` plus command sending: the engine records,
// resolves and fades; this file decides only which cell shows what and which command a tap sends.

/// The three palette windows. `wire` is the engine's window name (PAL-27).
enum PaletteWindowKind {
    case colours, positions, beams

    var wire: String {
        switch self {
        case .colours: return "Colours"
        case .positions: return "Positions"
        case .beams: return "Beams"
        }
    }
    var title: String { self == .beams ? "Gobos & Beams" : wire }
    /// The empty legend: the window name and the palette number (PAL-20).
    var fallback: String {
        switch self {
        case .colours: return "Colour"
        case .positions: return "Position"
        case .beams: return "Beam"
        }
    }
    /// The window's default record mask (PAL-01), for the title and as the Mask softkeys' start.
    // ponytail: display copy of the engine's `Window::mask`; the engine decides what a record stores.
    var mask: [String] {
        switch self {
        case .colours: return ["C"]
        case .positions: return ["P"]
        case .beams: return ["I", "G", "B", "E", "S"]
        }
    }

    static func of(wire: String) -> PaletteWindowKind? {
        [PaletteWindowKind.colours, .positions, .beams].first { $0.wire == wire }
    }
}

/// Group letters in console order; the Mask softkeys offer the first seven (IPCGBES).
let attributeGroupOrder = ["I", "P", "C", "G", "B", "E", "S", "FX"]

/// "2" for 2.0, "2.5" for 2.5.
func plain(_ d: Double) -> String { d == d.rounded() ? String(Int(d)) : String(format: "%g", d) }

extension PaletteInfo {
    /// The legend, or the window name and number when it is empty.
    var shownLegend: String {
        legend.isEmpty ? "\(PaletteWindowKind.of(wire: window)?.fallback ?? "Palette") \(number)" : legend
    }
}

// MARK: - Window

struct PaletteWindow: View {
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState
    @EnvironmentObject private var line: TokenLine
    let kind: PaletteWindowKind
    /// The last plain tap on an empty cell: a second one within `doubleClick` seconds is Quick Record.
    @State private var lastTap: (handle: Int, at: Date)?

    private let doubleClick: TimeInterval = 0.4

    var body: some View {
        let mine = engine.palettes.filter { $0.window == kind.wire }
        let byHandle = Dictionary(mine.map { ($0.handle, $0) }, uniquingKeysWith: { a, _ in a })
        // At least 36 cells (6 rows of 6), and always one empty row after the last used cell (PAL-28).
        let rows = max(6, ((mine.map(\.handle).max() ?? 0) + 5) / 6 + 1)
        let programmer = engine.programmer
        let inUse = Set(programmer.palettes.inUse), relevant = Set(programmer.palettes.relevant)
        let hasSelection = !programmer.selection.isEmpty

        PanelWindow(
            title: kind.title,
            info: "mask \(kind.mask.joined()) · \(mine.count) \(mine.count == 1 ? "palette" : "palettes")",
            accessory: { EmptyView() }
        ) {
            ScrollView {
                LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 2), count: 6), spacing: 2) {
                    ForEach(1...(rows * 6), id: \.self) { handle in
                        if let p = byHandle[handle] {
                            Cell(number: p.number, tag: p.kind, legend: p.shownLegend,
                                 picked: ui.isPicked(.palette, p.number),
                                 inProgrammer: inUse.contains(p.id),
                                 mask: true, maskGroups: p.groups,
                                 faded: hasSelection && !relevant.contains(p.id),
                                 icon: AnyView(PaletteIconView(palette: p))) { tap(handle, p) }
                        } else {
                            Cell(number: nil, empty: true, armed: ui.recordTargets) { tap(handle, nil) }
                        }
                    }
                }
                .padding(3)
            }
        }
    }

    /// What a tap on a cell sends, by what is armed (PAL-28, PAL-29).
    private func tap(_ handle: Int, _ p: PaletteInfo?) {
        if ui.recordTargets || ui.function != nil {
            guard ui.armedAllows(.palette, engine) else { return }   // UI-42
            lastTap = nil
            return PaletteTaps.armed(window: kind.wire, handle: handle, p, engine: engine, ui: ui)
        }
        guard let p else { return quickRecord(handle) }
        lastTap = nil
        // A number alone on the command line is the fade time of this tap (Titan).
        let time = line.onlyNumber
        engine.applyPalette(p.number, time: time, overlap: ui.pendingOverlap)
        if time != nil { line.clear() }
        ui.pendingOverlap = nil
    }

    /// Two plain taps on an empty cell: Quick Record with the window's mask.
    private func quickRecord(_ handle: Int) {
        if let last = lastTap, last.handle == handle, Date().timeIntervalSince(last.at) <= doubleClick {
            lastTap = nil
            engine.recordPalette(window: kind.wire, handle: handle)
        } else {
            lastTap = (handle, Date())
        }
    }
}

/// UI-46: the armed tap on a palette cell, called by the palette windows and by the command line (`Palette n`).
@MainActor
enum PaletteTaps {
    /// The armed tap: Record records, a function performs.
    static func armed(window: String, handle: Int, _ p: PaletteInfo?, engine: EngineClient, ui: UIState) {
        if ui.recordTargets { return record(window, handle, p, engine: engine, ui: ui) }
        if let function = ui.function { perform(function, window, handle, p, engine: engine, ui: ui) }
    }

    /// Record armed: an empty cell records. A used cell asks for Replace / Merge / Quick Merge on the
    /// softkeys; a second tap on the same cell is Quick Merge (Titan).
    private static func record(_ window: String, _ handle: Int, _ p: PaletteInfo?, engine: EngineClient, ui: UIState) {
        guard let p else {
            send(window, handle, mode: nil, engine: engine, ui: ui)
            return
        }
        let cell = UsedCell(window: window, handle: handle, number: p.number)
        if ui.usedCell == cell {
            send(window, handle, mode: "QuickMerge", engine: engine, ui: ui)
        } else {
            ui.usedCell = cell
            ui.maskMenu = false
            engine.say("Palette \(p.number) is used: Replace, Merge or Quick Merge on the softkeys.")
        }
    }

    private static func send(_ window: String, _ handle: Int, mode: String?, engine: EngineClient, ui: UIState) {
        engine.recordPalette(window: window, handle: handle, mask: ui.orderedRecordMask, recordBy: ui.recordBy,
                             recordAs: ui.recordAs, mode: mode)
        ui.recordArmed = false
    }

    private static func perform(_ function: PaletteFunction, _ window: String, _ handle: Int, _ p: PaletteInfo?, engine: EngineClient, ui: UIState) {
        switch function {
        case .copy, .move, .delete:   // UI-64
            let order = engine.palettes.filter { $0.window == window }.sorted { $0.handle < $1.handle }.map(\.number)
            switch ui.pickTap(.palette, item: p?.number, order: order) {
            case .picked: engine.sayPicks(ui)
            case .target:
                ui.sendPicks(engine) {
                    function == .copy ? engine.copyPalettes(ui.picks, window: window, handle: handle, layout: ui.layoutWire)
                                      : engine.movePalettes(ui.picks, window: window, handle: handle, layout: ui.layoutWire, swap: ui.swapIfRequired)
                }
            case .empty: engine.say(function == .delete ? "Delete: that cell is empty." : "Tap a recorded palette as the source.")
            }
        default:
            guard let p else { return engine.say("\(function.rawValue): that cell is empty.") }
            switch function {
            case .update:
                engine.recordPalette(window: window, handle: handle, mode: "QuickMerge")
                ui.cancelModes()
            case .include:
                engine.includePalette(p.number)
                ui.cancelModes()
            case .legend:
                ui.legendEditing = PaletteRef(number: p.number)
            case .copy, .move, .delete, .release, .options, .latch, .unfold:
                break
            }
        }
    }
}

// MARK: - Legend editor (PAL-29)

/// Text, number (Renumber) and icon of one palette. "Auto" is the automatic icon (PAL-20).
struct LegendEditor: View {
    @Environment(\.theme) private var t
    @Environment(\.dismiss) private var dismiss
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState
    let number: Int
    @State private var legend = ""
    @State private var numberText = ""
    @State private var icon: String?
    @State private var loaded = false

    private var palette: PaletteInfo? { engine.palettes.first { $0.number == number } }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(verbatim: "SET LEGEND · PALETTE \(number)").font(Typo.legend(Typo.body, .semibold)).tracking(0.7)

            HStack(spacing: 8) {
                TextField("Legend", text: $legend).textFieldStyle(.roundedBorder).onSubmit(apply)
                    .onChange(of: legend) { _, new in if new.count > 32 { legend = String(new.prefix(32)) } }
                Text(verbatim: "\(legend.count)/32").font(Typo.mono(Typo.small)).foregroundStyle(t.fg2)
            }
            HStack(spacing: 8) {
                Text("Number").font(Typo.ui(11)).foregroundStyle(t.fg2)
                TextField("Number", text: $numberText).textFieldStyle(.roundedBorder).frame(width: 80).onSubmit(apply)
            }

            IconPicker(icon: $icon, autoIcon: palette?.autoIcon)

            HStack {
                Spacer()
                Button("Cancel") { close() }.keyboardShortcut(.cancelAction)
                Button("Set") { apply() }.keyboardShortcut(.defaultAction)
            }
        }
        .padding(14)
        .frame(width: 420)
        .background(t.bg)
        .foregroundStyle(t.fg)
        .onAppear {
            guard !loaded, let palette else { return }
            loaded = true
            legend = palette.legend
            numberText = String(palette.number)
            icon = palette.icon
        }
    }

    /// The legend goes first, by the old number; then the renumber, if the number changed.
    private func apply() {
        guard let palette else { return close() }
        let trimmed = numberText.trimmingCharacters(in: .whitespaces)
        let renumber = trimmed.isEmpty ? nil : Int(trimmed)
        guard trimmed.isEmpty || renumber != nil else { return engine.say("The number must be a whole number.") }
        engine.setPaletteLegend(palette.number, legend: legend, icon: icon)
        if let renumber, renumber != palette.number { engine.renumberPalette(palette.number, to: renumber) }
        close()
    }

    private func close() {
        ui.cancelModes()
        ui.legendEditing = nil
        dismiss()
    }
}

// MARK: - Icon picker (PAL-29, PBK-38)

/// The icon library as a grid by category, and "Auto" with the automatic icon the engine computed.
/// Shared by the palette and the playback legend editors.
struct IconPicker: View {
    @Environment(\.theme) private var t
    @Binding var icon: String?
    let autoIcon: AutoIcon?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 8) {
                sectionTitle("Automatic")
                Button { icon = nil } label: {
                    VStack(spacing: 4) {
                        AutoIconView(icon: autoIcon).frame(width: 60, height: 14)
                        Text("Auto").font(Typo.legend(Typo.small))
                    }
                    .frame(width: 76, height: 40)
                    .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(icon == nil ? t.blue : t.line, lineWidth: icon == nil ? 2 : 1))
                }
                .buttonStyle(.plain)
                ForEach(PaletteIcons.categories, id: \.self) { category in
                    sectionTitle(category)
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 44), spacing: 4)], alignment: .leading, spacing: 4) {
                        ForEach(PaletteIcons.ids(in: category), id: \.id) { item in
                            Button { icon = item.id } label: {
                                IconGlyph(entry: item.entry, size: 18).foregroundStyle(t.fg)
                                    .frame(width: 44, height: 36)
                                    .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(icon == item.id ? t.blue : t.line, lineWidth: icon == item.id ? 2 : 1))
                            }
                            .buttonStyle(.plain)
                            .help(PaletteIcons.name(item.id))
                        }
                    }
                }
            }
        }
        .frame(height: 260)
    }

    private func sectionTitle(_ text: String) -> some View {
        Text(text.uppercased()).font(Typo.legend(Typo.small)).tracking(0.8).foregroundStyle(t.fg3)
    }
}
