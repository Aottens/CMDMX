import AppKit
import SwiftUI

// Shared controls and the attribute bar (IPC-16, IPC-17, IPC-19). Pure rendering of
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

// MARK: - Attribute bar (IPC-16, IPC-17, IPC-19)

/// 64 pt zone: tabs, wheels (with the colour model switch), command line, programmer summary.
struct AttributeBar: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @State private var command = ""

    var body: some View {
        let programmer = engine.programmer
        let dirty = !programmer.selection.isEmpty || !programmer.inProgrammer.isEmpty
        let touchedGroups = programmer.attributeGroups.filter(\.hasValues).map(\.group).joined(separator: " ")

        HStack(spacing: 10) {
            HStack(spacing: 2) {
                ForEach(programmer.attributeGroups) { tab in
                    AttributeTab(tab: tab, active: tab.group == programmer.activeGroup) {
                        engine.setAttributeGroup(tab.group)
                    }
                }
            }

            ViewThatFits(in: .horizontal) {
                wheelRow(programmer)
                ScrollView(.horizontal, showsIndicators: false) { wheelRow(programmer) }
            }

            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 8) {
                    Text("›").font(Typo.mono(13)).foregroundStyle(t.fg3)
                    // Only keyboard-driven control in the app: no global key handlers (IPC-17).
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
                .background(RoundedRectangle(cornerRadius: radius).fill(t.bg))
                .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(t.line, lineWidth: hairline))
                Text(engine.lastMessage ?? " ").font(Typo.ui(11)).foregroundStyle(t.amber).lineLimit(1)
            }
            .frame(minWidth: 200, maxWidth: .infinity)

            VStack(alignment: .leading, spacing: 0) {
                Text(!programmer.inProgrammer.isEmpty ? "\(programmer.inProgrammer.count) fx · \(touchedGroups)" : "empty")
                    .font(Typo.mono(13)).foregroundStyle(programmer.inProgrammer.isEmpty ? t.fg3 : t.red)
                Text("programmer").font(Typo.mono(Typo.small)).foregroundStyle(t.fg2)
            }
            .frame(minWidth: 96, alignment: .leading)

            VStack(spacing: 3) {
                HStack(spacing: 3) {
                    Button("Fix −1") { engine.step(-1) }
                    Button("Fix +1") { engine.step(1) }
                    Button("Odd/Even") { engine.pattern(2) }
                }
                HStack(spacing: 3) {
                    Button("All") { engine.selectAll() }
                    Button("Clear") { engine.clearProgrammer() }
                        .buttonStyle(FlatButton(fill: dirty ? .red : nil, compact: true))
                }
            }
            .buttonStyle(FlatButton(compact: true))
        }
        .padding(.horizontal, 8)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(t.bg)
    }

    /// The colour model switch (C tab only), then the wheels; at least three slots like the encoders.
    private func wheelRow(_ programmer: ProgrammerState) -> some View {
        HStack(spacing: 6) {
            if programmer.activeGroup == "C" {
                // IPC-19: wire values are the engine's ColourModel names.
                VStack(spacing: 2) {
                    ForEach([("HSI", "Hsi"), ("RGB", "Rgb"), ("CMY", "Cmy")], id: \.1) { label, model in
                        Button(label) { engine.setColourModel(model) }
                            .buttonStyle(ModeButton(on: programmer.colourModel == model))
                    }
                }
            }
            ForEach(programmer.wheels) { WheelView(wheel: $0) }
            ForEach(programmer.wheels.count..<max(3, programmer.wheels.count), id: \.self) { slot in
                EmptyWheel(slot: slot + 1)
            }
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
            .padding(.horizontal, 5).frame(height: 14)
            .background(RoundedRectangle(cornerRadius: radius).fill(on ? t.fg : Color.clear))
            .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(t.line, lineWidth: hairline))
            .opacity(configuration.isPressed ? 0.7 : 1)
    }
}

private struct EmptyWheel: View {
    @Environment(\.theme) private var t
    let slot: Int
    var body: some View {
        ZStack(alignment: .topLeading) {
            Text("enc \(slot)").font(Typo.mono(9)).foregroundStyle(t.fg3)
                .frame(maxWidth: .infinity, alignment: .trailing).padding(.horizontal, 6).padding(.top, 4)
            Text("—").font(Typo.mono(15, .medium)).foregroundStyle(t.fg3).padding(.leading, 6).padding(.top, 17)
        }
        .frame(width: 110, height: 46)
        .background(RoundedRectangle(cornerRadius: radius).fill(t.surf))
        .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(t.line, lineWidth: hairline))
    }
}

/// One wheel: name, value %, drag or scroll to send `programmer.wheel`.
private struct WheelView: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    let wheel: WheelState
    /// Whole steps already sent during the current drag (remainder stays in the translation).
    @State private var sentSteps = 0

    /// 1 step per 4 pt of vertical movement.
    private let pointsPerStep: CGFloat = 4

    var body: some View {
        ZStack(alignment: .topLeading) {
            Text(wheel.attribute.uppercased()).font(Typo.legend(11)).tracking(0.6).foregroundStyle(t.fg2).lineLimit(1)
                .padding(.leading, 6).padding(.top, 4)
            Text(wheel.attribute == "Hue" ? String(format: "%.0f°", wheel.value * 360)
                 : wheel.sixteenBit && engine.fineUsed.contains(wheel.attribute)
                 ? String(format: "%.1f %%", wheel.value * 100) : percent(wheel.value))
                .font(Typo.mono(15, .medium))
                .foregroundStyle(wheel.touched ? t.red : t.fg)
                .padding(.leading, 6).padding(.top, 17)
        }
        .frame(width: 110, height: 46, alignment: .topLeading)
        .background(RoundedRectangle(cornerRadius: radius).fill(t.surf))
        .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(t.line, lineWidth: hairline))
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
private func resolution(_ flags: NSEvent.ModifierFlags) -> WheelResolution {
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
