import SwiftUI

// Patch screen (PAT-10): pure rendering of `engine.fixtures` plus command sending. No overlap
// check, address or number logic lives here; the engine decides and refuses with a warning.

struct PatchView: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @State private var selection: Int?

    var body: some View {
        let rows = engine.fixtures.sorted { $0.number < $1.number }
        HStack(spacing: 8) {
            VStack(spacing: 8) {
                AddPanel()
                SelectedPanel(fixture: rows.first { $0.id == selection })
                if let message = engine.lastMessage {
                    Text(message).font(Typo.ui(11)).foregroundStyle(t.amber)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                Spacer(minLength: 0)
            }
            .frame(width: 320)
            PanelWindow(title: "Patch", info: "\(rows.count) fixtures", framed: true, accessory: { EmptyView() }) {
                Table(rows, selection: $selection) {
                    TableColumn("No") { Text("\($0.number)").monospacedDigit() }.width(min: 40, ideal: 50, max: 70)
                    TableColumn("Name") { Text($0.name) }.width(min: 60, ideal: 120)
                    TableColumn("Fixture") { Text($0.profile) }.width(min: 100, ideal: 200)
                    TableColumn("Mode") { Text($0.virtualDimmer ? "\($0.mode) · virtual dimmer" : $0.mode) }
                        .width(min: 100, ideal: 180)
                    TableColumn("Address") { Text("\($0.universe).\($0.address)").monospacedDigit() }
                        .width(min: 50, ideal: 70, max: 90)
                    TableColumn("Ch") { Text("\($0.footprint)").monospacedDigit() }.width(min: 30, ideal: 40, max: 60)
                }
            }
        }
        .padding(8)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(t.bg)
        .onChange(of: selection) { _, _ in engine.clearMessage() }
    }
}

/// "" = nil (omitted), a number = that number, anything else = invalid (`.some(nil)` is never produced).
private func optionalInt(_ text: String) -> Int?? {
    let t = text.trimmingCharacters(in: .whitespaces)
    if t.isEmpty { return .some(nil) }
    return Int(t).map { .some($0) }
}

private func labelled<Content: View>(_ label: String, @ViewBuilder _ content: () -> Content) -> some View {
    HStack(spacing: 6) {
        Text(label).font(.caption).foregroundStyle(.secondary).frame(width: 64, alignment: .leading)
        content()
    }
}

private struct AddPanel: View {
    @EnvironmentObject private var engine: EngineClient
    @State private var query = ""
    @State private var pickedID: String?
    @State private var mode = ""
    @State private var count = "1"
    @State private var universe = "1"
    @State private var address = ""
    @State private var number = ""

    var body: some View {
        let picked = engine.libraryResults.first { $0.id == pickedID }
        let countValue = Int(count), universeValue = Int(universe)
        let addressValue = optionalInt(address), numberValue = optionalInt(number)

        PanelWindow(title: "Add", framed: true, accessory: { EmptyView() }) {
            VStack(alignment: .leading, spacing: 6) {
                TextField("Search library, e.g. showtec par 56", text: $query)
                    .textFieldStyle(.roundedBorder)
                    .onSubmit { engine.searchLibrary(query) }
                    .onChange(of: query) { _, q in engine.searchLibrary(q) }
                List(engine.libraryResults, selection: $pickedID) { item in
                    Text("\(item.manufacturer) \(item.model)").font(.caption).lineLimit(1)
                }
                .listStyle(.plain)
                .frame(height: 110)
                .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(Color.secondary.opacity(0.3), lineWidth: hairline))
                .onChange(of: pickedID) { _, id in
                    mode = engine.libraryResults.first { $0.id == id }?.modes.first ?? ""
                }
                .onChange(of: engine.libraryResults) { _, _ in
                    // A new result list may hold the same pick under the same id; keep the mode valid.
                    if let item = engine.libraryResults.first(where: { $0.id == pickedID }) {
                        if !item.modes.contains(mode) { mode = item.modes.first ?? "" }
                    } else {
                        pickedID = nil
                        mode = ""
                    }
                }
                labelled("Mode") {
                    Picker("", selection: $mode) {
                        ForEach(picked?.modes ?? [], id: \.self) { Text($0).tag($0) }
                    }
                    .labelsHidden().disabled(picked == nil)
                }
                labelled("Count") { TextField("1", text: $count).textFieldStyle(.roundedBorder) }
                labelled("Universe") { TextField("1", text: $universe).textFieldStyle(.roundedBorder) }
                labelled("Address") { TextField("next free", text: $address).textFieldStyle(.roundedBorder) }
                labelled("Number") { TextField("next free", text: $number).textFieldStyle(.roundedBorder) }
                Button("Add") {
                    if let picked, let countValue, let universeValue, case let .some(addressValue) = addressValue,
                       case let .some(numberValue) = numberValue {
                        engine.patchAdd(profile: picked.profileKey, mode: mode, count: countValue,
                                        universe: universeValue, address: addressValue, number: numberValue)
                    }
                }
                .buttonStyle(FlatButton(fill: .blue))
                .disabled(picked == nil || mode.isEmpty || countValue == nil || universeValue == nil
                          || addressValue == nil || numberValue == nil)
            }
            .padding(8)
        }
    }
}

private struct SelectedPanel: View {
    @EnvironmentObject private var engine: EngineClient
    let fixture: FixtureInfo?
    @State private var universe = ""
    @State private var address = ""
    @State private var number = ""
    @State private var name = ""
    @State private var mode = ""
    /// Which destructive button has been clicked once; reset when the selection changes.
    @State private var armed: Armed?

    private enum Armed { case remove, mode }

    var body: some View {
        PanelWindow(title: fixture.map { "Selected · \($0.number)" } ?? "Selected", framed: true, accessory: { EmptyView() }) {
            if let fixture {
                VStack(alignment: .leading, spacing: 6) {
                    labelled("Move") {
                        TextField("univ", text: $universe).textFieldStyle(.roundedBorder).frame(width: 50)
                        TextField("address", text: $address).textFieldStyle(.roundedBorder)
                        Button("Move") {
                            if let u = Int(universe), let a = Int(address) { engine.patchMove(id: fixture.id, universe: u, address: a) }
                        }.buttonStyle(FlatButton())
                    }
                    labelled("Renumber") {
                        TextField("number", text: $number).textFieldStyle(.roundedBorder)
                        Button("Set") {
                            if let n = Int(number) { engine.patchRenumber(id: fixture.id, number: n) }
                        }.buttonStyle(FlatButton())
                    }
                    labelled("Name") {
                        TextField("name", text: $name).textFieldStyle(.roundedBorder)
                            .onSubmit { engine.patchName(id: fixture.id, name: name) }
                        Button("Set") { engine.patchName(id: fixture.id, name: name) }.buttonStyle(FlatButton())
                    }
                    labelled("Mode") {
                        TextField("mode name", text: $mode).textFieldStyle(.roundedBorder)
                        Button(armed == .mode ? "Confirm change" : "Change") {
                            if armed == .mode {
                                engine.patchChangeMode(id: fixture.id, mode: mode)
                                armed = nil
                            } else {
                                armed = .mode
                            }
                        }.buttonStyle(FlatButton(fill: armed == .mode ? .red : nil))
                    }
                    Button(armed == .remove ? "Confirm remove" : "Remove") {
                        if armed == .remove {
                            engine.patchRemove(ids: [fixture.id])
                            armed = nil
                        } else {
                            armed = .remove
                        }
                    }.buttonStyle(FlatButton(fill: armed == .remove ? .red : nil))
                }
                .padding(8)
                .onAppear { load(fixture) }
                .onChange(of: fixture.id) { _, _ in load(fixture) }
                .onChange(of: fixture.mode) { _, m in mode = m }
            } else {
                Text("Select a row").font(.caption).foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity).padding(12)
            }
        }
    }

    /// Prefill the fields from the engine's values for the newly selected fixture.
    private func load(_ f: FixtureInfo) {
        universe = "\(f.universe)"; address = "\(f.address)"; number = "\(f.number)"
        name = f.name; mode = f.mode; armed = nil
    }
}
