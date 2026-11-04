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
        if end == "Enter", let n = onlyNumber, ui.liveTimeCells(engine) != nil {   // UI-51: Enter is Set
            ui.setTimes(engine, n)
            clear()
            return
        }
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

    /// UI-44: Esc, whatever the Keys toggle says: leaves a text field, else Exit (line and modes), else the screen, else Back to List.
    @MainActor
    private static func escape(_ event: NSEvent, ui: UIState, line: TokenLine) -> Bool {
        // A sheet, dialog, alert or panel has the key window and takes Esc itself.
        guard let window = NSApp.keyWindow, window.isMainWindow else { return false }
        if event.isARepeat { return true }   // a held key counts once
        if window.firstResponder is NSText { ui.fieldEscaped = true; window.makeFirstResponder(nil); return true }
        // The keypad's Exit; a line hidden behind a screen does not count.
        if ui.modesOpen || (ui.screen == nil && !line.tokens.isEmpty) { line.clear(); ui.cancelModes(); return true }
        // Patch and Control have their own cancelAction (a nested level, the profile editor or deck focus goes first).
        if ui.screen == .patch || ui.screen == .control { return false }
        if ui.screen != nil { ui.screen = nil; return true }
        if ui.cueView != nil, Workspace.all[safe: ui.workspace]?.windows.contains(.cues) == true {
            ui.cueView = nil; ui.cueValue = nil; ui.cueRemoveConfirm = false   // same as "Back to List"
        } else if Workspace.all[safe: ui.workspace]?.windows.contains(.cues) == true {
            // UI-51: the selected time cells go first, then the Times view.
            if ui.timeCells != nil { ui.timeCells = nil } else if ui.cueTimesView { ui.cueTimesView = false }
        }
        return true
    }

    /// The command-line token a character types, or nil when Keys ignores it (UI-49: one list for typing and for the Keys-off hint).
    private static func token(for c: String) -> String? {
        guard c.count == 1 else { return nil }
        switch c {
        case "0"..."9", ".", "@": return c
        case "-": return "Thro"
        case "+", "=": return "And"
        case "f": return "Fixture"
        case "g": return "Group"
        case "l": return "Lee"   // UI-19, UI-32
        case "p": return "Palette"
        default: return nil
        }
    }

    /// UI-49: whether Keys would act on this key (F1-F5 and Space anywhere; the command-line keys on the workspace).
    private static func keysAct(on event: NSEvent, workspace: Bool) -> Bool {
        if workspaceKeys.contains(event.keyCode) || event.keyCode == 49 { return true }
        guard workspace else { return false }
        if [36, 76, 51, 126, 125].contains(event.keyCode) { return true }
        return event.characters.flatMap { token(for: $0.lowercased()) } != nil
    }

    @MainActor
    static func handle(_ event: NSEvent, ui: UIState, line: TokenLine, engine: EngineClient) -> Bool {
        ui.fieldEscaped = false   // UI-44: any key clears it
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
        if event.keyCode == 53, chord.isEmpty { return escape(event, ui: ui, line: line) }
        // A shortcut chord, or a text field (Patch, Show) with focus: not ours.
        guard event.modifierFlags.isDisjoint(with: [.command, .control, .option]),
              !(NSApp.keyWindow?.firstResponder is NSText)
        else { return false }
        // UI-49: Keys off, a key Keys would act on, main window up (a sheet's window is key but not main): say so once, do nothing.
        guard UserDefaults.standard.bool(forKey: "keysLive") else {
            guard NSApp.keyWindow?.isMainWindow == true, keysAct(on: event, workspace: ui.screen == nil) else { return false }
            if !event.isARepeat { engine.sayKeysOff() }
            return true
        }

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
        case 51: line.back()                // Backspace
        case 126: line.older()              // Up
        case 125: line.newer()              // Down
        default:
            guard let c = event.characters?.lowercased(), let token = token(for: c) else { return false }
            if c == "p", line.tokens.isEmpty { ui.paletteMenu = true }
            line.add(token)
        }
        return true
    }
}
