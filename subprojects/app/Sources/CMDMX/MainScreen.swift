import AppKit
import SwiftUI

// The main screen (UI-01): six zones, flush, separated by 1 px hairlines. Only the window grid
// resizes with the window; the other zones keep their size. Everything shown comes from
// `EngineClient`; nothing is computed here.

/// A function button armed for palettes and playback handles (UI-16, PAL-29, PBK-38); the next
/// palette or handle tap uses it. Release, Options and Latch only mean something on handles.
enum PaletteFunction: String {
    case update = "Update", include = "Include", copy = "Copy", move = "Move", delete = "Delete", legend = "Set Legend"
    case release = "Release", options = "Options", latch = "Latch", unfold = "Unfold"
}

/// What a tap can land on while Record or a function is armed (UI-42).
enum ArmedTarget { case group, fixture, cell, palette, handle, cue, effectCell, activeRow }

/// CUE-62: Record stays armed while a list or chase is built; the handle is known after its first tap.
struct Building: Equatable {
    let kind: String   // "Cues" or "Chase"
    var page: Int?
    var slot: Int?
}

/// A used handle tapped while Record is armed (PBK-38): the softkeys ask how to record onto it.
struct UsedHandle: Equatable {
    let page: Int
    let slot: Int
    let playback: Int
}

/// The playback whose legend editor is open (PBK-38).
struct PlaybackRef: Identifiable {
    let id: Int
}

/// The page whose legend editor is open (PBK-38).
struct PageRef: Identifiable {
    let number: Int
    var id: Int { number }
}

/// The group whose legend editor is open (PRG-94).
struct GroupRef: Identifiable {
    let number: Int
    var id: Int { number }
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

/// A cue of a cue list (CUE-43): what the Cue softkeys, Record on a row, the Cue view and the cue
/// legend editor act on.
struct CueRef: Equatable, Identifiable {
    let playback: Int
    let cue: Double
    var id: String { "\(playback)/\(cue)" }
}

/// What was last right-clicked in the Cue view (CUE-54, 59, 60): what Remove takes out.
enum CuePick: Equatable {
    case value(fixture: Int, cell: Int?, attribute: String?)   // attribute nil = the whole item (CUE-60)
    case effect(entry: Int, name: String)
    case dormant(fixture: Int, cell: Int?, attribute: String)   // CUE-61: a value that is off
}

/// UI-51: cues (list order) selected in one time column; `column` is the `cue.times` key, `anchor` the Shift-tap start.
struct TimeCells: Equatable {
    var playback: Int
    var column: String
    var title: String
    var cues: [Double]
    var anchor: Double
}

/// UI-only state shared by the zones. None of this reaches the engine.
@MainActor
final class UIState: ObservableObject {
    enum Screen { case patch, views, output, control, show, remote, options }

    @Published var workspace = 0
    /// UI-39: the Messages sheet.
    @Published var messagesOpen = false
    /// UI-38: the Lock prompt, and the digits typed into it or into the cover.
    @Published var lockPrompt = false
    @Published var lockCode = ""
    /// UI-38: Enter on the prompt locks, on the cover unlocks; the engine judges the code.
    func enterLockCode(_ engine: EngineClient) {
        engine.setLock(!engine.locked, code: lockCode)
        lockCode = ""; lockPrompt = false
    }
    /// UI-48: the Lock softkey and the status-bar Lock button (⌘L) both open the prompt; Learn ends first.
    func openLockPrompt(_ engine: EngineClient) {
        if learnMode { setLearnMode(false, engine) }
        lockCode = ""; lockPrompt = true
    }
    /// A full screen (Patch UI-10, Show SHW-17, Playback Options PBK-39) replaces the grid, side column and attribute bar;
    /// nil = the workspace windows. The screens exclude each other.
    /// UI-45: opening a screen disarms Record and every function, as hiding the keys must not leave one live.
    @Published var screen: Screen? { didSet { if screen != nil { cancelModes() } } }
    /// UI-44: any armed mode or open menu that Esc cancels first.
    var modesOpen: Bool { recordArmed || function != nil || cueMenu != nil || cueRemoveConfirm || paletteMenu || selectionMenu != nil }
    /// UI-44: Esc just left a text field, so a field that commits on losing focus restores instead.
    var fieldEscaped = false
    /// Record armed with the Record function button (UI-16); the next group or palette tap records.
    /// Disarming forgets the record choices below (they hold for one record only, PAL-29).
    @Published var recordArmed = false { didSet { if recordArmed { timeCells = nil } else { resetRecord() } } }
    /// Record + tap on a used cell: waiting for Replace / Merge / Quick Merge (PAL-06).
    @Published var usedCell: UsedCell? { didSet { if usedCell != nil { usedCue = nil; usedGroup = nil } } }
    /// Record + tap on a used handle: waiting for Replace / Merge (PBK-38).
    @Published var usedHandle: UsedHandle? { didSet { if usedHandle != nil { usedCue = nil; usedGroup = nil } } }
    /// CUE-62: Create List / Create Chase; cleared with the rest of Record (resetRecord), so Exit, Esc and a screen end it.
    @Published var building: Building?
    /// CUE-62: Record's tap on a group, palette or the command line; while building those do their normal action.
    var recordTargets: Bool { recordArmed && building == nil }
    /// Record + tap on a used group: waiting for Replace / Merge (PRG-97).
    @Published var usedGroup: Int?
    /// Record choices for this record only (PAL-29); nil = the window's mask / the engine's setting.
    @Published var recordMask: Set<String>?
    @Published var recordBy: String?
    @Published var recordAs = "Auto"
    /// The Mask softkeys (IPCGBES toggles) are showing instead of the Record softkeys.
    @Published var maskMenu = false
    /// Update, Include, Copy, Move, Delete or Set Legend armed (PAL-29).
    @Published var function: PaletteFunction? { didSet { if function != nil { timeCells = nil } } }
    /// UI-29: fixtures (ids) whose cells show in the Fixtures window. View state only.
    @Published var unfolded = Set<Int>()
    /// Copy / Move: the id of the source palette, after its tap.
    @Published var source: Int?
    /// Delete: the palette number waiting for the confirmation on softkey A (two-step, PAT-10).
    @Published var deleteTarget: Int?
    @Published var legendEditing: PaletteRef?
    /// UI-57: the fixture whose info sheet is open.
    @Published var fixtureInfo: FixtureRef?
    /// Copy / Move on handles: the source playback's id (PBK-38).
    @Published var handleSource: Int?
    /// Delete on handles: the playback waiting for the confirmation on softkey A.
    @Published var deletePlayback: Int?
    @Published var playbackLegendEditing: PlaybackRef?
    @Published var pageLegendEditing: PageRef?
    /// Copy / Move on groups: the source group's number (PRG-94).
    @Published var groupSource: Int?
    /// Delete on groups: the group waiting for the confirmation on softkey A.
    @Published var deleteGroup: Int?
    @Published var groupLegendEditing: GroupRef?
    /// The handle last tapped (a playback id): the Playback Options and Edit Times softkeys open it.
    @Published var lastHandle: Int?
    /// The playback shown on the Playback Options screen and its tab (PBK-39).
    @Published var optionsFor: Int?
    @Published var optionsTab = PlaybackOptionsTab.fader
    /// Cue lists (CUE-43..45, 54): the row a right-click opened the Cue softkeys on, Delete waiting
    /// for its confirmation on softkey A, the row tapped while Record is armed, this record's update
    /// mode (nil = the setting) and a master record armed ("Bpm" / "Rate").
    @Published var cueMenu: CueRef?
    @Published var cueDelete = false
    @Published var usedCue: CueRef? { didSet { if usedCue != nil { usedGroup = nil } } }
    @Published var updateMode: String?
    /// PBK-49: the Update softkeys; playback keys ("id/cue") unticked, palettes ticked, the softkey page.
    @Published var updateOff = Set<String>()
    @Published var updateOn = Set<Int>()
    @Published var updatePage = 0
    @Published var recordMaster: String?
    /// The group picked for a Group master record (GRM-15), waiting for an empty handle.
    @Published var recordMasterGroup: Int?
    /// The Cue view shown in place of the Cue list window, the hard value last right-clicked in it
    /// and Remove waiting for its confirmation.
    @Published var cueView: CueRef?
    /// UI-51: the Cue list window shows the timing columns; the cells selected in one column of them.
    @Published var cueTimesView = false
    /// UI-53: the memory (one-cue playback) the Cue list window shows in place of the connected list.
    @Published var cueMemory: Int?
    /// UI-53: the memory shown, while it still is one (deleted or appended into a list, it stops showing).
    func liveCueMemory(_ engine: EngineClient) -> PlaybackInfo? {
        cueMemory.flatMap { engine.playback($0) }.flatMap { $0.isMemory ? $0 : nil }
    }
    /// UI-53: leave the memory, and its Cue view with it.
    func endCueMemory() {
        if cueMemory != nil, cueView?.playback == cueMemory { cueView = nil; cueValue = nil; cueRemoveConfirm = false }
        cueMemory = nil
    }
    @Published var timeCells: TimeCells?
    @Published var cueValue: CuePick?
    @Published var cueRemoveConfirm = false
    @Published var cueLegendEditing: CueRef?
    /// The cue the Times tab opens on (Edit Times on a row).
    @Published var optionsCue: Double?
    /// Set Overlap (PAL-29): the overlap in % for the next palette tap only.
    @Published var pendingOverlap: Double?
    /// CTL-42: Learn mode. A click on a fader, handle button, Go block key, page button, GM or
    /// programmer key arms `control.learn` for that control's action instead of using it.
    @Published var learnMode = false
    /// The Palette softkeys (PAL-29): opened by the Palette key on an empty line, they stay open
    /// until Cancel, Exit or another context is chosen.
    @Published var paletteMenu = false
    /// UI-33: the Selection sub-context on show, "Order" or "Pattern"; nil = Selection itself.
    @Published var selectionMenu: String?

    /// Wraps a control's click: in Learn mode it arms Learn for `action` (nil: this control has none, so
    /// it runs), else it runs. The mode stays on after `Learned: …`.
    func learnOrRun(_ action: String?, _ engine: EngineClient, run: () -> Void) {
        guard learnMode, let action else { return run() }
        engine.controlLearn(action)
        engine.say("Move a control for \(action) … (Esc cancels)")
    }

    func setLearnMode(_ on: Bool, _ engine: EngineClient) {
        learnMode = on
        if !on { engine.controlLearnCancel(); engine.clearMessage() }
    }

    func resetRecord() {
        usedCell = nil; usedHandle = nil; usedGroup = nil; recordMask = nil; recordBy = nil; recordAs = "Auto"; maskMenu = false
        usedCue = nil; updateMode = nil; recordMaster = nil; recordMasterGroup = nil; building = nil
    }

    /// UI-42: the one table. nil = the tap may go on (allowed, or nothing armed); else the message that
    /// names where the armed Record or function works.
    func refusal(on target: ArmedTarget) -> String? {
        if recordArmed, let b = building {
            // CUE-62: only a handle stores; a cue row is refused, everything else does its normal action.
            guard target == .cue || target == .activeRow else { return nil }
            let at = b.slot.map { " \(b.page ?? 0).\($0)" } ?? ""
            return "Building\(at): tap a handle, Exit ends."
        }
        if recordArmed {
            let ok: [ArmedTarget] = [.group, .palette, .handle, .cue]
            return ok.contains(target) ? nil : "Record: tap a group, palette, cue or handle."
        }
        guard let function else { return nil }
        let name = function.rawValue
        if function == .latch { return "Latch: press Flash on a handle." }
        if function == .release, target == .activeRow { return nil }
        let places: [ArmedTarget]
        switch function {
        case .update: places = [.palette, .handle, .cue]
        case .include: places = [.palette, .handle, .cue]
        case .copy, .move, .delete, .legend: places = [.group, .palette, .handle, .cue]
        case .options: places = [.handle, .cue]
        case .release: places = [.handle]
        case .unfold: places = [.fixture]
        case .latch: places = []
        }
        if places.contains(target) { return nil }
        if target == .fixture || target == .cell, [.delete, .copy, .move, .legend].contains(function) {
            return "\(name): fixtures are changed on the Patch screen."
        }
        if target == .effectCell, places.contains(.palette) { return "\(name): tap the palette in its own window." }
        let names: [ArmedTarget: String] = [.group: "groups", .fixture: "fixtures", .palette: "palettes", .handle: "playback handles", .cue: "cues"]
        let list = places.compactMap { names[$0] }
        let joined = list.count > 1 ? list.dropLast().joined(separator: ", ") + " and " + list[list.count - 1] : list.joined()
        let hint = function == .update && target == .group ? " To add to a group: Record, then Merge." : ""
        return "\(name) works on \(joined).\(hint)"
    }

    /// UI-42: true when the tap on `target` may go on. A refusal is said and nothing else happens. An allowed
    /// tap off a cue closes the Cue softkeys, which win over the function softkeys in the side column.
    func armedAllows(_ target: ArmedTarget, _ engine: EngineClient) -> Bool {
        if let message = refusal(on: target) {
            engine.say(message)
            return false
        }
        if (recordArmed || function != nil), target != .cue { cueMenu = nil; cueDelete = false }
        return true
    }

    /// Delete waits for softkey A on exactly one target (UI-42, UI-47).
    func waitForDelete(palette: Int? = nil, playback: Int? = nil, group: Int? = nil) {
        deleteTarget = palette; deletePlayback = playback; deleteGroup = group
    }

    /// UI-46: the one place a waiting Delete is confirmed (softkey A and Enter). false = nothing was waiting.
    @discardableResult
    func confirmDelete(_ engine: EngineClient) -> Bool {
        if cueDelete, let ref = cueMenu {
            engine.deleteCue(ref.playback, cue: ref.cue)
        } else if function == .delete, let number = deleteTarget {
            engine.deletePalette(number)
        } else if function == .delete, let id = deletePlayback {
            engine.deletePlayback(id)
        } else if function == .delete, let number = deleteGroup {
            engine.deleteGroup(number)
        } else {
            return false
        }
        cancelModes()
        return true
    }

    /// PRG-94: a function the Groups window answers to.
    var groupFunctionArmed: Bool {
        function == .delete || function == .copy || function == .move || function == .legend
    }

    /// UI-51: the selected time cells while the Times view is the one on show.
    func liveTimeCells(_ engine: EngineClient) -> TimeCells? {
        guard let cells = timeCells, screen == nil, cueView == nil, cells.playback == engine.connected, engine.connectedList?.isChase != true,
              Workspace.all[safe: workspace]?.windows.contains(.cues) == true else { return nil }
        return cells
    }
    /// UI-51: Set (a number) or As In (NSNull) on the selected cells: one `cue.times` (IPC-56).
    func setTimes(_ engine: EngineClient, _ value: Any) {
        guard let cells = liveTimeCells(engine) else { return }
        engine.setCueTimes(cells.playback, cues: cells.cues, [cells.column: value])
    }

    /// Disarms Record and every palette function.
    func cancelModes() {
        recordArmed = false
        function = nil; source = nil; deleteTarget = nil
        handleSource = nil; deletePlayback = nil
        groupSource = nil; deleteGroup = nil
        cueMenu = nil; cueDelete = false; cueRemoveConfirm = false
        paletteMenu = false
        selectionMenu = nil
        recordMask = nil; maskMenu = false
        updateOff = []; updateOn = []; updatePage = 0
    }

    /// PBK-49: Store on the Update softkeys sends the ticked targets.
    func updateStore(_ engine: EngineClient) {
        let targets = engine.updateTargets
        let playbacks = (targets?.playbacks ?? []).filter { !updateOff.contains("\($0.id)/\($0.cue)") }.map { ($0.id, $0.cue) }
        let palettes = (targets?.palettes ?? []).filter(updateOn.contains)
        if playbacks.isEmpty && palettes.isEmpty {
            engine.say("Update: no targets; tap a palette, playback handle or cue.")
            return
        }
        engine.update(playbacks: playbacks, palettes: palettes)
        cancelModes()
    }

    /// PBK-39: open the Playback Options screen on a playback.
    func openOptions(_ playback: Int, tab: PlaybackOptionsTab = .fader, cue: Double? = nil) {
        optionsFor = playback
        optionsCue = cue
        optionsTab = tab
        screen = .options
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

    /// Rail entries and the workspace keys (F1–F5, ⌘1–⌘5) all come here.
    func show(workspace index: Int) {
        workspace = index
        screen = nil
        selectionMenu = nil   // UI-33a: an open Order/Pattern would otherwise hide the Shapes softkeys
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
                    .allowsHitTesting(!engine.locked)   // UI-38: readable (`.disabled` would dim the dots), not clickable
                HLine()
                Group {
                    if let screen = ui.screen {
                        HStack(spacing: 0) {
                            Rail().frame(width: 78)
                            VLine()
                            switch screen {
                            case .patch: PatchView()
                            case .views: ViewsScreen()
                            case .output: OutputView()
                            case .control: ControlView()
                            case .show: ShowView()
                            case .remote: RemoteView()
                            case .options: PlaybackOptionsView()
                            }
                        }
                        .frame(minHeight: 0, maxHeight: .infinity).clipped()   // UI-63: the row takes what is left; it never pushes the zones below off screen
                    } else {
                        HStack(spacing: 0) {
                            Rail().frame(width: 78)
                            VLine()
                            Workspaces()
                            VLine()
                            SideColumn(showKeypad: keypad && geo.size.width >= 1440)
                        }
                        .frame(minHeight: 0, maxHeight: .infinity).clipped()   // UI-63
                        HLine()
                        AttributeBar().frame(height: 96)
                    }
                    HLine()
                    PlaybackArea().frame(height: 232)
                }
                .disabled(engine.locked)   // UI-38: workspaces and shortcuts stop under the cover
                .accessibilityHidden(engine.locked)
            }
            .overlay { if engine.locked { LockCover().padding(.top, 27) } }   // status bar + hairline stay visible
        }
        .background(t.bg)
        .foregroundStyle(t.fg)
        // FX-17, FX-36: the effect wheels are on exactly while the Shapes workspace shows.
        .onChange(of: ui.screen == nil && ui.workspace == Workspace.shapes) { _, on in engine.setEffectWheels(on) }
        .overlay { if ui.learnMode { Rectangle().strokeBorder(t.amber, lineWidth: 2).allowsHitTesting(false) } }   // CTL-42
        .sheet(item: $ui.legendEditing, onDismiss: { ui.cancelModes() }) { ref in
            LegendEditor(number: ref.number)
                .environmentObject(engine).environmentObject(engine.livePlaybacks).environmentObject(ui)
                .environment(\.theme, t)
                .preferredColorScheme(day ? .light : .dark)
        }
        .sheet(item: $ui.playbackLegendEditing, onDismiss: { ui.cancelModes() }) { ref in
            PlaybackLegendEditor(id: ref.id)
                .environmentObject(engine).environmentObject(engine.livePlaybacks).environmentObject(ui)
                .environment(\.theme, t)
                .preferredColorScheme(day ? .light : .dark)
        }
        .sheet(item: $ui.pageLegendEditing, onDismiss: { ui.cancelModes() }) { ref in
            PageLegendEditor(page: ref.number)
                .environmentObject(engine).environmentObject(engine.livePlaybacks).environmentObject(ui)
                .environment(\.theme, t)
                .preferredColorScheme(day ? .light : .dark)
        }
        .sheet(item: $ui.groupLegendEditing, onDismiss: { ui.cancelModes() }) { ref in
            GroupLegendEditor(group: ref.number)
                .environmentObject(engine).environmentObject(engine.livePlaybacks).environmentObject(ui)
                .environment(\.theme, t)
                .preferredColorScheme(day ? .light : .dark)
        }
        .sheet(item: $ui.cueLegendEditing, onDismiss: { ui.cancelModes() }) { ref in
            CueLegendEditor(ref: ref)
                .environmentObject(engine).environmentObject(engine.livePlaybacks).environmentObject(ui)
                .environment(\.theme, t)
                .preferredColorScheme(day ? .light : .dark)
        }
        .sheet(item: $ui.fixtureInfo) { ref in
            FixtureInfoSheet(id: ref.id)
                .environmentObject(engine).environmentObject(engine.livePlaybacks).environmentObject(ui)
                .environment(\.theme, t)
                .preferredColorScheme(day ? .light : .dark)
        }
        .sheet(isPresented: $ui.messagesOpen) {
            MessagesSheet()
                .environmentObject(engine).environmentObject(engine.livePlaybacks).environmentObject(ui)
                .environment(\.theme, t)
                .preferredColorScheme(day ? .light : .dark)
        }
        .sheet(isPresented: $ui.lockPrompt) {
            LockPrompt()
                .environmentObject(engine).environmentObject(engine.livePlaybacks).environmentObject(ui)
                .environment(\.theme, t)
                .preferredColorScheme(day ? .light : .dark)
        }
        // UI-38: locking takes the console's open modes with it; Learn ends in the Lock softkey (the engine refuses commands while locked).
        .onChange(of: engine.locked) { _, on in
            ui.lockPrompt = false; ui.lockCode = ""
            guard on else { return }
            line.clear(); ui.cancelModes(); ui.messagesOpen = false; engine.clearMessage()
            if ui.learnMode { ui.setLearnMode(false, engine) }
        }
        // UI-50: a message said because a function was armed goes when the arming ends (Esc, Exit, a screen, a job done).
        .onChange(of: ui.modesOpen) { _, open in if !open { engine.endArmedMessage() } }
        .onAppear {
            engine.isArmed = { [ui] in MainActor.assumeIsolated { ui.modesOpen } }
            // Mac keys (UI-19): the monitor stays installed; the handler checks the Keys toggle.
            keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [ui, line, engine] event in
                MainActor.assumeIsolated { MacKeys.handle(event, ui: ui, line: line, engine: engine) } ? nil : event
            }
        }
        .onDisappear { keyMonitor.map(NSEvent.removeMonitor) }
    }
}

/// All five workspaces stay alive, only one is visible: switching never rebuilds a window, so each
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

/// UI-56, UI-69: the GRP tag apart from the status bar, so a live delta redraws only it.
private struct HoldingTag: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var livePlaybacks: LivePlaybacks

    var body: some View {
        if !engine.holdingMasters.isEmpty {
            Button { engine.say(engine.holdingMasters.map(engine.holdingText).joined(separator: " · ")) } label: {
                Text("GRP").font(Typo.ui(11, .semibold)).foregroundStyle(t.amber)
                    .padding(.horizontal, 7).padding(.vertical, 2)
                    .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(t.amber, lineWidth: 1))
            }
            .buttonStyle(.plain)
            .help("A group master holds its group: click to see which")
        }
    }
}

private struct StatusBar: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @AppStorage("dayTheme") private var day = false
    @AppStorage("consoleMode") private var console = true
    @AppStorage("keypadVisible") private var keypad = true
    @AppStorage("keysLive") private var keys = false
    @EnvironmentObject private var ui: UIState
    @ObservedObject private var daemon = DaemonLauncher.shared

    fileprivate static let clock: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "HH:mm:ss"
        return f
    }()

    /// OUT-36: one dot per protocol, counting enabled nodes only, coloured by the worst node state.
    private static let protocols = [("artnet", "Art-Net"), ("sacn", "sACN"), ("serial", "Serial")]

    @ViewBuilder private func protocolDot(_ kind: String, _ name: String) -> some View {
        let nodes = engine.outputs.filter { $0.kind == kind && $0.enabled }
        if !nodes.isEmpty {
            let rank: (OutputNode.State) -> Int = { [.ok: 0, .disabled: 1, .degraded: 2, .stopped: 3][$0] ?? 0 }
            let worst = nodes.map(\.state).max { rank($0) < rank($1) } ?? .ok
            let color = [OutputNode.State.ok: t.green, .disabled: t.fg3, .degraded: t.amber, .stopped: t.red][worst] ?? t.green
            let issues = nodes.filter { $0.state != .ok }.map { n in
                "#\(n.id) universe \(n.line)" + (n.detail.map { ": \($0)" } ?? "")
            }
            // UI-40: a click opens the Output screen.
            Button { ui.screen = .output } label: {
                Dot(color: color, label: "\(name) \(nodes.count)", warn: worst == .degraded || worst == .stopped)
            }.buttonStyle(.plain).help(issues.joined(separator: "\n"))
        }
    }

    /// CTL-43: one dot for the enabled control devices and OSC, coloured by the worst state.
    @ViewBuilder private var controlDot: some View {
        let c = engine.control
        let devices = c.devices.filter(\.enabled)
        if !devices.isEmpty || c.osc.enabled {
            let oscError = c.osc.enabled ? c.osc.error : nil
            let issues = devices.filter { $0.state != .ok }.map { "\($0.name): " + ($0.state == .missing ? "not connected" : $0.state.word.lowercased()) }
                + (oscError.map { ["OSC: \($0)"] } ?? [])
            let missing = devices.contains { $0.state != .ok && $0.state != .disabled }
            Button { ui.screen = .control } label: {   // UI-40
                Dot(color: oscError != nil ? t.red : missing ? t.amber : t.green, label: "Control \(devices.count + (c.osc.enabled ? 1 : 0))",
                    warn: oscError != nil || missing)
            }.buttonStyle(.plain).help(issues.joined(separator: "\n"))
        }
    }

    var body: some View {
        HStack(spacing: 14) {
            ShowLabel()

            Dot(color: engine.connection == .online ? t.green : t.fg3, label: "Engine")
            if engine.connection != .online {
                Text(engine.protocolMismatch.map { "Engine speaks protocol \($0), this app speaks v\(EngineClient.protocolVersion): install the app and engine from the same release." }
                    ?? (engine.connection == .connecting ? "Connecting to engine…" : "Engine offline — faders are not driving output"))
                    .foregroundStyle(t.amber)
            }
            if let note = daemon.note { Text(note).foregroundStyle(t.amber) }

            ForEach(Self.protocols, id: \.0) { protocolDot($0.0, $0.1) }
            controlDot

            if ui.learnMode {
                BarToggle(title: "LEARN", on: true) { ui.setLearnMode(false, engine) }
                    .help("Learn mode: click a control on the main screen to bind a hardware control to it · Esc ends")
            }

            // UI-06, PBK-41: the engine's DBO.
            if engine.dbo {
                Text("DBO").font(Typo.ui(11, .semibold)).foregroundStyle(t.amber)
                    .padding(.horizontal, 7).padding(.vertical, 2)
                    .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(t.amber, lineWidth: 1))
            }

            // UI-56: a group master holds its group; a click says which.
            HoldingTag()

            Spacer(minLength: 0)
            BarToggle(title: "Lock", on: false, enabled: engine.connection == .online) {
                if !engine.locked { ui.openLockPrompt(engine) }   // UI-48: ⌘ chords pass the lock block
            }
            .keyboardShortcut("l", modifiers: .command)
            .help("Lock the console with a code · ⌘L")

            BarToggle(title: "Console", on: console) { console.toggle() }
                .help("Full screen, Dock and menu bar hidden, asks before quitting")
            BarToggle(title: "Keys", on: keys) { keys.toggle() }
                .help("Mac keyboard drives the keypad and the command line · ⌘1–⌘5 always switch workspaces")
            BarToggle(title: "Keypad", on: keypad) { keypad.toggle() }
                .help("Hidden automatically on windows narrower than 1440 pt")
            BarToggle(title: day ? "Night" : "Day", on: false) { day.toggle() }

            LayerClock(text: { Self.clock.string(from: $0) }, font: .monospacedSystemFont(ofSize: 11, weight: .regular), color: NSColor(t.fg))
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
                LayerClock(text: { show.ageText(now: $0) }, font: .systemFont(ofSize: 11), color: NSColor(show.autosaveOk ? t.fg2 : t.amber))
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
        ScrollView(.vertical, showsIndicators: false) {   // UI-63: scrolls when the items do not fit
            VStack(spacing: 1) {
                ForEach(Array(Workspace.all.enumerated()), id: \.offset) { index, workspace in
                    RailItem(title: workspace.name, small: "F\(index + 1)", on: ui.screen == nil && ui.workspace == index) {
                        ui.show(workspace: index)
                    }
                    // F1–F5 follow the Keys toggle (MacKeys); ⌘1–⌘5 always work (UI-19).
                    .keyboardShortcut(KeyEquivalent(Character("\(index + 1)")), modifiers: .command)
                }
                divider
                RailItem(title: "Patch", small: "screen", on: ui.screen == .patch) { ui.screen = ui.screen == .patch ? nil : .patch }
                RailItem(title: "Views", small: "screen", on: ui.screen == .views) { ui.screen = ui.screen == .views ? nil : .views }
                RailItem(title: "Output", small: "screen", on: ui.screen == .output) { ui.screen = ui.screen == .output ? nil : .output }
                RailItem(title: "Control", small: "screen", on: ui.screen == .control) { ui.screen = ui.screen == .control ? nil : .control }
                RailItem(title: "Macros", small: "later")
                RailItem(title: "Show", small: "screen", on: ui.screen == .show) { ui.screen = ui.screen == .show ? nil : .show }
                RailItem(title: "Remote", small: "screen", on: ui.screen == .remote) { ui.screen = ui.screen == .remote ? nil : .remote }
                divider
                RailItem(title: "Pixelmapper", small: "later")
                RailItem(title: "Keystone", small: "later")
                Spacer(minLength: 0)
            }
            .padding(4)
        }
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

/// UI-39: the history of warnings, errors and notices, newest first.
private struct MessagesSheet: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient

    private static let clock: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "HH:mm:ss"
        return f
    }()

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Messages").font(.headline)
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 4) {
                    if engine.messageLog.isEmpty { Text("No messages").foregroundStyle(t.fg3) }
                    ForEach(engine.messageLog.reversed()) { m in
                        HStack(alignment: .firstTextBaseline, spacing: 8) {
                            Text(Self.clock.string(from: m.at)).font(Typo.mono(11)).foregroundStyle(t.fg3)
                            Text(m.level).font(Typo.mono(11)).frame(width: 52, alignment: .leading)
                            Text(m.message).font(Typo.ui(12)).fixedSize(horizontal: false, vertical: true)
                        }
                        .foregroundStyle(m.level == "error" ? t.red : m.level == "warning" ? t.amber : t.fg)
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
            }
            HStack { Spacer(); Button("Done") { dismiss() }.keyboardShortcut(.defaultAction) }
        }
        // Escape closes like Done; zero-size rather than .hidden(), which disables the shortcut.
        .background(Button("") { dismiss() }.keyboardShortcut(.cancelAction).frame(width: 0, height: 0).opacity(0))
        .padding(16)
        .frame(width: 640, height: 420)
        // Arrivals while open leave no count.
        .onAppear { engine.messagesSeen() }
        .onDisappear { engine.messagesSeen() }
    }
}

// MARK: - Lock (UI-38)

private struct CodeKeypad: View {
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState

    var body: some View {
        let digit = { (d: String) in KeyButton(label: d, spec: "", style: .number, height: 52) { ui.lockCode += d; engine.clearMessage() } }
        Grid(horizontalSpacing: 6, verticalSpacing: 6) {
            GridRow { digit("7"); digit("8"); digit("9") }
            GridRow { digit("4"); digit("5"); digit("6") }
            GridRow { digit("1"); digit("2"); digit("3") }
            GridRow {
                KeyButton(label: "Back", spec: "", height: 52) { _ = ui.lockCode.popLast() }
                digit("0")
                KeyButton(label: "Enter", spec: "", style: .enter, height: 52) { ui.enterLockCode(engine) }
            }
        }
        .frame(width: 3 * 64 + 12)
    }
}

/// The code as typed: dots on the cover, digits in the prompt.
private struct CodeBox: View {
    @Environment(\.theme) private var t
    let text: String

    var body: some View {
        Text(text).font(Typo.mono(Typo.large)).foregroundStyle(t.fg)
            .frame(width: 204, height: 36)
            .background(RoundedRectangle(cornerRadius: radius).fill(t.surf2))
            .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(t.line2, lineWidth: 1))
    }
}

private struct LockCover: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState

    var body: some View {
        VStack(spacing: 14) {
            Text("Locked").font(Typo.legend(32, .semibold))
            Text(engine.show.name.isEmpty ? "—" : engine.show.name).font(Typo.legend(Typo.large)).foregroundStyle(t.fg2)
            LayerClock(text: { StatusBar.clock.string(from: $0) }, font: .monospacedSystemFont(ofSize: Typo.large, weight: .regular), color: NSColor(t.fg2))
            CodeBox(text: String(repeating: "•", count: ui.lockCode.count))
            Text(engine.lastMessage ?? "").font(Typo.ui()).foregroundStyle(t.amber).frame(height: 16)
            CodeKeypad()
        }
        .foregroundStyle(t.fg)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(t.bg)
    }
}

private struct LockPrompt: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState

    var body: some View {
        VStack(spacing: 12) {
            Text("Lock the console").font(.headline)
            Text("Enter a code, or nothing for no code. Unlocking needs the same code.").foregroundStyle(t.fg2)
            CodeBox(text: ui.lockCode)
            CodeKeypad()
            Button("Cancel") { ui.lockPrompt = false; ui.lockCode = "" }.keyboardShortcut(.cancelAction)
        }
        .padding(16)
        .frame(width: 360)
    }
}

/// UI-71: a once-a-second label that sets its own string, so a tick costs no SwiftUI update.
private struct LayerClock: NSViewRepresentable {
    let text: (Date) -> String
    let font: NSFont
    let color: NSColor

    final class Coordinator {
        var timer: Timer?
        var text: (Date) -> String = { _ in "" }
        weak var label: NSTextField?
        func tick() {
            let s = text(Date())
            if let l = label, l.stringValue != s { l.stringValue = s }
        }
        deinit { timer?.invalidate() }
    }

    func makeCoordinator() -> Coordinator { Coordinator() }
    func makeNSView(context: Context) -> NSTextField {
        let l = NSTextField(labelWithString: "")
        l.setContentHuggingPriority(.required, for: .horizontal)
        l.setContentCompressionResistancePriority(.required, for: .horizontal)
        let c = context.coordinator
        c.label = l
        let timer = Timer(timeInterval: 1, repeats: true) { [weak c] _ in c?.tick() }
        RunLoop.main.add(timer, forMode: .common)
        c.timer = timer
        return l
    }
    func updateNSView(_ l: NSTextField, context: Context) {
        l.font = font; l.textColor = color
        context.coordinator.text = text
        context.coordinator.tick()
    }
    static func dismantleNSView(_ l: NSTextField, coordinator: Coordinator) { coordinator.timer?.invalidate() }
    func sizeThatFits(_ proposal: ProposedViewSize, nsView: NSTextField, context: Context) -> CGSize? { nsView.intrinsicContentSize }
}
