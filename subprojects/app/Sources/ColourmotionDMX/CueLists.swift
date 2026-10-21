import AppKit
import SwiftUI

// Cue lists in the app (CUE-42..44, 54; UI-13, UI-23): the Go block, the Cue list window, the Cue
// view and the cue legend editor. Rendering and command sending only: the engine runs the list,
// tracks the values and numbers the cues; everything shown comes from the snapshot and deltas.

extension EngineClient {
    /// CUE-26: the list the Go block, Space and the Cue list window act on.
    var connectedList: PlaybackInfo? { connected.flatMap { playback($0) } }
}

/// "Q3", or "S3" on a chase (its cues are steps).
func cueName(_ number: Double, chase: Bool = false) -> String { (chase ? "S" : "Q") + plain(number) }

/// Seconds with at most one decimal.
private func seconds(_ s: Double) -> String { plain((s * 10).rounded() / 10) }

/// "2" when in and out are equal, else "2/4" (CUE-42, 43).
func fadeText(_ cue: StoredCue) -> String {
    cue.fadeIn == cue.fadeOut ? seconds(cue.fadeIn) : "\(seconds(cue.fadeIn))/\(seconds(cue.fadeOut))"
}

private func linkText(_ link: CueLink) -> String {
    switch link.type {
    case "After": return "follow +\(seconds(link.offset))s"
    case "With": return "with +\(seconds(link.offset))s"
    default: return "·"
    }
}

private func trackText(_ track: String) -> String {
    ["Block": "B", "CueOnly": "CO", "Solo": "S"][track] ?? "·"
}

// MARK: - Go block (UI-23, CUE-42)

/// The Go block (210 pt) on the connected list: header, "now" line, the big GO with the next cue and
/// Back / Stop / Release; a chase adds Tap and the BPM field. Back, Stop, Release and GO send no id,
/// so the engine's connected list acts (CUE-27). Without a list it is drawn disabled, never covered.
struct GoBlock: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient

    var body: some View {
        let list = engine.connectedList
        let live = list?.live.cue
        VStack(spacing: 4) {
            header(list)
            nowLine(list)
            goButton(list)
            if let list, list.isChase {
                HStack(spacing: 2) {
                    KeyButton(label: "Tap", spec: "Tap the tempo", height: 30, action: { engine.tapPlayback(list.id) })
                    BPMField(id: list.id, bpm: list.live.bpm ?? list.stored.chase.bpm)
                }
            }
            HStack(spacing: 2) {
                KeyButton(label: "Back", spec: "Back one cue", height: 30, action: list == nil ? nil : { engine.cuelist("back") })
                KeyButton(label: "Stop", spec: "Pause the running transition; again = Back", height: 30, lit: live?.paused == true,
                          action: list == nil ? nil : { engine.cuelist("stop") })
                KeyButton(label: "Release", spec: "Release the list; it starts from its first cue next time", height: 30,
                          action: list == nil ? nil : { engine.releaseConnected() })
            }
        }
        .padding(6)
    }

    @ViewBuilder private func header(_ list: PlaybackInfo?) -> some View {
        VStack(spacing: 1) {
            HStack(spacing: 6) {
                if let list {
                    Text(list.shownLegend).font(Typo.legend(Typo.body, .semibold)).foregroundStyle(t.fg).lineLimit(1)
                    if !list.stored.legend.isEmpty { Text(list.homeHandle).foregroundStyle(t.fg2) }
                } else {
                    Text("CUE LIST").tracking(0.5).foregroundStyle(t.fg2)
                }
                Spacer(minLength: 0)
                Text(right(list)).foregroundStyle(t.fg2)
            }
            .font(Typo.mono(Typo.small)).lineLimit(1)
            // CUE-42: a Go at a low fader is explained.
            if let list, list.live.level < 0.995 {
                Text("fader \(percent(list.live.level))").font(Typo.mono(Typo.small)).foregroundStyle(t.amber)
                    .frame(maxWidth: .infinity, alignment: .trailing)
            }
        }
    }

    private func right(_ list: PlaybackInfo?) -> String {
        guard let list else { return "live — · next —" }
        if list.isChase {
            return "\(plain(((list.live.bpm ?? list.stored.chase.bpm) * 10).rounded() / 10)) BPM · xfade \(plain(list.stored.chase.crossfade)) %"
        }
        let live = list.cue(list.live.cue?.current).map { fadeText($0) + "s" } ?? "—"
        let next = list.cue(list.live.cue?.next).map { fadeText($0) + "s" } ?? "—"
        return "live \(live) · next \(next)"
    }

    private func nowLine(_ list: PlaybackInfo?) -> some View {
        HStack(spacing: 6) {
            if let list, let live = list.live.cue, let current = live.current {
                Text(cueName(current, chase: list.isChase)).font(Typo.mono()).foregroundStyle(t.green)
                Text(list.cue(current)?.legend ?? "").font(Typo.legend(13)).foregroundStyle(t.fg).lineLimit(1)
                Spacer(minLength: 0)
                if live.paused {
                    Text("paused").font(Typo.mono(Typo.small)).foregroundStyle(t.amber)
                } else if live.progress > 0 && live.progress < 1 {
                    Text(percent(live.progress)).font(Typo.mono(Typo.small)).foregroundStyle(t.green)
                }
            } else {
                Text("—").font(Typo.legend(13)).foregroundStyle(t.fg)
                Spacer(minLength: 0)
            }
        }
        .frame(height: 18)
    }

    private func goButton(_ list: PlaybackInfo?) -> some View {
        let live = list?.live.cue
        let next = live?.next
        let enabled = list != nil && next != nil
        let running = live.map { $0.current != nil && $0.progress > 0 && $0.progress < 1 } == true
        return Button { engine.cuelist("go") } label: {
            VStack(alignment: .leading, spacing: 6) {
                Text("GO").font(Typo.legend(24, .semibold)).tracking(2.9).foregroundStyle(t.green)
                if let list {
                    if let next {
                        (Text(cueName(next, chase: list.isChase)).font(Typo.mono()) + Text("  " + (list.cue(next)?.legend ?? "")).font(Typo.legend()))
                            .foregroundStyle(t.fg).lineLimit(1)
                    } else {
                        Text("end of list").font(Typo.legend()).foregroundStyle(t.fg).lineLimit(1)
                    }
                } else {
                    Text("No cue list").font(Typo.legend()).foregroundStyle(t.fg).lineLimit(1)
                }
            }
            .padding(.horizontal, 10).padding(.vertical, 8)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .background(RoundedRectangle(cornerRadius: radius).fill(t.surf))
            .overlay(alignment: .bottom) {
                if running, let progress = live?.progress {
                    GeometryReader { geo in
                        Rectangle().fill(t.green).frame(width: geo.size.width * progress, height: 3)
                            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomLeading)
                    }
                }
            }
            .clipShape(RoundedRectangle(cornerRadius: radius))
            .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(t.green, lineWidth: 2))
        }
        .buttonStyle(.plain).disabled(!enabled).opacity(enabled ? 1 : t.disabled)
    }
}

/// The chase's tempo (CUE-34): shows the effective BPM; scroll adjusts (Shift ±10, Option ±0.1),
/// typing a number and Return sets it (`playback.bpm`). The engine checks the range.
private struct BPMField: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    let id: Int
    let bpm: Double
    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        HStack(spacing: 4) {
            TextField("", text: $text).textFieldStyle(.plain).multilineTextAlignment(.trailing)
                .font(Typo.mono(Typo.large)).focused($focused).onSubmit(send)
                .onChange(of: focused) { _, now in if !now { send() } }
            Text("BPM").font(Typo.mono(Typo.small)).foregroundStyle(t.fg2)
        }
        .padding(.horizontal, 6).frame(maxWidth: .infinity).frame(height: 30)
        .background(RoundedRectangle(cornerRadius: radius).fill(t.surf2))
        .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(focused ? t.blue : t.line2, lineWidth: 1))
        .background(ScrollCatcher { steps, flags in
            let unit = flags.contains(.option) ? 0.1 : flags.contains(.shift) ? 10.0 : 1.0
            engine.setBPM(id, min(600, max(1, bpm + Double(steps) * unit)))
        })
        .onAppear { text = shown }
        .onChange(of: bpm) { _, _ in if !focused { text = shown } }
        .help("BPM: scroll to adjust, type a number and Return to set")
    }

    private var shown: String { plain((bpm * 10).rounded() / 10) }

    private func send() {
        if let n = Double(text.trimmingCharacters(in: .whitespaces).replacingOccurrences(of: ",", with: ".")), n != bpm {
            engine.setBPM(id, n)
        }
        text = shown
    }
}

// MARK: - Cue window (UI-13, CUE-43, 54)

/// The Playbacks workspace's cue window: the Cue list, or the Cue view in its place (CUE-54).
struct CueWindow: View {
    @EnvironmentObject private var ui: UIState

    var body: some View {
        if let ref = ui.cueView { CueViewWindow(ref: ref) } else { CueListWindow() }
    }
}

private struct CueListWindow: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState

    private static let widths: [CGFloat?] = [44, nil, 44, 56, 84, 36, 58]
    private static let titles = ["Q", "Legend", "Delay", "Fade", "Link", "Track", "Groups"]

    var body: some View {
        let list = engine.connectedList
        PanelWindow(title: list.map { "Cue list · \($0.shownLegend)" } ?? "Cue list",
                    info: list.map { "\($0.homeHandle) · \($0.stored.cues.count) cues" } ?? "No cue list connected",
                    accessory: { EmptyView() }) {
            VStack(spacing: 0) {
                HStack(spacing: 0) {
                    ForEach(Array(Self.titles.enumerated()), id: \.offset) { index, name in
                        Text(name.uppercased()).font(Typo.legend(Typo.small)).tracking(0.8).foregroundStyle(t.fg3)
                            .frame(width: Self.widths[index], alignment: .leading)
                            .frame(maxWidth: Self.widths[index] == nil ? .infinity : nil, alignment: .leading)
                    }
                }
                .padding(.horizontal, 7).padding(.vertical, 2)
                HLine()
                if let list {
                    ScrollView {
                        VStack(spacing: 0) { ForEach(list.stored.cues) { row(list, $0) } }
                    }
                } else {
                    Spacer(minLength: 0)
                }
            }
        }
    }

    private func row(_ list: PlaybackInfo, _ cue: StoredCue) -> some View {
        let live = list.live.cue
        let current = live?.current == cue.number
        let next = live?.next == cue.number
        let cells: [(String, Color)] = [
            (plain(cue.number), current ? t.green : t.fg2), (cue.legend, t.fg),
            (cue.delay == 0 ? "·" : seconds(cue.delay), t.fg2), (fadeText(cue), t.fg2),
            (linkText(cue.link), t.fg2), (trackText(cue.track), t.fg2),
            (attributeGroupOrder.filter(cue.groups.contains).joined(), t.fg2),
        ]
        return HStack(spacing: 0) {
            ForEach(Array(cells.enumerated()), id: \.offset) { index, cell in
                Text(cell.0).font(index == 1 ? Typo.legend() : Typo.mono(Typo.body)).foregroundStyle(current && index == 1 ? t.green : cell.1)
                    .lineLimit(1)
                    .frame(width: Self.widths[index], alignment: .leading)
                    .frame(maxWidth: Self.widths[index] == nil ? .infinity : nil, alignment: .leading)
            }
        }
        .padding(.horizontal, 7).frame(height: 24)
        .background {
            if current {
                // CUE-43: the running row shows its transition.
                GeometryReader { geo in
                    ZStack(alignment: .bottomLeading) {
                        t.green.opacity(0.12)
                        if let progress = live?.progress, progress > 0 && progress < 1 {
                            Rectangle().fill(t.green).frame(width: geo.size.width * progress, height: 2)
                        }
                    }
                }
            }
        }
        .overlay(alignment: .leading) { if current { Rectangle().fill(t.green).frame(width: 3) } }
        .overlay { if next { Rectangle().strokeBorder(t.fg2, lineWidth: 1) } }
        .overlay(alignment: .bottom) { HLine() }
        .contentShape(Rectangle())
        .onTapGesture { tap(list, cue) }
        .overlay { RightClickCatcher { rightClick(list, cue) } }
        .help("Tap: next · right-click: cue softkeys")
    }

    /// CUE-43: a tap sets the cue next (CUE-19), so it never changes the stage; with Record armed it
    /// picks the cue to record onto (CUE-45).
    private func tap(_ list: PlaybackInfo, _ cue: StoredCue) {
        guard ui.recordArmed else { return engine.cuelist("next", id: list.id, cue: cue.number) }
        ui.usedHandle = nil; ui.usedCell = nil; ui.maskMenu = false
        ui.usedCue = CueRef(playback: list.id, cue: cue.number)
        engine.say("Record onto \(cueName(cue.number)): Replace, Merge or Update on the softkeys.")
    }

    private func rightClick(_ list: PlaybackInfo, _ cue: StoredCue) {
        ui.cancelModes()
        ui.cueMenu = CueRef(playback: list.id, cue: cue.number)
        ui.lastHandle = list.id
    }
}

// MARK: - Cue view (CUE-54)

/// A row per fixture with the Programmer window's columns, showing the cue's whole state (IPC-24):
/// hard values in normal text, tracked ones magenta, palette references cyan. A right-click on a hard
/// value picks it for Remove. Asks for the view when it opens and after every stored change.
private struct CueViewWindow: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState
    let ref: CueRef

    private static let columns = ["I", "P", "C", "B"]

    var body: some View {
        let list = engine.playback(ref.playback)
        let reply = engine.cueView.flatMap { $0.playback == ref.playback && $0.cue == ref.cue ? $0 : nil }
        let fixtures = Dictionary(engine.fixtures.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
        PanelWindow(title: "Cue view · \(cueName(ref.cue, chase: list?.isChase == true))",
                    info: "\(list?.homeHandle ?? "?") · \(reply.map { "\($0.fixtures.count) fixtures" } ?? "…")",
                    accessory: {
                        Button("Back to list") { ui.cueView = nil; ui.cueValue = nil; ui.cueRemoveConfirm = false }
                            .buttonStyle(FlatButton(compact: true))
                    }) {
            if let reply, !reply.fixtures.isEmpty {
                ScrollView {
                    LazyVStack(spacing: 0, pinnedViews: .sectionHeaders) {
                        Section {
                            ForEach(reply.fixtures) { row in
                                let fixture = fixtures[row.fixture]
                                line(number: Text("\(fixture?.number ?? row.fixture)").foregroundStyle(t.fg2),
                                     name: Text(fixture.map(Self.title) ?? "").foregroundStyle(t.fg)) { group in
                                    cell(row, fixture, group)
                                }
                                HLine()
                            }
                        } header: {
                            VStack(spacing: 0) {
                                line(number: header("#"), name: header("Fixture")) { header($0) }
                                HLine()
                            }
                            .background(t.bg)
                        }
                    }
                }
            } else {
                NoteText(note: reply == nil ? "Asking the engine for this cue…" : "This cue holds no values.")
            }
        }
        .task(id: "\(ref.id)-\(engine.storedRevision)") { engine.viewCue(ref.playback, cue: ref.cue) }
    }

    private func header(_ s: String) -> some View {
        Text(s.uppercased()).font(Typo.legend(Typo.small)).tracking(0.8).foregroundStyle(t.fg3)
    }

    private func line(number: some View, name: some View, column: @escaping (String) -> some View) -> some View {
        HStack(alignment: .top, spacing: 0) {
            number.frame(width: 34, alignment: .leading)
            name.lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
            ForEach(Self.columns, id: \.self) { group in
                column(group).frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .font(Typo.mono(11))
        .padding(.horizontal, 7).padding(.vertical, 3)
    }

    private static func title(_ f: FixtureInfo) -> String {
        if !f.name.isEmpty { return f.name }
        return f.profile.split(separator: "/").last.map(String.init) ?? f.profile
    }

    @ViewBuilder private func cell(_ row: CueViewFixture, _ fixture: FixtureInfo?, _ group: String) -> some View {
        let mine = row.values.filter { fixture?.attributes[$0.key] == group }.sorted { $0.key < $1.key }
        if mine.isEmpty {
            Text("·").foregroundStyle(t.fg3)
        } else {
            HStack(spacing: 4) {
                ForEach(mine, id: \.key) { name, value in
                    item(row.fixture, name, value, named: mine.count > 1).overlay { RightClickCatcher { pick(row.fixture, name, value) } }
                }
            }
        }
    }

    private func item(_ fixture: Int, _ name: String, _ v: CueViewValue, named: Bool) -> Text {
        let text: Text
        switch v.value {
        case .level(let level): text = Text(named ? "\(name) \(percent(level))" : percent(level))
        case .colour(let r, let g, let b):
            text = Text("\(Text(Image(systemName: "square.fill")).font(.system(size: 8)).foregroundStyle(Color(red: r, green: g, blue: b))) \(ProgrammerWindow.hex(r, g, b))")
        }
        if let id = v.palette {
            let legend = engine.palettes.first { $0.id == id }?.shownLegend ?? "palette"
            return Text("\(text) \(legend)").foregroundStyle(t.cyan)
        }
        return text.foregroundStyle(v.from == ref.cue ? t.fg : t.magenta)
    }

    /// Only a value that is hard in this cue can be removed from it (CUE-54).
    private func pick(_ fixture: Int, _ name: String, _ v: CueViewValue) {
        guard v.from == ref.cue else {
            return engine.say("\(name) tracks from \(cueName(v.from)): remove it there.")
        }
        ui.cueValue = CueValueRef(fixture: fixture, attribute: name)
        ui.cueRemoveConfirm = false
        engine.say("\(name) picked: Remove on the softkeys takes it out of \(cueName(ref.cue)).")
    }
}

// MARK: - Cue legend (CUE-09)

/// The legend of one cue, 0–32 characters.
struct CueLegendEditor: View {
    @Environment(\.theme) private var t
    @Environment(\.dismiss) private var dismiss
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState
    let ref: CueRef
    @State private var legend = ""
    @State private var loaded = false

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("SET LEGEND · \(cueName(ref.cue))").font(Typo.legend(Typo.body, .semibold)).tracking(0.7)
            HStack(spacing: 8) {
                TextField("Legend", text: $legend).textFieldStyle(.roundedBorder).onSubmit(apply)
                    .onChange(of: legend) { _, new in if new.count > 32 { legend = String(new.prefix(32)) } }
                Text("\(legend.count)/32").font(Typo.mono(Typo.small)).foregroundStyle(t.fg2)
            }
            HStack {
                Spacer()
                Button("Cancel") { close() }.keyboardShortcut(.cancelAction)
                Button("Set") { apply() }.keyboardShortcut(.defaultAction)
            }
        }
        .padding(14)
        .frame(width: 360)
        .background(t.bg)
        .foregroundStyle(t.fg)
        .onAppear {
            guard !loaded else { return }
            loaded = true
            legend = engine.playback(ref.playback)?.cue(ref.cue)?.legend ?? ""
        }
    }

    private func apply() {
        engine.setCueLegend(ref.playback, cue: ref.cue, legend: legend)
        close()
    }

    private func close() {
        ui.cancelModes()
        ui.cueLegendEditing = nil
        dismiss()
    }
}
