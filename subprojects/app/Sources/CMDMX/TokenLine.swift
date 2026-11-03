import AppKit
import SwiftUI

// The token command line (UI-19) and its history (UI-27). The keypad, the Mac keys and the
// attribute bar share one instance. The app does not check the grammar: the engine parses the
// tokens and refuses with a warning (IPC-17).

private func isNumeric(_ token: String) -> Bool {
    token.first.map { $0.isNumber || $0 == "." } ?? false
}

@MainActor
final class TokenLine: ObservableObject {
    @Published private(set) var tokens: [String] = []
    /// Sent lines as space-joined tokens, oldest first, kept in the app's preferences.
    private var history = UserDefaults.standard.stringArray(forKey: "commandHistory") ?? []
    /// Index into `history` while browsing with Up / Down; nil = a line of its own.
    private var recall: Int?

    /// The line holds one number and nothing else: the fade time of a palette tap (PAL-28).
    var onlyNumber: Double? {
        tokens.count == 1 && isNumeric(tokens[0]) ? Double(tokens[0]) : nil
    }

    /// The number on the line (alone, or after `Palette`), for Master Time, Master Overlap and Set Overlap.
    var paletteArgument: Double? {
        if tokens.count == 1 { return onlyNumber }
        return tokens.count == 2 && tokens[0] == "Palette" && isNumeric(tokens[1]) ? Double(tokens[1]) : nil
    }

    /// Digits and `.` join the number being typed; any other token stands alone.
    func add(_ token: String) {
        recall = nil
        if isNumeric(token), let last = tokens.last, isNumeric(last) {
            tokens[tokens.count - 1] = last + token
        } else {
            tokens.append(token)
        }
    }

    /// Last character of a number, else the last token.
    func back() {
        recall = nil
        guard let last = tokens.last else { return }
        if isNumeric(last), last.count > 1 {
            tokens[tokens.count - 1] = String(last.dropLast())
        } else {
            tokens.removeLast()
        }
    }

    /// Clear key (UI-26): a number alone on the line is the Clear time, and the line empties.
    func clearProgrammer(_ engine: EngineClient) {
        let time = onlyNumber
        engine.clearProgrammer(time: time)
        if time != nil { clear() }
    }

    func clear() {
        recall = nil
        tokens = []
    }

    /// Enter: sends the tokens, records the line unless it equals the newest entry, empties the line.
    /// `end` is "Locate" for a selection line that locates instead (UI-26).
    func send(_ engine: EngineClient, end: String = "Enter") {
        guard !tokens.isEmpty else { return }
        engine.command(words: tokens, end: end)
        remember()
        clear()
    }

    private func remember() {
        let line = tokens.joined(separator: " ")
        if history.last != line {
            history = Array((history + [line]).suffix(100))
            UserDefaults.standard.set(history, forKey: "commandHistory")
        }
    }

    /// UI-46: Enter, and Locate on a non-empty line. An empty line confirms a waiting Delete. While Record or a
    /// function is armed, exactly `Group n` or `Palette n` is a tap on that cell; any other line is refused.
    func enter(_ engine: EngineClient, ui: UIState, end: String = "Enter") {
        if tokens.isEmpty {
            if end == "Enter" { ui.confirmDelete(engine) }
            ui.paletteMenu = false
            return
        }
        guard ui.recordArmed || ui.function != nil else {
            send(engine, end: end)
            ui.paletteMenu = false
            return
        }
        if end == "Enter", tokens.count == 2, let n = Int(tokens[1]), n >= 1, tokens[0] == "Group" || tokens[0] == "Palette" {
            if tokens[0] == "Group" {
                guard ui.armedAllows(.group, engine) else { return }
                finishTap(ui)
                GroupTaps.tap(n, engine.groups.first { $0.id == n }, engine: engine, ui: ui)
            } else {
                guard let p = engine.palettes.first(where: { $0.number == n }) else {
                    return engine.say(ui.refusal(on: .palette) ?? "Palette \(n) is empty: tap a cell in a palette window")
                }
                guard ui.armedAllows(.palette, engine) else { return }
                finishTap(ui)
                PaletteTaps.armed(window: p.window, handle: p.handle, p, engine: engine, ui: ui)
            }
            return
        }
        let name = ui.recordArmed ? "Record" : (ui.function?.rawValue ?? "")
        engine.say("\(name): type Group n or Palette n, or tap a target.")
    }

    private func finishTap(_ ui: UIState) {
        remember()
        clear()
        ui.paletteMenu = false
    }

    /// Up: the previous entry.
    func older() {
        guard !history.isEmpty else { return }
        show(max(0, (recall ?? history.count) - 1))
    }

    /// Down: the next entry; past the newest, an empty line.
    func newer() {
        guard let recall else { return }
        if recall + 1 < history.count { show(recall + 1) } else { clear() }
    }

    private func show(_ index: Int) {
        recall = index
        tokens = history[index].split(separator: " ").map(String.init)
    }
}

/// The Mac keyboard as keypad (UI-19). Called from a key-down monitor; true = key consumed.
enum MacKeys {
    private static let workspaceKeys: [UInt16] = [122, 120, 99, 118, 96]   // F1 F2 F3 F4 F5

    @MainActor
    static func handle(_ event: NSEvent, ui: UIState, line: TokenLine, engine: EngineClient) -> Bool {
        let chord = event.modifierFlags.intersection([.command, .shift, .control, .option])
        // UI-38: the prompt and the cover take digits, Delete and Return, whatever the Keys toggle says; nothing else reaches the console. ⌘ shortcuts go on to the menu, so ⌘Q reaches the refusal.
        if engine.locked || ui.lockPrompt {
            guard event.modifierFlags.isDisjoint(with: [.command, .control, .option]) else { return false }
            switch event.keyCode {
            case 36, 76: ui.enterLockCode(engine)
            case 51: _ = ui.lockCode.popLast()
            case 53: if ui.lockPrompt { ui.lockPrompt = false; ui.lockCode = "" }
            default:
                if let c = event.characters, c.count == 1, "0"..."9" ~= c { ui.lockCode += c; engine.clearMessage() }
            }
            return true
        }
        // UNDO-18: ⌘Z / ⇧⌘Z, whatever the Keys toggle says; a text field keeps its own undo.
        if (chord == .command || chord == [.command, .shift]), event.charactersIgnoringModifiers?.lowercased() == "z",
           !(NSApp.keyWindow?.firstResponder is NSText) {
            if chord == .command { engine.undo() } else { engine.redo() }
            return true
        }
        // UI-39: with the Messages sheet open the keypad stays out of it, so Esc reaches the sheet.
        if ui.messagesOpen { return false }
        // CTL-42: Esc ends Learn mode (and cancels an armed Learn), whatever the Keys toggle says.
        // Also from a text field: Esc there only drops the edit.
        if event.keyCode == 53, chord.isEmpty, ui.learnMode || engine.control.learn != nil {
            ui.setLearnMode(false, engine)
            return true
        }
        // Keys off, a shortcut chord, or a text field (Patch, Show) with focus: not ours.
        guard UserDefaults.standard.bool(forKey: "keysLive"),
              event.modifierFlags.isDisjoint(with: [.command, .control, .option]),
              !(NSApp.keyWindow?.firstResponder is NSText)
        else { return false }

        if let index = workspaceKeys.firstIndex(of: event.keyCode) {
            ui.show(workspace: index)
            return true
        }
        // CUE-47: Space = Go, Shift+Space = Back on the connected list; a held key fires once.
        if event.keyCode == 49 {
            if !event.isARepeat { engine.cuelist(event.modifierFlags.contains(.shift) ? "back" : "go") }
            return true
        }
        // The command line is only on the workspace screens.
        guard ui.screen == nil else { return false }

        switch event.keyCode {
        // Return, keypad Enter (UI-46); a held key fires once, so it cannot run on into a waiting Delete's confirmation.
        case 36, 76: if !event.isARepeat { line.enter(engine, ui: ui) }
        case 53: line.clear(); ui.cancelModes()                  // Esc = Exit
        case 51: line.back()                // Backspace
        case 126: line.older()              // Up
        case 125: line.newer()              // Down
        default:
            guard let c = event.characters?.lowercased(), c.count == 1 else { return false }
            switch c {
            case "0"..."9", ".": line.add(c)
            case "-": line.add("Thro")
            case "+", "=": line.add("And")
            case "@": line.add("@")
            case "f": line.add("Fixture")
            case "g": line.add("Group")
            case "l": line.add("Lee")   // UI-19, UI-32
            case "p":
                if line.tokens.isEmpty { ui.paletteMenu = true }
                line.add("Palette")
            default: return false
            }
        }
        return true
    }
}
