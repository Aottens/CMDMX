import SwiftUI

// Show screen (SHW-17): pure rendering of `engine.show` / `engine.shows` plus command sending.
// No name rules or duplicate checks live here; the engine decides and refuses with a warning.

struct ShowView: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @State private var selection: String?
    @State private var newName = ""
    @State private var saveName = ""
    /// Which show-switching button has been clicked once. New and Open change what is on stage,
    /// so they need a second click (SHW-17, PAT-10); Save As does not.
    @State private var armed: Armed?

    private enum Armed { case new, open }

    private static let modifiedFormat = Date.FormatStyle(date: .abbreviated, time: .standard)

    var body: some View {
        HStack(spacing: 8) {
            VStack(spacing: 8) {
                PanelWindow(title: "Open show", framed: true, accessory: { EmptyView() }) {
                    VStack(alignment: .leading, spacing: 6) {
                        Text(engine.show.name.isEmpty ? "—" : engine.show.name).font(Typo.legend(Typo.large, .semibold))
                        TimelineView(.periodic(from: .now, by: 1)) { context in
                            Text(engine.show.ageText(now: context.date)).font(Typo.ui(11))
                                .foregroundStyle(engine.show.autosaveOk ? t.fg2 : t.amber)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(8)
                }
                PanelWindow(title: "Open", framed: true, accessory: { EmptyView() }) {
                    VStack(alignment: .leading, spacing: 6) {
                        Text(selection ?? "Select a show in the list").font(Typo.ui(11)).foregroundStyle(t.fg2)
                        switchButton(.open, "Open", selection)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(8)
                }
                PanelWindow(title: "New", framed: true, accessory: { EmptyView() }) {
                    VStack(alignment: .leading, spacing: 6) {
                        TextField("name", text: $newName).textFieldStyle(.roundedBorder)
                        switchButton(.new, "New", newName)
                    }
                    .padding(8)
                }
                PanelWindow(title: "Save as", framed: true, accessory: { EmptyView() }) {
                    VStack(alignment: .leading, spacing: 6) {
                        TextField("name", text: $saveName).textFieldStyle(.roundedBorder)
                            .onSubmit { saveAs() }
                        Button("Save As") { saveAs() }
                            .buttonStyle(FlatButton())
                            .disabled(saveName.isEmpty)
                    }
                    .padding(8)
                }
                if let message = engine.lastMessage {
                    Text(message).font(Typo.ui(11)).foregroundStyle(t.amber)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                Spacer(minLength: 0)
            }
            .frame(width: 320)
            PanelWindow(title: "Shows", info: "\(engine.shows.count) shows", framed: true, accessory: { EmptyView() }) {
                Table(engine.shows, selection: $selection) {
                    TableColumn("Name") { Text($0.name) }.width(min: 100, ideal: 200)
                    TableColumn("Modified") { show in
                        Text(show.modified.map { $0.formatted(Self.modifiedFormat) } ?? "").monospacedDigit()
                    }.width(min: 140, ideal: 200)
                    TableColumn("") { show in
                        if show.name == engine.show.name { Text("open").foregroundStyle(t.green) }
                    }.width(min: 40, ideal: 50, max: 60)
                }
            }
        }
        .padding(8)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(t.bg)
        .onAppear { engine.listShows() }
        .onChange(of: selection) { _, _ in armed = nil; engine.clearMessage() }
        .onChange(of: newName) { _, _ in armed = nil }
        .onChange(of: engine.show.name) { _, _ in armed = nil }
    }

    /// First click arms ("Confirm open Kerk — output changes now"), the second sends.
    /// `name` is the selected row (Open) or the field text (New); empty or nil disables the button.
    private func switchButton(_ kind: Armed, _ title: String, _ name: String?) -> some View {
        let isArmed = armed == kind
        return Button {
            guard let name, !name.isEmpty else { return }
            if isArmed {
                armed = nil
                if kind == .open { engine.openShow(name) } else { engine.newShow(name) }
            } else {
                armed = kind
            }
        } label: {
            Text(isArmed ? "Confirm \(title.lowercased()) \(name ?? "") — output changes now" : title)
                .multilineTextAlignment(.leading).fixedSize(horizontal: false, vertical: true)
        }
        .buttonStyle(FlatButton(fill: isArmed ? .red : nil))
        .disabled(name?.isEmpty ?? true)
    }

    private func saveAs() {
        guard !saveName.isEmpty else { return }
        armed = nil
        engine.saveShowAs(saveName)
    }
}
