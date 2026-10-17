import AppKit
import SwiftUI

// Shared controls and the attribute bar (UI-20, IPC-16, IPC-17, IPC-19). Pure rendering of
// `EngineClient` state plus command sending: no selection order, targets or wheel layout
// is computed here. The Fixtures and Groups windows live in Windows.swift.

let hairline: CGFloat = 1
let radius: CGFloat = 2

/// Flat 1 px button; `fill` is a state tone (blue = selected, red = armed / touched).
struct FlatButton: ButtonStyle {
    enum Tone { case blue, red }
    var fill: Tone?
    /// Small variant for title rows.
    var compact = false

    func makeBody(configuration: Configuration) -> some View {
        FlatButtonBody(configuration: configuration, tone: fill, compact: compact)
    }
}

private struct FlatButtonBody: View {
    @Environment(\.theme) private var t
    @Environment(\.isEnabled) private var enabled
    let configuration: ButtonStyle.Configuration
    let tone: FlatButton.Tone?
    let compact: Bool

    var body: some View {
        let fill = tone.map { $0 == .blue ? t.blue : t.red }
        configuration.label
            .font(Typo.ui(compact ? Typo.small : 11))
            .padding(.horizontal, compact ? 6 : 8).padding(.vertical, compact ? 1 : 3)
            .foregroundStyle(fill == nil ? t.fg : Color.white)
            .background(RoundedRectangle(cornerRadius: radius).fill(fill ?? t.surf2))
            .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(t.line2, lineWidth: hairline))
            .opacity(!enabled ? 0.3 : configuration.isPressed ? 0.7 : 1)
    }
}

/// A window with a 22 pt title row (UI-09): uppercase condensed title on the left, an optional
/// accessory (e.g. Record, page tabs) and SF Mono info on the right. Content scrolls inside.
struct PanelWindow<Accessory: View, Content: View>: View {
    @Environment(\.theme) private var t
    let title: String
    var info = ""
    /// Draws a hairline around the window (side panels of the Patch screen).
    var framed = false
    @ViewBuilder var accessory: Accessory
    @ViewBuilder var content: Content

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                Text(title.uppercased()).font(Typo.legend(Typo.body, .semibold)).tracking(0.7)
                    .foregroundStyle(t.fg).lineLimit(1)
                Spacer(minLength: 0)
                accessory
                Text(info).font(Typo.mono(Typo.small)).foregroundStyle(t.fg2).lineLimit(1)
            }
            .padding(.horizontal, 8).frame(height: 22)
            HLine()
            content
        }
        .background(t.bg)
        .overlay { if framed { Rectangle().strokeBorder(t.line, lineWidth: hairline) } }
    }
}

// MARK: - Attribute bar (UI-20, IPC-16, IPC-17, IPC-19)

/// 96 pt zone in two rows: tabs, colour model and wheels (64 pt); command line, message, interim
/// buttons and programmer summary (32 pt).
struct AttributeBar: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @State private var command = ""

    var body: some View {
        let programmer = engine.programmer
        VStack(spacing: 0) {
            wheelsRow(programmer).frame(height: 64)
            commandRow(programmer).frame(height: 32)
        }
        .padding(.horizontal, 8)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(t.bg)
    }

    /// UI-20 row 1. The wheels share what the tabs and the colour switch leave: 168 pt each when
    /// there is room, down to 110 pt (eight fit at the 1280 pt minimum window).
    private func wheelsRow(_ programmer: ProgrammerState) -> some View {
        HStack(spacing: 10) {
            HStack(spacing: 2) {
                ForEach(programmer.attributeGroups) { tab in
                    AttributeTab(tab: tab, active: tab.group == programmer.activeGroup) {
                        engine.setAttributeGroup(tab.group)
                    }
                }
            }
            .fixedSize()

            if programmer.activeGroup == "C" {
                // IPC-19: wire values are the engine's ColourModel names.
                VStack(spacing: 2) {
                    ForEach([("HSI", "Hsi"), ("RGB", "Rgb"), ("CMY", "Cmy")], id: \.1) { label, model in
                        Button(label) { engine.setColourModel(model) }
                            .buttonStyle(ModeButton(on: programmer.colourModel == model))
                    }
                }
                .fixedSize()
            }

            GeometryReader { geo in
                // At least three slots like the encoders.
                let slots = max(3, programmer.wheels.count)
                let width = min(168, max(110, (geo.size.width - 6 * CGFloat(slots - 1)) / CGFloat(slots)))
                HStack(spacing: 6) {
                    ForEach(Array(programmer.wheels.enumerated()), id: \.element.id) { index, wheel in
                        WheelView(wheel: wheel, slot: index + 1, width: width)
                    }
                    ForEach(programmer.wheels.count..<slots, id: \.self) { index in
                        WheelFace(name: programmer.selection.isEmpty ? "select fixtures" : "", slot: index + 1,
                                  text: "—", value: nil, touched: false, width: width)
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
            }
        }
    }

    /// UI-20 row 2.
    private func commandRow(_ programmer: ProgrammerState) -> some View {
        let dirty = !programmer.selection.isEmpty || !programmer.inProgrammer.isEmpty
        let touchedGroups = programmer.attributeGroups.filter(\.hasValues).map(\.group).joined(separator: " ")
        return HStack(spacing: 10) {
            HStack(spacing: 8) {
                Text("›").font(Typo.mono(13)).foregroundStyle(t.fg3)
                // Still a text field; the token command line comes with UX-02c (UI-19).
                TextField("Command, e.g. 201 Thro 205 @ 50", text: $command)
                    .textFieldStyle(.plain)
                    .font(Typo.mono(13))
                    .foregroundStyle(t.fg)
                    .onSubmit {
                        let words = command.split(whereSeparator: \.isWhitespace).map(String.init)
                        if !words.isEmpty { engine.command(words: words) }
                        command = ""
                    }
            }
            .padding(.horizontal, 8).frame(height: 26)
            .frame(minWidth: 200, maxWidth: .infinity)
            .background(RoundedRectangle(cornerRadius: radius).fill(t.field))
            .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(t.line, lineWidth: hairline))

            // Only warnings and errors land here (IPC-17).
            Text(engine.lastMessage ?? " ").font(Typo.ui(11)).foregroundStyle(t.amber).lineLimit(1)
                .truncationMode(.tail).frame(minWidth: 0, maxWidth: .infinity, alignment: .leading)

            // Until UX-02c moves them to the keypad and softkeys.
            HStack(spacing: 3) {
                Button("Fix −1") { engine.step(-1) }
                Button("Fix +1") { engine.step(1) }
                Button("Odd/Even") { engine.pattern(2) }
                Button("All") { engine.selectAll() }
            }
            .buttonStyle(FlatButton(compact: true))
            .fixedSize()

            HStack(spacing: 4) {
                Text(!programmer.inProgrammer.isEmpty ? "\(programmer.inProgrammer.count) fx · \(touchedGroups)" : "empty")
                    .font(Typo.mono(13, .medium)).foregroundStyle(programmer.inProgrammer.isEmpty ? t.fg3 : t.red)
                    .lineLimit(1).padding(.horizontal, 8).frame(minWidth: 90, alignment: .leading)
                Button("Undo") {}.disabled(true).help("Undo · P1-10")
                Button("Clear") { engine.clearProgrammer() }.buttonStyle(ZoneKeyStyle(ring: dirty))
            }
            .buttonStyle(ZoneKeyStyle(ring: false))
            .fixedSize()
        }
    }
}

private struct AttributeTab: View {
    @Environment(\.theme) private var t
    let tab: AttributeGroupState
    let active: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(tab.group).font(Typo.legend(13, .semibold))
                .foregroundStyle(active ? t.bg : tab.available ? t.fg : t.fg2)
                .frame(width: 30, height: 34)
                .background(RoundedRectangle(cornerRadius: radius).fill(active ? t.fg : t.surf))
                .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(t.line, lineWidth: hairline))
                .overlay(alignment: .bottom) {
                    Rectangle().fill(Theme.groupHue(tab.group).opacity(0.7)).frame(height: 2).padding(.horizontal, 4).padding(.bottom, 3)
                }
                .overlay(alignment: .topTrailing) {
                    if tab.hasValues { Circle().fill(t.red).frame(width: 5, height: 5).padding(3) }
                }
                .opacity(tab.available ? 1 : 0.3)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(!tab.available)
    }
}

/// HSI / RGB / CMY switch next to the wheels.
private struct ModeButton: ButtonStyle {
    @Environment(\.theme) private var t
    let on: Bool
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.font(Typo.mono(9))
            .foregroundStyle(on ? t.bg : t.fg2)
            .padding(.horizontal, 5).frame(height: 16)
            .background(RoundedRectangle(cornerRadius: radius).fill(on ? t.fg : Color.clear))
            .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(t.line, lineWidth: hairline))
            .opacity(configuration.isPressed ? 0.7 : 1)
    }
}

/// Undo and Clear: 52 x 26 pt; the red ring means there is something to clear.
private struct ZoneKeyStyle: ButtonStyle {
    @Environment(\.theme) private var t
    @Environment(\.isEnabled) private var enabled
    let ring: Bool

    func makeBody(configuration: Configuration) -> some View {
        let down = configuration.isPressed
        configuration.label
            .font(Typo.legend(12, .medium))
            .foregroundStyle(down ? t.bg : ring ? t.red : t.fg)
            .frame(width: 52, height: 26)
            .background(RoundedRectangle(cornerRadius: radius).fill(down ? t.fg : t.surf))
            .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(ring ? t.red : t.line, lineWidth: ring ? 2 : hairline))
            .opacity(enabled ? 1 : 0.3)
    }
}

/// The face of a rolling encoder (the sketch's .wh): name, "enc n", value, roller strip and
/// position tick. `value` nil = an empty slot.
private struct WheelFace: View {
    @Environment(\.theme) private var t
    let name: String
    let slot: Int
    let text: String
    let value: Double?
    let touched: Bool
    let width: CGFloat

    var body: some View {
        ZStack(alignment: .topLeading) {
            Text(name.uppercased()).font(Typo.legend(11)).tracking(0.6).foregroundStyle(t.fg2).lineLimit(1)
                .padding(.leading, 6).padding(.top, 4)
            Text("enc \(slot)").font(Typo.mono(9)).foregroundStyle(t.fg3)
                .frame(maxWidth: .infinity, alignment: .trailing).padding(.trailing, 6).padding(.top, 4)
            Text(text).font(Typo.mono(15, .medium))
                .foregroundStyle(value == nil ? t.fg3 : touched ? t.red : t.fg).lineLimit(1)
                .padding(.horizontal, 6).padding(.top, 17)
        }
        .frame(width: width, height: 46, alignment: .topLeading)
        .overlay(alignment: .bottom) { roller }
        .background(RoundedRectangle(cornerRadius: radius).fill(t.surf))
        .clipShape(RoundedRectangle(cornerRadius: radius))
        .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(t.line, lineWidth: hairline))
    }

    /// 8 pt strip: hairline ticks every 6 pt that scroll with the value (-value x 300 pt, like the
    /// sketch's --x), plus a 2 pt tick at value x width.
    private var roller: some View {
        Canvas { ctx, size in
            var x = ((-(value ?? 0)) * 300).truncatingRemainder(dividingBy: 6)
            while x < size.width {
                ctx.fill(Path(CGRect(x: x, y: 0, width: 1, height: size.height)), with: .color(t.line2))
                x += 6
            }
            if let value {
                let p = min(max(value, 0), 1) * size.width
                ctx.fill(Path(CGRect(x: min(p, size.width - 2), y: 0, width: 2, height: size.height)), with: .color(t.fg))
            }
        }
        .frame(height: 8)
    }
}

/// One wheel: drag or scroll to send `programmer.wheel` (IPC-16).
private struct WheelView: View {
    @EnvironmentObject private var engine: EngineClient
    let wheel: WheelState
    let slot: Int
    let width: CGFloat
    /// Whole steps already sent during the current drag (remainder stays in the translation).
    @State private var sentSteps = 0

    /// 1 step per 4 pt of vertical movement.
    private let pointsPerStep: CGFloat = 4

    var body: some View {
        WheelFace(name: wheel.attribute, slot: slot,
                  text: wheel.attribute == "Hue" ? String(format: "%.0f°", wheel.value * 360)
                  : wheel.sixteenBit && engine.fineUsed.contains(wheel.attribute)
                  ? String(format: "%.1f %%", wheel.value * 100) : percent(wheel.value),
                  value: wheel.value, touched: wheel.touched, width: width)
        .background(ScrollCatcher { steps, flags in
            engine.wheel(attribute: wheel.attribute, steps: steps, resolution: resolution(flags))
        })
        .gesture(
            DragGesture(minimumDistance: 0)
                .onChanged { drag in
                    // Up = positive; Int() truncates toward zero, so the remainder is kept.
                    let total = Int(-drag.translation.height / pointsPerStep)
                    engine.wheel(attribute: wheel.attribute, steps: total - sentSteps,
                                 resolution: resolution(NSEvent.modifierFlags))
                    sentSteps = total
                }
                .onEnded { _ in sentSteps = 0 }
        )
    }
}

/// Option = Fine, Shift = Coarse (IPC-16).
func resolution(_ flags: NSEvent.ModifierFlags) -> WheelResolution {
    flags.contains(.option) ? .fine : flags.contains(.shift) ? .coarse : .normal
}

/// Reports scroll-wheel / trackpad scrolling over its bounds as whole steps.
/// A local event monitor instead of `scrollWheel(with:)` so it works behind
/// SwiftUI content that would otherwise take the hit test.
private struct ScrollCatcher: NSViewRepresentable {
    let onSteps: (Int, NSEvent.ModifierFlags) -> Void

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        context.coordinator.install(on: view)
        return view
    }

    func updateNSView(_ view: NSView, context: Context) { context.coordinator.onSteps = onSteps }

    static func dismantleNSView(_ view: NSView, coordinator: Coordinator) { coordinator.remove() }

    final class Coordinator {
        var onSteps: (Int, NSEvent.ModifierFlags) -> Void = { _, _ in }
        private var monitor: Any?
        private var remainder = 0.0

        func install(on view: NSView) {
            monitor = NSEvent.addLocalMonitorForEvents(matching: .scrollWheel) { [weak self, weak view] event in
                guard let self, let view, event.window === view.window,
                      view.bounds.contains(view.convert(event.locationInWindow, from: nil))
                else { return event }
                // Ignore momentum so a flick does not keep turning the wheel.
                if event.momentumPhase.isEmpty {
                    // Up = positive whatever the "natural scrolling" setting is.
                    let sign = event.isDirectionInvertedFromDevice ? -1.0 : 1.0
                    // Mouse wheel: 1 per tick. Trackpad (precise): 1 per 4 pt, like a drag.
                    let unit = event.hasPreciseScrollingDeltas ? 4.0 : 1.0
                    remainder += sign * Double(event.scrollingDeltaY) / unit
                    let whole = Int(remainder)   // truncates toward zero
                    if whole != 0 {
                        remainder -= Double(whole)
                        onSteps(whole, event.modifierFlags)
                    }
                }
                return nil
            }
        }

        func remove() {
            if let monitor { NSEvent.removeMonitor(monitor) }
            monitor = nil
        }
    }
}
