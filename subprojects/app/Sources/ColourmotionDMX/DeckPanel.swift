import AppKit
import SwiftUI

// The Control screen's on-screen Stream Deck (CTL-58): every key is painted from the look the engine
// reports (CTL-57), never computed here. Linking is by drag and drop or the right-click menu; each
// edit is a `control.binding` / `control.unbind` with the deck page shown (CTL-67).

struct DeckPanel: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    let device: ControlDevice
    /// Deselects the deck (CTL-58a): the Devices and OSC panels come back.
    let onClose: () -> Void
    /// The empty page `deckPages + 1` the `+` shows; the deck itself stays on its page until a binding lands there.
    @State private var virtualPage = false
    @State private var brightness = 100.0

    private var shownPage: Int { virtualPage ? device.deckPages + 1 : device.deckPage }

    var body: some View {
        let cols = max(1, device.cols)
        let size = min(96, floor((500 - 4 * CGFloat(cols - 1)) / CGFloat(cols)))
        PanelWindow(title: "Deck", info: device.name, framed: true, accessory: {
            if device.state != .ok { Text(device.state.word).font(Typo.ui(11)).foregroundStyle(t.amber) }
            Button("‹ Devices", action: onClose)
                .buttonStyle(.plain).font(Typo.ui(11)).foregroundStyle(t.fg2)
                .keyboardShortcut(.cancelAction)
        }) {
            VStack(alignment: .leading, spacing: 8) {
                VStack(spacing: 4) {
                    ForEach(0..<device.rows, id: \.self) { row in
                        HStack(spacing: 4) {
                            ForEach(0..<cols, id: \.self) { col in
                                tile(row * cols + col + 1, size: size)
                            }
                        }
                    }
                }
                pageSelector
                HStack(spacing: 8) {
                    Text("Brightness").font(Typo.ui(Typo.small)).foregroundStyle(t.fg2)
                    Slider(value: $brightness, in: 0...100) { editing in
                        if !editing { engine.controlDeck(device.name, brightness: Int(brightness.rounded())) }
                    }
                    Text("\(Int(brightness.rounded()))").font(Typo.mono(Typo.small)).foregroundStyle(t.fg).frame(width: 28, alignment: .trailing)
                }
            }
            .padding(8)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .fixedSize(horizontal: false, vertical: true)
        .onAppear { brightness = Double(device.brightness) }
        .onChange(of: device.brightness) { _, new in brightness = Double(new) }
        // The deck followed (or was moved): the screen shows its page again.
        .onChange(of: device.deckPage) { _, _ in virtualPage = false }
        .onChange(of: device.name) { _, _ in virtualPage = false }
    }

    private var pageSelector: some View {
        HStack(spacing: 4) {
            Text("Deck page").font(Typo.ui(Typo.small)).foregroundStyle(t.fg2)
            ForEach(1...max(1, device.deckPages), id: \.self) { n in
                Button(String(n)) {
                    virtualPage = false
                    engine.controlDeck(device.name, page: n)
                }
                .buttonStyle(FlatButton(fill: !virtualPage && n == device.deckPage ? .blue : nil, compact: true))
            }
            Button("+") { virtualPage = true }
                .buttonStyle(FlatButton(fill: virtualPage ? .blue : nil, compact: true))
                .help("An empty page after the last one; the first key linked there makes it real")
        }
    }

    // MARK: Keys

    private var doc: ControlMappingDoc? {
        device.mapping.flatMap { name in engine.controlMapping?.name == name ? engine.controlMapping : nil }
    }

    private func binding(_ key: Int, page: Int) -> ControlBinding? {
        doc?.bindings.first { $0.deckKey == key && $0.deckPage == page }
    }

    private func keyInput(_ key: Int) -> [String: Any] { ["kind": "key", "number": key] }

    @ViewBuilder private func tile(_ key: Int, size: CGFloat) -> some View {
        let page = shownPage
        let bound = binding(key, page: page)
        let look = virtualPage ? nil : device.keys.first { $0.key == key }
        DeckKeyTile(look: look, number: key, size: size)
            .deckDrag(bound != nil ? "deckkey:\(key)" : nil)
            .onDrop(of: [.plainText], isTargeted: nil) { providers, _ in
                guard let provider = providers.first else { return false }
                // The Option key is read now: the drop is where the operator lets go.
                let option = NSEvent.modifierFlags.contains(.option)
                _ = provider.loadObject(ofClass: NSString.self) { object, _ in
                    guard let text = object as? NSString else { return }
                    let payload = String(text)
                    Task { @MainActor in drop(payload, on: key, option: option) }
                }
                return true
            }
            .contextMenu { menu(key, bound: bound) }
            .onTapGesture { learnHere(key, bound: bound) }
    }

    /// CTL-58: with Learn armed on an action, a click on a key links that action to it.
    private func learnHere(_ key: Int, bound: ControlBinding?) {
        guard let action = engine.control.learn else { return }
        setAction(key, action, bound: bound)
        engine.controlLearnCancel()
        engine.say("Learned: \(device.name) key \(key) → \(action)")
    }

    /// Where a link is stored: the device's mapping, or a hint when it has none.
    private func target() -> String? {
        if let name = device.mapping { return name }
        engine.say("Assign a mapping to \(device.name) first")
        return nil
    }

    /// The deck follows a binding made on the `+` page, which is then a real one.
    private func followIfVirtual() {
        if virtualPage { engine.controlDeck(device.name, page: shownPage) }
    }

    private func drop(_ payload: String, on key: Int, option: Bool) {
        guard !payload.isEmpty, let mapping = target() else { return }
        let page = shownPage
        if let from = payload.hasPrefix("deckkey:") ? Int(payload.dropFirst(8)) : nil {
            move(from, to: key, page: page, mapping: mapping)
            return
        }
        var action = payload
        if option, action.hasPrefix("upper ") { action = "upper \(engine.page.number)." + action.dropFirst(6) }
        let old = binding(key, page: page)
        if old != nil { engine.controlUnbind(mapping, input: keyInput(key), deckPage: page) }
        engine.controlBinding(mapping, input: keyInput(key), ["deck_page": page, "action": action])
        followIfVirtual()
        if let old { engine.say("key \(key) was \(old.action)") }
    }

    /// A key onto another key: the whole binding moves; a bound target swaps places with it.
    private func move(_ from: Int, to key: Int, page: Int, mapping: String) {
        guard from != key, let source = binding(from, page: page) else { return }
        let other = binding(key, page: page)
        engine.controlUnbind(mapping, input: keyInput(from), deckPage: page)
        if other != nil { engine.controlUnbind(mapping, input: keyInput(key), deckPage: page) }
        engine.controlBinding(mapping, input: keyInput(key), whole(source, page: page))
        if let other {
            engine.controlBinding(mapping, input: keyInput(from), whole(other, page: page))
        }
        followIfVirtual()
    }

    /// A binding's own fields for another key: its `input` would win over the new one in `controlBinding`.
    private func whole(_ b: ControlBinding, page: Int) -> [String: Any] {
        var fields = b.raw
        fields["input"] = nil
        fields["deck_page"] = page
        return fields
    }

    // MARK: Right-click

    /// The plain actions of the bindings table that make sense on a key.
    private static let actions = ["go", "back", "stop", "release", "page next", "page prev", "gm", "dbo", "deck page next", "deck page prev"]
    private static let colours: [(String, String)] = [
        ("Red", "#ff0000"), ("Orange", "#ff6000"), ("Amber", "#ffb000"), ("Yellow", "#ffff00"), ("Green", "#00ff00"),
        ("Cyan", "#00ffff"), ("Blue", "#0000ff"), ("Magenta", "#ff00ff"), ("White", "#ffffff"),
    ]

    @ViewBuilder private func menu(_ key: Int, bound: ControlBinding?) -> some View {
        Menu("Action") {
            ForEach(Self.actions, id: \.self) { a in Button(a) { setAction(key, a, bound: bound) } }
            ForEach(1...(device.deckPages + 1), id: \.self) { n in Button("deck page \(n)") { setAction(key, "deck page \(n)", bound: bound) } }
            Divider()
            Menu("Group") {
                ForEach(engine.groups) { g in Button("Group \(g.id)") { setAction(key, "group \(g.id)", bound: bound) } }
            }
            Menu("Palette") {
                ForEach(engine.palettes) { p in
                    Button("\(p.number) \(p.legend)") { setAction(key, "palette \(p.number)", bound: bound) }
                }
            }
            Divider()
            Button("Edit action…") {
                if let text = promptText("Action for key \(key)", initial: bound?.action ?? "") { setAction(key, text, bound: bound) }
            }
        }
        if let bound, let mapping = device.mapping {
            Button("Label…") {
                guard let text = promptText("Label for key \(key)", initial: bound.raw["label"] as? String ?? "") else { return }
                change(key, ["label": text.isEmpty ? NSNull() : text as Any])
            }
            Menu("Colour") {
                ForEach(Self.colours, id: \.0) { c in Button(c.0) { change(key, ["colour": c.1]) } }
                Divider()
                Button("None") { change(key, ["colour": NSNull()]) }
            }
            Button("Remove") { engine.controlUnbind(mapping, input: keyInput(key), deckPage: shownPage) }
        }
    }

    private func setAction(_ key: Int, _ action: String, bound: ControlBinding?) {
        let action = action.trimmingCharacters(in: .whitespaces)
        guard !action.isEmpty, let mapping = target() else { return }
        var fields: [String: Any] = ["deck_page": shownPage, "action": action]
        if let bound {
            // CTL-58b: a different action drops the old look (null removes a field); the same one changes nothing.
            if bound.action == action { return }
            fields["label"] = NSNull()
            fields["colour"] = NSNull()
        }
        engine.controlBinding(mapping, input: keyInput(key), fields)
        if bound == nil { followIfVirtual() }
    }

    private func change(_ key: Int, _ fields: [String: Any]) {
        guard let mapping = device.mapping else { return }
        engine.controlBinding(mapping, input: keyInput(key), fields.merging(["deck_page": shownPage]) { a, _ in a })
    }

    private func promptText(_ title: String, initial: String) -> String? {
        let alert = NSAlert()
        alert.messageText = title
        let field = NSTextField(frame: NSRect(x: 0, y: 0, width: 260, height: 24))
        field.stringValue = initial
        alert.accessoryView = field
        alert.addButton(withTitle: "OK")
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = field
        guard alert.runModal() == .alertFirstButtonReturn else { return nil }
        return field.stringValue.trimmingCharacters(in: .whitespaces)
    }
}

/// One key as the engine's look draws it: black, halo bar on top, legend and sub, a level bar below.
private struct DeckKeyTile: View {
    @Environment(\.theme) private var t
    let look: DeckKeyLook?
    let number: Int
    let size: CGFloat

    var body: some View {
        let held = look?.state == .held
        let ink: Color = held ? .black : .white
        ZStack(alignment: .topLeading) {
            (held ? Color.white : Color.black)
            if let look {
                VStack(spacing: 1) {
                    Text(look.label).font(Typo.legend(11, .semibold)).lineLimit(3).minimumScaleFactor(0.6).truncationMode(.tail)
                        .multilineTextAlignment(.center)
                    if !look.sub.isEmpty { Text(look.sub).font(Typo.ui(8)).lineLimit(1).minimumScaleFactor(0.7).opacity(0.7) }
                }
                .foregroundStyle(ink)
                .padding(.horizontal, 3).padding(.top, 7)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)
                // One top bar like the daemon draws it: the level filled, the rest dim.
                let halo = look.halo.flatMap(Color.init(deckHex:))
                if let level = look.level {
                    ZStack(alignment: .leading) {
                        (halo ?? Color(white: 40 / 255)).opacity(halo == nil ? 1 : 0.25)
                        (halo ?? .white).frame(width: size * CGFloat(min(max(level, 0), 1)))
                    }
                    .frame(height: 4)
                } else if let halo {
                    halo.frame(height: 4)
                }
                if look.fixed {
                    Image(systemName: "lock.fill").font(.system(size: 10)).foregroundStyle(.white)
                        .padding(.top, 6).padding(.trailing, 3)
                        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topTrailing)
                }
            }
            Text(String(number)).font(Typo.mono(8)).foregroundStyle(t.fg3).padding(2)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing)
        }
        .opacity(look?.state == .inactive ? 0.4 : 1)
        .frame(width: size, height: size)
        .clipShape(RoundedRectangle(cornerRadius: 4))
        .overlay(RoundedRectangle(cornerRadius: 4).strokeBorder(t.line, lineWidth: 1))
        .contentShape(Rectangle())
    }
}

private extension Color {
    /// `#rrggbb`; nil for anything else.
    init?(deckHex hex: String) {
        guard hex.count == 7, hex.hasPrefix("#"), let v = UInt32(hex.dropFirst(), radix: 16) else { return nil }
        self.init(red: Double(v >> 16 & 255) / 255, green: Double(v >> 8 & 255) / 255, blue: Double(v & 255) / 255)
    }
}

extension View {
    /// CTL-58: dragging this view onto a deck key in the Control screen links `action`; the payload is plain text.
    @ViewBuilder func deckDrag(_ action: String?) -> some View {
        if let action { onDrag { NSItemProvider(object: action as NSString) } } else { self }
    }
}
