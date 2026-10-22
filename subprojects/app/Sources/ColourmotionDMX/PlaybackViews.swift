import AppKit
import SwiftUI

// Playbacks in the app (PBK-37..41): what a tap on a handle's legend does by the armed function
// button, the legend editors (handle and page) and the Active playbacks window. Rendering and
// command sending only: the engine records, copies, releases and latches; this file decides which
// command a tap sends.

// MARK: - Taps on handles (PBK-38)

@MainActor
enum HandleTaps {
    /// PBK-07: the slot shows a playback that Record, Copy and Move address. A `TransparentLock`
    /// playback from another page is only shown there; a record onto it makes a new playback.
    // ponytail: display copy of the engine's handle rule; the engine decides what a record or copy addresses.
    static func isUsed(_ info: PlaybackInfo?, page: Int) -> Bool {
        guard let info else { return false }
        return !(info.stored.options.paging == "TransparentLock" && info.stored.page != page)
    }

    /// A tap on the legend area of the strip for `slot` (1-based) on the current page.
    static func tap(slot: Int, info: PlaybackInfo?, engine: EngineClient, ui: UIState, line: TokenLine) {
        let page = engine.page.number
        let used = isUsed(info, page: page)
        if let info { ui.lastHandle = info.id }

        if ui.recordArmed {
            // CUE-45: BPM Master / Rate Master armed: an empty handle gets the master (IPC-26).
            if let kind = ui.recordMaster {
                guard !(info != nil && used) else { return engine.say("Pick an empty handle for the master.") }
                engine.recordMaster(page: page, slot: slot, kind: kind)
                ui.recordArmed = false
                return
            }
            // CUE-45: Record, a cue number, then a handle records to that cue (CUE-04) and empties the line.
            let cue = line.onlyNumber
            guard let info, used, cue == nil else {
                engine.recordPlayback(page: page, slot: slot, mask: ui.orderedRecordMask, recordBy: ui.recordBy,
                                      cue: cue, updateMode: ui.updateMode)
                if cue != nil { line.clear() }
                ui.recordArmed = false
                return
            }
            // A used handle needs Replace, Merge, Append or Insert (PBK-13, CUE-45): the softkeys ask.
            ui.usedCell = nil
            ui.maskMenu = false
            ui.usedHandle = UsedHandle(page: page, slot: slot, playback: info.id)
            engine.say("Handle \(page).\(slot) is used: Replace, Merge, Append or Insert on the softkeys.")
            return
        }
        guard let function = ui.function else {
            // The Options screen follows the handle tapped.
            if ui.screen == .options, let info, used { ui.optionsFor = info.id; ui.optionsCue = nil }
            // CUE-44: a plain tap on a list's or chase's legend connects it (CUE-26).
            else if let info, used, info.hasCues { engine.cuelist("connect", id: info.id) }
            return
        }
        let name = function.rawValue
        switch function {
        case .copy, .move:
            if let source = ui.handleSource {
                guard !used else { return engine.say("Pick an empty handle for the \(name.lowercased()).") }
                function == .copy ? engine.copyPlayback(source, page: page, slot: slot)
                                  : engine.movePlayback(source, page: page, slot: slot)
                ui.cancelModes()
            } else if let info, used {
                ui.source = nil
                ui.handleSource = info.id
                engine.say("\(name) playback \(info.homeHandle): tap an empty handle.")
            } else {
                engine.say("Tap a recorded playback as the source.")
            }
        case .latch:
            engine.say("Latch: press Flash on a handle.")
        case .include:
            engine.say("Include works on palettes.")
        default:
            guard let info, used else { return engine.say("\(name): that handle is empty.") }
            switch function {
            case .update:
                engine.recordPlayback(page: page, slot: slot, mode: "Update")
                ui.cancelModes()
            case .delete:
                ui.deleteTarget = nil
                ui.deletePlayback = info.id
                engine.say("Delete playback \(info.homeHandle)? Confirm on softkey A.")
            case .release:
                engine.releasePlayback(info.id)
                ui.cancelModes()
            case .options:
                ui.openOptions(info.id)
            case .legend:
                ui.playbackLegendEditing = PlaybackRef(id: info.id)
            case .copy, .move, .latch, .include:
                break
            }
        }
    }

    /// UI-18: a right-click on a strip's legend opens Playback Options.
    static func rightClick(info: PlaybackInfo?, engine: EngineClient, ui: UIState) {
        guard let info else { return }
        ui.lastHandle = info.id
        ui.openOptions(info.id)
    }

    /// CUE-44, 55: a press of the handle button `which` (upper / lower) that does `function`. A Flash
    /// with Latch armed, or on a latched playback, is `playback.latch` (on, or off again, PBK-28);
    /// true = one-shot, nothing to send on release. Everything else is `playback.button`.
    static func buttonPress(_ id: Int, which: String, function: String, info: PlaybackInfo?, engine: EngineClient, ui: UIState) -> Bool {
        if function == "Flash", ui.function == .latch || info?.live.latched == true {
            engine.latchPlayback(id)
            if ui.function == .latch { ui.cancelModes() }
            return true
        }
        engine.pressButton(id, which, on: true)
        return false
    }
}

// MARK: - Legend editors (PAL-29, PBK-38)

/// sRGB 0...1 of a SwiftUI colour, for the halo picker.
private func srgb(_ color: Color) -> AutoIcon.RGB {
    let c = NSColor(color).usingColorSpace(.sRGB) ?? .white
    return AutoIcon.RGB(r: Double(c.redComponent), g: Double(c.greenComponent), b: Double(c.blueComponent))
}

/// Legend text, icon and halo of one playback (PBK-31). "Auto" is the automatic icon and halo.
struct PlaybackLegendEditor: View {
    @Environment(\.theme) private var t
    @Environment(\.dismiss) private var dismiss
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState
    let id: Int
    @State private var legend = ""
    @State private var icon: String?
    @State private var halo: AutoIcon.RGB?
    @State private var pickerColor = Color.white
    @State private var loaded = false

    private var playback: PlaybackInfo? { engine.playback(id) }

    /// A few fixed halo colours; the picker covers the rest.
    private static let swatches: [(name: String, rgb: AutoIcon.RGB)] = [
        ("Red", .init(r: 1, g: 0.25, b: 0.25)), ("Orange", .init(r: 1, g: 0.55, b: 0)),
        ("Yellow", .init(r: 1, g: 0.85, b: 0.2)), ("Green", .init(r: 0.2, g: 0.8, b: 0.3)),
        ("Cyan", .init(r: 0.2, g: 0.8, b: 0.9)), ("Blue", .init(r: 0.25, g: 0.5, b: 1)),
        ("Violet", .init(r: 0.6, g: 0.35, b: 0.9)), ("White", .init(r: 1, g: 1, b: 1)),
    ]

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("SET LEGEND · PLAYBACK \(playback?.homeHandle ?? "")").font(Typo.legend(Typo.body, .semibold)).tracking(0.7)

            HStack(spacing: 8) {
                TextField("Legend", text: $legend).textFieldStyle(.roundedBorder).onSubmit(apply)
                    .onChange(of: legend) { _, new in if new.count > 32 { legend = String(new.prefix(32)) } }
                Text("\(legend.count)/32").font(Typo.mono(Typo.small)).foregroundStyle(t.fg2)
            }

            IconPicker(icon: $icon, autoIcon: playback?.stored.autoIcon)

            VStack(alignment: .leading, spacing: 6) {
                Text("HALO").font(Typo.legend(Typo.small)).tracking(0.8).foregroundStyle(t.fg3)
                HStack(spacing: 4) {
                    Button { halo = nil } label: {
                        Text("Auto").font(Typo.legend(Typo.small))
                            .frame(width: 44, height: 24)
                            .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(halo == nil ? t.blue : t.line, lineWidth: halo == nil ? 2 : 1))
                    }
                    .buttonStyle(.plain)
                    ForEach(Self.swatches, id: \.name) { swatch in
                        Button { halo = swatch.rgb } label: {
                            Rectangle().fill(Color(red: swatch.rgb.r, green: swatch.rgb.g, blue: swatch.rgb.b))
                                .frame(width: 24, height: 24)
                                .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(halo == swatch.rgb ? t.blue : t.line, lineWidth: halo == swatch.rgb ? 2 : 1))
                        }
                        .buttonStyle(.plain)
                        .help(swatch.name)
                    }
                    ColorPicker("Pick", selection: $pickerColor, supportsOpacity: false).labelsHidden()
                        .onChange(of: pickerColor) { _, new in halo = srgb(new) }
                }
            }

            HStack {
                Spacer()
                Button("Cancel") { close() }.keyboardShortcut(.cancelAction)
                Button("Set") { apply() }.keyboardShortcut(.defaultAction)
            }
        }
        .padding(14)
        .frame(width: 420)
        .background(t.bg)
        .foregroundStyle(t.fg)
        .onAppear {
            guard !loaded, let playback else { return }
            loaded = true
            legend = playback.stored.legend
            icon = playback.stored.icon
            halo = pickedHalo(playback.stored)
            if let halo { pickerColor = Color(red: halo.r, green: halo.g, blue: halo.b) }
        }
    }

    /// The wire carries the effective halo only (PBK-35): one equal to the automatic icon's first
    /// colour counts as Auto, anything else as picked.
    private func pickedHalo(_ stored: PlaybackStored) -> AutoIcon.RGB? {
        guard let halo = stored.halo else { return nil }
        if case .colours(let colours)? = stored.autoIcon, let first = colours.first,
           abs(first.r - halo.r) + abs(first.g - halo.g) + abs(first.b - halo.b) < 1e-4 {
            return nil
        }
        return halo
    }

    private func apply() {
        engine.setPlaybackLegend(id, legend: legend, icon: icon, halo: halo)
        close()
    }

    private func close() {
        ui.cancelModes()
        ui.playbackLegendEditing = nil
        dismiss()
    }
}

/// The legend of one page (PBK-08), 0–32 characters.
struct PageLegendEditor: View {
    @Environment(\.theme) private var t
    @Environment(\.dismiss) private var dismiss
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState
    let page: Int
    @State private var legend = ""
    @State private var loaded = false

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("SET LEGEND · PAGE \(page)").font(Typo.legend(Typo.body, .semibold)).tracking(0.7)
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
            legend = engine.pages.first { $0.number == page }?.legend ?? ""
        }
    }

    private func apply() {
        engine.setPageLegend(page, legend: legend)
        close()
    }

    private func close() {
        ui.cancelModes()
        ui.pageLegendEditing = nil
        dismiss()
    }
}

// MARK: - Active playbacks (UI-13, PBK-40)

/// Every playback that is active or releasing, in fire order (PBK-40); a tap on a row releases it.
struct ActivePlaybacksWindow: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient

    private static let widths: [CGFloat?] = [44, nil, 54, 70, 66]

    var body: some View {
        // PBK-40: fire order; releasing ones (no `fired`) after the active ones, in engine order.
        // `sorted` is stable, so equal keys keep the engine's order.
        let running = engine.playbacks.filter(\.isRunning).sorted { ($0.live.fired ?? .max) < ($1.live.fired ?? .max) }
        PanelWindow(title: "Active playbacks", info: "\(running.count) running", accessory: { EmptyView() }) {
            VStack(spacing: 0) {
                HStack(spacing: 0) {
                    ForEach(Array(["Handle", "Playback", "Level", "State", "Groups"].enumerated()), id: \.offset) { index, name in
                        Text(name.uppercased()).font(Typo.legend(Typo.small)).tracking(0.8).foregroundStyle(t.fg3)
                            .frame(width: Self.widths[index], alignment: .leading)
                            .frame(maxWidth: Self.widths[index] == nil ? .infinity : nil, alignment: .leading)
                    }
                }
                .padding(.horizontal, 7).padding(.vertical, 2)
                HLine()
                if running.isEmpty {
                    Text("Nothing running. Raise a fader.").font(Typo.ui(11)).foregroundStyle(t.fg2)
                        .frame(maxWidth: .infinity, alignment: .leading).padding(8)
                }
                ScrollView {
                    VStack(spacing: 0) {
                        ForEach(running) { row($0) }
                    }
                }
            }
        }
    }

    private func row(_ p: PlaybackInfo) -> some View {
        let releasing = p.live.state == "releasing"
        return Button { engine.releasePlayback(p.id) } label: {
            HStack(spacing: 0) {
                Text(p.homeHandle).font(Typo.mono(Typo.body)).foregroundStyle(t.fg2).frame(width: Self.widths[0], alignment: .leading)
                Text(p.shownLegend).font(Typo.legend()).foregroundStyle(t.fg).lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
                Text(percent(p.live.level)).font(Typo.mono(Typo.body)).foregroundStyle(t.fg).frame(width: Self.widths[2], alignment: .leading)
                Text(releasing ? "releasing" : "active").font(Typo.mono(Typo.small))
                    .foregroundStyle(releasing ? t.amber : t.green).frame(width: Self.widths[3], alignment: .leading)
                Text(attributeGroupOrder.filter(p.stored.groups.contains).joined()).font(Typo.mono(Typo.small))
                    .foregroundStyle(t.fg2).frame(width: Self.widths[4], alignment: .leading)
            }
            .padding(.horizontal, 7).frame(height: 26)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .overlay(alignment: .bottom) { HLine() }
        .help("Tap to release")
    }
}
