import AppKit
import SwiftUI

// The main screen (UI-01): six zones, flush, separated by 1 px hairlines. Only the window grid
// resizes with the window; the other zones keep their size. Everything shown comes from
// `EngineClient`; nothing is computed here.

/// A function button armed for palettes (UI-16, PAL-29); the next palette tap uses it.
enum PaletteFunction: String {
    case update = "Update", include = "Include", copy = "Copy", move = "Move", delete = "Delete", legend = "Set Legend"
}

/// A used cell tapped while Record is armed (PAL-29): the softkeys ask how to record onto it.
struct UsedCell: Equatable {
    let window: String
    let handle: Int
    let number: Int
}

/// The palette whose legend editor is open.
struct PaletteRef: Identifiable {
    let number: Int
    var id: Int { number }
}

/// UI-only state shared by the zones. None of this reaches the engine.
@MainActor
final class UIState: ObservableObject {
    enum Screen { case patch, show }

    @Published var workspace = 0
    /// A full screen (Patch UI-10, Show SHW-17) replaces the grid, side column and attribute bar;
    /// nil = the workspace windows. The screens exclude each other.
    @Published var screen: Screen?
    /// Record armed with the Record function button (UI-16); the next group or palette tap records.
    /// Disarming forgets the record choices below (they hold for one record only, PAL-29).
    @Published var recordArmed = false { didSet { if !recordArmed { resetRecord() } } }
    /// Record + tap on a used cell: waiting for Replace / Merge / Quick Merge (PAL-06).
    @Published var usedCell: UsedCell?
    /// Record choices for this record only (PAL-29); nil = the window's mask / the engine's setting.
    @Published var recordMask: Set<String>?
    @Published var recordBy: String?
    @Published var recordAs = "Auto"
    /// The Mask softkeys (IPCGBES toggles) are showing instead of the Record softkeys.
    @Published var maskMenu = false
    /// Update, Include, Copy, Move, Delete or Set Legend armed (PAL-29).
    @Published var function: PaletteFunction?
    /// Copy / Move: the id of the source palette, after its tap.
    @Published var source: Int?
    /// Delete: the palette number waiting for the confirmation on softkey A (two-step, PAT-10).
    @Published var deleteTarget: Int?
    @Published var legendEditing: PaletteRef?
    /// Set Overlap (PAL-29): the overlap in % for the next palette tap only.
    @Published var pendingOverlap: Double?
    /// The Palette softkeys (PAL-29): opened by the Palette key on an empty line, they stay open
    /// until Cancel, Exit or another context is chosen.
    @Published var paletteMenu = false

    func resetRecord() {
        usedCell = nil; recordMask = nil; recordBy = nil; recordAs = "Auto"; maskMenu = false
    }

    /// Disarms Record and every palette function.
    func cancelModes() {
        recordArmed = false
        function = nil; source = nil; deleteTarget = nil
        paletteMenu = false
    }

    /// A function button: arms it (disarming the others), or disarms it when it is the armed one.
    func arm(_ f: PaletteFunction) {
        let on = function != f
        cancelModes()
        if on { function = f }
    }

    /// The Record choices as the command wants them: console order.
    var orderedRecordMask: [String]? {
        recordMask.map { mask in attributeGroupOrder.filter(mask.contains) }
    }

    /// Rail entries and the workspace keys (F1–F4, ⌘1–⌘4) all come here.
    func show(workspace index: Int) {
        workspace = index
        screen = nil
    }
}

/// Applies the app's own theme (UI-02), then lays out the zones.
struct MainScreen: View {
    @AppStorage("dayTheme") private var day = false
    @AppStorage("consoleMode") private var console = true
    @StateObject private var ui = UIState()
    @StateObject private var line = TokenLine()

    var body: some View {
        Frame()
            .environmentObject(ui)
            .environmentObject(line)
            .environment(\.theme, day ? .day : .dark)
            .preferredColorScheme(day ? .light : .dark)   // native controls follow the app, not macOS
            .background(WindowConfigurator(console: console))
    }
}

private struct Frame: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var ui: UIState
    @EnvironmentObject private var line: TokenLine
    @EnvironmentObject private var engine: EngineClient
    @AppStorage("keypadVisible") private var keypad = true
    @AppStorage("dayTheme") private var day = false
    @State private var keyMonitor: Any?

    var body: some View {
        GeometryReader { geo in
            VStack(spacing: 0) {
                StatusBar().frame(height: 26)
                HLine()
                if let screen = ui.screen {
                    HStack(spacing: 0) {
                        Rail().frame(width: 78)
                        VLine()
                        switch screen {
                        case .patch: PatchView()
                        case .show: ShowView()
                        }
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
        .sheet(item: $ui.legendEditing, onDismiss: { ui.cancelModes() }) { ref in
            LegendEditor(number: ref.number)
                .environmentObject(engine).environmentObject(ui)
                .environment(\.theme, t)
                .preferredColorScheme(day ? .light : .dark)
        }
        .onAppear {
            // Mac keys (UI-19): the monitor stays installed; the handler checks the Keys toggle.
            keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [ui, line, engine] event in
                MainActor.assumeIsolated { MacKeys.handle(event, ui: ui, line: line, engine: engine) } ? nil : event
            }
        }
        .onDisappear { keyMonitor.map(NSEvent.removeMonitor) }
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
    @AppStorage("keysLive") private var keys = false

    private static let clock: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "HH:mm:ss"
        return f
    }()

    /// Display names for the node names of IPC-20.
    private static let outputNames = ["art-net": "Art-Net", "sacn": "sACN", "serial": "Serial"]

    var body: some View {
        HStack(spacing: 14) {
            ShowLabel()

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
            BarToggle(title: "Keys", on: keys) { keys.toggle() }
                .help("Mac keyboard drives the keypad and the command line · ⌘1–⌘4 always switch workspaces")
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

/// SHW-16: show name and autosave age. Before the first snapshot and while offline the last known
/// name stays, greyed, and the age is hidden (it would only count up on stale data).
private struct ShowLabel: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient

    var body: some View {
        let show = engine.show
        let online = engine.connection == .online
        HStack(spacing: 8) {
            Text(show.name.isEmpty ? "—" : show.name).font(Typo.legend(Typo.body, .semibold))
                .foregroundStyle(online ? t.fg : t.fg3)
            if online {
                TimelineView(.periodic(from: .now, by: 1)) { context in
                    Text(show.ageText(now: context.date)).foregroundStyle(show.autosaveOk ? t.fg2 : t.amber)
                }
            }
        }
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
                .opacity(enabled ? 1 : t.disabled)
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
                RailItem(title: workspace.name, small: "F\(index + 1)", on: ui.screen == nil && ui.workspace == index) {
                    ui.show(workspace: index)
                }
                // F1–F4 follow the Keys toggle (MacKeys); ⌘1–⌘4 always work (UI-19).
                .keyboardShortcut(KeyEquivalent(Character("\(index + 1)")), modifiers: .command)
            }
            divider
            RailItem(title: "Patch", small: "screen", on: ui.screen == .patch) { ui.screen = ui.screen == .patch ? nil : .patch }
            RailItem(title: "Macros", small: "later")
            RailItem(title: "Show", small: "screen", on: ui.screen == .show) { ui.screen = ui.screen == .show ? nil : .show }
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
        .opacity(action == nil ? t.disabled : 1)
        .help(action == nil ? "\(title) · \(small)" : title)
    }
}
