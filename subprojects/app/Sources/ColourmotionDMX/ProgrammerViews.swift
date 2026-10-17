import AppKit
import SwiftUI

// Programmer workspace (IPC-15..17). Pure rendering of `EngineClient` state plus
// command sending: no selection order, targets or wheel layout is computed here.

let hairline: CGFloat = 1
let radius: CGFloat = 3

/// Flat 1 px button; `fill` is a state colour (blue = selected, red = armed / touched).
struct FlatButton: ButtonStyle {
    var fill: Color?
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.caption)
            .padding(.horizontal, 8).padding(.vertical, 3)
            .foregroundStyle(fill == nil ? Color.primary : Color.white)
            .background(RoundedRectangle(cornerRadius: radius).fill(fill ?? Color.secondary.opacity(0.12)))
            .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(Color.secondary.opacity(0.4), lineWidth: hairline))
            .opacity(configuration.isPressed ? 0.7 : 1)
    }
}

/// A window with a small title bar and an optional accessory (e.g. Record).
struct PanelWindow<Accessory: View, Content: View>: View {
    let title: String
    @ViewBuilder var accessory: Accessory
    @ViewBuilder var content: Content

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text(title.uppercased()).font(.caption2.weight(.semibold)).foregroundStyle(.secondary)
                Spacer()
                accessory
            }
            .padding(.horizontal, 8).frame(height: 24)
            .background(Color.secondary.opacity(0.12))
            Divider()
            content
        }
        .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(Color.secondary.opacity(0.4), lineWidth: hairline))
        .clipShape(RoundedRectangle(cornerRadius: radius))
    }
}

// MARK: - Fixtures window (IPC-15)

struct FixturesWindow: View {
    @EnvironmentObject private var engine: EngineClient

    /// Display order only: by handle (page, slot); fixtures without a handle last, by number.
    private static func handleOrder(_ a: FixtureInfo, _ b: FixtureInfo) -> Bool {
        switch (a.handle, b.handle) {
        case let (x?, y?): return (x.page, x.slot) < (y.page, y.slot)
        case (_?, nil): return true
        case (nil, _?): return false
        case (nil, nil): return a.number < b.number
        }
    }

    var body: some View {
        let programmer = engine.programmer
        // Display lookups only: 1-based position in the engine's selection order.
        let order = Dictionary(programmer.selection.enumerated().map { ($1, $0 + 1) }, uniquingKeysWith: { a, _ in a })
        // Thick outline only while the engine's targets differ from its selection (sub-selection).
        let subSelection = programmer.targets != programmer.selection
        let targets = Set(programmer.targets)
        let inProgrammer = Set(programmer.inProgrammer)

        PanelWindow(title: "Fixtures", accessory: { EmptyView() }) {
            ScrollView {
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 60, maximum: 80), spacing: 4)], spacing: 4) {
                    ForEach(engine.fixtures.sorted(by: Self.handleOrder)) { fixture in
                        FixtureButton(
                            fixture: fixture,
                            position: order[fixture.id],
                            isTarget: subSelection && targets.contains(fixture.id),
                            inProgrammer: inProgrammer.contains(fixture.id)
                        ) { engine.selectFixture(fixture.id) }
                    }
                }
                .padding(6)
            }
        }
    }
}

private struct FixtureButton: View {
    let fixture: FixtureInfo
    let position: Int?       // selection order, nil = not selected
    let isTarget: Bool
    let inProgrammer: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            VStack(spacing: 1) {
                Text("\(fixture.number)").font(.title3.weight(.semibold).monospacedDigit())
                Text(fixture.profile).font(.caption2).lineLimit(1).minimumScaleFactor(0.6)
                    .foregroundStyle(position == nil ? Color.secondary : Color.white.opacity(0.85))
            }
            .foregroundStyle(position == nil ? Color.primary : Color.white)
            .frame(maxWidth: .infinity).frame(height: 46)
            .background(position == nil ? Color.secondary.opacity(0.12) : Color.blue)
            .contentShape(Rectangle())
            .overlay(alignment: .top) {
                if inProgrammer { Rectangle().fill(Color.red).frame(height: 4) }
            }
            .overlay(alignment: .topTrailing) {
                if let position {
                    Text("\(position)").font(.system(size: 9, weight: .bold).monospacedDigit())
                        .padding(.horizontal, 3).background(Capsule().fill(Color.black.opacity(0.35)))
                        .padding(2).padding(.top, inProgrammer ? 4 : 0)
                }
            }
            .overlay(
                RoundedRectangle(cornerRadius: radius)
                    .strokeBorder(isTarget ? Color.primary : Color.secondary.opacity(0.4), lineWidth: isTarget ? 3 : hairline)
            )
            .clipShape(RoundedRectangle(cornerRadius: radius))
        }
        .buttonStyle(.plain)
    }
}

// MARK: - Groups window (IPC-15)

struct GroupsWindow: View {
    @EnvironmentObject private var engine: EngineClient
    /// While armed, the next group tap records instead of selecting.
    @State private var recordArmed = false

    var body: some View {
        let byId = Dictionary(engine.groups.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
        // Empty cells up to 16 (or one past the last group) so Record has a target.
        let ids = Array(1...max(16, (engine.groups.map(\.id).max() ?? 0) + 1))

        PanelWindow(title: "Groups", accessory: {
            Button("Record") { recordArmed.toggle() }
                .buttonStyle(FlatButton(fill: recordArmed ? .red : nil))
                .controlSize(.small)
        }) {
            ScrollView {
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 56, maximum: 80), spacing: 4)], spacing: 4) {
                    ForEach(ids, id: \.self) { id in
                        GroupButton(id: id, count: byId[id]?.fixtures.count, armed: recordArmed) {
                            if recordArmed {
                                engine.recordGroup(id)
                                recordArmed = false
                            } else if byId[id] != nil {
                                engine.selectGroup(id)
                            }
                        }
                    }
                }
                .padding(6)
            }
        }
    }
}

private struct GroupButton: View {
    let id: Int
    let count: Int?   // nil = empty cell
    let armed: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            VStack(spacing: 1) {
                Text("\(id)").font(.title3.weight(.semibold).monospacedDigit())
                Text(count.map { "\($0) fix" } ?? " ").font(.caption2).foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity).frame(height: 46)
            .background(count == nil ? Color.clear : Color.secondary.opacity(0.12))
            .contentShape(Rectangle())
            .overlay(
                RoundedRectangle(cornerRadius: radius)
                    .strokeBorder(armed ? Color.red : Color.secondary.opacity(count == nil ? 0.2 : 0.4),
                                  lineWidth: armed ? 2 : hairline)
            )
            .opacity(count == nil && !armed ? 0.6 : 1)
        }
        .buttonStyle(.plain)
    }
}

// MARK: - Attribute bar (IPC-16, IPC-17)

struct AttributeBar: View {
    @EnvironmentObject private var engine: EngineClient
    @State private var command = ""

    var body: some View {
        let programmer = engine.programmer
        let dirty = !programmer.selection.isEmpty || !programmer.inProgrammer.isEmpty

        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 4) {
                ForEach(programmer.attributeGroups) { tab in
                    Button(tab.group) { engine.setAttributeGroup(tab.group) }
                        .buttonStyle(FlatButton(fill: tab.group == programmer.activeGroup ? .blue : nil))
                        .disabled(!tab.available)
                        .opacity(tab.available ? 1 : 0.35)
                        .overlay(alignment: .topTrailing) {
                            if tab.hasValues { Circle().fill(Color.red).frame(width: 6, height: 6).offset(x: -2, y: 2) }
                        }
                }
                Spacer()
                Button("Fix −1") { engine.step(-1) }.buttonStyle(FlatButton())
                Button("Fix +1") { engine.step(1) }.buttonStyle(FlatButton())
                Button("Odd/Even") { engine.pattern(2) }.buttonStyle(FlatButton())
                Button("All") { engine.selectAll() }.buttonStyle(FlatButton())
                Button("Clear") { engine.clearProgrammer() }.buttonStyle(FlatButton(fill: dirty ? .red : nil))
            }

            HStack(spacing: 4) {
                if programmer.activeGroup == "C" {
                    // IPC-19: wire values are the engine's ColourModel names.
                    ForEach([("HSI", "Hsi"), ("RGB", "Rgb"), ("CMY", "Cmy")], id: \.1) { label, model in
                        Button(label) { engine.setColourModel(model) }
                            .buttonStyle(FlatButton(fill: programmer.colourModel == model ? .blue : nil))
                    }
                    Divider().frame(height: 30)
                }
                ForEach(programmer.wheels) { wheel in
                    WheelView(wheel: wheel)
                }
                if programmer.wheels.isEmpty {
                    Text("No wheels").font(.caption).foregroundStyle(.secondary)
                        .frame(height: 44)
                }
                Spacer()
            }

            HStack(spacing: 8) {
                // Only keyboard-driven control in the app: no global key handlers (IPC-17).
                TextField("Command, e.g. 201 Thro 205 @ 50", text: $command)
                    .textFieldStyle(.roundedBorder)
                    .font(.body.monospaced())
                    .onSubmit {
                        let words = command.split(whereSeparator: \.isWhitespace).map(String.init)
                        if !words.isEmpty { engine.command(words: words) }
                        command = ""
                    }
                if let message = engine.lastMessage {
                    Text(message).font(.caption).foregroundStyle(.red).lineLimit(2)
                        .frame(maxWidth: 320, alignment: .leading)
                }
            }
        }
        .padding(8)
    }
}

/// One wheel: name, value %, drag or scroll to send `programmer.wheel`.
private struct WheelView: View {
    @EnvironmentObject private var engine: EngineClient
    let wheel: WheelState
    /// Whole steps already sent during the current drag (remainder stays in the translation).
    @State private var sentSteps = 0

    /// 1 step per 4 pt of vertical movement.
    private let pointsPerStep: CGFloat = 4

    var body: some View {
        VStack(spacing: 1) {
            Text(wheel.attribute).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
            Text(wheel.attribute == "Hue" ? String(format: "%.0f°", wheel.value * 360)
                 : wheel.sixteenBit && engine.fineUsed.contains(wheel.attribute)
                 ? String(format: "%.1f %%", wheel.value * 100) : percent(wheel.value))
                .font(.callout.monospacedDigit())
                .foregroundStyle(wheel.touched ? Color.red : Color.primary)
        }
        .frame(width: 76, height: 44)
        .background(RoundedRectangle(cornerRadius: radius).fill(Color.secondary.opacity(0.12)))
        .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(Color.secondary.opacity(0.4), lineWidth: hairline))
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
