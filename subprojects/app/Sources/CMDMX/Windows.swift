import AppKit
import Combine
import SwiftUI

// The windows of the 2x2 grid (UI-08..13) and the one cell view they share (UI-05).
// Pure rendering of `EngineClient` state plus command sending: selection order, targets and
// groups come from the engine; the only thing computed here is display order and lookups.

// MARK: - Cell (UI-05)

/// One button of the fixed 6-column grid: 72 pt high, number top-left, tag or selection order
/// top-right, condensed legend. State never changes the fill.
/// UI-60 / UI-68: the only reader of live output in a tile, so a delta redraws the stripes and not the window.
private struct LampStripe: NSViewRepresentable {
    let cell: OutputCell

    /// The stripe shows the output colour at its intensity; black at 0 (IPC-58).
    fileprivate static func colour(_ out: FixtureOutput?) -> CGColor {
        guard let o = out, o.i > 0 else { return CGColor(gray: 0, alpha: 1) }
        let c = o.rgb ?? [1, 1, 1]
        guard c.count == 3, let space = CGColorSpace(name: CGColorSpace.linearSRGB),
              let col = CGColor(colorSpace: space, components: [c[0] * o.i, c[1] * o.i, c[2] * o.i, 1])
        else { return CGColor(gray: 0, alpha: 1) }
        return col
    }

    final class StripeView: NSView {
        var bound: ObjectIdentifier?
        var sub: AnyCancellable?
        override init(frame: NSRect) { super.init(frame: frame); wantsLayer = true }
        required init?(coder: NSCoder) { nil }
        func bind(_ cell: OutputCell) {
            bound = ObjectIdentifier(cell)
            sub = cell.subject.sink { [weak self] out in
                CATransaction.begin(); CATransaction.setDisableActions(true)
                self?.layer?.backgroundColor = LampStripe.colour(out)
                CATransaction.commit()
            }
        }
    }

    func makeNSView(context: Context) -> StripeView { let v = StripeView(); v.bind(cell); return v }
    func updateNSView(_ v: StripeView, context: Context) {
        if v.bound != ObjectIdentifier(cell) { v.bind(cell) }
    }
}

struct Cell: View {
    @Environment(\.theme) private var t
    /// nil: no number (an empty palette cell gets its number when it is recorded).
    let number: Int?
    var tag: String?
    var legend = ""
    /// UI-59: the profile's model on a second small line under the legend.
    var model = ""
    /// 1-based selection order, shown top-right in blue (replaces the tag).
    var order: Int?
    /// UI-26: fixture is located; `LOC` shows under the order number, whatever the order/tag shows.
    var located = false
    var empty = false
    var selected = false
    /// UI-64: picked by Copy, Move or Delete: the function's colour (red) outline and tint.
    var picked = false
    /// Current target during a sub-selection: a double blue border.
    var current = false
    /// Selected but not a target during a sub-selection: blue at 40 %.
    var dimmed = false
    var inProgrammer = false
    /// Record armed: empty cells show a red number.
    var armed = false
    /// Disabled output lamp stripe (fixtures, IDEA-10) / empty IPCGBES mask strip (groups).
    var lamp = false
    /// UI-60 / IPC-58: with `lamp`, the item whose output the stripe shows; nil = black stripe.
    var lampCell: OutputCell?
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
                        .lineLimit(model.isEmpty ? 2 : 1).multilineTextAlignment(.leading)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.horizontal, 4).padding(.bottom, model.isEmpty ? 10 : 0)
                    if !model.isEmpty {
                        Text(model).font(Typo.mono(9)).foregroundStyle(t.fg2).lineLimit(1)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.horizontal, 4).padding(.bottom, 10)
                    }
                }
            }
            if lamp {
                LampStripe(cell: lampCell ?? OutputCell()).frame(height: 4)
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
        .overlay { if picked { Rectangle().fill(t.red.opacity(0.18)).overlay(Rectangle().strokeBorder(t.red, lineWidth: 2)) } }   // UI-64
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
    /// A tile of the grid: a fixture or one of its cells, or the empty handle slot (UI-77).
    private enum Tile: Hashable { case item(Item), empty(Int) }

    @EnvironmentObject private var engine: EngineClient

    @EnvironmentObject private var ui: UIState
    @State private var page = 0
    @Environment(\.theme) private var t
    /// UI-58: the item a tap in this window last selected; the engine's reply arrives a moment later.
    @State private var lastTap: Item?
    @State private var scrollTarget: Item?
    /// UI-59: the find field.
    @State private var find = ""
    @FocusState private var findFocused: Bool

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
    static func legend(_ f: FixtureInfo) -> String { f.name.isEmpty ? model(f) : f.name }

    /// The model part of `manufacturer/model`.
    static func model(_ f: FixtureInfo) -> String {
        guard let slash = f.profile.firstIndex(of: "/") else { return f.profile }
        return String(f.profile[f.profile.index(after: slash)...])
    }

    /// UI-59: number (prefix), name or model (contains, case-insensitive); an empty query matches all.
    private func matches(_ f: FixtureInfo) -> Bool { Self.matches(f, find) }

    static func matches(_ f: FixtureInfo, _ query: String) -> Bool {
        let q = query.trimmingCharacters(in: .whitespaces)
        return q.isEmpty || String(f.number).hasPrefix(q)
            || f.name.localizedCaseInsensitiveContains(q) || Self.model(f).localizedCaseInsensitiveContains(q)
    }

    /// One tile with its fixture (nil for an empty slot) and its handle slot.
    private struct Row { let tile: Tile; let fixture: FixtureInfo?; let slot: Int }

    /// UI-58, UI-77: what the window shows, worked out from the display-ordered fixtures.
    private struct Layout {
        var pageCount = 1
        var shown = 0
        var pageOf: [Int: Int] = [:]
        var selectedPages: Set<Int> = []
        var onShown = 0
        var rows: [Row] = []
    }

    private static func layout(sorted: [FixtureInfo], selection: [Item], page: Int, moving: Bool,
                               handlesPerPage: Int, unfolded: Set<Int>) -> Layout {
        var out = Layout()
        let handles: [FixtureInfo.Handle] = sorted.compactMap { $0.handle }
        let lastPage: Int = max(1, handles.map { $0.page }.max() ?? 1)
        out.pageCount = lastPage + (moving ? 1 : 0)
        let shown: Int = min(page, out.pageCount - 1)
        out.shown = shown

        var bySlot: [Int: FixtureInfo] = [:]
        for f in sorted {
            guard let h = f.handle else { continue }
            let key: Int = h.page * 1000 + h.slot
            if bySlot[key] == nil { bySlot[key] = f }
        }
        let onThisPage: [Int] = handles.filter { $0.page == shown + 1 }.map { $0.slot }
        let lastSlot: Int = moving ? handlesPerPage : (onThisPage.max() ?? 0)

        // UI-58: the page each selected fixture sits on (0-based handle page).
        var pageOf: [Int: Int] = [:]
        for f in sorted where pageOf[f.id] == nil { pageOf[f.id] = (f.handle?.page ?? lastPage) - 1 }
        out.pageOf = pageOf
        for item in selection {
            guard let p = pageOf[item.fixture] else { continue }
            out.selectedPages.insert(p)
            if p == shown { out.onShown += 1 }
        }

        // UI-77: a gap is an empty tile; a fixture without a handle goes after the slots of the last real page.
        var rows: [Row] = []
        for slot in 0..<lastSlot {
            let f: FixtureInfo? = bySlot[(shown + 1) * 1000 + slot + 1]
            rows.append(contentsOf: Self.rows(for: f, slot: slot + 1, unfolded: unfolded))
        }
        if shown == lastPage - 1 {
            for f in sorted where f.handle == nil { rows.append(contentsOf: Self.rows(for: f, slot: 0, unfolded: unfolded)) }
        }
        out.rows = rows
        return out
    }

    /// A fixture's tile, then its cells when it is unfolded (UI-29); an empty slot is one empty tile.
    private static func rows(for f: FixtureInfo?, slot: Int, unfolded: Set<Int>) -> [Row] {
        guard let f else { return [Row(tile: .empty(slot), fixture: nil, slot: slot)] }
        var out: [Row] = [Row(tile: .item(Item(fixture: f.id)), fixture: f, slot: slot)]
        if unfolded.contains(f.id) {
            for c in 0..<f.cells { out.append(Row(tile: .item(Item(fixture: f.id, cell: c + 1)), fixture: f, slot: slot)) }
        }
        return out
    }

    /// The 1-based position of each selected item; the first occurrence wins.
    private static func order(_ selection: [Item], positions: [Int]) -> [Item: Int] {
        var out: [Item: Int] = [:]
        for (i, item) in selection.enumerated() where out[item] == nil {
            out[item] = (positions[safe: i] ?? i) + 1
        }
        return out
    }

    var body: some View {
        let programmer = engine.programmer
        // Display lookups only: 1-based position in the engine's selection order.
        // PRG-55: a fixture button speaks for its whole-fixture item; cells show with VS2-34.
        // UI-33: the grid position the engine reports (0-based), else the index.
        let order: [Item: Int] = Self.order(programmer.selection, positions: programmer.positions)
        // Double outline only while the engine's targets differ from its selection (sub-selection).
        let subSelection = programmer.targets != programmer.selection
        let targets = Set(programmer.targets)
        let inProgrammer = Set(programmer.inProgrammer)

        let sorted = engine.fixtures.sorted(by: Self.handleOrder)
        // UI-77: tiles sit in their handle slots, so a gap is an empty tile. While Move holds a fixture every slot shows,
        // plus one empty page after the last. A fixture without a handle goes after the slots of the last real page.
        let moving = ui.function == .move && ui.moveFixture != nil
        let layout = Self.layout(sorted: sorted, selection: programmer.selection, page: page, moving: moving,
                                 handlesPerPage: handlesPerPage, unfolded: ui.unfolded)
        let pageCount = layout.pageCount, shown = layout.shown, pageOf = layout.pageOf
        let selectedPages = layout.selectedPages, onShown = layout.onShown, rows = layout.rows

        PanelWindow(
            title: "Fixtures",
            info: programmer.selection.isEmpty ? "\(sorted.count) fixtures"
                : onShown == programmer.selection.count ? "\(programmer.selection.count) selected · order shown"
                : "\(programmer.selection.count) selected · \(onShown) on page \(shown + 1)",
            accessory: {
                TextField("Find", text: $find).textFieldStyle(.plain).font(Typo.mono(Typo.small))
                    .frame(width: 110).focused($findFocused)
                    .onSubmit { selectMatches(sorted) }
                    .onChange(of: find) { _, _ in
                        // UI-59: the window goes to the first match's page, in display order, and scrolls it into view.
                        if !find.isEmpty, let m = sorted.first(where: matches) { page = pageOf[m.id] ?? 0; scrollTarget = Item(fixture: m.id) }
                    }
                    .onChange(of: findFocused) { _, now in if !now, ui.fieldEscaped { find = "" } }   // UI-44: Esc empties it
                if pageCount > 1 {
                    HStack(spacing: 2) {
                        ForEach(0..<pageCount, id: \.self) { p in
                            Button(String(p + 1)) { page = p }.buttonStyle(FlatButton(fill: p == shown ? .blue : nil, compact: true))
                                .overlay(alignment: .topTrailing) {
                                    if selectedPages.contains(p) { Circle().fill(p == shown ? Color.white : t.blue).frame(width: 5, height: 5).padding(1) }   // UI-58
                                }
                        }
                    }
                }
            }
        ) {
            ScrollViewReader { proxy in
            CellScroll {
                // UI-29: one button per item, the cells of an unfolded fixture right after it. A flat
                // list, so the lazy grid sees every button as its own element.
                ForEach(rows, id: \.tile) { row in
                    let slot = row.slot
                    if case let .item(item) = row.tile, let fixture = row.fixture {
                        let o: Int? = order[item]
                        let isTaken: Bool = moving && ui.moveFixture == fixture.id && item.cell == nil   // UI-77
                        fixtureTile(fixture, item, order: o, current: subSelection && targets.contains(item),
                                    dimmed: subSelection && o != nil && !targets.contains(item),
                                    inProgrammer: inProgrammer.contains(item), located: programmer.located[item] != nil, taken: isTaken)
                    } else {
                        emptyTile(page: shown + 1, slot: slot)
                    }
                }
            }
            .onChange(of: scrollTarget) { _, t in if let t { proxy.scrollTo(t, anchor: .center); scrollTarget = nil } }
            }
        }
        // UI-58: a selection change that did not come from a tap here brings the first fixture into view.
        // A change of only the tapped item is that tap.
        .onChange(of: programmer.selection) { old, sel in
            guard lastTap.map({ Set(old).symmetricDifference(sel) != [$0] }) ?? true, let first = sel.first, let p = pageOf[first.fixture] else { return }
            page = p
            scrollTarget = Item(fixture: first.fixture)
        }
    }
}

extension FixturesWindow {
    /// UI-59: Return selects the matches in number order; one already selected is skipped (a tap would deselect it).
    fileprivate func selectMatches(_ fixtures: [FixtureInfo]) {
        defer { find = ""; findFocused = false }   // done: the field empties and the keys go back to the console
        guard !find.isEmpty, ui.armedAllows(.fixture, engine) else { return }
        let selected = Set(engine.programmer.selection)
        for f in fixtures.filter(matches).sorted(by: { $0.number < $1.number }) where !selected.contains(Item(fixture: f.id)) {
            engine.selectFixture(f.id)
        }
    }

    /// One whole-fixture tile or one of its cells (UI-29).
    fileprivate func fixtureTile(_ fixture: FixtureInfo, _ item: Item, order: Int?, current: Bool, dimmed: Bool,
                                 inProgrammer: Bool, located: Bool, taken: Bool) -> some View {
        let model: String = item.cell == nil && !fixture.name.isEmpty ? Self.model(fixture) : ""   // UI-59
        let faded: Bool = !matches(fixture) || fixture.parked   // UI-59, PAT-35
        return Cell(number: item.cell == nil ? fixture.number : nil,
             tag: fixture.parked ? "PARK" : item.cell == nil && fixture.cells > 0 ? "\(fixture.cells)" : nil,   // PAT-35
             legend: item.cell.map { "\(fixture.number).\($0)" } ?? Self.legend(fixture),
             model: model,
             order: order,
             located: located,   // UI-26
             selected: order != nil,
             picked: taken,
             current: current,
             dimmed: dimmed,
             inProgrammer: inProgrammer,
             lamp: true,
             lampCell: fixture.parked ? nil : engine.live.cell(item),
             faded: faded) {
            if fixture.parked && ui.function != .move {
                return   // PAT-35: a parked fixture is not selectable (Move can take it, UI-77)
            } else if let n = item.cell {
                lastTap = item
                if ui.armedAllows(.cell, engine) { engine.selectCell(fixture.id, cell: n) }
            } else {
                lastTap = item
                tap(fixture)
            }
        }
        // UI-57: a right-click on a whole-fixture tile opens Fixture info.
        .overlay { if item.cell == nil { SecondaryClick { ui.fixtureInfo = FixtureRef(id: fixture.id) } } }
    }

    /// UI-77: an empty handle slot reacts only while Move is armed.
    fileprivate func emptyTile(page: Int, slot: Int) -> some View {
        let reacts = ui.function == .move
        let action: (() -> Void)? = reacts ? { moveTap(nil, page: page, slot: slot) } : nil
        return Cell(number: nil, empty: true, armed: reacts, action: action)
    }

    /// UI-77: Move on a fixture tile or an empty slot (`fixture` nil): the first fixture tapped is taken, the next tile is the target.
    fileprivate func moveTap(_ fixture: FixtureInfo?, page: Int, slot: Int) {
        guard ui.armedAllows(.fixtureButton, engine) else { return }   // UI-42
        guard let taken = ui.moveFixture else {
            guard let fixture else { return engine.say("Move: tap the fixture to move first.") }
            ui.moveFixture = fixture.id
            return engine.say("Move fixture \(fixture.number): tap an empty button, or a fixture to swap with.")
        }
        if fixture?.id == taken {
            ui.moveFixture = nil
            return engine.say("Move: tap the fixture to move.")
        }
        guard page > 0, slot > 0 else { return }   // a target without a handle cannot happen
        ui.sendPicks(engine) { engine.patchHandle(id: taken, page: page, slot: slot) }
    }

    /// While Unfold is armed a tap folds or unfolds instead of selecting (UI-29), then disarms.
    fileprivate func tap(_ fixture: FixtureInfo) {
        if ui.function == .move { return moveTap(fixture, page: fixture.handle?.page ?? 0, slot: fixture.handle?.slot ?? 0) }   // UI-77
        guard ui.armedAllows(.fixture, engine) else { return }   // UI-42
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
                        Cell(number: id, tag: "\(group.fixtures.count)", legend: group.shownLegend,
                             selected: !group.fixtures.isEmpty && group.fixtures.allSatisfy(selection.contains),
                             picked: ui.isPicked(.group, id),
                             mask: true) { GroupTaps.tap(id, group, engine: engine, ui: ui) }
                    } else {
                        // Empty cells only react while Record or a group function is armed; any other
                        // armed function gets its refusal said (UI-42).
                        let reacts = ui.recordTargets || ui.groupFunctionArmed
                        Cell(number: id, empty: true, armed: reacts,
                             action: ui.recordTargets || ui.function != nil ? { GroupTaps.tap(id, nil, engine: engine, ui: ui) } : nil)
                    }
                }
            }
        }
    }
}

/// UI-46: the armed tap on a group, called by the Groups window and by the command line (`Group n`).
@MainActor
enum GroupTaps {
    /// While armed, the next group tap records instead of selecting.
    static func tap(_ id: Int, _ group: GroupInfo?, engine: EngineClient, ui: UIState) {
        if ui.recordTargets, ui.recordMaster == "Group" {
            // GRM-15: this tap picks the group of a group master; the handle tap records it.
            guard group != nil else { return engine.say("Group Master: tap an existing group.") }
            ui.recordMasterGroup = id
            engine.say("Group Master: tap an empty handle.")
        } else if ui.recordTargets {
            // PRG-97: a used group asks Replace or Merge on the softkeys; a free one records at once.
            guard group != nil else {
                engine.recordGroup(id)
                ui.recordArmed = false
                return
            }
            ui.usedCell = nil
            ui.usedHandle = nil
            ui.usedCue = nil
            ui.maskMenu = false
            ui.usedGroup = id
            engine.say("Group \(id) is used: Replace or Merge on the softkeys.")
        } else if !ui.armedAllows(.group, engine) {
            return
        } else if let function = ui.function, ui.groupFunctionArmed {
            perform(function, id, group, engine: engine, ui: ui)
        } else if group != nil {
            engine.selectGroup(id)
        }
    }

    /// PRG-94: Copy, Move, Delete and Set Legend on a group; the engine refuses a bad target (PRG-89).
    private static func perform(_ function: PaletteFunction, _ id: Int, _ group: GroupInfo?, engine: EngineClient, ui: UIState) {
        switch function {
        case .copy, .move, .delete:   // UI-64
            switch ui.pickTap(.group, item: group != nil ? id : nil, order: engine.groups.map(\.id).sorted()) {
            case .picked: engine.sayPicks(ui)
            case .target:
                ui.sendPicks(engine) {
                    function == .copy ? engine.copyGroups(ui.picks, to: id, layout: ui.layoutWire)
                                      : engine.moveGroups(ui.picks, to: id, layout: ui.layoutWire, swap: ui.swapIfRequired)
                }
            case .empty: engine.say(function == .delete ? "Group \(id) is empty" : "Tap a recorded group as the source.")
            }
        case .legend:
            guard group != nil else { return engine.say("Group \(id) is empty") }
            ui.groupLegendEditing = GroupRef(number: id)
        default:
            break
        }
    }
}

// MARK: - Workspaces (UI-08)

enum WindowKind {
    case groups, fixtures, colours, positions, beams, programmer, picker, cues, active
    /// FX-36: the Shapes workspace.
    case looks, effect, effects

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
        case .looks: LooksWindow()
        case .effect: EffectWindow()
        case .effects: EffectsWindow()
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
        Workspace(name: "Shapes", windows: [.looks, .effect, .fixtures, .effects]),
    ]
    /// FX-36: its index (F5); the effect wheels are on while it shows.
    static let shapes = 4
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
