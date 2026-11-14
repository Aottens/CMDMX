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
    /// The Keypad toggle (UI-17).
    let showKeypad: Bool
    /// Window narrower than 1440 pt: the keypad then takes the place of the softkeys (UI-78).
    let narrow: Bool
    @Environment(\.theme) private var t

    var body: some View {
        HStack(spacing: 0) {
            FunctionButtons()
            VLine()
            if showKeypad {
                Keypad()
                if !narrow { VLine() }
            }
            if !(narrow && showKeypad) { Softkeys() }
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
        "Update": "Update: tick targets on the softkeys and Store, or tap the palette, playback handle or cue to update with what is in the programmer.",
        "Include": "Include: tap a palette, playback handle or cue; its values go into the programmer.",
        "Delete": "Delete: tap the group, palette, playback handle or cue to delete (⇧ range, ⌘ add one). You confirm on softkey A.",
        "Copy": "Copy: tap the source group, palette or playback handle (⇧ range, ⌘ add one), then an empty cell or handle; or tap a cue.",
        "Move": "Move: tap the source group, palette, playback handle or fixture (⇧ range, ⌘ add one), then an empty cell, handle or fixture button; or tap a cue.",
        "Release": "Release: tap a handle or an active row to release it, Release again for all, or a page button for a page.",
        "Options": "Options: tap a playback handle or cue to open its options.",
        "Unfold": "Unfold: tap a fixture to unfold or fold its cells.",
        "Latch": "Latch: now press Flash on a handle to latch it.",
    ]

    // Titan's order, two columns. An empty spec = live; otherwise the spec the key waits for.
    private static let keys: [(String, String)] = [
        ("Record", ""), ("Update", ""),
        ("Edit", "Palettes · phase 2"), ("Select If", "later"),
        ("Delete", ""), ("Unfold", ""),
        ("Copy", ""), ("Move", ""),
        ("Include", ""), ("Release", ""),
        ("Patch", ""), ("Disk", ""),
        ("Help", ""), ("System", "later"),
        ("Blind", "later"), ("Off", ""),
        ("Fan", ""), ("Options", ""),
        ("Latch", ""), ("Shape", "later"),
    ]

    var body: some View {
        VStack(spacing: 0) {
            ColumnHeader(text: "Functions")
            // UI-63: plain rows, so the keys can shrink from 40 to 34 pt when the column is short.
            VStack(spacing: 2) {
                ForEach(Array(stride(from: 0, to: Self.keys.count, by: 2)), id: \.self) { start in
                    HStack(spacing: 2) {
                        ForEach(Self.keys.dropFirst(start).prefix(2), id: \.0) { label, spec in
                            FunctionKey(label: label, spec: spec,
                                        ring: label == "Record" ? ui.recordArmed : ui.function?.rawValue == label,
                                        lit: label == "Fan" && engine.programmer.fan != nil,
                                        tip: label == "Off" ? "Off: takes the active attribute group out of the programmer for the targets (PRG-27)" : "",
                                        action: spec.isEmpty ? { press(label) } : nil)
                                .frame(width: 68)
                        }
                    }
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
                ui.cancelModes(); engine.endArmedMessage()   // UI-50
                ui.recordArmed = true
            }
        case "Patch": ui.screen = .patch
        case "Fan": ui.learnOrRun("fan", engine) { engine.fanToggle() }   // CTL-59
        case "Off": engine.programmerOff()   // UI-26: the active attribute group, as Titan's Off
        case "Disk": ui.screen = .show
        case "Help": engine.openManual()   // UI-55: no target, what is armed stays armed
        default:
            if let function = PaletteFunction(rawValue: label) {   // PAL-29
                if function == .release, ui.function == .release {   // UI-66: Release twice releases all
                    engine.releaseScope("all"); ui.cancelModes(); return
                }
                engine.endArmedMessage()   // UI-50: the old function's hint goes, the new one is said below
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
    /// Switched on (Fan): amber.
    var lit = false
    /// Tooltip of a live key.
    var tip = ""
    var action: (() -> Void)?

    var body: some View {
        Button { action?() } label: {
            Text(label).font(Typo.legend(13)).foregroundStyle(lit ? t.bg : t.fg)
                .padding(.horizontal, 7)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
                .background(RoundedRectangle(cornerRadius: radius).fill(lit ? t.amber : t.surf))
                .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(ring ? t.red : t.line, lineWidth: ring ? 2 : 1))
                .overlay(alignment: .bottom) { Rectangle().fill(t.line2).frame(height: 2).padding(.horizontal, 1) }
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .frame(minHeight: 34, maxHeight: 40)
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
        case "Enter": line.enter(engine, ui: ui)   // UI-46
        case "Locate":   // UI-26: an empty line locates everything, else the line ends in Locate
            if line.tokens.isEmpty { engine.locate() } else { line.enter(engine, ui: ui, end: "Locate") }
            ui.paletteMenu = false
        case "Highlight": engine.setHighlight(!engine.programmer.highlight)
        case "Clear": line.clearProgrammer(engine); if ui.building == nil { ui.cancelModes() }; ui.pendingOverlap = nil
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
        if let cells = ui.liveTimeCells(engine) { return timesContext(cells) }
        if ui.paletteMenu { return paletteContext }
        if let mark = ui.storedMark { return storedContext(mark) }   // PBK-54: until the selection changes, Esc or a function
        // UI-33a: Order and Pattern open from the Fan context too and win over it.
        if !engine.programmer.selection.isEmpty {
            if ui.selectionMenu == "Order" { return orderContext }
            if ui.selectionMenu == "Pattern" { return patternContext }
        }
        if let current = engine.programmer.fan { return fanContext(current) }
        if ui.screen == nil, ui.workspace == Workspace.shapes { return shapesContext }   // FX-39
        if !engine.programmer.selection.isEmpty {
            return ("Selection", [
                nextKey,
                Softkey(name: "Previous", sub: "Fix −1", action: { ui.learnOrRun("key step_prev", engine) { engine.step(-1) } }),
                allKey,
                orderKey,
                patternKey,
                Softkey(name: "Rem Dim", sub: "the rest to 0", action: { ui.learnOrRun("key rem_dim", engine) { engine.remDim() } }),   // UI-26
                optionsKey,
                undoKey,
            ])
        }
        return ("Main", [
            Softkey(name: "Edit Times", sub: "last tapped handle", action: { openLastHandle(.times) }),
            optionsKey,
            setLegendKey,
            Softkey(name: "Shapes & Effects", sub: "looks and effects", action: { ui.show(workspace: Workspace.shapes) }),   // FX-36
            Softkey(name: "Timecode", sub: "phase 3", spec: "Timecode · phase 3"),
            Softkey(name: "Lock", sub: "with a code", spec: "Engine offline",   // UI-38
                    action: engine.connection == .online ? { ui.openLockPrompt(engine) } : nil),
            undoKey,
        ])
    }

    /// UI-33: Order sub-context.
    private var orderContext: (name: String, keys: [Softkey?]) {
        var keys: [Softkey?] = ["Reverse", "Shuffle", "Sort", "Revert"].map { op in
            Softkey(name: op, action: { ui.learnOrRun("order \(op.lowercased())", engine) { engine.order(op) } })
        }
        keys.append(Softkey(name: "Back", action: { ui.selectionMenu = nil }))
        return ("Order", keys)
    }

    /// UI-33: Pattern sub-context. A number on the line is n; `n Thro m` gives n in m.
    private var patternContext: (name: String, keys: [Softkey?]) {
        let current = engine.programmer.grid?.shortcut
        let tk = line.tokens
        let thro: (Int, Int)? = tk.count == 3 && tk[1] == "Thro" ? Int(tk[0]).flatMap { n in Int(tk[2]).map { (n, $0) } } : nil
        let number: Int? = line.onlyNumber.map { Int($0) }
        let key = { (name: String, shortcut: String, learn: String, send: @escaping (Int) -> Void, def: Int) -> Softkey in
            Softkey(name: name, on: current == shortcut) {
                let n = number ?? def
                ui.learnOrRun("grid \(learn) \(n)", engine) { send(n); if number != nil { line.clear() } }
            }
        }
        let nIn = Softkey(name: "n in m", on: current == "NIn") {
            // ponytail: without `n Thro m` a lone number is m and n is 1.
            let (n, m) = thro ?? (1, number ?? 3)
            ui.learnOrRun("grid n_in \(n) \(m)", engine) { engine.grid(shortcut: "NIn", n: n, m: m); if thro != nil || number != nil { line.clear() } }
        }
        return ("Pattern", [
            Softkey(name: "Odd/Even", on: current == "OddEven", action: { ui.learnOrRun("grid odd_even", engine) { engine.grid(shortcut: "OddEven") } }),
            key("1 in n", "OneIn", "one_in", { engine.grid(shortcut: "OneIn", n: $0) }, 3),
            nIn,
            key("Blocks n", "Blocks", "blocks", { engine.grid(shortcut: "Blocks", n: $0) }, 2),
            key("Divide n", "Divide", "divide", { engine.grid(shortcut: "Divide", n: $0) }, 2),
            Softkey(name: "Outside in", on: current == "OutsideIn", action: { ui.learnOrRun("grid outside_in", engine) { engine.grid(shortcut: "OutsideIn") } }),
            Softkey(name: "Inside out", on: current == "InsideOut", action: { ui.learnOrRun("grid inside_out", engine) { engine.grid(shortcut: "InsideOut") } }),
            Softkey(name: "Grid off", action: { ui.learnOrRun("grid off", engine) { engine.grid() } }),
            nextKey,   // UI-33b
            Softkey(name: "Back", action: { ui.selectionMenu = nil }),
        ])
    }

    /// UI-30: while Fan is on, the curves (the current one inverted) and Fan off.
    /// UI-33a: with a selection also Next, All, Order and Pattern on F–I; their Back returns here.
    private func fanContext(_ current: String) -> (name: String, keys: [Softkey?]) {
        var keys: [Softkey?] = ["Line", "Mirror", "Wings", "Arrow"].map { curve in
            Softkey(name: curve, on: curve == current,
                    action: { ui.learnOrRun("fan \(curve.lowercased())", engine) { engine.fan(curve: curve) } })
        }
        keys.append(Softkey(name: "Fan off", action: { ui.learnOrRun("fan off", engine) { engine.fanOff() } }))
        if !engine.programmer.selection.isEmpty { keys += [nextKey, allKey, orderKey, patternKey] }
        return ("Fan", keys)
    }

    /// FX-39: the Shapes workspace. Record arms as the Record key does (an effect palette is a palette
    /// with entries, FX-22); the next palette tap in a palette window records it.
    private var shapesContext: (name: String, keys: [Softkey?]) {
        ("Shapes", [
            Softkey(name: "Stop FX", sub: "the selection", action: { ui.learnOrRun("effect stop", engine) { engine.stopEffects() } }),
            Softkey(name: "Restart", sub: "from the top", action: { ui.learnOrRun("effect restart", engine) { engine.restartEffects() } }),
            Softkey(name: "Tap", sub: "tempo", action: { ui.learnOrRun("effect tap", engine) { engine.tapEffect() } }),
            Softkey(name: "Record", sub: "then tap a palette", action: { ui.cancelModes(); engine.endArmedMessage(); ui.recordArmed = true }),
            undoKey,
        ])
    }

    private var nextKey: Softkey {
        Softkey(name: "Next", sub: "Fix +1", action: { ui.learnOrRun("key step_next", engine) { engine.step(1) } })
    }

    private var allKey: Softkey {
        Softkey(name: "All", sub: "whole selection", action: { ui.learnOrRun("key all", engine) { engine.selectAll() } })
    }

    private var orderKey: Softkey {
        Softkey(name: "Order", sub: "reverse, shuffle", action: { ui.selectionMenu = "Order" })
    }

    private var patternKey: Softkey {
        Softkey(name: "Pattern", sub: "grid, odd/even", action: { ui.selectionMenu = "Pattern" })
    }

    /// UNDO-18: the IPC-31 label as the second line; no action while there is nothing to undo.
    private var undoKey: Softkey {
        Softkey(name: "Undo", sub: engine.undoLabel.map { engine.undoEarlier ? "(earlier) \($0)" : $0 } ?? "nothing", spec: "Undo · ⌘Z",
                action: engine.undoLabel == nil && !ui.learnMode ? nil : { ui.learnOrRun("key undo", engine) { engine.undo() } })
    }

    private var setLegendKey: Softkey {
        Softkey(name: "Set Legend", sub: "then tap a cell or handle",
                action: { ui.arm(.legend); engine.say("Set Legend: tap a group, a palette, a playback handle, a cue or a page.") })
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
        let onPlayback = handle != nil || cue != nil || ui.building != nil
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
        let cancelKey = Softkey(name: "Cancel", action: { engine.endArmedMessage(); ui.recordArmed = false })
        // CUE-52: on a tracking list or chase an Update Mode softkey cycles the mode of this record.
        let updateModeKey = { (list: PlaybackInfo?) -> Softkey? in
            guard let list, list.hasCues, list.stored.list.tracking else { return nil }
            let current = ui.updateMode ?? engine.playbackSettings?.updateMode ?? "Forwards"
            return Softkey(name: "Update Mode", sub: Self.updateModes[current] ?? current) {
                let order = Self.updateOrder
                ui.updateMode = order[((order.firstIndex(of: current) ?? -1) + 1) % order.count]
            }
        }
        if let b = ui.building {
            // CUE-62: Append (once a handle is chosen), Mask, Record By and Exit; Exit is the only way out but Esc.
            let append = b.slot.map { slot in
                Softkey(name: "Append", sub: "next cue") {
                    let page = b.page ?? engine.page.number
                    let info = engine.playbacks.first { $0.stored.page == page && $0.stored.slot == slot }
                    HandleTaps.build(b, page: page, slot: slot, info: info, used: info != nil, engine: engine, ui: ui, line: line)
                }
            }
            let exit = Softkey(name: "Exit", sub: "ends building", action: { engine.endArmedMessage(); ui.recordArmed = false })
            let title = b.slot.map { "Building \(b.page ?? 0).\($0)" } ?? (b.kind == "Chase" ? "Create Chase" : "Create List")
            return (title, [append, maskKey, recordByKey, exit])
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
        if let group = ui.usedGroup {
            // PRG-97: a used group takes Replace or Merge.
            let mode = { (name: String, sub: String, wire: String) -> Softkey in
                Softkey(name: name, sub: sub) {
                    engine.recordGroup(group, mode: wire)
                    ui.recordArmed = false
                }
            }
            return ("Record · Group \(group)", [mode("Replace", "new contents", "Replace"), mode("Merge", "add the selection", "Merge"), cancelKey])
        }
        let mode = { (name: String, sub: String, wire: String) -> Softkey? in
            guard let used else { return nil }
            return Softkey(name: name, sub: sub) {
                engine.recordPalette(window: used.window, handle: used.handle, mask: ui.orderedRecordMask,
                                     recordBy: ui.recordBy, recordAs: ui.recordAs, mode: wire)
                ui.recordArmed = false
            }
        }
        // UI-18, FXB-10, GRM-15: with nothing tapped yet, A and B record a speed master on the lowest empty
        // handle of this page at once (and end Record); C arms a group master (a group, then a handle).
        let master = { (name: String, kind: String) -> Softkey in
            let group = kind == "Group"
            return Softkey(name: name, sub: group ? "group, then handle" : "next empty handle", on: group && ui.recordMaster == kind) {
                if group {
                    ui.recordMaster = ui.recordMaster == kind ? nil : kind
                    ui.recordMasterGroup = nil
                    if ui.recordMaster != nil { engine.say("\(name): tap a group.") }
                    return
                }
                let page = engine.page.number
                if let slot = HandleTaps.lowestEmptySlot(page: page, engine: engine) {
                    engine.recordMaster(page: page, slot: slot, kind: kind)
                    ui.recordArmed = false
                    ui.recordMaster = nil
                } else if let other = engine.pages.map(\.number).sorted().first(where: { $0 != page && HandleTaps.lowestEmptySlot(page: $0, engine: engine) != nil }) {
                    engine.say("Page \(page) is full: page \(other) has room.")
                } else {
                    engine.say("Every page is full.")
                }
            }
        }
        // CUE-62: Create List / Create Chase keep Record armed; the next handle tap stores the first cue.
        let building = { (name: String, kind: String) -> Softkey in
            Softkey(name: name, sub: "tap a handle") {
                ui.recordMaster = nil; ui.recordMasterGroup = nil
                ui.building = Building(kind: kind)
                engine.say("\(name): tap a handle.")
            }
        }
        return (used.map { "Record · \($0.number)" } ?? "Record", [
            used == nil ? master("BPM Master", "Bpm") : mode("Replace", "new contents", "Replace"),
            used == nil ? master("Rate Master", "Rate") : mode("Merge", "add and overwrite", "Merge"),
            used == nil ? master("Group Master", "Group") : mode("Quick Merge", "held attributes", "QuickMerge"),
            used == nil ? building("Create List", "Cues") : nil,
            used == nil ? building("Create Chase", "Chase") : nil,
            maskKey,
            recordByKey,
            Softkey(name: "Record as", sub: ui.recordAs) {
                ui.recordAs = ["Auto": "Normal", "Normal": "Shared"][ui.recordAs] ?? "Auto"
            },
            cancelKey,
        ])
    }

    /// PBK-54: right after a store, Legend opens the editor of what was stored: a new playback's legend, or the new
    /// cue (the number now on the handle that was not there before). Cleared by the next function, Record or Esc.
    private func storedContext(_ mark: (page: Int, slot: Int, before: Set<Double>)) -> (name: String, keys: [Softkey?]) {
        let legend = Softkey(name: "Legend", sub: "of what you stored") {
            guard let info = engine.playbacks.first(where: { $0.stored.page == mark.page && $0.stored.slot == mark.slot }) else {
                return engine.say("Nothing stored on \(mark.page).\(mark.slot).")
            }
            let new = info.stored.cues.map(\.number).filter { !mark.before.contains($0) }.min()
            if mark.before.isEmpty { ui.playbackLegendEditing = PlaybackRef(id: info.id) }
            else if let new { ui.cueLegendEditing = CueRef(playback: info.id, cue: new) }
        }
        return ("Stored · \(mark.page).\(mark.slot)", [legend])
    }

    /// This record's Record By: the operator's choice, else the engine's setting (PAL-25).
    /// For a playback record (a used handle tapped) the default is the cue setting (PBK-10).
    private var recordBy: String {
        ui.recordBy ?? (ui.usedHandle != nil || ui.usedCue != nil || ui.building != nil ? engine.playbackSettings?.recordBy ?? "Channel" : engine.paletteSettings?.recordBy ?? "Mixed")
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
            keys[0] = Softkey(name: "Confirm delete", sub: ui.cuePicks.count > 1 ? "\(ui.cuePicks.count) cues" : name) {
                ui.confirmDelete(engine)   // UI-46
            }
            if ui.function == .delete { keys[3] = menuLatchKey }   // UI-65: a right-click's Delete has no function to latch
            keys[6] = Softkey(name: "Cancel", action: { engine.endArmedMessage(); ui.cancelModes() })
            return ("Cue · \(name)", keys)
        }
        if ui.cueBlockPicking, let f = ui.function {   // CUE-69: the picked cues go to a number, the end, or another list
            var keys: [Softkey?] = Array(repeating: nil, count: 7)
            let title = f == .copy ? "Copy" : "Renumber"
            keys[0] = Softkey(name: title, sub: "number on the line") {
                guard let n = line.onlyNumber else { return engine.say("\(title): type the first cue number on the keypad first.") }
                ui.sendCueBlock(engine, to: n)
                line.clear()
            }
            keys[1] = Softkey(name: "End", sub: "after the last cue") { ui.sendCueBlock(engine, to: nil) }
            keys[3] = menuLatchKey
            keys[6] = Softkey(name: "Cancel", action: { engine.endArmedMessage(); ui.cancelModes() })
            return ("Cue · \(name)", keys)
        }
        func numbered(_ title: String, _ send: @escaping (Double) -> Void) -> Softkey {
            Softkey(name: title, sub: "number on the line") {
                guard let n = line.onlyNumber else { return engine.say("\(title): type the cue number on the keypad first.") }
                send(n)
                line.clear()
                ui.endAction(engine)
            }
        }
        let keys: [Softkey?] = [
            Softkey(name: "Go to Cue", sub: "runs it now") { engine.cuelist("goto", id: ref.playback, cue: ref.cue); ui.cancelModes() },
            Softkey(name: "View Cue", sub: "its whole state") { ui.cueView = ref; ui.cueValue = nil; ui.cancelModes() },
            Softkey(name: "Edit Times", sub: "Times tab") { ui.openOptions(ref.playback, tab: .times, cue: ref.cue) },
            Softkey(name: "Set Legend") { ui.cueLegendEditing = ref },
            numbered("Renumber") { engine.renumberCue(ref.playback, cue: ref.cue, to: $0) },
            numbered("Copy") { engine.copyCue(ref.playback, cue: ref.cue, to: $0) },
            Softkey(name: "Delete", sub: "confirm on A") {
                ui.cueDelete = true
                if let list = engine.playback(ref.playback), let cue = list.cue(ref.cue) {
                    engine.sayDelete(cue: cue, of: list)
                } else {
                    engine.say("Delete cue \(name)? Confirm on softkey A.")
                }
            },
            Softkey(name: "Cancel", action: { engine.endArmedMessage(); ui.cancelModes() }),
        ]
        // UI-53: a memory has one cue, so only View Cue, Edit Times, Set Legend and Cancel stay (positions kept).
        if engine.playback(ref.playback)?.isMemory == true {
            return ("Cue · \(name)", keys.enumerated().map { [1, 2, 3, 7].contains($0.offset) ? $0.element : nil })
        }
        return ("Cue · \(name)", keys)
    }

    /// UI-51: Set, As In (out columns only) and Done for the selected time cells.
    private func timesContext(_ cells: TimeCells) -> (name: String, keys: [Softkey?]) {
        let first = cells.cues.first.map { cueName($0) } ?? ""
        let last = cells.cues.last.map { cueName($0) } ?? ""
        var keys: [Softkey?] = Array(repeating: nil, count: 8)
        keys[0] = Softkey(name: "Set", sub: "number on the line") {
            guard let n = line.onlyNumber else { return engine.say("Set: type the time on the keypad first.") }
            ui.setTimes(engine, n)
            line.clear()
        }
        if cells.column == "delay_out" || cells.column == "fade_out" {
            keys[1] = Softkey(name: "As In") { ui.setTimes(engine, NSNull()) }
        }
        keys[7] = Softkey(name: "Done") { ui.timeCells = nil }
        return ((first == last ? first : "\(first)–\(last)") + " · " + cells.title, keys)
    }

    /// The Cue view (CUE-54): Back to List, and Remove for the hard value last right-clicked (two-step).
    private func cueViewContext(_ ref: CueRef) -> (name: String, keys: [Softkey?]) {
        let name = cueName(ref.cue)
        let target = ui.cueValue
        let what: String? = target.map {
            switch $0 {
            case let .value(id, cell, attribute):
                let number = "\(engine.fixtures.first { $0.id == id }?.number ?? id)\(cell.map { ".\($0)" } ?? "")"
                return attribute.map { "\($0) of \(number)" } ?? (cell == nil ? "fixture \(number)" : number)
            case let .effect(_, name): return name
            case let .dormant(id, cell, attribute):
                let number = "\(engine.fixtures.first { $0.id == id }?.number ?? id)\(cell.map { ".\($0)" } ?? "")"
                return "\(attribute) of \(number)"
            }
        }
        var keys: [Softkey?] = Array(repeating: nil, count: 7)
        if ui.cueRemoveConfirm, let target {
            keys[0] = Softkey(name: "Confirm remove", sub: what ?? "") {
                switch target {
                case let .value(fixture, cell, attribute):
                    engine.removeCueValue(ref.playback, cue: ref.cue, fixture: fixture, cell: cell, attribute: attribute)
                case let .effect(entry, _):
                    engine.removeCueEffect(ref.playback, cue: ref.cue, entry: entry)
                case .dormant: break   // never armed
                }
                ui.cueValue = nil; ui.cueRemoveConfirm = false
            }
            keys[6] = Softkey(name: "Cancel", action: { engine.endArmedMessage(); ui.cueRemoveConfirm = false })
            return ("Cue view · \(name)", keys)
        }
        keys[0] = Softkey(name: "Back to List") { ui.cueView = nil; ui.cueValue = nil; ui.cueRemoveConfirm = false }
        keys[1] = Softkey(name: "Remove", sub: what ?? "right-click a value, a fixture number or an effect") {
            guard let target else { return engine.say("Right-click a hard value, a fixture number or an effect in the Cue view first.") }
            if case .dormant = target { return engine.say("That value is off: On brings it back, then Remove.") }
            ui.cueRemoveConfirm = true
            engine.say("Remove \(what ?? "") from \(name)? Confirm on softkey A.")
        }
        // CUE-61: Off acts on a hard value or a whole item, On on a value that is off (or a whole item); one step, one Undo.
        var offTarget: (fixture: Int, cell: Int?, attribute: String?)?
        var onTarget: (fixture: Int, cell: Int?, attribute: String?)?
        switch target {
        case let .value(f, c, a)?: offTarget = (f, c, a); onTarget = a == nil ? (f, c, nil) : nil
        case let .dormant(f, c, a)?: onTarget = (f, c, a)
        default: break
        }
        keys[2] = Softkey(name: "Off", sub: offTarget != nil ? (what ?? "") : "right-click a hard value or a fixture number") {
            guard let t = offTarget else { return engine.say("Off: right-click a hard value or a fixture number first.") }
            engine.setCueOff(ref.playback, cue: ref.cue, fixture: t.fixture, cell: t.cell, attribute: t.attribute, off: true)
            ui.cueValue = nil
        }
        keys[3] = Softkey(name: "On", sub: onTarget != nil ? (what ?? "") : "right-click a value that is off") {
            guard let t = onTarget else { return engine.say("On: right-click a value that is off, or a fixture number, first.") }
            engine.setCueOff(ref.playback, cue: ref.cue, fixture: t.fixture, cell: t.cell, attribute: t.attribute, off: false)
            ui.cueValue = nil
        }
        return ("Cue view · \(name)", keys)
    }

    // MARK: Update, Include, Copy, Move, Delete, Set Legend (PAL-29)

    private var menuLatchKey: Softkey {
        Softkey(name: "Menu latch", sub: ui.menuLatch ? "on" : "off", on: ui.menuLatch) { ui.menuLatch.toggle() }   // UI-65
    }

    private func functionContext(_ function: PaletteFunction) -> (name: String, keys: [Softkey?]) {
        // PBK-48: Include's Mask covers all eight groups, FX too, and starts all on; a palette tap ignores it (PAL-12).
        if function == .include, ui.maskMenu {
            let mask = ui.recordMask ?? Set(attributeGroupOrder)
            let groups = attributeGroupOrder.map { group in
                Softkey(name: group, sub: mask.contains(group) ? "on" : "off", on: mask.contains(group)) {
                    var m = mask
                    if m.contains(group) { m.remove(group) } else { m.insert(group) }
                    ui.recordMask = m
                }
            }
            return ("Include · Mask", groups + [Softkey(name: "Back", action: { ui.maskMenu = false })])
        }
        var keys: [Softkey?] = Array(repeating: nil, count: 7)
        // UI-64, UI-65: Copy, Move and Delete pick several; the layout, All, Swap and Menu latch keys.
        if [.copy, .move, .delete].contains(function) {
            let n = ui.picks.count, first = ui.picks.first ?? 0
            if function == .delete, let kind = ui.pickKind, n > 0 {
                let confirm = { ui.confirmDelete(engine) }   // UI-46
                if n == 1 {
                    switch kind {
                    case .palette: keys[0] = Softkey(name: "Confirm delete", sub: "palette \(first)", action: { confirm() })
                    case .handle: keys[0] = Softkey(name: "Confirm delete", sub: "playback \(engine.playback(first)?.homeHandle ?? "")", action: { confirm() })
                    case .group: keys[0] = Softkey(name: "Delete group \(first)", sub: engine.groups.first { $0.id == first }?.legend ?? "", action: { confirm() })
                    }
                } else {
                    let noun = kind == .handle ? "playbacks" : kind == .palette ? "palettes" : "groups"
                    let page = kind == .handle ? ui.pickedPage(engine) : nil
                    keys[0] = Softkey(name: "Delete \(n) \(noun)", sub: ui.wholePagePhrase(engine) ?? page.map { "on page \($0)" } ?? "confirm", action: { confirm() })
                }
            } else if function != .delete {
                keys[0] = Softkey(name: ui.bunchUp ? "Bunch up" : "Retain layout", sub: "layout") { ui.bunchUp.toggle() }
            }
            if n > 0 {
                keys[1] = Softkey(name: "All", sub: ui.pickKind == .handle ? "this page" : ui.pickKind == .palette ? "this window" : "groups") { ui.pickAll(engine) }
            }
            if function == .move {
                keys[2] = Softkey(name: "Swap if required", sub: ui.swapIfRequired ? "on" : "off", on: ui.swapIfRequired) { ui.swapIfRequired.toggle() }
            }
            keys[3] = menuLatchKey
        }
        if function == .release, let page = ui.releasePage {
            keys[0] = Softkey(name: "Release page \(page)") { engine.releaseScope("page", page: page); ui.cancelModes() }
            keys[1] = Softkey(name: "Release all but page \(page)") { engine.releaseScope("other_pages", page: page); ui.cancelModes() }
        }
        if function == .unfold {
            // UI-29: no selection is touched; unfolding is view state.
            keys[0] = Softkey(name: "Selected fixtures") {
                let selected = Set(engine.programmer.selection.map(\.fixture))
                ui.unfolded.formUnion(engine.fixtures.filter { $0.cells > 0 && selected.contains($0.id) }.map(\.id))
                ui.cancelModes()
            }
            keys[1] = Softkey(name: "Exit Unfold") { ui.unfolded = []; ui.cancelModes() }
        }
        if function == .include {
            keys[0] = Softkey(name: "Mask", sub: ui.recordMask.map { attributeGroupOrder.filter($0.contains).joined() } ?? "all") {
                ui.maskMenu = true
            }
        }
        if function == .update {
            // ponytail: the targets are asked once on arming; a programmer or stage change while armed needs a new arm.
            let targets = engine.updateTargets
            var all: [Softkey] = (targets?.playbacks ?? []).map { t in
                let key = "\(t.id)/\(t.cue)"
                let info = engine.playback(t.id)
                let name = info.map { $0.stored.legend.isEmpty ? $0.homeHandle : $0.stored.legend } ?? "Playback \(t.id)"
                let on = !ui.updateOff.contains(key)
                return Softkey(name: name, sub: cueName(t.cue), on: on) {
                    if on { ui.updateOff.insert(key) } else { ui.updateOff.remove(key) }
                }
            }
            all += (targets?.palettes ?? []).map { n in
                let on = ui.updateOn.contains(n)
                let name = engine.palettes.first { $0.number == n }?.shownLegend ?? "Palette \(n)"
                return Softkey(name: name, sub: "palette \(n)", on: on) {
                    if on { ui.updateOn.remove(n) } else { ui.updateOn.insert(n) }
                }
            }
            let pages = all.count > 5 ? (all.count + 3) / 4 : 1
            let page = min(ui.updatePage, pages - 1)
            let shown = pages > 1 ? Array(all.dropFirst(page * 4).prefix(4)) : all
            for (i, key) in shown.enumerated() { keys[i] = key }
            if pages > 1 {
                keys[4] = Softkey(name: "More", sub: "\(page + 1)/\(pages)") { ui.updatePage = (page + 1) % pages }
            }
            keys[5] = Softkey(name: "Store") { ui.updateStore(engine) }
        }
        keys[6] = Softkey(name: "Cancel", action: { engine.endArmedMessage(); ui.cancelModes() })
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
            Softkey(name: "Cancel", action: { engine.endArmedMessage(); line.clear(); ui.paletteMenu = false }),
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
                    // UI-63: 10 keys at 34 pt fit the 443 pt row of the 800 pt window.
                    .frame(maxHeight: .infinity).frame(minHeight: 34)
                    .disabled(key?.action == nil)
                    .opacity(key?.action == nil ? t.disabled : 1)
                    .help(key?.spec ?? "")
                }
            }
        }
        .padding(4)
        .frame(width: 120)
        .background(t.panel)
        .onChange(of: engine.programmer.selection.isEmpty) { _, empty in if empty { ui.selectionMenu = nil } }
        .onChange(of: engine.programmer.selection) { _, _ in ui.storedMark = nil }   // PBK-54: a new look ends Legend
        .onChange(of: ui.function) { _, f in if f == .update { engine.askUpdateTargets() } }
    }
}
