import AppKit
import SwiftUI

/// UI-86: Titan's keys on ⌥ (and Page Up / Down). Each key does exactly what its screen button does, because the
/// screen buttons call the same code below. MacKeys (key monitor) and the Console menu both come to `fire`.
enum ConsoleKey: Hashable {
    case clear, record, update, include, delete, copy, move, release, unfold, locate, go
    case softkey(Int)   // 0...6 = A...G
    case fixPrev, fixNext, all, highlight, pageUp, pageDown

    static let every: [ConsoleKey] = [.clear, .record, .update, .include, .delete, .copy, .move, .release, .unfold, .locate, .go]
        + (0..<7).map { .softkey($0) } + [.fixPrev, .fixNext, .all, .highlight, .pageUp, .pageDown]

    /// Key equivalent: key, ⇧, ⌥ (Page Up / Down have none), and the key code for the keys that are not letters or digits.
    private var spec: (key: KeyEquivalent, shift: Bool, option: Bool, code: UInt16?) {
        switch self {
        case .clear: return ("c", false, true, nil)
        case .record: return ("r", false, true, nil)
        case .update: return ("u", true, true, nil)
        case .include: return ("i", false, true, nil)
        case .delete: return ("d", false, true, nil)
        case .copy: return ("c", true, true, nil)
        case .move: return ("m", false, true, nil)
        case .release: return ("r", true, true, nil)
        case .unfold: return ("u", false, true, nil)
        case .locate: return ("l", false, true, nil)
        case .go: return ("g", false, true, nil)
        case .softkey(let i): return (KeyEquivalent(Character(String(i + 1))), false, true, nil)
        case .fixPrev: return (.leftArrow, false, true, 123)
        case .fixNext: return (.rightArrow, false, true, 124)
        case .all: return (.upArrow, false, true, 126)
        case .highlight: return (.downArrow, false, true, 125)
        case .pageUp: return (.pageUp, false, false, 116)
        case .pageDown: return (.pageDown, false, false, 121)
        }
    }

    var shortcut: KeyboardShortcut {
        let s = spec
        var mods: EventModifiers = []
        if s.option { mods.insert(.option) }
        if s.shift { mods.insert(.shift) }
        return KeyboardShortcut(s.key, modifiers: mods)
    }

    /// The Console menu title; a softkey carries what it shows now.
    @MainActor func title(_ ui: UIState) -> String {
        switch self {
        case .clear: return "Clear"
        case .record: return "Record"
        case .update: return "Update"
        case .include: return "Include"
        case .delete: return "Delete"
        case .copy: return "Copy"
        case .move: return "Move"
        case .release: return "Release"
        case .unfold: return "Unfold"
        case .locate: return "Locate"
        case .go: return "Go"
        case .softkey(let i):
            let letter = String(UnicodeScalar(UInt8(65 + i)))
            if let name = ui.softkeyNames[safe: i] ?? nil { return "Softkey \(letter): \(name)" }
            return "Softkey \(letter)"
        case .fixPrev: return "Fix −1"
        case .fixNext: return "Fix +1"
        case .all: return "All"
        case .highlight: return "Highlight"
        case .pageUp: return "Previous Page"
        case .pageDown: return "Next Page"
        }
    }

    /// Whether the screen would offer the button now (the menu greys it out otherwise).
    @MainActor func enabled(_ ui: UIState) -> Bool {
        switch self {
        case .go, .pageUp, .pageDown: return true   // the playback area is on every screen; the engine refuses what does not apply
        case .softkey(let i): return ui.screen == nil && (ui.softkeyNames[safe: i] ?? nil) != nil
        default: return ui.screen == nil   // the side column and the attribute bar are hidden on a full screen
        }
    }

    /// The event as one of our keys, or nil. ⌥ keys by the layout letter; arrows and pages by key code.
    static func matching(_ event: NSEvent) -> ConsoleKey? {
        let chord = event.modifierFlags.intersection([.command, .shift, .control, .option])
        let letter = event.charactersIgnoringModifiers?.lowercased()
        return every.first { key in
            let s = key.spec
            guard chord == (s.option ? (s.shift ? [.option, .shift] : [.option]) : []) else { return false }
            if let code = s.code { return event.keyCode == code }
            return letter == String(s.key.character)
        }
    }

    /// The one entry for the keyboard and the menu: nothing under a sheet or the lock, nothing the screen would not offer.
    @MainActor
    func fire(ui: UIState, line: TokenLine, engine: EngineClient) {
        guard !engine.locked, !ui.lockPrompt, NSApp.mainWindow?.attachedSheet == nil, enabled(ui) else { return }
        perform(ui: ui, line: line, engine: engine)
    }

    /// What the screen button does, arming and refusals included.
    @MainActor
    func perform(ui: UIState, line: TokenLine, engine: EngineClient) {
        switch self {
        case .record: Self.press(function: "Record", ui: ui, engine: engine)
        case .update: Self.press(function: "Update", ui: ui, engine: engine)
        case .include: Self.press(function: "Include", ui: ui, engine: engine)
        case .delete: Self.press(function: "Delete", ui: ui, engine: engine)
        case .copy: Self.press(function: "Copy", ui: ui, engine: engine)
        case .move: Self.press(function: "Move", ui: ui, engine: engine)
        case .release: Self.press(function: "Release", ui: ui, engine: engine)
        case .unfold: Self.press(function: "Unfold", ui: ui, engine: engine)
        case .clear:
            ui.learnOrRun("key clear", engine) {
                line.clearProgrammer(engine); if ui.building == nil { ui.cancelModes() }; ui.pendingOverlap = nil
            }
        case .locate:   // UI-26: an empty line locates everything, else the line ends in Locate
            ui.learnOrRun("key locate", engine) {
                if line.tokens.isEmpty { engine.locate() } else { line.enter(engine, ui: ui, end: "Locate") }
                ui.paletteMenu = false
            }
        case .highlight: ui.learnOrRun("key highlight", engine) { engine.setHighlight(!engine.programmer.highlight) }
        case .fixPrev: ui.learnOrRun("key step_prev", engine) { engine.step(-1) }
        case .fixNext: ui.learnOrRun("key step_next", engine) { engine.step(1) }
        case .all:   // the All softkey exists with a selection only
            guard !engine.programmer.selection.isEmpty else { return }
            ui.learnOrRun("key all", engine) { engine.selectAll() }
        case .go:
            guard ui.learnMode || engine.connectedList?.live.cue?.next != nil else { return }
            Self.runGo(ui: ui, engine: engine)
        case .softkey(let i): ui.softkeyPress.send(i)
        case .pageUp: engine.setPage(engine.page.number - 1)   // the engine refuses at the ends, as page.set does
        case .pageDown: engine.setPage(engine.page.number + 1)
        }
    }

    /// The Go button on the connected list.
    @MainActor
    static func runGo(ui: UIState, engine: EngineClient) {
        ui.learnOrRun("go", engine) { engine.cuelist("go") }
    }

    /// What an armed palette function waits for (the sketch's MODE_HINT).
    private static let hints = [
        "Update": "Update: tick targets on the softkeys and Store, or tap the palette, playback handle or cue to update with what is in the programmer.",
        "Include": "Include: tap a palette, playback handle or cue; its values go into the programmer.",
        "Delete": "Delete: tap the group, palette, playback handle or cue to delete (⇧ range, ⌘ add one). You confirm on softkey A.",
        "Copy": "Copy: tap the source group, palette or playback handle (⇧ range, ⌘ add one), then an empty cell or handle; or tap a cue.",
        "Move": "Move: tap the source group, palette, playback handle or fixture (⇧ range, ⌘ add one), then an empty cell, handle or fixture button; or tap a cue.",
        "Release": "Release: tap a handle or an active row to release it, Release again for all, or a page button for a page.",
        "Options": "Options: tap a playback handle or cue to open its options.",
        "Unfold": "Unfold: tap a fixture to unfold or fold its cells.",
        "Latch": "Latch: now press Flash on a handle to latch it.",
    ]

    /// The Record key and the palette function keys of the Functions column (UI-16), by their label.
    @MainActor
    static func press(function label: String, ui: UIState, engine: EngineClient) {
        if label == "Record" {
            // The next group or palette tap records. In the Mask softkeys, Record steps back first.
            if ui.maskMenu { ui.maskMenu = false } else if ui.recordArmed { ui.recordArmed = false } else {
                ui.cancelModes(); engine.endArmedMessage()   // UI-50
                ui.recordArmed = true
            }
        } else if let function = PaletteFunction(rawValue: label) {   // PAL-29
            if function == .release, ui.function == .release {   // UI-66: Release twice releases all
                engine.releaseScope("all"); ui.cancelModes(); return
            }
            engine.endArmedMessage()   // UI-50: the old function's hint goes, the new one is said below
            ui.arm(function)
            if ui.function != nil, let hint = hints[label] { engine.say(hint) }
        }
    }
}

/// The Console menu (UI-86): every ⌥ key with its key equivalent, so they can be found. It watches `UIState` and
/// nothing else; the engine is a plain reference read inside the actions (UI-70).
struct ConsoleCommands: Commands {
    @ObservedObject var ui: UIState
    let line: TokenLine
    let engine: EngineClient

    var body: some Commands {
        CommandMenu("Console") {
            ForEach(ConsoleKey.every, id: \.self) { key in
                Button(key.title(ui)) { key.fire(ui: ui, line: line, engine: engine) }
                    .keyboardShortcut(key.shortcut)
                    .disabled(!key.enabled(ui))
            }
            Divider()
            Button("Show engine log") { DaemonLauncher.shared.revealLog() }   // REL-30
        }
    }
}

/// UI-84, IPC-64: what a surface (BCF2000, ...) does in the app. Dropped under the Lock, the Lock prompt and a sheet.
enum SurfaceKeys {
    private static let functions = [
        "record": "Record", "update": "Update", "include": "Include", "delete": "Delete", "copy": "Copy",
        "move": "Move", "release": "Release", "legend": "Set Legend", "options": "Options",
    ]

    /// The name `surface.armed` carries: Record, else the armed function; Latch is not a surface key (nil).
    @MainActor
    static func armedName(_ ui: UIState) -> String? {
        if ui.recordArmed { return "record" }
        switch ui.function {
        case .update: return "update"
        case .include: return "include"
        case .copy: return "copy"
        case .move: return "move"
        case .delete: return "delete"
        case .legend: return "legend"
        case .release: return "release"
        case .options: return "options"
        case .unfold: return "unfold"
        case .latch, nil: return nil
        }
    }

    @MainActor
    static func handle(_ e: EngineClient.SurfaceEvent, ui: UIState, line: TokenLine, engine: EngineClient) {
        guard !engine.locked, !ui.lockPrompt, NSApp.mainWindow?.attachedSheet == nil else { return }
        if let token = e.keypad {
            // Like the Mac keys: the line is on the workspace screens; Exit always clears and disarms.
            guard ui.screen == nil || token == "Exit" else { return }
            Keypad.run(token == "Backspace" ? "Back" : token, ui: ui, line: line, engine: engine, device: e.device)
        } else if let name = e.key.flatMap({ functions[$0] }) {
            guard ui.screen == nil else { return }
            ConsoleKey.press(function: name, ui: ui, engine: engine)
        } else if let tap = e.tap {
            guard ui.recordArmed || ui.function != nil else { return }
            // The playback stored there, else a Locked one of another page on that slot (PBK-07).
            let info = engine.playbacks.first { $0.stored.slot == tap.slot && $0.stored.page == tap.page }
                ?? engine.playbacks.first { $0.stored.slot == tap.slot && $0.stored.options.paging == "Locked" }
            HandleTaps.tap(slot: tap.slot, info: info, engine: engine, ui: ui, line: line, page: tap.page)
        } else if let n = e.group ?? e.palette {
            guard ui.recordArmed || ui.function != nil else { return }
            _ = TokenLine.armedTap(e.group != nil ? "Group" : "Palette", n, engine: engine, ui: ui)
        }
    }
}
