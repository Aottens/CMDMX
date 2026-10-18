import SwiftUI

// Theme tokens of UI v0.1 (specs/ux/main-screen.html, UI-02..04). The app has its own dark and
// day theme and never follows the macOS appearance. Colour means state only (UI-03).

struct Theme {
    // Zones and text.
    let chassis, panel, bg, surf, surf2, line, line2: Color
    let fg, fg2, fg3: Color
    /// Command line field: near-black, white in the day theme (the sketch's .cmd).
    let field: Color
    // State colours: red = programmer / unstored / armed, blue = selection, green = running / OK,
    // amber = fading / warning, cyan = from palette, violet = Blind.
    let red, blue, green, amber, cyan, violet: Color
    /// The one dim level of every disabled control (UI-28).
    let disabled = 0.3

    static let dark = Theme(
        chassis: Color(hex: 0x08090A), panel: Color(hex: 0x16181A), bg: Color(hex: 0x121314),
        surf: Color(hex: 0x1A1C1E), surf2: Color(hex: 0x222427), line: Color(hex: 0x2A2C2F), line2: Color(hex: 0x3A3D41),
        fg: Color(hex: 0xD8DADC), fg2: Color(hex: 0x8A8E93), fg3: Color(hex: 0x55595E), field: Color(hex: 0x0B0C0D),
        red: Color(hex: 0xE5484D), blue: Color(hex: 0x3E8EF7), green: Color(hex: 0x46A758),
        amber: Color(hex: 0xF5A524), cyan: Color(hex: 0x3FB7D9), violet: Color(hex: 0x8E4EC6))

    static let day = Theme(
        chassis: Color(hex: 0xC4C7CB), panel: Color(hex: 0xDFE1E3), bg: Color(hex: 0xE9EAEB),
        surf: Color(hex: 0xF7F7F8), surf2: Color(hex: 0xFFFFFF), line: Color(hex: 0xC9CBCE), line2: Color(hex: 0xA9ACB0),
        fg: Color(hex: 0x111213), fg2: Color(hex: 0x4D5156), fg3: Color(hex: 0x9A9EA3), field: Color(hex: 0xFFFFFF),
        red: Color(hex: 0xD1242F), blue: Color(hex: 0x0B63D6), green: Color(hex: 0x1F7A35),
        amber: Color(hex: 0xB36B00), cyan: Color(hex: 0x0A7EA4), violet: Color(hex: 0x8E4EC6))

    /// Attribute-group hue (I P C G B E S FX); used only for tags and tab underlines (UI-03).
    static func groupHue(_ group: String) -> Color {
        switch group {
        case "I": return Color(hex: 0xBDB38C)
        case "P": return Color(hex: 0x8FA3B5)
        case "C": return Color(hex: 0xB58FA8)
        case "G": return Color(hex: 0x8FB59C)
        case "B": return Color(hex: 0xB59E8F)
        case "E": return Color(hex: 0x9C8FB5)
        case "S": return Color(hex: 0x8F9396)
        default: return Color(hex: 0xB5A98F)
        }
    }
}

extension Color {
    init(hex: UInt32) {
        self.init(red: Double((hex >> 16) & 0xFF) / 255, green: Double((hex >> 8) & 0xFF) / 255, blue: Double(hex & 0xFF) / 255)
    }
}

private struct ThemeKey: EnvironmentKey { static let defaultValue = Theme.dark }

extension EnvironmentValues {
    var theme: Theme {
        get { self[ThemeKey.self] }
        set { self[ThemeKey.self] = newValue }
    }
}

/// Type (UI-04). Three sizes only: small 10, body 12, large 16.
enum Typo {
    static let small: CGFloat = 10
    static let body: CGFloat = 12
    static let large: CGFloat = 16

    /// Condensed SF Pro for legends and titles.
    static func legend(_ size: CGFloat = body, _ weight: Font.Weight = .medium) -> Font {
        .system(size: size, weight: weight).width(.condensed)
    }
    /// SF Mono with tabular digits for every number and the command line.
    static func mono(_ size: CGFloat = body, _ weight: Font.Weight = .regular) -> Font {
        .system(size: size, weight: weight, design: .monospaced).monospacedDigit()
    }
    static func ui(_ size: CGFloat = body, _ weight: Font.Weight = .regular) -> Font {
        .system(size: size, weight: weight)
    }
}

/// 1 px separators between flush zones.
struct HLine: View {
    @Environment(\.theme) private var t
    var body: some View { Rectangle().fill(t.line).frame(height: 1) }
}

struct VLine: View {
    @Environment(\.theme) private var t
    var body: some View { Rectangle().fill(t.line).frame(width: 1) }
}
