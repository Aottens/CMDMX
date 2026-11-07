import SwiftUI

// Views screen (UI-61): Intensity, Grid and DMX tabs over the engine's output feed (IPC-58) and the watched
// views (IPC-59). Pure drawing: every number, source label and channel value comes from the engine.

struct ViewsScreen: View {
    private enum Tab: String, CaseIterable { case intensity = "Intensity", grid = "Grid", dmx = "DMX" }
    private enum Filter: String, CaseIterable { case all = "All", stage = "Stage", programmer = "Programmer", selected = "Selected" }

    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState
    @State private var tab = Tab.intensity
    @State private var filter = Filter.all
    @State private var find = ""
    @FocusState private var findFocused: Bool
    @State private var group = "I"
    @State private var showPalettes = false
    @State private var showSources = false
    @State private var line: Int?

    /// The lines to pick from: every output node's line and every patched universe (OUT-13).
    private var lines: [Int] { Set(engine.outputs.map(\.line) + engine.fixtures.map(\.universe)).sorted() }
    private var activeLine: Int? { line.flatMap { lines.contains($0) ? $0 : nil } ?? lines.first }

    private var watch: OutputWatch? {
        switch tab {
        case .intensity: return nil
        case .grid: return .grid
        case .dmx: return activeLine.map(OutputWatch.dmx)
        }
    }

    /// UI-61: parked fixtures are left out; the filter and the find field narrow the rest. Display order = number.
    private var fixtures: [FixtureInfo] {
        let programmer = engine.programmer
        let touched = Set((filter == .programmer ? programmer.inProgrammer : programmer.selection).map(\.fixture))
        return engine.fixtures.filter { f in
            guard !f.parked, FixturesWindow.matches(f, find) else { return false }
            switch filter {
            case .all: return true
            case .stage: return (engine.output[Item(fixture: f.id)]?.i ?? 0) > 0
            case .programmer, .selected: return touched.contains(f.id)
            }
        }.sorted { $0.number < $1.number }
    }

    var body: some View {
        PanelWindow(title: "Views", framed: true, accessory: {
            HStack(spacing: 2) {
                ForEach(Tab.allCases, id: \.self) { tabCase in
                    Button(tabCase.rawValue) { tab = tabCase }.buttonStyle(FlatButton(fill: tab == tabCase ? .blue : nil, compact: true))
                }
            }
        }) {
            VStack(spacing: 0) {
                toolbar
                HLine()
                switch tab {
                case .intensity: IntensityTab(fixtures: fixtures)
                case .grid: gridTab
                case .dmx: DMXTab(line: activeLine)
                }
            }
        }
        // IPC-59: watch while a tab needs it, again after a reconnect (the daemon forgets it with the connection), stop when left.
        .task(id: "\(tab.rawValue) \(activeLine ?? 0) \(engine.connection)") {
            if engine.connection == .online { engine.watchOutput(watch) }
        }
        .onDisappear { engine.watchOutput(nil) }
    }

    private var toolbar: some View {
        HStack(spacing: 8) {
            if tab != .dmx {
                HStack(spacing: 2) {
                    ForEach(Filter.allCases, id: \.self) { f in
                        Button(f.rawValue) { filter = f }.buttonStyle(FlatButton(fill: filter == f ? .blue : nil, compact: true))
                    }
                }
                TextField("Find", text: $find).textFieldStyle(.plain).font(Typo.mono(Typo.small))
                    .frame(width: 110).focused($findFocused).onSubmit(selectMatches)
                    .onChange(of: findFocused) { _, now in if !now, ui.fieldEscaped { find = "" } }   // UI-44
            }
            if tab == .grid {
                Rectangle().fill(t.line).frame(width: 1, height: 14)
                HStack(spacing: 2) {
                    ForEach(attributeGroupOrder, id: \.self) { g in
                        Button(g) { group = g }.buttonStyle(FlatButton(fill: group == g ? .blue : nil, compact: true))
                    }
                }
                Toggle("Palettes", isOn: $showPalettes).toggleStyle(.checkbox).font(Typo.ui(Typo.small))
                Toggle("Sources", isOn: $showSources).toggleStyle(.checkbox).font(Typo.ui(Typo.small))
            }
            if tab == .dmx {
                Text("Line").font(Typo.ui(Typo.small)).foregroundStyle(t.fg2)
                HStack(spacing: 2) {
                    ForEach(lines, id: \.self) { l in
                        Button(String(l)) { line = l }.buttonStyle(FlatButton(fill: activeLine == l ? .blue : nil, compact: true))
                    }
                }
            }
            Spacer(minLength: 0)
            // The attribute bar's message zone is hidden on a screen, so its message shows here (a DMX tap, a refusal).
            if let message = engine.lastMessage {
                Text(message).font(Typo.ui(11)).foregroundStyle(t.amber).lineLimit(1)
            }
        }
        .padding(.horizontal, 8).frame(height: 28)
    }

    /// UI-59: Return selects the matches in number order, as the Fixtures window's find does; one already selected is skipped.
    private func selectMatches() {
        defer { find = ""; findFocused = false }
        guard !find.isEmpty, ui.armedAllows(.fixture, engine) else { return }
        let selected = Set(engine.programmer.selection)
        for f in fixtures where !selected.contains(Item(fixture: f.id)) { engine.selectFixture(f.id) }
    }

    // MARK: - Grid (UI-61)

    private static let columnWidth: CGFloat = 112

    /// Rows: a fixture, then its cell rows the feed holds (IPC-59). Columns: the attributes of the shown rows in the chosen group.
    private var gridTab: some View {
        let rows: [(FixtureInfo, Item)] = fixtures.flatMap { f -> [(FixtureInfo, Item)] in
            let cells = engine.outputGrid.keys.filter { $0.fixture == f.id && $0.cell != nil }.sorted { ($0.cell ?? 0) < ($1.cell ?? 0) }
            return [(f, Item(fixture: f.id))] + cells.map { (f, $0) }
        }
        let columns = Set(rows.flatMap { f, item in
            (engine.outputGrid[item] ?? [:]).keys.filter { Self.group(of: $0, f, item) == group }
        }).sorted()
        return GeometryReader { geo in ScrollView([.horizontal, .vertical]) {
            LazyVStack(alignment: .leading, spacing: 0, pinnedViews: []) {
                HStack(spacing: 0) {
                    Text("#").frame(width: 56, alignment: .leading)
                    ForEach(columns, id: \.self) { Text($0).frame(width: Self.columnWidth, alignment: .leading) }
                }
                .font(Typo.legend(Typo.small, .semibold)).foregroundStyle(t.fg2).padding(.vertical, 3)
                ForEach(rows, id: \.1) { f, item in
                    let values = engine.outputGrid[item] ?? [:]
                    HStack(spacing: 0) {
                        Text(item.cell.map { "\(f.number).\($0)" } ?? String(f.number)).font(Typo.mono())
                            .foregroundStyle(item.cell == nil ? t.fg : t.fg2).frame(width: 56, alignment: .leading)
                        ForEach(columns, id: \.self) { name in
                            Self.cell(values[name], palettes: showPalettes, sources: showSources, engine: engine, t: t)
                                .frame(width: Self.columnWidth, alignment: .leading).lineLimit(1)
                        }
                    }
                    .padding(.vertical, 2)
                    .contentShape(Rectangle())
                    .onTapGesture { if ui.armedAllows(.fixture, engine) { engine.selectFixture(f.id) } }
                    .overlay { if item.cell == nil { SecondaryClick { ui.fixtureInfo = FixtureRef(id: f.id) } } }
                }
            }
            .padding(8)
            .frame(minWidth: geo.size.width, minHeight: geo.size.height, alignment: .topLeading)   // a small grid sits top left, not centred
        } }
    }

    private static func group(of attribute: String, _ f: FixtureInfo, _ item: Item) -> String? {
        (item.cell == nil ? f.attributes : f.cellAttributes)[attribute]
    }

    /// One grid cell. SEE-10 will change what a cell shows, so the formatting stays in this one function.
    @ViewBuilder private static func cell(_ v: GridValue?, palettes: Bool, sources: Bool, engine: EngineClient, t: Theme) -> some View {
        if let v {
            if sources {
                Text(v.source ?? "·").font(Typo.ui(Typo.small)).foregroundStyle(v.source == nil ? t.fg3 : t.fg2)
            } else if palettes, let n = v.palette {
                Text(engine.palettes.first { $0.number == n }?.shownLegend ?? "Palette \(n)").font(Typo.ui(Typo.small)).foregroundStyle(t.cyan)
            } else {
                // As the Programmer window shows a value: a level in %, a colour as a swatch and its hex.
                switch v.v {
                case .level(let l):
                    Text(percent(l)).font(Typo.mono(Typo.small)).foregroundStyle(v.tracked ? t.magenta : t.fg)
                case .colour(let r, let g, let b):
                    Text("\(Text(Image(systemName: "square.fill")).font(.system(size: 8)).foregroundStyle(Color(red: r, green: g, blue: b))) \(ProgrammerWindow.hex(r, g, b))")
                        .font(Typo.mono(Typo.small)).foregroundStyle(v.tracked ? t.magenta : t.fg)
                }
            }
        } else {
            Text("·").foregroundStyle(t.fg3)
        }
    }
}

// MARK: - Intensity (UI-61)

private struct IntensityTab: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState
    let fixtures: [FixtureInfo]

    var body: some View {
        let selected = Set(engine.programmer.selection.map(\.fixture))
        let inProgrammer = Set(engine.programmer.inProgrammer.map(\.fixture))
        ScrollView {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 104), spacing: 4)], spacing: 4) {
                ForEach(fixtures) { f in
                    block(f, engine.output[Item(fixture: f.id)], selected: selected.contains(f.id), inProgrammer: inProgrammer.contains(f.id))
                        .contentShape(Rectangle())
                        .onTapGesture { if ui.armedAllows(.fixture, engine) { engine.selectFixture(f.id) } }   // as a tile does
                        .overlay { SecondaryClick { ui.fixtureInfo = FixtureRef(id: f.id) } }   // UI-57
                }
            }
            .padding(8)
        }
    }

    private func block(_ f: FixtureInfo, _ o: FixtureOutput?, selected: Bool, inProgrammer: Bool) -> some View {
        let level = o?.i ?? 0
        return VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 4) {
                Text(String(f.number)).font(Typo.mono(Typo.small, .semibold))
                Spacer(minLength: 0)
                // Tracked "=" in magenta, from an effect "~" (UI-61).
                if o?.tracked == true { Text("=").font(Typo.mono(Typo.small, .semibold)).foregroundStyle(t.magenta) }
                if o?.effect == true { Text("~").font(Typo.mono(Typo.small, .semibold)).foregroundStyle(t.amber) }
            }
            .padding(.horizontal, 4).frame(height: 16)
            .foregroundStyle(selected || inProgrammer ? Color.white : t.fg2)
            .background(selected ? t.blue : inProgrammer ? t.red : Color.clear)   // UI-61: blue when selected, red in the programmer
            Text(FixturesWindow.legend(f)).font(Typo.legend()).foregroundStyle(t.fg).lineLimit(1).padding(.horizontal, 4)
            Text(percent(level)).font(Typo.mono(Typo.large)).foregroundStyle(t.fg).padding(.horizontal, 4)
            GeometryReader { g in
                ZStack(alignment: .leading) {
                    Rectangle().fill(t.surf2)
                    Rectangle().fill(t.fg2).frame(width: g.size.width * CGFloat(level.clamped01))
                }
            }
            .frame(height: 4).padding(.horizontal, 4)
            Text(o?.source ?? " ").font(Typo.ui(Typo.small)).foregroundStyle(t.fg2).lineLimit(1).padding(.horizontal, 4).padding(.bottom, 4)
        }
        .background(t.surf)
        .overlay(Rectangle().strokeBorder(selected ? t.blue : t.line, lineWidth: selected ? 2 : 1))
    }
}

// MARK: - DMX (UI-61)

private struct DMXTab: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    let line: Int?

    var body: some View {
        // Which fixture holds each of the 512 channels, and the first channel of each run.
        let on = engine.fixtures.filter { !$0.parked && $0.universe == line }.sorted { $0.address < $1.address }
        var owner = [FixtureInfo?](repeating: nil, count: 512)
        for f in on { for ch in f.address..<(f.address + f.footprint) where (1...512).contains(ch) { owner[ch - 1] = f } }
        let tints = [t.blue, t.green, t.amber, t.cyan, t.violet, t.magenta]
        let order = Dictionary(on.enumerated().map { ($1.id, $0) }, uniquingKeysWith: { a, _ in a })
        let values = engine.outputDMX.flatMap { $0.line == line ? $0.channels : nil } ?? []
        return ScrollView {
            LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 1), count: 32), spacing: 1) {
                ForEach(0..<512, id: \.self) { i in
                    let f = owner[i]
                    VStack(spacing: 0) {
                        Text(f.map { $0.address == i + 1 ? String($0.number) : " " } ?? " ").font(Typo.mono(8)).foregroundStyle(t.fg)
                        Text(values.indices.contains(i) ? String(values[i]) : "·").font(Typo.mono(Typo.small)).foregroundStyle(t.fg)
                    }
                    .lineLimit(1).minimumScaleFactor(0.6)
                    .frame(maxWidth: .infinity).frame(height: 30)
                    .background(f.map { tints[(order[$0.id] ?? 0) % tints.count].opacity(0.3) } ?? t.surf)
                    .contentShape(Rectangle())
                    .onTapGesture { if let f { say(f, channel: i + 1 - f.address + 1) } }
                }
            }
            .padding(8)
        }
        // The profiles of the line's fixtures, so the first tap can already name the attribute.
        .task(id: line) {
            for key in Set(on.map(\.profile)) where engine.profileDocs[key] == nil { engine.requestProfile(key: key) }
        }
    }

    /// Names fixture and attribute of a channel from the profile, asked for when not yet known.
    private func say(_ f: FixtureInfo, channel n: Int) {
        let who = "\(f.number) \(FixturesWindow.legend(f))"
        if let doc = engine.profileDocs[f.profile], let what = Self.attribute(of: n, in: f, doc) {
            engine.say("\(who): \(what) (channel \(n) of \(f.footprint))")
        } else {
            if engine.profileDocs[f.profile] == nil { engine.requestProfile(key: f.profile) }
            engine.say("\(who): channel \(n) of \(f.footprint)")
        }
    }

    // ponytail: a cell's attribute channels are read as offsets in the whole mode, like the mode's own; check against a real cell profile.
    private static func attribute(of n: Int, in f: FixtureInfo, _ doc: ProfileDoc) -> String? {
        guard let mode = doc.modes.first(where: { $0.name == f.mode }) else { return nil }
        let parts = [("", mode.attributes, mode.colour)] + mode.cells.enumerated().map { ("cell \($0 + 1) ", $1.attributes, $1.colour) }
        for (prefix, attributes, colour) in parts {
            if let a = attributes.first(where: { $0.channel == n }) { return prefix + a.name }
            if let a = attributes.first(where: { $0.fineChannel == n }) { return prefix + a.name + " fine" }
            if let c = colour, let i = c.components.firstIndex(where: { $0.channel == n || $0.fineChannel == n }),
               let role = c.roles[safe: i] { return prefix + role }
        }
        return nil
    }
}
