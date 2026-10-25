import SwiftUI

// Side column (UI-16..18): function buttons, keypad, softkeys. A control without an `action` is
// disabled and its tooltip names the spec it waits for.

/// One key of the console. Disabled while `action` is nil.
struct KeyButton: View {
    enum Style { case normal, number, command, enter }

    @Environment(\.theme) private var t
    let label: String
    /// Tooltip: the spec or task this key waits for.
    let spec: String
    var style = Style.normal
    var height: CGFloat = 40
    /// Red ring: there is something to clear (Clear key).
    var ring = false
    /// Amber fill: a switch that is on (DBO, PBK-41).
    var lit = false
    var action: (() -> Void)?
    /// CTL-58: dragging the key onto a deck key links this action.
    var drag: String?

    var body: some View {
        let background: Color = lit ? t.amber : style == .enter ? t.fg : style == .command ? t.bg : t.surf2
        let foreground: Color = lit ? t.bg : ring ? t.red : style == .enter ? t.bg : style == .command ? t.fg2 : t.fg
        Button { action?() } label: {
            Text(label)
                .font(style == .number ? Typo.mono(Typo.large) : Typo.legend(style == .command ? 11 : Typo.body))
                .foregroundStyle(foreground)
                .lineLimit(1).minimumScaleFactor(0.7)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(RoundedRectangle(cornerRadius: radius).fill(background))
                .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(ring ? t.red : style == .normal ? t.line : t.line2, lineWidth: ring ? 2 : 1))
        }
        .buttonStyle(.plain)
        .frame(height: height)
        .disabled(action == nil)
        .opacity(action == nil ? t.disabled : 1)
        .deckDrag(drag)   // outside `.disabled`: a key without a list still links (CTL-58)
        .help(spec)
    }
}

/// Zone header: uppercase condensed, hairline below.
private struct ColumnHeader: View {
    @Environment(\.theme) private var t
    let text: String
    var body: some View {
        VStack(spacing: 0) {
            Text(text.uppercased()).font(Typo.legend(11, .semibold)).tracking(1.1).foregroundStyle(t.fg2)
                .lineLimit(1).minimumScaleFactor(0.7).frame(maxWidth: .infinity, alignment: .leading).frame(height: 22)
            HLine()
        }
        .padding(.bottom, 2)
    }
}

struct SideColumn: View {
    /// False when the Keypad toggle is off or the window is narrower than 1440 pt (UI-17).
    let showKeypad: Bool
    @Environment(\.theme) private var t

    var body: some View {
        HStack(spacing: 0) {
            FunctionButtons()
            VLine()
            if showKeypad {
                Keypad()
                VLine()
            }
            Softkeys()
        }
        .fixedSize(horizontal: true, vertical: false)
        .background(t.bg)
    }
}

// MARK: - Function buttons (UI-16)

private struct FunctionButtons: View {
    @EnvironmentObject private var ui: UIState
    @EnvironmentObject private var engine: EngineClient

    /// What an armed palette function waits for (the sketch's MODE_HINT).
    private static let hints = [
        "Update": "Update: tap the palette or playback handle to update with what is in the programmer.",
        "Include": "Include: tap a palette; its values go into the programmer.",
        "Delete": "Delete: tap the palette or playback handle to delete. You confirm on softkey A.",
        "Copy": "Copy: tap the source palette or playback handle, then an empty cell or handle.",
        "Move": "Move: tap the source palette or playback handle, then an empty cell or handle.",
        "Release": "Release: tap a running playback handle to release it.",
        "Options": "Options: tap a playback handle to open its options.",
        "Latch": "Latch: now press Flash on a handle to latch it.",
    ]

    // Titan's order, two columns. An empty spec = live; otherwise the spec the key waits for.
    private static let keys: [(String, String)] = [
        ("Record", ""), ("Update", ""),
        ("Edit", "Palettes · phase 2"), ("Select If", "later"),
        ("Delete", ""), ("Unfold", "later"),
        ("Copy", ""), ("Move", ""),
        ("Include", ""), ("Release", ""),
        ("Patch", ""), ("Disk", ""),
        ("Help", "later"), ("System", "later"),
        ("Blind", "later"), ("Off", ""),
        ("Fan", "later"), ("Options", ""),
        ("Latch", ""), ("Shape", "later"),
    ]

    var body: some View {
        VStack(spacing: 0) {
            ColumnHeader(text: "Functions")
            LazyVGrid(columns: Array(repeating: GridItem(.fixed(68), spacing: 2), count: 2), spacing: 2) {
                ForEach(Self.keys, id: \.0) { label, spec in
                    FunctionKey(label: label, spec: spec,
                                ring: label == "Record" ? ui.recordArmed : ui.function?.rawValue == label,
                                tip: label == "Off" ? "Off: takes the active attribute group out of the programmer for the targets (PRG-27)" : "",
                                action: spec.isEmpty ? { press(label) } : nil)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(4)
    }

    private func press(_ label: String) {
        switch label {
        case "Record":
            // The next group or palette tap records. In the Mask softkeys, Record steps back first.
            if ui.maskMenu { ui.maskMenu = false } else if ui.recordArmed { ui.recordArmed = false } else {
                ui.cancelModes()
                ui.recordArmed = true
            }
        case "Patch": ui.screen = .patch
        case "Off": engine.programmerOff()   // UI-26: the active attribute group, as Titan's Off
        case "Disk": ui.screen = .show
        default:
            if let function = PaletteFunction(rawValue: label) {   // PAL-29
                ui.arm(function)
                if ui.function != nil, let hint = Self.hints[label] { engine.say(hint) }
            }
        }
    }
}

private struct FunctionKey: View {
    @Environment(\.theme) private var t
    let label: String
    let spec: String
    /// Record armed: red ring.
    var ring = false
    /// Tooltip of a live key.
    var tip = ""
    var action: (() -> Void)?

    var body: some View {
        Button { action?() } label: {
            Text(label).font(Typo.legend(13)).foregroundStyle(t.fg)
                .padding(.horizontal, 7)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
                .background(RoundedRectangle(cornerRadius: radius).fill(t.surf))
                .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(ring ? t.red : t.line, lineWidth: ring ? 2 : 1))
                .overlay(alignment: .bottom) { Rectangle().fill(t.line2).frame(height: 2).padding(.horizontal, 1) }
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .frame(height: 40)
        .disabled(action == nil)
        .opacity(action == nil ? t.disabled : 1)
        .help(spec.isEmpty ? tip : spec)
    }
}

// MARK: - Keypad (UI-17)

private struct Keypad: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState
    @EnvironmentObject private var line: TokenLine

    // The sketch's 30 keys in order; the second field is the spec a disabled key waits for
    // (empty = live).
    private static let keys: [(String, String, KeyButton.Style)] = [
        ("Fixture", "", .normal), ("Palette", "", .normal),
        ("Macro", "later", .normal), ("Group", "", .normal),
        ("Back", "", .normal), ("Thro", "", .normal),
        ("And", "", .normal), ("@", "", .normal),
        ("7", "", .number), ("8", "", .number),
        ("9", "", .number), ("Avo", "later", .command),
        ("4", "", .number), ("5", "", .number),
        ("6", "", .number), ("Set", "", .normal),
        ("1", "", .number), ("2", "", .number),
        ("3", "", .number), ("Clear", "", .normal),
        ("Exit", "", .command), ("0", "", .number),
        (".", "", .number), ("Enter", "", .enter),
        ("Fix -1", "", .command), ("Fix +1", "", .command),
        ("Odd/Even", "", .command), ("Highlight", "", .normal),
        ("View", "later", .command), ("Locate", "", .normal),
    ]

    var body: some View {
        VStack(spacing: 0) {
            ColumnHeader(text: "Keypad")
            LazyVGrid(columns: Array(repeating: GridItem(.fixed(50), spacing: 2), count: 4), spacing: 2) {
                ForEach(Self.keys, id: \.0) { label, spec, style in
                    KeyButton(label: label, spec: spec, style: style, height: 38,
                              ring: label == "Clear" && dirty, lit: label == "Highlight" && engine.programmer.highlight, action: spec.isEmpty ? { press(label) } : nil)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(4)
        .background(t.panel)
    }

    /// Something is selected or in the programmer: Clear glows (as in the attribute bar).
    private var dirty: Bool { !engine.programmer.selection.isEmpty || !engine.programmer.inProgrammer.isEmpty }

    /// CTL-42: the action a key arms in Learn mode (CTL-05); nil = the key has none and just runs.
    private static let learnActions = [
        "Clear": "key clear", "Locate": "key locate", "Highlight": "key highlight", "Fix -1": "key step_prev", "Fix +1": "key step_next",
        "Back": "keypad Backspace", "Enter": "keypad Enter", "Thro": "keypad Thro", "And": "keypad And", "@": "keypad @",
        "Fixture": "keypad Fixture", "Group": "keypad Group", "Palette": "keypad Palette", ".": "keypad .",
    ]

    private func press(_ label: String) {
        ui.learnOrRun(Self.learnActions[label] ?? (label.count == 1 && label.first?.isNumber == true ? "keypad \(label)" : nil), engine) { run(label) }
    }

    private func run(_ label: String) {
        switch label {
        case "Back": line.back()
        case "Exit": line.clear(); ui.cancelModes()   // also disarms Record and the function buttons (PBK-28)
        case "Enter": line.send(engine); ui.paletteMenu = false
        case "Locate":   // UI-26: an empty line locates everything, else the line ends in Locate
            if line.tokens.isEmpty { engine.locate() } else { line.send(engine, end: "Locate") }
            ui.paletteMenu = false
        case "Highlight": engine.setHighlight(!engine.programmer.highlight)
        case "Clear": engine.clearProgrammer(); ui.cancelModes(); ui.pendingOverlap = nil
        case "Set": ui.arm(.legend)   // Set Legend, as the softkey
        case "Fix -1": engine.step(-1)
        case "Fix +1": engine.step(1)
        case "Odd/Even": engine.pattern(2)
        case "Palette":
            if line.tokens.isEmpty { ui.paletteMenu = true }   // PAL-29: the Palette softkeys, until Cancel or Exit
            line.add(label)
        default: line.add(label)   // digits, ".", Fixture, Group, Thro, And, @
        }
    }
}

// MARK: - Softkeys (UI-18)

private struct SoftkeyShape: Shape {
    /// The sketch's arrow: flat right side, point on the left.
    func path(in r: CGRect) -> Path {
        var p = Path()
        p.move(to: CGPoint(x: 10, y: 0))
        p.addLine(to: CGPoint(x: r.maxX, y: 0))
        p.addLine(to: CGPoint(x: r.maxX, y: r.maxY))
        p.addLine(to: CGPoint(x: 10, y: r.maxY))
        p.addLine(to: CGPoint(x: 0, y: r.midY))
        p.closeSubpath()
        return p
    }
}

private struct Softkey {
    let name: String
    var sub = ""
    /// The spec a disabled key waits for.
    var spec = ""
    /// Switched on (a group in the Record mask): inverted, as on Titan.
    var on = false
    var action: (() -> Void)?
}

private struct Softkeys: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState

    @EnvironmentObject private var line: TokenLine

    /// The context follows the state: Record armed, an armed palette function, the Palette key,
    /// else a selection, else Main (UI-18).
    private var context: (name: String, keys: [Softkey?]) {
        if ui.recordArmed { return recordContext }
        // CUE-54: the Cue view's softkeys while its window is on show; CUE-43: the Cue softkeys of a right-click.
        if let ref = ui.cueView, ui.screen == nil, Workspace.all[safe: ui.workspace]?.windows.contains(.cues) == true {
            return cueViewContext(ref)
        }
        if let ref = ui.cueMenu { return cueContext(ref) }
        if let function = ui.function { return functionContext(function) }
        if ui.paletteMenu { return paletteContext }
        if !engine.programmer.selection.isEmpty {
            return ("Selection", [
                Softkey(name: "Next", sub: "Fix +1", action: { ui.learnOrRun("key step_next", engine) { engine.step(1) } }),
                Softkey(name: "Previous", sub: "Fix −1", action: { ui.learnOrRun("key step_prev", engine) { engine.step(-1) } }),
                Softkey(name: "All", sub: "whole selection", action: { ui.learnOrRun("key all", engine) { engine.selectAll() } }),
                Softkey(name: "Odd/Even", action: { engine.pattern(2) }),
                Softkey(name: "Rem Dim", sub: "the rest to 0", action: { ui.learnOrRun("key rem_dim", engine) { engine.remDim() } }),   // UI-26
                optionsKey,
                undoKey,
            ])
        }
        return ("Main", [
            Softkey(name: "Edit Times", sub: "last tapped handle", action: { openLastHandle(.times) }),
            optionsKey,
            setLegendKey,
            Softkey(name: "Shapes & Effects", sub: "phase 2", spec: "Shapes · phase 2"),
            Softkey(name: "Timecode", sub: "phase 3", spec: "Timecode · phase 3"),
            Softkey(name: "Lock", sub: "venue mode", spec: "later"),
            undoKey,
        ])
    }

    /// UNDO-18: the IPC-31 label as the second line; no action while there is nothing to undo.
    private var undoKey: Softkey {
        Softkey(name: "Undo", sub: engine.undoLabel ?? "nothing", spec: "Undo · ⌘Z",
                action: engine.undoLabel == nil && !ui.learnMode ? nil : { ui.learnOrRun("key undo", engine) { engine.undo() } })
    }

    private var setLegendKey: Softkey {
        Softkey(name: "Set Legend", sub: "then tap a palette or handle",
                action: { ui.arm(.legend); engine.say("Set Legend: tap a palette, a playback handle or a page.") })
    }

    /// PBK-39: the Playback Options screen for the handle tapped last.
    private var optionsKey: Softkey {
        Softkey(name: "Playback Options", sub: "last tapped handle", action: { openLastHandle(.fader) })
    }

    private func openLastHandle(_ tab: PlaybackOptionsTab) {
        if let id = ui.lastHandle, engine.playback(id) != nil {
            ui.openOptions(id, tab: tab)
        } else {
            engine.say("Tap a playback handle first.")
        }
    }

    // MARK: Record (PAL-06, PAL-29)

    /// Record armed. After a tap on a used cell, A–C pick the mode; D–F are for this record only.
    private var recordContext: (name: String, keys: [Softkey?]) {
        let used = ui.usedCell, handle = ui.usedHandle, cue = ui.usedCue
        let onPlayback = handle != nil || cue != nil
        if ui.maskMenu {
            let base = used.flatMap { PaletteWindowKind.of(wire: $0.window) }?.mask
                ?? (onPlayback ? attributeGroupOrder : workspaceMaskBase)
            let mask = ui.recordMask ?? Set(base)
            return ("Record · Mask", Array(attributeGroupOrder.prefix(7)).map { group in
                Softkey(name: group, sub: mask.contains(group) ? "on" : "off", on: mask.contains(group)) {
                    var m = mask
                    if m.contains(group) { m.remove(group) } else { m.insert(group) }
                    ui.recordMask = m
                }
            })
        }
        let maskKey = Softkey(name: "Mask", sub: (ui.recordMask.map { attributeGroupOrder.filter($0.contains).joined() }) ?? (onPlayback ? "all" : "window")) {
            ui.maskMenu = true
        }
        let recordByKey = Softkey(name: "Record By", sub: recordBy) { ui.recordBy = recordBy == "Channel" ? "Mixed" : "Channel" }
        let cancelKey = Softkey(name: "Cancel", action: { engine.clearMessage(); ui.recordArmed = false })
        // CUE-52: on a tracking list or chase an Update Mode softkey cycles the mode of this record.
        let updateModeKey = { (list: PlaybackInfo?) -> Softkey? in
            guard let list, list.hasCues, list.stored.list.tracking else { return nil }
            let current = ui.updateMode ?? engine.playbackSettings?.updateMode ?? "Forwards"
            return Softkey(name: "Update Mode", sub: Self.updateModes[current] ?? current) {
                let order = Self.updateOrder
                ui.updateMode = order[((order.firstIndex(of: current) ?? -1) + 1) % order.count]
            }
        }
        if let handle {
            // PBK-38, CUE-45: a used handle takes Replace, Merge, Append or Insert; Mask, Record By
            // and Update Mode are for this record.
            let mode = { (name: String, sub: String, wire: String) -> Softkey in
                Softkey(name: name, sub: sub) {
                    engine.recordPlayback(page: handle.page, slot: handle.slot, mask: ui.orderedRecordMask,
                                          recordBy: ui.recordBy, mode: wire, updateMode: ui.updateMode)
                    ui.recordArmed = false
                }
            }
            return ("Record · \(handle.page).\(handle.slot)", [
                mode("Replace", "new contents", "Replace"), mode("Merge", "add and overwrite", "Merge"),
                mode("Append", "new cue at the end", "Append"), mode("Insert", "new cue after current", "Insert"),
                maskKey, recordByKey, updateModeKey(engine.playback(handle.playback)), cancelKey,
            ].compactMap { $0 })
        }
        if let cue, let list = engine.playback(cue.playback) {
            // CUE-45: a tapped row takes Replace, Merge or Update for that cue.
            let mode = { (name: String, sub: String, wire: String) -> Softkey in
                Softkey(name: name, sub: sub) {
                    engine.recordPlayback(page: list.stored.page, slot: list.stored.slot, mask: ui.orderedRecordMask,
                                          recordBy: ui.recordBy, mode: wire, cue: cue.cue, updateMode: ui.updateMode)
                    ui.recordArmed = false
                }
            }
            return ("Record · \(cueName(cue.cue))", [
                mode("Replace", "new contents", "Replace"), mode("Merge", "add and overwrite", "Merge"),
                mode("Update", "held attributes", "Update"),
                maskKey, recordByKey, updateModeKey(list), cancelKey,
            ].compactMap { $0 })
        }
        let mode = { (name: String, sub: String, wire: String) -> Softkey? in
            guard let used else { return nil }
            return Softkey(name: name, sub: sub) {
                engine.recordPalette(window: used.window, handle: used.handle, mask: ui.orderedRecordMask,
                                     recordBy: ui.recordBy, recordAs: ui.recordAs, mode: wire)
                ui.recordArmed = false
            }
        }
        // CUE-45, GRM-15: with nothing tapped yet, A and B arm a speed master for the next empty handle,
        // C a group master (a group, then a handle).
        let master = { (name: String, kind: String) -> Softkey in
            let group = kind == "Group"
            return Softkey(name: name, sub: group ? "group, then handle" : "next empty handle", on: ui.recordMaster == kind) {
                ui.recordMaster = ui.recordMaster == kind ? nil : kind
                ui.recordMasterGroup = nil
                if ui.recordMaster != nil { engine.say("\(name): tap \(group ? "a group" : "an empty handle").") }
            }
        }
        return (used.map { "Record · \($0.number)" } ?? "Record", [
            used == nil ? master("BPM Master", "Bpm") : mode("Replace", "new contents", "Replace"),
            used == nil ? master("Rate Master", "Rate") : mode("Merge", "add and overwrite", "Merge"),
            used == nil ? master("Group Master", "Group") : mode("Quick Merge", "held attributes", "QuickMerge"),
            maskKey,
            recordByKey,
            Softkey(name: "Record as", sub: ui.recordAs) {
                ui.recordAs = ["Auto": "Normal", "Normal": "Shared"][ui.recordAs] ?? "Auto"
            },
            cancelKey,
        ])
    }

    /// This record's Record By: the operator's choice, else the engine's setting (PAL-25).
    /// For a playback record (a used handle tapped) the default is the cue setting (PBK-10).
    private var recordBy: String {
        ui.recordBy ?? (ui.usedHandle != nil || ui.usedCue != nil ? engine.playbackSettings?.recordBy ?? "Channel" : engine.paletteSettings?.recordBy ?? "Mixed")
    }

    /// Where the Mask toggles start before a cell is tapped: the first palette window of the workspace,
    /// else every group (the Playbacks workspace, where the next tap is a handle).
    private var workspaceMaskBase: [String] {
        Workspace.all[safe: ui.workspace]?.windows.lazy.compactMap(\.paletteKind).first?.mask ?? attributeGroupOrder
    }

    // MARK: Cue lists (CUE-43, 45, 54)

    private static let updateOrder = ["Forwards", "CueOnly", "Backwards", "Both"]
    private static let updateModes = ["Forwards": "Forwards", "CueOnly": "Cue Only", "Backwards": "Backwards", "Both": "Both"]

    /// A right-click on a Cue list row (UI-18). Renumber and Copy take the number on the command
    /// line; Delete asks for its confirmation on softkey A.
    private func cueContext(_ ref: CueRef) -> (name: String, keys: [Softkey?]) {
        let name = cueName(ref.cue)
        if ui.cueDelete {
            var keys: [Softkey?] = Array(repeating: nil, count: 7)
            keys[0] = Softkey(name: "Confirm delete", sub: name) {
                engine.deleteCue(ref.playback, cue: ref.cue)
                ui.cancelModes()
            }
            keys[6] = Softkey(name: "Cancel", action: { engine.clearMessage(); ui.cancelModes() })
            return ("Cue · \(name)", keys)
        }
        func numbered(_ title: String, _ send: @escaping (Double) -> Void) -> Softkey {
            Softkey(name: title, sub: "number on the line") {
                guard let n = line.onlyNumber else { return engine.say("\(title): type the cue number on the keypad first.") }
                send(n)
                line.clear()
                ui.cancelModes()
            }
        }
        return ("Cue · \(name)", [
            Softkey(name: "Go to Cue", sub: "runs it now") { engine.cuelist("goto", id: ref.playback, cue: ref.cue); ui.cancelModes() },
            Softkey(name: "View Cue", sub: "its whole state") { ui.cueView = ref; ui.cueValue = nil; ui.cancelModes() },
            Softkey(name: "Edit Times", sub: "Times tab") { ui.openOptions(ref.playback, tab: .times, cue: ref.cue) },
            Softkey(name: "Set Legend") { ui.cueLegendEditing = ref },
            numbered("Renumber") { engine.renumberCue(ref.playback, cue: ref.cue, to: $0) },
            numbered("Copy") { engine.copyCue(ref.playback, cue: ref.cue, to: $0) },
            Softkey(name: "Delete", sub: "confirm on A") {
                ui.cueDelete = true
                engine.say("Delete cue \(name)? Confirm on softkey A.")
            },
            Softkey(name: "Cancel", action: { engine.clearMessage(); ui.cancelModes() }),
        ])
    }

    /// The Cue view (CUE-54): Back to List, and Remove for the hard value last right-clicked (two-step).
    private func cueViewContext(_ ref: CueRef) -> (name: String, keys: [Softkey?]) {
        let name = cueName(ref.cue)
        let target = ui.cueValue
        let fixture = target.flatMap { t in engine.fixtures.first { $0.id == t.fixture }?.number }
        let what = target.map { "\($0.attribute) of \(fixture ?? $0.fixture)" }
        var keys: [Softkey?] = Array(repeating: nil, count: 7)
        if ui.cueRemoveConfirm, let target {
            keys[0] = Softkey(name: "Confirm remove", sub: what ?? "") {
                engine.removeCueValue(ref.playback, cue: ref.cue, fixture: target.fixture, attribute: target.attribute)
                engine.viewCue(ref.playback, cue: ref.cue)
                ui.cueValue = nil; ui.cueRemoveConfirm = false
            }
            keys[6] = Softkey(name: "Cancel", action: { engine.clearMessage(); ui.cueRemoveConfirm = false })
            return ("Cue view · \(name)", keys)
        }
        keys[0] = Softkey(name: "Back to List") { ui.cueView = nil; ui.cueValue = nil; ui.cueRemoveConfirm = false }
        keys[1] = Softkey(name: "Remove", sub: what ?? "right-click a value") {
            guard target != nil else { return engine.say("Right-click a hard value in the Cue view first.") }
            ui.cueRemoveConfirm = true
            engine.say("Remove \(what ?? "") from \(name)? Confirm on softkey A.")
        }
        return ("Cue view · \(name)", keys)
    }

    // MARK: Update, Include, Copy, Move, Delete, Set Legend (PAL-29)

    private func functionContext(_ function: PaletteFunction) -> (name: String, keys: [Softkey?]) {
        var keys: [Softkey?] = Array(repeating: nil, count: 7)
        if function == .delete, let number = ui.deleteTarget {
            keys[0] = Softkey(name: "Confirm delete", sub: "palette \(number)") {
                engine.deletePalette(number)
                ui.cancelModes()
            }
        }
        if function == .delete, let id = ui.deletePlayback {
            keys[0] = Softkey(name: "Confirm delete", sub: "playback \(engine.playback(id)?.homeHandle ?? "")") {
                engine.deletePlayback(id)
                ui.cancelModes()
            }
        }
        keys[6] = Softkey(name: "Cancel", action: { engine.clearMessage(); ui.cancelModes() })
        return (function.rawValue, keys)
    }

    // MARK: Palette key (PAL-15, PAL-29)

    /// Master Time and Master Overlap take the number on the line; without one they show the value.
    private var paletteContext: (name: String, keys: [Softkey?]) {
        func master(_ name: String, unit: String, current: Double?, send: @escaping (Double) -> Void) -> Softkey {
            Softkey(name: name, sub: current.map { "\(plain($0)) \(unit)" } ?? "?") {
                if let n = line.paletteArgument {
                    send(n)
                    line.clear()
                } else {
                    engine.say("\(name): \(current.map { "\(plain($0)) \(unit)" } ?? "unknown"). Type a number first to change it.")
                }
            }
        }
        return ("Palette", [
            master("Master Time", unit: "s", current: engine.paletteSettings?.time) { engine.setPaletteMaster(time: $0) },
            master("Master Overlap", unit: "%", current: engine.paletteSettings?.overlap) { engine.setPaletteMaster(overlap: $0) },
            Softkey(name: "Set Overlap", sub: ui.pendingOverlap.map { "next tap \(plain($0)) %" } ?? "next tap only") {
                if let n = line.paletteArgument {
                    ui.pendingOverlap = n
                    line.clear()
                    engine.say("Overlap \(plain(n)) % for the next palette tap.")
                } else {
                    engine.say("Set Overlap: type the overlap in % first.")
                }
            },
            nil, nil, nil,
            Softkey(name: "Cancel", action: { engine.clearMessage(); line.clear(); ui.paletteMenu = false }),
        ])
    }

    var body: some View {
        let context = context
        VStack(spacing: 0) {
            ColumnHeader(text: context.name)
            VStack(spacing: 2) {
                ForEach(Array(context.keys.enumerated()), id: \.offset) { index, key in
                    Button { key?.action?() } label: {
                        ZStack(alignment: .topLeading) {
                            SoftkeyShape().fill(key?.on == true ? t.fg : t.surf2)
                            Text(String(UnicodeScalar(UInt8(65 + index)))).font(Typo.mono(11, .semibold)).foregroundStyle(key?.on == true ? t.bg : t.fg3)
                                .padding(.leading, 14).padding(.top, 6)
                            if let key {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(key.name).font(Typo.legend()).foregroundStyle(key.on ? t.bg : t.fg)
                                    if !key.sub.isEmpty { Text(key.sub).font(Typo.mono(Typo.small)).foregroundStyle(key.on ? t.bg : t.fg2) }
                                }
                                .padding(.leading, 28).padding(.top, 6)
                            }
                        }
                        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                        .contentShape(SoftkeyShape())
                    }
                    .buttonStyle(.plain)
                    .frame(maxHeight: .infinity).frame(minHeight: 44)
                    .disabled(key?.action == nil)
                    .opacity(key?.action == nil ? t.disabled : 1)
                    .help(key?.spec ?? "")
                }
            }
        }
        .padding(4)
        .frame(width: 120)
        .background(t.panel)
    }
}
