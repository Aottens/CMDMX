import AppKit
import SwiftUI

// Fixture info (UI-57): a right-click on a fixture tile opens a sheet with what the fixture is, what uses it
// (`usage` detail) and, per attribute, its value and in words why (`why`, IPC-60). Wording only: the daemon
// decides where a value comes from (ENG-25).

/// The fixture whose info sheet is open.
struct FixtureRef: Identifiable { let id: Int }

/// IPC-60: the answer to `why` (this client only).
struct WhyReply: Decodable, Equatable {
    struct Base: Decodable, Equatable {
        let source: String?
        let playback: Int?
        let cue: Double?
        let value: ProgrammerValue
        let tracked: Bool
        let palette: Int?
    }
    struct Other: Decodable, Equatable { let source: String?; let value: ProgrammerValue }
    struct Modifier: Decodable, Equatable {
        let kind: String
        var playback: Int?
        var legend: String?
        var mode: String?
        var level: Double?
        var enabled: Bool?
    }
    struct Attribute: Decodable, Equatable, Identifiable {
        let name: String
        let cell: Int?
        let value: ProgrammerValue
        let base: Base
        let others: [Other]
        let modifiers: [Modifier]
        var id: String { "\(cell ?? 0).\(name)" }
    }
    let fixture: Int
    let attributes: [Attribute]
}

/// IPC-60: the answer to `usage` with `detail`: the lists behind the counts.
struct UsageDetail: Decodable, Equatable {
    struct Cue: Decodable, Equatable { let playback: Int; let cue: Double; let legend: String }
    struct Entry: Decodable, Equatable { let id: Int; let legend: String }
    let fixtures: [Int]
    let cues: [Cue]
    let palettes: [Entry]
    let groups: [Entry]
}

/// A right-click (or ctrl-click) catcher: it takes the mouse only for that event, so taps reach the tile below.
struct SecondaryClick: NSViewRepresentable {
    let action: () -> Void

    final class Catcher: NSView {
        var action: () -> Void = {}
        override func hitTest(_ point: NSPoint) -> NSView? {
            guard let e = NSApp.currentEvent else { return nil }
            let secondary = e.type == .rightMouseDown || (e.type == .leftMouseDown && e.modifierFlags.contains(.control))
            return secondary ? super.hitTest(point) : nil
        }
        override func rightMouseDown(with event: NSEvent) { action() }
        override func mouseDown(with event: NSEvent) { action() }
    }

    func makeNSView(context: Context) -> Catcher { Catcher() }
    func updateNSView(_ view: Catcher, context: Context) { view.action = action }
}

@MainActor
struct FixtureInfoSheet: View {
    @Environment(\.theme) private var t
    @Environment(\.dismiss) private var dismiss
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState
    let id: Int

    private var fixture: FixtureInfo? { engine.fixtures.first { $0.id == id } }
    private var why: WhyReply? { engine.why?.fixture == id ? engine.why : nil }
    private var usage: UsageDetail? { engine.usageDetail?.fixtures == [id] ? engine.usageDetail : nil }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if let f = fixture {
                Text(verbatim: "FIXTURE \(f.number)\(f.name.isEmpty ? "" : " · \(f.name)")").font(Typo.legend(Typo.body, .semibold)).tracking(0.7)
                let model = f.profile.replacingOccurrences(of: "/", with: " · ")
                Text(verbatim: "\(model) · \(f.mode) · \(f.universe).\(f.address)").font(Typo.mono(Typo.small)).foregroundStyle(t.fg2)
            }
            ScrollView {
                VStack(alignment: .leading, spacing: 10) {
                    if let u = usage { used(u) }
                    now
                }.frame(maxWidth: .infinity, alignment: .leading)
            }
            HStack { Spacer(); Button("Done") { dismiss() }.keyboardShortcut(.defaultAction) }
        }
        // Escape closes like Done; zero-size rather than .hidden(), which disables the shortcut.
        .background(Button("") { dismiss() }.keyboardShortcut(.cancelAction).frame(width: 0, height: 0).opacity(0))
        .padding(16)
        .frame(width: 600, height: 460)
        .onAppear { engine.askUsageDetail(fixture: id) }
        // Asks again every second while open; the task ends with the sheet.
        .task {
            while !Task.isCancelled {
                engine.askWhy(fixture: id)
                try? await Task.sleep(for: .seconds(1))
            }
        }
    }

    private func section(_ title: String, _ lines: [(String, Int?)]) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title).font(Typo.legend(Typo.small, .semibold)).tracking(0.7).foregroundStyle(t.fg2)
            if lines.isEmpty {
                Text("none").font(Typo.ui(11)).foregroundStyle(t.fg3)
            } else {
                ForEach(Array(lines.enumerated()), id: \.offset) { _, line in goTo(line.1) { Text(line.0).font(Typo.ui(11)) } }
            }
        }
    }

    @ViewBuilder private func used(_ u: UsageDetail) -> some View {
        section("GROUPS", u.groups.map { ($0.legend.isEmpty ? "Group \($0.id)" : $0.legend, nil) })
        section("CUES", u.cues.map { c in
            let p = engine.playback(c.playback)
            return ("\(p.map { engine.shownLegend($0) == $0.homeHandle ? $0.homeHandle : "\($0.homeHandle) \(engine.shownLegend($0))" } ?? "Playback \(c.playback)") · Q\(Self.number(c.cue))\(c.legend.isEmpty ? "" : " \(c.legend)")", c.playback)
        })
        section("PALETTES", u.palettes.map { ($0.legend.isEmpty ? "Palette \($0.id)" : $0.legend, nil) })
    }

    private var now: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text("NOW").font(Typo.legend(Typo.small, .semibold)).tracking(0.7).foregroundStyle(t.fg2)
            if let w = why {
                ForEach(w.attributes) { a in
                    goTo(a.modifiers.first { $0.kind == "master" }?.playback ?? a.base.playback) {
                        VStack(alignment: .leading, spacing: 0) {
                            (Text(a.cell.map { "Cell \($0) " } ?? "") + Text(a.name) + Text(" ") + Self.valueText(a.value)).font(Typo.ui(11, .semibold))
                            Text(reason(a)).font(Typo.ui(11)).foregroundStyle(t.fg2).fixedSize(horizontal: false, vertical: true)
                        }
                    }
                }
            }
        }
    }

    /// A row that goes to a playback's page on a tap (like the holding rows of Active playbacks, UI-56).
    @ViewBuilder private func goTo<C: View>(_ playback: Int?, @ViewBuilder _ content: () -> C) -> some View {
        if let playback, let p = engine.playback(playback) {
            Button {
                guard ui.armedAllows(.activeRow, engine) else { return }
                engine.setPage(p.stored.page)
                dismiss()
            } label: { content().frame(maxWidth: .infinity, alignment: .leading).contentShape(Rectangle()) }
                .buttonStyle(.plain).help("Tap to go to its page")
        } else {
            content().frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    static func number(_ n: Double) -> String { n.rounded() == n ? String(Int(n)) : String(n) }

    /// A level as a percentage, a colour as a swatch and its hex (as the Programmer window shows them).
    static func valueText(_ v: ProgrammerValue) -> Text {
        switch v {
        case .level(let l): return Text(percent(l))
        case .colour(let r, let g, let b):
            return Text("\(Text(Image(systemName: "square.fill")).font(.system(size: 8)).foregroundStyle(Color(red: r, green: g, blue: b))) \(ProgrammerWindow.hex(r, g, b))")
        }
    }

    private static func plain(_ v: ProgrammerValue) -> String {
        switch v {
        case .level(let l): return percent(l)
        case .colour(let r, let g, let b): return ProgrammerWindow.hex(r, g, b)
        }
    }

    /// "Dienst Q4 gives 100 %, Master Zaal (1.8) scales it to 0 %": the daemon's facts in words.
    private func reason(_ a: WhyReply.Attribute) -> String {
        let b = a.base
        let gives = Self.plain(b.value) + (b.palette.map { n in " (palette \(engine.palettes.first { $0.number == n }?.shownLegend ?? String(n)))" } ?? "")
        var parts: [String]
        switch b.source {
        case nil: parts = ["nothing gives it a value"]
        case "Programmer"?: parts = ["the Programmer gives \(gives)"]
        case "Held"?: parts = ["held from an earlier look at \(gives)"]
        case let s?:
            parts = ["\(s)\(b.cue.map { " Q\(Self.number($0))" } ?? "") gives \(gives)\(b.tracked ? ", tracked from an earlier cue" : "")"]
        }
        if !a.others.isEmpty {
            parts.append("lower: " + a.others.map { "\($0.source ?? "default") \(Self.plain($0.value))" }.joined(separator: ", "))
        }
        parts += a.modifiers.map(modifier)
        return parts.joined(separator: ", ")
    }

    private func modifier(_ m: WhyReply.Modifier) -> String {
        let level = percent(m.level ?? 0)
        switch m.kind {
        case "master":
            let handle = m.playback.flatMap(engine.playback)?.homeHandle
            let name = "Master \(m.legend ?? "")\(handle.map { " (\($0))" } ?? "")"
            if m.enabled == false { return "\(name) is LOCKED at \(level)" }
            switch m.mode {
            case "Htp": return "\(name) lifts it to at least \(level)"
            case "Limit": return "\(name) caps it at \(level)"
            case "TakeOver": return "\(name) takes it over at \(level)"
            default: return m.level == 0 ? "\(name) scales it to 0 %" : "\(name) scales it by \(level)"
            }
        case "grand_master": return "Grand Master at \(level)"
        case "dbo": return "DBO is on"
        case "effect": return "an effect moves it"
        case "locate": return "Locate is on"
        case "highlight": return "Highlight is on"
        case "auto_mix": return "auto-mix changes it"
        default: return m.kind
        }
    }
}
