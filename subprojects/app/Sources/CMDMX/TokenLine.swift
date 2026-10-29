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
        let line = tokens.joined(separator: " ")
        if history.last != line {
            history = Array((history + [line]).suffix(100))
            UserDefaults.standard.set(history, forKey: "commandHistory")
        }
        clear()
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
        // UNDO-18: ⌘Z / ⇧⌘Z, whatever the Keys toggle says; a text field keeps its own undo.
        let chord = event.modifierFlags.intersection([.command, .shift, .control, .option])
        if (chord == .command || chord == [.command, .shift]), event.charactersIgnoringModifiers?.lowercased() == "z",
           !(NSApp.keyWindow?.firstResponder is NSText) {
            if chord == .command { engine.undo() } else { engine.redo() }
            return true
        }
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
        case 36, 76: line.send(engine); ui.paletteMenu = false   // Return, keypad Enter
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
