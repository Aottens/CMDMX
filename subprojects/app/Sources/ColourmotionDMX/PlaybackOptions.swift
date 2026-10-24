import SwiftUI

// Playback Options screen (UI-10, PBK-39, CUE-46): the options and times of one playback. Every
// control shows the engine's value and sends its change at once (`playback.options`, `cue.times`,
// `settings.playback`); nothing is computed here, and a refused value simply stays as it was (the
// warning shows below). The tabs follow the kind: a master has Handle and Master only.

enum PlaybackOptionsTab: String, CaseIterable {
    case fader = "Fader", times = "Times", release = "Release", playback = "Playback", handle = "Handle"
    case cueList = "Cue list", chase = "Chase", master = "Master", settings = "Settings"

    /// The tab's label; the Settings tab says it is show-wide (CUE-46).
    var title: String { self == .settings ? "Settings · show-wide" : rawValue }

    static func tabs(for playback: PlaybackInfo) -> [PlaybackOptionsTab] {
        if playback.stored.isMaster { return [.handle, .master, .settings] }
        let memory = !playback.isChase && !playback.isCueList // CUE-15, 31: no fader modes on lists
        return (memory ? [.fader] : []) + [.times, .release, .playback, .handle, .cueList] + (playback.isChase ? [.chase] : []) + [.settings]
    }
}

struct PlaybackOptionsView: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState

    var body: some View {
        let playback = ui.optionsFor.flatMap { engine.playback($0) }
        PanelWindow(
            title: "Playback options",
            info: playback.map { "\($0.homeHandle) · \($0.stored.legend.isEmpty ? "no legend" : $0.stored.legend)" } ?? "—",
            framed: true,
            accessory: { EmptyView() }
        ) {
            if let playback {
                let tabs = PlaybackOptionsTab.tabs(for: playback)
                let shown = tabs.contains(ui.optionsTab) ? ui.optionsTab : tabs[0]
                VStack(alignment: .leading, spacing: 0) {
                    HStack(spacing: 2) {
                        ForEach(tabs, id: \.self) { tab in
                            Button(tab.title) { ui.optionsTab = tab }
                                .buttonStyle(FlatButton(fill: shown == tab ? .blue : nil))
                        }
                        Spacer(minLength: 0)
                        // Full size and blue, at the end of the tabs (VS3-05 run: the title-row Exit was hard to find).
                        Button("Exit") { ui.screen = nil }.buttonStyle(FlatButton(fill: .blue)).keyboardShortcut(.cancelAction)
                    }
                    .padding(8)
                    HLine()
                    ScrollView {
                        VStack(alignment: .leading, spacing: 14) {
                            switch shown {
                            case .fader: FaderTab(playback: playback)
                            case .times: TimesTab(playback: playback)
                            case .release: ReleaseTab(playback: playback)
                            case .playback: PlaybackTab(playback: playback)
                            case .handle: HandleTab(playback: playback)
                            case .cueList: CueListTab(playback: playback)
                            case .chase: ChaseTab(playback: playback)
                            case .master: MasterTab(playback: playback)
                            case .settings: SettingsTab()
                            }
                            if let message = engine.lastMessage {
                                Text(message).font(Typo.ui(11)).foregroundStyle(t.amber)
                            }
                        }
                        .frame(maxWidth: 560, alignment: .leading)
                        .padding(14)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
                .id(playback.id)
                // The "Options: tap a playback handle" prompt is done once the screen is open (VS3-05 run).
                .onAppear { engine.clearMessage() }
            } else {
                Text("This playback no longer exists.").font(Typo.ui(11)).foregroundStyle(t.fg2).padding(14)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            }
        }
        .padding(8)
        .background(t.bg)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

// MARK: - Controls

/// A label over its control, with an optional note under it.
private struct OptionRow<Content: View>: View {
    @Environment(\.theme) private var t
    let label: String
    var note = ""
    @ViewBuilder var content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(label.uppercased()).font(Typo.legend(Typo.small)).tracking(0.8).foregroundStyle(t.fg3)
            content
            if !note.isEmpty { Text(note).font(Typo.ui(11)).foregroundStyle(t.fg2) }
        }
    }
}

/// A row of exclusive choices; the selected one is the engine's value.
private struct Choice<Value: Hashable>: View {
    let options: [(label: String, value: Value)]
    let selected: Value
    let pick: (Value) -> Void

    var body: some View {
        HStack(spacing: 2) {
            ForEach(Array(options.enumerated()), id: \.offset) { _, option in
                Button(option.label) { pick(option.value) }
                    .buttonStyle(FlatButton(fill: option.value == selected ? .blue : nil))
            }
        }
    }
}

/// A number the operator types; it is sent on Return or when the field loses focus. The engine's
/// value is shown again at once, and replaced by the new one when the engine accepts the change.
private struct NumberField: View {
    @Environment(\.theme) private var t
    let value: Double
    let unit: String
    var width: CGFloat = 90
    let commit: (Double) -> Void
    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        HStack(spacing: 6) {
            TextField("", text: $text).textFieldStyle(.roundedBorder).frame(width: width)
                .font(Typo.mono()).focused($focused).onSubmit(send)
                .onChange(of: focused) { _, now in if !now { send() } }
            Text(unit).font(Typo.ui(11)).foregroundStyle(t.fg2)
        }
        .onAppear { text = plain(value) }
        .onChange(of: value) { _, new in if !focused { text = plain(new) } }
    }

    /// Shows what was sent (VS3-05 run: showing the old value until the engine answered looked
    /// like the field had jumped back); a refused value shows its warning below.
    private func send() {
        if let n = Double(text.trimmingCharacters(in: .whitespaces).replacingOccurrences(of: ",", with: ".")), n != value {
            commit(n)
            text = plain(n)
        } else {
            text = plain(value)
        }
    }
}

// MARK: - Tabs

private struct FaderTab: View {
    @EnvironmentObject private var engine: EngineClient
    let playback: PlaybackInfo
    private var options: PlaybackStored.Options { playback.stored.options }

    var body: some View {
        OptionRow(label: "Fader mode", note: [
            0: "Mode 0: the intensity follows the fader; position, colour and beam snap when the fader fires it.",
            1: "Mode 1: as mode 0, and pulling the fader down fades the intensity out over the fade out time.",
            2: "Mode 2: times are ignored; the fader crossfades every value from what was there to the cue.",
        ][options.faderMode] ?? "") {
            Choice(options: [("0", 0), ("1", 1), ("2", 2)], selected: options.faderMode) {
                engine.setPlaybackOptions(playback.id, ["fader_mode": $0])
            }
        }
        OptionRow(label: "Kill point", note: "When the playback leaves the merge after the fader reaches 0.") {
            Choice(options: [("Fade out complete", "FadeOutComplete"), ("Fader at 0", "FaderAtZero")], selected: options.killPoint) {
                engine.setPlaybackOptions(playback.id, ["kill_point": $0])
            }
        }
    }
}

/// The times, link and track of one cue, and its group times (CUE-07, 14, 20, 51, 46). The cue is
/// picked here; it starts on the one Edit Times was opened for, else the current, else the first.
private struct TimesTab: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState
    let playback: PlaybackInfo
    @State private var picked: Double?

    private var cue: StoredCue? {
        let cues = playback.stored.cues
        return [picked, ui.optionsCue, playback.live.cue?.current].compactMap { $0 }.lazy.compactMap { playback.cue($0) }.first ?? cues.first
    }

    var body: some View {
        if let cue {
            picker(cue)
            Text("Times of \(cueName(cue.number)): 0 to 600 s, overlap 0 to 100 %.").font(Typo.ui(11)).foregroundStyle(.secondary)
            times(cue)
            linkAndTrack(cue)
            groupTimes(cue)
        } else {
            Text("This playback holds no cues.").font(Typo.ui(11)).foregroundStyle(t.fg2)
        }
    }

    private func picker(_ cue: StoredCue) -> some View {
        OptionRow(label: "Cue") {
            Picker("Cue", selection: Binding(get: { cue.number }, set: { picked = $0 })) {
                ForEach(playback.stored.cues) { c in
                    Text(c.legend.isEmpty ? cueName(c.number) : "\(cueName(c.number)) \(c.legend)").tag(c.number)
                }
            }
            .labelsHidden().pickerStyle(.menu).frame(width: 220)
        }
    }

    @ViewBuilder private func times(_ cue: StoredCue) -> some View {
        let id = playback.id
        OptionRow(label: "Delay") { NumberField(value: cue.delay, unit: "s") { engine.setCueTimes(id, cue: cue.number, ["delay": $0]) } }
        OptionRow(label: "Fade in") { NumberField(value: cue.fadeIn, unit: "s") { engine.setCueTimes(id, cue: cue.number, ["fade_in": $0]) } }
        OptionRow(label: "Fade out") { NumberField(value: cue.fadeOut, unit: "s") { engine.setCueTimes(id, cue: cue.number, ["fade_out": $0]) } }
        OptionRow(label: "Overlap") { NumberField(value: cue.overlap, unit: "%") { engine.setCueTimes(id, cue: cue.number, ["overlap": $0]) } }
    }

    @ViewBuilder private func linkAndTrack(_ cue: StoredCue) -> some View {
        let id = playback.id
        let link = cue.link
        OptionRow(label: "Link", note: "Wait: starts on Go. After: N s after the cue before has finished. With: N s after it started.") {
            Choice(options: [("Wait", "Wait"), ("After", "After"), ("With", "With")], selected: link.type) { type in
                let wire: [String: Any] = type == "Wait" ? ["type": type] : ["type": type, "offset": link.offset]
                engine.setCueTimes(id, cue: cue.number, ["link": wire])
            }
            if link.type != "Wait" {
                NumberField(value: link.offset, unit: "s") {
                    engine.setCueTimes(id, cue: cue.number, ["link": ["type": link.type, "offset": $0] as [String: Any]])
                }
            }
        }
        if !playback.isChase {
            OptionRow(label: "Track", note: "What the cue does with the values before it (tracking lists).") {
                Choice(options: [("Track", "Track"), ("Block", "Block"), ("Cue only", "CueOnly"), ("Solo", "Solo")], selected: cue.track) {
                    engine.setCueTrack(id, cue: cue.number, track: $0)
                }
            }
        }
    }

    private func groupTimes(_ cue: StoredCue) -> some View {
        OptionRow(label: "Group times", note: "Delay and fade per attribute group; — uses the cue's own times.") {
            HStack(alignment: .top, spacing: 6) {
                if !cue.groupTimes.isEmpty {
                    VStack(alignment: .trailing, spacing: 3) {
                        Text("").frame(height: 14)
                        Text("delay").frame(height: 24)
                        Text("fade").frame(height: 24)
                    }
                    .font(Typo.ui(11)).foregroundStyle(t.fg2)
                }
                ForEach(Array(attributeGroupOrder.prefix(7)), id: \.self) { group in groupColumn(cue, group) }
            }
        }
    }

    private func groupColumn(_ cue: StoredCue, _ group: String) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(group).font(Typo.mono(Typo.small, .semibold)).foregroundStyle(Theme.groupHue(group)).frame(height: 14)
            if let time = cue.groupTimes[group] {
                NumberField(value: time.delay, unit: "", width: 54) { send(cue, group, GroupTime(delay: $0, fade: time.fade)) }.frame(height: 24)
                HStack(spacing: 2) {
                    NumberField(value: time.fade, unit: "", width: 54) { send(cue, group, GroupTime(delay: time.delay, fade: $0)) }
                    Button("×") { send(cue, group, nil) }.buttonStyle(FlatButton(compact: true)).fixedSize().help("Back to the cue's own times")
                }
                .frame(height: 24)
            } else {
                Button("—") { send(cue, group, GroupTime(delay: cue.delay, fade: cue.fadeIn)) }
                    .buttonStyle(FlatButton()).help("Give \(group) its own times")
                if !cue.groupTimes.isEmpty { Spacer().frame(height: 24) }
            }
        }
    }

    /// `group_times` replaces the cue's whole map; a nil time takes the group out of it.
    private func send(_ cue: StoredCue, _ group: String, _ time: GroupTime?) {
        var map = cue.groupTimes.mapValues { ["delay": $0.delay, "fade": $0.fade] }
        map[group] = time.map { ["delay": $0.delay, "fade": $0.fade] }
        engine.setCueTimes(playback.id, cue: cue.number, ["group_times": map])
    }
}

private struct ReleaseTab: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    let playback: PlaybackInfo
    private var options: PlaybackStored.Options { playback.stored.options }
    private var global: PlaybackSettings? { engine.playbackSettings }

    var body: some View {
        let local = options.releaseMask != nil
        OptionRow(label: "Release mask", note: local ? "" : "Global: \(maskText(global?.releaseMask ?? [])) (Settings).") {
            Choice(options: [("Global", false), ("Local", true)], selected: local) { toLocal in
                // Local starts from the global mask; Global gives the playback's own mask up.
                engine.setPlaybackOptions(playback.id, ["release_mask": toLocal ? (global?.releaseMask ?? []) as Any : NSNull()])
            }
            if let mask = options.releaseMask {
                HStack(spacing: 2) {
                    ForEach(attributeGroupOrder, id: \.self) { group in
                        let on = mask.contains(group)
                        Button(group) {
                            let next = attributeGroupOrder.filter { $0 == group ? !on : mask.contains($0) }
                            engine.setPlaybackOptions(playback.id, ["release_mask": next])
                        }
                        .buttonStyle(FlatButton(fill: on ? .blue : nil))
                    }
                }
            }
        }
        OptionRow(label: "Release time",
                  note: options.releaseTime == nil ? "Global: \(plain(global?.releaseTime ?? 0)) s (Settings)." : "") {
            Choice(options: [("Global", false), ("Own", true)], selected: options.releaseTime != nil) { own in
                engine.setPlaybackOptions(playback.id, ["release_time": own ? (global?.releaseTime ?? 0) as Any : NSNull()])
            }
            if let time = options.releaseTime {
                NumberField(value: time, unit: "s") { engine.setPlaybackOptions(playback.id, ["release_time": $0]) }
            }
        }
    }

    private func maskText(_ mask: [String]) -> String { mask.isEmpty ? "nothing releases" : attributeGroupOrder.filter(mask.contains).joined() }
}

private struct PlaybackTab: View {
    @EnvironmentObject private var engine: EngineClient
    let playback: PlaybackInfo
    private var options: PlaybackStored.Options { playback.stored.options }

    var body: some View {
        OptionRow(label: "Priority", note: "Merges as Titan's priority levels; a change applies from the next tick.") {
            Choice(options: [("Low", "Low"), ("Normal", "Normal"), ("High", "High"), ("Programmer", "Programmer"), ("Very high", "VeryHigh")],
                   selected: options.priority) { engine.setPlaybackOptions(playback.id, ["priority": $0]) }
        }
        OptionRow(label: "Playback group", note: "Raising one playback of a group takes the others of that group out.") {
            Picker("Group", selection: Binding(get: { options.group ?? 0 },
                                               set: { engine.setPlaybackOptions(playback.id, ["group": $0 == 0 ? NSNull() : $0 as Any]) })) {
                Text("None").tag(0)
                ForEach(1...99, id: \.self) { Text("\($0)").tag($0) }
            }
            .labelsHidden().pickerStyle(.menu).frame(width: 110)
        }
    }
}

private struct HandleTab: View {
    @EnvironmentObject private var engine: EngineClient
    let playback: PlaybackInfo

    var body: some View {
        OptionRow(label: "Handle paging", note: [
            "Unlocked": "Shown on its home page only.",
            "TransparentLock": "Shown at its slot on every page whose own slot is empty.",
            "Locked": "Shown at its slot on every page.",
        ][playback.stored.options.paging] ?? "") {
            Choice(options: [("Unlocked", "Unlocked"), ("Transparent lock", "TransparentLock"), ("Locked", "Locked")],
                   selected: playback.stored.options.paging) { engine.setPlaybackOptions(playback.id, ["paging": $0]) }
        }
        Text("Home handle \(playback.homeHandle).").font(Typo.ui(11)).foregroundStyle(.secondary)
    }
}

// MARK: - Cue list, chase, master and show-wide settings (CUE-46)

private func yesNo(_ on: Bool) -> [(label: String, value: Bool)] { [("On", true), ("Off", false)] }

/// The kind of the playback, and the options of a cue list (CUE-38); Renumber All (CUE-08).
private struct CueListTab: View {
    @EnvironmentObject private var engine: EngineClient
    let playback: PlaybackInfo
    private var list: ListOptions { playback.stored.list }

    var body: some View {
        let id = playback.id
        OptionRow(label: "Kind", note: "A chase steps through its cues by itself (Chase tab).") {
            Choice(options: [("Cues", "Cues"), ("Chase", "Chase")], selected: playback.stored.kind) {
                engine.setPlaybackOptions(id, ["kind": $0])
            }
        }
        if !playback.isChase {
            OptionRow(label: "Tracking", note: "On: a cue holds only what changed. Off: every cue holds everything.") {
                Choice(options: yesNo(true), selected: list.tracking) { engine.setPlaybackOptions(id, ["list": ["tracking": $0]]) }
            }
            OptionRow(label: "Fader", note: "What 0 means: Kill with Off keeps the cue and only the intensity goes out.") {
                Choice(options: [("Kill with Off", "KillWithOff"), ("Kill at 0", "KillAtZero")], selected: list.fader) {
                    engine.setPlaybackOptions(id, ["list": ["fader": $0]])
                }
            }
            OptionRow(label: "Fire first cue", note: "Raising the fader of a list that is off runs its first cue.") {
                Choice(options: yesNo(true), selected: list.fireFirstCue) { engine.setPlaybackOptions(id, ["list": ["fire_first_cue": $0]]) }
            }
            OptionRow(label: "Loop") {
                Choice(options: [("Stop on final", "StopOnFinal"), ("Loop", "Loop")], selected: list.loop) {
                    engine.setPlaybackOptions(id, ["list": ["loop": $0]])
                }
            }
        }
        OptionRow(label: "Cue numbers", note: "Renumber All makes the cues Q1, Q2, Q3 … in their order.") {
            Button("Renumber All") { engine.renumberAllCues(id) }.buttonStyle(FlatButton())
        }
    }
}

/// The options of a chase (CUE-29, 30, 38).
private struct ChaseTab: View {
    @EnvironmentObject private var engine: EngineClient
    let playback: PlaybackInfo
    private var chase: ChaseOptions { playback.stored.chase }

    var body: some View {
        let id = playback.id
        OptionRow(label: "Kind") {
            Choice(options: [("Cues", "Cues"), ("Chase", "Chase")], selected: playback.stored.kind) {
                engine.setPlaybackOptions(id, ["kind": $0])
            }
        }
        OptionRow(label: "BPM", note: "Used while the speed source is Free (1 to 600).") {
            NumberField(value: chase.bpm, unit: "BPM") { engine.setPlaybackOptions(id, ["chase": ["bpm": $0]]) }
        }
        OptionRow(label: "Crossfade", note: "0 snaps each step; 100 fades over the whole step.") {
            NumberField(value: chase.crossfade, unit: "%") { engine.setPlaybackOptions(id, ["chase": ["crossfade": $0]]) }
        }
        OptionRow(label: "Direction") {
            Choice(options: ["Forward", "Backward", "Bounce", "Random"].map { ($0, $0) }, selected: chase.direction) {
                engine.setPlaybackOptions(id, ["chase": ["direction": $0]])
            }
        }
        OptionRow(label: "Loop") {
            Choice(options: [("Loop", "Loop"), ("Stop on final", "StopOnFinal")], selected: chase.loop) {
                engine.setPlaybackOptions(id, ["chase": ["loop": $0]])
            }
        }
        OptionRow(label: "Speed source", note: "Free uses the BPM above; a BPM or Rate master drives the chase.") {
            Choice(options: [("Free", "Free"), ("BPM master", "Bpm"), ("Rate master", "Rate")], selected: chase.speedSource.kind) { kind in
                var source = SpeedSource()
                source.kind = kind
                source.number = kind == chase.speedSource.kind ? chase.speedSource.number : 1
                engine.setPlaybackOptions(id, ["chase": ["speed_source": source.wire]])
            }
            if chase.speedSource.kind != "Free" {
                let kind = chase.speedSource.kind
                Picker("Master", selection: Binding(get: { chase.speedSource.number }, set: {
                    var source = chase.speedSource
                    source.number = $0
                    engine.setPlaybackOptions(id, ["chase": ["speed_source": source.wire]])
                })) {
                    ForEach(1...(kind == "Bpm" ? 8 : 4), id: \.self) { Text("\(kind == "Bpm" ? "BPM" : "Rate") \($0)").tag($0) }
                }
                .labelsHidden().pickerStyle(.menu).frame(width: 110)
            }
        }
    }
}

/// A master's settings: multiplier and fader for a BPM master, scale for a Rate master (CUE-35, 36),
/// mode, scale and Disabled for a group master (GRM-17).
private struct MasterTab: View {
    @EnvironmentObject private var engine: EngineClient
    let playback: PlaybackInfo

    var body: some View {
        let id = playback.id
        if let m = playback.stored.master, playback.stored.isGroupMaster {
            let mode = m.mode ?? "Scale"
            OptionRow(label: "Master") {
                Text(verbatim: "Group \(m.group ?? 0)").font(Typo.mono())
            }
            OptionRow(label: "Mode", note: "Scale: the fader scales the group's intensity. HTP: it is a floor. Limit: it is a ceiling. Take Over: it puts a fixture in the programmer once the level reaches its intensity.") {
                Choice(options: [("Scale", "Scale"), ("HTP", "Htp"), ("Limit", "Limit"), ("Take Over", "TakeOver")], selected: mode) {
                    engine.setPlaybackOptions(id, ["master": ["mode": $0]])
                }
            }
            if mode == "Scale" {
                OptionRow(label: "Scale", note: "The fader runs the group from 0 to this many % of its level.") {
                    Choice(options: [100, 200, 400, 600, 1000].map { ("\($0) %", $0) }, selected: m.scale ?? 100) {
                        engine.setPlaybackOptions(id, ["master": ["scale": $0]])
                    }
                }
            }
            OptionRow(label: "Disabled", note: "Disabled locks the group at the master's last level; the fader is ignored until it is enabled again.") {
                Choice(options: yesNo(true), selected: m.enabled == false) { engine.setPlaybackOptions(id, ["master": ["enabled": !$0]]) }
            }
        } else if let m = playback.stored.master {
            OptionRow(label: "Master") {
                Text(verbatim: "\(m.rateScale == nil ? "BPM" : "Rate") \(m.number)").font(Typo.mono())
            }
            if let rate = m.rateScale {
                OptionRow(label: "Rate scale", note: "The fader runs a chase at 0 to this many % of its speed.") {
                    Choice(options: [("100 %", 100), ("200 %", 200)], selected: rate) {
                        engine.setPlaybackOptions(id, ["master": ["rate_scale": $0]])
                    }
                }
            } else {
                OptionRow(label: "Multiplier", note: "Scales the tempo this master gives the chases that follow it.") {
                    Choice(options: [("/4", 0.25), ("/2", 0.5), ("×1", 1.0), ("×2", 2.0), ("×4", 4.0)], selected: m.multiplier ?? 1) {
                        engine.setPlaybackOptions(id, ["master": ["multiplier": $0]])
                    }
                }
                OptionRow(label: "Fader", note: "None: the fader does nothing. Bpm: it sets the tempo. Multiplier: its zones pick the multiplier.") {
                    Choice(options: [("None", "None"), ("Bpm", "Bpm"), ("Multiplier", "Multiplier")], selected: m.fader ?? "None") {
                        engine.setPlaybackOptions(id, ["master": ["fader": $0]])
                    }
                }
                OptionRow(label: "Fader maximum", note: "The tempo at fader 100 % (1 to 600).") {
                    NumberField(value: m.faderMax ?? 240, unit: "BPM") { engine.setPlaybackOptions(id, ["master": ["fader_max": $0]]) }
                }
            }
        }
    }
}

/// The show-wide playback settings (CUE-38, CUE-25): they apply to every playback, not this one.
private struct SettingsTab: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient

    private static let kinds = [("Memory", "Memory"), ("CueList", "Cue list"), ("Chase", "Chase"), ("BpmMaster", "BPM master"), ("GroupMaster", "Group master")]
    // ponytail: display copy of CUE-25's BestPractice pairs and of what each kind may have; it only
    // seeds the Custom editor, the engine decides what a handle's buttons do.
    private static let best = ["Memory": ["Swop", "Flash"], "CueList": ["Go", "Flash"], "Chase": ["Tap", "Flash"], "BpmMaster": ["None", "Tap"], "GroupMaster": ["None", "Flash"]]
    private static let allowed = ["Memory": ["Flash", "Swop", "Release"], "CueList": ["Go", "Back", "Stop", "Flash", "Swop", "Release"],
                                  "Chase": ["Tap", "Flash", "Swop", "Release"], "BpmMaster": ["Tap"], "GroupMaster": ["Flash", "Swop"]]

    var body: some View {
        if let s = engine.playbackSettings {
            Text("These settings are show-wide: they apply to every playback.").font(Typo.ui(11)).foregroundStyle(t.fg2)
            OptionRow(label: "Auto connect", note: "A list that fires or gets Go from its own handle becomes the connected list.") {
                Choice(options: yesNo(true), selected: s.autoConnect) { engine.setPlaybackSettings(["auto_connect": $0]) }
            }
            OptionRow(label: "Cue list fader", note: "The fader option new lists start with.") {
                Choice(options: [("Kill with Off", "KillWithOff"), ("Kill at 0", "KillAtZero")], selected: s.cueListFader) {
                    engine.setPlaybackSettings(["cue_list_fader": $0])
                }
            }
            OptionRow(label: "Update mode", note: "Where Update stores a changed value on a tracking list.") {
                Choice(options: [("Forwards", "Forwards"), ("Cue only", "CueOnly"), ("Backwards", "Backwards"), ("Both", "Both")],
                       selected: s.updateMode) { engine.setPlaybackSettings(["update_mode": $0]) }
            }
            OptionRow(label: "Handle buttons", note: "Best practice: Go on lists, Tap on chases. Titan: Swop / Flash everywhere. Custom: a pair per kind.") {
                Choice(options: [("Best practice", "BestPractice"), ("Titan", "Titan"), ("Custom", "Custom")], selected: s.handleButtons.mode) { mode in
                    engine.setPlaybackSettings(["handle_buttons": mode == "Custom" ? ["Custom": s.handleButtons.custom] : mode])
                }
                if s.handleButtons.mode == "Custom" {
                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(Self.kinds, id: \.0) { kind, title in
                            let pair = s.handleButtons.custom[kind] ?? Self.best[kind] ?? ["None", "None"]
                            HStack(spacing: 6) {
                                Text(title).font(Typo.ui(11)).foregroundStyle(t.fg2).frame(width: 80, alignment: .leading)
                                ForEach(0..<2, id: \.self) { index in
                                    Picker(index == 0 ? "Upper" : "Lower", selection: Binding(get: { pair[index] }, set: {
                                        var next = pair
                                        next[index] = $0
                                        var custom = s.handleButtons.custom
                                        custom[kind] = next
                                        engine.setPlaybackSettings(["handle_buttons": ["Custom": custom]])
                                    })) {
                                        ForEach((Self.allowed[kind] ?? []) + ["None"], id: \.self) { Text($0).tag($0) }
                                    }
                                    .labelsHidden().pickerStyle(.menu).frame(width: 100)
                                }
                            }
                        }
                    }
                }
            }
        } else {
            Text("Waiting for the engine.").font(Typo.ui(11)).foregroundStyle(t.fg2)
        }
        programmerSection
    }

    /// UI-26 / PRG-43: the show-wide programmer settings; each change sends only its own field.
    @ViewBuilder private var programmerSection: some View {
        if let p = engine.programmerSettings {
            Text("PROGRAMMER").font(Typo.legend(Typo.small)).tracking(0.8).foregroundStyle(t.fg3).padding(.top, 8)
            OptionRow(label: "Clear order", note: "What the first Clear presses release, in turn.") {
                Picker("Clear order", selection: Binding(get: { p.clearPrecedence }, set: { engine.setProgrammerSettings(["clear_precedence": $0]) })) {
                    Text("Selection, then programmer").tag("SelectionThenProgrammer")
                    Text("Selection with programmer").tag("SelectionWithProgrammer")
                    Text("Programmer, then selection").tag("ProgrammerThenSelection")
                }
                .labelsHidden().pickerStyle(.menu).frame(width: 240)
            }
            OptionRow(label: "LTP on Clear", note: "Release: LTP attributes fall back to the playbacks. Freeze: they hold their value.") {
                Choice(options: [("Release", "Release"), ("Freeze", "Freeze")], selected: p.clearLtp) {
                    engine.setProgrammerSettings(["clear_ltp": $0])
                }
            }
            OptionRow(label: "Lowlight level", note: "The level Rem Dim gives the fixtures that are not selected.") {
                NumberField(value: (p.lowlightLevel * 100).rounded(), unit: "%") {
                    engine.setProgrammerSettings(["lowlight_level": min(max($0, 0), 100) / 100])
                }
            }
        }
    }
}
