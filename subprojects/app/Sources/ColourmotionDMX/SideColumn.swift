import SwiftUI

// Side column (UI-16..18): function buttons, keypad, softkeys. Everything is drawn disabled in
// UX-02a; the tooltip names the spec each button waits for. UX-02c makes them live by passing
// an `action`.

/// One key of the console. Disabled while `action` is nil.
struct KeyButton: View {
    enum Style { case normal, number, command, enter }

    @Environment(\.theme) private var t
    let label: String
    /// Tooltip: the spec or task this key waits for.
    let spec: String
    var style = Style.normal
    var height: CGFloat = 40
    var action: (() -> Void)?

    var body: some View {
        let background: Color = style == .enter ? t.fg : style == .command ? t.bg : t.surf2
        let foreground: Color = style == .enter ? t.bg : style == .command ? t.fg2 : t.fg
        Button { action?() } label: {
            Text(label)
                .font(style == .number ? Typo.mono(Typo.large) : Typo.legend(style == .command ? 11 : Typo.body))
                .foregroundStyle(foreground)
                .lineLimit(1).minimumScaleFactor(0.7)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(RoundedRectangle(cornerRadius: radius).fill(background))
                .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(style == .normal ? t.line : t.line2, lineWidth: 1))
        }
        .buttonStyle(.plain)
        .frame(height: height)
        .disabled(action == nil)
        .opacity(action == nil ? 0.3 : 1)
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
    // Titan's order, two columns. Spec = what the key waits for; UX-02c wires Record and Patch.
    private static let keys: [(String, String)] = [
        ("Record", "UI-16 · UX-02c"), ("Update", "Palettes · P1-03"),
        ("Edit", "Palettes · P1-03"), ("Select If", "later"),
        ("Delete", "Palettes · P1-03"), ("Unfold", "later"),
        ("Copy", "Palettes · P1-03"), ("Move", "Palettes · P1-03"),
        ("Include", "Palettes · P1-03"), ("Release", "Playbacks · P1-04"),
        ("Patch", "UI-16 · UX-02c"), ("Disk", "Show file · P1-09"),
        ("Help", "later"), ("System", "later"),
        ("Blind", "later"), ("Off", "Highlight and Off · VS2-05"),
        ("Fan", "later"), ("Options", "Playbacks · P1-04"),
        ("Latch", "Playbacks · P1-04"), ("Shape", "later"),
    ]

    var body: some View {
        VStack(spacing: 0) {
            ColumnHeader(text: "Functions")
            LazyVGrid(columns: Array(repeating: GridItem(.fixed(68), spacing: 2), count: 2), spacing: 2) {
                ForEach(Self.keys, id: \.0) { FunctionKey(label: $0.0, spec: $0.1) }
            }
            Spacer(minLength: 0)
        }
        .padding(4)
    }
}

private struct FunctionKey: View {
    @Environment(\.theme) private var t
    let label: String
    let spec: String

    var body: some View {
        Text(label).font(Typo.legend(13)).foregroundStyle(t.fg)
            .padding(.horizontal, 7)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: radius).fill(t.surf))
            .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(t.line, lineWidth: 1))
            .overlay(alignment: .bottom) { Rectangle().fill(t.line2).frame(height: 2).padding(.horizontal, 1) }
            .frame(height: 40)
            .opacity(0.3)
            .help(spec)
            .accessibilityAddTraits(.isButton)
            .accessibilityHint("Disabled: \(spec)")
    }
}

// MARK: - Keypad (UI-17)

private struct Keypad: View {
    @Environment(\.theme) private var t

    // The sketch's 30 keys in order; the second field is the tooltip.
    private static let keys: [(String, String, KeyButton.Style)] = [
        ("Fixture", "UI-17 · UX-02c", .normal), ("Palette", "Palettes · P1-03", .normal),
        ("Macro", "later", .normal), ("Group", "UI-17 · UX-02c", .normal),
        ("Back", "UI-17 · UX-02c", .normal), ("Thro", "UI-17 · UX-02c", .normal),
        ("And", "UI-17 · UX-02c", .normal), ("@", "UI-17 · UX-02c", .normal),
        ("7", "UI-17 · UX-02c", .number), ("8", "UI-17 · UX-02c", .number),
        ("9", "UI-17 · UX-02c", .number), ("Avo", "later", .command),
        ("4", "UI-17 · UX-02c", .number), ("5", "UI-17 · UX-02c", .number),
        ("6", "UI-17 · UX-02c", .number), ("Set", "Set Legend", .normal),
        ("1", "UI-17 · UX-02c", .number), ("2", "UI-17 · UX-02c", .number),
        ("3", "UI-17 · UX-02c", .number), ("Clear", "UI-17 · UX-02c", .normal),
        ("Exit", "UI-17 · UX-02c", .command), ("0", "UI-17 · UX-02c", .number),
        (".", "UI-17 · UX-02c", .number), ("Enter", "UI-17 · UX-02c", .enter),
        ("Fix -1", "UI-17 · UX-02c", .command), ("Fix +1", "UI-17 · UX-02c", .command),
        ("Odd/Even", "UI-17 · UX-02c", .command), ("Highlight", "Highlight and Locate · VS2-05", .normal),
        ("View", "later", .command), ("Locate", "Highlight and Locate · VS2-05", .normal),
    ]

    var body: some View {
        VStack(spacing: 0) {
            ColumnHeader(text: "Keypad")
            LazyVGrid(columns: Array(repeating: GridItem(.fixed(50), spacing: 2), count: 4), spacing: 2) {
                ForEach(Self.keys, id: \.0) { KeyButton(label: $0.0, spec: $0.1, style: $0.2, height: 38) }
            }
            Spacer(minLength: 0)
        }
        .padding(4)
        .background(t.panel)
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

private struct Softkeys: View {
    @Environment(\.theme) private var t

    // Main context (nothing selected); selection and Record contexts come with UX-02c.
    private static let main: [(String, String, String)] = [
        ("Edit Times", "selected playback", "Cue lists · P1-08"),
        ("Playback Options", "", "Playbacks · P1-04"),
        ("Set Legend", "", "Set Legend"),
        ("Shapes & Effects", "phase 2", "Shapes · phase 2"),
        ("Timecode", "phase 3", "Timecode · phase 3"),
        ("Lock", "venue mode", "later"),
        ("Undo", "nothing", "Undo · P1-10"),
    ]

    var body: some View {
        VStack(spacing: 0) {
            ColumnHeader(text: "Softkeys · Main")
            VStack(spacing: 2) {
                ForEach(Array(Self.main.enumerated()), id: \.offset) { index, key in
                    ZStack(alignment: .topLeading) {
                        SoftkeyShape().fill(t.surf2)
                        Text(String(UnicodeScalar(UInt8(65 + index)))).font(Typo.mono(11, .semibold)).foregroundStyle(t.fg3)
                            .padding(.leading, 14).padding(.top, 6)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(key.0).font(Typo.legend()).foregroundStyle(t.fg)
                            if !key.1.isEmpty { Text(key.1).font(Typo.mono(Typo.small)).foregroundStyle(t.fg2) }
                        }
                        .padding(.leading, 28).padding(.top, 6)
                    }
                    .frame(maxHeight: .infinity).frame(minHeight: 44)
                    .opacity(0.3)
                    .help(key.2)
                    .accessibilityAddTraits(.isButton)
                    .accessibilityHint("Disabled: \(key.2)")
                }
            }
        }
        .padding(4)
        .frame(width: 120)
        .background(t.panel)
    }
}
