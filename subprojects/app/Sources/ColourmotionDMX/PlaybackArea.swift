import AppKit
import SwiftUI

// Playback area (UI-21..24): pages column, one strip per fader on the page, the Go block and
// the Grand Master. Pages, strips, their two handle buttons, GM and DBO are live (PBK-01..05,
// PBK-37, PBK-41, IPC-09, CUE-44); the Go block is in CueLists.swift. What a tap on a legend does
// by the armed function button is in PlaybackViews.swift (HandleTaps).

func percent(_ v: Double) -> String {
    "\(Int((v * 100).rounded())) %"
}

/// 232 pt zone along the bottom.
struct PlaybackArea: View {
    var body: some View {
        HStack(spacing: 0) {
            PagesColumn().frame(width: 64)
            VLine()
            StripsRow()
            VLine()
            GoBlock().frame(width: 210)
            VLine()
            GrandMaster().frame(width: 70)
        }
        .frame(maxWidth: .infinity)
    }
}

/// UI-21, PBK-05: one button per page with its legend (PBK-08), the current one highlighted; a click
/// sends `page.set`. With Set Legend armed a click edits the page's legend (PBK-38).
private struct PagesColumn: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            // Compact so all ten fit the 232 pt zone with the "per page" read-out.
            ForEach(1...max(1, engine.page.count), id: \.self) { number in
                let current = number == engine.page.number
                let legend = engine.pages.first { $0.number == number }?.legend ?? ""
                Button {
                    if ui.function == .legend { ui.pageLegendEditing = PageRef(number: number) } else { engine.setPage(number) }
                } label: {
                    HStack(spacing: 3) {
                        Text("\(number)").font(Typo.mono(Typo.small))
                        Text(legend).font(Typo.legend(9)).lineLimit(1)
                        Spacer(minLength: 0)
                    }
                    .foregroundStyle(current ? t.bg : t.fg2)
                    .padding(.horizontal, 5).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
                    .background(RoundedRectangle(cornerRadius: radius).fill(current ? t.fg : Color.clear))
                    .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(ui.function == .legend ? t.red : t.line, lineWidth: 1))
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain).frame(height: 16)
                .help(legend.isEmpty ? "Page \(number)" : "Page \(number): \(legend)")
            }
            Spacer(minLength: 0)
            Text("per page").font(Typo.mono(9)).foregroundStyle(t.fg3)
            Text("\(engine.fadersPerPage)").font(Typo.mono(Typo.small)).foregroundStyle(t.fg)
                .padding(.horizontal, 4).padding(.vertical, 2).frame(maxWidth: .infinity, alignment: .leading)
                .background(RoundedRectangle(cornerRadius: radius).fill(t.surf))
                .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(t.line, lineWidth: 1))
        }
        .padding(4)
    }
}

/// UI-22: `faders_per_page` strips side by side; a bank frame comes with a control surface (VS1-07).
private struct StripsRow: View {
    @EnvironmentObject private var engine: EngineClient

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 2) { ForEach(engine.slots) { StripCell(slot: $0) } }
                .padding(.top, 13).padding([.horizontal, .bottom], 3)
                .frame(maxHeight: .infinity)
                .padding(4)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

private struct StripCell: View {
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState
    @EnvironmentObject private var line: TokenLine
    let slot: PlaybackSlot

    /// CUE-44, 55: a handle button presses and releases `playback.button`; the engine does what the
    /// setting `handle_buttons` assigns. Latch armed (or a latched playback) turns a Flash press
    /// into `playback.latch` (PBK-28).
    private func hold(_ index: Int) -> HoldActions {
        let function = slot.info?.stored.buttons[safe: index] ?? "None"
        let which = index == 0 ? "upper" : "lower"
        return HoldActions(press: { HandleTaps.buttonPress($0, which: which, function: function, info: slot.info, engine: engine, ui: ui) },
                           release: { engine.pressButton($0, which, on: false) })
    }

    var body: some View {
        FaderStrip(slot: slot, page: engine.page.number,
                   selected: slot.playback.map { ui.handleSource == $0 || ui.deletePlayback == $0 } ?? false,
                   connected: slot.playback != nil && slot.playback == engine.connected,
                   armed: ui.recordArmed || (ui.function != nil && ui.function != .include && ui.function != .latch),
                   onChange: { level in
                       if let id = slot.playback { engine.setFader(playback: id, level: level) }
                   },
                   onTouch: { held in
                       if let id = slot.playback { engine.touchFader(playback: id, held: held) }
                   },
                   onScroll: { steps, flags in
                       if let id = slot.playback { engine.nudgeFader(playback: id, by: Double(steps) * faderScrollStep(flags)) }
                   },
                   onLegendTap: { HandleTaps.tap(slot: slot.id + 1, info: slot.info, engine: engine, ui: ui, line: line) },
                   onRightClick: { HandleTaps.rightClick(info: slot.info, engine: engine, ui: ui) },
                   upper: hold(0), lower: hold(1))
    }
}

/// Level change per scroll step: Option fine, Shift coarse (UI-22, as the wheels' IPC-16).
private func faderScrollStep(_ flags: NSEvent.ModifierFlags) -> Double {
    switch resolution(flags) {
    case .fine: return 0.005
    case .coarse: return 0.1
    case .normal: return 0.02
    }
}

/// Press and hold for Swop and Flash (PBK-37). `press` returns true when the press was a one-shot
/// (Latch), so there is nothing to send on release.
struct HoldActions {
    let press: (Int) -> Bool
    let release: (Int) -> Void
}

/// An empty slot (PBK-05) is a strip without a cap or a level, its Swop and Flash disabled; its
/// legend area still takes taps (Record, Copy, Move onto an empty handle, PBK-38).
private struct FaderStrip: View {
    @Environment(\.theme) private var t
    let slot: PlaybackSlot
    let page: Int
    /// Copy / Move source or Delete target: blue border, as a selected cell (UI-05).
    let selected: Bool
    /// CUE-26: the strip of the connected list has a 2 pt blue frame (UI-03).
    let connected: Bool
    /// A function button is armed: the legends get a red hairline (the sketch's `.armed .leg`).
    let armed: Bool
    let onChange: (Double) -> Void
    let onTouch: (Bool) -> Void
    let onScroll: (Int, NSEvent.ModifierFlags) -> Void
    let onLegendTap: () -> Void
    let onRightClick: () -> Void
    let upper: HoldActions
    let lower: HoldActions

    var body: some View {
        let empty = slot.playback == nil
        let functions = slot.info?.stored.buttons ?? ["None", "None"]
        VStack(spacing: 2) {
            StripLegend(info: slot.info, page: page)
                .overlay { if armed { Rectangle().strokeBorder(t.red, lineWidth: 1) } }
                .overlay { if selected { Rectangle().strokeBorder(t.blue, lineWidth: 2) } }
                .onTapGesture(perform: onLegendTap)
                .overlay { RightClickCatcher(action: onRightClick) }
                .help(empty ? "Empty handle \(page).\(slot.id + 1)" : "Right-click: Playback Options")
            if let info = slot.info {
                LevelBar(value: Binding(get: { info.live.level }, set: onChange), running: slot.isActive,
                         progress: slot.isActive ? info.live.cue.flatMap { $0.current != nil && $0.progress < 1 ? $0.progress : nil } : nil,
                         onTouch: onTouch, onScroll: onScroll)
            } else {
                Rectangle().fill(t.bg)
            }
            ForEach(0..<2, id: \.self) { index in
                let function = functions[safe: index] ?? "None"
                HoldKey(label: function == "None" ? "" : function, target: function == "None" ? nil : slot.playback,
                        inverted: function == "Flash" && slot.info?.live.latched == true, actions: index == 0 ? upper : lower)
            }
        }
        .padding(2)
        .frame(width: 92)
        .background(RoundedRectangle(cornerRadius: radius).fill(empty ? Color.clear : t.surf))
        .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(connected ? t.blue : t.line, lineWidth: connected ? 2 : 1))
        .overlay(alignment: .bottom) {
            // UI-03: green = running, amber = releasing.
            if slot.isActive || slot.isReleasing {
                Rectangle().fill(slot.isActive ? t.green : t.amber).frame(height: 3).padding(.horizontal, 2)
            }
        }
    }
}

/// The legend area (44 pt, PBK-37): halo as a 3 pt top bar, the icon, the legend (else `page.slot`),
/// and the lock mark with the home page when a lock shows the playback away from home.
private struct StripLegend: View {
    @Environment(\.theme) private var t
    let info: PlaybackInfo?
    let page: Int

    var body: some View {
        ZStack(alignment: .topLeading) {
            t.bg
            if let info {
                // Text left, a 26 pt icon right (owner, VS2-22 run: the icon above the text was too small).
                HStack(alignment: .top, spacing: 3) {
                    VStack(alignment: .leading, spacing: 1) {
                        Text(info.shownLegend).font(Typo.legend()).foregroundStyle(t.fg).lineLimit(2)
                        meta(info)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    icon(info).frame(width: 26, height: 26)
                }
                .padding(.horizontal, 5).padding(.top, 6)
                if let halo = info.stored.halo {
                    Rectangle().fill(Color(red: halo.r, green: halo.g, blue: halo.b)).frame(height: 3)
                }
            } else {
                Text("empty").font(Typo.legend()).foregroundStyle(t.fg3).padding(.horizontal, 5).padding(.top, 6)
            }
        }
        .frame(maxWidth: .infinity).frame(height: 44)
        .clipped()
        .contentShape(Rectangle())
    }

    @ViewBuilder private func icon(_ info: PlaybackInfo) -> some View {
        if let id = info.stored.icon, let entry = PaletteIcons.entry(id) {
            IconGlyph(entry: entry, size: 15).foregroundStyle(t.fg)
        } else {
            AutoIconView(icon: info.stored.autoIcon).clipped()
        }
    }

    /// "page 2" with a lock for Locked / TransparentLock away from the home page, else the home
    /// handle when a legend hides it.
    @ViewBuilder private func meta(_ info: PlaybackInfo) -> some View {
        let paging = info.stored.options.paging
        HStack(spacing: 3) {
            if paging != "Unlocked" && info.stored.page != page {
                Image(systemName: paging == "Locked" ? "lock.fill" : "lock").font(.system(size: 8))
                Text("page \(info.stored.page)")
            } else if let summary = cueSummary(info) {
                Text(summary)
            } else if !info.stored.legend.isEmpty {
                Text(info.homeHandle)
            }
        }
        .font(Typo.mono(9)).foregroundStyle(t.fg2).lineLimit(1)
    }
}

/// CUE-44: the legend's second line on a list ("Q3 › 4"), a chase ("2/8 · 120") or a master
/// ("BPM 1 · 120 ×1", "RATE 2 · ×1.0"); nil for a memory.
private func cueSummary(_ info: PlaybackInfo) -> String? {
    if let m = info.stored.master {
        if let scale = m.rateScale { return "RATE \(m.number) · ×" + String(format: "%.1f", info.live.level * Double(scale) / 100) }
        return "BPM \(m.number) · \(Int((info.live.bpm ?? m.bpm ?? 0).rounded())) ×\(plain(m.multiplier ?? 1))"
    }
    guard let cue = info.live.cue else { return nil }
    if info.isChase {
        return "\(cue.current.map(plain) ?? "–")/\(info.stored.cues.count) · \(Int((info.live.bpm ?? info.stored.chase.bpm).rounded()))"
    }
    return "Q\(cue.current.map(plain) ?? "–") › \(cue.next.map(plain) ?? "–")"
}

/// A handle button: `press` on mouse down, `release` on mouse up (PBK-37). Disabled without a
/// playback or a function (UI-28). A latched Flash is drawn inverted.
private struct HoldKey: View {
    @Environment(\.theme) private var t
    let label: String
    let target: Int?
    let inverted: Bool
    let actions: HoldActions
    @State private var held: Int?
    @State private var oneShot = false

    var body: some View {
        let down = held != nil
        Text(label.uppercased()).font(Typo.legend(11)).tracking(0.7).foregroundStyle(inverted ? t.bg : t.fg)
            .frame(maxWidth: .infinity).frame(height: 24)
            .background(inverted ? t.fg : down ? t.line2 : t.surf2)
            .overlay(Rectangle().strokeBorder(t.line, lineWidth: 1))
            .opacity(target == nil ? t.disabled : 1)
            .contentShape(Rectangle())
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { _ in
                        guard held == nil, let target else { return }
                        held = target
                        oneShot = actions.press(target)
                    }
                    .onEnded { _ in end() }
            )
            .onDisappear { end() }   // a strip that goes away mid-press must not leave Flash on
            .help(target == nil ? (label.isEmpty ? "No function" : "Empty handle") : "\(label): hold")
    }

    private func end() {
        guard let id = held else { return }
        held = nil
        if !oneShot { actions.release(id) }
        oneShot = false
    }
}

/// Catches a right click without taking left clicks, scrolls or drags from the view below.
struct RightClickCatcher: NSViewRepresentable {
    let action: () -> Void

    func makeNSView(context: Context) -> CatcherView { CatcherView() }
    func updateNSView(_ view: CatcherView, context: Context) { view.action = action }

    final class CatcherView: NSView {
        var action: () -> Void = {}

        override func hitTest(_ point: NSPoint) -> NSView? {
            guard let event = NSApp.currentEvent, event.type == .rightMouseDown else { return nil }
            return super.hitTest(point)
        }

        override func rightMouseDown(with event: NSEvent) { action() }
    }
}

/// Vertical level bar with a cap and the % in SF Mono. Drag to set; the engine's delta stays authoritative.
private struct LevelBar: View {
    @Environment(\.theme) private var t
    @Binding var value: Double
    var running = false
    /// Grand Master turns amber below 100 % (UI-24).
    var warnBelowFull = false
    /// CUE-44: the running transition of a cue list, 0...1 (a green line along the bottom).
    var progress: Double?
    /// UI-22 / IPC-05: true on the first drag event, false on release (motor-fader touch).
    var onTouch: (Bool) -> Void = { _ in }
    /// UI-22: scroll over the bar moves the level (whole steps, modifiers as for the wheels).
    var onScroll: ((Int, NSEvent.ModifierFlags) -> Void)?
    @State private var touching = false

    var body: some View {
        GeometryReader { geo in
            let height = geo.size.height
            let fill: Color = warnBelowFull && value < 1 ? t.amber.opacity(0.45) : running ? t.green.opacity(0.4) : t.line2
            ZStack(alignment: .bottom) {
                Rectangle().fill(t.bg)
                Rectangle().fill(fill).frame(height: height * value)
                Rectangle().fill(t.fg).frame(height: 2).offset(y: -(height * value) + 1)
                if let progress { Rectangle().fill(t.green).frame(width: geo.size.width * progress, height: 3).frame(maxWidth: .infinity, alignment: .leading) }
                Text(percent(value)).font(Typo.mono(11)).foregroundStyle(t.fg)
                    .padding(.top, 3).padding(.trailing, 4)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topTrailing)
            }
            .contentShape(Rectangle())
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { drag in
                        if !touching { touching = true; onTouch(true) }
                        value = height > 0 ? (1 - drag.location.y / height).clamped01 : value
                    }
                    .onEnded { _ in touching = false; onTouch(false) }
            )
        }
        .background { if let onScroll { ScrollCatcher(onSteps: onScroll) } }
        .clipped()
    }
}

/// UI-24: a level bar like a strip, amber below 100 %; DBO under it toggles `dbo.set`, amber while
/// the engine reports it on (PBK-41).
private struct GrandMaster: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient

    var body: some View {
        VStack(spacing: 3) {
            Text("GM").font(Typo.legend(11)).foregroundStyle(t.fg2).frame(height: 18)
            LevelBar(value: Binding(get: { engine.grandMaster }, set: { engine.setGrandMaster($0) }), warnBelowFull: true,
                     onTouch: { engine.touchGrandMaster(held: $0) })
            KeyButton(label: "DBO", spec: "Dead Black Out: all intensity to 0 (toggle)", height: 30,
                      lit: engine.dbo, action: { engine.setDBO(!engine.dbo) })
        }
        .padding(4)
    }
}
