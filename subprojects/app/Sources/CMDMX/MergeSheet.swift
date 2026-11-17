import SwiftUI

// UI-89: merge from another show (SHW-27..31). Pure rendering of the engine's contents and plan;
// the app computes no placement and validates no map. Every change asks for a dry run.

struct MergeSheet: View {
    let name: String
    @Environment(\.theme) private var t
    @Environment(\.dismiss) private var dismiss
    @EnvironmentObject private var engine: EngineClient
    @State private var groups: Set<Int> = []
    @State private var palettes: Set<Int> = []
    @State private var playbacks: Set<String> = []
    /// Fixture target fields as typed, and the lines changed from the engine's proposal.
    @State private var targets: [Int: String] = [:]
    @State private var map: [Int: Int?] = [:]
    @State private var tab = 0

    private var contents: MergeContents? { engine.mergeContents?.name == name ? engine.mergeContents : nil }
    private var nothingTicked: Bool { groups.isEmpty && palettes.isEmpty && playbacks.isEmpty }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Merge from \(name)").font(Typo.legend(Typo.large, .semibold))
            if let c = contents {
                Picker("", selection: $tab) {
                    Text("Groups").tag(0); Text("Palettes").tag(1); Text("Playbacks").tag(2); Text("Fixtures").tag(3)
                }.pickerStyle(.segmented).labelsHidden()
                ScrollView { list(c).frame(maxWidth: .infinity, alignment: .leading).padding(4) }
                    .frame(maxHeight: .infinity)
                    .background(t.bg)
                footer
            } else {
                if let m = engine.lastMessage { Text(m).foregroundStyle(t.amber) } else { Text("Reading \(name)…").foregroundStyle(t.fg2) }
                Spacer()
            }
            HStack {
                Spacer()
                Button("Cancel") { dismiss() }
                Button("Merge") { ask(dryRun: false) }
                    .disabled(nothingTicked || engine.mergePlan?.dryRun != true)
            }.buttonStyle(FlatButton())
        }
        .font(Typo.ui(11))
        .padding(12)
        .frame(width: 620, height: 520)
        .onAppear { engine.readMerge(name) }
        .onChange(of: contents) { _, c in
            targets = Dictionary(uniqueKeysWithValues: (c?.fixtures ?? []).map { ($0.number, $0.to.map(String.init) ?? "") })
        }
        .onChange(of: engine.mergePlan) { _, plan in if let plan, !plan.dryRun { dismiss() } }
    }

    @ViewBuilder private func list(_ c: MergeContents) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            switch tab {
            case 0:
                ForEach(c.groups) { g in check("\(g.number) \(g.legend)", isOn($groups, g.number)) }
            case 1:
                ForEach(c.palettes) { p in check("\(p.window) \(p.number) \(p.legend)", isOn($palettes, p.number)) }
            case 2:
                ForEach(c.playbacks) { p in
                    check("\(p.page).\(p.slot) \(p.legend) — \(Self.kind(p)) (\(p.cues) \(p.cues == 1 ? "cue" : "cues"))", isOn($playbacks, p.id))
                }
            default:
                ForEach(c.fixtures) { f in
                    HStack {
                        let unmapped = (targets[f.number] ?? "").isEmpty
                        Text("\(f.number) \(f.name) \(f.profile)\(f.mode.map { " (\($0))" } ?? "")")
                            .foregroundStyle(unmapped ? t.amber : t.fg)
                        Spacer()
                        Text("→").foregroundStyle(t.fg2)
                        TextField("", text: Binding(get: { targets[f.number] ?? "" }, set: { targets[f.number] = $0 }))
                            .textFieldStyle(.roundedBorder).frame(width: 60)
                            .onSubmit { commit(f) }
                    }
                }
            }
        }
    }

    private func check(_ title: String, _ on: Binding<Bool>) -> some View {
        Toggle(title, isOn: on).toggleStyle(.checkbox)
    }

    /// A binding that toggles membership and asks for a new dry run.
    private func isOn<K: Hashable>(_ set: Binding<Set<K>>, _ key: K) -> Binding<Bool> {
        Binding(get: { set.wrappedValue.contains(key) }, set: { on in
            if on { set.wrappedValue.insert(key) } else { set.wrappedValue.remove(key) }
            ask(dryRun: true)
        })
    }

    private var footer: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 2) {
                if let m = engine.lastMessage {
                    Text(m).foregroundStyle(t.amber)
                } else if nothingTicked {
                    Text("Tick what to merge").foregroundStyle(t.fg2)
                } else if let p = engine.mergePlan {
                    ForEach(p.placed) { x in
                        Text("\(x.kind.capitalized) \(x.from) → \(x.to)\(x.brought ? " (brought along)" : "")")
                    }
                    if !p.leftOutFixtures.isEmpty || p.leftOutValues > 0 {
                        Text("Left out: " + [
                            p.leftOutFixtures.isEmpty ? nil : "fixtures \(p.leftOutFixtures.map(String.init).joined(separator: ", ")) not in this show",
                            p.leftOutValues > 0 ? "\(p.leftOutValues) values" : nil,
                        ].compactMap { $0 }.joined(separator: "; ")).foregroundStyle(t.fg2)
                    }
                }
            }.frame(maxWidth: .infinity, alignment: .leading)
        }
        .frame(maxHeight: 110)
    }

    private static func kind(_ p: MergeContents.Playback) -> String {
        switch p.kind {
        case "Cues": return p.cues == 1 ? "Memory" : "Cue list"
        case "BpmMaster": return "BPM master"
        case "RateMaster": return "Rate master"
        case "GroupMaster": return "Group master"
        default: return p.kind
        }
    }

    /// An empty field means unmapped; the engine's proposal is not sent back as a change.
    private func commit(_ f: MergeContents.Fixture) {
        let v = Int((targets[f.number] ?? "").trimmingCharacters(in: .whitespaces))
        if v == f.to { map[f.number] = nil } else { map[f.number] = .some(v) }
        ask(dryRun: true)
    }

    private func ask(dryRun: Bool) {
        guard !nothingTicked else { engine.clearMessage(); return }
        let pbs: [(page: Int, slot: Int)] = (contents?.playbacks ?? []).filter { playbacks.contains($0.id) }.map { ($0.page, $0.slot) }
        engine.merge(name, map: map, groups: groups.sorted(), palettes: palettes.sorted(), playbacks: pbs, dryRun: dryRun)
    }
}
