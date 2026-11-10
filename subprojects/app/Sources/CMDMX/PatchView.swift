import SwiftUI
import UniformTypeIdentifiers

// Patch screen (PAT-10): pure rendering of `engine.fixtures` plus command sending. No overlap
// check, address or number logic lives here; the engine decides and refuses with a warning.

/// UI-72: what the Add strip would exchange the selection to; nil when it cannot be sent.
private struct ExchangeTarget: Equatable {
    let key: String
    let label: String
    let mode: String
    let universe: Int?
    let address: Int?
}

struct PatchView: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState
    @State private var selection = Set<Int>()
    /// LIB-23: the profile editor covers the screen while set; the Add panel stays alive underneath.
    @State private var editor: ProfileEditorTarget?
    @State private var pendingPick: SavedPick?
    @State private var exchange: ExchangeTarget?
    @State private var clickMonitor: Any?
    /// LIB-35: the report of the last GDTF import, shown until Done.
    @State private var gdtfReport: GdtfImported?
    /// UI-73: the find text, and the Add strip's Hide state (remembered on this Mac).
    @State private var find = ""
    @FocusState private var findFocused: Bool
    @AppStorage("patchAddHidden") private var addHidden = false

    var body: some View {
        ZStack {
            patchScreen
                .background(Button("") { ui.screen = nil }.keyboardShortcut(.cancelAction).frame(width: 0, height: 0).opacity(0))   // UI-44
                .disabled(editor != nil).opacity(editor == nil ? 1 : 0)
            if let editor {
                ProfileEditor(target: editor) { saved in
                    if let saved { pendingPick = saved }
                    self.editor = nil
                }
                .id(editor.key + editor.mode + "\(editor.source)")
            }
        }
        .onChange(of: engine.gdtfImported) { _, done in
            guard let done else { return }
            gdtfReport = done
            pendingPick = SavedPick(manufacturer: done.manufacturer, model: done.model, mode: done.modes.first?.name ?? "")
        }
        .sheet(item: $gdtfReport) { GdtfReportSheet(report: $0) }
    }

    /// UI-73: the rows that match the find field, in number order.
    private var shownRows: [FixtureInfo] {
        engine.fixtures.filter { FixturesWindow.matches($0, find) }.sorted { $0.number < $1.number }
    }

    private var patchScreen: some View {
        let rows = shownRows
        return VStack(spacing: 8) {
            // UI-73: hidden, the panel stays alive so its typed text and a pending pick survive.
            AddPanel(pendingPick: $pendingPick, exchange: $exchange, edit: { editor = $0 }, hide: { addHidden = true })
                .frame(height: addHidden ? 0 : 220).clipped().disabled(addHidden).opacity(addHidden ? 0 : 1)
            HStack(spacing: 8) {
                PanelWindow(title: "Patch", info: find.isEmpty ? "\(rows.count) fixtures" : "\(rows.count) of \(engine.fixtures.count)", framed: true, accessory: {
                    TextField("Find", text: $find).textFieldStyle(.plain).font(Typo.mono(Typo.small))
                        .frame(width: 110).focused($findFocused)
                        .onSubmit { selection = Set(shownRows.map(\.id)) }   // UI-73: Return selects every row shown
                        .onChange(of: findFocused) { _, now in if !now, ui.fieldEscaped { find = "" } }   // UI-44
                    if addHidden { Button("Show Add") { addHidden = false }.buttonStyle(FlatButton(compact: true)) }   // UI-73
                }) {
                    Table(rows, selection: $selection) {
                        TableColumn("No") { Text(String($0.number)).monospacedDigit().opacity($0.parked ? 0.45 : 1) }.width(min: 40, ideal: 50, max: 70)
                        TableColumn("Name") { Text($0.name).opacity($0.parked ? 0.45 : 1) }.width(min: 60, ideal: 120)
                        TableColumn("Fixture") { Text($0.profile).opacity($0.parked ? 0.45 : 1) }.width(min: 100, ideal: 200)
                        TableColumn("Mode") { f in
                            HStack(spacing: 6) {
                                Text(f.virtualDimmer ? "\(f.mode) · virtual dimmer" : f.mode)
                                let tags = [f.behaviour.values.contains { $0.invert } ? "INV" : nil,
                                            f.behaviour.values.contains { $0.limits } ? "LIM" : nil].compactMap { $0 }
                                if !tags.isEmpty { Text(tags.joined(separator: " ")).font(Typo.mono(Typo.small)).foregroundStyle(t.amber) }
                            }.opacity(f.parked ? 0.45 : 1)
                        }
                            .width(min: 100, ideal: 180)
                        TableColumn("Address") {
                            Text(verbatim: $0.parked ? "parked (was \($0.universe).\($0.address))" : "\($0.universe).\($0.address)")
                                .monospacedDigit().opacity($0.parked ? 0.45 : 1)
                        }
                        .width(min: 50, ideal: 70, max: 130)
                        TableColumn("Ch") { Text(String($0.footprint)).monospacedDigit().opacity($0.parked ? 0.45 : 1) }.width(min: 30, ideal: 40, max: 60)
                    }
                }
                VStack(spacing: 8) {
                    SelectedPanel(fixtures: rows.filter { selection.contains($0.id) }, exchange: exchange, edit: { editor = $0 })
                    if let armed = ui.patchSwapFrom {
                        Text("Pick the fixtures to swap with \(swapNumbers(armed))").font(Typo.ui(11)).foregroundStyle(t.amber)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    if let message = engine.lastMessage {
                        Text(message).font(Typo.ui(11)).foregroundStyle(t.amber)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    Spacer(minLength: 0)
                }
                .frame(width: 320)
            }
        }
        .padding(8)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(t.bg)
        .onChange(of: rows.map(\.id)) { _, ids in selection.formIntersection(ids) }   // UI-73: leaving rows leave the selection
        .onChange(of: selection) { old, new in
            // UI-73: only when some of `old` is still shown; a filter that empties the selection must not restore a row.
            if !old.isEmpty, new.isEmpty, !old.isDisjoint(with: rows.map(\.id)), let id = clickedRowID(rows) { selection = [id] }
            engine.clearMessage()
        }
        // A plain click on a row inside a multi-selection keeps only that row, on mouse-down (PAT-25);
        // the onChange above puts it back when the Table empties the selection afterwards.
        .onAppear {
            clickMonitor = NSEvent.addLocalMonitorForEvents(matching: .leftMouseDown) { event in
                if selection.count > 1, let id = clickedRowID(shownRows),
                   selection.contains(id) { selection = [id] }
                return event
            }
        }
        .onDisappear { clickMonitor.map(NSEvent.removeMonitor); clickMonitor = nil; ui.patchSwapFrom = nil }
    }
}

/// macOS 26 SwiftUI `Table` empties a multi-selection on a plain click inside it, and only after the
/// double-click wait, instead of keeping the clicked row at once (PAT-25, VS2-28 run). The id of the row
/// under the mouse, with no Shift/Cmd held; no row dragging here, so acting on mouse-down loses nothing.
/// ponytail: AppKit hit test on the Table's NSTableView, drop when Apple fixes it.
private func clickedRowID(_ rows: [FixtureInfo]) -> Int? {
    guard NSEvent.modifierFlags.intersection([.shift, .command]).isEmpty, let window = NSApp.keyWindow else { return nil }
    let point = window.convertPoint(fromScreen: NSEvent.mouseLocation)
    var v = window.contentView?.hitTest(point)
    while let c = v, !(c is NSTableView) { v = c.superview }
    guard let table = v as? NSTableView else { return nil }
    let r = table.row(at: table.convert(point, from: nil))
    return rows.indices.contains(r) ? rows[r].id : nil
}

/// "" = nil (omitted), a number = that number, anything else = invalid (`.some(nil)` is never produced).
extension PatchView {
    /// UI-75: user numbers of the armed range, "101, 102, 103" or "101 … 108 (8)".
    fileprivate func swapNumbers(_ ids: [Int]) -> String {
        let n = engine.fixtures.filter { ids.contains($0.id) }.map(\.number).sorted()
        guard n.count > 4, let first = n.first, let last = n.last else { return n.map(String.init).joined(separator: ", ") }
        return "\(first) … \(last) (\(n.count))"
    }
}

private func optionalInt(_ text: String) -> Int?? {
    let t = text.trimmingCharacters(in: .whitespaces)
    if t.isEmpty { return .some(nil) }
    return Int(t).map { .some($0) }
}

func labelled<Content: View>(_ label: String, @ViewBuilder _ content: () -> Content) -> some View {
    HStack(spacing: 6) {
        Text(label).font(.caption).foregroundStyle(.secondary).frame(width: 64, alignment: .leading)
        content()
    }
}

/// Library browser (LIB-20..22): search, then Manufacturer / Model / Mode columns, then the patch fields.
/// All lookups are the engine's (`library.manufacturers`, `library.models`, `patch.profile`); this view only
/// holds the selection. Keyboard: Down in the search field enters Manufacturer; Up/Down move within the
/// focused column, Left/Right move between columns (Right selects the first row of an empty Model/Mode
/// column), Return adds when Add is enabled.
private struct AddPanel: View {
    @AppStorage("patchAutoGroups") private var autoGroups = false   // PAT-43
    @EnvironmentObject private var engine: EngineClient
    /// A model saved in the profile editor: shown and picked here once the library lists it (LIB-26).
    @Binding var pendingPick: SavedPick?
    @Binding var exchange: ExchangeTarget?
    let edit: (ProfileEditorTarget) -> Void
    let hide: () -> Void
    /// UI-74: the fixture count when the last Add was sent; a longer list means the engine accepted it.
    @State private var countBeforeAdd: Int?
    @State private var wantedMode: String?
    @State private var query = ""
    @State private var mfr: ManufacturerPick?
    @State private var pick: ModelPick?
    @State private var mode = ""
    @State private var count = "1"
    @State private var universe = "1"
    @State private var address = ""
    @State private var number = ""
    /// UI-75: Duplicate… asks for a name; Delete profile asks twice. `duplicateAs` is the name sent, awaiting `library.saved`.
    @State private var askDuplicate = false
    @State private var duplicateName = ""
    @State private var duplicateAs: String?
    @State private var deleteArmed = false
    @FocusState private var focus: Field?

    private enum Field: Hashable { case search, manufacturer, model, mode }
    private enum ManufacturerPick: Hashable { case show, named(String) }
    /// Manufacturer/model key plus whether it came from the show's own copy.
    private struct ModelPick: Hashable { let key: String; let fromShow: Bool }
    private struct ModelRow: Identifiable {
        let manufacturer: String
        let model: String
        let source: String
        let modes: [LibraryMode]
        let fromShow: Bool
        var pick: ModelPick { ModelPick(key: "\(manufacturer)/\(model)", fromShow: fromShow) }
        var id: ModelPick { pick }
        /// QLC+ and built-in profiles are the stock library; anything else is the user's own.
        var isUser: Bool {
            fromShow ? !source.isEmpty && !source.hasPrefix("qlcplus") && !source.hasPrefix("builtin") : source == "user"
        }
        var sourceLabel: String { source.hasPrefix("builtin") ? "built in" : isUser ? "user profile" : "QLC+" }
    }

    /// Distinct profile keys in the show whose "manufacturer model" text holds every query word.
    private var showKeys: [String] {
        let words = query.lowercased().split(whereSeparator: \.isWhitespace)
        let keys = Set(engine.fixtures.map(\.profile)).filter { key in
            let text = key.replacingOccurrences(of: "/", with: " ").lowercased()
            return words.allSatisfy { text.contains($0) }
        }
        return keys.sorted()
    }

    private var modelRows: [ModelRow] {
        if mfr == .show {
            return showKeys.map { key in
                if let p = engine.profileCache[key] {
                    return ModelRow(manufacturer: p.manufacturer, model: p.model, source: p.source, modes: p.modes, fromShow: true)
                }
                let parts = key.split(separator: "/", maxSplits: 1).map(String.init)
                return ModelRow(manufacturer: parts.first ?? "", model: parts.count > 1 ? parts[1] : key,
                                source: "", modes: [], fromShow: true)
            }
        }
        return engine.libraryModels.map {
            ModelRow(manufacturer: $0.manufacturer, model: $0.model, source: $0.source, modes: $0.modes, fromShow: false)
        }
    }

    private var pickedRow: ModelRow? { modelRows.first { $0.pick == pick } }

    /// UI-72: Universe and Address are sent only when Address is filled in, and then both must parse.
    private var exchangeTarget: ExchangeTarget? {
        guard let picked = pickedRow, !mode.isEmpty, let a = optionalInt(address) else { return nil }
        var u: Int?
        if a != nil {
            guard let uv = Int(universe) else { return nil }
            u = uv
        }
        return ExchangeTarget(key: picked.pick.key, label: picked.model, mode: mode, universe: u, address: a)
    }

    /// The Manufacturer column's choices in display order.
    private var mfrChoices: [ManufacturerPick] {
        (showKeys.isEmpty ? [] : [.show]) + engine.libraryManufacturers.map { .named($0.name) }
    }

    var body: some View {
        let picked = pickedRow
        let countValue = Int(count), universeValue = Int(universe)
        let addressValue = optionalInt(address), numberValue = optionalInt(number)
        let canAdd = picked != nil && !mode.isEmpty && countValue != nil && universeValue != nil
            && addressValue != nil && numberValue != nil

        PanelWindow(title: "Add", framed: true, accessory: { Button("Hide", action: hide).buttonStyle(FlatButton(compact: true)) }) {
            VStack(alignment: .leading, spacing: 6) {
                TextField("Search library, e.g. showtec par 56", text: $query)
                    .textFieldStyle(.roundedBorder)
                    .focused($focus, equals: .search)
                    .onSubmit { if canAdd { add() } }
                    .onKeyPress(.downArrow) {
                        focus = .manufacturer
                        if mfr == nil { mfr = mfrChoices.first }
                        return .handled
                    }
                HStack(spacing: 6) {
                    column("Manufacturer", count: engine.libraryManufacturers.count, field: .manufacturer, selected: mfr) {
                        if !showKeys.isEmpty {
                            row(mfr == .show, id: ManufacturerPick.show, tap: { toggle(.show) }) {
                                Text("In this show").lineLimit(1)
                                Spacer(minLength: 4)
                                Text(String(showKeys.count)).font(Typo.mono(10)).foregroundStyle(.secondary)
                            }
                        }
                        ForEach(engine.libraryManufacturers, id: \.name) { m in
                            row(mfr == .named(m.name), id: ManufacturerPick.named(m.name), tap: { toggle(.named(m.name)) }) {
                                Text(m.name).lineLimit(1)
                                Spacer(minLength: 4)
                                Text(String(m.models)).font(Typo.mono(10)).foregroundStyle(.secondary)
                            }
                        }
                    }
                    column("Model", count: modelRows.count, field: .model, selected: pick) {
                        ForEach(modelRows) { r in
                            row(pick == r.pick, id: r.pick, tap: { pick = r.pick }) {
                                if mfr == nil || mfr == .show {
                                    Text(r.manufacturer).foregroundStyle(.secondary).lineLimit(1)
                                }
                                Text(r.model).lineLimit(1)
                                if r.isUser { Text("USER").font(.system(size: 8, weight: .bold)).foregroundStyle(.secondary) }
                                Spacer(minLength: 4)
                                Text(String(r.modes.count)).font(Typo.mono(10)).foregroundStyle(.secondary)
                            }
                        }
                        if modelRows.isEmpty { Text("No match").font(.caption).foregroundStyle(.secondary).padding(6) }
                        if engine.libraryModelsTruncated && mfr != .show {
                            Text("truncated, refine the search").font(.caption).foregroundStyle(.secondary).padding(6)
                        }
                    }
                    column("Mode", count: picked?.modes.count ?? 0, field: .mode, selected: mode) {
                        ForEach(picked?.modes ?? [], id: \.name) { m in
                            row(mode == m.name, id: m.name, tap: { mode = m.name }) {
                                Text(m.name).lineLimit(1)
                                Spacer(minLength: 4)
                                Text(verbatim: "\(m.channels) ch").font(Typo.mono(10)).foregroundStyle(.secondary)
                            }
                        }
                    }
                }
                HStack(spacing: 8) {
                    VStack(alignment: .leading, spacing: 0) {
                        Text(picked.map { "\($0.manufacturer) \($0.model)" } ?? "Pick a model").lineLimit(1)
                        if let picked, let m = picked.modes.first(where: { $0.name == mode }) {
                            Text(verbatim: "\(m.name) · \(m.channels) channels · \(picked.sourceLabel)")
                                .font(.caption).foregroundStyle(.secondary).lineLimit(1)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    field("Count", $count, "1")
                    field("Universe", $universe, "1")
                    field("Address", $address, "next free")
                    field("Number", $number, "next free")
                    Button("Edit profile…") { editPicked() }.buttonStyle(FlatButton()).disabled(picked == nil)
                    if deleteArmed, let picked {
                        Text("Delete user profile \(picked.model)? Shows that use it keep their copy.")
                            .font(.caption).foregroundStyle(.red).lineLimit(2)
                        Button("Delete") {
                            engine.libraryDeleteProfile(manufacturer: picked.manufacturer, model: picked.model)
                            deleteArmed = false
                        }.buttonStyle(FlatButton(fill: .red))
                        Button("Cancel") { deleteArmed = false }.buttonStyle(FlatButton())
                    } else {
                        Button("Duplicate…") { duplicateName = ""; askDuplicate = true }.buttonStyle(FlatButton()).disabled(picked == nil)
                        if let picked, picked.isUser, !picked.fromShow {
                            Button("Delete profile") { deleteArmed = true }.buttonStyle(FlatButton())
                        }
                    }
                    Button("New from channel list…") { edit(ProfileEditorTarget(source: .channelList)) }.buttonStyle(FlatButton())
                    Button("Import GDTF…") { importGdtf() }.buttonStyle(FlatButton())
                    Toggle("Auto groups", isOn: $autoGroups).toggleStyle(.checkbox)
                    Button("Add") { add() }.buttonStyle(FlatButton(fill: .blue)).disabled(!canAdd)
                }
            }
            .padding(8)
        }
        .onAppear { engine.requestManufacturers(query: query) }
        .alert("Duplicate \(picked?.model ?? "")", isPresented: $askDuplicate) {
            TextField("New model name", text: $duplicateName)
            Button("Duplicate") {
                let name = duplicateName.trimmingCharacters(in: .whitespaces)
                guard let picked, !name.isEmpty else { return }
                duplicateAs = name
                engine.libraryDuplicateProfile(manufacturer: picked.manufacturer, model: picked.model, newModel: name)
            }
            Button("Cancel", role: .cancel) {}
        }
        // The copy is listed and picked like a model saved in the editor (LIB-26).
        .onChange(of: engine.profileSaved) { _, s in
            guard let s, let name = duplicateAs, s.model == name else { return }
            duplicateAs = nil
            pendingPick = SavedPick(manufacturer: s.manufacturer, model: s.model, mode: mode)
        }
        // LIB-41: the list is asked again, as after a save.
        .onChange(of: engine.profileDeleted) { _, _ in
            pick = nil
            engine.requestManufacturers(query: query)
            refreshModels()
        }
        .onChange(of: pick) { _, _ in deleteArmed = false }
        // ponytail: a warning can accompany an accepted Add, so only a longer list counts; a refused Add followed by someone else's Add also empties them.
        .onChange(of: engine.fixtures.count) { _, n in   // UI-74: an accepted Add starts over
            if let before = countBeforeAdd, n > before { number = ""; address = ""; countBeforeAdd = nil }
        }
        .onChange(of: query) { _, _ in
            pick = nil
            engine.requestManufacturers(query: query)
            refreshModels()
        }
        .onChange(of: mfr) { _, _ in pick = nil; refreshModels() }
        .onChange(of: engine.libraryManufacturers) { _, list in
            // A selected manufacturer that left the list is dropped.
            if case let .named(name) = mfr, !list.contains(where: { $0.name == name }) { mfr = nil }
        }
        .onAppear { exchange = exchangeTarget }
        .onChange(of: exchangeTarget) { _, target in exchange = target }
        .onChange(of: showKeys) { _, keys in
            guard mfr == .show else { return }
            // A type patched meanwhile needs its show copy too.
            if keys.isEmpty { mfr = nil } else { keys.filter { engine.profileCache[$0] == nil }.forEach(engine.requestProfile) }
        }
        .onChange(of: pick) { _, _ in
            let first = pickedRow?.modes.first?.name ?? ""
            mode = wantedMode.flatMap { w in pickedRow?.modes.contains { $0.name == w } == true ? w : nil } ?? first
            wantedMode = nil
        }
        .onChange(of: pendingPick) { _, p in
            guard let p else { return }
            if query.isEmpty, mfr == .named(p.manufacturer) {
                engine.requestManufacturers(query: "")
                refreshModels()
                applyPending()
            } else {
                query = ""
                mfr = .named(p.manufacturer)
                engine.requestManufacturers(query: "")
                refreshModels()
            }
        }
        .onChange(of: engine.libraryModels) { _, _ in applyPending() }
        .onChange(of: picked?.modes) { _, modes in
            // The show copy's modes arrive later (patch.profile); keep the mode valid.
            if let modes, !modes.contains(where: { $0.name == mode }) { mode = modes.first?.name ?? "" }
        }
    }

    /// Picks the saved model (and its mode) once the library lists it.
    private func applyPending() {
        guard let p = pendingPick,
              let row = modelRows.first(where: { $0.manufacturer == p.manufacturer && $0.model == p.model }) else { return }
        wantedMode = p.mode
        pendingPick = nil
        if pick == row.pick {
            mode = row.modes.contains { $0.name == p.mode } ? p.mode : (row.modes.first?.name ?? "")
            wantedMode = nil
        } else {
            pick = row.pick
        }
    }

    /// LIB-23: the show's copy when the show carries the profile, else the library's.
    private func editPicked() {
        guard let picked = pickedRow else { return }
        let inShow = engine.fixtures.contains { $0.profile == picked.pick.key }
        edit(ProfileEditorTarget(source: inShow ? .show : .library, manufacturer: picked.manufacturer,
                                 model: picked.model, mode: mode))
    }

    private func importGdtf() {
        let panel = NSOpenPanel()
        if let gdtf = UTType(filenameExtension: "gdtf") { panel.allowedContentTypes = [gdtf] }
        panel.allowsMultipleSelection = false
        guard panel.runModal() == .OK, let url = panel.url else { return }
        engine.importGdtf(path: url.path)
    }

    private func add() {
        guard let picked = pickedRow, let c = Int(count), let u = Int(universe),
              case let .some(a) = optionalInt(address), case let .some(n) = optionalInt(number) else { return }
        engine.patchAdd(profile: picked.pick.key, mode: mode, count: c, universe: u, address: a, number: n, autoGroups: autoGroups)
        countBeforeAdd = engine.fixtures.count   // UI-74
    }

    /// Clicking the selected manufacturer again deselects it.
    private func toggle(_ m: ManufacturerPick) { mfr = mfr == m ? nil : m }

    private func refreshModels() {
        if mfr == .show {
            showKeys.forEach { engine.requestProfile(key: $0) }
        } else if case let .named(name) = mfr {
            engine.requestModels(manufacturer: name, query: query)
        } else {
            engine.requestModels(manufacturer: nil, query: query)
        }
    }

    private func field(_ label: String, _ text: Binding<String>, _ prompt: String) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(label).font(.caption).foregroundStyle(.secondary)
            TextField(prompt, text: text).textFieldStyle(.roundedBorder).frame(width: label == "Count" ? 50 : 76)
        }
    }

    private func row<C: View>(_ selected: Bool, id: AnyHashable, tap: @escaping () -> Void, @ViewBuilder _ content: () -> C) -> some View {
        HStack(spacing: 4) { content() }
            .font(.caption)
            .padding(.horizontal, 6).padding(.vertical, 2)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(selected ? Color.accentColor.opacity(0.35) : Color.clear)
            .contentShape(Rectangle())
            .onTapGesture(perform: tap)
            .id(id)
    }

    /// `selected` is the row id kept scrolled into view, so arrow keys never leave the selection off screen.
    private func column<C: View>(_ title: String, count: Int, field f: Field, selected: AnyHashable?, @ViewBuilder _ rows: () -> C) -> some View {
        VStack(spacing: 0) {
            HStack {
                Text(title).font(.caption).foregroundStyle(.secondary)
                Spacer()
                Text(String(count)).font(Typo.mono(10)).foregroundStyle(.secondary)
            }
            .padding(.horizontal, 6).padding(.vertical, 2)
            let content = rows()
            ScrollViewReader { proxy in
                ScrollView { LazyVStack(spacing: 0) { content } }
                    .onChange(of: selected) { _, id in if let id { proxy.scrollTo(id) } }
            }
        }
        .overlay(RoundedRectangle(cornerRadius: radius)
            .strokeBorder(focus == f ? Color.accentColor : Color.secondary.opacity(0.3), lineWidth: hairline))
        .focusable()
        .focusEffectDisabled()
        .focused($focus, equals: f)
        .onKeyPress(.upArrow) { step(-1, f); return .handled }
        .onKeyPress(.downArrow) { step(1, f); return .handled }
        .onKeyPress(.leftArrow) { focus = f == .mode ? .model : .manufacturer; return .handled }
        .onKeyPress(.rightArrow) { enter(f); return .handled }
        .onKeyPress(.return) { add(); return .handled }
        .simultaneousGesture(TapGesture().onEnded { focus = f })
    }

    /// Up/Down inside a column: move the selection one row, starting at the first row when nothing is selected.
    private func step(_ delta: Int, _ f: Field) {
        func moved<T: Equatable>(_ items: [T], _ current: T?) -> T? {
            guard !items.isEmpty else { return nil }
            guard let current, let i = items.firstIndex(of: current) else { return items[0] }
            return items[min(max(i + delta, 0), items.count - 1)]
        }
        switch f {
        case .manufacturer: mfr = moved(mfrChoices, mfr)
        case .model: pick = moved(modelRows.map(\.pick), pick)
        case .mode: if let m = moved(pickedRow?.modes.map(\.name) ?? [], mode) { mode = m }
        case .search: break
        }
    }

    /// Right: move focus to the next column, selecting its first row when it has no selection.
    private func enter(_ f: Field) {
        switch f {
        case .manufacturer:
            focus = .model
            if pick == nil { pick = modelRows.first?.pick }
        case .model:
            if pickedRow != nil { focus = .mode }
        default: break
        }
    }
}

private struct SelectedPanel: View {
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState
    @State private var copyNumber = ""
    /// The selection in number order (PAT-25); every action sends all their ids (IPC-40).
    let fixtures: [FixtureInfo]
    let exchange: ExchangeTarget?
    let edit: (ProfileEditorTarget) -> Void
    @State private var universe = ""
    @State private var address = ""
    @State private var number = ""
    @State private var name = ""
    @State private var mode = ""
    @State private var showBehaviour = false
    /// Which destructive button has been clicked once; reset when the selection changes.
    @State private var armed: Armed?
    /// The last Move sent, so a refused overlap can be resent with `park_conflicting` (PAT-35).
    @State private var lastMove: (ids: [Int], universe: Int, address: Int)?

    private enum Armed { case remove, mode, exchange }

    private var ids: [Int] { fixtures.map(\.id) }
    /// The current mode when every selected fixture has the same one, else "".
    private var sharedMode: String {
        guard let first = fixtures.first, fixtures.allSatisfy({ $0.mode == first.mode }) else { return "" }
        return first.mode
    }
    /// Modes every selected fixture has, in the first fixture's order.
    private var commonModes: [String] {
        guard let first = fixtures.first else { return [] }
        return first.modes.filter { m in fixtures.allSatisfy { $0.modes.contains(m) } }
    }
    private var sharedProfile: Bool { fixtures.dropFirst().allSatisfy { $0.profile == fixtures.first?.profile } }

    var body: some View {
        let title = fixtures.count > 1 ? "Selected · \(fixtures.count) fixtures"
            : fixtures.first.map { "Selected · \($0.number)" } ?? "Selected"
        PanelWindow(title: title, framed: true, accessory: { EmptyView() }) {
            if let fixture = fixtures.first {
                VStack(alignment: .leading, spacing: 6) {
                    labelled("Move") {
                        TextField("univ", text: $universe).textFieldStyle(.roundedBorder).frame(width: 50)
                        TextField("address", text: $address).textFieldStyle(.roundedBorder)
                        Button("Move") {
                            if let u = Int(universe), let a = Int(address) {
                                lastMove = (ids, u, a)
                                engine.patchMove(ids: ids, universe: u, address: a)
                            }
                        }.buttonStyle(FlatButton())
                    }
                    if engine.lastMessage?.contains("overlap") == true, let m = lastMove {
                        Button("Park conflicting and move") {
                            engine.patchMove(ids: m.ids, universe: m.universe, address: m.address, parkConflicting: true)
                            lastMove = nil
                        }.buttonStyle(FlatButton())
                    }
                    labelled("Park") {
                        Button("Park") { lastMove = nil; engine.patchPark(ids: fixtures.filter { !$0.parked }.map(\.id)) }
                            .buttonStyle(FlatButton()).disabled(fixtures.allSatisfy(\.parked))
                        Button("Unpark") { lastMove = nil; engine.patchUnpark(ids: fixtures.filter(\.parked).map(\.id)) }
                            .buttonStyle(FlatButton()).disabled(!fixtures.contains(where: \.parked))
                    }
                    // UI-75: copies start parked; an empty start number is the next free one (PAT-40).
                    labelled("Copy") {
                        TextField("start no", text: $copyNumber).textFieldStyle(.roundedBorder)
                        Button("Copy") {
                            guard let start = optionalInt(copyNumber) else { return }
                            engine.patchCopy(ids: ids, number: start)
                            copyNumber = ""
                        }.buttonStyle(FlatButton())
                    }
                    labelled("Behaviour") {
                        Button("Behaviour…") { showBehaviour = true }.buttonStyle(FlatButton())
                    }
                    // PAT-42: only when the whole selection shares one profile; the engine refuses a misfit.
                    labelled("Library") {
                        Button("Update from library") {
                            engine.patchUpdateProfile(profile: fixture.profile)
                        }.buttonStyle(FlatButton()).disabled(!sharedProfile)
                    }
                    // UI-75: armed on the first range; a new selection and Confirm send both (PAT-41).
                    labelled("Swap") {
                        if let from = ui.patchSwapFrom {
                            Button("Confirm swap") { engine.patchSwapAddresses(ids: from, with: ids); ui.patchSwapFrom = nil }
                                .buttonStyle(FlatButton(fill: .red)).disabled(ids == from)
                            Button("Clear") { ui.patchSwapFrom = nil }.buttonStyle(FlatButton())
                        } else {
                            Button("Swap addresses") { ui.patchSwapFrom = ids }.buttonStyle(FlatButton())
                        }
                    }
                    labelled("Renumber") {
                        TextField("number", text: $number).textFieldStyle(.roundedBorder)
                        Button("Set") {
                            if let n = Int(number) { engine.patchRenumber(ids: ids, number: n) }
                        }.buttonStyle(FlatButton())
                    }
                    labelled("Name") {
                        TextField("name", text: $name).textFieldStyle(.roundedBorder)
                            .onSubmit { engine.patchName(ids: ids, name: name) }
                        Button("Set") { engine.patchName(ids: ids, name: name) }.buttonStyle(FlatButton())
                    }
                    if fixtures.count > 1 {
                        Text("Par 1 numbers them Par 1, Par 2, …").font(.caption).foregroundStyle(.secondary)
                    }
                    labelled("Mode") {
                        Picker("", selection: $mode) {
                            if !commonModes.contains(mode) { Text("").tag(mode) }
                            ForEach(commonModes, id: \.self) { Text($0).tag($0) }
                        }
                        .labelsHidden()
                        Button(armed == .mode ? "Confirm change" : "Change") {
                            if armed == .mode {
                                engine.patchChangeMode(ids: ids, mode: mode)
                                armed = nil
                            } else {
                                armed = .mode
                            }
                        }
                        .buttonStyle(FlatButton(fill: armed == .mode ? .red : nil))
                        .disabled(mode.isEmpty || mode == sharedMode)
                    }
                    // UI-72: two-step, with UI-47's usage line; sends the Add strip's pick.
                    Button(armed == .exchange ? "Confirm exchange" : exchange.map { "Exchange to \($0.label) · \($0.mode)" } ?? "Exchange") {
                        guard let x = exchange else { return }
                        if armed == .exchange {
                            engine.patchExchange(ids: ids, profile: x.key, mode: x.mode, universe: x.universe, address: x.address)
                            armed = nil
                        } else {
                            armed = .exchange
                            engine.askUsage(fixtures: ids)
                        }
                    }
                    .buttonStyle(FlatButton(fill: armed == .exchange ? .red : nil))
                    .lineLimit(1)
                    .disabled(exchange == nil)
                    usageCaption(for: .exchange)
                    Button("Edit profile…") { edit(.show(key: fixture.profile, mode: fixture.mode)) }
                        .buttonStyle(FlatButton())
                        .disabled(!sharedProfile)
                    Button(armed == .remove ? "Confirm remove" : fixtures.count > 1 ? "Remove \(fixtures.count) fixtures" : "Remove") {
                        if armed == .remove {
                            engine.patchRemove(ids: ids)
                            armed = nil
                        } else {
                            armed = .remove
                            engine.askUsage(fixtures: ids)
                        }
                    }.buttonStyle(FlatButton(fill: armed == .remove ? .red : nil))
                    usageCaption(for: .remove)
                }
                .padding(8)
                .sheet(isPresented: $showBehaviour) { BehaviourSheet(fixtures: fixtures) }
                .onAppear { load(fixture) }
                .onChange(of: ids) { _, _ in load(fixture) }
                .onChange(of: sharedMode) { _, m in mode = m }
                .onChange(of: fixture.address) { _, a in address = "\(a)"; universe = "\(fixture.universe)" }   // UI-75: a swap or undo moved it
                .onChange(of: exchange) { _, _ in if armed == .exchange { armed = nil } }
            } else {
                Text("Select a row").font(.caption).foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity).padding(12)
            }
        }
    }

    /// UI-47's usage line under an armed button.
    @ViewBuilder private func usageCaption(for action: Armed) -> some View {
        if armed == action, let usage = engine.usage, usage.fixtures == ids {
            Text(usage.patchText(withEmpties: action == .remove)).font(.caption).foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    /// Prefill the fields from the engine's values for the first selected fixture.
    private func load(_ f: FixtureInfo) {
        universe = "\(f.universe)"; address = "\(f.address)"; number = "\(f.number)"
        name = f.name; mode = sharedMode; armed = nil; lastMove = nil; copyNumber = ""
    }
}

/// UI-76: per-attribute invert, limits and offset for the selection, and Pan/Tilt swap. The engine does the shaping.
private struct BehaviourSheet: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    let fixtures: [FixtureInfo]

    private var ids: [Int] { fixtures.map(\.id) }

    /// Attributes every selected fixture has (not Colour), by group order then name.
    private var attributes: [String] {
        guard let first = fixtures.first else { return [] }
        func rank(_ g: String) -> Int { attributeGroupOrder.firstIndex(of: g) ?? attributeGroupOrder.count }
        return first.attributes
            .filter { kv in kv.key != "Colour" && fixtures.allSatisfy { $0.attributes[kv.key] != nil } }
            .sorted { a, b in (rank(a.value), a.key) < (rank(b.value), b.key) }
            .map(\.key)
    }

    /// The value every selected fixture shares for `attribute` (missing entry = default), else nil.
    private func shared<V: Equatable>(_ attribute: String, _ value: (FixtureBehaviour) -> V, default d: V) -> V? {
        let values = fixtures.map { $0.behaviour[attribute].map(value) ?? d }
        guard let first = values.first, values.allSatisfy({ $0 == first }) else { return nil }
        return first
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Behaviour · \(fixtures.count == 1 ? "fixture \(fixtures.first?.number ?? 0)" : "\(fixtures.count) fixtures")").font(.headline)
            ScrollView {
                VStack(alignment: .leading, spacing: 6) {
                    HStack(spacing: 8) {
                        Text("").frame(width: 90)
                        Text("Invert").frame(width: 44, alignment: .leading)
                        ForEach(["Low %", "High %", "Offset %"], id: \.self) { Text($0).frame(width: 56, alignment: .trailing) }
                    }.font(.caption).foregroundStyle(.secondary)
                    ForEach(attributes, id: \.self) { a in
                        HStack(spacing: 8) {
                            Text(a).frame(width: 90, alignment: .leading).lineLimit(1)
                            Toggle("Invert", isOn: Binding(
                                get: { shared(a, \.invert, default: false) == true },
                                set: { engine.patchBehaviour(ids: ids, attribute: a, invert: $0) }))
                                .toggleStyle(.checkbox).labelsHidden().frame(width: 44, alignment: .leading)
                            PercentField(label: "Low", value: shared(a, \.low, default: 0)) { engine.patchBehaviour(ids: ids, attribute: a, low: $0) }
                            PercentField(label: "High", value: shared(a, \.high, default: 1)) { engine.patchBehaviour(ids: ids, attribute: a, high: $0) }
                            PercentField(label: "Offset", value: shared(a, \.offset, default: 0)) { engine.patchBehaviour(ids: ids, attribute: a, offset: $0) }
                        }
                    }
                }.frame(maxWidth: .infinity, alignment: .leading)
            }.frame(maxHeight: 360)
            if fixtures.allSatisfy({ $0.attributes["Pan"] != nil && $0.attributes["Tilt"] != nil }) {
                Toggle("Swap Pan and Tilt", isOn: Binding(
                    get: { fixtures.allSatisfy(\.swapPanTilt) },
                    set: { engine.patchSwapPanTilt(ids: ids, swapped: $0) }))
            }
            // Always one line, so a refusal does not squeeze the rows: the sheet keeps its height.
            Text(engine.lastMessage ?? " ").font(Typo.ui(11)).foregroundStyle(t.amber)
            HStack {
                Button("Reset") { engine.patchBehaviourReset(ids: ids) }
                    .buttonStyle(FlatButton()).disabled(!fixtures.contains { !$0.behaviour.isEmpty })
                Spacer()
                Button("Done") { dismiss() }   // no .defaultAction: Return commits a field
            }
        }
        // Escape closes like Done; zero-size rather than .hidden(), which disables the shortcut.
        .background(Button("") { dismiss() }.keyboardShortcut(.cancelAction).frame(width: 0, height: 0).opacity(0))
        .padding(16)
        .frame(width: 460)
    }

    /// A percent field: the shared value × 100, empty when the selection differs; commits on Return and focus loss.
    private struct PercentField: View {
        let label: String
        let value: Double?
        let commit: (Double) -> Void
        @State private var text = ""
        @FocusState private var focused: Bool

        private var shown: String {
            guard let value else { return "" }
            return String(format: "%g", (value * 1000).rounded() / 10)
        }

        var body: some View {
            TextField(label, text: $text)
                .textFieldStyle(.roundedBorder).frame(width: 56).multilineTextAlignment(.trailing)
                .focused($focused)
                .help(label + " (%)")
                .onAppear { text = shown }
                .onChange(of: value) { _, _ in text = shown }
                .onSubmit(send)
                .onChange(of: focused) { _, f in if !f { send() } }
        }

        private func send() {
            guard let n = Double(text.trimmingCharacters(in: .whitespaces)), text != shown else { return }
            commit(n / 100)
        }
    }
}

/// LIB-35: what the GDTF import made of every channel, then its warnings.
private struct GdtfReportSheet: View {
    @Environment(\.dismiss) private var dismiss
    let report: GdtfImported

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("\(report.manufacturer) \(report.model)").font(.headline)
            if report.replaced { Text("Replaced the existing user profile.").font(.caption).foregroundStyle(.secondary) }
            Table(report.report.channels.enumerated().map { Row(id: $0.offset, line: $0.element) }) {
                TableColumn("Mode") { Text($0.line.mode).help($0.line.mode) }.width(min: 60, ideal: 90)
                TableColumn("Ch") { Text(String($0.line.channel)).monospacedDigit() }.width(min: 30, ideal: 36, max: 50)
                TableColumn("GDTF name") { Text($0.line.sourceName).help($0.line.sourceName) }.width(min: 80, ideal: 140)
                TableColumn("Became") { Text($0.line.became).lineLimit(nil).fixedSize(horizontal: false, vertical: true).help($0.line.became) }.width(min: 120, ideal: 240)
            }
            if !report.report.warnings.isEmpty {
                Text("Warnings").font(.subheadline)
                ScrollView {
                    VStack(alignment: .leading, spacing: 2) {
                        ForEach(Array(report.report.warnings.enumerated()), id: \.offset) { Text($0.element).font(.caption) }
                    }.frame(maxWidth: .infinity, alignment: .leading)
                }.frame(maxHeight: 90)
            }
            HStack { Spacer(); Button("Done") { dismiss() }.keyboardShortcut(.defaultAction) }
        }
        // Escape closes like Done; zero-size rather than .hidden(), which disables the shortcut.
        .background(Button("") { dismiss() }.keyboardShortcut(.cancelAction).frame(width: 0, height: 0).opacity(0))
        .padding(16)
        .frame(width: 640, height: 420)
    }

    private struct Row: Identifiable { let id: Int; let line: GdtfImported.Line }
}
