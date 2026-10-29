import AppKit
import SwiftUI

// Palette icons (PAL-21, PAL-22, IDEA-14 phase 1). The show stores only our own ids
// (`category.name`, lower case); this file maps them to SF Symbols, or to a few own glyphs where
// SF Symbols has nothing fitting (gobo, moving head, par can, beam angle). One line style,
// monochrome: the only colour on a palette button is a colour palette's own (research.md).
// Unknown ids are drawn as the automatic icon the engine computed.

/// The stroke of every own glyph (research.md "Palette iconography": 1.25 pt).
private let glyphLine = StrokeStyle(lineWidth: 1.25, lineCap: .round, lineJoin: .round)

/// Glyphs drawn by us, in a unit square.
enum OwnGlyph {
    case movingHead, par, beamWide, beamNarrow, goboDots, goboRays, goboRing
}

private struct OwnGlyphShape: Shape {
    let glyph: OwnGlyph

    func path(in rect: CGRect) -> Path {
        var p = Path()
        func circle(_ x: CGFloat, _ y: CGFloat, _ r: CGFloat) {
            p.addEllipse(in: CGRect(x: x - r, y: y - r, width: 2 * r, height: 2 * r))
        }
        func line(_ x1: CGFloat, _ y1: CGFloat, _ x2: CGFloat, _ y2: CGFloat) {
            p.move(to: CGPoint(x: x1, y: y1)); p.addLine(to: CGPoint(x: x2, y: y2))
        }
        switch glyph {
        case .movingHead:
            p.addRoundedRect(in: CGRect(x: 0.2, y: 0.86, width: 0.6, height: 0.12), cornerSize: CGSize(width: 0.03, height: 0.03))
            line(0.28, 0.86, 0.28, 0.4)
            line(0.72, 0.86, 0.72, 0.4)
            let head = Path(roundedRect: CGRect(x: -0.17, y: -0.26, width: 0.34, height: 0.52), cornerRadius: 0.08)
            p.addPath(head.applying(CGAffineTransform(rotationAngle: -0.45).concatenating(CGAffineTransform(translationX: 0.5, y: 0.38))))
        case .par:
            circle(0.5, 0.42, 0.3)
            circle(0.5, 0.42, 0.14)
            p.move(to: CGPoint(x: 0.14, y: 0.42))
            p.addLine(to: CGPoint(x: 0.14, y: 0.88)); p.addLine(to: CGPoint(x: 0.86, y: 0.88)); p.addLine(to: CGPoint(x: 0.86, y: 0.42))
        case .beamWide:
            circle(0.5, 0.08, 0.06)
            line(0.5, 0.14, 0.04, 0.95); line(0.5, 0.14, 0.96, 0.95); line(0.04, 0.95, 0.96, 0.95)
        case .beamNarrow:
            circle(0.5, 0.08, 0.06)
            line(0.5, 0.14, 0.28, 0.95); line(0.5, 0.14, 0.72, 0.95); line(0.28, 0.95, 0.72, 0.95)
        case .goboDots:
            circle(0.5, 0.5, 0.45)
            circle(0.36, 0.4, 0.07); circle(0.63, 0.34, 0.06); circle(0.5, 0.66, 0.08)
        case .goboRays:
            circle(0.5, 0.5, 0.45)
            for k in 0..<6 {
                let a = CGFloat(k) * .pi / 3
                line(0.5 + 0.12 * cos(a), 0.5 + 0.12 * sin(a), 0.5 + 0.36 * cos(a), 0.5 + 0.36 * sin(a))
            }
        case .goboRing:
            circle(0.5, 0.5, 0.45); circle(0.5, 0.5, 0.28); circle(0.5, 0.5, 0.1)
        }
        return p.applying(CGAffineTransform(a: rect.width, b: 0, c: 0, d: rect.height, tx: rect.minX, ty: rect.minY))
    }
}

enum IconEntry {
    case symbol(String)
    case own(OwnGlyph)
}

/// The built-in icon library (PAL-22): our ids, grouped by category in picker order.
enum PaletteIcons {
    static let categories = ["colour", "position", "beam", "gobo", "effect", "fixture", "service", "generic"]

    static let all: [(id: String, entry: IconEntry)] = [
        ("colour.drop", .symbol("drop.fill")), ("colour.warm", .symbol("sun.max")),
        ("colour.cool", .symbol("snowflake")), ("colour.rainbow", .symbol("rainbow")),
        ("colour.fire", .symbol("flame")), ("colour.palette", .symbol("paintpalette")),

        ("position.centre", .symbol("scope")), ("position.stage-left", .symbol("arrow.left")),
        ("position.stage-right", .symbol("arrow.right")), ("position.up", .symbol("arrow.up")),
        ("position.down", .symbol("arrow.down")), ("position.pulpit", .symbol("mic")),
        ("position.audience", .symbol("person.3")),

        ("beam.zoom", .symbol("plus.magnifyingglass")), ("beam.focus", .symbol("viewfinder")),
        ("beam.iris", .symbol("camera.aperture")), ("beam.frost", .symbol("cloud")),
        ("beam.strobe", .symbol("bolt")), ("beam.wide", .own(.beamWide)), ("beam.narrow", .own(.beamNarrow)),

        ("gobo.dots", .own(.goboDots)), ("gobo.rays", .own(.goboRays)), ("gobo.ring", .own(.goboRing)),
        ("gobo.star", .symbol("star")), ("gobo.leaf", .symbol("leaf")), ("gobo.cross", .symbol("cross")),

        ("effect.prism", .symbol("triangle")), ("effect.rotate", .symbol("arrow.triangle.2.circlepath")),
        ("effect.pulse", .symbol("waveform.path.ecg")), ("effect.sparkle", .symbol("sparkles")),
        ("effect.blackout", .symbol("moon")),

        ("fixture.moving-head", .own(.movingHead)), ("fixture.par", .own(.par)),
        ("fixture.spot", .symbol("flashlight.on.fill")), ("fixture.bar", .symbol("rectangle.split.3x1")),

        ("service.welcome", .symbol("hand.wave")), ("service.worship", .symbol("music.note")),
        ("service.prayer", .symbol("hands.sparkles")), ("service.sermon", .symbol("book")),
        ("service.baptism", .symbol("water.waves")), ("service.communion", .symbol("wineglass")),
        ("service.video", .symbol("play.rectangle")), ("service.walk-out", .symbol("figure.walk")),
        ("service.offering", .symbol("gift")), ("service.children", .symbol("figure.and.child.holdinghands")),
        ("service.announcements", .symbol("megaphone")), ("service.band", .symbol("guitars")),
        ("service.choir", .symbol("music.mic")), ("service.blessing", .symbol("hand.raised")),
        ("service.pre-service", .symbol("clock")),

        ("generic.star", .symbol("star.fill")), ("generic.heart", .symbol("heart")),
        ("generic.circle", .symbol("circle")), ("generic.square", .symbol("square")),
        ("generic.flag", .symbol("flag")), ("generic.bell", .symbol("bell")),
    ]

    private static let byId = Dictionary(all.map { ($0.id, $0.entry) }, uniquingKeysWith: { a, _ in a })

    static func entry(_ id: String) -> IconEntry? { byId[id] }

    static func ids(in category: String) -> [(id: String, entry: IconEntry)] {
        all.filter { $0.id.hasPrefix(category + ".") }
    }

    /// "service.walk-out" → "walk out"
    static func name(_ id: String) -> String {
        String(id.drop { $0 != "." }.dropFirst()).replacingOccurrences(of: "-", with: " ")
    }
}

/// One library glyph, monochrome in the current foreground style.
struct IconGlyph: View {
    @Environment(\.theme) private var t
    let entry: IconEntry
    var size: CGFloat = 15

    var body: some View {
        switch entry {
        case .symbol(let name):
            // A symbol this macOS lacks would draw nothing; a dot keeps the button readable.
            Image(systemName: NSImage(systemSymbolName: name, accessibilityDescription: nil) == nil ? "circle.fill" : name)
                .font(.system(size: size, weight: .regular))
                .frame(width: size * 1.3, height: size * 1.3)
        case .own(let glyph):
            OwnGlyphShape(glyph: glyph).stroke(style: glyphLine)
                .frame(width: size, height: size)
                .frame(width: size * 1.3, height: size * 1.3)
        }
    }
}

/// The automatic icon (PAL-21): a swatch, a split swatch, a position plot or "ATTR nn%".
struct AutoIconView: View {
    @Environment(\.theme) private var t
    let icon: AutoIcon?

    var body: some View {
        switch icon {
        case .colours(let colours):
            // Up to 4 colours side by side: one swatch, or a split swatch.
            HStack(spacing: 0) {
                ForEach(Array(colours.enumerated()), id: \.offset) { _, c in
                    Rectangle().fill(Color(red: c.r, green: c.g, blue: c.b))
                }
            }
            .frame(height: 12).clipShape(RoundedRectangle(cornerRadius: 1))
        case .position(let points):
            Canvas { ctx, size in
                // The sketch's plot: a floor line, and a beam from the rig down to each point.
                ctx.stroke(Path { $0.move(to: CGPoint(x: 0, y: size.height - 0.5)); $0.addLine(to: CGPoint(x: size.width, y: size.height - 0.5)) },
                           with: .color(t.fg2), lineWidth: 1)
                for p in points.prefix(8) {
                    let x = 4 + p.pan.clamped01 * (size.width - 8), y = 3 + p.tilt.clamped01 * (size.height - 6)
                    ctx.stroke(Path { $0.move(to: CGPoint(x: size.width / 2, y: 0.5)); $0.addLine(to: CGPoint(x: x, y: y)) },
                               with: .color(t.fg2), lineWidth: 1)
                    ctx.fill(Path(ellipseIn: CGRect(x: x - 1.8, y: y - 1.8, width: 3.6, height: 3.6)), with: .color(t.fg))
                }
            }
            .frame(width: 48, height: 16)
        case .value(let attribute, let value):
            // Two short lines so it fits a strip's 26 pt icon (VS3-05 run: "DIMME…").
            VStack(spacing: 0) {
                Text(String(attribute.uppercased().prefix(3)))
                Text("\(Int((value * 100).rounded()))%")
            }
            .font(Typo.mono(Typo.small)).foregroundStyle(t.fg2).lineLimit(1).minimumScaleFactor(0.7)
        case nil:
            Color.clear.frame(height: 12)
        }
    }
}

/// The icon of a palette: the picked one if the library knows it, else the automatic icon.
struct PaletteIconView: View {
    @Environment(\.theme) private var t
    let palette: PaletteInfo
    /// The picker's "Auto" button always shows the automatic icon.
    var forceAuto = false

    var body: some View {
        if !forceAuto, let id = palette.icon, let entry = PaletteIcons.entry(id) {
            IconGlyph(entry: entry).foregroundStyle(t.fg).frame(maxWidth: .infinity, alignment: .leading)
        } else if !forceAuto, palette.autoIcon == nil, let fx = palette.effects.first {
            // FX-22: a palette with only effect entries shows its effect's still.
            ShapeIcon(shape: fx.shape).frame(maxWidth: .infinity)
        } else {
            AutoIconView(icon: palette.autoIcon)
        }
    }
}
