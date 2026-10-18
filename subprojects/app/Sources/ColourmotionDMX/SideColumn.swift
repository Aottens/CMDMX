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

    // Titan's order, two columns. An empty spec = live; otherwise the spec the key waits for.
    private static let keys: [(String, String)] = [
        ("Record", ""), ("Update", "Palettes · P1-03"),
        ("Edit", "Palettes · P1-03"), ("Select If", "later"),
        ("Delete", "Palettes · P1-03"), ("Unfold", "later"),
        ("Copy", "Palettes · P1-03"), ("Move", "Palettes · P1-03"),
        ("Include", "Palettes · P1-03"), ("Release", "Playbacks · P1-04"),
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
                    FunctionKey(label: label, spec: spec, ring: label == "Record" && ui.recordArmed,
                                action: spec.isEmpty ? { press(label) } : nil)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(4)
    }

    private func press(_ label: String) {
        switch label {
        case "Record": ui.recordArmed.toggle()   // the next group tap records (Groups window)
        case "Patch": ui.screen = .patch
        default: ui.screen = .show               // Disk
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
        ("Fixture", "", .normal), ("Palette", "Palettes · P1-03", .normal),
        ("Macro", "later", .normal), ("Group", "", .normal),
        ("Back", "", .normal), ("Thro", "", .normal),
        ("And", "", .normal), ("@", "", .normal),
        ("7", "", .number), ("8", "", .number),
        ("9", "", .number), ("Avo", "later", .command),
        ("4", "", .number), ("5", "", .number),
        ("6", "", .number), ("Set", "Set Legend", .normal),
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
        case "Clear": engine.clearProgrammer(); ui.recordArmed = false
        case "Fix -1": engine.step(-1)
        case "Fix +1": engine.step(1)
        case "Odd/Even": engine.pattern(2)
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
    var action: (() -> Void)?
}

private struct Softkeys: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @EnvironmentObject private var ui: UIState

    /// The context follows the state: Record armed, else a selection, else Main (UI-18).
    private var context: (name: String, keys: [Softkey?]) {
        if ui.recordArmed {
            return ("Record", [
                Softkey(name: "Merge / Replace", spec: "Palettes · P1-03"),
                Softkey(name: "Record by", spec: "Palettes · P1-03"),
                Softkey(name: "Mask", spec: "Palettes · P1-03"),
                nil, nil, nil,
                Softkey(name: "Cancel", action: { ui.recordArmed = false }),
            ])
        }
        if !engine.programmer.selection.isEmpty {
            return ("Selection", [
                Softkey(name: "Next", sub: "Fix +1", action: { engine.step(1) }),
                Softkey(name: "Previous", sub: "Fix −1", action: { engine.step(-1) }),
                Softkey(name: "All", sub: "whole selection", action: { engine.selectAll() }),
                Softkey(name: "Odd/Even", action: { engine.pattern(2) }),
                Softkey(name: "Set Legend", spec: "Set Legend"),
                Softkey(name: "Playback Options", spec: "Playbacks · P1-04"),
                Softkey(name: "Undo", sub: "nothing", spec: "Undo · P1-10"),
            ])
        }
        return ("Main", [
            Softkey(name: "Edit Times", sub: "selected playback", spec: "Cue lists · P1-08"),
            Softkey(name: "Playback Options", spec: "Playbacks · P1-04"),
            Softkey(name: "Set Legend", spec: "Set Legend"),
            Softkey(name: "Shapes & Effects", sub: "phase 2", spec: "Shapes · phase 2"),
            Softkey(name: "Timecode", sub: "phase 3", spec: "Timecode · phase 3"),
            Softkey(name: "Lock", sub: "venue mode", spec: "later"),
            Softkey(name: "Undo", sub: "nothing", spec: "Undo · P1-10"),
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
