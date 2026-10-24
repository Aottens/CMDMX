import AppKit
import SwiftUI

// Profile editor (LIB-23..27, IPC-37..39). A thin client: it edits a local copy of the engine's profile
// document and sends it back on Save. Overlaps, footprints and used attributes are the engine's to check;
// it refuses with a warning that shows here. Test sends ordinary `programmer.set` values.

/// What the editor opens on. `show` = the show's copy (`patch.profile`), `library` = `library.profile`,
/// `channelList` = a new profile from a LIB-11 channel list (manufacturer/model/mode are typed).
struct ProfileEditorTarget: Equatable {
    enum Source { case show, library, channelList }
    var source: Source
    var manufacturer = ""
    var model = ""
    var mode = ""
    var key: String { "\(manufacturer)/\(model)" }

    /// The show's copy of a patched profile key `manufacturer/model`.
    static func show(key: String, mode: String) -> ProfileEditorTarget {
        let parts = key.split(separator: "/", maxSplits: 1).map(String.init)
        return ProfileEditorTarget(source: .show, manufacturer: parts.first ?? "", model: parts.count > 1 ? parts[1] : "", mode: mode)
    }
}

/// What the Add panel picks after a Save.
struct SavedPick: Equatable {
    let manufacturer: String
    let model: String
    let mode: String
}

private let groups = ["I", "P", "C", "G", "B", "E", "S", "FX"]

struct ProfileEditor: View {
    let target: ProfileEditorTarget
    let onClose: (SavedPick?) -> Void

    var body: some View {
        if target.source == .channelList {
            ChannelListEditor(onClose: onClose)
        } else {
            ProfileDocEditor(target: target, onClose: onClose)
        }
    }
}

// MARK: - Colour conversion (sRGB in the picker, linear in the profile; CoreGraphics does the curve)

private func swiftUIColour(_ c: TargetColourDoc) -> Color {
    guard let lin = CGColorSpace(name: CGColorSpace.linearSRGB), let srgb = CGColorSpace(name: CGColorSpace.sRGB),
          let cg = CGColor(colorSpace: lin, components: [CGFloat(c.r), CGFloat(c.g), CGFloat(c.b), 1]),
          let out = cg.converted(to: srgb, intent: .defaultIntent, options: nil) else {
        return Color(red: c.r, green: c.g, blue: c.b)
    }
    return Color(cgColor: out)
}

private func targetColour(_ colour: Color) -> TargetColourDoc? {
    guard let lin = CGColorSpace(name: CGColorSpace.linearSRGB),
          let out = NSColor(colour).cgColor.converted(to: lin, intent: .defaultIntent, options: nil),
          let comps = out.components, comps.count >= 3 else { return nil }
    func unit(_ v: CGFloat) -> Double { min(max(Double(v), 0), 1) }
    return TargetColourDoc(r: unit(comps[0]), g: unit(comps[1]), b: unit(comps[2]))
}

// MARK: - Shared pieces

/// Slider plus typed percent plus the DMX value; `optional` adds a checkbox for "none".
private struct LevelRow: View {
    @Environment(\.theme) private var t
    let label: String
    let optional: Bool
    @Binding var value: Double?

    var body: some View {
        HStack(spacing: 6) {
            Text(label).font(Typo.ui(11)).foregroundStyle(t.fg2).frame(width: 64, alignment: .leading)
            if optional {
                Toggle("", isOn: Binding(get: { value != nil }, set: { value = $0 ? (value ?? 0) : nil }))
                    .labelsHidden().toggleStyle(.checkbox)
            }
            if let v = value {
                Slider(value: Binding(get: { v }, set: { value = $0 }), in: 0...1)
                TextField("", value: Binding(get: { v * 100 }, set: { value = min(max($0, 0), 100) / 100 }),
                          format: .number.precision(.fractionLength(0...1)))
                    .textFieldStyle(.roundedBorder).frame(width: 56)
                Text("%").font(Typo.ui(11)).foregroundStyle(t.fg2)
                Text("= \(Int((v * 255).rounded()))").font(Typo.mono(Typo.small)).foregroundStyle(t.fg2)
                    .frame(width: 50, alignment: .leading)
            } else {
                Text("none").font(Typo.ui(11)).foregroundStyle(t.fg3)
                Spacer(minLength: 0)
            }
        }
    }
}

private struct GroupTag: View {
    let group: String
    var body: some View {
        Text(group).font(Typo.mono(Typo.small, .semibold))
            .foregroundStyle(Theme.groupHue(group))
    }
}

/// "201, 203–205": consecutive numbers collapse to a range.
private func rangeText(_ numbers: [Int]) -> String {
    var runs: [String] = []
    var i = 0
    let sorted = numbers.sorted()
    while i < sorted.count {
        var j = i
        while j + 1 < sorted.count, sorted[j + 1] == sorted[j] + 1 { j += 1 }
        runs.append(j > i ? "\(sorted[i])–\(sorted[j])" : "\(sorted[i])")
        i = j + 1
    }
    return runs.joined(separator: ", ")
}

// MARK: - Edit an existing profile

private struct ProfileDocEditor: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    let target: ProfileEditorTarget
    let onClose: (SavedPick?) -> Void

    @State private var doc = ProfileDoc(manufacturer: "", model: "")
    /// nil while the profile is still loading.
    @State private var original: ProfileDoc?
    @State private var modeName = ""
    @State private var selCh: Int?
    @State private var selSlot: UUID?
    @State private var testOn = false
    @State private var askDiscard = false
    @State private var saving = false

    private struct Row: Identifiable {
        let id: Int
        let channel: Int
        let entry: ChannelEntry?
    }

    // MARK: Document access

    private var modeIndex: Int? { doc.modes.firstIndex { $0.name == modeName } }
    private var mode: ModeDoc? { modeIndex.map { doc.modes[$0] } }
    private var dirty: Bool { original != nil && doc != original }

    private func edit(_ change: (inout ModeDoc) -> Void) {
        guard let i = modeIndex else { return }
        change(&doc.modes[i])
    }

    private func attribute(_ m: ModeDoc, _ e: ChannelEntry) -> AttributeDoc? {
        switch e.ref {
        case let .attr(i): return m.attributes[safe: i]
        case let .cellAttr(c, i): return m.cells[safe: c]?.attributes[safe: i]
        case .mixing: return nil
        }
    }

    private func role(_ m: ModeDoc, _ e: ChannelEntry) -> String? {
        guard case let .mixing(cell, i) = e.ref else { return nil }
        let mix: ColourMixingDoc?
        if let cell { mix = m.cells[safe: cell]?.colour } else { mix = m.colour }
        return mix?.roles[safe: i]
    }

    private func cellPrefix(_ e: ChannelEntry) -> String {
        switch e.ref {
        case let .cellAttr(c, _): return "Cell \(c + 1) · "
        case let .mixing(cell?, _): return "Cell \(cell + 1) · "
        default: return ""
        }
    }

    private func label(_ m: ModeDoc, _ e: ChannelEntry) -> String {
        if let a = attribute(m, e) { return cellPrefix(e) + (e.fine ? "fine of \(a.name)" : a.name) }
        if let r = role(m, e) { return cellPrefix(e) + (e.fine ? "fine of \(r) (colour)" : "\(r) (colour)") }
        return "?"
    }

    private func rows(_ m: ModeDoc) -> [Row] {
        let entries = m.entries
        var out: [Row] = []
        let top = max(m.footprint, entries.map(\.channel).max() ?? 0)
        guard top >= 1 else { return out }
        for ch in 1...top {
            let here = entries.filter { $0.channel == ch }
            if here.isEmpty {
                out.append(Row(id: out.count, channel: ch, entry: nil))
            } else {
                for e in here { out.append(Row(id: out.count, channel: ch, entry: e)) }
            }
        }
        return out
    }

    private var selectedEntry: ChannelEntry? {
        guard let selCh, let m = mode else { return nil }
        return m.entries.first { $0.channel == selCh }
    }

    private func attrBinding<V>(_ i: Int, _ kp: WritableKeyPath<AttributeDoc, V>, _ fallback: V) -> Binding<V> {
        Binding(get: { mode?.attributes[safe: i]?[keyPath: kp] ?? fallback },
                set: { v in edit { m in if m.attributes.indices.contains(i) { m.attributes[i][keyPath: kp] = v } } })
    }

    // MARK: Test (LIB-27)

    private var testNumbers: [Int] {
        let selected = Set(engine.programmer.selection)
        return engine.fixtures.filter { $0.profile == target.key && selected.contains($0.id) }.map(\.number).sorted()
    }

    private func testSend(_ attribute: String, _ value: Double) {
        if testOn { engine.programmerSet(attribute: attribute, value: value) }
    }

    /// Default / Locate / Highlight of the attribute at `i`; a change also goes to the programmer while Test is on.
    private func levelBinding(_ i: Int, _ kp: WritableKeyPath<AttributeDoc, Double?>) -> Binding<Double?> {
        Binding(get: { mode?.attributes[safe: i]?[keyPath: kp] ?? nil },
                set: { v in
                    edit { m in if m.attributes.indices.contains(i) { m.attributes[i][keyPath: kp] = v } }
                    if let v, let name = mode?.attributes[safe: i]?.name { testSend(name, v) }
                })
    }

    private func defaultBinding(_ i: Int) -> Binding<Double?> {
        Binding(get: { mode?.attributes[safe: i]?.defaultLevel },
                set: { v in
                    guard let v else { return }
                    edit { m in if m.attributes.indices.contains(i) { m.attributes[i].defaultLevel = v } }
                    if let name = mode?.attributes[safe: i]?.name { testSend(name, v) }
                })
    }

    // MARK: Channel edits (document only)

    private func addChannel() {
        edit { m in
            let n = m.footprint + 1
            m.attributes.append(AttributeDoc(name: "Channel \(n)", group: "S", kind: "Ltp", channel: n))
            m.footprint = n
        }
        selCh = mode?.footprint
        selSlot = nil
    }

    private func removeChannel() {
        guard let ch = selCh else { return }
        edit { m in
            guard let e = m.entries.first(where: { $0.channel == ch }), case let .attr(i) = e.ref,
                  m.attributes.indices.contains(i) else { return }
            var freed = [ch]
            if e.fine {
                m.attributes[i].fineChannel = nil
            } else {
                if let f = m.attributes[i].fineChannel { freed.append(f) }
                m.attributes.remove(at: i)
            }
            for c in freed.sorted(by: >) { m.mapChannels { $0 > c ? $0 - 1 : $0 } }
            m.footprint = max(0, m.footprint - freed.count)
        }
        selCh = nil
        selSlot = nil
    }

    private func moveChannel(_ delta: Int) {
        guard let a = selCh, let footprint = mode?.footprint else { return }
        let b = a + delta
        guard b >= 1, b <= footprint else { return }
        edit { m in m.mapChannels { $0 == a ? b : ($0 == b ? a : $0) } }
        selCh = b
    }

    // MARK: Slot edits (LIB-24)

    private func slots(_ i: Int) -> [SlotDoc] { mode?.attributes[safe: i]?.slots ?? [] }

    /// Applies `change` to the slots of attribute `i` and stores them sorted by From (ties keep their order).
    private func editSlots(_ i: Int, _ change: (inout [SlotDoc]) -> Void) {
        edit { m in
            guard m.attributes.indices.contains(i) else { return }
            var list = m.attributes[i].slots
            change(&list)
            m.attributes[i].slots = list.enumerated()
                .sorted { ($0.element.from, $0.offset) < ($1.element.from, $1.offset) }.map(\.element)
        }
    }

    private func selectedSlotIndex(_ i: Int) -> Int? {
        guard let selSlot else { return nil }
        return slots(i).firstIndex { $0.id == selSlot }
    }

    private func addSlot(_ i: Int) {
        if slots(i).isEmpty {
            let s = SlotDoc(name: "Slot 1", from: 0, to: 255)
            editSlots(i) { $0 = [s] }
            selSlot = s.id
        } else {
            splitSlot(i)
        }
    }

    private func splitSlot(_ i: Int) {
        guard let k = selectedSlotIndex(i) else { return }
        let s = slots(i)[k]
        guard s.to > s.from else { return }
        let mid = (s.from + s.to) / 2
        editSlots(i) { list in
            list[k].to = mid
            let names = Set(list.map(\.name))
            let n = (1...).first { !names.contains("Slot \($0)") } ?? list.count + 1
            list.insert(SlotDoc(name: "Slot \(n)", from: mid + 1, to: s.to), at: k + 1)
        }
    }

    private func mergeSlot(_ i: Int) {
        guard let k = selectedSlotIndex(i), k + 1 < slots(i).count else { return }
        editSlots(i) { list in
            list[k].to = list[k + 1].to
            list.remove(at: k + 1)
        }
    }

    /// The range below takes the freed values; the first range hands them to the next one.
    private func removeSlot(_ i: Int) {
        guard let k = selectedSlotIndex(i) else { return }
        editSlots(i) { list in
            let gone = list.remove(at: k)
            if k > 0 {
                list[k - 1].to = gone.to
            } else if !list.isEmpty {
                list[0].from = gone.from
            }
        }
        selSlot = nil
    }

    private func slotBinding<V>(_ i: Int, _ id: UUID, _ kp: WritableKeyPath<SlotDoc, V>, _ fallback: V) -> Binding<V> {
        Binding(get: { slots(i).first { $0.id == id }?[keyPath: kp] ?? fallback },
                set: { v in editSlots(i) { list in if let k = list.firstIndex(where: { $0.id == id }) { list[k][keyPath: kp] = v } } })
    }

    private func clampedBinding(_ i: Int, _ id: UUID, _ kp: WritableKeyPath<SlotDoc, Int>) -> Binding<Int> {
        let inner = slotBinding(i, id, kp, 0)
        return Binding(get: { inner.wrappedValue }, set: { inner.wrappedValue = min(max($0, 0), 255) })
    }

    private func colourBinding(_ i: Int, _ id: UUID) -> Binding<Color> {
        Binding(get: { slots(i).first { $0.id == id }?.colour.map(swiftUIColour) ?? .white },
                set: { c in
                    guard let tc = targetColour(c) else { return }
                    editSlots(i) { list in if let k = list.firstIndex(where: { $0.id == id }) { list[k].colour = tc } }
                })
    }

    /// Clicking a slot selects it; with Test on the channel goes to the slot's middle value.
    private func pickSlot(_ i: Int, _ s: SlotDoc) {
        selSlot = s.id
        if let name = mode?.attributes[safe: i]?.name { testSend(name, Double((s.from + s.to) / 2) / 255) }
    }

    // MARK: View

    var body: some View {
        PanelWindow(title: "Patch › Profile editor", info: dirty ? "unsaved changes" : "", framed: true, accessory: { EmptyView() }) {
            VStack(spacing: 0) {
                headerBar
                HLine()
                if original == nil {
                    Text(engine.lastMessage ?? "Loading…").font(Typo.ui(11)).foregroundStyle(t.fg2)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else if let m = mode {
                    HStack(spacing: 0) {
                        channelTable(m).frame(minWidth: 420, idealWidth: 520)
                        VLine()
                        rightPane(m).frame(minWidth: 380, maxWidth: .infinity)
                    }
                    if let message = engine.lastMessage {
                        HLine()
                        Text(message).font(Typo.ui(11)).foregroundStyle(t.amber)
                            .frame(maxWidth: .infinity, alignment: .leading).padding(8)
                    }
                } else {
                    Text("This profile has no modes").font(Typo.ui(11)).foregroundStyle(t.fg2)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
        }
        .padding(8)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(t.bg)
        .onAppear {
            engine.clearEditorProfile()
            engine.clearMessage()
            if target.source == .library {
                engine.requestLibraryProfile(manufacturer: target.manufacturer, model: target.model)
            } else {
                engine.requestShowProfile(key: target.key)
            }
        }
        .onChange(of: engine.editorProfile) { _, p in
            guard original == nil, let p, p.key == target.key else { return }
            doc = p.doc
            original = p.doc
            modeName = p.doc.modes.contains { $0.name == target.mode } ? target.mode : (p.doc.modes.first?.name ?? "")
        }
        .onChange(of: modeName) { _, _ in selCh = nil; selSlot = nil }
        .onChange(of: testNumbers.isEmpty) { _, empty in if empty { testOn = false } }
        .onChange(of: engine.profileSaved) { _, s in
            guard saving, let s, s.manufacturer == doc.manufacturer, s.model == doc.model else { return }
            engine.say("Saved \(s.model)")
            onClose(SavedPick(manufacturer: s.manufacturer, model: s.model, mode: modeName))
        }
        .confirmationDialog("Discard changes?", isPresented: $askDiscard) {
            Button("Yes, discard", role: .destructive) { onClose(nil) }
            Button("No", role: .cancel) {}
        }
    }

    private var headerBar: some View {
        let numbers = testNumbers
        return HStack(spacing: 8) {
            Text("\(doc.manufacturer) · \(doc.model)").font(Typo.ui(Typo.large, .medium)).lineLimit(1)
            Picker("", selection: $modeName) {
                ForEach(doc.modes, id: \.name) { Text($0.name).tag($0.name) }
            }
            .labelsHidden().frame(width: 170).disabled(original == nil)
            Spacer(minLength: 8)
            Button(numbers.isEmpty ? "Test" : "Test on \(rangeText(numbers))") { testOn.toggle() }
                .buttonStyle(FlatButton(fill: testOn ? .red : nil)).disabled(numbers.isEmpty)
            Button("Cancel") { if dirty { askDiscard = true } else { onClose(nil) } }.buttonStyle(FlatButton())
            Button("Save") { saving = true; engine.saveProfile(doc) }
                .buttonStyle(FlatButton(fill: .blue)).disabled(original == nil)
        }
        .padding(8)
    }

    // MARK: Channel table

    private func channelTable(_ m: ModeDoc) -> some View {
        VStack(spacing: 0) {
            HStack(spacing: 6) {
                head("Ch", 34); head("Attribute", nil); head("Grp", 34); head("Kind", 40); head("Default", 56); head("Slots", 40)
            }
            .padding(.horizontal, 8).padding(.vertical, 3)
            HLine()
            ScrollView {
                LazyVStack(spacing: 0) {
                    ForEach(rows(m)) { r in channelRow(m, r) }
                }
            }
            HLine()
            HStack(spacing: 6) {
                Button("+") { addChannel() }.buttonStyle(FlatButton())
                Button("−") { removeChannel() }.buttonStyle(FlatButton()).disabled(!canRemove(m))
                Button("▲") { moveChannel(-1) }.buttonStyle(FlatButton()).disabled(selCh == nil)
                Button("▼") { moveChannel(1) }.buttonStyle(FlatButton()).disabled(selCh == nil)
                Spacer()
                Text("\(m.footprint) channels").font(Typo.mono(Typo.small)).foregroundStyle(t.fg2)
            }
            .padding(8)
        }
    }

    private func canRemove(_ m: ModeDoc) -> Bool {
        guard let e = selectedEntry, case .attr = e.ref else { return false }
        return true
    }

    private func head(_ text: String, _ width: CGFloat?) -> some View {
        Text(text).font(Typo.ui(Typo.small)).foregroundStyle(t.fg2)
            .frame(width: width, alignment: .leading).frame(maxWidth: width == nil ? .infinity : nil, alignment: .leading)
    }

    private func channelRow(_ m: ModeDoc, _ r: Row) -> some View {
        let selected = selCh == r.channel
        let a = r.entry.flatMap { attribute(m, $0) }
        let coarse = r.entry.map { !$0.fine } ?? false
        let tag: String? = !coarse ? nil : (a?.group ?? (r.entry.flatMap { role(m, $0) } != nil ? "C" : nil))
        return HStack(spacing: 6) {
            Text("\(r.channel)").font(Typo.mono()).frame(width: 34, alignment: .leading)
            Text(r.entry.map { label(m, $0) } ?? "—").lineLimit(1)
                .foregroundStyle(r.entry == nil ? t.fg3 : t.fg).frame(maxWidth: .infinity, alignment: .leading)
            Group {
                if let tag { GroupTag(group: tag) } else { Text("") }
            }.frame(width: 34, alignment: .leading)
            Text(a.map { coarse ? ($0.kind == "Htp" ? "HTP" : "LTP") : "" } ?? "").font(Typo.ui(Typo.small))
                .foregroundStyle(t.fg2).frame(width: 40, alignment: .leading)
            Text(a.map { coarse ? "\(Int(($0.defaultLevel * 100).rounded())) %" : "" } ?? "").font(Typo.mono(Typo.small))
                .frame(width: 56, alignment: .leading)
            Text(a.map { coarse && !$0.slots.isEmpty ? "\($0.slots.count)" : "" } ?? "").font(Typo.mono(Typo.small))
                .frame(width: 40, alignment: .leading)
        }
        .font(Typo.ui(11))
        .padding(.horizontal, 8).padding(.vertical, 3)
        .background(selected ? t.blue.opacity(0.3) : Color.clear)
        .contentShape(Rectangle())
        .onTapGesture { if r.entry != nil { selCh = r.channel; selSlot = nil } }
    }

    // MARK: Right pane

    @ViewBuilder
    private func rightPane(_ m: ModeDoc) -> some View {
        if let e = selectedEntry {
            ScrollView {
                VStack(alignment: .leading, spacing: 10) {
                    properties(m, e)
                    if case let .attr(i) = e.ref, !e.fine { slotSection(i) }
                }
                .padding(10)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        } else {
            Text("Select a channel").font(Typo.ui(11)).foregroundStyle(t.fg2)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    @ViewBuilder
    private func properties(_ m: ModeDoc, _ e: ChannelEntry) -> some View {
        if e.fine {
            let owner = attribute(m, e)?.name ?? role(m, e).map { "\($0) (colour)" } ?? "?"
            Text("Channel \(e.channel): fine of \(owner)").font(Typo.ui(Typo.large, .medium))
            Text("The fine byte has no fields of its own.").font(Typo.ui(11)).foregroundStyle(t.fg2)
        } else if case let .attr(i) = e.ref, let a = m.attributes[safe: i] {
            Text("Channel \(e.channel)").font(Typo.ui(Typo.large, .medium))
            labelled("Name") { TextField("", text: attrBinding(i, \.name, "")).textFieldStyle(.roundedBorder) }
            labelled("Group") {
                Picker("", selection: attrBinding(i, \.group, "S")) {
                    ForEach(groups, id: \.self) { Text($0).tag($0) }
                }
                .labelsHidden().frame(width: 90).disabled(a.emitter)
                if a.emitter { Text("emitter").font(Typo.ui(Typo.small)).foregroundStyle(t.fg2) }
            }
            labelled("Kind") {
                Picker("", selection: attrBinding(i, \.kind, "Ltp")) {
                    Text("HTP").tag("Htp")
                    Text("LTP").tag("Ltp")
                }
                .labelsHidden().pickerStyle(.segmented).frame(width: 120).disabled(a.emitter)
            }
            LevelRow(label: "Default", optional: false, value: defaultBinding(i))
            LevelRow(label: "Locate", optional: true, value: levelBinding(i, \.locate))
            LevelRow(label: "Highlight", optional: true, value: levelBinding(i, \.highlight))
        } else if let r = role(m, e) {
            Text("Channel \(e.channel): \(r) (colour component)").font(Typo.ui(Typo.large, .medium))
            Text("Part of the colour mixing system; the role stays as it is.").font(Typo.ui(11)).foregroundStyle(t.fg2)
        } else if let a = attribute(m, e) {
            Text("Channel \(e.channel): \(label(m, e))").font(Typo.ui(Typo.large, .medium))
            Text("\(a.group) · \(a.kind == "Htp" ? "HTP" : "LTP") · cells are read-only here.")
                .font(Typo.ui(11)).foregroundStyle(t.fg2)
        }
    }

    // MARK: Slots

    @ViewBuilder
    private func slotSection(_ i: Int) -> some View {
        let list = slots(i)
        let group = mode?.attributes[safe: i]?.group ?? ""
        let k = selectedSlotIndex(i)
        Divider()
        Text("Slots").font(Typo.ui(Typo.body, .semibold))
        VStack(spacing: 2) {
            HStack(spacing: 6) {
                Text("From").frame(width: 56, alignment: .leading)
                Text("To").frame(width: 56, alignment: .leading)
                Text("Name").frame(maxWidth: .infinity, alignment: .leading)
                if group == "C" { Text("Colour").frame(width: 80, alignment: .leading) }
            }
            .font(Typo.ui(Typo.small)).foregroundStyle(t.fg2)
            ForEach(list) { s in
                HStack(spacing: 6) {
                    TextField("", value: clampedBinding(i, s.id, \.from), format: .number)
                        .textFieldStyle(.roundedBorder).frame(width: 56)
                    TextField("", value: clampedBinding(i, s.id, \.to), format: .number)
                        .textFieldStyle(.roundedBorder).frame(width: 56)
                    TextField("", text: slotBinding(i, s.id, \.name, "")).textFieldStyle(.roundedBorder)
                    if group == "C" { colourCell(i, s) }
                }
                .padding(2)
                .background(s.id == selSlot ? t.blue.opacity(0.3) : Color.clear)
                .contentShape(Rectangle())
                .onTapGesture { pickSlot(i, s) }
            }
            if list.isEmpty { Text("No slots").font(Typo.ui(11)).foregroundStyle(t.fg3).padding(4) }
        }
        slotBar(i, list, group)
        HStack(spacing: 6) {
            Button("+ Slot") { addSlot(i) }.buttonStyle(FlatButton()).disabled(!list.isEmpty && k == nil)
            Button("Split") { splitSlot(i) }.buttonStyle(FlatButton()).disabled(k.map { list[$0].to <= list[$0].from } ?? true)
            Button("Merge with next") { mergeSlot(i) }.buttonStyle(FlatButton()).disabled(k.map { $0 + 1 >= list.count } ?? true)
            Button("Remove") { removeSlot(i) }.buttonStyle(FlatButton()).disabled(k == nil)
        }
    }

    @ViewBuilder
    private func colourCell(_ i: Int, _ s: SlotDoc) -> some View {
        HStack(spacing: 4) {
            if s.colour != nil {
                ColorPicker("", selection: colourBinding(i, s.id), supportsOpacity: false).labelsHidden()
                Button("×") { editSlots(i) { list in if let k = list.firstIndex(where: { $0.id == s.id }) { list[k].colour = nil } } }
                    .buttonStyle(FlatButton(compact: true))
            } else {
                Button("Set") {
                    editSlots(i) { list in
                        if let k = list.firstIndex(where: { $0.id == s.id }) { list[k].colour = TargetColourDoc(r: 1, g: 1, b: 1) }
                    }
                }
                .buttonStyle(FlatButton(compact: true))
            }
        }
        .frame(width: 80, alignment: .leading)
    }

    /// 0–255 with each range drawn; clicking a range selects its row.
    private func slotBar(_ i: Int, _ list: [SlotDoc], _ group: String) -> some View {
        VStack(spacing: 2) {
            GeometryReader { geo in
                ZStack(alignment: .topLeading) {
                    Rectangle().fill(t.surf)
                    ForEach(Array(list.enumerated()), id: \.element.id) { n, s in
                        let x = CGFloat(s.from) / 256 * geo.size.width
                        let w = max(2, CGFloat(s.to - s.from + 1) / 256 * geo.size.width)
                        Rectangle()
                            .fill(s.colour.map(swiftUIColour) ?? Theme.groupHue(group).opacity(n % 2 == 0 ? 0.55 : 0.8))
                            .frame(width: w)
                            .overlay(Rectangle().strokeBorder(s.id == selSlot ? t.fg : t.line2, lineWidth: s.id == selSlot ? 2 : hairline))
                            .offset(x: x)
                            .onTapGesture { pickSlot(i, s) }
                    }
                }
            }
            .frame(height: 30)
            .overlay(Rectangle().strokeBorder(t.line, lineWidth: hairline))
            HStack {
                Text("0"); Spacer(); Text("255")
            }
            .font(Typo.mono(Typo.small)).foregroundStyle(t.fg2)
        }
    }
}

// MARK: - New profile from a channel list (LIB-26)

private enum ChannelDraft: Equatable {
    case plain(String)
    case emitter(String)
    case other(name: String, group: String, level: Double)

    var label: String {
        switch self {
        case let .plain(n): return n
        case let .emitter(n): return "Emitter · \(n)"
        case let .other(n, g, d): return "Other · \(n) (\(g), \(Int((d * 100).rounded())) %)"
        }
    }

    /// The LIB-11 / LIB-14 wire form.
    var json: Any {
        switch self {
        case let .plain(n): return n
        case let .emitter(n): return ["Emitter": n]
        case let .other(n, g, d): return ["Other": ["name": n, "group": g, "default": d]]
        }
    }
}

private let plainChannelTypes = [
    "Dimmer", "Dimmer fine", "Red", "Green", "Blue", "White", "Warm White", "Cold White", "Amber", "Lime", "UV",
    "Indigo", "Cyan", "Magenta", "Yellow", "Pan", "Pan fine", "Tilt", "Tilt fine", "Strobe", "Zoom", "Focus", "Colour Macro",
]

private struct ChannelListEditor: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    let onClose: (SavedPick?) -> Void

    private enum Pending { case emitter, other }

    @State private var manufacturer = ""
    @State private var model = ""
    @State private var modeName = ""
    @State private var channels: [ChannelDraft] = []
    @State private var selected: Int?
    @State private var pending: Pending?
    @State private var pendingName = ""
    @State private var pendingGroup = "S"
    @State private var pendingLevel = 0.0
    @State private var askDiscard = false
    @State private var saving = false

    private var dirty: Bool { !(manufacturer + model + modeName).isEmpty || !channels.isEmpty }
    private var trimmed: (String, String, String) {
        let w = CharacterSet.whitespaces
        return (manufacturer.trimmingCharacters(in: w), model.trimmingCharacters(in: w), modeName.trimmingCharacters(in: w))
    }

    private func append(_ c: ChannelDraft) {
        channels.append(c)
        selected = channels.count - 1
    }

    private func move(_ d: Int) {
        guard let s = selected, channels.indices.contains(s + d) else { return }
        channels.swapAt(s, s + d)
        selected = s + d
    }

    private func remove() {
        guard let s = selected, channels.indices.contains(s) else { return }
        channels.remove(at: s)
        selected = nil
    }

    private func confirmPending() {
        let name = pendingName.trimmingCharacters(in: .whitespaces)
        switch pending {
        case .emitter: append(.emitter(name))
        case .other: append(.other(name: name, group: pendingGroup, level: pendingLevel / 100))
        case nil: break
        }
        pending = nil
    }

    private func ask(_ p: Pending) {
        pendingName = ""; pendingGroup = "S"; pendingLevel = 0
        pending = p
    }

    var body: some View {
        PanelWindow(title: "Patch › Profile editor", info: "new from channel list", framed: true, accessory: { EmptyView() }) {
            VStack(spacing: 0) {
                HStack(spacing: 8) {
                    TextField("Manufacturer", text: $manufacturer).textFieldStyle(.roundedBorder)
                    TextField("Model", text: $model).textFieldStyle(.roundedBorder)
                    TextField("Mode name", text: $modeName).textFieldStyle(.roundedBorder).frame(width: 160)
                    Button("Cancel") { if dirty { askDiscard = true } else { onClose(nil) } }.buttonStyle(FlatButton())
                    Button("Save") {
                        let (m, mo, md) = trimmed
                        saving = true
                        engine.saveChannelList(manufacturer: m, model: mo, mode: md, channels: channels.map(\.json))
                    }
                    .buttonStyle(FlatButton(fill: .blue))
                }
                .padding(8)
                HLine()
                if let p = pending { pendingForm(p); HLine() }
                list
                if let message = engine.lastMessage {
                    HLine()
                    Text(message).font(Typo.ui(11)).foregroundStyle(t.amber)
                        .frame(maxWidth: .infinity, alignment: .leading).padding(8)
                }
            }
        }
        .padding(8)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(t.bg)
        .onAppear { engine.clearMessage() }
        .onChange(of: engine.profileSaved) { _, s in
            let (m, mo, md) = trimmed
            guard saving, let s, s.manufacturer == m, s.model == mo else { return }
            engine.say("Saved \(s.model)")
            onClose(SavedPick(manufacturer: m, model: mo, mode: md))
        }
        .confirmationDialog("Discard changes?", isPresented: $askDiscard) {
            Button("Yes, discard", role: .destructive) { onClose(nil) }
            Button("No", role: .cancel) {}
        }
    }

    private func pendingForm(_ p: Pending) -> some View {
        HStack(spacing: 8) {
            Text(p == .emitter ? "Emitter name" : "Other channel").font(Typo.ui(11)).foregroundStyle(t.fg2)
            TextField("Name", text: $pendingName).textFieldStyle(.roundedBorder).frame(width: 180)
            if p == .other {
                Picker("", selection: $pendingGroup) { ForEach(groups, id: \.self) { Text($0).tag($0) } }
                    .labelsHidden().frame(width: 80)
                Text("Default").font(Typo.ui(11)).foregroundStyle(t.fg2)
                TextField("", value: $pendingLevel, format: .number.precision(.fractionLength(0...1)))
                    .textFieldStyle(.roundedBorder).frame(width: 56)
                Text("%").font(Typo.ui(11)).foregroundStyle(t.fg2)
            }
            Button("Add") { confirmPending() }.buttonStyle(FlatButton(fill: .blue))
                .disabled(pendingName.trimmingCharacters(in: .whitespaces).isEmpty)
            Button("Cancel") { pending = nil }.buttonStyle(FlatButton())
            Spacer(minLength: 0)
        }
        .padding(8)
    }

    private var list: some View {
        VStack(spacing: 0) {
            ScrollView {
                LazyVStack(spacing: 0) {
                    ForEach(Array(channels.enumerated()), id: \.offset) { n, c in
                        HStack(spacing: 6) {
                            Text("\(n + 1)").font(Typo.mono()).frame(width: 34, alignment: .leading)
                            Text(c.label).lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
                        }
                        .font(Typo.ui(11))
                        .padding(.horizontal, 8).padding(.vertical, 3)
                        .background(selected == n ? t.blue.opacity(0.3) : Color.clear)
                        .contentShape(Rectangle())
                        .onTapGesture { selected = n }
                    }
                    if channels.isEmpty {
                        Text("No channels yet. + adds one.").font(Typo.ui(11)).foregroundStyle(t.fg3).padding(10)
                    }
                }
            }
            HLine()
            HStack(spacing: 6) {
                Menu("+") {
                    ForEach(plainChannelTypes, id: \.self) { type in Button(type) { append(.plain(type)) } }
                    Divider()
                    Button("Emitter…") { ask(.emitter) }
                    Button("Other…") { ask(.other) }
                }
                .menuStyle(.borderlessButton).fixedSize()
                Button("−") { remove() }.buttonStyle(FlatButton()).disabled(selected == nil)
                Button("▲") { move(-1) }.buttonStyle(FlatButton()).disabled(selected == nil)
                Button("▼") { move(1) }.buttonStyle(FlatButton()).disabled(selected == nil)
                Spacer()
                Text("\(channels.count) channels").font(Typo.mono(Typo.small)).foregroundStyle(t.fg2)
            }
            .padding(8)
        }
    }
}
