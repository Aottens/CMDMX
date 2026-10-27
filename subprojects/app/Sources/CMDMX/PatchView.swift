import SwiftUI
import UniformTypeIdentifiers

// Patch screen (PAT-10): pure rendering of `engine.fixtures` plus command sending. No overlap
// check, address or number logic lives here; the engine decides and refuses with a warning.

struct PatchView: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @State private var selection = Set<Int>()
    /// LIB-23: the profile editor covers the screen while set; the Add panel stays alive underneath.
    @State private var editor: ProfileEditorTarget?
    @State private var pendingPick: SavedPick?
    @State private var clickMonitor: Any?
    /// LIB-35: the report of the last GDTF import, shown until Done.
    @State private var gdtfReport: GdtfImported?

    var body: some View {
        ZStack {
            patchScreen.disabled(editor != nil).opacity(editor == nil ? 1 : 0)
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

    private var patchScreen: some View {
        let rows = engine.fixtures.sorted { $0.number < $1.number }
        return VStack(spacing: 8) {
            AddPanel(pendingPick: $pendingPick, edit: { editor = $0 }).frame(height: 220)
            HStack(spacing: 8) {
                PanelWindow(title: "Patch", info: "\(rows.count) fixtures", framed: true, accessory: { EmptyView() }) {
                    Table(rows, selection: $selection) {
                        TableColumn("No") { Text(String($0.number)).monospacedDigit() }.width(min: 40, ideal: 50, max: 70)
                        TableColumn("Name") { Text($0.name) }.width(min: 60, ideal: 120)
                        TableColumn("Fixture") { Text($0.profile) }.width(min: 100, ideal: 200)
                        TableColumn("Mode") { Text($0.virtualDimmer ? "\($0.mode) · virtual dimmer" : $0.mode) }
                            .width(min: 100, ideal: 180)
                        TableColumn("Address") { Text(verbatim: "\($0.universe).\($0.address)").monospacedDigit() }
                            .width(min: 50, ideal: 70, max: 90)
                        TableColumn("Ch") { Text(String($0.footprint)).monospacedDigit() }.width(min: 30, ideal: 40, max: 60)
                    }
                }
                VStack(spacing: 8) {
                    SelectedPanel(fixtures: rows.filter { selection.contains($0.id) }, edit: { editor = $0 })
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
        .onChange(of: selection) { old, new in
            if !old.isEmpty, new.isEmpty, let id = clickedRowID(rows) { selection = [id] }
            engine.clearMessage()
        }
        // A plain click on a row inside a multi-selection keeps only that row, on mouse-down (PAT-25);
        // the onChange above puts it back when the Table empties the selection afterwards.
        .onAppear {
            clickMonitor = NSEvent.addLocalMonitorForEvents(matching: .leftMouseDown) { event in
                if selection.count > 1, let id = clickedRowID(engine.fixtures.sorted { $0.number < $1.number }),
                   selection.contains(id) { selection = [id] }
                return event
            }
        }
        .onDisappear { clickMonitor.map(NSEvent.removeMonitor); clickMonitor = nil }
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
    @EnvironmentObject private var engine: EngineClient
    /// A model saved in the profile editor: shown and picked here once the library lists it (LIB-26).
    @Binding var pendingPick: SavedPick?
    let edit: (ProfileEditorTarget) -> Void
    @State private var wantedMode: String?
    @State private var query = ""
    @State private var mfr: ManufacturerPick?
    @State private var pick: ModelPick?
    @State private var mode = ""
    @State private var count = "1"
    @State private var universe = "1"
    @State private var address = ""
    @State private var number = ""
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
        /// QLC+ profiles are the stock library; anything else is the user's own.
        var isUser: Bool { fromShow ? !source.isEmpty && !source.hasPrefix("qlcplus") : source == "user" }
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

        PanelWindow(title: "Add", framed: true, accessory: { EmptyView() }) {
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
                            Text(verbatim: "\(m.name) · \(m.channels) channels · \(picked.isUser ? "user profile" : "QLC+")")
                                .font(.caption).foregroundStyle(.secondary).lineLimit(1)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    field("Count", $count, "1")
                    field("Universe", $universe, "1")
                    field("Address", $address, "next free")
                    field("Number", $number, "next free")
                    Button("Edit profile…") { editPicked() }.buttonStyle(FlatButton()).disabled(picked == nil)
                    Button("New from channel list…") { edit(ProfileEditorTarget(source: .channelList)) }.buttonStyle(FlatButton())
                    Button("Import GDTF…") { importGdtf() }.buttonStyle(FlatButton())
                    Button("Add") { add() }.buttonStyle(FlatButton(fill: .blue)).disabled(!canAdd)
                }
            }
            .padding(8)
        }
        .onAppear { engine.requestManufacturers(query: query) }
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
        engine.patchAdd(profile: picked.pick.key, mode: mode, count: c, universe: u, address: a, number: n)
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
    /// The selection in number order (PAT-25); every action sends all their ids (IPC-40).
    let fixtures: [FixtureInfo]
    let edit: (ProfileEditorTarget) -> Void
    @State private var universe = ""
    @State private var address = ""
    @State private var number = ""
    @State private var name = ""
    @State private var mode = ""
    /// Which destructive button has been clicked once; reset when the selection changes.
    @State private var armed: Armed?

    private enum Armed { case remove, mode }

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
                            if let u = Int(universe), let a = Int(address) { engine.patchMove(ids: ids, universe: u, address: a) }
                        }.buttonStyle(FlatButton())
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
                    Button("Edit profile…") { edit(.show(key: fixture.profile, mode: fixture.mode)) }
                        .buttonStyle(FlatButton())
                        .disabled(!sharedProfile)
                    Button(armed == .remove ? "Confirm remove" : fixtures.count > 1 ? "Remove \(fixtures.count) fixtures" : "Remove") {
                        if armed == .remove {
                            engine.patchRemove(ids: ids)
                            armed = nil
                        } else {
                            armed = .remove
                        }
                    }.buttonStyle(FlatButton(fill: armed == .remove ? .red : nil))
                }
                .padding(8)
                .onAppear { load(fixture) }
                .onChange(of: ids) { _, _ in load(fixture) }
                .onChange(of: sharedMode) { _, m in mode = m }
            } else {
                Text("Select a row").font(.caption).foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity).padding(12)
            }
        }
    }

    /// Prefill the fields from the engine's values for the first selected fixture.
    private func load(_ f: FixtureInfo) {
        universe = "\(f.universe)"; address = "\(f.address)"; number = "\(f.number)"
        name = f.name; mode = sharedMode; armed = nil
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
