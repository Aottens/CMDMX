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
    var action: (() -> Void)?

    var body: some View {
        let background: Color = style == .enter ? t.fg : style == .command ? t.bg : t.surf2
        let foreground: Color = ring ? t.red : style == .enter ? t.bg : style == .command ? t.fg2 : t.fg
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
                .lineLimit(1).frame(maxWidth: .infinity, alignment: .leading).frame(height: 22)
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
        "Update": "Update: tap the palette to update with what is in the programmer.",
        "Include": "Include: tap a palette; its values go into the programmer.",
        "Delete": "Delete: tap the palette to delete. You confirm on softkey A.",
        "Copy": "Copy: tap the source palette, then an empty cell.",
        "Move": "Move: tap the source palette, then an empty cell.",
    ]

    // Titan's order, two columns. An empty spec = live; otherwise the spec the key waits for.
    private static let keys: [(String, String)] = [
        ("Record", ""), ("Update", ""),
        ("Edit", "Palettes · phase 2"), ("Select If", "later"),
        ("Delete", ""), ("Unfold", "later"),
        ("Copy", ""), ("Move", ""),
        ("Include", ""), ("Release", "Playbacks · P1-04"),
        ("Patch", ""), ("Disk", ""),
        ("Help", "later"), ("System", "later"),
        ("Blind", "later"), ("Off", "Highlight and Off · VS2-05"),
        ("Fan", "later"), ("Options", "Playbacks · P1-04"),
        ("Latch", "Playbacks · P1-04"), ("Shape", "later"),
    ]

    var body: some View {
        VStack(spacing: 0) {
            ColumnHeader(text: "Functions")
            LazyVGrid(columns: Array(repeating: GridItem(.fixed(68), spacing: 2), count: 2), spacing: 2) {
                ForEach(Self.keys, id: \.0) { label, spec in
                    FunctionKey(label: label, spec: spec,
                                ring: label == "Record" ? ui.recordArmed : ui.function?.rawValue == label,
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
        .help(spec)
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
        ("Odd/Even", "", .command), ("Highlight", "Highlight and Locate · VS2-05", .normal),
        ("View", "later", .command), ("Locate", "Highlight and Locate · VS2-05", .normal),
    ]

    var body: some View {
        VStack(spacing: 0) {
            ColumnHeader(text: "Keypad")
            LazyVGrid(columns: Array(repeating: GridItem(.fixed(50), spacing: 2), count: 4), spacing: 2) {
                ForEach(Self.keys, id: \.0) { label, spec, style in
                    KeyButton(label: label, spec: spec, style: style, height: 38,
                              ring: label == "Clear" && dirty, action: spec.isEmpty ? { press(label) } : nil)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(4)
        .background(t.panel)
    }

    /// Something is selected or in the programmer: Clear glows (as in the attribute bar).
    private var dirty: Bool { !engine.programmer.selection.isEmpty || !engine.programmer.inProgrammer.isEmpty }

    private func press(_ label: String) {
        switch label {
        case "Back": line.back()
        case "Exit": line.clear()
        case "Enter": line.send(engine)
        case "Clear": engine.clearProgrammer(); ui.cancelModes(); ui.pendingOverlap = nil
        case "Set": ui.arm(.legend)   // Set Legend, as the softkey
        case "Fix -1": engine.step(-1)
        case "Fix +1": engine.step(1)
        case "Odd/Even": engine.pattern(2)
        default: line.add(label)   // digits, ".", Fixture, Group, Palette, Thro, And, @; the Palette softkeys follow the line
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
        if let function = ui.function { return functionContext(function) }
        if line.paletteMenu { return paletteContext }
        if !engine.programmer.selection.isEmpty {
            return ("Selection", [
                Softkey(name: "Next", sub: "Fix +1", action: { engine.step(1) }),
                Softkey(name: "Previous", sub: "Fix −1", action: { engine.step(-1) }),
                Softkey(name: "All", sub: "whole selection", action: { engine.selectAll() }),
                Softkey(name: "Odd/Even", action: { engine.pattern(2) }),
                setLegendKey,
                Softkey(name: "Playback Options", spec: "Playbacks · P1-04"),
                Softkey(name: "Undo", sub: "nothing", spec: "Undo · P1-10"),
            ])
        }
        return ("Main", [
            Softkey(name: "Edit Times", sub: "selected playback", spec: "Cue lists · P1-08"),
            Softkey(name: "Playback Options", spec: "Playbacks · P1-04"),
            setLegendKey,
            Softkey(name: "Shapes & Effects", sub: "phase 2", spec: "Shapes · phase 2"),
            Softkey(name: "Timecode", sub: "phase 3", spec: "Timecode · phase 3"),
            Softkey(name: "Lock", sub: "venue mode", spec: "later"),
            Softkey(name: "Undo", sub: "nothing", spec: "Undo · P1-10"),
        ])
    }

    private var setLegendKey: Softkey {
        Softkey(name: "Set Legend", sub: "then tap a palette", action: { ui.arm(.legend); engine.say("Set Legend: tap a palette.") })
    }

    // MARK: Record (PAL-06, PAL-29)

    /// Record armed. After a tap on a used cell, A–C pick the mode; D–F are for this record only.
    private var recordContext: (name: String, keys: [Softkey?]) {
        let used = ui.usedCell
        if ui.maskMenu {
            let base = used.flatMap { PaletteWindowKind.of(wire: $0.window) }?.mask ?? workspaceMaskBase
            let mask = ui.recordMask ?? Set(base)
            return ("Record · Mask", Array(attributeGroupOrder.prefix(7)).map { group in
                Softkey(name: group, sub: mask.contains(group) ? "on" : "off") {
                    var m = mask
                    if m.contains(group) { m.remove(group) } else { m.insert(group) }
                    ui.recordMask = m
                }
            })
        }
        let mode = { (name: String, sub: String, wire: String) -> Softkey? in
            guard let used else { return nil }
            return Softkey(name: name, sub: sub) {
                engine.recordPalette(window: used.window, handle: used.handle, mask: ui.orderedRecordMask,
                                     recordBy: ui.recordBy, recordAs: ui.recordAs, mode: wire)
                ui.recordArmed = false
            }
        }
        return (used.map { "Record · \($0.number)" } ?? "Record", [
            mode("Replace", "new contents", "Replace"),
            mode("Merge", "add and overwrite", "Merge"),
            mode("Quick Merge", "held attributes", "QuickMerge"),
            Softkey(name: "Mask", sub: (ui.recordMask.map { attributeGroupOrder.filter($0.contains).joined() }) ?? "window") {
                ui.maskMenu = true
            },
            Softkey(name: "Record By", sub: recordBy) { ui.recordBy = recordBy == "Channel" ? "Mixed" : "Channel" },
            Softkey(name: "Record as", sub: ui.recordAs) {
                ui.recordAs = ["Auto": "Normal", "Normal": "Shared"][ui.recordAs] ?? "Auto"
            },
            Softkey(name: "Cancel", action: { ui.recordArmed = false }),
        ])
    }

    /// This record's Record By: the operator's choice, else the engine's setting (PAL-25).
    private var recordBy: String { ui.recordBy ?? engine.paletteSettings?.recordBy ?? "Mixed" }

    /// Where the Mask toggles start before a cell is tapped: the first palette window of the workspace.
    private var workspaceMaskBase: [String] {
        Workspace.all[safe: ui.workspace]?.windows.lazy.compactMap(\.paletteKind).first?.mask ?? ["C"]
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
        keys[6] = Softkey(name: "Cancel", action: { ui.cancelModes() })
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
            Softkey(name: "Cancel", action: { line.clear() }),
        ])
    }

    var body: some View {
        let context = context
        VStack(spacing: 0) {
            ColumnHeader(text: "Softkeys · \(context.name)")
            VStack(spacing: 2) {
                ForEach(Array(context.keys.enumerated()), id: \.offset) { index, key in
                    Button { key?.action?() } label: {
                        ZStack(alignment: .topLeading) {
                            SoftkeyShape().fill(t.surf2)
                            Text(String(UnicodeScalar(UInt8(65 + index)))).font(Typo.mono(11, .semibold)).foregroundStyle(t.fg3)
                                .padding(.leading, 14).padding(.top, 6)
                            if let key {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(key.name).font(Typo.legend()).foregroundStyle(t.fg)
                                    if !key.sub.isEmpty { Text(key.sub).font(Typo.mono(Typo.small)).foregroundStyle(t.fg2) }
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
