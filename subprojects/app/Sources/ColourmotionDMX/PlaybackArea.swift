import AppKit
import SwiftUI

// Playback area (UI-21..24): pages column, one strip per fader on the page, the Go block and
// the Grand Master. Pages, strips and GM are live (PBK-01..05, IPC-09); Swop/Flash and the Go
// block are drawn disabled until their specs land (P1-04, P1-08).

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

/// UI-21, PBK-05: one button per page, the current one highlighted; a click sends `page.set`.
private struct PagesColumn: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            // Compact so all ten fit the 232 pt zone with the "per page" read-out.
            ForEach(1...max(1, engine.page.count), id: \.self) { number in
                let current = number == engine.page.number
                Button { engine.setPage(number) } label: {
                    Text("\(number)").font(Typo.mono(Typo.small))
                        .foregroundStyle(current ? t.bg : t.fg2)
                        .padding(.horizontal, 5).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
                        .background(RoundedRectangle(cornerRadius: radius).fill(current ? t.fg : Color.clear))
                        .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(t.line, lineWidth: 1))
                }
                .buttonStyle(.plain).frame(height: 16)
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

/// UI-22: the first eight strips sit in a frame mirroring the BCF2000's bank; the rest sit loose beside it.
private struct StripsRow: View {
    @EnvironmentObject private var engine: EngineClient
    @Environment(\.theme) private var t
    private let bankSize = 8

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 2) {
                HStack(spacing: 2) { ForEach(engine.slots.prefix(bankSize)) { StripCell(slot: $0) } }
                    .padding(.top, 13).padding([.horizontal, .bottom], 3)
                    .frame(maxHeight: .infinity)
                    .overlay(alignment: .topLeading) {
                        Text("BCF2000 · faders 1–\(min(bankSize, engine.slots.count))")
                            .font(Typo.mono(9)).tracking(0.5).foregroundStyle(t.fg2).padding(.top, 1).padding(.leading, 5)
                    }
                    .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(t.line2, lineWidth: 1))
                HStack(spacing: 2) { ForEach(engine.slots.dropFirst(bankSize)) { StripCell(slot: $0) } }
                    .padding(.top, 13).padding(.bottom, 3)
                    .frame(maxHeight: .infinity)
            }
            .padding(4)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

private struct StripCell: View {
    @EnvironmentObject private var engine: EngineClient
    let slot: PlaybackSlot

    var body: some View {
        FaderStrip(slot: slot,
                   onChange: { level in
                       if let id = slot.playback { engine.setFader(playback: id, level: level) }
                   },
                   onTouch: { held in
                       if let id = slot.playback { engine.touchFader(playback: id, held: held) }
                   },
                   onScroll: { steps, flags in
                       if let id = slot.playback { engine.nudgeFader(playback: id, by: Double(steps) * faderScrollStep(flags)) }
                   })
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

/// An empty slot (PBK-05) is a disabled strip without a cap or a level.
private struct FaderStrip: View {
    @Environment(\.theme) private var t
    let slot: PlaybackSlot
    let onChange: (Double) -> Void
    let onTouch: (Bool) -> Void
    let onScroll: (Int, NSEvent.ModifierFlags) -> Void

    var body: some View {
        let empty = slot.playback == nil
        VStack(spacing: 2) {
            VStack(alignment: .leading, spacing: 3) {
                Text(empty ? "empty" : slot.legend).font(Typo.legend()).foregroundStyle(empty ? t.fg3 : t.fg).lineLimit(1)
                Text("\(slot.id + 1)").font(Typo.mono(9)).foregroundStyle(t.fg2)
            }
            .padding(.horizontal, 5).padding(.top, 6)
            .frame(maxWidth: .infinity, maxHeight: 44, alignment: .topLeading)
            .background(t.bg)
            if empty {
                Rectangle().fill(t.bg)
            } else {
                LevelBar(value: Binding(get: { slot.level }, set: onChange), running: slot.isActive,
                         onTouch: onTouch, onScroll: onScroll)
            }
            ForEach(["Swop", "Flash"], id: \.self) { name in
                Text(name.uppercased()).font(Typo.legend(11)).tracking(0.7).foregroundStyle(t.fg)
                    .frame(maxWidth: .infinity).frame(height: 24)
                    .background(t.surf2)
                    .overlay(Rectangle().strokeBorder(t.line, lineWidth: 1))
                    .opacity(t.disabled)
                    .help("Playbacks · P1-04")
            }
        }
        .padding(2)
        .frame(width: 92)
        .background(RoundedRectangle(cornerRadius: radius).fill(empty ? Color.clear : t.surf))
        .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(t.line, lineWidth: 1))
        .overlay(alignment: .bottom) {
            if slot.isActive { Rectangle().fill(t.green).frame(height: 3).padding(.horizontal, 2) }
        }
    }
}

/// Vertical level bar with a cap and the % in SF Mono. Drag to set; the engine's delta stays authoritative.
private struct LevelBar: View {
    @Environment(\.theme) private var t
    @Binding var value: Double
    var running = false
    /// Grand Master turns amber below 100 % (UI-24).
    var warnBelowFull = false
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

/// Cue list Go block: drawn disabled until P1-08 (UI-23), as in the sketch: header, "now" line,
/// the big green-outlined GO and Back / Stop / Release.
private struct GoBlock: View {
    @Environment(\.theme) private var t
    private let spec = "Cue lists · P1-08"

    var body: some View {
        VStack(spacing: 4) {
            HStack {
                Text("CUE LIST").tracking(0.5)
                Spacer()
                Text("live — · next —")
            }
            .font(Typo.mono(Typo.small)).foregroundStyle(t.fg2).lineLimit(1)
            Text("—").font(Typo.legend(13)).foregroundStyle(t.fg)
                .frame(maxWidth: .infinity, alignment: .leading)
            Button {} label: {
                VStack(alignment: .leading, spacing: 6) {
                    Text("GO").font(Typo.legend(24, .semibold)).tracking(2.9).foregroundStyle(t.green)
                    Text("no cue list").font(Typo.legend()).foregroundStyle(t.fg).lineLimit(1)
                }
                .padding(.horizontal, 10).padding(.vertical, 8)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                .background(RoundedRectangle(cornerRadius: radius).fill(t.surf))
                .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(t.green, lineWidth: 2))
            }
            .buttonStyle(.plain).disabled(true).opacity(t.disabled)
            HStack(spacing: 2) {
                ForEach(["Back", "Stop", "Release"], id: \.self) { KeyButton(label: $0, spec: spec, height: 30) }
            }
        }
        .padding(6)
        .help(spec)
    }
}

/// UI-24: a level bar like a strip, amber below 100 %; DBO waits for P1-04.
private struct GrandMaster: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient

    var body: some View {
        VStack(spacing: 3) {
            Text("GM").font(Typo.legend(11)).foregroundStyle(t.fg2).frame(height: 18)
            LevelBar(value: Binding(get: { engine.grandMaster }, set: { engine.setGrandMaster($0) }), warnBelowFull: true,
                     onTouch: { engine.touchGrandMaster(held: $0) })
            KeyButton(label: "DBO", spec: "DBO · P1-04", height: 30)
        }
        .padding(4)
    }
}
