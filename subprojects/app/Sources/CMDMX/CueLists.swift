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
    @EnvironmentObject private var livePlaybacks: LivePlaybacks   // UI-69: shows live values
    @EnvironmentObject private var ui: UIState

    var body: some View {
        let list = engine.connectedList
        let live = list?.live.cue
        VStack(spacing: 4) {
            header(list)
            nowLine(list)
            goButton(list)
            if let list, list.isChase {
                HStack(spacing: 2) {
                    KeyButton(label: "Tap", spec: "Tap the tempo", height: 30, action: { ui.learnOrRun("tap", engine) { engine.tapPlayback(list.id) } })
                    BPMField(id: list.id, bpm: list.live.bpm ?? list.stored.chase.bpm)
                }
                // CUE-65, 66: a row of its own, so the tempo keeps its width ("128.5 BPM").
                HStack(spacing: 2) {
                    // CUE-66: the direction; a click cycles Forward, Backward, Bounce, Random.
                    let dirs = ["Forward", "Backward", "Bounce", "Random"]
                    let dir = list.stored.chase.direction
                    let next = dirs[((dirs.firstIndex(of: dir) ?? 0) + 1) % dirs.count]
                    KeyButton(label: ["Forward": "\u{2192}", "Backward": "\u{2190}", "Bounce": "\u{2194}", "Random": "Rnd"][dir] ?? dir,
                              spec: "Direction: \(dir) (click for \(next))", height: 30,
                              action: { engine.setPlaybackOptions(list.id, ["chase": ["direction": next]]) })
                    // CUE-65: the crossfade, the same field as the tempo.
                    BPMField(id: list.id, bpm: list.stored.chase.crossfade, unit: "% xfade", range: 0...100, whole: true,
                             help: "Crossfade: scroll to adjust, type a number and Return to set",
                             set: { engine, id, n in engine.setPlaybackOptions(id, ["chase": ["crossfade": n]]) })
                }
            }
            HStack(spacing: 2) {
                // CTL-42: in Learn mode the keys are live without a list and arm Learn for their plain action.
                let usable = list != nil || ui.learnMode
                KeyButton(label: "Back", spec: "Back one cue", height: 30, action: usable ? { ui.learnOrRun("back", engine) { engine.cuelist("back") } } : nil, drag: "back")
                KeyButton(label: "Stop", spec: "Pause the running transition; again = Back", height: 30, lit: live?.paused == true,
                          action: usable ? { ui.learnOrRun("stop", engine) { engine.cuelist("stop") } } : nil, drag: "stop")
                KeyButton(label: "Release", spec: "Release the list; it starts from its first cue next time", height: 30,
                          action: usable ? { ui.learnOrRun("release", engine) { engine.releaseConnected() } } : nil, drag: "release")
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
                // UI-71: shown by the threshold, the number is live.
                LiveLabel(tick: engine.livePlaybacks.tick(list.id), text: { percent($0.level) == "0 %" ? "fader 0 % \u{00B7} no output" : "fader \(percent($0.level))" }, color: t.amber,
                          sample: "fader 0 % \u{00B7} no output", alignment: .right)
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
                Text(cueName(current, chase: list.isChase)).font(Typo.mono(20)).foregroundStyle(t.green)
                Text(list.cue(current)?.legend ?? "").font(Typo.legend(20)).foregroundStyle(t.green).lineLimit(1)
                Spacer(minLength: 0)
                if live.paused {
                    Text("paused").font(Typo.mono(Typo.small)).foregroundStyle(t.amber)
                } else if live.progress > 0 && live.progress < 1 {
                    LiveLabel(tick: engine.livePlaybacks.tick(list.id), text: { percent($0.progress ?? 0) }, color: t.green, sample: "100 %")
                }
            } else {
                Text("—").font(Typo.legend(13)).foregroundStyle(t.fg)
                Spacer(minLength: 0)
            }
        }
        .padding(.leading, 8)
        .frame(height: 28)
        .overlay(alignment: .leading) { if list?.live.cue?.current != nil { Rectangle().fill(t.green).frame(width: 3) } }
    }

    private func goButton(_ list: PlaybackInfo?) -> some View {
        let live = list?.live.cue
        let next = live?.next
        let enabled = (list != nil && next != nil) || ui.learnMode
        let running = live.map { $0.current != nil && $0.progress > 0 && $0.progress < 1 } == true
        return Button { ui.learnOrRun("go", engine) { engine.cuelist("go") } } label: {
            VStack(alignment: .leading, spacing: 6) {
                Text("GO").font(Typo.legend(24, .semibold)).tracking(2.9).foregroundStyle(t.green)
                if let list {
                    if let next {
                        (Text("Next: " + cueName(next, chase: list.isChase)).font(Typo.mono(Typo.small)) + Text("  " + (list.cue(next)?.legend ?? "")).font(Typo.legend(Typo.small)))
                            .foregroundStyle(t.fg2).lineLimit(1)
                    } else {
                        Text("end of list").font(Typo.legend(Typo.small)).foregroundStyle(t.fg2).lineLimit(1)
                    }
                } else {
                    Text("No cue list").font(Typo.legend()).foregroundStyle(t.fg).lineLimit(1)
                }
            }
            .padding(.horizontal, 10).padding(.vertical, 8)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .background(RoundedRectangle(cornerRadius: radius).fill(t.surf))
            .overlay(alignment: .bottom) {
                if running, let list {
                    LiveProgressLine(tick: engine.livePlaybacks.tick(list.id), color: t.green).frame(height: 3)
                }
            }
            .clipShape(RoundedRectangle(cornerRadius: radius))
            .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(t.green, lineWidth: 2))
        }
        .buttonStyle(.plain).disabled(!enabled).opacity(enabled ? 1 : t.disabled).deckDrag("go")
    }
}

/// The chase's tempo (CUE-34): shows the effective BPM; scroll adjusts (Shift ±10, Option ±0.1),
/// typing a number and Return sets it (`playback.bpm`). The engine checks the range.
private struct BPMField: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    let id: Int
    let bpm: Double
    /// CUE-65: the same field also carries the crossfade (percent, whole steps).
    var unit = "BPM"
    var range: ClosedRange<Double> = 1...600
    var whole = false
    var help = "BPM: scroll to adjust, type a number and Return to set"
    var set: (EngineClient, Int, Double) -> Void = { engine, id, n in engine.setBPM(id, n) }
    @EnvironmentObject private var ui: UIState
    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        HStack(spacing: 4) {
            TextField("", text: $text).textFieldStyle(.plain).multilineTextAlignment(.trailing)
                .font(Typo.mono(Typo.large)).focused($focused).onSubmit(send)
                .onChange(of: focused) { _, now in if !now { if ui.fieldEscaped { text = shown } else { send() } } }   // UI-44
            Text(unit).font(Typo.mono(Typo.small)).foregroundStyle(t.fg2).lineLimit(1).fixedSize()
        }
        .padding(.horizontal, 6).frame(maxWidth: .infinity).frame(height: 30)
        .background(RoundedRectangle(cornerRadius: radius).fill(t.surf2))
        .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(focused ? t.blue : t.line2, lineWidth: 1))
        .background(ScrollCatcher { steps, flags in
            let unit = flags.contains(.option) && !whole ? 0.1 : flags.contains(.shift) ? 10.0 : 1.0
            apply(min(range.upperBound, max(range.lowerBound, bpm + Double(steps) * unit)))
        })
        .onAppear { text = shown }
        .onChange(of: bpm) { _, _ in if !focused { text = shown } }
        .help(help)
    }

    private func apply(_ n: Double) {
        set(engine, id, n)
    }

    private var shown: String { plain((bpm * 10).rounded() / 10) }

    /// Shows what was sent, as NumberField does (VS3-05 run: the old tempo stayed in the field).
    private func send() {
        if let n = Double(text.trimmingCharacters(in: .whitespaces).replacingOccurrences(of: ",", with: ".")), n != bpm {
            apply(n)
            text = plain((n * 10).rounded() / 10)
        } else {
            text = shown
        }
    }
}

// MARK: - Cue window (UI-13, CUE-43, 54)

/// The Playbacks workspace's cue window: the Cue list, or the Cue view in its place (CUE-54).
struct CueWindow: View {
    @EnvironmentObject private var ui: UIState
    @EnvironmentObject private var engine: EngineClient

    var body: some View {
        Group {
            if let ref = ui.cueView { CueViewWindow(ref: ref) } else { CueListWindow() }
        }
        .onChange(of: engine.connected) { _, _ in ui.endCueMemory() }   // UI-53: another connected list ends the memory
    }
}

private struct CueListWindow: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var livePlaybacks: LivePlaybacks   // UI-69: shows live values
    @EnvironmentObject private var ui: UIState
    @AppStorage("cueFollow") private var follow = true   // CUE-67: the window follows the current cue

    private static let widths: [CGFloat?] = [44, nil, 44, 56, 84, 36, 58, 62]
    private static let titles = ["Q", "Legend", "Delay", "Fade", "Link", "Track", "Groups", ""]
    /// UI-51: the Times view's columns after Q and Legend: `cue.times` key and title.
    private static let timeColumns = [("delay", "Delay in"), ("delay_out", "Delay out"), ("fade_in", "Fade in"),
                                      ("fade_out", "Fade out"), ("overlap", "Overlap")]
    private static let timeWidths: [CGFloat?] = [36, nil, 58, 64, 54, 60, 54, 62]
    private static let rowHeight: CGFloat = 24

    var body: some View {
        let memory = ui.liveCueMemory(engine)   // UI-53
        let list = memory ?? engine.connectedList
        let times = ui.cueTimesView && list?.isChase != true && memory == nil   // UI-51
        let titles = times ? ["Q", "Legend"] + Self.timeColumns.map(\.1) + [""] : Self.titles
        let widths = times ? Self.timeWidths : Self.widths
        PanelWindow(title: memory.map { "Cue · \($0.homeHandle)" } ?? list.map { "Cue list · \($0.shownLegend)" } ?? "Cue list",
                    info: memory.map(\.shownLegend) ?? list.map { "\($0.homeHandle) · \($0.stored.cues.count) cues" } ?? "No cue list connected",
                    accessory: {
                        if list != nil {
                            Button("Follow") { follow.toggle() }   // CUE-67
                                .buttonStyle(FlatButton(fill: follow ? .blue : nil, compact: true))
                                .help("Follow: the window scrolls to the current cue on every change · a hand scroll does not turn it off")
                        }
                        if list?.isChase != true && memory == nil {
                            Button("Times") { ui.cueTimesView.toggle(); ui.timeCells = nil }
                                .buttonStyle(FlatButton(fill: ui.cueTimesView ? .blue : nil, compact: true))
                        }
                    }) {
            VStack(spacing: 0) {
                HStack(spacing: 0) {
                    ForEach(Array(titles.enumerated()), id: \.offset) { index, name in
                        Text(name.uppercased()).font(Typo.legend(Typo.small)).tracking(0.8).foregroundStyle(t.fg3)
                            .frame(width: widths[index], alignment: .leading)
                            .frame(maxWidth: widths[index] == nil ? .infinity : nil, alignment: .leading)
                    }
                }
                .padding(.horizontal, 7).padding(.vertical, 2)
                HLine()
                if let list {
                    ScrollViewReader { proxy in
                        ScrollView {
                            VStack(spacing: 0) { ForEach(list.stored.cues) { if times { timesRow(list, $0) } else { row(list, $0) } } }
                                .coordinateSpace(name: "cueTimes")
                        }
                        // CUE-67: a changed current cue (or another list) brings it and the row after it into sight.
                        .onChange(of: FollowKey(list: list.id, cue: list.live.cue?.current)) { _, _ in scrollToCurrent(list, proxy) }
                        .onAppear { scrollToCurrent(list, proxy) }
                    }
                } else {
                    Spacer(minLength: 0)
                }
            }
        }
    }

    /// CUE-67: nil anchor scrolls the least: a row already in sight stays, the row above shows only when there is room.
    private func scrollToCurrent(_ list: PlaybackInfo, _ proxy: ScrollViewProxy) {
        guard follow, let current = list.live.cue?.current else { return }
        let cues = list.stored.cues
        guard let index = cues.firstIndex(where: { $0.number == current }) else { return }
        proxy.scrollTo(cues[max(index - 1, 0)].id, anchor: nil)
        proxy.scrollTo(cues[min(index + 1, cues.count - 1)].id, anchor: nil)
    }

    private struct FollowKey: Equatable { var list: Int; var cue: Double? }

    private func row(_ list: PlaybackInfo, _ cue: StoredCue) -> some View {
        let live = list.live.cue
        let current = live?.current == cue.number
        let cells: [(String, Color)] = [
            (plain(cue.number), current ? t.green : t.fg2), (cue.legend, t.fg),
            (cue.delay == 0 ? "·" : seconds(cue.delay), t.fg2), (fadeText(cue), t.fg2),
            (linkText(cue.link), t.fg2), (trackText(cue.track), t.fg2),
            (attributeGroupOrder.filter(cue.groups.contains).joined(), t.fg2),
        ]
        return decorated(list, cue) {
            HStack(spacing: 0) {
                ForEach(Array(cells.enumerated()), id: \.offset) { index, cell in
                    Text(cell.0).font(index == 1 ? Typo.legend() : Typo.mono(Typo.body)).foregroundStyle(current && index == 1 ? t.green : cell.1)
                        .lineLimit(1)
                        .frame(width: Self.widths[index], alignment: .leading)
                        .frame(maxWidth: Self.widths[index] == nil ? .infinity : nil, alignment: .leading)
                }
                rowButtons(list, cue)
            }
        }
        .contentShape(Rectangle())
        .onTapGesture { tap(list, cue) }
        .overlay { RightClickCatcher { rightClick(list, cue) } }
        .help(list.isMemory ? "View: Cue view · ⋯ or right-click: cue softkeys" : "Tap: next · View: Cue view · ⋯ or right-click: cue softkeys")
    }

    /// UI-54: View (the Cue view) and ⋯ (the cue softkeys) on a row; armed, the whole row is the target (UI-42).
    private func rowButtons(_ list: PlaybackInfo, _ cue: StoredCue) -> some View {
        let armed = ui.recordArmed || ui.function != nil
        return HStack(spacing: 2) {
            Button("View") {
                if armed { return tap(list, cue) }
                ui.timeCells = nil; ui.cueView = CueRef(playback: list.id, cue: cue.number); ui.cueValue = nil; ui.cancelModes()
            }
            Button("⋯") {
                if armed { return tap(list, cue) }
                ui.timeCells = nil; rightClick(list, cue)
            }
        }
        .buttonStyle(FlatButton(compact: true))
        .frame(width: 62, alignment: .trailing)
    }

    /// The row frame both views share: current, next and progress (CUE-43).
    private func decorated<Content: View>(_ list: PlaybackInfo, _ cue: StoredCue, @ViewBuilder _ content: () -> Content) -> some View {
        let live = list.live.cue
        let current = live?.current == cue.number
        let next = live?.next == cue.number
        return content()
        .padding(.horizontal, 7).frame(height: Self.rowHeight)
        .background {
            if current {
                // CUE-43: the running row shows its transition.
                ZStack(alignment: .bottomLeading) {
                    t.green.opacity(0.12)
                    if let progress = live?.progress, progress > 0 && progress < 1 {
                        LiveProgressLine(tick: engine.livePlaybacks.tick(list.id), color: t.green).frame(height: 2)
                    }
                }
            }
        }
        .overlay(alignment: .leading) { if current { Rectangle().fill(t.green).frame(width: 3) } }
        .overlay { if next { Rectangle().strokeBorder(t.fg2, lineWidth: 1) } }
        .overlay(alignment: .bottom) { HLine() }
        .overlay { if (ui.cueDelete || ui.cueBlockPicking) && ui.cueMenu?.playback == list.id && ui.cuePicks.contains(cue.number) { Rectangle().fill(t.red.opacity(0.18)).overlay(Rectangle().strokeBorder(t.red, lineWidth: 2)) } }   // UI-64
    }

    /// UI-51: a row of the Times view. A tap selects one time cell and never sets Next; armed, the whole row is the target (UI-42).
    private func timesRow(_ list: PlaybackInfo, _ cue: StoredCue) -> some View {
        let current = list.live.cue?.current == cue.number
        let armed = ui.recordArmed || ui.function != nil
        let values = [cue.delay, cue.delayOut, cue.fadeIn, cue.fadeOut, cue.overlap]
        let dim = [false, cue.delayOutAsIn, false, cue.fadeOutAsIn, false]
        let selected = ui.timeCells.flatMap { $0.playback == engine.connected && $0.cues.contains(cue.number) ? $0.column : nil }
        return decorated(list, cue) {
            HStack(spacing: 0) {
                Text(plain(cue.number)).font(Typo.mono(Typo.body)).foregroundStyle(current ? t.green : t.fg2)
                    .lineLimit(1).frame(width: Self.timeWidths[0], alignment: .leading).frame(maxHeight: .infinity)
                    .contentShape(Rectangle()).onTapGesture { if armed { tap(list, cue) } }
                Text(cue.legend).font(Typo.legend()).foregroundStyle(current ? t.green : t.fg)
                    .lineLimit(1).frame(maxWidth: .infinity, alignment: .leading).frame(maxHeight: .infinity)
                    .contentShape(Rectangle()).onTapGesture { if armed { tap(list, cue) } }
                ForEach(Array(Self.timeColumns.enumerated()), id: \.offset) { index, column in
                    Text(plain((values[index] * 100).rounded() / 100)).font(Typo.mono(Typo.body))
                        .foregroundStyle(dim[index] ? t.fg3 : t.fg2)
                        .lineLimit(1).frame(width: Self.timeWidths[index + 2], alignment: .leading)
                        .frame(maxHeight: .infinity)
                        .background(selected == column.0 ? t.blue.opacity(0.35) : Color.clear)
                        .contentShape(Rectangle())
                        .onTapGesture { tapTime(list, cue, column) }
                        .gesture(DragGesture(minimumDistance: 4, coordinateSpace: .named("cueTimes")).onChanged { drag in
                            guard !armed else { return }
                            dragTimes(list, from: cue, column, drag)
                        })
                }
                rowButtons(list, cue)
            }
        }
        .overlay { RightClickCatcher { ui.timeCells = nil; rightClick(list, cue) } }
        .help("Tap a time · drag or Shift-tap: more in this column · View: Cue view · ⋯ or right-click: cue softkeys")
    }

    /// UI-51: a tap selects this cell; Shift-tap in the same column extends from the anchor.
    private func tapTime(_ list: PlaybackInfo, _ cue: StoredCue, _ column: (String, String)) {
        if ui.recordArmed || ui.function != nil { return tap(list, cue) }
        if NSApp.currentEvent?.modifierFlags.contains(.shift) == true, let cells = ui.timeCells, cells.playback == list.id, cells.column == column.0 {
            select(list, column, from: cells.anchor, to: cue.number)
        } else {
            ui.timeCells = TimeCells(playback: list.id, column: column.0, title: column.1, cues: [cue.number], anchor: cue.number)
        }
    }

    /// UI-51: a drag selects from the row it started on to the row under the pointer, in the start cell's column.
    private func dragTimes(_ list: PlaybackInfo, from cue: StoredCue, _ column: (String, String), _ drag: DragGesture.Value) {
        let rows = list.stored.cues
        let index = min(max(Int(floor(drag.location.y / Self.rowHeight)), 0), rows.count - 1)
        guard let to = rows[safe: index] else { return }
        select(list, column, from: cue.number, to: to.number)
    }

    private func select(_ list: PlaybackInfo, _ column: (String, String), from: Double, to: Double) {
        let numbers = list.stored.cues.map(\.number)
        guard let a = numbers.firstIndex(of: from), let b = numbers.firstIndex(of: to) else { return }
        ui.timeCells = TimeCells(playback: list.id, column: column.0, title: column.1,
                                 cues: Array(numbers[min(a, b)...max(a, b)]), anchor: from)
    }

    /// CUE-43: a tap sets the cue next (CUE-19), so it never changes the stage; with Record armed it
    /// picks the cue to record onto (CUE-45).
    private func tap(_ list: PlaybackInfo, _ cue: StoredCue) {
        if ui.recordArmed, ui.building == nil {   // CUE-62: while building, armedAllows says Building
            ui.usedHandle = nil; ui.usedCell = nil; ui.maskMenu = false
            ui.usedCue = CueRef(playback: list.id, cue: cue.number)
            engine.say("Record onto \(cueName(cue.number)): Replace, Merge or Update on the softkeys.")
            return
        }
        guard ui.armedAllows(.cue, engine) else { return }   // UI-42
        let ref = CueRef(playback: list.id, cue: cue.number)
        // Copy, Move and Delete open the Cue softkeys on this row (CUE-43) and stay armed; every Cue softkey disarms.
        func openCueMenu() {
            ui.pickKind = nil; ui.picks = []; ui.cuePicks = []
            ui.cueMenu = ref; ui.cueDelete = false; ui.lastHandle = list.id
        }
        // UI-53: a memory's one cue is not copied, moved or deleted from its row.
        if list.isMemory, let f = ui.function, [.copy, .move, .delete].contains(f) {
            return engine.say("\(f.rawValue): \(list.homeHandle) holds one cue, tap its handle.")
        }
        switch ui.function {
        case nil:
            if !list.isMemory { engine.cuelist("next", id: list.id, cue: cue.number) }
        case .update:
            engine.recordPlayback(page: list.stored.page, slot: list.stored.slot, mode: "Update", cue: cue.number)
            ui.cancelModes()
        case .include:
            engine.includePlayback(list.id, cue: cue.number, mask: ui.orderedRecordMask)   // PBK-48
            ui.cancelModes()
        case .copy, .move:
            // CUE-69: picks rows like Delete (⇧ a range, ⌘ one more); a number and Enter, Copy / Renumber or End sends them.
            let name = ui.function?.rawValue ?? ""
            let flags = NSApp.currentEvent?.modifierFlags ?? []
            if flags.contains(.shift) || flags.contains(.command), ui.cueBlockPicking, ui.cueMenu?.playback == list.id {
                ui.cuePicks = extended(ui.cuePicks, with: cue.number, in: list.stored.cues.map(\.number), range: flags.contains(.shift))
                if let last = ui.cuePicks.last, !ui.cuePicks.contains(ui.cueMenu?.cue ?? -1) { ui.cueMenu = CueRef(playback: list.id, cue: last) }
                if ui.cuePicks.isEmpty { ui.cueMenu = nil; ui.cueBlockTarget = nil; engine.say("\(name): nothing picked.") }
            } else {
                openCueMenu()
                ui.cueBlockTarget = nil
                ui.cuePicks = [cue.number]
            }
            let n = ui.cuePicks.count
            if n > 0 {
                engine.say(n == 1 ? "\(name) cue \(plain(cue.number)): type the first number, then Enter or \(name == "Copy" ? "Copy" : "Renumber"), End, or tap a list's handle."
                                  : "\(name) \(n) cues: type the first number, then Enter or \(name == "Copy" ? "Copy" : "Renumber"), End, or tap a list's handle.")
            }
        case .delete:
            // UI-64: ⇧ / ⌘ on a row while this list's Delete waits picks more cues.
            let flags = NSApp.currentEvent?.modifierFlags ?? []
            if flags.contains(.shift) || flags.contains(.command), ui.cueDelete, ui.cueMenu?.playback == list.id {
                ui.cuePicks = extended(ui.cuePicks, with: cue.number, in: list.stored.cues.map(\.number), range: flags.contains(.shift))
                if let last = ui.cuePicks.last, !ui.cuePicks.contains(ui.cueMenu?.cue ?? -1) { ui.cueMenu = CueRef(playback: list.id, cue: last) }
                if ui.cuePicks.isEmpty { ui.cueMenu = nil; ui.cueDelete = false; engine.say("Delete: nothing picked.") }
                else if ui.cuePicks.count == 1, let only = list.cue(ui.cuePicks.first) { engine.sayDelete(cue: only, of: list) }
                else { engine.say("Delete \(ui.cuePicks.count) cues of \(list.homeHandle)? Confirm on softkey A.") }
                return
            }
            openCueMenu()
            ui.cueDelete = true
            ui.cuePicks = [cue.number]
            engine.sayDelete(cue: cue, of: list)
        case .legend:
            ui.cueLegendEditing = ref
        case .options:
            ui.openOptions(list.id, tab: .times, cue: cue.number)
        default:
            break   // refused above
        }
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
    @EnvironmentObject private var livePlaybacks: LivePlaybacks   // UI-69: shows live values
    @EnvironmentObject private var ui: UIState
    let ref: CueRef
    /// UI-62: the collapsed cells ("item+group") a right-click opened; reset when the cue changes.
    @State private var open: Set<String> = []

    private static let columns = ["I", "P", "C", "B"]

    var body: some View {
        let list = engine.playback(ref.playback)
        let reply = engine.cueView.flatMap { $0.playback == ref.playback && $0.cue == ref.cue ? $0 : nil }
        let fixtures = Dictionary(engine.fixtures.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
        PanelWindow(title: "Cue view · \(cueName(ref.cue, chase: list?.isChase == true))",
                    info: "\(list?.homeHandle ?? "?") · \(reply.map { "\(Set($0.fixtures.map(\.fixture)).count) fixtures" } ?? "…")",
                    accessory: {
                        Button("Back to list") { ui.cueView = nil; ui.cueValue = nil; ui.cueRemoveConfirm = false }
                            .buttonStyle(FlatButton(compact: true))
                    }) {
            if let reply, !(reply.fixtures.isEmpty && reply.effects.isEmpty) {
                ScrollView {
                    LazyVStack(spacing: 0, pinnedViews: .sectionHeaders) {
                        Section {
                            ForEach(cellsAfterFixtures(reply.fixtures, engine.fixtures) { $0.item }) { row in
                                let fixture = fixtures[row.fixture]
                                line(number: Text(ProgrammerWindow.number(row.item, fixture)).foregroundStyle(t.fg2)
                                        .overlay { RightClickCatcher { pickItem(row.item, fixture) } },
                                     name: Text(row.item.cell == nil ? fixture.map(Self.title) ?? "" : "").foregroundStyle(t.fg)) { group in
                                    cell(row, fixture, group)
                                }
                                HLine()
                            }
                            ForEach(reply.effects) { effectRow($0) }
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
        .onChange(of: ref.id) { _, _ in open = [] }
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
        let mine = row.values.filter { (row.item.cell == nil ? fixture?.attributes : fixture?.cellAttributes)?[$0.key] == group }.sorted { $0.key < $1.key }
        let off = row.dormant.filter { (row.item.cell == nil ? fixture?.attributes : fixture?.cellAttributes)?[$0.key] == group }.sorted { $0.key < $1.key }
        let key = "\(row.item.fixture).\(row.item.cell ?? 0)\(group)"
        if mine.isEmpty && off.isEmpty {
            Text("·").foregroundStyle(t.fg3)
        } else if off.isEmpty, !open.contains(key), let id = ProgrammerWindow.onePalette(mine.map(\.value.palette)) {
            // UI-62: one palette for the whole group shows once; a right-click shows the values.
            let legend = engine.palettes.first { $0.id == id }?.shownLegend ?? "palette"
            ProgrammerWindow.paletteLine(mine.map(\.value.value), legend, t).overlay { RightClickCatcher { open.insert(key) } }
        } else {
            HStack(spacing: 4) {
                ForEach(mine, id: \.key) { name, value in
                    item(row.fixture, name, value, named: mine.count > 1).overlay { RightClickCatcher { pick(row.item, name, value) } }
                }
                // CUE-61: what is off stays in sight, struck through and dim.
                ForEach(off, id: \.key) { name, value in
                    shown(name, value.value, named: Set(mine.map(\.key) + off.map(\.key)).count > 1, palette: value.palette)
                        .strikethrough().foregroundStyle(t.fg3)
                        .overlay { RightClickCatcher { pickDormant(row.item, name) } }
                }
            }
        }
    }

    private func item(_ fixture: Int, _ name: String, _ v: CueViewValue, named: Bool) -> Text {
        let text = shown(name, v.value, named: named, palette: v.palette)
        if v.palette != nil { return text.foregroundStyle(t.cyan) }
        return text.foregroundStyle(v.from == ref.cue ? t.fg : t.magenta)
    }

    /// The value as text, with the palette legend after a reference; the caller colours it.
    private func shown(_ name: String, _ value: ProgrammerValue, named: Bool, palette: Int?) -> Text {
        let text: Text
        switch value {
        case .level(let level): text = Text(named ? "\(name) \(percent(level))" : percent(level))
        case .colour(let r, let g, let b):
            text = Text("\(Text(Image(systemName: "square.fill")).font(.system(size: 8)).foregroundStyle(Color(red: r, green: g, blue: b))) \(ProgrammerWindow.hex(r, g, b))")
        }
        guard let id = palette else { return text }
        let legend = engine.palettes.first { $0.id == id }?.shownLegend ?? "palette"
        return Text("\(text) \(legend)")
    }

    /// CUE-59: look or shape, attributes, member count; magenta when it tracks from an earlier cue.
    private func effectRow(_ e: CueViewEffect) -> some View {
        let name = e.look ?? e.shape.title
        let count = e.members.count
        return VStack(spacing: 0) {
            Text("\(name) · \(e.attributes.joined(separator: ", ")) · \(count) \(count == 1 ? "fixture" : "fixtures")")
                .foregroundStyle(e.from == ref.cue ? t.fg : t.magenta)
                .lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
                .font(Typo.mono(11))
                .padding(.horizontal, 7).padding(.vertical, 3)
                .overlay {
                    RightClickCatcher {
                        guard e.from == ref.cue else {
                            return engine.say("\(name) tracks from \(cueName(e.from)): remove it there.")
                        }
                        ui.cueValue = .effect(entry: e.entry, name: name)
                        ui.cueRemoveConfirm = false
                        engine.say("\(name) picked: Remove on the softkeys takes it out of \(cueName(ref.cue)).")
                    }
                }
            HLine()
        }
    }

    /// CUE-60: a right-click on the number picks the whole item; the engine refuses what is not hard here.
    private func pickItem(_ item: Item, _ fixture: FixtureInfo?) {
        ui.cueValue = .value(fixture: item.fixture, cell: item.cell, attribute: nil)
        ui.cueRemoveConfirm = false
        engine.say("\(ProgrammerWindow.number(item, fixture)) picked: Remove on the softkeys takes it out of \(cueName(ref.cue)).")
    }

    /// CUE-61: a value that is off is picked for On.
    private func pickDormant(_ item: Item, _ name: String) {
        ui.cueValue = .dormant(fixture: item.fixture, cell: item.cell, attribute: name)
        ui.cueRemoveConfirm = false
        engine.say("\(name) is off: On on the softkeys brings it back to \(cueName(ref.cue)).")
    }

    /// Only a value that is hard in this cue can be removed from it (CUE-54).
    private func pick(_ item: Item, _ name: String, _ v: CueViewValue) {
        guard v.from == ref.cue else {
            return engine.say("\(name) tracks from \(cueName(v.from)): remove it there.")
        }
        ui.cueValue = .value(fixture: item.fixture, cell: item.cell, attribute: name)
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
                Text(verbatim: "\(legend.count)/32").font(Typo.mono(Typo.small)).foregroundStyle(t.fg2)
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
