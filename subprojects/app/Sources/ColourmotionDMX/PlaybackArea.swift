import SwiftUI

// Playback area (UI-21..24): pages column, one strip per fader on the page, the Go block and
// the Grand Master. The strips and GM are live (IPC-09); pages, Swop/Flash and the Go block are
// drawn disabled until their specs land. Full fidelity (bank frame, scroll, halos) is UX-02d.

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

private struct PagesColumn: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            // Page 1 only until paging lands (VS1-06).
            VStack(alignment: .leading, spacing: 2) {
                Text("1").font(Typo.mono(Typo.small))
                Text(engine.pageLegend).font(Typo.legend(11)).lineLimit(1)
            }
            .foregroundStyle(t.bg)
            .padding(.horizontal, 5).frame(maxWidth: .infinity, minHeight: 30, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: radius).fill(t.fg))
            .help("Paging · VS1-06")
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

private struct StripsRow: View {
    @EnvironmentObject private var engine: EngineClient

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 2) {
                ForEach(engine.slots) { slot in
                    FaderStrip(slot: slot, onChange: { engine.setFader(index: slot.id, level: $0) },
                              onTouch: { engine.touchFader(index: slot.id, held: $0) })
                }
            }
            .padding(4)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

private struct FaderStrip: View {
    @Environment(\.theme) private var t
    let slot: PlaybackSlot
    let onChange: (Double) -> Void
    let onTouch: (Bool) -> Void

    var body: some View {
        VStack(spacing: 2) {
            Text(slot.legend).font(Typo.legend()).foregroundStyle(t.fg).lineLimit(1)
                .padding(.horizontal, 5).padding(.top, 6)
                .frame(maxWidth: .infinity, maxHeight: 44, alignment: .topLeading)
                .background(t.bg)
            LevelBar(value: Binding(get: { slot.level }, set: onChange), running: slot.isActive, onTouch: onTouch)
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
        .background(RoundedRectangle(cornerRadius: radius).fill(t.surf))
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
        .clipped()
    }
}

/// Cue list Go block: drawn disabled until P1-08 (UI-23).
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
            Text("—").font(Typo.legend(13)).foregroundStyle(t.fg3)
                .frame(maxWidth: .infinity, alignment: .leading)
            VStack(alignment: .leading, spacing: 6) {
                Text("GO").font(Typo.legend(20, .semibold)).tracking(2.4).foregroundStyle(t.fg3)
                Text("no cue list").font(Typo.legend()).foregroundStyle(t.fg3)
            }
            .padding(.horizontal, 10).padding(.vertical, 8)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .background(RoundedRectangle(cornerRadius: radius).fill(t.surf))
            .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(t.line, lineWidth: 1))
            HStack(spacing: 2) {
                ForEach(["Back", "Stop", "Release"], id: \.self) { KeyButton(label: $0, spec: spec, height: 30) }
            }
        }
        .padding(6)
        .help(spec)
    }
}

private struct GrandMaster: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient

    var body: some View {
        VStack(spacing: 3) {
            Text("GM").font(Typo.legend(11)).foregroundStyle(t.fg2).frame(height: 18)
            LevelBar(value: Binding(get: { engine.grandMaster }, set: { engine.setGrandMaster($0) }), warnBelowFull: true,
                     onTouch: { engine.touchGrandMaster(held: $0) })
            KeyButton(label: "DBO", spec: "Dead black out · P1-04", height: 30)
        }
        .padding(4)
    }
}
