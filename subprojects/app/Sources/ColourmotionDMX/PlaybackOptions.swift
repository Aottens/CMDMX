import SwiftUI

// Playback Options screen (UI-10, PBK-39): the options and times of one playback. Every control
// shows the engine's value and sends its change at once (`playback.options`, `playback.times`);
// nothing is computed here, and a refused value simply stays as it was (the warning shows below).

enum PlaybackOptionsTab: String, CaseIterable {
    case fader = "Fader", times = "Times", release = "Release", playback = "Playback", handle = "Handle"
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
            accessory: {
                Button("Exit") { ui.screen = nil }.buttonStyle(FlatButton(compact: true)).keyboardShortcut(.cancelAction)
            }
        ) {
            if let playback {
                VStack(alignment: .leading, spacing: 0) {
                    HStack(spacing: 2) {
                        ForEach(PlaybackOptionsTab.allCases, id: \.self) { tab in
                            Button(tab.rawValue) { ui.optionsTab = tab }
                                .buttonStyle(FlatButton(fill: ui.optionsTab == tab ? .blue : nil))
                        }
                        Spacer(minLength: 0)
                    }
                    .padding(8)
                    HLine()
                    ScrollView {
                        VStack(alignment: .leading, spacing: 14) {
                            switch ui.optionsTab {
                            case .fader: FaderTab(playback: playback)
                            case .times: TimesTab(playback: playback)
                            case .release: ReleaseTab(playback: playback)
                            case .playback: PlaybackTab(playback: playback)
                            case .handle: HandleTab(playback: playback)
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
    let commit: (Double) -> Void
    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        HStack(spacing: 6) {
            TextField("", text: $text).textFieldStyle(.roundedBorder).frame(width: 90)
                .font(Typo.mono()).focused($focused).onSubmit(send)
                .onChange(of: focused) { _, now in if !now { send() } }
            Text(unit).font(Typo.ui(11)).foregroundStyle(t.fg2)
        }
        .onAppear { text = plain(value) }
        .onChange(of: value) { _, new in if !focused { text = plain(new) } }
    }

    private func send() {
        if let n = Double(text.trimmingCharacters(in: .whitespaces).replacingOccurrences(of: ",", with: ".")), n != value {
            commit(n)
        }
        text = plain(value)
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

private struct TimesTab: View {
    @EnvironmentObject private var engine: EngineClient
    let playback: PlaybackInfo
    private var times: PlaybackStored.Times { playback.stored.times }

    var body: some View {
        Text("Times of the playback's cue: 0 to 600 s, overlap 0 to 100 %.")
            .font(Typo.ui(11)).foregroundStyle(.secondary)
        OptionRow(label: "Delay") {
            NumberField(value: times.delay, unit: "s") { engine.setPlaybackTimes(playback.id, ["delay": $0]) }
        }
        OptionRow(label: "Fade in") {
            NumberField(value: times.fadeIn, unit: "s") { engine.setPlaybackTimes(playback.id, ["fade_in": $0]) }
        }
        OptionRow(label: "Fade out") {
            NumberField(value: times.fadeOut, unit: "s") { engine.setPlaybackTimes(playback.id, ["fade_out": $0]) }
        }
        OptionRow(label: "Overlap") {
            NumberField(value: times.overlap, unit: "%") { engine.setPlaybackTimes(playback.id, ["overlap": $0]) }
        }
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
