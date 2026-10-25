import AppKit
import SwiftUI
import UniformTypeIdentifiers

// Control screen (CTL-41): pure rendering of `engine.control` and the shown mapping, plus command
// sending. Which edits are valid is the engine's call; a refused edit lands in `engine.lastMessage`
// and the fields snap back to what the engine reports.

struct ControlView: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState
    @State private var selected: String?
    /// The mapping whose bindings are shown.
    @State private var mappingName: String?

    var body: some View {
        VStack(spacing: 8) {
            topBar
            HStack(alignment: .top, spacing: 8) {
                VStack(spacing: 8) {
                    DevicesPanel(selected: $selected)
                    if let deck = engine.control.devices.first(where: { $0.name == selected && $0.isDeck }) {
                        DeckPanel(device: deck)
                    }
                    OSCPanel()
                }
                .frame(width: 540)
                BindingsPanel(mappingName: $mappingName)
            }
        }
        .padding(8)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(t.bg)
        .onAppear {
            // Only a mapping that exists: the OSC mapping is made by the first OSC Learn (CTL-35).
            let known = Set(engine.control.mappings.map(\.name))
            mappingName = mappingName ?? [engine.control.osc.mapping, engine.control.devices.first?.mapping]
                .compactMap { $0 }.first(where: known.contains) ?? engine.control.mappings.first?.name
            engine.showMapping(mappingName)
        }
        .onDisappear { engine.showMapping(nil) }
        .onChange(of: mappingName) { _, name in engine.showMapping(name) }
        // A row selects its mapping for the bindings table.
        .onChange(of: selected) { _, name in
            if let mapping = engine.control.devices.first(where: { $0.name == name })?.mapping { mappingName = mapping }
        }
        // The shown mapping was deleted, or none was shown while offline: fall back to the first one.
        .onChange(of: engine.control.mappings) { _, list in
            if !list.contains(where: { $0.name == mappingName }) { mappingName = list.first?.name }
        }
    }

    private var topBar: some View {
        let c = engine.control
        return HStack(spacing: 10) {
            Text("LAST INPUT").font(Typo.legend(11, .semibold)).tracking(0.7).foregroundStyle(t.fg2)
            Text(c.lastInput.map { "\($0.device) · \($0.input) · \($0.value)" } ?? "—").font(Typo.mono(Typo.body)).foregroundStyle(t.fg)
            Spacer(minLength: 0)
            if let message = engine.lastMessage {
                Text(message).font(Typo.ui(11)).foregroundStyle(t.amber).lineLimit(1)
            }
            if let armed = c.learn {
                Text("Learn: \(armed) — move a control").font(Typo.ui(11)).foregroundStyle(t.amber)
                Button("Cancel") { engine.controlLearnCancel() }.buttonStyle(FlatButton(compact: true))
            }
            // CTL-42: the main screen's controls arm Learn instead of acting while this is on.
            Button(ui.learnMode ? "Learn mode: on" : "Learn mode") { ui.setLearnMode(!ui.learnMode, engine) }
                .buttonStyle(FlatButton(fill: ui.learnMode ? .blue : nil))
                .help("Then click a fader, handle button, Go key, page, GM or programmer key on the main screen · Esc ends")
        }
        .frame(height: 24)
    }
}

// MARK: - Devices (CTL-41, CTL-31)

private struct DevicesPanel: View {
    @EnvironmentObject private var engine: EngineClient
    @Binding var selected: String?

    var body: some View {
        let rows = engine.control.orderedDevices
        let names = rows.map(\.name)
        PanelWindow(title: "Devices", info: "\(rows.count)", framed: true, accessory: { EmptyView() }) {
            // The cells get what they need as plain values: a Picker in a Table cell builds its menu
            // outside the view's environment, where `engine` would trap.
            Table(rows, selection: $selected) {
                TableColumn("Order") { OrderCell(device: $0, names: names, enabled: rows.filter(\.enabled).map(\.name), client: engine) }
                    .width(min: 44, ideal: 46, max: 50)
                TableColumn("Device") { Text($0.name).lineLimit(1) }.width(min: 80, ideal: 110)
                TableColumn("State") { DeviceStateCell(device: $0) }.width(min: 62, ideal: 70, max: 80)
                TableColumn("On") { DeviceEnabledCell(device: $0, client: engine) }.width(min: 26, ideal: 28, max: 34)
                TableColumn("Mapping") { DeviceMappingCell(device: $0, mappings: engine.control.mappings.map(\.name), client: engine) }
                    .width(min: 110, ideal: 130)
                TableColumn("Errors") { Text($0.errors == 0 ? "" : "\($0.errors)").font(Typo.mono(Typo.small)) }.width(min: 30, ideal: 36, max: 44)
            }
        }
    }
}

/// CTL-31: the table is the Mackie unit order; the arrows move an enabled row past the next enabled one.
@MainActor
private struct OrderCell: View {
    let device: ControlDevice
    let names: [String]
    let enabled: [String]
    let client: EngineClient

    var body: some View {
        HStack(spacing: 2) {
            if device.enabled {
                Button("▲") { move(-1) }.disabled(enabled.first == device.name)
                Button("▼") { move(1) }.disabled(enabled.last == device.name)
            }
        }
        .buttonStyle(FlatButton(compact: true))
    }

    private func move(_ step: Int) {
        guard let i = enabled.firstIndex(of: device.name), let other = enabled.indices.contains(i + step) ? enabled[i + step] : nil,
              let a = names.firstIndex(of: device.name), let b = names.firstIndex(of: other) else { return }
        var order = names
        order.swapAt(a, b)
        client.controlOrder(order)
    }
}

private struct DeviceStateCell: View {
    @Environment(\.theme) private var t
    let device: ControlDevice

    var body: some View {
        let color: Color = device.state == .ok ? t.green : device.state == .disabled ? t.fg3 : t.amber
        HStack(spacing: 6) {
            Circle().fill(color).frame(width: 6, height: 6)
            Text(device.state.word).font(.caption).foregroundStyle(.secondary)
        }
    }
}

private struct DeviceEnabledCell: View {
    let device: ControlDevice
    let client: EngineClient

    var body: some View {
        let name = device.name
        Toggle("", isOn: Binding(get: { device.enabled }, set: { client.controlEnable(name, on: $0) }))
            .toggleStyle(.checkbox).labelsHidden()
    }
}

private struct DeviceMappingCell: View {
    let device: ControlDevice
    let mappings: [String]
    let client: EngineClient

    var body: some View {
        let name = device.name, stored = device.mapping
        Picker("", selection: Binding(get: { stored ?? "" }, set: { client.controlAssign(name, mapping: $0) })) {
            if stored == nil { Text("—").tag("") }
            ForEach(mappings, id: \.self) { Text($0).tag($0) }
        }.labelsHidden()
    }
}

// MARK: - OSC (CTL-35, CTL-41)

private struct OSCPanel: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    /// When the app saw each client's `last_heard` change (the value itself is daemon time).
    /// ponytail: shows 0 s at the app's first sight; a daemon-side age would need the daemon clock in the delta.
    @State private var seen: [String: (heard: Double, at: Date)] = [:]

    var body: some View {
        let osc = engine.control.osc
        PanelWindow(title: "OSC", info: osc.errors == 0 ? "" : "\(osc.errors) errors", framed: true, accessory: { EmptyView() }) {
            VStack(alignment: .leading, spacing: 6) {
                labelled("Enabled") {
                    Toggle("", isOn: Binding(get: { osc.enabled }, set: { engine.controlOsc(["enabled": $0]) }))
                        .toggleStyle(.checkbox).labelsHidden()
                }
                labelled("Port") { portField(osc.port, key: "port") }
                labelled("Feedback") { portField(osc.feedbackPort, key: "feedback_port") }
                labelled("Allow") {
                    CommitField(placeholder: "empty = anyone", load: osc.allow.joined(separator: ", ")) { text in
                        engine.controlOsc(["allow": list(text)])
                    }
                }
                labelled("Clients") {
                    CommitField(placeholder: "host:port, …", load: osc.clients.filter(\.configured).map(\.address).joined(separator: ", ")) { text in
                        engine.controlOsc(["clients": list(text)])
                    }
                }
                labelled("Mapping") {
                    let mappings = engine.control.mappings.map(\.name), stored = osc.mapping
                    Picker("", selection: Binding(get: { stored ?? "" }, set: { engine.controlOsc(["mapping": $0]) })) {
                        if stored == nil || !mappings.contains(stored ?? "") { Text(stored ?? "—").tag(stored ?? "") }
                        ForEach(mappings, id: \.self) { Text($0).tag($0) }
                    }.labelsHidden()
                }
                if let error = osc.error {
                    Text(error).font(Typo.ui(11)).foregroundStyle(t.amber)
                }
                heard(osc.clients)
            }
            .padding(8)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .fixedSize(horizontal: false, vertical: true)
    }

    private func portField(_ port: Int, key: String) -> some View {
        CommitField(placeholder: "port", load: "\(port)", width: 70) { text in
            if let n = Int(text) { engine.controlOsc([key: n]) } else { engine.say("A port is a number") }
        }
    }

    private func list(_ text: String) -> [String] {
        text.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
    }

    @ViewBuilder private func heard(_ clients: [OSCClientInfo]) -> some View {
        if !clients.isEmpty {
            TimelineView(.periodic(from: .now, by: 1)) { context in
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(clients) { client in
                        HStack(spacing: 8) {
                            Text(client.address).font(Typo.mono(Typo.small)).foregroundStyle(t.fg)
                            Text(age(client, now: context.date)).font(Typo.mono(Typo.small)).foregroundStyle(t.fg2)
                        }
                    }
                }
            }
            .onAppear { track(clients) }
            .onChange(of: clients) { _, new in track(new) }
        }
    }

    private func track(_ clients: [OSCClientInfo]) {
        for c in clients {
            guard let heard = c.lastHeard else { seen[c.address] = nil; continue }
            if seen[c.address]?.heard != heard { seen[c.address] = (heard, Date()) }
        }
    }

    private func age(_ client: OSCClientInfo, now: Date) -> String {
        guard client.lastHeard != nil, let at = seen[client.address]?.at else { return client.configured ? "configured" : "" }
        return "\(Int(now.timeIntervalSince(at))) s ago" + (client.configured ? " · configured" : "")
    }
}

/// A text field that commits on Enter or when it loses focus, and re-loads from the engine
/// afterwards, so a refused edit snaps back.
private struct CommitField: View {
    let placeholder: String
    let load: String
    var width: CGFloat?
    let commit: (String) -> Void
    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        TextField(placeholder, text: $text).textFieldStyle(.roundedBorder).frame(width: width)
            .focused($focused)
            .onSubmit { send() }
            .onChange(of: focused) { _, now in if !now { send() } }
            .onAppear { text = load }
            .onChange(of: load) { _, new in text = new }
    }

    private func send() {
        let trimmed = text.trimmingCharacters(in: .whitespaces)
        if trimmed != load.trimmingCharacters(in: .whitespaces) { commit(trimmed) }
        text = load
    }
}

// MARK: - Bindings (CTL-41)

private struct BindingsPanel: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @Binding var mappingName: String?
    @State private var confirmDelete = false
    @State private var action = ""
    @State private var newName = ""
    @State private var newDevice = "*"

    private static let columns: [(String, CGFloat)] = [
        ("Input", 140), ("Action", 120), ("", 120), ("Pickup", 74), ("Inv", 30), ("Mode", 96), ("Encoding", 112),
        ("Min", 46), ("Max", 46), ("FB", 30), ("On", 46), ("Off", 46), ("Page", 40),
    ]

    var body: some View {
        let info = engine.control.mappings.first { $0.name == mappingName }
        let doc = engine.controlMapping?.name == mappingName ? engine.controlMapping : nil
        PanelWindow(title: "Bindings", info: doc.map { "\($0.bindings.count)" } ?? "", framed: true, accessory: { EmptyView() }) {
            VStack(spacing: 0) {
                header(info)
                HLine()
                ScrollView([.horizontal, .vertical]) {
                    VStack(alignment: .leading, spacing: 0) {
                        HStack(spacing: 6) {
                            ForEach(Array(Self.columns.enumerated()), id: \.offset) { _, c in
                                Text(c.0).font(Typo.ui(Typo.small)).foregroundStyle(t.fg2).frame(width: c.1, alignment: .leading)
                            }
                        }
                        .padding(.horizontal, 8).frame(height: 20)
                        ForEach(doc?.bindings ?? []) { binding in
                            BindingRow(mapping: doc?.name ?? "", binding: binding, columns: Self.columns.map(\.1), client: engine)
                        }
                    }
                }
                .defaultScrollAnchor(.topLeading)
                HLine()
                footer
            }
        }
    }

    private func header(_ info: ControlMappingInfo?) -> some View {
        HStack(spacing: 8) {
            Picker("", selection: $mappingName) {
                ForEach(engine.control.mappings, id: \.name) { Text($0.name + ($0.profile ? " (profile)" : "")).tag(String?.some($0.name)) }
            }.labelsHidden().frame(width: 200)
            Spacer(minLength: 0)
            Button("Export") { export() }.disabled(engine.controlMapping == nil)
            Button("Import") { importMapping() }
            if let info, !info.profile {
                Button(confirmDelete ? "Confirm" : "Delete") {
                    if confirmDelete { engine.controlMappingDelete(info.name); confirmDelete = false } else { confirmDelete = true }
                }
                .buttonStyle(FlatButton(fill: confirmDelete ? .red : nil, compact: true))
            }
        }
        .buttonStyle(FlatButton(compact: true))
        .padding(.horizontal, 8).frame(height: 28)
        .onChange(of: mappingName) { _, _ in confirmDelete = false }
    }

    /// Add a binding by Learn, and a new mapping.
    private var footer: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                TextField("action, e.g. fader 1", text: $action).textFieldStyle(.roundedBorder).frame(width: 180)
                Button("Learn") {
                    engine.controlLearn(action.trimmingCharacters(in: .whitespaces))
                }
                .buttonStyle(FlatButton(fill: .blue, compact: true))
                .disabled(action.trimmingCharacters(in: .whitespaces).isEmpty)
                .help("Arms Learn: the next input is bound to this action in its device's mapping (OSC: the OSC mapping)")
                Spacer(minLength: 0)
                TextField("new mapping", text: $newName).textFieldStyle(.roundedBorder).frame(width: 140)
                TextField("device", text: $newDevice).textFieldStyle(.roundedBorder).frame(width: 110)
                    .help("Device name pattern; * matches any")
                Button("New") {
                    let name = newName.trimmingCharacters(in: .whitespaces)
                    engine.controlMappingNew(name, device: newDevice.trimmingCharacters(in: .whitespaces).isEmpty ? "*" : newDevice)
                    mappingName = name
                    newName = ""
                }
                .buttonStyle(FlatButton(compact: true))
                .disabled(newName.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
        .padding(8)
    }

    private func export() {
        guard let doc = engine.controlMapping, let data = doc.exportData else { return }
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.json]
        panel.nameFieldStringValue = "\(doc.name).json"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        do { try data.write(to: url, options: .atomic) } catch { engine.say("Cannot write \(url.lastPathComponent): \(error.localizedDescription)") }
    }

    /// A name that exists asks first; then the import goes out with `replace`.
    private func importMapping() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.json]
        panel.allowsMultipleSelection = false
        guard panel.runModal() == .OK, let url = panel.url else { return }
        guard let text = try? String(contentsOf: url, encoding: .utf8) else { return engine.say("Cannot read \(url.lastPathComponent)") }
        let name = ((try? JSONSerialization.jsonObject(with: Data(text.utf8))) as? [String: Any])?["name"] as? String
        var replace = false
        if let name, engine.control.mappings.contains(where: { $0.name == name }) {
            let alert = NSAlert()
            alert.messageText = "A mapping named \(name) exists."
            alert.informativeText = "Replace it with the imported one?"
            alert.addButton(withTitle: "Replace")
            alert.addButton(withTitle: "Cancel")
            guard alert.runModal() == .alertFirstButtonReturn else { return }
            replace = true
        }
        engine.controlMappingImport(json: text, replace: replace)
        if let name { mappingName = name }
    }
}

/// One binding with its options editable in place; each control sends only the field it changed.
@MainActor
private struct BindingRow: View {
    @Environment(\.theme) private var t
    let mapping: String
    let binding: ControlBinding
    let columns: [CGFloat]
    let client: EngineClient
    @State private var confirmRemove = false

    private static let encodings = [("", "–"), ("sign_magnitude", "Sign magnitude"), ("twos_complement", "Two's complement"), ("binary_offset", "Binary offset")]

    var body: some View {
        HStack(spacing: 6) {
            Text(binding.inputText).font(Typo.mono(Typo.body)).lineLimit(1).frame(width: columns[0], alignment: .leading)
            CommitField(placeholder: "action", load: binding.action) { send(["action": $0]) }.frame(width: columns[1])
            HStack(spacing: 4) {
                Button("Learn") { client.controlLearn(binding.action) }
                    .help("Arms Learn: the next input replaces this binding's input")
                Button(confirmRemove ? "Confirm" : "Remove") {
                    if confirmRemove { client.controlUnbind(mapping, input: binding.input, deckPage: binding.isDeck ? binding.deckPage : nil); confirmRemove = false } else { confirmRemove = true }
                }
                .buttonStyle(FlatButton(fill: confirmRemove ? .red : nil, compact: true))
            }
            .buttonStyle(FlatButton(compact: true))
            .frame(width: columns[2], alignment: .leading)
            Picker("", selection: Binding(get: { binding.pickup }, set: { send(["pickup": $0]) })) {
                Text("Pickup").tag(true); Text("Jump").tag(false)
            }.labelsHidden().frame(width: columns[3])
            check(binding.invert, "invert").frame(width: columns[4], alignment: .leading)
            Picker("", selection: Binding(get: { binding.buttonMode }, set: { send(["button_mode": $0]) })) {
                Text("Momentary").tag("momentary"); Text("Toggle").tag("toggle")
            }.labelsHidden().frame(width: columns[5])
            Picker("", selection: Binding(get: { binding.encoding ?? "" }, set: { send(["encoding": $0.isEmpty ? NSNull() : $0 as Any]) })) {
                ForEach(Self.encodings, id: \.0) { Text($0.1).tag($0.0) }
            }.labelsHidden().frame(width: columns[6])
            number(binding.min, "min").frame(width: columns[7])
            number(binding.max, "max").frame(width: columns[8])
            check(binding.feedback, "feedback").frame(width: columns[9], alignment: .leading)
            whole(binding.onValue, "on_value").frame(width: columns[10])
            whole(binding.offValue, "off_value").frame(width: columns[11])
            Text(binding.isDeck ? String(binding.deckPage) : "").font(Typo.mono(Typo.body)).frame(width: columns[12], alignment: .leading)
        }
        .padding(.horizontal, 8).padding(.vertical, 2)
        .onChange(of: mapping) { _, _ in confirmRemove = false }
    }

    private func send(_ fields: [String: Any]) {
        client.controlBinding(mapping, input: binding.input, binding.isDeck ? fields.merging(["deck_page": binding.deckPage]) { a, _ in a } : fields)
    }

    private func check(_ value: Bool, _ key: String) -> some View {
        Toggle("", isOn: Binding(get: { value }, set: { send([key: $0]) })).toggleStyle(.checkbox).labelsHidden()
    }

    private func number(_ value: Double, _ key: String) -> some View {
        CommitField(placeholder: key, load: value == value.rounded() ? "\(Int(value))" : "\(value)") { text in
            if let n = Double(text) { send([key: n]) } else { client.say("\(key) is a number") }
        }
    }

    private func whole(_ value: Int, _ key: String) -> some View {
        CommitField(placeholder: key, load: "\(value)") { text in
            if let n = Int(text) { send([key: n]) } else { client.say("\(key) is a whole number") }
        }
    }
}
