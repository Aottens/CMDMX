import AppKit
import Combine
import SwiftUI

// UI-71: values that change on every engine tick are drawn by small AppKit views that take them from
// `LivePlaybacks.tick(id)` and set them straight on a layer; SwiftUI is not asked to update.

typealias TickSubject = CurrentValueSubject<LiveTick, Never>

private func cg(_ c: Color) -> CGColor { NSColor(c).cgColor }

/// A layer view that never takes a click, so the SwiftUI gestures and buttons around it keep working.
/// SNP-06: generic over the subject's value (a tick, an output cell, a grid row, the DMX channels).
class LiveLayerView<Value>: NSView {
    var sub: AnyCancellable?
    private var bound: ObjectIdentifier?
    var onTick: (Value) -> Void = { _ in }

    override init(frame: NSRect) { super.init(frame: frame); wantsLayer = true; layer?.masksToBounds = true }
    required init?(coder: NSCoder) { nil }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    /// Follows `subject` (once per subject; the latest value is applied at once).
    func bind(_ subject: CurrentValueSubject<Value, Never>?) {
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

/// A text from the subject's value. `sample` is the widest text it can show; it fixes the size so SwiftUI never relayouts.
/// SNP-06: `fillsWidth` takes the proposed width instead (truncating at the end), `attributed` draws colours inside the text.
struct LiveLabel<Value>: NSViewRepresentable {
    let tick: CurrentValueSubject<Value, Never>
    /// A `String`, or an `NSAttributedString` that carries its own font and colours.
    let render: (Value) -> Any
    var size: CGFloat = Typo.small
    var ui = false
    let color: Color
    let sample: String
    var alignment = CATextLayerAlignmentMode.left
    var fillsWidth = false

    init(tick: CurrentValueSubject<Value, Never>, text: @escaping (Value) -> String, size: CGFloat = Typo.small, ui: Bool = false,
         color: Color, sample: String, alignment: CATextLayerAlignmentMode = .left, fillsWidth: Bool = false) {
        self.tick = tick; render = text; self.size = size; self.ui = ui; self.color = color
        self.sample = sample; self.alignment = alignment; self.fillsWidth = fillsWidth
    }

    init(tick: CurrentValueSubject<Value, Never>, attributed: @escaping (Value) -> NSAttributedString, size: CGFloat = Typo.small,
         ui: Bool = false, sample: String, alignment: CATextLayerAlignmentMode = .left, fillsWidth: Bool = false) {
        self.tick = tick; render = attributed; self.size = size; self.ui = ui; color = .primary
        self.sample = sample; self.alignment = alignment; self.fillsWidth = fillsWidth
    }

    private var font: NSFont { ui ? NSFont.systemFont(ofSize: size) : NSFont.monospacedSystemFont(ofSize: size, weight: .regular) }

    final class View: LiveLayerView<Value> {
        let label = CATextLayer()
        var render: (Value) -> Any = { _ in "" }
        private var last: Any?
        override init(frame: NSRect) {
            super.init(frame: frame)
            label.contentsScale = NSScreen.main?.backingScaleFactor ?? 2
            layer?.addSublayer(label)
            onTick = { [weak self] value in self?.show(value) }
        }
        required init?(coder: NSCoder) { nil }
        /// Sets the text only when it differs (the 512 DMX values change rarely).
        func show(_ value: Value, force: Bool = false) {
            let new = render(value)
            if !force, let last, (last as AnyObject).isEqual(new as AnyObject) { return }
            last = new
            plain { label.string = new }
        }
        override func layout() { super.layout(); plain { label.frame = bounds } }
        override func viewDidChangeBackingProperties() {
            super.viewDidChangeBackingProperties()
            label.contentsScale = window?.backingScaleFactor ?? 2
        }
    }

    func makeNSView(context: Context) -> View { View() }

    func updateNSView(_ v: View, context: Context) {
        v.plain {
            v.render = render
            v.label.font = font
            v.label.fontSize = size
            v.label.foregroundColor = cg(color)
            v.label.alignmentMode = alignment
            v.label.truncationMode = fillsWidth ? .end : .none
        }
        v.show(tick.value, force: true)
        v.bind(tick)
    }

    func sizeThatFits(_ proposal: ProposedViewSize, nsView: View, context: Context) -> CGSize? {
        let s = (sample as NSString).size(withAttributes: [.font: font])
        let proposed = proposal.width.flatMap { $0.isFinite ? $0 : nil }
        return CGSize(width: fillsWidth ? (proposed ?? ceil(s.width)) : ceil(s.width), height: ceil(s.height))
    }
}

/// SNP-06: a bar that is `fraction` of its width wide on a track, for a level that moves with the output.
struct LiveFill<Value>: NSViewRepresentable {
    let tick: CurrentValueSubject<Value, Never>
    let fraction: (Value) -> Double
    let track: Color
    let fill: Color

    final class View: LiveLayerView<Value> {
        let bar = CALayer()
        var fraction: (Value) -> Double = { _ in 0 }
        private var amount = 0.0
        override init(frame: NSRect) {
            super.init(frame: frame)
            layer?.addSublayer(bar)
            onTick = { [weak self] v in self?.set(v) }
        }
        required init?(coder: NSCoder) { nil }
        func set(_ v: Value) { amount = min(1, max(0, fraction(v))); place() }
        override func layout() { super.layout(); place() }
        func place() { plain { bar.frame = CGRect(x: 0, y: 0, width: bounds.width * amount, height: bounds.height) } }
    }

    func makeNSView(context: Context) -> View { View() }
    func updateNSView(_ v: View, context: Context) {
        v.plain { v.layer?.backgroundColor = cg(track); v.bar.backgroundColor = cg(fill) }
        v.fraction = fraction
        v.set(tick.value)
        v.bind(tick)
    }
}

/// A line along the left and bottom of its frame, as wide as the progress. Fills the frame it is given in height.
struct LiveProgressLine: NSViewRepresentable {
    let tick: TickSubject
    let color: Color

    final class View: LiveLayerView<LiveTick> {
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

    final class View: LiveLayerView<LiveTick> {
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
