import SwiftUI
import AppKit

// Show screen (SHW-17): pure rendering of `engine.show` / `engine.shows` plus command sending.
// No name rules or duplicate checks live here; the engine decides and refuses with a warning.

struct ShowView: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @State private var selection: String?
    @State private var newName = ""
    @State private var saveName = ""
    @State private var renameTo = ""
    /// SHW-19/20: what New takes over ("looks" = patch, groups and palettes). Reset whenever the screen opens.
    @State private var keep = "looks"
    /// SHW-24: Open runs this Mac's output nodes instead of those in the file. On by default, remembered.
    @AppStorage("keepMacOutput") private var keepOutput = true
    /// Which show-switching button has been clicked once. New and Open change what is on stage,
    /// so they need a second click (SHW-17, PAT-10); Save As does not.
    @State private var armed: Armed?

    private enum Armed { case new, open, delete }

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
                        Button("Report") { engine.showReport() }.buttonStyle(FlatButton())
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(8)
                }
                PanelWindow(title: "Open", framed: true, accessory: { EmptyView() }) {
                    VStack(alignment: .leading, spacing: 6) {
                        Text(selection ?? "Select a show in the list").font(Typo.ui(11)).foregroundStyle(t.fg2)
                        Toggle("Keep this Mac's output", isOn: $keepOutput).toggleStyle(.checkbox).font(Typo.ui(11))
                        switchButton(.open, "Open", selection)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(8)
                }
                PanelWindow(title: "New", framed: true, accessory: { EmptyView() }) {
                    VStack(alignment: .leading, spacing: 6) {
                        TextField("name", text: $newName).textFieldStyle(.roundedBorder)
                        keepPicker
                        switchButton(.new, "New", newName)
                    }
                    .padding(8)
                }
                PanelWindow(title: "Save as / copy", framed: true, accessory: { EmptyView() }) {
                    VStack(alignment: .leading, spacing: 6) {
                        TextField("name", text: $saveName).textFieldStyle(.roundedBorder)
                            .onSubmit { saveAs() }
                        HStack {
                            Button("Save As") { saveAs() }
                            // SHW-21: a copy under this name; the open show stays open.
                            Button("Save Copy") { if !saveName.isEmpty { engine.saveShowCopy(saveName) } }
                        }
                        .buttonStyle(FlatButton())
                        .disabled(saveName.isEmpty)
                    }
                    .padding(8)
                }
                PanelWindow(title: "Selected show", framed: true, accessory: { EmptyView() }) { selectedShow }
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
        .onAppear { keep = "looks"; engine.listShows() }
        .onChange(of: selection) { _, _ in armed = nil; engine.clearMessage() }
        .onChange(of: newName) { _, _ in armed = nil }
        .onChange(of: keep) { _, _ in armed = nil }
        .onChange(of: keepOutput) { _, _ in armed = nil }
        .onChange(of: engine.show.name) { _, _ in armed = nil }
        .onChange(of: engine.shows) { _, shows in
            if let s = selection, !shows.contains(where: { $0.name == s }) { selection = nil }
        }
    }

    /// SHW-22: rename, delete (two-step, it goes to the Trash) and reveal the selected row.
    private var selectedShow: some View {
        VStack(alignment: .leading, spacing: 6) {
            TextField("new name", text: $renameTo).textFieldStyle(.roundedBorder)
            HStack {
                Button("Rename") { if let selection { engine.renameShow(selection, to: renameTo) } }
                    .disabled(selection == nil || renameTo.isEmpty)
                Button("Show in Finder") { reveal() }
                    .disabled(selection == nil || engine.showsDir == nil)
            }
            .buttonStyle(FlatButton())
            deleteButton
        }
        .padding(8)
    }

    private var deleteButton: some View {
        let isArmed = armed == .delete
        return Button {
            guard let selection else { return }
            if isArmed {
                armed = nil
                engine.deleteShow(selection)
            } else {
                armed = .delete
            }
        } label: {
            Text(isArmed ? "Delete \(selection ?? "") — it goes to the Trash" : "Delete")
                .multilineTextAlignment(.leading).fixedSize(horizontal: false, vertical: true)
        }
        .buttonStyle(FlatButton(fill: isArmed ? .red : nil))
        .disabled(selection == nil)
    }

    private func reveal() {
        guard let selection, let dir = engine.showsDir else { return }
        let file = URL(fileURLWithPath: dir).appendingPathComponent(selection + ".cmdmx.json")
        NSWorkspace.shared.activateFileViewerSelecting([file])
    }

    /// First click arms ("Confirm open Kerk — output changes now"), the second sends.
    /// `name` is the selected row (Open) or the field text (New); empty or nil disables the button.
    private func switchButton(_ kind: Armed, _ title: String, _ name: String?) -> some View {
        let isArmed = armed == kind
        return Button {
            guard let name, !name.isEmpty else { return }
            if isArmed {
                armed = nil
                if kind == .open { engine.openShow(name, keepOutput: keepOutput) } else { engine.newShow(name, keep: keep == "none" ? nil : keep) }
            } else {
                armed = kind
            }
        } label: {
            Text(isArmed ? armedLabel(kind, title, name) : title)
                .multilineTextAlignment(.leading).fixedSize(horizontal: false, vertical: true)
        }
        .buttonStyle(FlatButton(fill: isArmed ? .red : nil))
        .disabled(name?.isEmpty ?? true)
    }

    private func armedLabel(_ kind: Armed, _ title: String, _ name: String?) -> String {
        if kind == .new && keep == "looks" {
            return "New \(name ?? "") — playbacks and cue lists start empty"
        }
        if kind == .new && keep == "patch" {
            return "New show \(name ?? "") with the patch of \(engine.show.name): groups, palettes and playbacks start empty"
        }
        return "Confirm \(title.lowercased()) \(name ?? "") — output changes now"
    }

    private var keepPicker: some View {
        Picker("Keep", selection: $keep) {
            Text("Empty").tag("none")
            Text("The patch").tag("patch")
            Text("Patch, groups and palettes").tag("looks")
        }
        .pickerStyle(.radioGroup).font(Typo.ui(11))
    }

    private func saveAs() {
        guard !saveName.isEmpty else { return }
        armed = nil
        engine.saveShowAs(saveName)
    }
}
