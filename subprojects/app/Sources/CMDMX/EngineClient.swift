import AppKit
import Foundation
import Combine

/// Connection state to cmdmx-daemon.
enum EngineConnection: Equatable {
    case offline
    case connecting
    case online
}

/// What the UI knows about one slot on the current page. The engine says which playback a slot
/// holds (PBK-03); level and state mirror its `PlaybackState`. The app never computes these.
struct PlaybackSlot: Identifiable, Equatable {
    let id: Int              // 0-based slot index on the page
    var info: PlaybackInfo?  // nil: an empty slot (PBK-05)
    var playback: Int? { info?.id }
    var level: Double { info?.live.level ?? 0 }
    /// In the merge (green strip, UI-22); a releasing playback is not (amber strip).
    var isActive: Bool { info?.live.state == "active" }
    var isReleasing: Bool { info?.live.state == "releasing" }
}

/// CUE-40: a cue list's or chase's current and next cue (a chase: the step numbers) and the running
/// transition. `nil` for a memory or a master.
struct CueLive: Decodable, Equatable {
    var current: Double?
    var next: Double?
    var progress = 0.0
    var paused = false

    private enum CodingKeys: String, CodingKey { case current, next, progress, paused }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        current = try? c.decode(Double.self, forKey: .current)
        next = try? c.decode(Double.self, forKey: .next)
        progress = ((try? c.decode(Double.self, forKey: .progress)) ?? 0).clamped01
        paused = (try? c.decode(Bool.self, forKey: .paused)) ?? false
    }
}

/// PBK-35: the live part of a playback entry, sent alone in the small `playback` delta.
struct PlaybackLive: Decodable, Equatable {
    var level = 0.0
    /// `off`, `active` or `releasing`.
    var state = "off"
    /// Held or latched.
    var flash = false
    var latched = false
    /// Orders the active playbacks by when they fired; nil while off or releasing.
    var fired: Int?
    /// CUE-40, IPC-28: lists and chases only.
    var cue: CueLive?
    /// The effective BPM of a chase or BPM master.
    var bpm: Double?
    /// CUE-44: the effective multiplier of a BPM master (the fader's zone included).
    var multiplier: Double?
    /// UI-56: a group master that holds its group; the engine decides, the app only reads it.
    var holding: Bool?

    init() {}

    /// UI-71: the level and the cue progress change every tick and are drawn outside SwiftUI; this is the rest.
    var withoutTicks: PlaybackLive {
        var copy = self
        copy.level = 0
        copy.cue?.progress = 0
        return copy
    }

    private enum CodingKeys: String, CodingKey { case level, state, flash, latched, fired, cue, bpm, multiplier, holding }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        level = ((try? c.decode(Double.self, forKey: .level)) ?? 0).clamped01
        state = (try? c.decode(String.self, forKey: .state)) ?? "off"
        flash = (try? c.decode(Bool.self, forKey: .flash)) ?? false
        latched = (try? c.decode(Bool.self, forKey: .latched)) ?? false
        fired = try? c.decode(Int.self, forKey: .fired)
        cue = try? c.decode(CueLive.self, forKey: .cue)
        bpm = try? c.decode(Double.self, forKey: .bpm)
        multiplier = try? c.decode(Double.self, forKey: .multiplier)
        holding = try? c.decode(Bool.self, forKey: .holding)
    }
}

/// CUE-20: how a cue follows the one before it; `Wait`, `After` or `With`, offset in seconds.
struct CueLink: Decodable, Equatable {
    var type = "Wait"
    var offset = 0.0

    init() {}

    private enum CodingKeys: String, CodingKey { case type, offset }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        type = (try? c.decode(String.self, forKey: .type)) ?? "Wait"
        offset = (try? c.decode(Double.self, forKey: .offset)) ?? 0
    }
}

/// CUE-14: delay and fade of one attribute group in a cue.
struct GroupTime: Decodable, Equatable {
    var delay = 0.0
    var fade = 0.0
}

/// CUE-40: a stored cue without its values.
struct StoredCue: Decodable, Identifiable, Equatable {
    var number = 1.0
    var legend = ""
    var delay = 0.0
    var fadeIn = 0.0
    var fadeOut = 0.0
    /// CUE-56: the out times in force, and whether each is As In.
    var delayOut = 0.0
    var fadeOutAsIn = false
    var delayOutAsIn = false
    var overlap = 100.0
    var link = CueLink()
    /// `Track`, `Block`, `CueOnly` or `Solo` (CUE-51).
    var track = "Track"
    var groupTimes: [String: GroupTime] = [:]
    var groups: [String] = []
    var id: Double { number }

    private enum CodingKeys: String, CodingKey {
        case number, legend, delay, overlap, link, track, groups
        case fadeIn = "fade_in", fadeOut = "fade_out", groupTimes = "group_times"
        case delayOut = "delay_out", fadeOutAsIn = "fade_out_as_in", delayOutAsIn = "delay_out_as_in"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        number = try c.decode(Double.self, forKey: .number)
        legend = (try? c.decode(String.self, forKey: .legend)) ?? ""
        delay = (try? c.decode(Double.self, forKey: .delay)) ?? 0
        fadeIn = (try? c.decode(Double.self, forKey: .fadeIn)) ?? 0
        fadeOut = (try? c.decode(Double.self, forKey: .fadeOut)) ?? 0
        delayOut = (try? c.decode(Double.self, forKey: .delayOut)) ?? delay
        fadeOutAsIn = (try? c.decode(Bool.self, forKey: .fadeOutAsIn)) ?? false
        delayOutAsIn = (try? c.decode(Bool.self, forKey: .delayOutAsIn)) ?? false
        overlap = (try? c.decode(Double.self, forKey: .overlap)) ?? 100
        link = (try? c.decode(CueLink.self, forKey: .link)) ?? CueLink()
        track = (try? c.decode(String.self, forKey: .track)) ?? "Track"
        groupTimes = (try? c.decode([String: GroupTime].self, forKey: .groupTimes)) ?? [:]
        groups = (try? c.decode([String].self, forKey: .groups)) ?? []
    }
}

/// CUE-38: the options of a cue list.
struct ListOptions: Decodable, Equatable {
    var tracking = true
    /// `KillWithOff` or `KillAtZero` (CUE-15).
    var fader = "KillWithOff"
    var fireFirstCue = true
    /// `StopOnFinal` or `Loop`.
    var loop = "StopOnFinal"

    init() {}

    private enum CodingKeys: String, CodingKey {
        case tracking, fader, loop
        case fireFirstCue = "fire_first_cue"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        tracking = (try? c.decode(Bool.self, forKey: .tracking)) ?? true
        fader = (try? c.decode(String.self, forKey: .fader)) ?? "KillWithOff"
        fireFirstCue = (try? c.decode(Bool.self, forKey: .fireFirstCue)) ?? true
        loop = (try? c.decode(String.self, forKey: .loop)) ?? "StopOnFinal"
    }
}

/// CUE-29: where a chase gets its tempo: `Free`, `Bpm` n or `Rate` n (wire: "Free", {"Bpm":n}).
struct SpeedSource: Decodable, Equatable {
    var kind = "Free"
    var number = 1

    init() {}

    init(from decoder: Decoder) throws {
        if let text = try? decoder.singleValueContainer().decode(String.self) {
            kind = text
        } else if let pair = try? decoder.singleValueContainer().decode([String: Int].self), let first = pair.first {
            kind = first.key
            number = first.value
        }
    }

    /// The wire form of `playback.options`.
    var wire: Any { kind == "Free" ? "Free" : [kind: number] }
}

/// CUE-29, CUE-38: the options of a chase.
struct ChaseOptions: Decodable, Equatable {
    var bpm = 120.0
    var crossfade = 0.0
    var direction = "Forward"
    var loop = "Loop"
    var speedSource = SpeedSource()

    init() {}

    private enum CodingKeys: String, CodingKey {
        case bpm, crossfade, direction, loop
        case speedSource = "speed_source"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        bpm = (try? c.decode(Double.self, forKey: .bpm)) ?? 120
        crossfade = (try? c.decode(Double.self, forKey: .crossfade)) ?? 0
        direction = (try? c.decode(String.self, forKey: .direction)) ?? "Forward"
        loop = (try? c.decode(String.self, forKey: .loop)) ?? "Loop"
        speedSource = (try? c.decode(SpeedSource.self, forKey: .speedSource)) ?? SpeedSource()
    }
}

/// CUE-35, CUE-36: a speed master's settings; a BPM master has `bpm` and friends, a Rate master
/// `rateScale`.
struct MasterInfo: Decodable, Equatable {
    var number = 1
    var bpm: Double?
    var multiplier: Double?
    /// `None`, `Bpm` or `Multiplier`.
    var fader: String?
    var faderMax: Double?
    var rateScale: Int?
    /// GRM-15: a group master has `group`, `mode` (`Scale`, `Htp`, `Limit`, `TakeOver`), `scale` (%),
    /// `enabled` and `level` (the saved level, what the strip shows while disabled).
    var group: Int?
    var mode: String?
    var scale: Int?
    var enabled: Bool?
    var level: Double?

    private enum CodingKeys: String, CodingKey {
        case number, bpm, multiplier, fader, group, mode, scale, enabled, level
        case faderMax = "fader_max", rateScale = "rate_scale"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        number = (try? c.decode(Int.self, forKey: .number)) ?? 1
        bpm = try? c.decode(Double.self, forKey: .bpm)
        multiplier = try? c.decode(Double.self, forKey: .multiplier)
        fader = try? c.decode(String.self, forKey: .fader)
        faderMax = try? c.decode(Double.self, forKey: .faderMax)
        rateScale = try? c.decode(Int.self, forKey: .rateScale)
        group = try? c.decode(Int.self, forKey: .group)
        mode = try? c.decode(String.self, forKey: .mode)
        scale = try? c.decode(Int.self, forKey: .scale)
        enabled = try? c.decode(Bool.self, forKey: .enabled)
        level = try? c.decode(Double.self, forKey: .level)
    }
}

/// PBK-35: what a playback has stored; changes only by an edit.
struct PlaybackStored: Decodable, Identifiable, Equatable {
    struct Options: Decodable, Equatable {
        var priority = "Normal"
        var faderMode = 0
        var killPoint = "FadeOutComplete"
        /// nil = the global release mask (PBK-20).
        var releaseMask: [String]?
        /// nil = the global release time.
        var releaseTime: Double?
        var paging = "Unlocked"
        var group: Int?

        init() {}

        private enum CodingKeys: String, CodingKey {
            case priority, paging, group
            case faderMode = "fader_mode", killPoint = "kill_point"
            case releaseMask = "release_mask", releaseTime = "release_time"
        }

        init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            priority = (try? c.decode(String.self, forKey: .priority)) ?? "Normal"
            faderMode = (try? c.decode(Int.self, forKey: .faderMode)) ?? 0
            killPoint = (try? c.decode(String.self, forKey: .killPoint)) ?? "FadeOutComplete"
            releaseMask = try? c.decode([String].self, forKey: .releaseMask)
            releaseTime = try? c.decode(Double.self, forKey: .releaseTime)
            paging = (try? c.decode(String.self, forKey: .paging)) ?? "Unlocked"
            group = try? c.decode(Int.self, forKey: .group)
        }
    }

    /// The first cue's times (PBK-34).
    struct Times: Decodable, Equatable {
        var delay = 0.0
        var fadeIn = 0.0
        var fadeOut = 0.0
        var delayOut = 0.0
        var fadeOutAsIn = false
        var delayOutAsIn = false
        var overlap = 100.0

        init() {}

        private enum CodingKeys: String, CodingKey {
            case delay, overlap
            case fadeIn = "fade_in", fadeOut = "fade_out"
            case delayOut = "delay_out", fadeOutAsIn = "fade_out_as_in", delayOutAsIn = "delay_out_as_in"
        }

        init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            delay = (try? c.decode(Double.self, forKey: .delay)) ?? 0
            fadeIn = (try? c.decode(Double.self, forKey: .fadeIn)) ?? 0
            fadeOut = (try? c.decode(Double.self, forKey: .fadeOut)) ?? 0
            delayOut = (try? c.decode(Double.self, forKey: .delayOut)) ?? delay
            fadeOutAsIn = (try? c.decode(Bool.self, forKey: .fadeOutAsIn)) ?? false
            delayOutAsIn = (try? c.decode(Bool.self, forKey: .delayOutAsIn)) ?? false
            overlap = (try? c.decode(Double.self, forKey: .overlap)) ?? 100
        }
    }

    let id: Int
    /// The home handle (PBK-06).
    let page: Int
    let slot: Int
    let legend: String
    /// A picked icon id of the app's library, else nil (the auto icon is drawn).
    let icon: String?
    let autoIcon: AutoIcon?
    /// The effective halo (PBK-33): picked, else the automatic icon's first colour, else nil.
    let halo: AutoIcon.RGB?
    let options: Options
    let times: Times
    /// Attribute groups all cues hold, in I P C G B E S FX order.
    let groups: [String]
    /// CUE-40: `Cues` (a memory or a cue list), `Chase`, `BpmMaster`, `RateMaster` or `GroupMaster`.
    let kind: String
    let cues: [StoredCue]
    let list: ListOptions
    let chase: ChaseOptions
    let master: MasterInfo?
    /// CUE-25, IPC-28: what the two handle buttons do now (upper, lower); `None` = no function.
    let buttons: [String]

    private enum CodingKeys: String, CodingKey {
        case id, page, slot, legend, icon, halo, options, times, groups, kind, cues, list, chase, master, buttons
        case autoIcon = "auto_icon"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(Int.self, forKey: .id)
        page = (try? c.decode(Int.self, forKey: .page)) ?? 1
        slot = (try? c.decode(Int.self, forKey: .slot)) ?? 1
        legend = (try? c.decode(String.self, forKey: .legend)) ?? ""
        icon = try? c.decode(String.self, forKey: .icon)
        autoIcon = try? c.decode(AutoIcon.self, forKey: .autoIcon)
        halo = try? c.decode(AutoIcon.RGB.self, forKey: .halo)
        options = (try? c.decode(Options.self, forKey: .options)) ?? Options()
        times = (try? c.decode(Times.self, forKey: .times)) ?? Times()
        groups = (try? c.decode([String].self, forKey: .groups)) ?? []
        kind = (try? c.decode(String.self, forKey: .kind)) ?? "Cues"
        cues = ((try? c.decode([Tolerant<StoredCue>].self, forKey: .cues)) ?? []).compactMap(\.value)
        list = (try? c.decode(ListOptions.self, forKey: .list)) ?? ListOptions()
        chase = (try? c.decode(ChaseOptions.self, forKey: .chase)) ?? ChaseOptions()
        master = try? c.decode(MasterInfo.self, forKey: .master)
        let pair = (try? c.decode([String].self, forKey: .buttons)) ?? []
        buttons = pair.count == 2 ? pair : ["None", "None"]
    }

    var isMaster: Bool { kind == "BpmMaster" || kind == "RateMaster" || isGroupMaster }
    var isGroupMaster: Bool { kind == "GroupMaster" }
}

/// One `playbacks` entry (PBK-35): the stored part and the live part in one JSON object.
struct PlaybackEntry: Decodable {
    let stored: PlaybackStored
    let live: PlaybackLive

    init(from decoder: Decoder) throws {
        stored = try PlaybackStored(from: decoder)
        live = (try? PlaybackLive(from: decoder)) ?? PlaybackLive()
    }
}

/// The `playback` delta: an id and the live part, flattened.
private struct PlaybackLiveDelta: Decodable {
    let id: Int
    let live: PlaybackLive

    private enum CodingKeys: String, CodingKey { case id }

    init(from decoder: Decoder) throws {
        id = try decoder.container(keyedBy: CodingKeys.self).decode(Int.self, forKey: .id)
        live = try PlaybackLive(from: decoder)
    }
}

/// A playback as the UI draws it: stored part plus the live values the app shows (a held fader
/// parks the engine's level, UI-22).
struct PlaybackInfo: Identifiable, Equatable {
    let stored: PlaybackStored
    var live: PlaybackLive

    var id: Int { stored.id }
    var homeHandle: String { "\(stored.page).\(stored.slot)" }
    /// UI-56: a disabled group master, and the level it masters at (the saved one while disabled).
    var locked: Bool { stored.master?.enabled == false }
    var masterLevel: Double { locked ? (stored.master?.level ?? live.level) : live.level }
    /// PBK-37: the legend, else the home handle ("2.3").
    /// GRM-16: a group master without a legend is "Group 6".
    var shownLegend: String {
        if !stored.legend.isEmpty { return stored.legend }
        if stored.isGroupMaster, let group = stored.master?.group { return "Group \(group)" }
        return homeHandle
    }
    /// GRM-11: a disabled group master; the strip shows the fader and "LOCKED 50%" (VS3-07 run).
    var isLocked: Bool { stored.isGroupMaster && stored.master?.enabled == false }
    /// GRM-16: a Scale or Limit group master below 100 %, an Htp one above 0, shows its level in amber.
    var levelWarns: Bool {
        guard stored.isGroupMaster, !isLocked, let m = stored.master else { return false }
        switch m.mode {
        case "Scale", "Limit": return live.level < 1
        case "Htp": return live.level > 0
        default: return false
        }
    }
    var isRunning: Bool { live.state != "off" }
    /// A cue list or a chase: the engine sends live cue state for these only (CUE-40).
    var hasCues: Bool { live.cue != nil }
    var isChase: Bool { stored.kind == "Chase" }
    /// One cue, no Go: the Cue list window shows it on a tap (UI-53).
    var isMemory: Bool { stored.kind == "Cues" && stored.cues.count == 1 }
    /// A cue list: kind Cues with two or more cues; fewer is a memory (CUE-46).
    var isCueList: Bool { stored.kind == "Cues" && stored.cues.count >= 2 }
    /// The stored cue with this number.
    func cue(_ number: Double?) -> StoredCue? { number.flatMap { n in stored.cues.first { $0.number == n } } }
}

/// PBK-35: a page's number and legend.
struct PageLegend: Decodable, Equatable, Identifiable {
    let number: Int
    let legend: String
    var id: Int { number }
}

/// PBK-35: the cue and release settings (PBK-12, PBK-20). `releaseMask` and `releaseTime` are the
/// global ones.
struct PlaybackSettings: Decodable, Equatable {
    let recordBy: String
    let delay: Double
    let fadeIn: Double
    let fadeOut: Double
    /// CUE-57: the new-cue out times in force, and whether each is As In.
    let delayOut: Double
    let fadeOutAsIn: Bool
    let delayOutAsIn: Bool
    let overlap: Double
    let releaseMask: [String]
    let releaseTime: Double
    /// CUE-38: Auto connect, the fader of new lists (`KillWithOff` / `KillAtZero`), the default
    /// update mode (`Forwards` / `CueOnly` / `Backwards` / `Both`) and the handle buttons (CUE-25).
    let autoConnect: Bool
    let cueListFader: String
    let updateMode: String
    let handleButtons: HandleButtons

    private enum CodingKeys: String, CodingKey {
        case delay, overlap
        case recordBy = "record_by", fadeIn = "fade_in", fadeOut = "fade_out"
        case releaseMask = "release_mask", releaseTime = "release_time"
        case autoConnect = "auto_connect", cueListFader = "cue_list_fader"
        case updateMode = "update_mode", handleButtons = "handle_buttons"
        case delayOut = "delay_out", fadeOutAsIn = "fade_out_as_in", delayOutAsIn = "delay_out_as_in"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        recordBy = (try? c.decode(String.self, forKey: .recordBy)) ?? "Channel"
        delay = (try? c.decode(Double.self, forKey: .delay)) ?? 0
        fadeIn = (try? c.decode(Double.self, forKey: .fadeIn)) ?? 0
        fadeOut = (try? c.decode(Double.self, forKey: .fadeOut)) ?? 0
        delayOut = (try? c.decode(Double.self, forKey: .delayOut)) ?? delay
        fadeOutAsIn = (try? c.decode(Bool.self, forKey: .fadeOutAsIn)) ?? false
        delayOutAsIn = (try? c.decode(Bool.self, forKey: .delayOutAsIn)) ?? false
        overlap = (try? c.decode(Double.self, forKey: .overlap)) ?? 100
        releaseMask = (try? c.decode([String].self, forKey: .releaseMask)) ?? []
        releaseTime = (try? c.decode(Double.self, forKey: .releaseTime)) ?? 0
        autoConnect = (try? c.decode(Bool.self, forKey: .autoConnect)) ?? true
        cueListFader = (try? c.decode(String.self, forKey: .cueListFader)) ?? "KillWithOff"
        updateMode = (try? c.decode(String.self, forKey: .updateMode)) ?? "Forwards"
        handleButtons = (try? c.decode(HandleButtons.self, forKey: .handleButtons)) ?? HandleButtons()
    }
}

/// CUE-25: the setting `handle_buttons`: `BestPractice`, `Titan` or `Custom` with a pair
/// [upper, lower] per kind (wire: "Titan", {"Custom":{"CueList":["Go","Back"]}}).
struct HandleButtons: Decodable, Equatable {
    var mode = "BestPractice"
    var custom: [String: [String]] = [:]

    init() {}

    init(from decoder: Decoder) throws {
        if let text = try? decoder.singleValueContainer().decode(String.self) {
            mode = text
        } else if let wrapped = try? decoder.singleValueContainer().decode([String: [String: [String]]].self),
                  let pairs = wrapped["Custom"] {
            mode = "Custom"
            custom = pairs
        }
    }
}

/// CUE-54, IPC-24: one value of a cue's state; `from` is the cue it is hard in.
struct CueViewValue: Decodable, Equatable {
    let value: ProgrammerValue
    let from: Double
    /// The palette id when the value is a palette reference.
    let palette: Int?
}

/// IPC-42: a selection item, a whole fixture (`cell == nil`, wire: an integer) or one cell of it
/// (wire: the string `"901.2"`, cells are 1-based).
struct Item: Hashable, Decodable {
    let fixture: Int
    let cell: Int?

    init(fixture: Int, cell: Int? = nil) {
        self.fixture = fixture
        self.cell = cell
    }

    /// Parses `"901"` or `"901.2"`; also the form of a `located` key.
    init?(_ text: String) {
        let parts = text.split(separator: ".", omittingEmptySubsequences: false)
        guard parts.count <= 2, let f = Int(parts[0]) else { return nil }
        if parts.count == 2 {
            guard let c = Int(parts[1]) else { return nil }
            self.init(fixture: f, cell: c)
        } else {
            self.init(fixture: f)
        }
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if let f = try? c.decode(Int.self) {
            self.init(fixture: f)
        } else if let s = try? c.decode(String.self), let item = Item(s) {
            self = item
        } else {
            throw DecodingError.dataCorruptedError(in: c, debugDescription: "not an item")
        }
    }
}

/// Display order of programmer / cue rows: the engine's fixture order, a fixture's cell rows right
/// after its own row (UI-29). Rows of unknown fixtures go last.
func cellsAfterFixtures<T>(_ rows: [T], _ fixtures: [FixtureInfo], item: (T) -> Item) -> [T] {
    let place = Dictionary(fixtures.enumerated().map { ($1.id, $0) }, uniquingKeysWith: { a, _ in a })
    return rows.sorted {
        let (a, b) = (item($0), item($1))
        return (place[a.fixture] ?? .max, a.cell ?? 0) < (place[b.fixture] ?? .max, b.cell ?? 0)
    }
}

struct CueViewFixture: Decodable, Identifiable, Equatable {
    let item: Item
    let values: [String: CueViewValue]
    /// CUE-61: the cue's own values that are off.
    let dormant: [String: CueViewDormant]
    var id: Item { item }
    var fixture: Int { item.fixture }

    private enum CodingKeys: String, CodingKey { case fixture, values, dormant }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        item = try c.decode(Item.self, forKey: .fixture)
        values = ((try? c.decode([String: Tolerant<CueViewValue>].self, forKey: .values)) ?? [:]).compactMapValues(\.value)
        dormant = ((try? c.decode([String: Tolerant<CueViewDormant>].self, forKey: .dormant)) ?? [:]).compactMapValues(\.value)
    }
}

/// CUE-61: a hard value the cue holds but does not play.
struct CueViewDormant: Decodable, Equatable {
    let value: ProgrammerValue
    let palette: Int?
}

/// CUE-59: an effect entry of the cue's state; `from` is the cue it is hard in.
struct CueViewEffect: Decodable, Identifiable, Equatable {
    let entry: Int
    let look: String?
    let shape: EffectShape
    let attributes: [String]
    let members: [Item]
    let from: Double
    var id: Int { entry }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        entry = try c.decode(Int.self, forKey: .entry)
        look = try? c.decode(String.self, forKey: .look)
        shape = try c.decode(EffectShape.self, forKey: .shape)
        attributes = (try? c.decode([String].self, forKey: .attributes)) ?? []
        members = (try? c.decode([Item].self, forKey: .members)) ?? []
        from = try c.decode(Double.self, forKey: .from)
    }
    private enum CodingKeys: String, CodingKey { case entry, look, shape, attributes, members, from }
}

/// CUE-54: the answer to `cue.view` (this client only).
struct CueViewReply: Decodable, Equatable {
    let playback: Int
    let cue: Double
    let fixtures: [CueViewFixture]
    let effects: [CueViewEffect]
    /// UI-34: item → 0-based position in the cue's fixture order; empty when the cue has none.
    let order: [Item: Int]

    private enum CodingKeys: String, CodingKey { case playback, cue, fixtures, effects, order }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        playback = try c.decode(Int.self, forKey: .playback)
        cue = try c.decode(Double.self, forKey: .cue)
        fixtures = ((try? c.decode([Tolerant<CueViewFixture>].self, forKey: .fixtures)) ?? []).compactMap(\.value)
        effects = ((try? c.decode([Tolerant<CueViewEffect>].self, forKey: .effects)) ?? []).compactMap(\.value)
        let wire = (try? c.decode([String: Tolerant<Int>].self, forKey: .order)) ?? [:]
        order = Dictionary(wire.compactMap { key, step in Item(key).flatMap { item in step.value.map { (item, $0) } } }, uniquingKeysWith: { a, _ in a })
    }
}

/// IPC-57: the answer to `update.targets` (this client only): what the Update softkeys offer.
struct UpdateTargetsAnswer: Decodable, Equatable {
    struct Playback: Decodable, Equatable { let id: Int; let cue: Double }
    var playbacks: [Playback] = []
    var palettes: [Int] = []
}

/// IPC-54: the answer to `usage` (this client only). The daemon counts; `patchText` only words it.
struct UsageReply: Decodable, Equatable {
    var palette: Int?
    var fixtures: [Int]?
    var groups = 0, cues = 0, playbacks = 0, palettes = 0, emptyGroups = 0, emptyCues = 0

    private enum CodingKeys: String, CodingKey {
        case palette, fixtures, groups, cues, playbacks, palettes
        case emptyGroups = "empty_groups", emptyCues = "empty_cues"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        palette = try? c.decode(Int.self, forKey: .palette)
        fixtures = try? c.decode([Int].self, forKey: .fixtures)
        groups = (try? c.decode(Int.self, forKey: .groups)) ?? 0
        cues = (try? c.decode(Int.self, forKey: .cues)) ?? 0
        playbacks = (try? c.decode(Int.self, forKey: .playbacks)) ?? 0
        palettes = (try? c.decode(Int.self, forKey: .palettes)) ?? 0
        emptyGroups = (try? c.decode(Int.self, forKey: .emptyGroups)) ?? 0
        emptyCues = (try? c.decode(Int.self, forKey: .emptyCues)) ?? 0
    }

    /// UI-47: what removing the fixtures touches, for the Patch screen. UI-72: an exchange empties nothing,
    /// so `withEmpties: false` leaves that sentence out.
    func patchText(withEmpties: Bool = true) -> String {
        func list(_ parts: [String]) -> String {
            parts.count < 2 ? parts.joined() : parts.dropLast().joined(separator: ", ") + " and " + (parts.last ?? "")
        }
        if groups == 0 && cues == 0 && palettes == 0 { return "Not used anywhere." }
        var used: [String] = []
        if groups > 0 { used.append("\(groups) \(groups == 1 ? "group" : "groups")") }
        if cues > 0 { used.append("\(cues) \(cues == 1 ? "cue" : "cues") on \(playbacks) \(playbacks == 1 ? "playback" : "playbacks")") }
        if palettes > 0 { used.append("\(palettes) \(palettes == 1 ? "palette" : "palettes")") }
        var text = "In \(list(used))."
        var empty: [String] = []
        if emptyGroups > 0 { empty.append("\(emptyGroups) \(emptyGroups == 1 ? "group" : "groups")") }
        if emptyCues > 0 { empty.append("\(emptyCues) \(emptyCues == 1 ? "cue" : "cues")") }
        if withEmpties && !empty.isEmpty {
            text += " \(list(empty)) \(emptyGroups + emptyCues == 1 ? "ends" : "end") up empty and \(emptyGroups + emptyCues == 1 ? "stays" : "stay")."
        }
        return text + " Undo brings it all back."
    }
}

/// The current page of playbacks (PBK-03): snapshot object or `page` delta (flattened).
/// Fields missing from a message fall back to a single empty page.
struct PageState: Decodable, Equatable {
    struct Slot: Decodable, Equatable {
        let slot: Int          // 1-based
        let playback: Int?
    }

    var number = 1
    var count = 1
    var slots: [Slot] = []

    init() {}

    private enum CodingKeys: String, CodingKey { case number, count, slots }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        number = (try? c.decode(Int.self, forKey: .number)) ?? 1
        count = (try? c.decode(Int.self, forKey: .count)) ?? 1
        slots = (try? c.decode([Slot].self, forKey: .slots)) ?? []
    }
}

/// One patched fixture (IPC-10, PAT-09). `id` is the internal id, `number` the user number.
/// Every field but `id` falls back to a default so a malformed entry cannot drop a snapshot.
struct FixtureInfo: Decodable, Identifiable, Equatable {
    struct Handle: Decodable, Equatable {
        let page: Int
        let slot: Int
    }

    let id: Int
    let number: Int
    let name: String
    let profile: String
    let mode: String
    let universe: Int
    let address: Int
    let footprint: Int
    /// PAT-29: parked fixtures keep their number and remembered address but make no output.
    let parked: Bool
    /// IPC-42: the mode's cell count, 0 without cells.
    let cells: Int
    let virtualDimmer: Bool
    let handle: Handle?
    /// Attribute name → group letter (IPC-21); places programmer values in the I/P/C/B columns.
    let attributes: [String: String]
    /// IPC-42: the same for a cell item's values; empty without cells.
    let cellAttributes: [String: String]
    /// Every mode name of the fixture's profile (PAT-09), for the Change Mode dropdown.
    let modes: [String]
    /// PTC-07: attribute → the behaviour that is not default; absent attributes are default.
    let behaviour: [String: FixtureBehaviour]
    let swapPanTilt: Bool

    private enum CodingKeys: String, CodingKey {
        case id, number, name, profile, mode, universe, address, footprint, parked, cells, handle, attributes, modes, behaviour
        case swapPanTilt = "swap_pan_tilt"
        case virtualDimmer = "virtual_dimmer"
        case cellAttributes = "cell_attributes"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(Int.self, forKey: .id)
        number = (try? c.decode(Int.self, forKey: .number)) ?? id
        name = (try? c.decode(String.self, forKey: .name)) ?? ""
        profile = (try? c.decode(String.self, forKey: .profile)) ?? ""
        mode = (try? c.decode(String.self, forKey: .mode)) ?? ""
        universe = (try? c.decode(Int.self, forKey: .universe)) ?? 0
        address = (try? c.decode(Int.self, forKey: .address)) ?? 0
        footprint = (try? c.decode(Int.self, forKey: .footprint)) ?? 0
        parked = (try? c.decode(Bool.self, forKey: .parked)) ?? false
        cells = (try? c.decode(Int.self, forKey: .cells)) ?? 0
        virtualDimmer = (try? c.decode(Bool.self, forKey: .virtualDimmer)) ?? false
        handle = try? c.decode(Handle.self, forKey: .handle)
        attributes = (try? c.decode([String: String].self, forKey: .attributes)) ?? [:]
        cellAttributes = (try? c.decode([String: String].self, forKey: .cellAttributes)) ?? [:]
        modes = (try? c.decode([String].self, forKey: .modes)) ?? []
        behaviour = (try? c.decode([String: FixtureBehaviour].self, forKey: .behaviour)) ?? [:]
        swapPanTilt = (try? c.decode(Bool.self, forKey: .swapPanTilt)) ?? false
    }
}

/// PTC-07: how one attribute of a fixture is shaped at the output; every field falls back to its default.
struct FixtureBehaviour: Decodable, Equatable {
    let invert: Bool
    let low: Double
    let high: Double
    let offset: Double

    private enum CodingKeys: String, CodingKey { case invert, low, high, offset }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        invert = (try? c.decode(Bool.self, forKey: .invert)) ?? false
        low = (try? c.decode(Double.self, forKey: .low)) ?? 0
        high = (try? c.decode(Double.self, forKey: .high)) ?? 1
        offset = (try? c.decode(Double.self, forKey: .offset)) ?? 0
    }

    var limits: Bool { low != 0 || high != 1 || offset != 0 }
}

/// One manufacturer in a `library.manufacturers` reply (LIB-20), with its model count.
struct LibraryManufacturer: Decodable, Equatable {
    let name: String
    let models: Int
}

/// One mode of a library model: `channels` in a `library.models` reply, `footprint` in a `patch.profile` reply.
struct LibraryMode: Decodable, Equatable {
    let name: String
    let channels: Int

    private enum CodingKeys: String, CodingKey { case name, channels, footprint }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        name = try c.decode(String.self, forKey: .name)
        channels = (try? c.decode(Int.self, forKey: .channels)) ?? (try? c.decode(Int.self, forKey: .footprint)) ?? 0
    }
}

/// One fixture type from `library.models` (LIB-21) or `patch.profile`. The patch profile key is `manufacturer/model`.
struct LibraryModel: Decodable, Equatable {
    let manufacturer: String
    let model: String
    let modes: [LibraryMode]
    let source: String
    var profileKey: String { "\(manufacturer)/\(model)" }

    private enum CodingKeys: String, CodingKey { case manufacturer, model, modes, source }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        manufacturer = try c.decode(String.self, forKey: .manufacturer)
        model = try c.decode(String.self, forKey: .model)
        modes = (try? c.decode([LibraryMode].self, forKey: .modes)) ?? []
        source = (try? c.decode(String.self, forKey: .source)) ?? ""
    }
}

private struct ManufacturersReply: Decodable {
    let query: String?
    let items: [LibraryManufacturer]
}

private struct ModelsReply: Decodable {
    let manufacturer: String?
    let query: String?
    let items: [LibraryModel]
    let truncated: Bool?
}

/// A profile for the editor, and the key (`manufacturer/model`) it was asked under.
struct EditorProfile: Equatable {
    let key: String
    let doc: ProfileDoc
}

struct ProfileSaved: Equatable {
    let manufacturer: String
    let model: String
    let token: Int
}

/// LIB-35: the `library.imported` reply; `token` changes with every reply.
struct GdtfImported: Decodable, Equatable, Identifiable {
    struct Line: Decodable, Equatable {
        let mode: String
        let channel: Int
        let sourceName: String
        let became: String
        private enum CodingKeys: String, CodingKey { case mode, channel, sourceName = "source_name", became }
    }
    struct Report: Decodable, Equatable {
        let channels: [Line]
        let warnings: [String]
    }
    let manufacturer: String
    let model: String
    let modes: [LibraryMode]
    let replaced: Bool
    let report: Report
    var token = 0
    var id: Int { token }
    private enum CodingKeys: String, CodingKey { case manufacturer, model, modes, replaced, report }
}

private struct SavedReply: Decodable {
    let manufacturer: String
    let model: String
}

private struct DocReply: Decodable {
    let profile: ProfileDoc
    let profileKey: String?
    private enum CodingKeys: String, CodingKey { case profileKey = "profile_key", profile }
}

private struct ProfileReply: Decodable {
    let profileKey: String
    let profile: LibraryModel
    private enum CodingKeys: String, CodingKey { case profileKey = "profile_key", profile }
}

/// UI-68: live output (IPC-58) and the watched Views data (IPC-59) apart from EngineClient, so a delta redraws only its readers.
final class LiveOutput: ObservableObject {
    @Published var output: [Item: FixtureOutput] = [:]
    @Published var grid: [Item: [String: GridValue]] = [:]
    @Published var dmx: (line: Int, channels: [Int])?

    /// UI-68: one observable cell per item, so a tile stripe redraws only when its own output changes.
    private var cells: [Item: OutputCell] = [:]
    func cell(_ item: Item) -> OutputCell {
        if let c = cells[item] { return c }
        let c = OutputCell(); cells[item] = c; return c
    }
    /// Sets `output` and the cells of the entries that differ.
    func set(_ entries: [FixtureOutput], replacing: Bool = false) {
        if replacing { output = [:]; for c in cells.values where c.value != nil { c.value = nil } }
        for e in entries {
            output[e.item] = e
            let c = cell(e.item)
            if c.value != e { c.value = e }
        }
    }
}

/// UI-71: not observed by SwiftUI; the tile stripe's layer subscribes to `subject` and sets its colour directly.
final class OutputCell {
    let subject = CurrentValueSubject<FixtureOutput?, Never>(nil)
    var value: FixtureOutput? {
        get { subject.value }
        set { subject.send(newValue) }
    }
}

/// UI-71: the two values that change every tick, sent straight to the AppKit views that draw them.
struct LiveTick: Equatable {
    var level: Double
    /// The running cue transition (raw `live.cue?.progress`; each reader applies its own condition).
    var progress: Double?
}

/// UI-71: what SwiftUI must still see of a playback: everything but the ticks, plus the coarse thresholds views branch on.
struct LiveKey: Equatable {
    var info: PlaybackInfo?
    var belowFull = false, warns = false, progressPositive = false, progressBelowOne = false

    init(_ info: PlaybackInfo?) {
        self.info = info.map { PlaybackInfo(stored: $0.stored, live: $0.live.withoutTicks) }
        guard let info else { return }
        belowFull = info.live.level < 0.995
        warns = info.levelWarns
        let p = info.live.cue?.progress ?? 0
        progressPositive = p > 0
        progressBelowOne = p < 1
    }
}

/// UI-69: every playback with its live part, and the strips' slots, apart from EngineClient, so a `playback` delta redraws only its readers.
/// UI-71: the published copies change only when something other than a tick does; `current` and `currentSlots` are always fresh.
final class LivePlaybacks: ObservableObject {
    @Published private(set) var playbacks: [PlaybackInfo] = []
    @Published private(set) var slots: [PlaybackSlot] = []
    private(set) var current: [PlaybackInfo] = []
    private(set) var currentSlots: [PlaybackSlot] = []
    private var keys: [LiveKey] = []
    private var slotKeys: [LiveKey] = []
    private var cellKeys: [LiveKey] = []
    /// One cell per strip, so a moving fader redraws only its strip (plain var: the count follows `fadersPerPage`).
    private(set) var slotCells: [SlotCell] = []
    private var ticks: [Int: CurrentValueSubject<LiveTick, Never>] = [:]

    /// The live level and progress of a playback; the AppKit views subscribe, SwiftUI does not.
    func tick(_ id: Int) -> CurrentValueSubject<LiveTick, Never> {
        if let subject = ticks[id] { return subject }
        let live = current.first { $0.id == id }?.live
        let subject = CurrentValueSubject<LiveTick, Never>(LiveTick(level: live?.level ?? 0, progress: live?.cue?.progress))
        ticks[id] = subject
        return subject
    }

    func sync(playbacks new: [PlaybackInfo], slots newSlots: [PlaybackSlot]) {
        current = new
        currentSlots = newSlots
        for p in new {
            let value = LiveTick(level: p.live.level, progress: p.live.cue?.progress)
            let subject = tick(p.id)
            if subject.value != value { subject.send(value) }
        }
        let newKeys = new.map { LiveKey($0) }
        if newKeys != keys { keys = newKeys; playbacks = new }
        let newSlotKeys = newSlots.map { LiveKey($0.info) }
        if newSlotKeys != slotKeys { slotKeys = newSlotKeys; slots = newSlots }
        if slotCells.count != newSlots.count {
            slotCells = newSlots.map { SlotCell($0) }
            cellKeys = newSlotKeys
        }
        for (index, cell) in slotCells.enumerated() where cellKeys[index] != newSlotKeys[index] {
            cellKeys[index] = newSlotKeys[index]
            cell.slot = newSlots[index]
        }
    }
}

final class SlotCell: ObservableObject, Identifiable {
    @Published var slot: PlaybackSlot
    var id: Int { slot.id }
    init(_ slot: PlaybackSlot) { self.slot = slot }
}

/// One output node as reported by the daemon (OUT-33, IPC-20). Every field falls back to a default,
/// so an odd entry cannot drop the snapshot it came in.
struct OutputNode: Decodable, Identifiable, Equatable {
    enum State { case ok, degraded, stopped, disabled }

    let id: Int
    let line: Int
    let enabled: Bool
    let name: String
    let kind: String
    let portAddress: Int?
    let target: String?
    let universe: Int?
    let priority: Int?
    let unicast: String?
    let port: String?
    let state: State
    let detail: String?

    private enum CodingKeys: String, CodingKey {
        case id, line, enabled, name, kind, target, universe, priority, unicast, port, state, detail
        case portAddress = "port_address"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = (try? c.decode(Int.self, forKey: .id)) ?? 0
        line = (try? c.decode(Int.self, forKey: .line)) ?? 1
        enabled = (try? c.decode(Bool.self, forKey: .enabled)) ?? true
        name = (try? c.decode(String.self, forKey: .name)) ?? ""
        kind = (try? c.decode(String.self, forKey: .kind)) ?? "?"
        portAddress = try? c.decode(Int.self, forKey: .portAddress)
        target = try? c.decode(String.self, forKey: .target)
        universe = try? c.decode(Int.self, forKey: .universe)
        priority = try? c.decode(Int.self, forKey: .priority)
        unicast = try? c.decode(String.self, forKey: .unicast)
        port = try? c.decode(String.self, forKey: .port)
        switch try? c.decode(String.self, forKey: .state) {
        case "Ok": state = .ok
        case "Stopped": state = .stopped
        case "Disabled": state = .disabled
        default: state = .degraded   // "Degraded" or something newer: show it as a warning
        }
        detail = try? c.decode(String.self, forKey: .detail)
    }

    /// OUT-42: how a node is named for a person, the engine's rule: `serial #3`, `Kerk Eurolite (serial #3)`.
    var label: String {
        let k = kind == "artnet" ? "art-net" : kind
        return name.isEmpty ? "\(k) #\(id)" : "\(name) (\(k) #\(id))"
    }
}

/// A serial port the daemon found (OUT-35). `enttec` is OUT-09's product-string test.
struct SerialPort: Decodable, Identifiable, Equatable {
    let path: String
    let product: String?
    let vid: String?
    let pid: String?
    let enttec: Bool
    var id: String { path }

    private enum CodingKeys: String, CodingKey { case path, product, vid, pid, enttec }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        path = (try? c.decode(String.self, forKey: .path)) ?? ""
        product = try? c.decode(String.self, forKey: .product)
        vid = try? c.decode(String.self, forKey: .vid)
        pid = try? c.decode(String.self, forKey: .pid)
        enttec = (try? c.decode(Bool.self, forKey: .enttec)) ?? false
    }
}

/// Decodes to nil instead of throwing, for fields that must never drop a whole message.
struct Tolerant<T: Decodable>: Decodable {
    let value: T?
    init(from decoder: Decoder) throws { value = try? T(from: decoder) }
}

/// IPC-31: the snapshot nests this under `undo`; the delta carries it at top level.
private struct UndoLabels: Decodable {
    let undo: String?
    let redo: String?
}

private struct SnapshotUndo: Decodable { let undo: Tolerant<UndoLabels>? }

/// IPC-58 / UI-60: what one fixture (or cell) outputs now. `rgb` is linear 0..1, nil without colour.
struct FixtureOutput: Decodable, Equatable {
    let item: Item
    let i: Double
    let rgb: [Double]?
    let source: String?
    let tracked: Bool
    let effect: Bool

    private enum CodingKeys: String, CodingKey { case id, i, rgb, source, tracked, effect }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        if let n = try? c.decode(Int.self, forKey: .id) {
            item = Item(fixture: n)
        } else if let t = try? c.decode(String.self, forKey: .id), let it = Item(t) {
            item = it
        } else {
            throw DecodingError.dataCorruptedError(forKey: .id, in: c, debugDescription: "bad output id")
        }
        i = try c.decode(Double.self, forKey: .i)
        rgb = try? c.decode([Double].self, forKey: .rgb)
        source = try? c.decode(String.self, forKey: .source)
        tracked = (try? c.decode(Bool.self, forKey: .tracked)) ?? false
        effect = (try? c.decode(Bool.self, forKey: .effect)) ?? false
    }
}

/// IPC-59: one attribute of one grid row: the value, the palette's number if it is a reference, the source label.
struct GridValue: Decodable {
    let v: ProgrammerValue
    let palette: Int?
    let source: String?
    let tracked: Bool

    private enum CodingKeys: String, CodingKey { case v, palette, source, tracked }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        v = try c.decode(ProgrammerValue.self, forKey: .v)
        palette = try? c.decode(Int.self, forKey: .palette)
        source = try? c.decode(String.self, forKey: .source)
        tracked = (try? c.decode(Bool.self, forKey: .tracked)) ?? false
    }
}

/// IPC-59: the one view this client watches (UI-61).
enum OutputWatch: Equatable {
    case grid
    case dmx(Int)
}

private struct GridMessage: Decodable {
    struct Entry: Decodable {
        let id: Item
        let values: [String: Tolerant<GridValue>]
    }
    let fixtures: [Tolerant<Entry>]
}

private struct DMXMessage: Decodable {
    let line: Int
    let channels: [Int]
}

/// The snapshot carries `output`, the delta `fixtures` (changed entries only).
private struct OutputEntries: Decodable {
    let output: [Tolerant<FixtureOutput>]?
    let fixtures: [Tolerant<FixtureOutput>]?
}

/// One recorded group; `fixtures` are in group order (IPC-10).
struct GroupInfo: Decodable, Identifiable, Equatable {
    let id: Int
    let fixtures: [Item]
    /// PRG-90: absent when empty.
    let legend: String?
    /// UI-12: the legend, else "Group 5".
    var shownLegend: String { legend.flatMap { $0.isEmpty ? nil : $0 } ?? "Group \(id)" }
    /// PRG-90: "Group 5 Pars outside in", or "Group 5" without a legend.
    var title: String { legend.flatMap { $0.isEmpty ? nil : "Group \(id) \($0)" } ?? "Group \(id)" }
}

/// One attribute-group tab I P C G B E S FX (IPC-11).
struct AttributeGroupState: Decodable, Identifiable, Equatable {
    let group: String
    let available: Bool
    let hasValues: Bool
    var id: String { group }

    enum CodingKeys: String, CodingKey {
        case group, available
        case hasValues = "has_values"
    }
}

/// One wheel of the current layout; `value` is 0.0...1.0 (IPC-11).
struct WheelState: Decodable, Identifiable, Equatable {
    let attribute: String
    let value: Double
    let touched: Bool
    let sixteenBit: Bool
    /// PAL-26: the palette the value comes from (first target), nil for a manual or output value.
    let palette: Int?
    /// IPC-46: a target auto-mixes this emitter (LIB-36).
    let auto: Bool
    var id: String { attribute }

    private enum CodingKeys: String, CodingKey {
        case attribute, value, touched, palette, auto
        case sixteenBit = "sixteen_bit"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        attribute = try c.decode(String.self, forKey: .attribute)
        value = try c.decode(Double.self, forKey: .value)
        touched = try c.decode(Bool.self, forKey: .touched)
        sixteenBit = (try? c.decode(Bool.self, forKey: .sixteenBit)) ?? false
        palette = try? c.decode(Int.self, forKey: .palette)
        auto = (try? c.decode(Bool.self, forKey: .auto)) ?? false
    }
}

/// One programmer value (IPC-21): a level 0...1 or an sRGB colour 0...1.
enum ProgrammerValue: Decodable, Equatable {
    case level(Double)
    case colour(r: Double, g: Double, b: Double)

    private struct RGB: Decodable { let r, g, b: Double }

    init(from decoder: Decoder) throws {
        if let level = try? decoder.singleValueContainer().decode(Double.self) {
            self = .level(level)
        } else {
            let c = try RGB(from: decoder)
            self = .colour(r: c.r, g: c.g, b: c.b)
        }
    }
}

/// The programmer values of one fixture, by attribute name (IPC-21).
struct ProgrammerContent: Decodable, Identifiable, Equatable {
    let item: Item
    let values: [String: ProgrammerValue]
    /// PAL-26: attribute name → palette id, for the values that are palette references.
    let palettes: [String: Int]
    /// PAL-26: attribute name → progress 0...1 of a palette fade still running; absent once done.
    let fading: [String: Double]
    var id: Item { item }
    var fixture: Int { item.fixture }

    private enum CodingKeys: String, CodingKey { case fixture, values, palettes, fading }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        item = try c.decode(Item.self, forKey: .fixture)
        let raw = (try? c.decode([String: Tolerant<ProgrammerValue>].self, forKey: .values)) ?? [:]
        values = raw.compactMapValues(\.value)
        palettes = (try? c.decode([String: Int].self, forKey: .palettes)) ?? [:]
        fading = (try? c.decode([String: Double].self, forKey: .fading)) ?? [:]
    }
}

/// LIB-39: the `profile.emitter` reply. `status` is `auto`, `no_colour` or `outside_gamut`.
struct EmitterInfo: Decodable, Equatable {
    let name: String
    let cie: [Double]?
    let defaultCie: [Double]?
    let status: String

    private enum CodingKeys: String, CodingKey {
        case name, cie, status
        case defaultCie = "default_cie"
    }
}

/// The first target's colour as HSI hue and saturation plus sRGB, all 0...1 (IPC-22).
struct TargetColour: Decodable, Equatable {
    let hue: Double
    let saturation: Double
    let r: Double
    let g: Double
    let b: Double
    /// IPC-46: correlated colour temperature in K (rounded to 10 by the engine), nil for black.
    var kelvin: Double?
    /// IPC-47: the LEE gel the colour reads as (PRG-69), nil when it matches none.
    var gel: GelRef?
}

/// IPC-47: a gel named by the engine, for the colour reading.
struct GelRef: Decodable, Equatable {
    let range: String
    let number: Int
    let name: String
}

/// IPC-47: one row of `library.gels`; r g b are the engine's colour (drawing only).
struct Gel: Decodable, Equatable, Identifiable {
    let number: Int
    let name: String
    let r: Double
    let g: Double
    let b: Double
    var id: Int { number }
}

private struct GelsReply: Decodable { let gels: [Gel] }

/// PAL-26: `in_use` = palettes the programmer references, `relevant` = palettes that resolve
/// something for a target (all of them when nothing is selected).
struct PaletteUse: Decodable, Equatable {
    var inUse: [Int] = []
    var relevant: [Int] = []

    init() {}

    private enum CodingKeys: String, CodingKey {
        case inUse = "in_use"
        case relevant
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        inUse = (try? c.decode([Int].self, forKey: .inUse)) ?? []
        relevant = (try? c.decode([Int].self, forKey: .relevant)) ?? []
    }
}

/// The automatic icon the engine computes from a palette's values (PAL-21): colours as sRGB 0...1,
/// pan/tilt points 0...1, or the first stored attribute with its value.
enum AutoIcon: Decodable, Equatable {
    struct RGB: Decodable, Equatable { let r, g, b: Double }
    struct Point: Equatable { let pan, tilt: Double }

    case colours([RGB])
    case position([Point])
    case value(attribute: String, value: Double)

    private struct Raw: Decodable {
        struct Value: Decodable { let attribute: String; let value: Double }
        let colours: [RGB]?
        let position: [[Double]]?
        let value: Value?
    }

    init(from decoder: Decoder) throws {
        let raw = try Raw(from: decoder)
        if let colours = raw.colours, !colours.isEmpty {
            self = .colours(colours)
        } else if let points = raw.position, !points.isEmpty {
            self = .position(points.compactMap { $0.count == 2 ? Point(pan: $0[0], tilt: $0[1]) : nil })
        } else if let v = raw.value {
            self = .value(attribute: v.attribute, value: v.value)
        } else {
            throw DecodingError.dataCorrupted(.init(codingPath: decoder.codingPath, debugDescription: "no auto icon"))
        }
    }
}

/// One palette (PAL-25). `id` is the engine's internal id (references use it), `number` the user
/// number (commands use it). `window` is `Colours`, `Positions` or `Beams`; `handle` is 1-based.
struct PaletteInfo: Decodable, Identifiable, Equatable {
    let id: Int
    let number: Int
    let window: String
    let handle: Int
    let legend: String
    /// A picked icon id of the app's library, else nil (the auto icon is drawn).
    let icon: String?
    let autoIcon: AutoIcon?
    /// `G`, `S` or `N`.
    let kind: String
    /// Attribute groups the palette holds, in I P C G B E S FX order.
    let groups: [String]
    /// FX-39: the effect entries the palette holds (key absent when none).
    let effects: [PaletteEffect]

    private enum CodingKeys: String, CodingKey {
        case id, number, window, handle, legend, icon, kind, groups, effects
        case autoIcon = "auto_icon"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(Int.self, forKey: .id)
        number = try c.decode(Int.self, forKey: .number)
        window = try c.decode(String.self, forKey: .window)
        handle = try c.decode(Int.self, forKey: .handle)
        legend = (try? c.decode(String.self, forKey: .legend)) ?? ""
        icon = try? c.decode(String.self, forKey: .icon)
        autoIcon = try? c.decode(AutoIcon.self, forKey: .autoIcon)
        kind = (try? c.decode(String.self, forKey: .kind)) ?? "N"
        groups = (try? c.decode([String].self, forKey: .groups)) ?? []
        effects = ((try? c.decode([Tolerant<PaletteEffect>].self, forKey: .effects)) ?? []).compactMap(\.value)
    }
}

/// PAL-25: master fade time (s), master overlap (%) and the default Record By (`Channel` / `Mixed`).
struct PaletteSettings: Decodable, Equatable {
    let time: Double
    let overlap: Double
    let recordBy: String

    private enum CodingKeys: String, CodingKey {
        case time, overlap
        case recordBy = "record_by"
    }
}

/// PRG-43: how Clear unwinds, what it does to LTP attributes, and the Rem Dim level (IPC-33).
struct ProgrammerSettings: Decodable, Equatable {
    let clearPrecedence: String
    let clearLtp: String
    let lowlightLevel: Double

    private enum CodingKeys: String, CodingKey {
        case clearPrecedence = "clear_precedence", clearLtp = "clear_ltp", lowlightLevel = "lowlight_level"
    }
}

/// UI-33: the selection grid of IPC-48. The name is derived for display only, from the PRG-75 mapping.
struct GridState: Decodable, Equatable {
    var wings = 1, groups = 0, blocks = 1
    var flip = false

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        wings = (try? c.decode(Int.self, forKey: .wings)) ?? 1
        groups = (try? c.decode(Int.self, forKey: .groups)) ?? 0
        blocks = (try? c.decode(Int.self, forKey: .blocks)) ?? 1
        flip = (try? c.decode(Bool.self, forKey: .flip)) ?? false
    }
    private enum CodingKeys: String, CodingKey { case wings, groups, blocks, flip }

    /// The shortcut this grid matches (`Divide` shows as `Blocks`); nil = none.
    var shortcut: String? {
        if wings == 2 && groups == 0 && blocks == 1 { return flip ? "InsideOut" : "OutsideIn" }
        guard wings == 1, !flip else { return nil }
        if groups == 0 && blocks > 1 { return "Blocks" }
        if groups == 2 && blocks == 1 { return "OddEven" }
        if groups >= 2 && blocks == 1 { return "OneIn" }
        if groups >= 2 && blocks > 1 { return "NIn" }
        return nil
    }

    /// The tag text: `1 IN 3`, `ODD/EVEN`, `2 IN 4`, `BLOCKS 2`, `OUTSIDE IN`, `INSIDE OUT`, else `W2 G3 B1`.
    var name: String {
        switch shortcut {
        case "InsideOut": return "INSIDE OUT"
        case "OutsideIn": return "OUTSIDE IN"
        case "Blocks": return "BLOCKS \(blocks)"
        case "OddEven": return "ODD/EVEN"
        case "OneIn": return "1 IN \(groups)"
        case "NIn": return "\(blocks) IN \(groups)"
        default: return "W\(wings) G\(groups) B\(blocks)"
        }
    }
}

/// Mirror of the engine's programmer state (IPC-11). Fields missing from a
/// message fall back to empty so a malformed message cannot drop a snapshot.
struct ProgrammerState: Decodable, Equatable {
    var selection: [Item] = []
    var targets: [Item] = []
    var activeGroup: String = ""
    var attributeGroups: [AttributeGroupState] = []
    var wheels: [WheelState] = []
    var inProgrammer: [Item] = []
    /// `Hsi`, `Rgb` or `Cmy` (IPC-11, PRG-30).
    var colourModel: String = "Hsi"
    /// IPC-21: programmer values per fixture; IPC-22: the first target's colour, nil without one.
    var contents: [ProgrammerContent] = []
    var colour: TargetColour?
    /// PAL-26: palette ids referenced in the programmer, and those relevant to the selection.
    var palettes = PaletteUse()
    /// IPC-32: fixture id → attributes that Locate drives; Highlight on or off.
    var located: [Item: [String]] = [:]
    var highlight = false
    /// IPC-43: the Fan curve while Fan is on (nil = off) and the last curve used.
    var fan: String?
    var fanCurve = "Line"
    /// FX-33: the programmer's effect entries; FX-17: the wheels are the effect wheels; FX-37: the
    /// 1-based catalogue ids of the looks some target can run.
    var effects: [EffectEntry] = []
    var effectWheels = false
    var runnableLooks: [Int] = []
    /// IPC-48: the selection grid (nil = off), the stage (nil = all), each selection item's grid position
    /// (0-based, in selection order) and the number of positions.
    var grid: GridState?
    var stage: Int?
    var positions: [Int] = []
    var positionCount = 0

    init() {}

    enum CodingKeys: String, CodingKey {
        case selection, targets, wheels, contents, colour, palettes, located, highlight, fan, effects, grid, stage, positions
        case positionCount = "position_count"
        case effectWheels = "effect_wheels"
        case runnableLooks = "runnable_looks"
        case activeGroup = "active_group"
        case attributeGroups = "attribute_groups"
        case inProgrammer = "in_programmer"
        case colourModel = "colour_model"
        case fanCurve = "fan_curve"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        selection = (try? c.decode([Item].self, forKey: .selection)) ?? []
        targets = (try? c.decode([Item].self, forKey: .targets)) ?? []
        activeGroup = (try? c.decode(String.self, forKey: .activeGroup)) ?? ""
        attributeGroups = (try? c.decode([AttributeGroupState].self, forKey: .attributeGroups)) ?? []
        wheels = (try? c.decode([WheelState].self, forKey: .wheels)) ?? []
        inProgrammer = (try? c.decode([Item].self, forKey: .inProgrammer)) ?? []
        colourModel = (try? c.decode(String.self, forKey: .colourModel)) ?? "Hsi"
        contents = ((try? c.decode([Tolerant<ProgrammerContent>].self, forKey: .contents)) ?? []).compactMap(\.value)
        colour = try? c.decode(TargetColour.self, forKey: .colour)
        palettes = (try? c.decode(PaletteUse.self, forKey: .palettes)) ?? PaletteUse()
        let rawLocated = (try? c.decode([String: [String]].self, forKey: .located)) ?? [:]
        located = Dictionary(rawLocated.compactMap { key, value in Item(key).map { ($0, value) } }, uniquingKeysWith: { a, _ in a })
        highlight = (try? c.decode(Bool.self, forKey: .highlight)) ?? false
        fan = try? c.decode(String.self, forKey: .fan)
        fanCurve = (try? c.decode(String.self, forKey: .fanCurve)) ?? "Line"
        effects = ((try? c.decode([Tolerant<EffectEntry>].self, forKey: .effects)) ?? []).compactMap(\.value)
        effectWheels = (try? c.decode(Bool.self, forKey: .effectWheels)) ?? false
        runnableLooks = (try? c.decode([Int].self, forKey: .runnableLooks)) ?? []
        grid = try? c.decode(GridState.self, forKey: .grid)
        stage = try? c.decode(Int.self, forKey: .stage)
        positions = (try? c.decode([Int].self, forKey: .positions)) ?? []
        positionCount = (try? c.decode(Int.self, forKey: .positionCount)) ?? 0
    }

    /// FX-18: the entry the editor shows.
    var currentEffect: EffectEntry? { effects.first(where: \.current) }
}

/// `programmer.wheel` resolution (IPC-13); raw values are the wire strings.
enum WheelResolution: String {
    case normal = "Normal", fine = "Fine", coarse = "Coarse"
}

/// ISO-8601 UTC as the daemon writes it, with or without fractional seconds (SHW-15).
private func parseISODate(_ text: String) -> Date? {
    (try? Date(text, strategy: .iso8601)) ?? (try? Date(text, strategy: Date.ISO8601FormatStyle(includingFractionalSeconds: true)))
}

/// RMT-15: the phone remote's state as the engine reports it; the password is never part of it.
struct RemoteState: Decodable, Equatable {
    var on = false
    var username: String?
    var port = 7320
    var addresses: [String] = []
    var clients = 0
}

/// The open show (SHW-15): name, last successful autosave and whether autosave works. An empty
/// name means no snapshot has arrived yet.
struct ShowState: Decodable, Equatable {
    var name = ""
    var savedAt: Date?
    var autosaveOk = true

    init() {}

    private enum CodingKeys: String, CodingKey {
        case name
        case savedAt = "saved_at"
        case autosaveOk = "autosave_ok"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        name = (try? c.decode(String.self, forKey: .name)) ?? ""
        savedAt = (try? c.decode(String.self, forKey: .savedAt)).flatMap(parseISODate)
        autosaveOk = (try? c.decode(Bool.self, forKey: .autosaveOk)) ?? true
    }

    /// SHW-16: "not saved yet", "saved 4 s ago", above 59 s "saved 2 min ago".
    func ageText(now: Date) -> String {
        guard let savedAt else { return "not saved yet" }
        let s = max(0, Int(now.timeIntervalSince(savedAt)))
        return s < 60 ? "saved \(s) s ago" : "saved \(s / 60) min ago"
    }
}

/// One saved show in the shows folder (SHW-15 `show.list`).
struct ShowEntry: Decodable, Identifiable, Equatable {
    let name: String
    let modified: Date?
    var id: String { name }

    private enum CodingKeys: String, CodingKey { case name, modified }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        name = try c.decode(String.self, forKey: .name)
        modified = (try? c.decode(String.self, forKey: .modified)).flatMap(parseISODate)
    }
}

/// UI-39: one line of the Messages sheet; `level` is "warning", "notice" or "error".
struct MessageLine: Identifiable {
    let id = UUID()
    let at: Date
    let level: String
    let message: String
}

/// Thin IPC client: WebSocket to cmdmx-daemon (specs/modules/ipc.md, IPC-03..09).
/// Holds no lighting logic; local state is a mirror of the engine's snapshot and deltas.
@MainActor
final class EngineClient: ObservableObject {
    @Published private(set) var connection: EngineConnection = .offline
    /// Attributes a Fine step was sent on since the app started (IPC-16: one decimal).
    @Published private(set) var fineUsed: Set<String> = []
    /// From the engine's settings snapshot. Never hard-code 8 or 10.
    @Published private(set) var fadersPerPage: Int = 8
    /// PBK-03: the current page and the playback each slot holds; `slots` is derived from it.
    @Published private(set) var page = PageState()
    /// UI-69: the playbacks and slots live in `livePlaybacks`; these read the current value (press handlers, stored reads).
    /// A view that shows a live value declares `@EnvironmentObject LivePlaybacks` too.
    let livePlaybacks = LivePlaybacks()
    var slots: [PlaybackSlot] { livePlaybacks.currentSlots }
    /// PBK-35: every playback, sorted by id (the engine's order); `slots` and the Active
    /// playbacks window are derived from it.
    var playbacks: [PlaybackInfo] { livePlaybacks.current }
    /// PBK-35: the ten pages' legends, `dbo` and the cue / release settings; nil until the snapshot.
    @Published private(set) var pages: [PageLegend] = []
    @Published private(set) var dbo = false
    @Published private(set) var playbackSettings: PlaybackSettings?
    /// CUE-26: the connected list (the Go block, Space and the Cue list window act on it), or nil.
    @Published private(set) var connected: Int?
    /// CUE-54: the last `cue.view` reply (this client only), and a counter that goes up with every
    /// stored change (snapshot, `playbacks` delta) so an open Cue view asks again.
    @Published private(set) var cueView: CueViewReply?
    /// IPC-54: the answer to the last `usage` ask.
    @Published private(set) var usage: UsageReply?
    /// UI-57, IPC-60: the answers to the last `why` and `usage` detail asks.
    @Published private(set) var why: WhyReply?
    @Published private(set) var usageDetail: UsageDetail?
    /// IPC-57: the answer to the last `update.targets` ask.
    @Published private(set) var updateTargets: UpdateTargetsAnswer?
    /// UI-47: the palette delete message still waiting for its count, and the text it showed.
    private var pendingDelete: (palette: Int, shown: String)?
    @Published private(set) var storedRevision = 0
    /// Stored part and shown live part per playback id, so a page switch shows the right values at once.
    private var playbackStored: [Int: PlaybackStored] = [:]
    private var playbackLive: [Int: PlaybackLive] = [:]
    @Published private(set) var grandMaster: Double = 1.0
    /// Latest `warning` / `error` / `notice` message from the engine; cleared when a
    /// `programmer.command` is sent (IPC-17).
    @Published private(set) var lastMessage: String?
    /// UI-38, IPC-50: the engine's lock; the cover follows it.
    @Published private(set) var locked = false
    static let shared = EngineClient()
    /// UI-39: the Messages sheet's history, oldest first, at most `messageCap` lines. Merged from the
    /// engine's snapshot (IPC-51) plus the app's own lines; `unseenMessages` counts warnings and errors
    /// since the sheet was last open.
    @Published private(set) var messageLog: [MessageLine] = []
    @Published private(set) var unseenMessages = 0
    private static let messageCap = 500
    private var launcherNotes: AnyCancellable?
    /// IPC-31: what Undo / Redo would do now; nil when there is nothing.
    @Published private(set) var undoLabel: String?
    /// UI-79: this app has sent an edit of its own; until then the undo history may hold earlier work.
    @Published private(set) var ownEdit = false
    private var editSent = false
    /// UI-79: the label a first Undo asked to confirm.
    private var undoAsked: String?
    /// UI-79: the Undo key reads "(earlier)" until this app has edited.
    var undoEarlier: Bool { !ownEdit && undoLabel != nil }
    /// UI-80: the text of the last engine notice, so a change of workspace or screen can end it.
    private var noticeText: String?
    @Published private(set) var redoLabel: String?
    /// IPC-10..12: all straight from the snapshot / deltas.
    @Published private(set) var fixtures: [FixtureInfo] = []
    /// IPC-58 / UI-60: live output per fixture or cell; the app draws it and animates nothing (FX-41).
    let live = LiveOutput()
    /// IPC-59 / UI-61: the watched view's last replies, cleared when the watch changes and when offline.
    private var watching: OutputWatch?
    /// `patch.profile` replies in full, by profile key: the DMX view names a channel from them (UI-61).
    @Published private(set) var profileDocs: [String: ProfileDoc] = [:]
    @Published private(set) var groups: [GroupInfo] = []
    /// PAL-25: all palettes, sorted by number.
    @Published private(set) var palettes: [PaletteInfo] = []
    /// FX-32: the look gallery and raw shapes; snapshot only.
    @Published private(set) var effectCatalogue = EffectCatalogue()
    /// FX-17: the Shapes workspace is on show; sent again after a reconnect.
    private var effectWheelsWanted = false
    /// PAL-25: the engine's master time, overlap and default Record By; nil until the snapshot.
    @Published private(set) var paletteSettings: PaletteSettings?
    /// PRG-43: Clear order, LTP on Clear and the Rem Dim level; nil until the snapshot.
    @Published private(set) var programmerSettings: ProgrammerSettings?
    /// Replies to the last `library.manufacturers` / `library.models` (this client only, LIB-20..21).
    @Published private(set) var libraryManufacturers: [LibraryManufacturer] = []
    @Published private(set) var libraryModels: [LibraryModel] = []
    @Published private(set) var libraryModelsTruncated = false
    /// `patch.profile` replies by profile key, for the "In this show" modes (LIB-22).
    @Published private(set) var profileCache: [String: LibraryModel] = [:]
    /// LIB-23: the full profile the editor asked for (`patch.profile` or `library.profile`), and the last
    /// `library.saved` reply (the token changes with every reply so repeated saves fire `onChange`).
    @Published private(set) var editorProfile: EditorProfile?
    @Published private(set) var profileSaved: ProfileSaved?
    @Published private(set) var gdtfImported: GdtfImported?
    /// LIB-41: the last `library.deleted` reply (token as in `profileSaved`).
    @Published private(set) var profileDeleted: ProfileSaved?
    /// LIB-39: latest `profile.emitter` reply per emitter name.
    @Published private(set) var emitterInfo: [String: EmitterInfo] = [:]
    /// IPC-47 / UI-32: the LEE table, asked once per connection.
    @Published private(set) var gels: [Gel] = []
    private var editorAsk: String?
    /// What was asked last (empty = nil); a reply that echoes something else is stale and dropped.
    private var lastManufacturersQuery: String?
    private var lastModelsAsk: [String?] = [nil, nil]
    @Published private(set) var programmer = ProgrammerState()
    /// IPC-20: one entry per output node; empty while offline (the state is unknown then).
    @Published private(set) var outputs: [OutputNode] = []
    @Published private(set) var serialPorts: [SerialPort] = []
    /// CTL-39: devices, mappings, Learn, the last input, OSC and the unit order; empty while offline.
    @Published private(set) var control = ControlState()
    /// The mapping the Control screen shows (reply to `control.mapping`, CTL-40); nil until asked.
    @Published private(set) var controlMapping: ControlMappingDoc?
    private var shownMapping: String?
    /// SHW-15: the open show; keeps the last known value while offline. `shows` is the reply to the
    /// last `show.list` (this client only).
    @Published private(set) var show = ShowState()
    @Published private(set) var shows: [ShowEntry] = []
    /// SHW-22: the shows folder's absolute path, from the last `show.list`.
    @Published private(set) var showsDir: String?
    /// RMT-15: the phone remote; default off until the snapshot says otherwise.
    @Published private(set) var remote = RemoteState()

    /// UI-22 / IPC-05 "touch": playback ids and the GM held by this client's pointer. While held,
    /// engine levels are parked here (not shown) so the bar follows the finger. After release the
    /// parking goes on until the engine echoes the level sent last (or `settleTimeout` passes), so
    /// older echoes still in flight never show; then the parked level is shown.
    private var heldFaders: [Int: Double?] = [:]
    private var settling: Set<Int> = []
    private var lastSent: [Int: Double] = [:]
    private var gmHeld = false
    private var gmSettling = false
    private var gmLastSent: Double?
    private var gmEngineLevel: Double?
    private var releaseTasks: [Int: Task<Void, Never>] = [:]  // key 0 = GM, else playback id
    private let settleTimeout: UInt64 = 1_000_000_000
    private let echoEpsilon = 0.001

    private let url = URL(string: "ws://127.0.0.1:\(EngineClient.ipcPort)")
    private var socket: URLSessionWebSocketTask?

    /// The engine's WebSocket port (IPC-08): `--ipc-port <n>` (1...65535), else 7319.
    nonisolated static let ipcPort: Int = {
        let args = CommandLine.arguments
        guard let i = args.firstIndex(of: "--ipc-port") else { return 7319 }
        if i + 1 < args.count, let n = Int(args[i + 1]), (1...65535).contains(n) { return n }
        FileHandle.standardError.write(Data("ignoring invalid --ipc-port, using 7319\n".utf8))
        return 7319
    }()

    /// The protocol version this app speaks (IPC-41); the one place to bump.
    nonisolated static let protocolVersion = 1
    /// The engine's version ("v1", or "no version") while its snapshot named another; nothing
    /// from it is shown.
    @Published private(set) var protocolMismatch: String?

    init() {
        rebuildSlots()
        // UI-39: the launcher's notes (REL-12, 24..28); the sheet gets a line per change of text, not per attempt (REL-28).
        launcherNotes = DaemonLauncher.shared.$note.removeDuplicates().compactMap { $0 }
            .sink { [weak self] in self?.addMessage(level: "warning", message: $0, at: Date()) }
        Task { await connectForever() }
    }

    func messagesSeen() { unseenMessages = 0 }

    private func addMessage(level: String, message: String, at: Date) {
        messageLog.append(MessageLine(at: at, level: level, message: message))
        if messageLog.count > Self.messageCap { messageLog.removeFirst(messageLog.count - Self.messageCap) }
        if level != "notice" { unseenMessages += 1 }
    }

    /// IPC-51: a missing or unparsable timestamp counts as now.
    private static func messageDate(_ at: String?) -> Date {
        at.flatMap { ISO8601DateFormatter().date(from: $0) } ?? Date()
    }

    // MARK: - Commands (app → engine)

    /// `page.set` (PBK-03). No local update: the engine's `page` delta changes the strips.
    func setPage(_ number: Int) { sendJSON("page.set", ["page": number]) }

    /// `fader.set` for the playback a slot holds (PBK-03) — optimistic local update; the engine's
    /// delta is authoritative.
    func setFader(playback id: Int, level: Double) {
        var live = playbackLive[id] ?? PlaybackLive()
        live.level = level.clamped01 // the state (active, releasing) is always the engine's
        playbackLive[id] = live
        lastSent[id] = level.clamped01
        rebuildSlots()
        send(FaderSet(playback: id, level: level.clamped01))
    }

    /// UI-22: a scroll step is a short touch, so the engine's echo of the previous step cannot
    /// pull the bar back while the wheel is still turning.
    func nudgeFader(playback id: Int, by delta: Double) {
        touchFader(playback: id, held: true)
        setFader(playback: id, level: (playbackLive[id]?.level ?? 0) + delta)
        touchFader(playback: id, held: false)
    }

    /// UI-22 / IPC-05: pointer down/up on a fader. Down holds (and cancels a pending release); up
    /// starts the settle wait: the parked level is shown once the engine echoes `lastSent`, or after
    /// `settleTimeout`.
    func touchFader(playback id: Int, held: Bool) {
        releaseTasks.removeValue(forKey: id)?.cancel()
        if held {
            if heldFaders[id] == nil { heldFaders[id] = .some(nil) }
            settling.remove(id); lastSent[id] = nil
            return
        }
        settling.insert(id)
        if releaseIfEchoed(id) { return }
        releaseTasks[id] = Task {
            try? await Task.sleep(nanoseconds: settleTimeout)
            guard !Task.isCancelled else { return }
            releaseFader(id)
        }
    }

    /// Ends the settle wait for `id` when the parked level matches the last sent one (or none was sent).
    @discardableResult
    private func releaseIfEchoed(_ id: Int) -> Bool {
        guard settling.contains(id), let parked = heldFaders[id] else { return false }
        if let sent = lastSent[id] {
            guard let level = parked, abs(level - sent) < echoEpsilon else { return false }
        }
        releaseFader(id)
        return true
    }

    private func releaseFader(_ id: Int) {
        releaseTasks.removeValue(forKey: id)?.cancel()
        settling.remove(id)
        if let parked = heldFaders.removeValue(forKey: id), let level = parked, playbackLive[id] != nil {
            playbackLive[id]?.level = level
            rebuildSlots()
        }
    }

    /// Same for the Grand Master.
    func touchGrandMaster(held: Bool) {
        releaseTasks.removeValue(forKey: 0)?.cancel()
        if held { gmHeld = true; gmSettling = false; gmLastSent = nil; return }
        gmSettling = true
        if releaseGMIfEchoed() { return }
        releaseTasks[0] = Task {
            try? await Task.sleep(nanoseconds: settleTimeout)
            guard !Task.isCancelled else { return }
            releaseGM()
        }
    }

    @discardableResult
    private func releaseGMIfEchoed() -> Bool {
        guard gmSettling, gmHeld else { return false }
        if let sent = gmLastSent {
            guard let level = gmEngineLevel, abs(level - sent) < echoEpsilon else { return false }
        }
        releaseGM()
        return true
    }

    private func releaseGM() {
        releaseTasks.removeValue(forKey: 0)?.cancel()
        gmHeld = false; gmSettling = false
        if let level = gmEngineLevel { grandMaster = level }
        gmEngineLevel = nil
    }

    /// `grand_master.set`
    func setGrandMaster(_ level: Double) {
        grandMaster = level.clamped01
        gmLastSent = level.clamped01
        send(GrandMasterSet(level: level.clamped01))
    }

    // Programmer commands (IPC-13). One-liners; the engine decides what they mean.

    func selectFixture(_ id: Int) { sendJSON("programmer.select", ["fixture": id]) }
    func selectCell(_ fixture: Int, cell: Int) { sendJSON("programmer.select", ["fixture": fixture, "cell": cell]) }
    func selectGroup(_ id: Int) { sendJSON("programmer.select_group", ["group": id]) }
    func step(_ direction: Int) { sendJSON("programmer.step", ["direction": direction]) }
    func pattern(_ stages: Int) { sendJSON("programmer.pattern", ["stages": stages]) }
    func selectAll() { sendJSON("programmer.all") }
    /// IPC-48: `Reverse`, `Shuffle`, `Sort` or `Revert`.
    func order(_ op: String) { sendJSON("programmer.order", ["op": op]) }
    /// IPC-48: a grid shortcut with its numbers; no shortcut = grid off.
    func grid(shortcut: String? = nil, n: Int? = nil, m: Int? = nil) {
        var f: [String: Any] = [:]
        if let shortcut { f["shortcut"] = shortcut }
        if let n { f["n"] = n }
        if let m { f["m"] = m }
        sendJSON("programmer.grid", f)
    }
    /// A time fades the Clear out (ENG-10, IPC-32); nil = the plain Clear (PRG-29).
    func clearProgrammer(time: Double? = nil) { lastMessage = nil; sendJSON("programmer.clear", time.map { ["time": $0] } ?? [:]) }
    /// UI-26: Locate (no groups = all), Highlight, Off (no field = the active group; or one attribute)
    /// and Rem Dim; the `programmer` delta shows the result.
    func locate() { sendJSON("programmer.locate") }
    /// IPC-43: Fan key (no curve = toggle), a curve ("Line", "Mirror", "Wings", "Arrow") or off.
    func fanToggle() { sendJSON("programmer.fan") }
    func fan(curve: String) { sendJSON("programmer.fan", ["curve": curve]) }
    func fanOff() { sendJSON("programmer.fan", ["curve": NSNull()]) }
    func setHighlight(_ on: Bool) { sendJSON("programmer.highlight", ["on": on]) }
    func programmerOff(attribute: String? = nil) {
        sendJSON("programmer.off", attribute.map { ["attribute": $0] } ?? [:])
    }
    func remDim() { sendJSON("programmer.rem_dim") }
    /// `settings.programmer`: only the given field changes.
    func setProgrammerSettings(_ fields: [String: Any]) { patch("settings.programmer", fields) }
    func setAttributeGroup(_ group: String) { sendJSON("programmer.attribute_group", ["group": group]) }
    func setColourModel(_ model: String) { sendJSON("programmer.colour_model", ["model": model]) }
    /// IPC-23: hue and saturation 0...1 under the pointer; the engine maps them to every colour target.
    func setColour(hue: Double, saturation: Double) {
        sendJSON("programmer.colour", ["hue": hue.clamped01, "saturation": saturation.clamped01])
    }
    /// IPC-46: step the target colour along the colour-temperature locus (`cto_half`, `ctb_full`, …).
    func ctStep(_ step: String) { sendJSON("programmer.ct_step", ["step": step]) }
    /// IPC-47: set the target colour to a LEE gel by number.
    func gel(_ number: Int) { sendJSON("programmer.gel", ["range": "Lee", "number": number]) }
    /// PRG-97: `mode` is Replace or Merge, needed when the group is used.
    func recordGroup(_ id: Int, mode: String? = nil) {
        var body: [String: Any] = ["group": id]
        if let mode { body["mode"] = mode }
        sendJSON("group.record", body)
    }
    // UI-64: the list forms (IPC-61); the engine places, refuses and undoes as one step.
    func deleteGroups(_ groups: [Int]) { patch("group.delete", ["groups": groups]) }
    func copyGroups(_ groups: [Int], to: Int, layout: String) { patch("group.copy", ["groups": groups, "to": to, "layout": layout]) }
    func moveGroups(_ groups: [Int], to: Int, layout: String, swap: Bool) { patch("group.move", ["groups": groups, "to": to, "layout": layout, "swap": swap]) }
    func setGroupLegend(_ id: Int, legend: String) { patch("group.legend", ["group": id, "legend": legend]) }
    /// GRM-16, PRG-90: a group master without its own legend shows its group's.
    func shownLegend(_ p: PlaybackInfo) -> String {
        if p.stored.legend.isEmpty, p.stored.isGroupMaster, let g = p.stored.master?.group,
           let group = groups.first(where: { $0.id == g }) { return group.shownLegend }
        return p.shownLegend
    }

    // Effect commands (FX-34). The engine validates ranges and refuses with a warning.

    func applyLook(_ name: String) { sendJSON("effect.apply", ["look": name]) }
    func applyShape(_ shape: EffectShape) { sendJSON("effect.apply", ["shape": shape.json]) }
    func applyEffectPalette(_ number: Int) { sendJSON("effect.apply", ["palette": number]) }
    func setEffect(_ param: String, _ value: Any) { sendJSON("effect.set", ["param": param, "value": value]) }
    func selectEffect(_ entry: Int) { sendJSON("effect.select", ["entry": entry]) }
    func stopEffects() { sendJSON("effect.stop") }
    func restartEffects() { sendJSON("effect.restart") }
    func tapEffect() { sendJSON("effect.tap") }
    func setEffectWheels(_ on: Bool) {
        effectWheelsWanted = on
        sendJSON("effect.wheels", ["on": on])
    }

    func wheel(attribute: String, steps: Int, resolution: WheelResolution) {
        guard steps != 0 else { return }
        if resolution == .fine { fineUsed.insert(attribute) }
        sendJSON("programmer.wheel", ["attribute": attribute, "steps": steps, "resolution": resolution.rawValue])
    }

    /// Sends the words plus `Enter` and clears the previous message (IPC-17).
    /// `end` is `Enter`, or `Locate` for a selection line that locates (UI-26).
    func command(words: [String], end: String = "Enter") {
        lastMessage = nil
        sendJSON("programmer.command", ["tokens": words + [end]])
    }

    // Patch commands (PAT-09, LIB-14). All take internal ids; the engine checks and may refuse
    // with a warning/error, which lands in `lastMessage`.

    private func trimmedOrNil(_ text: String?) -> String? {
        let trimmed = text?.trimmingCharacters(in: .whitespaces) ?? ""
        return trimmed.isEmpty ? nil : trimmed
    }

    func requestManufacturers(query: String) {
        let q = trimmedOrNil(query)
        lastManufacturersQuery = q
        sendJSON("library.manufacturers", q.map { ["query": $0] } ?? [:])
    }

    /// With neither a manufacturer nor a query nothing is sent and the list is empty (LIB-21).
    func requestModels(manufacturer: String?, query: String) {
        let m = trimmedOrNil(manufacturer), q = trimmedOrNil(query)
        lastModelsAsk = [m, q]
        guard m != nil || q != nil else { libraryModels = []; libraryModelsTruncated = false; return }
        var fields: [String: Any] = [:]
        if let m { fields["manufacturer"] = m }
        if let q { fields["query"] = q }
        sendJSON("library.models", fields)
    }

    /// LIB-39: ask the engine for an emitter's default CIE and whether this CIE would auto-mix.
    func profileEmitter(name: String, cie: [Double]?) {
        var fields: [String: Any] = ["name": name]
        if let cie { fields["cie"] = cie }
        sendJSON("profile.emitter", fields)
    }

    /// UI-61, IPC-59: watch one view, or none (nil); the daemon forgets it with the connection, so views send it again once online.
    func watchOutput(_ view: OutputWatch?) {
        watching = view
        live.grid = [:]; live.dmx = nil
        switch view {
        case .grid?: sendJSON("output.watch", ["view": "grid"])
        case .dmx(let line)?: sendJSON("output.watch", ["view": "dmx", "line": line])
        case nil: sendJSON("output.watch", ["view": NSNull()])
        }
    }

    func requestProfile(key: String) { sendJSON("patch.profile", ["profile": key]) }

    // Profile editor (LIB-23..27, IPC-37..39). The engine validates and refuses with a warning.

    func clearEditorProfile() { editorProfile = nil; editorAsk = nil }

    /// The show's own copy of a profile (IPC-37).
    func requestShowProfile(key: String) { editorAsk = key; sendJSON("patch.profile", ["profile": key]) }

    func requestLibraryProfile(manufacturer: String, model: String) {
        editorAsk = "\(manufacturer)/\(model)"
        sendJSON("library.profile", ["manufacturer": manufacturer, "model": model])
    }

    func saveProfile(_ doc: ProfileDoc) {
        guard let data = try? JSONEncoder().encode(doc),
              let object = try? JSONSerialization.jsonObject(with: data) else { return }
        lastMessage = nil
        sendJSON("profile.save", ["profile": object])
    }

    /// `channels` are LIB-11 channel types as JSON values (a string, `{"Emitter": name}` or `{"Other": {...}}`).
    func saveChannelList(manufacturer: String, model: String, mode: String, channels: [Any]) {
        lastMessage = nil
        sendJSON("library.save_channel_list",
                 ["manufacturer": manufacturer, "model": model, "mode": mode, "channels": channels])
    }

    /// LIB-34: a refused file comes back as an `error`, which lands in `lastMessage`.
    func importGdtf(path: String) {
        lastMessage = nil
        sendJSON("library.import_gdtf", ["path": path])
    }

    /// IPC-39: an ordinary programmer value for the selected fixtures that have the attribute.
    func programmerSet(attribute: String, value: Double) {
        sendJSON("programmer.set", ["attribute": attribute, "value": value])
    }

    private func acceptEditorProfile(_ data: Data) {
        guard let ask = editorAsk else { return }
        guard let reply = try? JSONDecoder().decode(DocReply.self, from: data) else {
            lastMessage = "Could not read the profile"
            return
        }
        let key = reply.profileKey ?? "\(reply.profile.manufacturer)/\(reply.profile.model)"
        guard key == ask else { return }
        editorAsk = nil
        editorProfile = EditorProfile(key: key, doc: reply.profile)
    }

    /// `address` / `number` nil = omitted, the engine picks (PAT-03).
    func patchAdd(profile: String, mode: String, count: Int, universe: Int, address: Int?, number: Int?, autoGroups: Bool = false) {
        var fields: [String: Any] = ["profile": profile, "mode": mode, "count": count, "universe": universe]
        if let address { fields["address"] = address }
        if let number { fields["number"] = number }
        if autoGroups { fields["auto_groups"] = true }
        patch("patch.add", fields)
    }
    func patchMove(ids: [Int], universe: Int, address: Int, parkConflicting: Bool = false) {
        var fields: [String: Any] = ["ids": ids, "universe": universe, "address": address]
        if parkConflicting { fields["park_conflicting"] = true }
        patch("patch.move", fields)
    }
    func patchPark(ids: [Int]) { patch("patch.park", ["ids": ids]) }
    func patchUnpark(ids: [Int]) { patch("patch.unpark", ["ids": ids]) }
    func patchRemove(ids: [Int]) { patch("patch.remove", ["ids": ids]) }
    func patchRenumber(ids: [Int], number: Int) { patch("patch.renumber", ["ids": ids, "number": number]) }
    func patchName(ids: [Int], name: String) { patch("patch.name", ["ids": ids, "name": name]) }
    func patchChangeMode(ids: [Int], mode: String) { patch("patch.change_mode", ["ids": ids, "mode": mode]) }

    /// PTC-01: universe and address go along only when both are given (else the engine keeps each fixture's own).
    func patchExchange(ids: [Int], profile: String, mode: String, universe: Int?, address: Int?) {
        var fields: [String: Any] = ["ids": ids, "profile": profile, "mode": mode]
        if let universe, let address { fields["universe"] = universe; fields["address"] = address }
        patch("patch.exchange", fields)
    }

    func patchCopy(ids: [Int], number: Int?) {
        var fields: [String: Any] = ["ids": ids]
        if let number { fields["number"] = number }
        patch("patch.copy", fields)
    }

    /// PTC-07: only the fields given are sent; the engine keeps the rest.
    func patchBehaviour(ids: [Int], attribute: String, invert: Bool? = nil, low: Double? = nil, high: Double? = nil, offset: Double? = nil) {
        var fields: [String: Any] = ["ids": ids, "attribute": attribute]
        if let invert { fields["invert"] = invert }
        if let low { fields["low"] = low }
        if let high { fields["high"] = high }
        if let offset { fields["offset"] = offset }
        patch("patch.behaviour", fields)
    }

    func patchBehaviourReset(ids: [Int]) { patch("patch.behaviour_reset", ["ids": ids]) }

    func patchSwapPanTilt(ids: [Int], swapped: Bool) { patch("patch.swap_pan_tilt", ["ids": ids, "swapped": swapped]) }

    /// PAT-42: the engine fetches the library's copy of `profile` itself.
    func patchUpdateProfile(profile: String) { patch("patch.update_profile", ["profile": profile]) }

    /// LIB-41, LIB-42: a refusal comes back as an `error`, which lands in `lastMessage`.
    func libraryDeleteProfile(manufacturer: String, model: String) {
        lastMessage = nil
        sendJSON("library.delete_profile", ["manufacturer": manufacturer, "model": model])
    }

    func libraryDuplicateProfile(manufacturer: String, model: String, newModel: String) {
        lastMessage = nil
        sendJSON("library.duplicate_profile", ["manufacturer": manufacturer, "model": model, "new_model": newModel])
    }

    /// PAT-14, UI-77: moves a fixture to a handle (1-based page and slot); the engine swaps with the occupant.
    func patchHandle(id: Int, page: Int, slot: Int) { patch("patch.handle", ["id": id, "page": page, "slot": slot]) }
    func patchSwapAddresses(ids: [Int], with: [Int]) { patch("patch.swap_addresses", ["ids": ids, "with": with]) }

    // Palette commands (PAL-27). Numbers and cells, never ids; a refusal comes back as a warning.

    func recordPalette(window: String, handle: Int, mask: [String]? = nil, recordBy: String? = nil,
                       recordAs: String? = nil, mode: String? = nil) {
        var fields: [String: Any] = ["window": window, "handle": handle]
        if let mask { fields["mask"] = mask }
        if let recordBy { fields["record_by"] = recordBy }
        if let recordAs, recordAs != "Auto" { fields["record_as"] = recordAs }
        if let mode { fields["mode"] = mode }
        patch("palette.record", fields)
    }
    func applyPalette(_ number: Int, time: Double? = nil, overlap: Double? = nil) {
        var fields: [String: Any] = ["number": number]
        if let time { fields["time"] = time }
        if let overlap { fields["overlap"] = overlap }
        patch("palette.apply", fields)
    }
    func includePalette(_ number: Int) { patch("palette.include", ["number": number]) }
    /// PBK-48: a cue's own values and effects into the programmer; no cue = the playback's current one.
    func includePlayback(_ id: Int, cue: Double? = nil, mask: [String]? = nil) {
        var fields: [String: Any] = ["id": id]
        if let cue { fields["cue"] = cue }
        if let mask { fields["mask"] = mask }
        patch("playback.include", fields)
    }
    func copyPalettes(_ numbers: [Int], window: String, handle: Int, layout: String) {
        patch("palette.copy", ["numbers": numbers, "to": ["window": window, "handle": handle], "layout": layout])
    }
    func movePalettes(_ numbers: [Int], window: String, handle: Int, layout: String, swap: Bool) {
        patch("palette.move", ["numbers": numbers, "to": ["window": window, "handle": handle], "layout": layout, "swap": swap])
    }
    func renumberPalette(_ number: Int, to: Int) { patch("palette.renumber", ["number": number, "to": to]) }
    func deletePalettes(_ numbers: [Int]) { patch("palette.delete", ["numbers": numbers]) }
    /// `icon` nil = the automatic icon.
    func setPaletteLegend(_ number: Int, legend: String, icon: String?) {
        patch("palette.legend", ["number": number, "legend": legend, "icon": icon.map { $0 as Any } ?? NSNull()])
    }
    /// `settings.palette`: master fade time (s) and/or overlap (%). The `palette_settings` delta
    /// shows the result.
    func setPaletteMaster(time: Double? = nil, overlap: Double? = nil) {
        var fields: [String: Any] = [:]
        if let time { fields["time"] = time }
        if let overlap { fields["overlap"] = overlap }
        patch("settings.palette", fields)
    }

    // Playback commands (PBK-36). Ids, and pages and slots for handles; a refusal comes back as a warning.

    /// `mode` is Replace, Merge, Update, Append or Insert; `cue` records onto that cue number
    /// (CUE-04, not with Append / Insert); `updateMode` is Forwards, CueOnly, Backwards or Both (CUE-52).
    func recordPlayback(page: Int, slot: Int, mask: [String]? = nil, recordBy: String? = nil, mode: String? = nil,
                        cue: Double? = nil, updateMode: String? = nil, kind: String? = nil) {
        var fields: [String: Any] = ["page": page, "slot": slot]
        if let kind { fields["kind"] = kind }
        if let mask { fields["mask"] = mask }
        if let recordBy { fields["record_by"] = recordBy }
        if let mode { fields["mode"] = mode }
        if let cue { fields["cue"] = cue }
        if let updateMode { fields["update_mode"] = updateMode }
        patch("playback.record", fields)
    }
    /// IPC-26: `kind` is Bpm or Rate; the engine picks the master's number.
    func recordMaster(page: Int, slot: Int, kind: String, group: Int? = nil) {
        var fields: [String: Any] = ["page": page, "slot": slot, "kind": kind]
        if let group { fields["group"] = group }
        patch("master.record", fields)
    }
    /// GRM-21: point a group master at another group.
    func setMasterGroup(id: Int, group: Int) { patch("master.group", ["id": id, "group": group]) }
    /// Press and hold: `on: true` on press, `false` on release (PBK-37).
    func flashPlayback(_ id: Int, on: Bool) { sendJSON("playback.flash", ["id": id, "on": on]) }
    func swopPlayback(_ id: Int, on: Bool) { sendJSON("playback.swop", ["id": id, "on": on]) }
    func latchPlayback(_ id: Int) { sendJSON("playback.latch", ["id": id]) }
    func releasePlayback(_ id: Int) { sendJSON("playback.release", ["id": id]) }
    func deletePlaybacks(_ ids: [Int]) { patch("playback.delete", ["ids": ids]) }
    func copyPlaybacks(_ ids: [Int], page: Int, slot: Int, layout: String) {
        patch("playback.copy", ["ids": ids, "to": ["page": page, "slot": slot], "layout": layout])
    }
    func movePlaybacks(_ ids: [Int], page: Int, slot: Int, layout: String, swap: Bool) {
        patch("playback.move", ["ids": ids, "to": ["page": page, "slot": slot], "layout": layout, "swap": swap])
    }
    /// `playback.options` (PBK-36): only the given fields change; `NSNull()` clears a nullable one.
    func setPlaybackOptions(_ id: Int, _ fields: [String: Any]) { patch("playback.options", fields.merging(["id": id]) { a, _ in a }) }
    /// `playback.times`: delay, delay_out, fade_in, fade_out (s) and overlap (%) of the first cue; `fade_out` and `delay_out` also take NSNull for As In (CUE-56).
    func setPlaybackTimes(_ id: Int, _ fields: [String: Any]) { patch("playback.times", fields.merging(["id": id]) { a, _ in a }) }
    /// `icon` and `halo` nil = automatic (PBK-31, PBK-33).
    func setPlaybackLegend(_ id: Int, legend: String, icon: String?, halo: AutoIcon.RGB?) {
        patch("playback.legend", ["id": id, "legend": legend, "icon": icon.map { $0 as Any } ?? NSNull(),
                                  "halo": halo.map { ["r": $0.r, "g": $0.g, "b": $0.b] as [String: Any] } ?? NSNull()])
    }
    // Cue list commands (CUE-41). Without an `id` the connected list acts (CUE-27); a refusal comes
    // back as a warning.

    /// `action` is go, back, stop, goto, next or connect (connect needs an id; goto and next a cue).
    func cuelist(_ action: String, id: Int? = nil, cue: Double? = nil) {
        var fields: [String: Any] = [:]
        if let id { fields["id"] = id }
        if let cue { fields["cue"] = cue }
        patch("cuelist.\(action)", fields)
    }
    /// `playback.release` without an id releases the connected list.
    func releaseConnected() { sendJSON("playback.release") }
    /// PBK-52: `scope` is all, page or other_pages; the two page scopes need `page`. The engine counts and says it.
    func releaseScope(_ scope: String, page: Int? = nil) {
        sendJSON("playback.release", page.map { ["scope": scope, "page": $0] } ?? ["scope": scope])
    }
    /// CUE-55: `which` is upper or lower; the engine does what `handle_buttons` assigns. Held
    /// functions follow `on`, the others act on the press.
    func pressButton(_ id: Int, _ which: String, on: Bool) { sendJSON("playback.button", ["id": id, "button": which, "on": on]) }
    func tapPlayback(_ id: Int) { sendJSON("playback.tap", ["id": id]) }
    func setBPM(_ id: Int, _ bpm: Double) { patch("playback.bpm", ["id": id, "bpm": bpm]) }
    /// `fields` as `cue.times`: delay, delay_out, fade_in, fade_out (the out times also NSNull for As In, CUE-56), overlap, link, group_times (CUE-07, 14, 20).
    func setCueTimes(_ id: Int, cue: Double, _ fields: [String: Any]) {
        patch("cue.times", fields.merging(["id": id, "cue": cue]) { a, _ in a })
    }
    /// IPC-56: the same fields to every cue in `cues`, one undo step.
    func setCueTimes(_ id: Int, cues: [Double], _ fields: [String: Any]) {
        patch("cue.times", fields.merging(["id": id, "cues": cues]) { a, _ in a })
    }
    /// UI-34, PRG-81: item → 0-based step; items left out follow after the given ones (the engine does that).
    func setCueOrder(_ id: Int, cue: Double, order: [Item: Int]) {
        let wire = Dictionary(uniqueKeysWithValues: order.map { item, step in
            (item.cell.map { "\(item.fixture).\($0)" } ?? "\(item.fixture)", step)
        })
        patch("cue.order", ["list": id, "cue": cue, "order": wire])
    }
    func setCueLegend(_ id: Int, cue: Double, legend: String) { patch("cue.legend", ["id": id, "cue": cue, "legend": legend]) }
    func setCueTrack(_ id: Int, cue: Double, track: String) { patch("cue.track", ["id": id, "cue": cue, "track": track]) }
    func deleteCues(_ id: Int, cues: [Double]) { patch("cue.delete", ["id": id, "cues": cues]) }
    func renumberCue(_ id: Int, cue: Double, to: Double) { patch("cue.renumber", ["id": id, "cue": cue, "to": to]) }
    func renumberAllCues(_ id: Int) { patch("cue.renumber_all", ["id": id]) }
    func copyCue(_ id: Int, cue: Double, to: Double) { patch("cue.copy", ["id": id, "cue": cue, "to": to]) }
    /// CUE-54: the reply lands in `cueView`.
    func viewCue(_ id: Int, cue: Double) { sendJSON("cue.view", ["id": id, "cue": cue]) }
    func removeCueValue(_ id: Int, cue: Double, fixture: Int, cell: Int? = nil, attribute: String?) {
        // IPC-42: an integer for the whole fixture, "<id>.<cell>" for one cell.
        var fields: [String: Any] = ["id": id, "cue": cue, "fixture": cell.map { "\(fixture).\($0)" as Any } ?? fixture]
        if let attribute { fields["attribute"] = attribute }
        patch("cue.remove", fields)
    }
    /// CUE-61: moves a hard value (or all of an item's) off the cue's output, or back on.
    func setCueOff(_ id: Int, cue: Double, fixture: Int, cell: Int? = nil, attribute: String?, off: Bool) {
        var fields: [String: Any] = ["id": id, "cue": cue, "fixture": cell.map { "\(fixture).\($0)" as Any } ?? fixture, "off": off]
        if let attribute { fields["attribute"] = attribute }
        patch("cue.off", fields)
    }
    /// CUE-59: takes a hard effect entry out of the cue.
    func removeCueEffect(_ id: Int, cue: Double, entry: Int) { patch("cue.remove_effect", ["id": id, "cue": cue, "entry": entry]) }
    /// `settings.playback` (show-wide): only the given fields change.
    func setPlaybackSettings(_ fields: [String: Any]) { patch("settings.playback", fields) }

    func setPageLegend(_ page: Int, legend: String) { patch("page.legend", ["page": page, "legend": legend]) }
    /// `dbo.set` (PBK-41); the engine's `dbo` delta shows the result.
    func setDBO(_ on: Bool) { sendJSON("dbo.set", ["on": on]) }

    // MARK: Outputs (OUT-33..35): the engine fills defaults and refuses bad edits with a warning.
    func addOutput(line: Int, kind: String) { patch("output.add", ["line": line, "kind": kind]) }
    /// `NSNull()` as a value clears it (e.g. `unicast` back to multicast).
    func editOutput(_ id: Int, _ fields: [String: Any]) { patch("output.edit", fields.merging(["id": id]) { a, _ in a }) }
    func removeOutputs(ids: [Int]) { patch("output.remove", ["ids": ids]) }
    func listPorts() { sendJSON("output.ports") }

    // MARK: Control (CTL-40): the engine owns every rule; a refusal arrives as a warning.
    func controlEnable(_ device: String, on: Bool) { patch("control.enable", ["device": device, "on": on]) }
    func controlAssign(_ device: String, mapping: String) { patch("control.assign", ["device": device, "mapping": mapping]) }
    func controlOrder(_ devices: [String]) { patch("control.order", ["devices": devices]) }
    /// Arms Learn for `action`; the next input of an enabled device (or OSC message) is bound to it.
    /// No `mapping`: the input lands in its own device's mapping, or the OSC mapping (CTL-25, 35).
    func controlLearn(_ action: String) { sendJSON("control.learn", ["action": action]) }
    func controlLearnCancel() { sendJSON("control.learn_cancel") }
    /// `fields` are the changed ones (`action`, `invert`, `pickup`, …); `input` is the binding's own JSON.
    func controlBinding(_ mapping: String, input: Any, _ fields: [String: Any]) {
        patch("control.binding", fields.merging(["mapping": mapping, "input": input]) { a, _ in a })
    }
    /// `deckPage` picks among the pages of a deck key (CTL-67); nil = the daemon's default, page 1.
    func controlUnbind(_ mapping: String, input: Any, deckPage: Int? = nil) {
        var fields: [String: Any] = ["mapping": mapping, "input": input]
        if let deckPage { fields["deck_page"] = deckPage }
        patch("control.unbind", fields)
    }
    /// CTL-71: removes every deck key of one page in one write.
    func controlDeckClear(_ mapping: String, page: Int) { patch("control.deck_clear", ["mapping": mapping, "deck_page": page]) }
    /// CTL-53, 67: a Stream Deck's brightness 0...100, or the deck page it shows.
    func controlDeck(_ device: String, brightness: Int) { patch("control.deck", ["device": device, "brightness": brightness]) }
    func controlDeck(_ device: String, page: Int) { patch("control.deck", ["device": device, "page": page]) }
    func controlMappingNew(_ name: String, device: String) { patch("control.mapping_new", ["name": name, "device": device]) }
    func controlMappingRename(_ name: String, to: String) { patch("control.mapping_rename", ["name": name, "to": to]) }
    func controlMappingDuplicate(_ name: String, to: String) { patch("control.mapping_duplicate", ["name": name, "to": to]) }
    func controlMappingDevice(_ name: String, device: String) { patch("control.mapping_device", ["name": name, "device": device]) }
    func controlMappingDelete(_ name: String) { patch("control.mapping_delete", ["name": name]) }
    func controlMappingImport(json: String, replace: Bool = false) {
        patch("control.mapping_import", replace ? ["json": json, "replace": true] : ["json": json])
    }
    /// Only the given settings change; a bad value refuses the whole command.
    func controlOsc(_ fields: [String: Any]) { patch("control.osc", fields) }
    /// Shows this mapping's bindings; the reply replaces `controlMapping`.
    func showMapping(_ name: String?) {
        shownMapping = name
        if name == nil { controlMapping = nil } else if controlMapping?.name != name { controlMapping = nil }
        if let name { sendJSON("control.mapping", ["name": name]) }
    }

    /// UI-56: the group masters that hold their group, by page and slot.
    var holdingMasters: [PlaybackInfo] {
        playbacks.filter { $0.live.holding == true }.sorted { ($0.stored.page, $0.stored.slot) < ($1.stored.page, $1.stored.slot) }
    }

    /// UI-56: what the GRP tag says for one holding master.
    func holdingText(_ p: PlaybackInfo) -> String {
        "\(p.stored.legend.isEmpty ? "Master" : p.stored.legend) (\(p.homeHandle)) holds Group \(p.stored.master?.group ?? 0) at \(percent(p.masterLevel))"
            + (p.locked ? ", locked" : "")
    }

    /// A hint for the operator in the message zone (the app's own, not an engine warning).
    func say(_ text: String) { lastMessage = text; armedMessage = isArmed() ? text : nil }

    /// UI-55: the copy inside the app (REL-29), else the website (a local build). Changes nothing on stage.
    func openManual() {
        guard !locked else { return }
        let url = Bundle.main.url(forResource: "index", withExtension: "html", subdirectory: "manual")
            ?? URL(string: "https://aottens.github.io/CMDMX/manual/")
        guard let url, NSWorkspace.shared.open(url) else { say("The manual could not be opened."); return }
    }
    /// UI-50: set by the Frame; whether a function is armed when the app says something.
    var isArmed: () -> Bool = { false }
    /// UI-64: counts warnings and errors, so a send can tell whether the engine refused it. Not published.
    private(set) var refusals = 0
    /// UI-50: the text the app said because a function was armed.
    private var armedMessage: String?
    /// UI-50: the function disarmed. Comparing the text means an engine message, or a later one that replaced it, stays.
    func endArmedMessage() { if lastMessage == armedMessage { lastMessage = nil }; armedMessage = nil }
    /// UI-49: Keys off and a key it would act on (or a click on the command line); not in the Messages sheet, so not `addMessage`.
    func sayKeysOff() { lastMessage = "The Mac keyboard is off: turn on Keys or Keypad in the status bar." }

    func askUsage(fixtures: [Int]) { usage = nil; sendJSON("usage", ["fixtures": fixtures]) }

    /// UI-57: asked again every second while Fixture info is open, so the answer is not cleared between asks.
    func askWhy(fixture: Int) { sendJSON("why", ["fixture": fixture]) }
    func askUsageDetail(fixture: Int) { usageDetail = nil; sendJSON("usage", ["fixtures": [fixture], "detail": true]) }

    /// PBK-49, IPC-57: the reply lands in `updateTargets`.
    func askUpdateTargets() { updateTargets = nil; sendJSON("update.targets", [:]) }
    func update(playbacks: [(Int, Double)], palettes: [Int]) {
        patch("update", ["playbacks": playbacks.map { ["id": $0.0, "cue": $0.1] as [String: Any] }, "palettes": palettes])
    }

    /// UNDO-19: confirm with the label the key showed; a refusal arrives as a warning and overwrites.
    /// UI-79: while the history may hold work from before this app opened, the first Undo asks twice.
    func undo() {
        if undoEarlier, let label = undoLabel, undoAsked != label {
            undoAsked = label
            say("Undo \(label), done before this app opened? Undo again to go ahead")
            return
        }
        undoAsked = nil
        lastMessage = undoLabel.map { "Undone: \($0)" }; sendJSON("undo")
    }
    /// UI-80: a notice goes when the workspace or the screen changes; a warning or error stays.
    func endNotice() { if lastMessage == noticeText { lastMessage = nil }; noticeText = nil }
    func redo() { lastMessage = redoLabel.map { "Redone: \($0)" }; sendJSON("redo") }

    // Show commands (SHW-15). The engine checks names and refuses with a warning.

    func listShows() { sendJSON("show.list") }
    /// SHW-25: the daemon writes the report and answers its path.
    func showReport() { sendJSON("show.report") }
    /// `keep`: nil (empty), "patch" or "looks" (SHW-19/20).
    func newShow(_ name: String, keep: String? = nil) {
        patch("show.new", keep.map { ["name": name, "keep": $0] } ?? ["name": name])
    }
    func openShow(_ name: String, keepOutput: Bool) {
        var args: [String: Any] = ["name": name]
        if keepOutput { args["output"] = "keep" }
        patch("show.open", args)
    }
    func saveShowAs(_ name: String) { patch("show.save_as", ["name": name]) }
    /// SHW-21: a copy under a new name; the open show stays open.
    func saveShowCopy(_ name: String) { patch("show.save_copy", ["name": name]) }
    func renameShow(_ name: String, to: String) { patch("show.rename", ["name": name, "to": to]) }
    func deleteShow(_ name: String) { patch("show.delete", ["name": name]) }

    /// Hides the last engine message (e.g. when the operator selects another row).
    func clearMessage() { lastMessage = nil }
    /// UI-38: the engine checks the code; a wrong one comes back as a warning.
    func setLock(_ on: Bool, code: String) { lastMessage = nil; sendJSON("lock.set", ["on": on, "code": code]) }

    /// RMT-15: the engine validates (1-64 characters, port 1024-65535) and refuses with a message.
    func setRemoteLogin(username: String, password: String, port: Int) {
        patch("remote.set_login", ["username": username, "password": password, "port": port])
    }
    func remoteOff() { patch("remote.off", [:]) }

    private func patch(_ cmd: String, _ fields: [String: Any]) {
        lastMessage = nil
        sendJSON(cmd, fields)
    }

    /// The app's own reads, not the operator's commands (REL-27).
    private static let asks: Set<String> = ["cue.view", "usage", "why", "update.targets", "show.list", "show.report", "library.manufacturers", "library.models", "library.profile", "library.gels", "patch.profile", "profile.emitter", "output.ports", "output.watch", "control.mapping", "effect.wheels"]

    private func sendJSON(_ cmd: String, _ fields: [String: Any] = [:]) {
        guard let socket,
              let data = try? JSONSerialization.data(withJSONObject: fields.merging(["v": Self.protocolVersion, "cmd": cmd]) { a, _ in a }),
              let text = String(data: data, encoding: .utf8) else { return }
        if !Self.asks.contains(cmd) && cmd != "undo" && cmd != "redo" { editSent = true; undoAsked = nil }   // UI-79
        if !Self.asks.contains(cmd) { DaemonLauncher.shared.commandGiven() }   // REL-27
        socket.send(.string(text)) { error in
            if error != nil { socket.cancel(with: .goingAway, reason: nil) }
        }
    }

    private struct FaderSet: Encodable {
        let v = EngineClient.protocolVersion
        let cmd = "fader.set"
        let playback: Int
        let level: Double
    }

    private struct GrandMasterSet: Encodable {
        let v = EngineClient.protocolVersion
        let cmd = "grand_master.set"
        let level: Double
    }

    /// Fire and forget; a failed send cancels the socket, which triggers the reconnect path.
    private func send(_ command: some Encodable) {
        guard let socket,
              let data = try? JSONEncoder().encode(command),
              let text = String(data: data, encoding: .utf8) else { return }
        DaemonLauncher.shared.commandGiven()   // REL-27
        socket.send(.string(text)) { error in
            if error != nil { socket.cancel(with: .goingAway, reason: nil) }
        }
    }

    // MARK: - Connection (IPC-08)

    /// One connection attempt after another, 1 s apart, for the life of the app.
    private func connectForever() async {
        while !Task.isCancelled {
            await runOneConnection()
            socket = nil
            if connection == .online {
                addMessage(level: "warning", message: "Engine offline", at: Date())
                DaemonLauncher.shared.engineLost()   // REL-27
            }
            connection = .offline
            outputs = []; live.grid = [:]; live.dmx = nil
            control = ControlState(); controlMapping = nil; remote = RemoteState()
            try? await Task.sleep(nanoseconds: 1_000_000_000)
        }
    }

    /// Returns when the socket fails or closes. No `Origin` header is set (IPC-02).
    private func runOneConnection() async {
        guard let url else { return }
        connection = .connecting
        let task = URLSession.shared.webSocketTask(with: url)
        socket = task
        task.resume()
        do {
            while true {
                handle(try await task.receive())
            }
        } catch {
            task.cancel(with: .goingAway, reason: nil)
        }
    }

    // MARK: - Messages (engine → app)

    private struct WireMessage: Decodable { let at: String?; let level: String; let message: String }

    /// Union of all server message shapes; unused fields stay nil.
    private struct Incoming: Decodable {
        let type: String
        let v: Int?
        fileprivate let page: Tolerant<PageState>?
        let fadersPerPage: Int?
        let grandMaster: Double?
        fileprivate let playbacks: Tolerant<[Tolerant<PlaybackEntry>]>?
        fileprivate let pages: Tolerant<[PageLegend]>?
        let dbo: Bool?
        let on: Bool?
        let locked: Bool?
        fileprivate let playbackSettings: Tolerant<PlaybackSettings>?
        let level: Double?
        let message: String?
        let at: String?
        /// IPC-51: the snapshot's history.
        fileprivate let messages: Tolerant<[Tolerant<WireMessage>]>?
        fileprivate let fixtures: Tolerant<[FixtureInfo]>?   // a `cue.view` reply has other fixtures
        fileprivate let groups: Tolerant<[GroupInfo]>?   // a `usage` reply has a count
        /// The snapshot's `connected` (an id or null) and the `connected` delta's `id`.
        fileprivate let connected: Tolerant<Int>?
        fileprivate let id: Tolerant<Int>?
        fileprivate let palettes: Tolerant<[Tolerant<PaletteInfo>]>?
        fileprivate let paletteSettings: Tolerant<PaletteSettings>?
        fileprivate let programmerSettings: Tolerant<ProgrammerSettings>?
        let programmer: ProgrammerState?
        fileprivate let effectCatalogue: Tolerant<EffectCatalogue>?
        fileprivate let outputs: Tolerant<[OutputNode]>?
        fileprivate let control: Tolerant<ControlState>?
        fileprivate let ports: Tolerant<[Tolerant<SerialPort>]>?
        fileprivate let show: Tolerant<ShowState>?
        fileprivate let remote: Tolerant<RemoteState>?
        fileprivate let shows: Tolerant<[Tolerant<ShowEntry>]>?
        fileprivate let dir: String?
        fileprivate let path: String?

        enum CodingKeys: String, CodingKey {
            case type, v, page, pages, playbacks, dbo, on, locked, level, message, at, messages, fixtures, groups, connected, id, palettes, programmer, outputs, control, ports, show, shows, dir, path, remote
            case fadersPerPage = "faders_per_page"
            case grandMaster = "grand_master"
            case paletteSettings = "palette_settings"
            case programmerSettings = "programmer_settings"
            case playbackSettings = "playback_settings"
            case effectCatalogue = "effect_catalogue"
        }
    }

    private func handle(_ message: URLSessionWebSocketTask.Message) {
        guard case .string(let text) = message,
              case let data = Data(text.utf8),
              let incoming = try? JSONDecoder().decode(Incoming.self, from: data)
        else { return }

        // IPC-41: after a version mismatch nothing but a matching snapshot is applied.
        if incoming.type != "snapshot", protocolMismatch != nil { return }
        switch incoming.type {
        case "snapshot":
            if incoming.v != Self.protocolVersion {
                protocolMismatch = incoming.v.map { "v\($0)" } ?? "no version"
                let task = socket
                socket = nil
                task?.cancel(with: .goingAway, reason: nil)
                return
            }
            protocolMismatch = nil
            releaseTasks.values.forEach { $0.cancel() }; releaseTasks = [:]
            heldFaders = [:]; settling = []; lastSent = [:]
            gmHeld = false; gmSettling = false; gmLastSent = nil; gmEngineLevel = nil
            fadersPerPage = max(1, incoming.fadersPerPage ?? fadersPerPage)
            page = incoming.page?.value ?? PageState()
            playbackStored = [:]; playbackLive = [:]
            storePlaybackList(incoming.playbacks?.value)
            pages = incoming.pages?.value ?? []
            dbo = incoming.dbo ?? false
            locked = incoming.locked ?? false
            playbackSettings = incoming.playbackSettings?.value
            connected = incoming.connected?.value
            cueView = nil
            usage = nil
            updateTargets = nil
            grandMaster = (incoming.grandMaster ?? grandMaster).clamped01
            fixtures = incoming.fixtures?.value ?? []
            groups = incoming.groups?.value ?? []
            palettes = (incoming.palettes?.value ?? []).compactMap(\.value)
            paletteSettings = incoming.paletteSettings?.value
            programmerSettings = incoming.programmerSettings?.value
            programmer = incoming.programmer ?? ProgrammerState()
            effectCatalogue = incoming.effectCatalogue?.value ?? EffectCatalogue()
            if effectWheelsWanted { sendJSON("effect.wheels", ["on": true]) }   // FX-17: a new daemon starts with them off
            sendJSON("library.gels", ["range": "Lee"])   // IPC-47
            outputs = incoming.outputs?.value ?? []
            control = incoming.control?.value ?? ControlState()
            if let name = shownMapping { sendJSON("control.mapping", ["name": name]) }
            if let show = incoming.show?.value { self.show = show }
            remote = incoming.remote?.value ?? RemoteState()
            let labels = (try? JSONDecoder().decode(SnapshotUndo.self, from: data))?.undo?.value
            undoLabel = labels?.undo; redoLabel = labels?.redo
            let entries = (try? JSONDecoder().decode(OutputEntries.self, from: data))?.output ?? []
            live.set(entries.compactMap(\.value), replacing: true)
            // UI-39: merge, the app keeps earlier lines across a daemon restart; known lines are skipped.
            var known = Set(messageLog.map { "\($0.at.timeIntervalSince1970)|\($0.level)|\($0.message)" })
            var added = false
            for m in (incoming.messages?.value ?? []).compactMap(\.value) {
                let at = Self.messageDate(m.at)
                guard known.insert("\(at.timeIntervalSince1970)|\(m.level)|\(m.message)").inserted else { continue }
                addMessage(level: m.level, message: m.message, at: at); added = true
            }
            if added { messageLog.sort { $0.at < $1.at } }
            if connection != .online { addMessage(level: "notice", message: "Engine connected", at: Date()) }
            connection = .online
            DaemonLauncher.shared.engineConnected(show: incoming.show?.value?.name)   // REL-26
            listShows()
        case "page":
            // PBK-03: the delta carries the page fields at top level.
            if let state = try? JSONDecoder().decode(PageState.self, from: data) { page = state; rebuildSlots() }
        case "undo":
            if let labels = try? JSONDecoder().decode(UndoLabels.self, from: data) { undoLabel = labels.undo; redoLabel = labels.redo }
            // ponytail: the delta has no author, so a non-edit command from this app followed by another client's edit also
            // counts; asking twice stays the safe side when an own edit leaves the label unchanged (no delta). Not reset on reconnect.
            if editSent { ownEdit = true }
        case "programmer":
            // IPC-12: the delta carries the IPC-11 fields at top level.
            if let state = try? JSONDecoder().decode(ProgrammerState.self, from: data) { programmer = state }
        case "fixtures":
            if let fixtures = incoming.fixtures?.value { self.fixtures = fixtures }
        case "output":
            // IPC-58: changed entries only, merged; an entry is never removed by a delta.
            for e in ((try? JSONDecoder().decode(OutputEntries.self, from: data))?.fixtures ?? []).compactMap(\.value) { live.set([e]) }
        case "output_grid":
            guard watching == .grid, let msg = try? JSONDecoder().decode(GridMessage.self, from: data) else { break }
            for e in msg.fixtures.compactMap(\.value) { live.grid[e.id] = e.values.compactMapValues(\.value) }   // each changed row comes whole
        case "output_dmx":
            if let msg = try? JSONDecoder().decode(DMXMessage.self, from: data), watching == .dmx(msg.line) { live.dmx = (msg.line, msg.channels) }
        case "outputs":
            if let outputs = incoming.outputs?.value { self.outputs = outputs }
        case "control":
            // CTL-39: the delta carries the section at top level. The shown mapping is asked for again
            // (Learn and edits change it), except after a delta that is only a new last input.
            if let state = try? JSONDecoder().decode(ControlState.self, from: data) {
                var rest = state; rest.lastInput = control.lastInput
                let onlyInput = rest == control && state.lastInput != control.lastInput
                control = state
                // A renamed or deleted mapping is not asked for: the screen moves on to the new name.
                if !onlyInput, let name = shownMapping, state.mappings.contains(where: { $0.name == name }) {
                    sendJSON("control.mapping", ["name": name])
                }
            }
        case "control.mapping":
            if let doc = ControlMappingDoc(data), doc.name == shownMapping { controlMapping = doc }
        case "ports":
            if let ports = incoming.ports?.value { serialPorts = ports.compactMap(\.value) }
        case "show":
            // SHW-15: the delta carries the show fields at top level. A switch or save changes the
            // list too, so ask again.
            if let state = try? JSONDecoder().decode(ShowState.self, from: data) { show = state }
            listShows()
        case "remote":
            // RMT-15: the delta carries the whole section under `remote`.
            if let state = incoming.remote?.value { remote = state }
        case "show.list":
            if let shows = incoming.shows?.value { self.shows = shows.compactMap(\.value) }
            showsDir = incoming.dir
        case "show.report":
            if let path = incoming.path, NSWorkspace.shared.open(URL(fileURLWithPath: path)) { break }
            say("The report could not be opened.")
        case "library.manufacturers":
            if let reply = try? JSONDecoder().decode(ManufacturersReply.self, from: data),
               trimmedOrNil(reply.query) == lastManufacturersQuery { libraryManufacturers = reply.items }
        case "library.models":
            if let reply = try? JSONDecoder().decode(ModelsReply.self, from: data),
               [trimmedOrNil(reply.manufacturer), trimmedOrNil(reply.query)] == lastModelsAsk {
                libraryModels = reply.items
                libraryModelsTruncated = reply.truncated ?? false
            }
        case "patch.profile":
            if let reply = try? JSONDecoder().decode(ProfileReply.self, from: data) { profileCache[reply.profileKey] = reply.profile }
            if let doc = try? JSONDecoder().decode(DocReply.self, from: data) {
                profileDocs[doc.profileKey ?? "\(doc.profile.manufacturer)/\(doc.profile.model)"] = doc.profile
            }
            acceptEditorProfile(data)
        case "library.profile":
            acceptEditorProfile(data)
        case "library.gels":
            if let reply = try? JSONDecoder().decode(GelsReply.self, from: data) { gels = reply.gels }
        case "profile.emitter":
            if let info = try? JSONDecoder().decode(EmitterInfo.self, from: data) { emitterInfo[info.name] = info }
        case "library.saved":
            if let saved = try? JSONDecoder().decode(SavedReply.self, from: data) {
                profileSaved = ProfileSaved(manufacturer: saved.manufacturer, model: saved.model, token: (profileSaved?.token ?? 0) + 1)
            }
        case "library.deleted":
            if let gone = try? JSONDecoder().decode(SavedReply.self, from: data) {
                profileDeleted = ProfileSaved(manufacturer: gone.manufacturer, model: gone.model, token: (profileDeleted?.token ?? 0) + 1)
            }
        case "library.imported":
            if var done = try? JSONDecoder().decode(GdtfImported.self, from: data) {
                done.token = (gdtfImported?.token ?? 0) + 1
                gdtfImported = done
            }
        case "groups":
            if let groups = incoming.groups?.value { self.groups = groups }
        case "palette_settings":
            // The delta carries the settings at top level.
            if let state = try? JSONDecoder().decode(PaletteSettings.self, from: data) { paletteSettings = state }
        case "programmer_settings":
            if let state = try? JSONDecoder().decode(ProgrammerSettings.self, from: data) { programmerSettings = state }
        case "palettes":
            if let list = incoming.palettes?.value { palettes = list.compactMap(\.value) }
        case "playback":
            // PBK-35: the small live delta; the stored part stays.
            if let delta = try? JSONDecoder().decode(PlaybackLiveDelta.self, from: data), playbackStored[delta.id] != nil {
                storeLive(id: delta.id, delta.live)
                scheduleRebuild()
            }
        case "playbacks":
            // PBK-35: the whole list after a stored change; a playback that is gone disappears.
            storePlaybackList(incoming.playbacks?.value)
        case "pages":
            if let list = incoming.pages?.value { pages = list }
        case "connected":
            connected = incoming.id?.value
        case "cue.view":
            if let reply = try? JSONDecoder().decode(CueViewReply.self, from: data) { cueView = reply }
        case "update.targets":
            if let reply = try? JSONDecoder().decode(UpdateTargetsAnswer.self, from: data) { updateTargets = reply }
        case "why":
            if let reply = try? JSONDecoder().decode(WhyReply.self, from: data) { why = reply }
        case "usage":
            // UI-57: the detail answer has lists where the plain one has counts.
            if let detail = try? JSONDecoder().decode(UsageDetail.self, from: data) { usageDetail = detail; break }
            if let reply = try? JSONDecoder().decode(UsageReply.self, from: data) {
                usage = reply
                if let p = pendingDelete, p.palette == reply.palette {
                    if lastMessage == p.shown {
                        let what = p.shown.dropLast(" Confirm on softkey A.".count)
                        let used = reply.cues == 0 ? "No cue uses it."
                            : "\(reply.cues) \(reply.cues == 1 ? "cue" : "cues") on \(reply.playbacks) \(reply.playbacks == 1 ? "playback" : "playbacks") \(reply.cues == 1 ? "uses" : "use") it and \(reply.cues == 1 ? "keeps" : "keep") its values."
                        lastMessage = "\(what) \(used) Confirm on softkey A."
                        if armedMessage == p.shown { armedMessage = lastMessage }   // UI-50: still the function's own
                    }
                    pendingDelete = nil
                }
            }
        case "dbo":
            if let on = incoming.on { dbo = on }
        case "playback_settings":
            if let state = try? JSONDecoder().decode(PlaybackSettings.self, from: data) { playbackSettings = state }
        case "locked":
            if let on = incoming.locked { locked = on }
        case "grand_master":
            if let level = incoming.level {
                if gmHeld { gmEngineLevel = level.clamped01; releaseGMIfEchoed() } else { grandMaster = level.clamped01 }
            }
        case "notice", "warning", "error": // PRG-96: a notice is informational; UI-39: all three go to the Messages list
            lastMessage = incoming.message
            if incoming.type == "notice" { noticeText = incoming.message }   // UI-80
            addMessage(level: incoming.type, message: incoming.message ?? "", at: Self.messageDate(incoming.at))
            if incoming.type != "notice" { refusals += 1; print("engine \(incoming.type): \(incoming.message ?? "")") }
        default:
            break
        }
    }

    /// Keeps the engine's live values per playback id. A held fader parks only the level (UI-22);
    /// the state is always shown as the engine reports it, so a mode-1 fade-out stays green.
    private func storeLive(id: Int, _ new: PlaybackLive) {
        var state = playbackLive[id] ?? PlaybackLive()
        state.flash = new.flash; state.latched = new.latched; state.fired = new.fired
        state.state = new.state
        state.cue = new.cue; state.bpm = new.bpm; state.multiplier = new.multiplier; state.holding = new.holding
        if heldFaders[id] != nil {
            heldFaders[id] = .some(new.level.clamped01)
        } else {
            state.level = new.level.clamped01
        }
        playbackLive[id] = state
        releaseIfEchoed(id)
    }

    /// Replaces the stored part of every playback (snapshot, `playbacks` delta).
    private func storePlaybackList(_ list: [Tolerant<PlaybackEntry>]?) {
        let entries = (list ?? []).compactMap(\.value)
        playbackStored = Dictionary(entries.map { ($0.stored.id, $0.stored) }, uniquingKeysWith: { _, last in last })
        playbackLive = playbackLive.filter { playbackStored[$0.key] != nil }
        for entry in entries { storeLive(id: entry.stored.id, entry.live) }
        storedRevision += 1
        rebuildSlots()
    }

    private var rebuildPending = false

    /// Live deltas come up to 40 Hz per running list or chase; the views redraw from them at most
    /// every 1/30 s (VS3-05 run: a running list and a chase kept the main thread at 100 % and the
    /// screen seconds behind the engine). ponytail: one ObservableObject still redraws every zone
    /// (~50 % CPU with a list and a chase running); split the live state per zone if that matters.
    private func scheduleRebuild() {
        guard !rebuildPending else { return }
        rebuildPending = true
        Task { @MainActor [weak self] in
            try? await Task.sleep(nanoseconds: 33_000_000)
            self?.rebuildPending = false
            self?.rebuildSlots()
        }
    }

    /// PBK-01/05: the strips show the current page's slots; the engine decides which playback each holds.
    private func rebuildSlots() {
        let playbacks = playbackStored.values.sorted { $0.id < $1.id }.map {
            PlaybackInfo(stored: $0, live: playbackLive[$0.id] ?? PlaybackLive())
        }
        let byId = Dictionary(playbacks.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
        let byNumber = Dictionary(page.slots.map { ($0.slot, $0.playback) }, uniquingKeysWith: { first, _ in first })
        livePlaybacks.sync(playbacks: playbacks, slots: (0..<fadersPerPage).map { index in
            PlaybackSlot(id: index, info: byNumber[index + 1].flatMap { $0 }.flatMap { byId[$0] })
        })
    }

    /// The playback with this id, if it still exists.
    func playback(_ id: Int) -> PlaybackInfo? { playbacks.first { $0.id == id } }
}

extension Double {
    var clamped01: Double { Swift.min(1, Swift.max(0, self)) }
}

/// UI-47: a delete says what it does before the confirmation.
extension EngineClient {
    func sayDelete(palette p: PaletteInfo) {
        let shown = "Delete palette \(p.number)\(p.legend.isEmpty ? "" : " " + p.legend)? Confirm on softkey A."
        say(shown)
        pendingDelete = (p.number, shown)
        sendJSON("usage", ["palette": p.number])
    }

    func sayDelete(playback info: PlaybackInfo) {
        var text = "Delete playback \(info.homeHandle)\(info.stored.legend.isEmpty ? "" : " " + info.stored.legend)?"
        if info.isRunning { text += " It is running: its output ends at once." }
        if info.stored.cues.count > 1 { text += " \(info.stored.cues.count) cues go with it." }
        say(text + " Confirm on softkey A.")
    }

    func sayDelete(cue: StoredCue, of list: PlaybackInfo) {
        var text = "Delete cue \(plain(cue.number))\(cue.legend.isEmpty ? "" : " " + cue.legend)?"
        if list.isRunning && list.live.cue?.current == cue.number { text += " It is the live cue: the output stays until the next Go." }
        say(text + " Confirm on softkey A.")
    }
}

/// UI-64: what the message zone says about the picks.
extension EngineClient {
    func sayPicks(_ ui: UIState) {
        let name = ui.function?.rawValue ?? "", n = ui.picks.count, del = ui.function == .delete
        guard let kind = ui.pickKind, let first = ui.picks.first else { return say("\(name): nothing picked.") }
        if n == 1 {
            switch kind {
            case .handle:
                guard let info = playback(first) else { return }
                if del { sayDelete(playback: info) } else { say("\(name) playback \(info.homeHandle): tap an empty handle.") }
            case .palette:
                guard let p = palettes.first(where: { $0.number == first }) else { return }
                if del { sayDelete(palette: p) } else { say("\(name) palette \(first): tap an empty cell.") }
            case .group:
                guard let g = groups.first(where: { $0.id == first }) else { return }
                say(del ? "Delete \(g.title)? Confirm on softkey A." : "\(name) group \(first): tap an empty cell.")
            }
            return
        }
        let noun = kind == .handle ? "playbacks" : kind == .palette ? "palettes" : "groups"
        guard del else { return say("\(name) \(n) \(noun): tap the target") }
        var text = "Delete \(n) \(noun)"
        if kind == .handle, let whole = ui.wholePagePhrase(self) { text += " of \(whole)" }
        else if kind == .handle, let page = ui.pickedPage(self) { text += " on page \(page)" }
        text += "?"
        let running = kind == .handle ? ui.picks.filter { playback($0)?.isRunning == true }.count : 0
        if running > 0 { text += running == 1 ? " 1 is running: its output ends at once." : " \(running) are running: their output ends at once." }
        say(text + " Confirm on softkey A.")
    }
}
