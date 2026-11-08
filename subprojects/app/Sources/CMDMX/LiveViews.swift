import AppKit
import Combine
import SwiftUI

// UI-71: values that change on every engine tick are drawn by small AppKit views that take them from
// `LivePlaybacks.tick(id)` and set them straight on a layer; SwiftUI is not asked to update.

typealias TickSubject = CurrentValueSubject<LiveTick, Never>

private func cg(_ c: Color) -> CGColor { NSColor(c).cgColor }

/// A layer view that never takes a click, so the SwiftUI gestures and buttons around it keep working.
class LiveLayerView: NSView {
    var sub: AnyCancellable?
    private var bound: ObjectIdentifier?
    var onTick: (LiveTick) -> Void = { _ in }

    override init(frame: NSRect) { super.init(frame: frame); wantsLayer = true; layer?.masksToBounds = true }
    required init?(coder: NSCoder) { nil }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    /// Follows `subject` (once per subject; the latest value is applied at once).
    func bind(_ subject: TickSubject?) {
        let id = subject.map { ObjectIdentifier($0) }
        guard id != bound else { return }
        bound = id
        sub = subject?.sink { [weak self] tick in self?.onTick(tick) }
    }

    /// Layer changes without the implicit animation.
    func plain(_ body: () -> Void) {
        CATransaction.begin(); CATransaction.setDisableActions(true)
        body()
        CATransaction.commit()
    }
}

/// A text from the tick. `sample` is the widest text it can show; it fixes the size so SwiftUI never relayouts.
struct LiveLabel: NSViewRepresentable {
    let tick: TickSubject
    let text: (LiveTick) -> String
    var size: CGFloat = Typo.small
    let color: Color
    let sample: String
    var alignment = CATextLayerAlignmentMode.left

    private var font: NSFont { NSFont.monospacedSystemFont(ofSize: size, weight: .regular) }

    final class View: LiveLayerView {
        let label = CATextLayer()
        var text: (LiveTick) -> String = { _ in "" }
        override init(frame: NSRect) {
            super.init(frame: frame)
            label.contentsScale = NSScreen.main?.backingScaleFactor ?? 2
            layer?.addSublayer(label)
            onTick = { [weak self] t in
                guard let self else { return }
                self.plain { self.label.string = self.text(t) }
            }
        }
        required init?(coder: NSCoder) { nil }
        override func layout() { super.layout(); plain { label.frame = bounds } }
        override func viewDidChangeBackingProperties() {
            super.viewDidChangeBackingProperties()
            label.contentsScale = window?.backingScaleFactor ?? 2
        }
    }

    func makeNSView(context: Context) -> View { View() }

    func updateNSView(_ v: View, context: Context) {
        v.plain {
            v.text = text
            v.label.font = font
            v.label.fontSize = size
            v.label.foregroundColor = cg(color)
            v.label.alignmentMode = alignment
            v.label.string = text(tick.value)
        }
        v.bind(tick)
    }

    func sizeThatFits(_ proposal: ProposedViewSize, nsView: View, context: Context) -> CGSize? {
        let s = (sample as NSString).size(withAttributes: [.font: font])
        return CGSize(width: ceil(s.width), height: ceil(s.height))
    }
}

/// A line along the left and bottom of its frame, as wide as the progress. Fills the frame it is given in height.
struct LiveProgressLine: NSViewRepresentable {
    let tick: TickSubject
    let color: Color

    final class View: LiveLayerView {
        let line = CALayer()
        var fraction = 0.0
        override init(frame: NSRect) {
            super.init(frame: frame)
            layer?.addSublayer(line)
            onTick = { [weak self] t in self?.fraction = min(1, max(0, t.progress ?? 0)); self?.place() }
        }
        required init?(coder: NSCoder) { nil }
        override func layout() { super.layout(); place() }
        func place() { plain { line.frame = CGRect(x: 0, y: 0, width: bounds.width * fraction, height: bounds.height) } }
    }

    func makeNSView(context: Context) -> View { View() }
    func updateNSView(_ v: View, context: Context) {
        v.plain { v.line.backgroundColor = cg(color) }
        v.fraction = min(1, max(0, tick.value.progress ?? 0)); v.place()
        v.bind(tick)
    }
}

/// UI-22, UI-24: the level bar's picture: background, fill, cap, cue progress and the %. `tick` drives a playback's strip;
/// without it `level` is drawn (the Grand Master).
struct LiveBar: NSViewRepresentable {
    let tick: TickSubject?
    let level: Double
    let running: Bool
    /// The fill is amber: a group master that darkens or lights its group, or the Grand Master below 100 %.
    let fillWarn: Bool
    /// The % is amber.
    let textWarn: Bool
    let showsProgress: Bool
    let theme: Theme

    final class View: LiveLayerView {
        let fill = CALayer(), cap = CALayer(), progress = CALayer(), text = CATextLayer()
        var level = 0.0, fraction = 0.0, showsProgress = false
        static let font = NSFont.monospacedSystemFont(ofSize: 11, weight: .regular)

        override init(frame: NSRect) {
            super.init(frame: frame)
            text.font = Self.font; text.fontSize = 11; text.alignmentMode = .right
            text.contentsScale = NSScreen.main?.backingScaleFactor ?? 2
            for l in [fill, cap, progress, text] { layer?.addSublayer(l) }
            onTick = { [weak self] t in
                guard let self else { return }
                self.level = t.level; self.fraction = min(1, max(0, t.progress ?? 0))
                self.place()
            }
        }
        required init?(coder: NSCoder) { nil }
        override func layout() { super.layout(); place() }
        override func viewDidChangeBackingProperties() {
            super.viewDidChangeBackingProperties()
            text.contentsScale = window?.backingScaleFactor ?? 2
        }

        func place() {
            plain {
                let w = bounds.width, h = bounds.height
                fill.frame = CGRect(x: 0, y: 0, width: w, height: h * level)
                cap.frame = CGRect(x: 0, y: h * level - 1, width: w, height: 2)
                progress.frame = CGRect(x: 0, y: 0, width: w * (showsProgress ? fraction : 0), height: 3)
                let lineHeight = ceil(Self.font.ascender - Self.font.descender + Self.font.leading)
                text.frame = CGRect(x: 0, y: h - 3 - lineHeight, width: max(0, w - 4), height: lineHeight)
                text.string = percent(level)
            }
        }
    }

    func makeNSView(context: Context) -> View { View() }

    func updateNSView(_ v: View, context: Context) {
        let t = theme
        v.plain {
            v.layer?.backgroundColor = cg(t.bg)
            v.fill.backgroundColor = cg(fillWarn ? t.amber.opacity(0.45) : running ? t.green.opacity(0.4) : t.line2)
            v.cap.backgroundColor = cg(t.fg)
            v.progress.backgroundColor = cg(t.green)
            v.text.foregroundColor = cg(textWarn ? t.amber : t.fg)
        }
        v.showsProgress = showsProgress
        if let tick {
            v.level = tick.value.level; v.fraction = min(1, max(0, tick.value.progress ?? 0))
        } else {
            v.level = level; v.fraction = 0
        }
        v.place()
        v.bind(tick)
    }
}
