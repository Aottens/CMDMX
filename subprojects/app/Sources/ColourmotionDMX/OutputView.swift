import SwiftUI

// Output screen (OUT-37): pure rendering of `engine.outputs` plus command sending. Which edits are
// valid is the engine's call; a refused edit lands in `engine.lastMessage` and the cells snap back.

struct OutputView: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @State private var armed: Int?

    var body: some View {
        let rows = engine.outputs.sorted { ($0.line, $0.id) < ($1.line, $1.id) }
        // A universe with fixtures but no enabled node sends nothing.
        let counts = Dictionary(grouping: engine.fixtures, by: \.universe).mapValues(\.count)
        let silent = counts.keys.sorted().filter { u in !engine.outputs.contains { $0.line == u && $0.enabled } }
        HStack(spacing: 8) {
            VStack(spacing: 8) {
                AddNodePanel()
                if let message = engine.lastMessage {
                    Text(message).font(Typo.ui(11)).foregroundStyle(t.amber)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                Spacer(minLength: 0)
            }
            .frame(width: 320)
            VStack(spacing: 8) {
                ForEach(silent, id: \.self) { u in
                    let n = counts[u] ?? 0
                    Text(verbatim: "Universe \(u): \(n) fixture\(n == 1 ? "" : "s"), no output").font(Typo.ui(11)).foregroundStyle(t.amber)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                PanelWindow(title: "Output", info: "\(rows.count) nodes", framed: true, accessory: { EmptyView() }) {
                    // Each cell gets `engine` again: after a reconnect rebuilds the rows, macOS Table cells
                    // can lose the inherited environment object, and reading it then traps.
                    Table(rows) {
                        TableColumn("Universe") { LineCell(node: $0).environmentObject(engine) }.width(min: 50, ideal: 60, max: 80)
                        TableColumn("Protocol") { Text(protocolName($0.kind)) }.width(min: 60, ideal: 70, max: 90)
                        TableColumn("Destination") { DestinationCell(node: $0).environmentObject(engine) }.width(min: 220, ideal: 360)
                        TableColumn("On") { EnabledCell(node: $0).environmentObject(engine) }.width(min: 30, ideal: 36, max: 50)
                        TableColumn("State") { StateCell(node: $0) }.width(min: 80, ideal: 160)
                        TableColumn("") { RemoveCell(node: $0, armed: $armed).environmentObject(engine) }.width(min: 70, ideal: 80, max: 90)
                    }
                }
            }
        }
        .padding(8)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(t.bg)
        .onAppear { engine.listPorts() }
    }
}

private func protocolName(_ kind: String) -> String {
    ["artnet": "Art-Net", "sacn": "sACN", "serial": "Serial"][kind] ?? kind
}

private struct AddNodePanel: View {
    @EnvironmentObject private var engine: EngineClient
    @State private var universe = "1"
    @State private var kind = "artnet"

    var body: some View {
        PanelWindow(title: "Add", framed: true, accessory: { EmptyView() }) {
            VStack(alignment: .leading, spacing: 6) {
                labelled("Universe") { TextField("1", text: $universe).textFieldStyle(.roundedBorder) }
                labelled("Protocol") {
                    Picker("", selection: $kind) {
                        Text("Art-Net").tag("artnet"); Text("sACN").tag("sacn"); Text("Serial").tag("serial")
                    }.labelsHidden()
                }
                Button("Add") { if let u = Int(universe) { engine.addOutput(line: u, kind: kind) } }
                    .buttonStyle(FlatButton(fill: .blue))
                    .disabled(Int(universe) == nil)
            }
            .padding(8)
        }
    }
}

/// A text field that re-loads from the engine whenever the node changes, so a refused edit snaps back.
private struct NodeField: View {
    let node: OutputNode
    let placeholder: String
    var width: CGFloat?
    let load: (OutputNode) -> String
    let commit: (String) -> Void
    @State private var text = ""

    var body: some View {
        TextField(placeholder, text: $text).textFieldStyle(.roundedBorder).frame(width: width)
            .onSubmit { commit(text.trimmingCharacters(in: .whitespaces)); text = load(node) }
            .onAppear { text = load(node) }
            .onChange(of: node) { _, n in text = load(n) }
    }
}

private struct LineCell: View {
    @EnvironmentObject private var engine: EngineClient
    let node: OutputNode
    var body: some View {
        NodeField(node: node, placeholder: "1", load: { "\($0.line)" }) { s in
            if let n = Int(s), n != node.line { engine.editOutput(node.id, ["line": n]) }
        }
    }
}

private struct DestinationCell: View {
    @EnvironmentObject private var engine: EngineClient
    let node: OutputNode

    var body: some View {
        switch node.kind {
        case "artnet":
            HStack(spacing: 6) {
                NodeField(node: node, placeholder: "net:sub:uni", width: 90, load: { n in
                    let pa = n.portAddress ?? 0
                    return "\(pa >> 8):\((pa >> 4) & 15):\(pa & 15)"
                }) { s in
                    if let pa = portAddress(s) { if pa != node.portAddress { engine.editOutput(node.id, ["port_address": pa]) } }
                    else { engine.say("Port address is net:subnet:universe or a number") }
                }
                Text(verbatim: "(\(node.portAddress ?? 0))").font(.caption).foregroundStyle(.secondary)
                NodeField(node: node, placeholder: "target", load: { $0.target ?? "" }) { s in
                    if s != node.target { engine.editOutput(node.id, ["target": s]) }
                }
            }
        case "sacn":
            HStack(spacing: 6) {
                NodeField(node: node, placeholder: "universe", width: 60, load: { "\($0.universe ?? 0)" }) { s in
                    if let n = Int(s), n != node.universe { engine.editOutput(node.id, ["universe": n]) }
                }
                NodeField(node: node, placeholder: "multicast", load: { $0.unicast ?? "" }) { s in
                    if s != (node.unicast ?? "") { engine.editOutput(node.id, ["unicast": s.isEmpty ? NSNull() : s as Any]) }
                }
                NodeField(node: node, placeholder: "prio", width: 50, load: { "\($0.priority ?? 100)" }) { s in
                    if let n = Int(s), n != node.priority { engine.editOutput(node.id, ["priority": n]) }
                }
            }
        case "serial":
            // Read the environment object here: a Picker in a Table cell builds its menu outside the
            // view's environment, where `engine` would trap.
            let stored = node.port ?? "auto", ports = engine.serialPorts, client = engine, id = node.id
            HStack(spacing: 6) {
                Picker("", selection: Binding(get: { stored }, set: { client.editOutput(id, ["port": $0]) })) {
                    Text("auto").tag("auto")
                    ForEach(ports) { port in
                        Text(port.product.map { "\(port.path) · \($0)" } ?? port.path).tag(port.path)
                    }
                    if stored != "auto" && !ports.contains(where: { $0.path == stored }) {
                        Text("\(stored) (not connected)").tag(stored)
                    }
                }.labelsHidden()
                Button("Refresh") { client.listPorts() }.buttonStyle(FlatButton(compact: true))
            }
        default:
            Text("?").foregroundStyle(.secondary)
        }
    }

    /// `n:s:u` (n 0..127, s and u 0..15) or a plain number; nil when neither.
    private func portAddress(_ s: String) -> Int? {
        let parts = s.split(separator: ":", omittingEmptySubsequences: false).map { Int($0) }
        if parts.count == 1 { return parts[0] }
        guard parts.count == 3, let n = parts[0], let sub = parts[1], let u = parts[2],
              (0...127).contains(n), (0...15).contains(sub), (0...15).contains(u) else { return nil }
        return n << 8 | sub << 4 | u
    }
}

/// Its own view with a binding: a Table cell closure does not redraw when the parent's state changes.
private struct RemoveCell: View {
    @EnvironmentObject private var engine: EngineClient
    let node: OutputNode
    @Binding var armed: Int?
    var body: some View {
        Button(armed == node.id ? "Confirm" : "Remove") {
            if armed == node.id { engine.removeOutputs(ids: [node.id]); armed = nil } else { armed = node.id }
        }
        .buttonStyle(FlatButton(fill: armed == node.id ? .red : nil, compact: true))
    }
}

private struct EnabledCell: View {
    @EnvironmentObject private var engine: EngineClient
    let node: OutputNode
    var body: some View {
        let client = engine, id = node.id
        Toggle("", isOn: Binding(get: { node.enabled }, set: { client.editOutput(id, ["enabled": $0]) }))
            .toggleStyle(.checkbox).labelsHidden()
    }
}

private struct StateCell: View {
    @Environment(\.theme) private var t
    let node: OutputNode
    var body: some View {
        let color = !node.enabled ? t.fg3 : [OutputNode.State.ok: t.green, .degraded: t.amber, .stopped: t.red, .disabled: t.fg3][node.state] ?? t.fg3
        HStack(spacing: 6) {
            Circle().fill(color).frame(width: 6, height: 6)
            Text(!node.enabled ? "off" : node.detail ?? "").font(.caption).foregroundStyle(.secondary).lineLimit(1)
        }
    }
}
