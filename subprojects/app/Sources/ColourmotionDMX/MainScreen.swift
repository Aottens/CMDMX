import SwiftUI

// The main screen (UI-01): six zones, flush, separated by 1 px hairlines. Only the window grid
// resizes with the window; the other zones keep their size. Everything shown comes from
// `EngineClient`; nothing is computed here.

/// UI-only state shared by the zones. None of this reaches the engine.
@MainActor
final class UIState: ObservableObject {
    @Published var workspace = 0
    /// The Patch screen replaces the grid, side column and attribute bar (UI-10).
    @Published var showPatch = false
    /// Record armed in the Groups window; moves to the Record function button in UX-02c.
    @Published var recordArmed = false
}

/// Applies the app's own theme (UI-02), then lays out the zones.
struct MainScreen: View {
    @AppStorage("dayTheme") private var day = false
    @AppStorage("consoleMode") private var console = true
    @StateObject private var ui = UIState()

    var body: some View {
        Frame()
            .environmentObject(ui)
            .environment(\.theme, day ? .day : .dark)
            .preferredColorScheme(day ? .light : .dark)   // native controls follow the app, not macOS
            .background(WindowConfigurator(console: console))
    }
}

private struct Frame: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var ui: UIState
    @AppStorage("keypadVisible") private var keypad = true

    var body: some View {
        GeometryReader { geo in
            VStack(spacing: 0) {
                StatusBar().frame(height: 26)
                HLine()
                if ui.showPatch {
                    HStack(spacing: 0) {
                        Rail().frame(width: 78)
                        VLine()
                        PatchView()
                    }
                } else {
                    HStack(spacing: 0) {
                        Rail().frame(width: 78)
                        VLine()
                        Workspaces()
                        VLine()
                        SideColumn(showKeypad: keypad && geo.size.width >= 1440)
                    }
                    HLine()
                    AttributeBar().frame(height: 96)
                }
                HLine()
                PlaybackArea().frame(height: 232)
            }
        }
        .background(t.bg)
        .foregroundStyle(t.fg)
    }
}

/// All four workspaces stay alive, only one is visible: switching never rebuilds a window, so each
/// keeps its scroll position (UI-08).
private struct Workspaces: View {
    @EnvironmentObject private var ui: UIState

    var body: some View {
        ZStack {
            ForEach(Array(Workspace.all.enumerated()), id: \.offset) { index, workspace in
                let visible = index == ui.workspace
                WindowGrid(workspace: workspace)
                    .opacity(visible ? 1 : 0)
                    .allowsHitTesting(visible)
                    .accessibilityHidden(!visible)
            }
        }
    }
}

// MARK: - Status bar (UI-06)

private struct StatusBar: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @AppStorage("dayTheme") private var day = false
    @AppStorage("consoleMode") private var console = true
    @AppStorage("keypadVisible") private var keypad = true

    private static let clock: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "HH:mm:ss"
        return f
    }()

    /// Display names for the node names of IPC-20.
    private static let outputNames = ["art-net": "Art-Net", "sacn": "sACN", "serial": "Serial"]

    var body: some View {
        HStack(spacing: 14) {
            Text("Demo show").font(Typo.legend(Typo.body, .semibold)).foregroundStyle(t.fg)

            Dot(color: engine.connection == .online ? t.green : t.fg3, label: "Engine")
            if engine.connection != .online {
                Text(engine.connection == .connecting ? "Connecting to engine…" : "Engine offline — faders are not driving output")
                    .foregroundStyle(t.amber)
            }

            ForEach(engine.outputs) { output in
                let name = Self.outputNames[output.name] ?? output.name
                switch output.state {
                case .ok: Dot(color: t.green, label: name)
                case .degraded: Dot(color: t.amber, label: output.detail.map { "\(name): \($0)" } ?? name, warn: true)
                case .stopped: Dot(color: t.fg3, label: name)
                }
            }

            Spacer(minLength: 0)

            BarToggle(title: "Console", on: console) { console.toggle() }
                .help("Full screen, Dock and menu bar hidden, asks before quitting")
            BarToggle(title: "Keys", on: false, enabled: false) {}
                .help("Mac keyboard drives the keypad · UX-02c")
            BarToggle(title: "Keypad", on: keypad) { keypad.toggle() }
                .help("Hidden automatically on windows narrower than 1440 pt")
            BarToggle(title: day ? "Night" : "Day", on: false) { day.toggle() }

            TimelineView(.periodic(from: .now, by: 1)) { context in
                Text(Self.clock.string(from: context.date)).font(Typo.mono(11)).foregroundStyle(t.fg)
            }
        }
        .font(Typo.ui(11))
        .foregroundStyle(t.fg2)
        .lineLimit(1)
        .padding(.horizontal, 10)
        .frame(maxWidth: .infinity)
        .background(t.bg)
    }
}

private struct Dot: View {
    @Environment(\.theme) private var t
    let color: Color
    let label: String
    var warn = false
    var body: some View {
        HStack(spacing: 5) {
            Circle().fill(color).frame(width: 6, height: 6)
            Text(label).foregroundStyle(warn ? t.amber : t.fg2)
        }
    }
}

private struct BarToggle: View {
    @Environment(\.theme) private var t
    let title: String
    let on: Bool
    var enabled = true
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(title).font(Typo.ui(11)).foregroundStyle(on ? t.fg : t.fg2)
                .padding(.horizontal, 7).padding(.vertical, 2)
                .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(on ? t.fg2 : t.line, lineWidth: 1))
                .opacity(enabled ? 1 : 0.35)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
    }
}

// MARK: - Rail (UI-07)

private struct Rail: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var ui: UIState

    var body: some View {
        VStack(spacing: 1) {
            ForEach(Array(Workspace.all.enumerated()), id: \.offset) { index, workspace in
                RailItem(title: workspace.name, small: "F\(index + 1)", on: !ui.showPatch && ui.workspace == index) {
                    ui.workspace = index
                    ui.showPatch = false
                }
                .keyboardShortcut(Self.functionKey(index), modifiers: [])
            }
            divider
            RailItem(title: "Patch", small: "screen", on: ui.showPatch) { ui.showPatch.toggle() }
            RailItem(title: "Macros", small: "later")
            RailItem(title: "Show", small: "P1-09")
            divider
            RailItem(title: "Shapes", small: "later")
            RailItem(title: "Pixelmapper", small: "later")
            RailItem(title: "Keystone", small: "later")
            Spacer(minLength: 0)
        }
        .padding(4)
        .background(t.bg)
    }

    private var divider: some View { HLine().padding(.vertical, 4) }

    /// F1...F4 (plain keys; they go live with the Keys toggle in UX-02c, UI-19).
    private static func functionKey(_ index: Int) -> KeyEquivalent {
        // NSF1FunctionKey is U+F704; the others follow it.
        KeyEquivalent(Character(Unicode.Scalar(UInt32(0xF704 + index)) ?? " "))
    }
}

/// A rail entry; without an `action` it is disabled and its small text says what it waits for.
private struct RailItem: View {
    @Environment(\.theme) private var t
    let title: String
    let small: String
    var on = false
    var action: (() -> Void)?

    var body: some View {
        Button { action?() } label: {
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(Typo.legend()).foregroundStyle(on ? t.fg : t.fg2)
                    .multilineTextAlignment(.leading).lineLimit(2).minimumScaleFactor(0.75)
                    .fixedSize(horizontal: false, vertical: true)
                Text(small).font(Typo.mono(9)).foregroundStyle(t.fg3)
            }
            .padding(.horizontal, 6).padding(.vertical, 5)
            .frame(maxWidth: .infinity, minHeight: 44, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: radius).fill(on ? t.surf2 : Color.clear))
            .overlay(alignment: .leading) { if on { Rectangle().fill(t.fg).frame(width: 2) } }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(action == nil)
        .opacity(action == nil ? 0.35 : 1)
        .help(action == nil ? "\(title) · \(small)" : title)
    }
}
