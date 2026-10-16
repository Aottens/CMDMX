import SwiftUI

/// One Titan-style workspace: preset grid on top, fader row at the bottom.
/// Everything shown comes from `EngineClient`; nothing is computed here.
struct ContentView: View {
    @EnvironmentObject private var engine: EngineClient

    var body: some View {
        VStack(spacing: 0) {
            if engine.connection != .online {
                OfflineBanner(state: engine.connection)
            }
            PresetGridPlaceholder()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            Divider()
            FaderRow()
                .frame(height: 260)
        }
        .background(Color(nsColor: .windowBackgroundColor))
    }
}

private struct OfflineBanner: View {
    let state: EngineConnection
    var body: some View {
        HStack {
            Image(systemName: "bolt.slash")
            Text(state == .connecting ? "Connecting to engine…" : "Engine offline — faders are not driving output")
                .font(.callout)
            Spacer()
        }
        .padding(8)
        .background(Color.orange.opacity(0.25))
    }
}

private struct PresetGridPlaceholder: View {
    private let columns = Array(repeating: GridItem(.flexible(), spacing: 6), count: 8)
    var body: some View {
        ScrollView {
            LazyVGrid(columns: columns, spacing: 6) {
                ForEach(0..<32, id: \.self) { i in
                    RoundedRectangle(cornerRadius: 6)
                        .fill(Color.secondary.opacity(0.15))
                        .frame(height: 56)
                        .overlay(Text("Palette \(i + 1)").font(.caption).foregroundStyle(.secondary))
                }
            }
            .padding(12)
        }
    }
}

private struct FaderRow: View {
    @EnvironmentObject private var engine: EngineClient

    var body: some View {
        HStack(alignment: .bottom, spacing: 10) {
            ForEach(engine.slots) { slot in
                FaderView(slot: slot) { level in
                    engine.setFader(index: slot.id, level: level)
                }
            }
            Divider().frame(height: 220)
            VStack(spacing: 4) {
                Text("GM").font(.caption2).foregroundStyle(.secondary)
                VerticalSlider(value: Binding(
                    get: { engine.grandMaster },
                    set: { engine.setGrandMaster($0) }
                ))
                Text(percent(engine.grandMaster)).font(.caption2.monospacedDigit())
            }
            .frame(width: 60)
        }
        .padding(12)
        .overlay(alignment: .topLeading) {
            Text(engine.pageLegend).font(.caption).foregroundStyle(.secondary).padding(6)
        }
    }
}

private struct FaderView: View {
    let slot: PlaybackSlot
    let onChange: (Double) -> Void

    var body: some View {
        VStack(spacing: 4) {
            Text(slot.legend).font(.caption).lineLimit(1)
            VerticalSlider(value: Binding(get: { slot.level }, set: onChange))
            Text(percent(slot.level)).font(.caption2.monospacedDigit())
            HStack(spacing: 4) {
                Button("Swop") {}.buttonStyle(.bordered).controlSize(.mini)
                Button("Flash") {}.buttonStyle(.bordered).controlSize(.mini)
            }
        }
        .frame(maxWidth: .infinity)
        .padding(6)
        .background(RoundedRectangle(cornerRadius: 6).fill(slot.isActive ? Color.accentColor.opacity(0.15) : Color.clear))
    }
}

/// A plain vertical slider until the fader module gets its own AppKit/Metal view.
private struct VerticalSlider: View {
    @Binding var value: Double
    var body: some View {
        GeometryReader { geo in
            Slider(value: $value, in: 0...1)
                .rotationEffect(.degrees(-90))
                .frame(width: geo.size.height, height: geo.size.width)
                .position(x: geo.size.width / 2, y: geo.size.height / 2)
        }
        .frame(width: 44, height: 150)
    }
}

func percent(_ v: Double) -> String {
    "\(Int((v * 100).rounded())) %"
}
