import SwiftUI

// The windows of the 2x2 grid (UI-08..13) and the one cell view they share (UI-05).
// Pure rendering of `EngineClient` state plus command sending: selection order, targets and
// groups come from the engine; the only thing computed here is display order and lookups.

// MARK: - Cell (UI-05)

/// One button of the fixed 6-column grid: 72 pt high, number top-left, tag or selection order
/// top-right, condensed legend. State never changes the fill.
struct Cell: View {
    @Environment(\.theme) private var t
    /// nil: no number (an empty palette cell gets its number when it is recorded).
    let number: Int?
    var tag: String?
    var legend = ""
    /// 1-based selection order, shown top-right in blue (replaces the tag).
    var order: Int?
    /// UI-26: fixture is located; `LOC` shows under the order number, whatever the order/tag shows.
    var located = false
    var empty = false
    var selected = false
    /// Current target during a sub-selection: a double blue border.
    var current = false
    /// Selected but not a target during a sub-selection: blue at 40 %.
    var dimmed = false
    var inProgrammer = false
    /// Record armed: empty cells show a red number.
    var armed = false
    /// Disabled output lamp stripe (fixtures, IDEA-10) / empty IPCGBES mask strip (groups).
    var lamp = false
    var mask = false
    /// With `mask`: the attribute groups that are lit in the IPCGBES strip (palettes, PAL-28).
    var maskGroups: [String] = []
    /// Not relevant to the selection: 38 % opacity (UI-05).
    var faded = false
    /// A palette's icon, between the number row and the legend.
    var icon: AnyView?
    var action: (() -> Void)?

    var body: some View {
        ZStack(alignment: .topLeading) {
            Rectangle().fill(empty ? Color.clear : t.surf)

            if let number {
                Text(String(number)).font(Typo.mono(Typo.small))
                    .foregroundStyle(selected ? t.blue.opacity(dimmed ? 0.4 : 1) : empty ? (armed ? t.red : t.fg3) : t.fg2)
                    .padding(.leading, 4).padding(.top, 4)
            } else if armed {
                // Record armed: an empty palette cell shows a red mark where its number will be.
                Text("+").font(Typo.mono(Typo.small)).foregroundStyle(t.red).padding(.leading, 4).padding(.top, 4)
            }

            if !empty {
                VStack(spacing: 0) {
                    Spacer(minLength: 0)
                    Text(legend).font(Typo.legend()).foregroundStyle(t.fg)
                        .lineLimit(2).multilineTextAlignment(.leading)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.horizontal, 4).padding(.bottom, 10)
                }
            }
            if lamp {
                Rectangle().fill(Color.black).frame(height: 4)
                    .overlay(Rectangle().strokeBorder(t.line, lineWidth: 1))
                    .padding(.horizontal, 4).padding(.bottom, 3)
                    .frame(maxHeight: .infinity, alignment: .bottom)
            }
            if let icon {
                icon.padding(.horizontal, 4).padding(.top, 19)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            }
            if mask {
                HStack(spacing: 1) {
                    ForEach(attributeGroupOrder, id: \.self) { group in
                        Rectangle().fill(maskGroups.contains(group) ? Theme.groupHue(group) : t.line).frame(height: 3)
                    }
                }
                    .padding(.horizontal, 3).padding(.bottom, 2)
                    .frame(maxHeight: .infinity, alignment: .bottom)
            }
        }
        .frame(maxWidth: .infinity).frame(height: 72)
        .overlay(alignment: .topTrailing) {
            if let order {
                Text(String(order)).font(Typo.mono(Typo.small, .semibold)).foregroundStyle(t.blue.opacity(dimmed ? 0.4 : 1)).padding(4)
            } else if let tag {
                Text(tag.uppercased()).font(Typo.mono(9, .medium)).tracking(0.5).foregroundStyle(t.fg2).padding(4)
            }
        }
        .overlay(alignment: .top) {
            if inProgrammer { Rectangle().fill(t.red).frame(height: 3) }
        }
        .overlay(alignment: .topTrailing) {   // UI-26
            if located {
                Text("LOC").font(Typo.mono(9, .medium)).tracking(0.5).foregroundStyle(t.amber).padding(.horizontal, 4).padding(.top, 17)
            }
        }
        .overlay {
            if current {
                // inset 2 blue, 2 background, 1 blue (the sketch's .cur)
                ZStack {
                    Rectangle().strokeBorder(t.blue, lineWidth: 2)
                    Rectangle().inset(by: 2).strokeBorder(t.bg, lineWidth: 2)
                    Rectangle().inset(by: 4).strokeBorder(t.blue, lineWidth: 1)
                }
            } else {
                Rectangle().strokeBorder(selected ? t.blue.opacity(dimmed ? 0.4 : 1) : t.line, lineWidth: selected ? 2 : 1)
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: radius))
        .opacity(faded ? 0.38 : 1)
        .contentShape(Rectangle())
        .onTapGesture { action?() }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(action == nil ? [] : .isButton)
    }
}

/// A scrolling area holding the 8-column cell grid with 2 pt gaps.
private struct CellScroll<Content: View>: View {
    @ViewBuilder var content: Content
    var body: some View {
        ScrollView {
            LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 2), count: 6), spacing: 2) { content }
                .padding(3)
        }
    }
}

// MARK: - Fixtures (UI-11)

struct FixturesWindow: View {
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState
    @State private var page = 0

    // ponytail: `handles_per_page` is a setting (PAT-14) that is not on the wire yet; 40 is its default.
    private let handlesPerPage = 40

    /// Display order only: by handle (page, slot); fixtures without a handle last, by number.
    private static func handleOrder(_ a: FixtureInfo, _ b: FixtureInfo) -> Bool {
        switch (a.handle, b.handle) {
        case let (x?, y?): return (x.page, x.slot) < (y.page, y.slot)
        case (_?, nil): return true
        case (nil, _?): return false
        case (nil, nil): return a.number < b.number
        }
    }

    /// The legend is the fixture's name, else the model part of `manufacturer/model`.
    private static func legend(_ f: FixtureInfo) -> String {
        if !f.name.isEmpty { return f.name }
        guard let slash = f.profile.firstIndex(of: "/") else { return f.profile }
        return String(f.profile[f.profile.index(after: slash)...])
    }

    var body: some View {
        let programmer = engine.programmer
        // Display lookups only: 1-based position in the engine's selection order.
        // PRG-55: a fixture button speaks for its whole-fixture item; cells show with VS2-34.
        let order = Dictionary(programmer.selection.enumerated().map { ($1, $0 + 1) }, uniquingKeysWith: { a, _ in a })
        // Double outline only while the engine's targets differ from its selection (sub-selection).
        let subSelection = programmer.targets != programmer.selection
        let targets = Set(programmer.targets)
        let inProgrammer = Set(programmer.inProgrammer)

        let sorted = engine.fixtures.sorted(by: Self.handleOrder)
        let pageCount = max(1, (sorted.count + handlesPerPage - 1) / handlesPerPage)
        let shown = min(page, pageCount - 1)
        let visible = sorted.dropFirst(shown * handlesPerPage).prefix(handlesPerPage)

        PanelWindow(
            title: "Fixtures",
            info: programmer.selection.isEmpty ? "\(sorted.count) fixtures" : "\(programmer.selection.count) selected · order shown",
            accessory: {
                if pageCount > 1 {
                    HStack(spacing: 2) {
                        ForEach(0..<pageCount, id: \.self) { p in
                            Button(String(p + 1)) { page = p }.buttonStyle(FlatButton(fill: p == shown ? .blue : nil, compact: true))
                        }
                    }
                }
            }
        ) {
            CellScroll {
                ForEach(visible) { fixture in
                    Cell(number: fixture.number,
                         tag: fixture.cells > 0 ? "\(fixture.cells)" : nil,
                         legend: Self.legend(fixture),
                         order: order[Item(fixture: fixture.id)],
                         located: programmer.located[Item(fixture: fixture.id)] != nil,   // UI-26
                         selected: order[Item(fixture: fixture.id)] != nil,
                         current: subSelection && targets.contains(Item(fixture: fixture.id)),
                         dimmed: subSelection && order[Item(fixture: fixture.id)] != nil && !targets.contains(Item(fixture: fixture.id)),
                         inProgrammer: inProgrammer.contains(Item(fixture: fixture.id)),
                         lamp: true) { tap(fixture) }
                    if ui.unfolded.contains(fixture.id) {
                        // UI-29: the cells follow their fixture, in the same button system.
                        ForEach(Array(1..<fixture.cells + 1), id: \.self) { n in
                            let item = Item(fixture: fixture.id, cell: n)
                            Cell(number: nil,
                                 legend: "\(fixture.number).\(n)",
                                 order: order[item],
                                 located: programmer.located[item] != nil,
                                 selected: order[item] != nil,
                                 current: subSelection && targets.contains(item),
                                 dimmed: subSelection && order[item] != nil && !targets.contains(item),
                                 inProgrammer: inProgrammer.contains(item),
                                 lamp: true) { engine.selectCell(fixture.id, cell: n) }
                        }
                    }
                }
            }
        }
    }
}

extension FixturesWindow {
    /// While Unfold is armed a tap folds or unfolds instead of selecting (UI-29), then disarms.
    fileprivate func tap(_ fixture: FixtureInfo) {
        guard ui.function == .unfold else { return engine.selectFixture(fixture.id) }
        if fixture.cells == 0 {
            engine.say("Unfold: this fixture has no cells.")
        } else if !ui.unfolded.insert(fixture.id).inserted {
            ui.unfolded.remove(fixture.id)
        }
        ui.cancelModes()
    }
}

// MARK: - Groups (UI-12)

struct GroupsWindow: View {
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState

    var body: some View {
        let byId = Dictionary(engine.groups.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
        // 24 cells, or more (whole rows of 6) when the show has higher group numbers.
        let cells = max(24, ((engine.groups.map(\.id).max() ?? 0) + 5) / 6 * 6)
        let selection = Set(engine.programmer.selection)

        PanelWindow(
            title: "Groups",
            info: "\(engine.groups.count) groups",
            accessory: { EmptyView() }   // Record is a function button now (UI-16)
        ) {
            CellScroll {
                ForEach(1...cells, id: \.self) { id in
                    if let group = byId[id] {
                        Cell(number: id, tag: "\(group.fixtures.count)", legend: "Group \(id)",
                             selected: !group.fixtures.isEmpty && group.fixtures.allSatisfy(selection.contains),
                             mask: true) { tap(id, exists: true) }
                    } else {
                        // Empty cells only react while Record is armed.
                        Cell(number: id, empty: true, armed: ui.recordArmed,
                             action: ui.recordArmed ? { tap(id, exists: false) } : nil)
                    }
                }
            }
        }
    }

    /// While armed, the next group tap records instead of selecting.
    private func tap(_ id: Int, exists: Bool) {
        if ui.recordArmed, ui.recordMaster == "Group" {
            // GRM-15: this tap picks the group of a group master; the handle tap records it.
            guard exists else { return engine.say("Group Master: tap an existing group.") }
            ui.recordMasterGroup = id
            engine.say("Group Master: tap an empty handle.")
        } else if ui.recordArmed {
            engine.recordGroup(id)
            ui.recordArmed = false
        } else if exists {
            engine.selectGroup(id)
        }
    }
}

// MARK: - Workspaces (UI-08)

enum WindowKind {
    case groups, fixtures, colours, positions, beams, programmer, picker, cues, active

    /// The palette window this is, if it is one.
    var paletteKind: PaletteWindowKind? {
        switch self {
        case .colours: return .colours
        case .positions: return .positions
        case .beams: return .beams
        default: return nil
        }
    }

    @ViewBuilder var view: some View {
        switch self {
        case .groups: GroupsWindow()
        case .fixtures: FixturesWindow()
        case .colours: PaletteWindow(kind: .colours)
        case .positions: PaletteWindow(kind: .positions)
        case .beams: PaletteWindow(kind: .beams)
        case .programmer: ProgrammerWindow()
        case .picker: ColourPickerWindow()
        case .cues: CueWindow()
        case .active: ActivePlaybacksWindow()
        }
    }
}

struct Workspace {
    let name: String
    let windows: [WindowKind]

    static let all = [
        Workspace(name: "Groups + Palettes", windows: [.groups, .colours, .positions, .beams]),
        Workspace(name: "Fixtures + Groups", windows: [.fixtures, .groups, .colours, .programmer]),
        Workspace(name: "Colour", windows: [.fixtures, .picker, .colours, .programmer]),
        Workspace(name: "Playbacks", windows: [.cues, .active, .fixtures, .programmer]),
    ]
}

/// The fixed 2x2 grid of one workspace, separated by 1 px hairlines.
struct WindowGrid: View {
    let workspace: Workspace

    var body: some View {
        GeometryReader { geo in
            let w = (geo.size.width - 1) / 2, h = (geo.size.height - 1) / 2
            VStack(spacing: 0) {
                HStack(spacing: 0) {
                    cell(0, w, h); VLine(); cell(1, w, h)
                }
                HLine()
                HStack(spacing: 0) {
                    cell(2, w, h); VLine(); cell(3, w, h)
                }
            }
        }
    }

    @ViewBuilder private func cell(_ i: Int, _ w: CGFloat, _ h: CGFloat) -> some View {
        if let kind = workspace.windows[safe: i] {
            kind.view.frame(width: max(0, w), height: max(0, h)).clipped()
        }
    }
}

extension Array {
    subscript(safe index: Int) -> Element? { indices.contains(index) ? self[index] : nil }
}
