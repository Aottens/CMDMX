//! The daemon side of the generic MIDI control layer (VS7-02, `specs/modules/control-surfaces.md`).
//!
//! `midi_io` owns the CoreMIDI ports on its own thread and sends [`MidiEvent`]s; the tick thread
//! owns [`Control`]: devices and their states (CTL-08, 15, 16, 24), mapping files and the
//! `control` key of `state.json` (CTL-23), conversion of inputs to engine commands coalesced per
//! tick (CTL-06, 14), feedback after the tick (CTL-21), Learn (CTL-25), `last_input` (CTL-28) and
//! the `control` section of the snapshot and its delta (CTL-39). OSC (VS7-03) is the same layer
//! over a socket, see [`osc`]. Stream Decks (VS7-06) are devices too: `deck_io` owns the USB
//! side, the tick turns their reports into inputs and their key looks into [`DeckCmd`]s.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::PathBuf;

use cmdmx_control::action::{Action, AttrGroup, FanArg, Key, KeypadToken, Target, ValueKind};
use cmdmx_control::convert::{ActionValue, BindingState, Reading, Resolution};
use cmdmx_control::deck::{self, DeckInput, DeckKind, DeckReader, Model};
use cmdmx_control::deck_image::{key_look, KeyLook, KeyState, PlaybackView, Subject};
use cmdmx_control::feedback::Feedback;
use cmdmx_control::learn;
use cmdmx_control::mapping::{device_matches, Binding, Input, Mapping};
use cmdmx_control::mcu::{self, Window};
use cmdmx_control::midi::{self, Decoder, Kind, MidiInput, MidiMessage};
use cmdmx_core::attribute::AttributeGroup;
use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::palette::Timing;
use cmdmx_core::playback::{Button, HandleButton, ListAction, PlaybackId, PAGES};
use serde_json::{json, Value};

use crate::deck_io::{DeckCmd, DeckEvent, DeckState};
use crate::ipc::{self, Command, ControlRequest};
use crate::shows::{update_state, valid_name, write_atomic};

/// One device the MIDI thread sees now. `name` is already numbered (`BCF2000 #2`, CTL-08).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortInfo {
    pub name: String,
    pub input: bool,
    pub output: bool,
}

/// From the MIDI thread to the tick, over a channel the tick drains without waiting (CTL-14).
#[derive(Debug, Clone, PartialEq)]
pub enum MidiEvent {
    /// Every device connected now; sent whenever the list changes (CTL-08, 16).
    Ports(Vec<PortInfo>),
    /// One CoreMIDI packet from a device's input. Only the macOS MIDI thread sends it.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Packet { device: String, bytes: Vec<u8> },
}

/// From the tick to the MIDI thread: feedback bytes for a device's output (CTL-17..21).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MidiOut {
    pub device: String,
    pub bytes: Vec<u8>,
}

/// CTL-30: the mapping files that ship with the app.
const PROFILES: [&str; 1] = [include_str!("../../profiles/mackie-control.json")];

fn shipped() -> Vec<Mapping> {
    (PROFILES.iter())
        .filter_map(|t| Mapping::from_json(t).ok().map(|(m, _)| m))
        .collect()
}

/// CTL-31: a mapping with this `driver` belongs to a Mackie Control unit.
fn is_mackie(mappings: &BTreeMap<String, Mapping>, dev: &Dev) -> bool {
    let driver = dev.mapping.as_ref().and_then(|n| mappings.get(n));
    driver.is_some_and(|m| m.extra.get("driver").and_then(Value::as_str) == Some("mackie_control"))
}

/// CTL-31: a Mackie unit's slot numbers moved by its base and the shared window; `None` when
/// the fader is unbound. Other mappings (`win` is `None`) pass through.
fn route(a: Action, win: Option<(Window, u16)>, fpp: u16) -> Option<Action> {
    let Some((w, base)) = win else { return Some(a) };
    let t = |t: Target| match t {
        Target::Slot(n) => w.slot(base.saturating_add(n), fpp).map(Target::Slot),
        t => Some(t),
    };
    Some(match a {
        Action::Fader(x) => Action::Fader(t(x)?),
        Action::Upper(x) => Action::Upper(t(x)?),
        Action::Lower(x) => Action::Lower(t(x)?),
        Action::Go(x) => Action::Go(t(x)?),
        Action::Back(x) => Action::Back(t(x)?),
        Action::Stop(x) => Action::Stop(t(x)?),
        Action::Release(x) => Action::Release(t(x)?),
        Action::Tap(x) => Action::Tap(t(x)?),
        a => a,
    })
}

/// CTL-31: a unit shows 8 faders; the window is as wide as all units together.
fn mackie_width(units: &[String]) -> u16 {
    u16::try_from(units.len() * 8).unwrap_or(u16::MAX)
}

/// CTL-31: where the unit `name` starts in the window (8 per unit before it).
fn mackie_base(units: &[String], name: &str) -> Option<u16> {
    let i = units.iter().position(|u| u == name)?;
    u16::try_from(i * 8).ok()
}

/// CTL-28: how many seconds apart `last_input` updates reach the clients.
const LAST_INPUT_EVERY: f64 = 0.1;
/// CTL-15: the first malformed message per device per minute is logged.
const LOG_EVERY: f64 = 60.0;
/// CTL-25: a continuous input has to move this many 7-bit steps to be learned.
const LEARN_STEPS: i32 = 3;
/// Learn keeps this many messages per device; `learn::guess` needs a handful.
const LEARN_SEEN: usize = 1024;

struct Dev {
    input: bool,
    output: bool,
    connected: bool,
    enabled: bool,
    mapping: Option<String>,
    errors: u64,
    logged: Option<f64>,
    /// The 14-bit pairs `decoder` was built for.
    cc14: Vec<(u8, u8)>,
    decoder: Decoder,
    states: HashMap<MidiInput, BindingState>,
    feedback: Feedback<MidiInput, u16>,
    /// Decoded since the last `commands`.
    pending: Vec<MidiMessage>,
    /// Elements an input set this tick, with the raw value it sent (CTL-20).
    echo: Vec<(MidiInput, u16)>,
    /// Motor faders a hand is on (CTL-20).
    touched: Vec<MidiInput>,
    /// Set for a Stream Deck (CTL-44).
    deck: Option<DeckDev>,
}

/// What the tick knows of a Stream Deck (CTL-44..57).
struct DeckDev {
    /// `None` for an Elgato product id not in the table: Unsupported (CTL-45).
    model: Option<Model>,
    reader: Option<DeckReader>,
    /// What the deck thread last said; `None` = not opened yet.
    thread: Option<DeckState>,
    /// The deck page, from 1; per deck and not saved (CTL-48).
    page: u16,
    /// 0..=100 (CTL-53).
    brightness: u8,
    pending: Vec<(DeckInput, Reading)>,
    states: HashMap<DeckInput, BindingState>,
    /// What the keys of the deck show, by printed key number (CTL-52).
    feedback: Feedback<u16, KeyLook>,
    /// The looks of the current page, for the snapshot (CTL-57).
    looks: Vec<KeyLook>,
    busy_warned: bool,
    /// For the deck thread, handed over by `deck_out`.
    outbox: Vec<DeckCmd>,
    /// Pressed keys whose press learned; their release is not an input.
    learned_down: Vec<DeckInput>,
}

impl DeckDev {
    fn new(brightness: u8) -> DeckDev {
        DeckDev {
            model: None,
            reader: None,
            thread: None,
            page: 1,
            brightness,
            pending: Vec::new(),
            states: HashMap::new(),
            feedback: Feedback::default(),
            looks: Vec::new(),
            busy_warned: false,
            outbox: Vec::new(),
            learned_down: Vec::new(),
        }
    }
}

/// CTL-53: the brightness a deck starts with.
const BRIGHTNESS: u8 = 70;

impl Dev {
    fn new() -> Dev {
        Dev {
            input: false,
            output: false,
            connected: false,
            enabled: false,
            mapping: None,
            errors: 0,
            logged: None,
            cc14: Vec::new(),
            decoder: Decoder::default(),
            states: HashMap::new(),
            feedback: Feedback::default(),
            pending: Vec::new(),
            echo: Vec::new(),
            touched: Vec::new(),
            deck: None,
        }
    }

    /// A deck with a brightness of its own (CTL-53).
    fn custom_deck(&self) -> bool {
        self.deck
            .as_ref()
            .is_some_and(|d| d.brightness != BRIGHTNESS)
    }

    /// A device nobody enabled, assigned or set up is forgotten once it is gone.
    fn kept(&self) -> bool {
        self.connected || self.enabled || self.mapping.is_some() || self.custom_deck()
    }

    fn state(&self) -> &'static str {
        if let Some(d) = &self.deck {
            // CTL-45, 46: Unsupported and Busy are not Ok.
            return match (self.enabled, self.connected, d.thread) {
                (_, true, _) if d.model.is_none() => "Unsupported",
                (false, _, _) => "Disabled",
                (true, false, _) | (true, true, Some(DeckState::Missing)) => "Missing",
                (true, true, Some(DeckState::Busy)) => "Busy",
                _ => "Ok",
            };
        }
        match (self.enabled, self.connected) {
            (false, _) => "Disabled",
            (true, true) => "Ok",
            (true, false) => "Missing",
        }
    }
}

/// An armed Learn (CTL-25).
struct Learn {
    action: Action,
    mapping: Option<String>,
    /// First 7-bit value seen per input since arming.
    first: HashMap<(String, MidiInput), i32>,
    seen: HashMap<String, Vec<MidiMessage>>,
}

impl Learn {
    /// Record `m`; true when it is the input to learn: a press, or a move of 3 steps.
    fn feed(&mut self, device: &str, m: MidiMessage) -> bool {
        let seen = self.seen.entry(device.to_string()).or_default();
        if seen.len() < LEARN_SEEN {
            seen.push(m);
        }
        match m.input.kind {
            Kind::Note | Kind::Program => m.value > 0,
            kind => {
                let shift = if matches!(kind, Kind::Cc14 | Kind::PitchBend) {
                    7
                } else {
                    0
                };
                let v = i32::from(m.value >> shift);
                let first = *self.first.entry((device.to_string(), m.input)).or_insert(v);
                (v - first).abs() >= LEARN_STEPS
            }
        }
    }
}

struct LastInput {
    device: String,
    input: String,
    /// A MIDI value, or the first argument of an OSC message.
    value: Value,
}

pub struct Control {
    dir: PathBuf,
    state_file: PathBuf,
    devices: BTreeMap<String, Dev>,
    mappings: BTreeMap<String, Mapping>,
    /// Names in `mappings` that are still the shipped profile, not a user copy (CTL-30).
    profiles: BTreeSet<String>,
    /// What the Mackie units show (CTL-32), and the page it belongs to.
    window: Window,
    window_page: u16,
    /// The user's unit order (CTL-31); units not listed follow, by name.
    order: Vec<String>,
    learn: Option<Learn>,
    last_input: Option<LastInput>,
    messages: Vec<String>,
    /// Keypad tokens typed since the last Enter.
    keypad: Vec<String>,
    /// Something but `last_input` changed since the last delta.
    dirty: bool,
    input_dirty: bool,
    /// When the last delta that carried a new `last_input` went out.
    input_sent: f64,
    osc: osc::Osc,
    /// Faders per page, which the generated deck profiles follow (CTL-49).
    fpp: u16,
}

/// The text of an input, as the user sees it.
fn input_text(i: &Input) -> String {
    match i {
        Input::Midi(m) => m.to_string(),
        Input::Osc(a) => a.clone(),
        Input::Deck(d) => d.to_string(),
    }
}

fn cc14_pairs(m: &Mapping) -> Vec<(u8, u8)> {
    m.bindings
        .iter()
        .filter_map(|b| match &b.input {
            Input::Midi(i) if i.kind == Kind::Cc14 => Some((i.channel, i.number?)),
            _ => None,
        })
        .collect()
}

fn fan_curve(a: FanArg) -> Option<cmdmx_core::programmer::FanCurve> {
    use cmdmx_core::programmer::FanCurve as C;
    match a {
        FanArg::Line => Some(C::Line),
        FanArg::Mirror => Some(C::Mirror),
        FanArg::Wings => Some(C::Wings),
        FanArg::Arrow => Some(C::Arrow),
        FanArg::Toggle | FanArg::Off => None,
    }
}

fn core_group(g: AttrGroup) -> AttributeGroup {
    match g {
        AttrGroup::I => AttributeGroup::I,
        AttrGroup::P => AttributeGroup::P,
        AttrGroup::C => AttributeGroup::C,
        AttrGroup::G => AttributeGroup::G,
        AttrGroup::B => AttributeGroup::B,
        AttrGroup::E => AttributeGroup::E,
        AttrGroup::S => AttributeGroup::S,
        AttrGroup::FX => AttributeGroup::Fx,
    }
}

/// The playback a target means now; `None` for an empty slot.
fn resolve(engine: &Engine, t: Target) -> Option<PlaybackId> {
    match t {
        Target::Connected => engine.connected(),
        Target::Slot(s) => {
            let slots = engine.page_slots();
            slots.get(usize::from(s).checked_sub(1)?).copied().flatten()
        }
        Target::PageSlot { page, slot } => engine
            .playbacks()
            .find(|p| (p.page, p.slot) == (page, slot))
            .map(|p| p.id),
        Target::Playback(id) => Some(id),
    }
}

fn level_of(engine: &Engine, id: PlaybackId) -> f32 {
    engine.playback_state(id).map_or(0.0, |s| s.level)
}

/// What the actions of one `commands` call share (CTL-05).
struct Tick<'a> {
    engine: &'a Engine,
    now: f64,
    /// The page after the commands so far, so two presses in a tick step twice.
    page: u16,
    layout: Option<Vec<String>>,
}

impl Tick<'_> {
    /// The CTL-05 table: an action's values as engine commands.
    fn commands(
        &mut self,
        action: Action,
        values: &[ActionValue],
        keypad: &mut Vec<String>,
    ) -> Vec<Command> {
        let engine = self.engine;
        let mut out = Vec::new();
        for v in values {
            let pressed = matches!(v, ActionValue::Press);
            let cmd = match (action, v) {
                (Action::Fader(t), ActionValue::Level(level)) => {
                    resolve(engine, t).map(|playback| Command::FaderSet {
                        playback,
                        level: *level,
                    })
                }
                (Action::Gm, ActionValue::Level(level)) => {
                    Some(Command::GrandMasterSet { level: *level })
                }
                (
                    Action::Upper(t) | Action::Lower(t),
                    ActionValue::Press | ActionValue::Release,
                ) => {
                    let button = match action {
                        Action::Upper(_) => HandleButton::Upper,
                        _ => HandleButton::Lower,
                    };
                    resolve(engine, t).map(|id| Command::PlaybackButton {
                        id,
                        button,
                        on: pressed,
                        at: self.now,
                    })
                }
                (Action::Dbo, ActionValue::Press | ActionValue::Release) => {
                    Some(Command::DboSet { on: pressed })
                }
                (Action::Wheel(n), ActionValue::Steps(steps)) => {
                    let layout = self.layout.get_or_insert_with(|| engine.wheel_layout());
                    let name = usize::from(n).checked_sub(1).and_then(|i| layout.get(i));
                    name.map(|attribute| Command::Wheel {
                        attribute: attribute.clone(),
                        steps: *steps,
                        resolution: cmdmx_core::programmer::Resolution::Normal,
                    })
                }
                (_, ActionValue::Press) => self.press(action, keypad),
                _ => None,
            };
            out.extend(cmd);
        }
        out
    }

    /// The actions that happen on a press.
    fn press(&mut self, action: Action, keypad: &mut Vec<String>) -> Option<Command> {
        let engine = self.engine;
        let list = |t, action| match t {
            Target::Connected => Some(Command::CuelistAction { id: None, action }),
            t => resolve(engine, t).map(|id| Command::CuelistAction {
                id: Some(id),
                action,
            }),
        };
        let page = |this: &mut Self, page: Option<u16>| {
            let page = page.filter(|p| (1..=PAGES).contains(p))?;
            this.page = page;
            Some(Command::PageSet { page })
        };
        match action {
            Action::Go(t) => list(t, ListAction::Go),
            Action::Back(t) => list(t, ListAction::Back),
            Action::Stop(t) => list(t, ListAction::Stop),
            Action::Release(Target::Connected) => Some(Command::CuelistAction {
                id: None,
                action: ListAction::Release,
            }),
            Action::Release(t) => resolve(engine, t).map(|id| Command::PlaybackRelease { id }),
            Action::Tap(t) => {
                resolve(engine, t).map(|id| Command::PlaybackTap { id, at: self.now })
            }
            Action::PageNext => {
                let next = self.page.checked_add(1);
                page(self, next)
            }
            Action::PagePrev => {
                let prev = self.page.checked_sub(1);
                page(self, prev)
            }
            Action::Page(n) => page(self, Some(n)),
            // The deck page belongs to the deck device (VS7-06), not to the engine.
            Action::DeckPageNext | Action::DeckPagePrev | Action::DeckPage(_) => None,
            Action::EffectTap => Some(Command::EffectTap { at: self.now }),
            Action::EffectRestart => Some(Command::EffectRestart),
            Action::EffectStop => Some(Command::EffectStop { group: None }),
            Action::Fan(a) => Some(Command::Fan {
                curve: match a {
                    FanArg::Toggle => None,
                    FanArg::Off => Some(None),
                    c => Some(fan_curve(c)),
                },
            }),
            Action::Key(k) => Some(match k {
                Key::Clear => Command::Clear,
                Key::Locate => Command::Locate { groups: None },
                Key::Highlight => Command::Highlight {
                    on: !engine.highlight(),
                },
                Key::RemDim => Command::RemDim,
                Key::Undo => Command::Undo,
                Key::Redo => Command::Redo,
                Key::All => Command::All,
                Key::StepNext => Command::Step { forward: true },
                Key::StepPrev => Command::Step { forward: false },
            }),
            Action::Keypad(KeypadToken::Backspace) => {
                keypad.pop();
                None
            }
            Action::Keypad(KeypadToken::Enter) => {
                keypad.push("Enter".into());
                Some(Command::Line {
                    tokens: std::mem::take(keypad),
                })
            }
            Action::Keypad(t) => {
                keypad.push(t.to_string());
                None
            }
            Action::AttributeGroup(g) => Some(Command::AttributeGroup {
                group: core_group(g),
            }),
            Action::Group(group) => Some(Command::SelectGroup { group }),
            Action::Palette(number) => Some(Command::PaletteApply {
                number,
                timing: Timing::default(),
            }),
            _ => None,
        }
    }
}

/// What an action shows now, before any device turns it into its own numbers (CTL-17).
enum Shown {
    /// A level 0..=1; `None` for a slot without a playback.
    Level(Option<f32>),
    On(bool),
    /// A wheel's value; `None` for a wheel the programmer does not have.
    Wheel(Option<f32>),
}

/// CTL-17: the state of `action`; `wheels` caches the wheel values for the call.
fn state_of(engine: &Engine, action: Action, wheels: &mut Option<Vec<f32>>) -> Shown {
    let slot = |t: Target| resolve(engine, t).map(|id| level_of(engine, id));
    match action {
        Action::Fader(t) => Shown::Level(slot(t)),
        Action::Gm => Shown::Level(Some(engine.grand_master())),
        Action::Upper(t) | Action::Lower(t) => Shown::On(
            resolve(engine, t)
                .and_then(|id| engine.playback_state(id))
                .is_some_and(|s| s.flash || s.latched),
        ),
        Action::Go(t) => Shown::On(
            resolve(engine, t)
                .and_then(|id| engine.list_live(id))
                .is_some_and(|l| l.current.is_some()),
        ),
        Action::Page(n) => Shown::On(engine.page() == n),
        Action::Dbo => Shown::On(engine.dbo()),
        Action::Key(Key::Highlight) => Shown::On(engine.highlight()),
        // `fan off` is lit while Fan is on, so the button shows there is something to turn off.
        Action::Fan(FanArg::Toggle | FanArg::Off) => Shown::On(engine.fan().is_some()),
        Action::Fan(c) => Shown::On(engine.fan() == fan_curve(c)),
        Action::AttributeGroup(g) => Shown::On(engine.active_group() == core_group(g)),
        Action::Wheel(n) => {
            let all = wheels.get_or_insert_with(|| {
                (engine.programmer_view().wheels.iter())
                    .map(|w| w.value)
                    .collect()
            });
            let v = usize::from(n).checked_sub(1).and_then(|i| all.get(i));
            Shown::Wheel(v.copied())
        }
        _ => Shown::On(false),
    }
}

/// CTL-17..21: the raw value an element shows now for a binding; `None` for no feedback.
/// `win` is set for a Mackie unit (CTL-31, 33); `wheels` caches the wheel values for the call.
fn shown(
    engine: &Engine,
    b: &Binding,
    kind: Kind,
    win: Option<(Window, u16)>,
    wheels: &mut Option<Vec<f32>>,
) -> Option<u16> {
    let Some(action) = route(b.action, win, engine.settings().faders_per_page) else {
        return Some(if matches!(b.action, Action::Fader(_)) {
            0
        } else {
            b.off_value
        });
    };
    Some(match state_of(engine, action, wheels) {
        Shown::Level(None) => 0,
        Shown::Level(Some(level)) => {
            let x = if b.invert { 1.0 - level } else { level };
            let v = b.min + x * (b.max - b.min);
            if win.is_some() && kind == Kind::PitchBend {
                mcu::position(v)
            } else {
                midi::to_raw(kind, v)
            }
        }
        Shown::On(on) => {
            if on {
                b.on_value
            } else {
                b.off_value
            }
        }
        Shown::Wheel(v) if win.is_some() => mcu::ring(v),
        Shown::Wheel(v) => v.map_or(b.off_value, |v| midi::to_raw(kind, v)),
    })
}

impl Control {
    /// Mapping files from `dir` (created when missing), device choices from the `control` key of
    /// `state_file`. The strings are load warnings (a binding or a file left out).
    pub fn load(dir: PathBuf, state_file: PathBuf) -> (Control, Vec<String>) {
        let mut warnings = Vec::new();
        if let Err(e) = fs::create_dir_all(&dir) {
            warnings.push(format!("cannot create {}: {e}", dir.display()));
        }
        let mut files: Vec<PathBuf> = fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        files.sort();
        let mut mappings = BTreeMap::new();
        for m in shipped() {
            mappings.insert(m.name.clone(), m);
        }
        let mut profiles: BTreeSet<String> = mappings.keys().cloned().collect();
        for file in files {
            let shown = file.display();
            let loaded = fs::read_to_string(&file)
                .map_err(|e| e.to_string())
                .and_then(|t| Mapping::from_json(&t).map_err(|e| e.to_string()))
                .and_then(|(m, w)| valid_name(&m.name).map(|()| (m, w)));
            match loaded {
                Ok((m, w)) => {
                    warnings.extend(w.into_iter().map(|w| format!("{}: {w}", m.name)));
                    profiles.remove(&m.name);
                    mappings.insert(m.name.clone(), m);
                }
                Err(e) => warnings.push(format!("mapping file {shown} left out: {e}")),
            }
        }
        // CTL-23: devices enabled in an earlier run are known, Missing until they are seen.
        let saved: Option<Value> = fs::read_to_string(&state_file)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok());
        let mut devices = BTreeMap::new();
        let known = saved
            .as_ref()
            .and_then(|s| s.get("control")?.get("devices")?.as_object());
        for (name, d) in known.into_iter().flatten() {
            let mut dev = Dev::new();
            dev.enabled = d.get("enabled").and_then(Value::as_bool).unwrap_or(false);
            dev.mapping = d.get("mapping").and_then(Value::as_str).map(str::to_string);
            if let Some(b) = d.get("brightness").and_then(Value::as_u64) {
                dev.deck = Some(DeckDev::new(u8::try_from(b.min(100)).unwrap_or(BRIGHTNESS)));
            }
            devices.insert(name.clone(), dev);
        }
        let order = (saved.as_ref())
            .and_then(|s| s.get("control")?.get("order")?.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|n| n.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let mut control = Control {
            order,
            dir,
            state_file,
            devices,
            mappings,
            profiles,
            window: Window::default(),
            window_page: 0,
            learn: None,
            last_input: None,
            messages: Vec::new(),
            keypad: Vec::new(),
            dirty: true,
            input_dirty: false,
            input_sent: f64::NEG_INFINITY,
            osc: osc::Osc::load(saved.as_ref()),
            fpp: Settings::default().faders_per_page,
        };
        control.refresh();
        (control, warnings)
    }

    /// After a mapping or assignment changed: decoders follow the 14-bit pairs, and every
    /// device gets its full feedback again (CTL-21).
    fn refresh(&mut self) {
        for dev in self.devices.values_mut() {
            let pairs = dev
                .mapping
                .as_ref()
                .and_then(|n| self.mappings.get(n))
                .map(cc14_pairs)
                .unwrap_or_default();
            if pairs != dev.cc14 {
                dev.decoder = Decoder::new(pairs.clone());
                dev.cc14 = pairs;
            }
            dev.feedback.clear();
            if let Some(d) = &mut dev.deck {
                d.feedback.clear();
            }
        }
        self.osc.feedback.clear();
        self.dirty = true;
    }

    /// Called while the tick drains its channels; never blocks.
    pub fn event(&mut self, event: MidiEvent, now: f64) {
        match event {
            MidiEvent::Ports(list) => self.ports(list),
            MidiEvent::Packet { device, bytes } => self.packet(&device, &bytes, now),
        }
    }

    /// CTL-08, 16, 24: devices come, go and come back.
    fn ports(&mut self, list: Vec<PortInfo>) {
        for p in &list {
            let dev = self.devices.entry(p.name.clone()).or_insert_with(Dev::new);
            if !dev.connected {
                // Back (or new): the decoder starts clean and the whole state goes out (CTL-16).
                dev.decoder = Decoder::new(dev.cc14.clone());
                dev.states.clear();
                dev.feedback.clear();
                dev.touched.clear();
            }
            (dev.connected, dev.input, dev.output) = (true, p.input, p.output);
        }
        // Decks come and go with the deck thread's list, not the MIDI one.
        for (name, dev) in self.devices.iter_mut().filter(|(_, d)| d.deck.is_none()) {
            if dev.connected && !list.iter().any(|p| &p.name == name) {
                (dev.connected, dev.input, dev.output) = (false, false, false);
                if dev.enabled {
                    self.messages.push(format!("{name} disconnected"));
                }
            }
        }
        // A device nobody enabled or assigned is forgotten once it is gone.
        self.devices.retain(|_, d| d.kept());
        self.dirty = true;
    }

    /// CTL-14, 15, 25, 28: decode at once; nothing waits.
    fn packet(&mut self, device: &str, bytes: &[u8], now: f64) {
        let Some(dev) = self.devices.get_mut(device) else {
            return;
        };
        let mackie = is_mackie(&self.mappings, dev);
        let before = dev.decoder.malformed();
        let msgs = dev.decoder.decode(bytes);
        let bad = dev.decoder.malformed().saturating_sub(before);
        if bad > 0 {
            dev.errors += bad;
            self.dirty = true;
            if dev.logged.is_none_or(|t| now - t >= LOG_EVERY) {
                dev.logged = Some(now);
                eprintln!("midi: malformed message from {device}: {bytes:02x?}");
            }
        }
        let mut learned = None;
        for m in msgs {
            // CTL-20: a touch note is not an input; it holds the motor still.
            let touch = (m.input.kind == Kind::Note && m.input.channel == 1)
                .then(|| m.input.number.and_then(mcu::touch))
                .flatten();
            if let (true, Some(ch)) = (mackie, touch) {
                let fader = MidiInput {
                    kind: Kind::PitchBend,
                    channel: ch,
                    number: None,
                };
                dev.touched.retain(|t| *t != fader);
                if m.value > 0 {
                    dev.touched.push(fader);
                }
                continue;
            }
            self.last_input = Some(LastInput {
                device: device.to_string(),
                input: m.input.to_string(),
                value: m.value.into(),
            });
            self.input_dirty = true;
            if !dev.enabled {
                continue;
            }
            match &mut self.learn {
                // An input that learns does not also act (CTL-25).
                Some(learn) => {
                    if learned.is_none() && learn.feed(device, m) {
                        learned = Some(m.input);
                    }
                }
                None => dev.pending.push(m),
            }
        }
        if let Some(input) = learned {
            self.finish_learn(device, input);
        }
    }

    /// CTL-25, 26, 27: bind what the device sent to the armed action.
    fn finish_learn(&mut self, device: &str, input: MidiInput) {
        let Some(mut learn) = self.learn.take() else {
            return;
        };
        self.dirty = true;
        let mut seen = learn.seen.remove(device).unwrap_or_default();
        // The learned input first: `guess` looks at the first message.
        seen.sort_by_key(|m| m.input != input);
        let Some(binding) = learn::guess(learn.action, &seen) else {
            return;
        };
        self.bind_learned(learn.mapping, device, binding);
    }

    /// Put the learned binding into the armed mapping, or the device's own; say so (CTL-27).
    fn bind_learned(&mut self, mapping: Option<String>, device: &str, binding: Binding) {
        let target = mapping.or_else(|| self.devices.get(device)?.mapping.clone());
        let Some(mapping) = target.and_then(|t| self.mappings.get_mut(&t)) else {
            self.messages
                .push(format!("{device} has no mapping to learn into"));
            return;
        };
        let (text, action) = (input_text(&binding.input), binding.action);
        let old = mapping.bind(binding);
        self.messages
            .push(format!("Learned: {device} {text} → {action}"));
        if let Some(old) = old {
            self.messages.push(format!("{text} was {}", old.action));
        }
        let name = mapping.name.clone();
        if let Err(e) = self.save(&name) {
            self.messages.push(e);
        }
        self.refresh();
    }

    /// Engine commands for the inputs since the last call, each with its device name (for the
    /// refusal text, CTL-06). Values per input coalesced to the last; buttons never (CTL-14).
    pub fn commands(&mut self, engine: &Engine, now: f64) -> Vec<(String, Command)> {
        let units = self.mackie_units(engine);
        let fpp = engine.settings().faders_per_page;
        self.follow_fpp(fpp);
        let width = mackie_width(&units);
        let mut out = Vec::new();
        let mut tick = Tick {
            engine,
            now,
            page: engine.page(),
            layout: None,
        };
        for (name, dev) in &mut self.devices {
            let base = mackie_base(&units, name);
            let msgs = std::mem::take(&mut dev.pending);
            let mapping = dev.mapping.as_ref().and_then(|n| self.mappings.get(n));
            let Some(mapping) = mapping.filter(|_| dev.enabled && !msgs.is_empty()) else {
                continue;
            };
            // (binding, input, reading, raw value); a level takes the place of the one before it.
            let mut items: Vec<(&Binding, MidiInput, Reading, u16)> = Vec::new();
            let mut at: HashMap<MidiInput, usize> = HashMap::new();
            for m in msgs {
                let want = Input::Midi(m.input);
                let Some(b) = mapping.bindings.iter().find(|b| b.input == want) else {
                    continue;
                };
                let mut reading = b.read(&m);
                // CTL-01: Mackie faders are 10 bits, so full travel is exactly 1.0.
                let motor = base.is_some() && m.input.kind == Kind::PitchBend;
                if let (true, Reading::Level(_)) = (motor, reading) {
                    reading = Reading::Level(mcu::level(m.value));
                }
                // A level that acts as a button (a fader as a Flash) is never coalesced.
                let button = b.action.value_kind() == ValueKind::Button;
                let slot = at.get(&m.input).and_then(|i| items.get_mut(*i));
                match (slot, reading) {
                    (Some(item), Reading::Level(_)) => (item.2, item.3) = (reading, m.value),
                    (Some(item), Reading::Steps(more)) => {
                        if let Reading::Steps(sum) = item.2 {
                            item.2 = Reading::Steps(sum + more);
                        }
                    }
                    _ => {
                        if !button && matches!(reading, Reading::Level(_) | Reading::Steps(_)) {
                            at.insert(m.input, items.len());
                        }
                        items.push((b, m.input, reading, m.value));
                    }
                }
            }
            for (b, input, reading, raw) in items {
                let win = base.map(|base| (self.window, base));
                // An unbound Mackie fader does nothing (CTL-31).
                let Some(action) = route(b.action, win, fpp) else {
                    continue;
                };
                let motor = win.is_some() && input.kind == Kind::PitchBend;
                // Motor faders never pick up (CTL-13).
                let current = match action {
                    _ if motor => None,
                    Action::Fader(t) => {
                        Some(resolve(engine, t).map_or(0.0, |id| level_of(engine, id)))
                    }
                    Action::Gm => Some(engine.grand_master()),
                    _ => None,
                };
                let state = dev.states.entry(input).or_default();
                let values = state.apply(b, reading, current, Resolution::Normal);
                // CTL-32: Channel ◀ ▶ move the window of the Mackie units, not the engine.
                if let (Action::ChannelNext | Action::ChannelPrev, true) = (action, win.is_some()) {
                    if values.contains(&ActionValue::Press) {
                        let forward = action == Action::ChannelNext;
                        self.window.channel(forward, width, fpp);
                    }
                    continue;
                }
                let cmds = tick.commands(action, &values, &mut self.keypad);
                if cmds.is_empty() {
                    continue;
                }
                // CTL-20: no echo; a motor's echo is the position feedback would send.
                if b.feedback_to.is_none() && matches!(reading, Reading::Level(_)) {
                    let raw = if motor {
                        mcu::position(mcu::level(raw))
                    } else {
                        raw
                    };
                    dev.echo.push((input, raw));
                }
                out.extend(cmds.into_iter().map(|c| (name.clone(), c)));
            }
        }
        self.deck_commands(&mut tick, &mut out);
        out.extend(self.osc_commands(&mut tick));
        out
    }

    /// Feedback after the tick's frame (CTL-19..21).
    pub fn feedback(&mut self, engine: &Engine) -> Vec<MidiOut> {
        let units = self.mackie_units(engine);
        let mut wheels = None;
        let mut out = Vec::new();
        for (name, dev) in &mut self.devices {
            let win = mackie_base(&units, name).map(|base| (self.window, base));
            let echo = std::mem::take(&mut dev.echo);
            let mapping = dev.mapping.as_ref().and_then(|n| self.mappings.get(n));
            let Some(mapping) = mapping.filter(|_| dev.enabled && dev.connected && dev.output)
            else {
                continue;
            };
            let wanted = mapping
                .bindings
                .iter()
                .filter(|b| b.feedback)
                .filter_map(|b| {
                    let Input::Midi(element) = b.feedback_to.as_ref().unwrap_or(&b.input) else {
                        return None;
                    };
                    Some((*element, shown(engine, b, element.kind, win, &mut wheels)?))
                });
            for (element, value) in dev.feedback.tick(wanted, &echo, &dev.touched) {
                let bytes = midi::encode(&element, value);
                if !bytes.is_empty() {
                    out.push(MidiOut {
                        device: name.clone(),
                        bytes,
                    });
                }
            }
        }
        out
    }

    /// The connected, enabled Mackie units by device name; also keeps the shared window valid:
    /// any page change resets it (CTL-32), and it stays inside the page.
    fn mackie_units(&mut self, engine: &Engine) -> Vec<String> {
        let mut units: Vec<String> = (self.devices.iter())
            .filter(|(_, d)| d.connected && d.enabled && is_mackie(&self.mappings, d))
            .map(|(n, _)| n.clone())
            .collect();
        // Stable: listed units in list order, the rest keep their by-name order (CTL-31).
        units.sort_by_key(|n| self.order.iter().position(|o| o == n).unwrap_or(usize::MAX));
        if engine.page() != self.window_page {
            self.window_page = engine.page();
            self.window.reset();
        }
        let width = mackie_width(&units);
        self.window.clamp(width, engine.settings().faders_per_page);
        units
    }

    /// Write one mapping's file; from then on it is a user copy, not the shipped profile
    /// (CTL-30). ponytail: a small write on the tick thread, once per edit.
    fn save(&mut self, name: &str) -> Result<(), String> {
        let m = self
            .mappings
            .get(name)
            .ok_or_else(|| format!("unknown mapping '{name}'"))?;
        write_atomic(&self.dir.join(format!("{name}.json")), &m.to_json())
            .map_err(|e| format!("cannot save mapping '{name}': {e}"))?;
        self.profiles.remove(name);
        Ok(())
    }

    /// CTL-23: the devices the user enabled or assigned, into `state.json` beside `last_show`.
    fn persist(&self) -> Result<(), String> {
        let devices: serde_json::Map<String, Value> = (self.devices.iter())
            .filter(|(_, d)| d.enabled || d.mapping.is_some() || d.custom_deck())
            .map(|(n, d)| {
                let mut entry = json!({"enabled": d.enabled, "mapping": d.mapping});
                if let (Some(deck), Some(o)) = (&d.deck, entry.as_object_mut()) {
                    o.insert("brightness".into(), deck.brightness.into());
                }
                (n.clone(), entry)
            })
            .collect();
        let control = json!({ "devices": devices, "osc": self.osc.settings, "order": self.order });
        update_state(&self.state_file, "control", control)
            .map_err(|e| format!("cannot save the control settings: {e}"))
    }

    /// A mapping that exists, or the refusal.
    fn mapping_mut(&mut self, name: &str) -> Result<&mut Mapping, String> {
        (self.mappings.get_mut(name)).ok_or_else(|| format!("unknown mapping '{name}'"))
    }

    /// A `control.*` command (CTL-40). `Err` is the refusal text.
    pub fn request(&mut self, req: ipc::ControlRequest) -> Result<(), String> {
        let result = self.run_request(req);
        self.dirty = true;
        result
    }

    fn run_request(&mut self, req: ControlRequest) -> Result<(), String> {
        match req {
            ControlRequest::Enable { device, on } => {
                let (mapping, is_deck) = match self.devices.get(&device) {
                    None => return Err(format!("unknown device '{device}'")),
                    Some(d)
                        if on
                            && d.connected
                            && d.deck.as_ref().is_some_and(|k| k.model.is_none()) =>
                    {
                        return Err(format!("{device} is not supported"));
                    }
                    Some(d) => (d.mapping.clone(), d.deck.is_some()),
                };
                if on && mapping.is_none() {
                    // CTL-49: a deck gets a mapping for decks, never an empty one.
                    let name = if is_deck {
                        self.deck_mapping(&device)
                    } else {
                        Some(self.first_mapping(&device)?)
                    };
                    if let Some(d) = self.devices.get_mut(&device) {
                        d.mapping = name;
                    }
                }
                if let Some(d) = self.devices.get_mut(&device) {
                    d.enabled = on;
                    let connected = d.connected;
                    if let Some(k) = &mut d.deck {
                        // CTL-46: open when enabled, blank and close when disabled.
                        k.thread = None;
                        k.busy_warned = false;
                        k.outbox.clear();
                        if !on {
                            k.outbox.push(DeckCmd::Close {
                                name: device.clone(),
                            });
                        } else if connected {
                            k.outbox.push(DeckCmd::Open {
                                name: device.clone(),
                                brightness: k.brightness,
                            });
                        }
                    }
                }
                self.refresh();
                self.persist()
            }
            ControlRequest::Deck {
                device,
                brightness,
                page,
            } => {
                if let Some(b) = brightness.filter(|b| *b > 100) {
                    return Err(format!("brightness {b} outside 0..=100"));
                }
                let d = (self.devices.get_mut(&device))
                    .ok_or(format!("'{device}' is not a Stream Deck"))?;
                let pages = (d.mapping.as_ref().and_then(|n| self.mappings.get(n)))
                    .map_or(1, deck::deck_pages);
                let k = d
                    .deck
                    .as_mut()
                    .ok_or(format!("'{device}' is not a Stream Deck"))?;
                if let Some(page) = page.filter(|p| !(1..=pages).contains(p)) {
                    return Err(format!("deck page {page} outside 1..={pages}"));
                }
                if let Some(page) = page {
                    if page != k.page {
                        k.page = page;
                        k.feedback.clear();
                    }
                    self.dirty = true;
                }
                if let Some(brightness) = brightness {
                    k.brightness = brightness;
                    k.outbox.push(DeckCmd::Brightness {
                        name: device,
                        percent: brightness,
                    });
                    return self.persist();
                }
                Ok(())
            }
            ControlRequest::Order { devices } => {
                self.order = devices;
                self.persist()
            }
            ControlRequest::Assign { device, mapping } => {
                if !self.mappings.contains_key(&mapping) {
                    return Err(format!("unknown mapping '{mapping}'"));
                }
                let d =
                    (self.devices.get_mut(&device)).ok_or(format!("unknown device '{device}'"))?;
                d.mapping = Some(mapping);
                self.refresh();
                self.persist()
            }
            ControlRequest::Learn { action, mapping } => {
                if let Some(m) = &mapping {
                    self.mapping_mut(m)?;
                }
                self.learn = Some(Learn {
                    action,
                    mapping,
                    first: HashMap::new(),
                    seen: HashMap::new(),
                });
                Ok(())
            }
            ControlRequest::LearnCancel => {
                self.learn = None;
                Ok(())
            }
            ControlRequest::Binding { mapping, change } => {
                let m = self.mapping_mut(&mapping)?;
                let input: Input = change
                    .get("input")
                    .and_then(|i| serde_json::from_value(i.clone()).ok())
                    .ok_or("missing or invalid \"input\"")?;
                // Like `output.edit`: the given fields over the stored binding.
                let deck_page = (change.get("deck_page"))
                    .and_then(Value::as_u64)
                    .and_then(|p| u16::try_from(p).ok())
                    .unwrap_or(1);
                let found = m
                    .bindings
                    .iter()
                    .find(|b| b.input == input && b.deck_page == deck_page);
                let mut merged = match found {
                    Some(stored) => match serde_json::to_value(stored) {
                        Ok(Value::Object(o)) => o,
                        _ => return Err("cannot read the stored binding".into()),
                    },
                    None if change.contains_key("action") => serde_json::Map::new(),
                    None => return Err("a new binding needs an \"action\"".into()),
                };
                merged.extend(change);
                let binding: Binding = serde_json::from_value(Value::Object(merged))
                    .map_err(|e| format!("invalid binding: {e}"))?;
                cmdmx_control::mapping::check(&binding)?;
                m.bind(binding);
                self.save(&mapping)?;
                self.refresh();
                Ok(())
            }
            ControlRequest::Unbind {
                mapping,
                input,
                deck_page,
            } => {
                let m = self.mapping_mut(&mapping)?;
                let before = m.bindings.len();
                m.bindings
                    .retain(|b| !(b.input == input && b.deck_page == deck_page));
                if m.bindings.len() == before {
                    return Err(format!("{} is not bound", input_text(&input)));
                }
                self.save(&mapping)?;
                self.refresh();
                Ok(())
            }
            ControlRequest::MappingNew { name, device } => {
                valid_name(&name)?;
                if self.mappings.contains_key(&name) {
                    return Err(format!("mapping '{name}' exists"));
                }
                self.add_mapping(name, device)
            }
            ControlRequest::MappingRename { name, to } => {
                if self.profiles.contains(&name) {
                    return Err(format!("'{name}' is a profile"));
                }
                valid_name(&to)?;
                if self.mappings.contains_key(&to) {
                    return Err(format!("mapping '{to}' exists"));
                }
                let mut m =
                    (self.mappings.remove(&name)).ok_or(format!("unknown mapping '{name}'"))?;
                m.name = to.clone();
                self.mappings.insert(to.clone(), m);
                self.save(&to)?;
                fs::remove_file(self.dir.join(format!("{name}.json")))
                    .map_err(|e| format!("cannot remove the old file of '{name}': {e}"))?;
                for d in self.devices.values_mut() {
                    if d.mapping.as_deref() == Some(&name) {
                        d.mapping = Some(to.clone());
                    }
                }
                self.refresh();
                self.persist()
            }
            ControlRequest::MappingDelete { name } => {
                if self.profiles.contains(&name) {
                    return Err(format!("'{name}' is a profile"));
                }
                self.mappings
                    .remove(&name)
                    .ok_or(format!("unknown mapping '{name}'"))?;
                // Deleting a user copy of a profile brings the profile back (CTL-30).
                if let Some(p) = shipped().into_iter().find(|m| m.name == name) {
                    self.profiles.insert(name.clone());
                    self.mappings.insert(name.clone(), p);
                } else {
                    for d in self.devices.values_mut() {
                        if d.mapping.as_deref() == Some(&name) {
                            d.mapping = None;
                        }
                    }
                }
                self.refresh();
                fs::remove_file(self.dir.join(format!("{name}.json")))
                    .map_err(|e| format!("cannot remove the file of '{name}': {e}"))?;
                self.persist()
            }
            ControlRequest::Osc(change) => {
                self.osc_request(&change)?;
                self.persist()
            }
            ControlRequest::MappingImport { json, replace } => {
                let (m, warnings) = Mapping::from_json(&json).map_err(|e| e.to_string())?;
                valid_name(&m.name)?;
                if self.mappings.contains_key(&m.name) && !replace {
                    return Err(format!("mapping '{}' exists", m.name));
                }
                self.messages
                    .extend(warnings.into_iter().map(|w| format!("{}: {w}", m.name)));
                let name = m.name.clone();
                self.mappings.insert(name.clone(), m);
                self.save(&name)?;
                self.refresh();
                Ok(())
            }
        }
    }

    /// CTL-24: the first mapping whose `device` pattern matches, else a new empty one named after
    /// the device.
    fn first_mapping(&mut self, device: &str) -> Result<String, String> {
        let found = self
            .mappings
            .values()
            .find(|m| device_matches(&m.device, device));
        if let Some(m) = found {
            return Ok(m.name.clone());
        }
        valid_name(device)?;
        let mut name = device.to_string();
        let mut n = 1;
        while self.mappings.contains_key(&name) {
            n += 1;
            name = format!("{device} {n}");
        }
        self.add_mapping(name.clone(), device.to_string())?;
        Ok(name)
    }

    fn add_mapping(&mut self, name: String, device: String) -> Result<(), String> {
        let m = Mapping {
            name: name.clone(),
            device,
            bindings: Vec::new(),
            extra: serde_json::Map::new(),
        };
        self.mappings.insert(name.clone(), m);
        self.save(&name)
    }

    /// The answer to `control.mapping {name}`, for the asking client only.
    pub fn mapping_reply(&self, name: &str) -> String {
        match self.mappings.get(name).map(serde_json::to_value) {
            Some(Ok(mapping)) => {
                json!({"v": crate::ipc::PROTOCOL, "type": "control.mapping", "name": name, "mapping": mapping})
                    .to_string()
            }
            _ => ipc::error(&format!("unknown mapping '{name}'")),
        }
    }

    /// Texts for the message zone since the last call (`BCF2000 disconnected`, `Learned: …`).
    pub fn take_messages(&mut self) -> Vec<String> {
        std::mem::take(&mut self.messages)
    }

    /// The snapshot's `control` section (CTL-39).
    pub fn snapshot(&self) -> Value {
        let devices: Vec<Value> = (self.devices.iter())
            .map(|(name, d)| {
                let mut dev = json!({
                    "name": name,
                    "input": d.input,
                    "output": d.output,
                    "state": d.state(),
                    "enabled": d.enabled,
                    "mapping": d.mapping,
                    "errors": d.errors,
                    "kind": if d.deck.is_some() { "streamdeck" } else { "midi" },
                });
                if let (Some(k), Some(o)) = (&d.deck, dev.as_object_mut()) {
                    let mapping = d.mapping.as_ref().and_then(|n| self.mappings.get(n));
                    o.extend(deck_snapshot(k, mapping));
                }
                dev
            })
            .collect();
        let mappings: Vec<Value> = (self.mappings.keys())
            .map(|name| json!({"name": name, "profile": self.profiles.contains(name)}))
            .collect();
        let last_input = self
            .last_input
            .as_ref()
            .map(|l| json!({"device": l.device, "input": l.input, "value": l.value}));
        json!({
            "devices": devices,
            "mappings": mappings,
            "order": self.order,
            "learn": self.learn.as_ref().map(|l| l.action.to_string()),
            "last_input": last_input,
            "osc": self.osc_snapshot(),
        })
    }

    /// A `control` delta after a change; `last_input` at most 10 per second (CTL-28, 39).
    pub fn delta(&mut self, now: f64) -> Option<String> {
        let input_due = self.input_dirty && now - self.input_sent >= LAST_INPUT_EVERY;
        if !self.dirty && !input_due {
            return None;
        }
        if self.input_dirty {
            self.input_sent = now;
        }
        (self.dirty, self.input_dirty) = (false, false);
        let mut delta = self.snapshot();
        let obj = delta.as_object_mut()?;
        obj.insert("v".into(), 0.into());
        obj.insert("type".into(), "control".into());
        Some(delta.to_string())
    }
}

/// The playback an action works on, when it names a slot or playback (CTL-50).
fn playback_target(a: Action) -> Option<Target> {
    match a {
        Action::Upper(t)
        | Action::Lower(t)
        | Action::Fader(t)
        | Action::Go(t)
        | Action::Back(t)
        | Action::Stop(t)
        | Action::Release(t)
        | Action::Tap(t) => Some(t),
        _ => None,
    }
}

/// What a key shows, from the engine (CTL-50, CTL-17).
fn subject_of(engine: &Engine, b: &Binding, wheels: &mut Option<Vec<f32>>) -> Subject {
    let other = |wheels: &mut Option<Vec<f32>>| {
        let shown = state_of(engine, b.action, wheels);
        Subject::Other {
            name: match b.action {
                Action::Palette(n) => (engine.palette(n))
                    .map(|p| p.legend.clone())
                    .filter(|l| !l.is_empty()),
                _ => None,
            },
            active: matches!(shown, Shown::On(true)),
            level: match shown {
                Shown::Level(l) => l,
                _ => None,
            },
        }
    };
    // `go` and friends without a target act on the connected list: a plain key.
    let Some(target) = playback_target(b.action).filter(|t| *t != Target::Connected) else {
        return other(wheels);
    };
    let Some(id) = resolve(engine, target) else {
        return Subject::Empty;
    };
    let state = engine.playback_state(id);
    // The function of the handle button this key presses (CUE-25).
    let button = match b.action {
        Action::Upper(_) => engine.handle_buttons(id).map(|p| p[0]),
        Action::Lower(_) => engine.handle_buttons(id).map(|p| p[1]),
        _ => None,
    };
    let held = match button {
        Some(Button::Flash) => state.is_some_and(|s| s.flash),
        Some(Button::Swop) => state.is_some_and(|s| s.swop),
        _ => false,
    };
    let byte = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    Subject::Playback(PlaybackView {
        id,
        legend: engine
            .playback(id)
            .map_or_else(String::new, |p| p.legend.clone()),
        halo: engine
            .playback_halo(id)
            .map(|h| [byte(h.r), byte(h.g), byte(h.b)]),
        level: level_of(engine, id),
        running: (engine.list_live(id)).is_some_and(|l| l.current.is_some()),
        function: button
            .filter(|b| *b != Button::None)
            .map(|b| format!("{b:?}")),
        held,
    })
}

/// CTL-57: the deck fields of a device in the snapshot.
fn deck_snapshot(k: &DeckDev, mapping: Option<&Mapping>) -> serde_json::Map<String, Value> {
    let size = |s: Option<deck::Size>| s.map_or(Value::Null, |s| json!({"w": s.w, "h": s.h}));
    let mut o = serde_json::Map::new();
    let m = k.model.as_ref();
    let mut put = |key: &str, v: Value| o.insert(key.to_string(), v);
    put("model", m.map_or(Value::Null, |m| m.name.clone().into()));
    put("verified", m.is_some_and(|m| m.verified).into());
    put("cols", m.map_or(0, |m| m.cols).into());
    put("rows", m.map_or(0, |m| m.rows).into());
    put(
        "extras",
        json!({
            "dials": m.map_or(0, |m| m.dials),
            "touch_keys": m.map_or(0, |m| m.touch_keys),
            "strip": size(m.and_then(|m| m.strip)),
            "info": size(m.and_then(|m| m.info)),
        }),
    );
    put("deck_page", k.page.into());
    put("deck_pages", mapping.map_or(1, deck::deck_pages).into());
    put("brightness", k.brightness.into());
    let hex = |c: [u8; 3]| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
    let keys: Vec<Value> = (1u16..)
        .zip(&k.looks)
        .map(|(n, l)| {
            let state = match l.state {
                KeyState::Off => "off",
                KeyState::Inactive => "inactive",
                KeyState::Active => "active",
                KeyState::Held => "held",
            };
            json!({
                "key": n,
                "label": l.label,
                "sub": l.sub,
                "halo": l.halo.map(hex),
                "level": l.level.map(f64::from),
                "state": state,
                "fixed": l.fixed,
            })
        })
        .collect();
    put("keys", keys.into());
    o
}

impl Control {
    /// CTL-49: the generated profiles follow the faders per page while they are not edited.
    fn follow_fpp(&mut self, fpp: u16) {
        if fpp == self.fpp {
            return;
        }
        self.fpp = fpp;
        for m in deck::models().unwrap_or_default() {
            let profile = deck::default_mapping(&m, fpp);
            if self.profiles.contains(&profile.name) {
                self.mappings.insert(profile.name.clone(), profile);
            }
        }
        // Only the decks show it; MIDI feedback stays as it is.
        for k in self.devices.values_mut().filter_map(|d| d.deck.as_mut()) {
            k.feedback.clear();
        }
        self.dirty = true;
    }

    /// CTL-49: the mapping a deck gets when enabled without one: the first whose `device`
    /// pattern matches and that has a deck binding, else the generated profile.
    fn deck_mapping(&self, device: &str) -> Option<String> {
        let for_decks = |m: &&Mapping| {
            device_matches(&m.device, device)
                && m.bindings.iter().any(|b| matches!(b.input, Input::Deck(_)))
        };
        if let Some(m) = self.mappings.values().find(for_decks) {
            return Some(m.name.clone());
        }
        let model = self.devices.get(device)?.deck.as_ref()?.model.as_ref()?;
        Some(model.name.clone())
    }

    /// Called while the tick drains the deck thread's channel; never blocks.
    pub fn deck_event(&mut self, event: DeckEvent, now: f64) {
        match event {
            DeckEvent::Decks(list) => self.decks(&list),
            DeckEvent::State { name, state } => {
                let Some((dev, k)) = self.devices.get_mut(&name).and_then(|d| {
                    let k = d.deck.as_mut()?;
                    Some((&mut d.enabled, k))
                }) else {
                    return;
                };
                k.thread = Some(state);
                match state {
                    // CTL-54: the whole page goes out again.
                    DeckState::Ok => {
                        k.feedback.clear();
                        k.busy_warned = false;
                    }
                    DeckState::Busy => {
                        if *dev && !std::mem::replace(&mut k.busy_warned, true) {
                            self.messages.push(format!(
                                "{name} is in use by another app; quit the Elgato Stream Deck app"
                            ));
                        }
                    }
                    DeckState::Missing => {}
                }
                self.dirty = true;
            }
            DeckEvent::Report { name, bytes } => self.deck_report(&name, &bytes, now),
        }
    }

    /// CTL-44, 45, 46, 49: decks come, go and come back.
    fn decks(&mut self, list: &[crate::deck_io::DeckInfo]) {
        for info in list {
            let model = deck::model(info.product_id);
            // The generated profile, unless a mapping of that name exists (a user copy).
            if let Some(m) = &model {
                let profile = deck::default_mapping(m, self.fpp);
                if !self.mappings.contains_key(&profile.name) {
                    self.profiles.insert(profile.name.clone());
                    self.mappings.insert(profile.name.clone(), profile);
                }
            }
            let dev = self
                .devices
                .entry(info.name.clone())
                .or_insert_with(Dev::new);
            let k = dev.deck.get_or_insert_with(|| DeckDev::new(BRIGHTNESS));
            if dev.connected {
                continue;
            }
            (dev.connected, dev.input, dev.output) = (true, true, true);
            k.reader = model.as_ref().map(DeckReader::new);
            k.model = model;
            (k.thread, k.busy_warned) = (None, false);
            k.pending.clear();
            k.states.clear();
            k.feedback.clear();
            k.outbox.clear();
            k.learned_down.clear();
            if k.model.is_none() {
                self.messages
                    .push(format!("{} is not supported", info.name));
            } else if dev.enabled {
                k.outbox.push(DeckCmd::Open {
                    name: info.name.clone(),
                    brightness: k.brightness,
                });
                if dev.mapping.is_none() {
                    dev.mapping = k.model.as_ref().map(|m| m.name.clone());
                }
            }
        }
        for (name, dev) in &mut self.devices {
            let gone = dev.connected && !list.iter().any(|d| &d.name == name);
            if let (true, Some(k)) = (gone, dev.deck.as_mut()) {
                (dev.connected, dev.input, dev.output) = (false, false, false);
                (k.thread, k.pending, k.outbox) = (None, Vec::new(), Vec::new());
                if dev.enabled {
                    self.messages.push(format!("{name} disconnected"));
                }
            }
        }
        self.devices.retain(|_, d| d.kept());
        self.dirty = true;
        self.refresh();
    }

    /// CTL-15, 47, 56: decode a report; Learn takes a qualifying input, else it waits for the
    /// next `commands`.
    fn deck_report(&mut self, name: &str, bytes: &[u8], now: f64) {
        let Some(dev) = self.devices.get_mut(name) else {
            return;
        };
        let Some(k) = dev.deck.as_mut() else { return };
        let Some(reader) = k.reader.as_mut() else {
            return;
        };
        let events = match reader.read(bytes) {
            Ok(events) => events,
            Err(e) => {
                dev.errors += 1;
                self.dirty = true;
                if dev.logged.is_none_or(|t| now - t >= LOG_EVERY) {
                    dev.logged = Some(now);
                    eprintln!("deck: bad report from {name}: {e}");
                }
                return;
            }
        };
        let mut learned = None;
        for (input, reading) in events {
            let value = match reading {
                Reading::Button(down) => i32::from(down),
                Reading::Steps(n) => n as i32,
                _ => 1,
            };
            self.last_input = Some(LastInput {
                device: name.to_string(),
                input: input.to_string(),
                value: value.into(),
            });
            self.input_dirty = true;
            if !dev.enabled {
                continue;
            }
            if reading == Reading::Button(true) {
                k.learned_down.retain(|i| *i != input);
            } else if reading == Reading::Button(false) && k.learned_down.contains(&input) {
                // The release of a press that learned is not an input.
                k.learned_down.retain(|i| *i != input);
                continue;
            }
            if self.learn.is_none() {
                k.pending.push((input, reading));
                continue;
            }
            // CTL-56: a press or tap at once, a dial after one step; releases never.
            let qualifies = match reading {
                Reading::Button(down) => down,
                Reading::Tap => true,
                Reading::Steps(n) => n != 0.0,
                Reading::Level(_) => false,
            };
            if learned.is_none() && qualifies {
                if reading == Reading::Button(true) {
                    k.learned_down.push(input);
                }
                learned = Some((input, k.page));
            }
        }
        if let Some((input, page)) = learned {
            self.finish_deck_learn(name, input, page);
        }
    }

    /// CTL-56: bind on the deck's current deck page.
    fn finish_deck_learn(&mut self, device: &str, input: DeckInput, page: u16) {
        let Some(learn) = self.learn.take() else {
            return;
        };
        self.dirty = true;
        let mut binding = Binding::new(Input::Deck(input), learn.action);
        if input.kind == DeckKind::Key {
            binding.deck_page = page;
        }
        self.bind_learned(learn.mapping, device, binding);
    }

    /// CTL-47, 48: the deck inputs since the last call as engine commands.
    fn deck_commands(&mut self, tick: &mut Tick, out: &mut Vec<(String, Command)>) {
        let engine = tick.engine;
        for (name, dev) in &mut self.devices {
            let Some(k) = dev.deck.as_mut() else { continue };
            let inputs = std::mem::take(&mut k.pending);
            let mapping = dev.mapping.as_ref().and_then(|n| self.mappings.get(n));
            let Some(mapping) = mapping.filter(|_| dev.enabled) else {
                continue;
            };
            for (input, reading) in inputs {
                let want = Input::Deck(input);
                let Some(b) =
                    (mapping.bindings.iter()).find(|b| b.input == want && deck::on_page(b, k.page))
                else {
                    continue;
                };
                if matches!(
                    b.action,
                    Action::DeckPageNext | Action::DeckPagePrev | Action::DeckPage(_)
                ) {
                    // Only this deck turns its page, on a press (CTL-48).
                    if matches!(reading, Reading::Button(true) | Reading::Tap) {
                        let pages = deck::deck_pages(mapping);
                        let page = deck::turn_page(k.page, pages, &b.action);
                        if page != k.page {
                            k.page = page;
                            k.feedback.clear();
                            self.dirty = true;
                        }
                    }
                    continue;
                }
                // CTL-47: a continuous action on a key only shows; a dial or strip drives it.
                if b.action.value_kind() == ValueKind::Continuous
                    && !matches!(input.kind, DeckKind::Dial)
                {
                    continue;
                }
                let current = match b.action {
                    Action::Fader(t) => {
                        Some(resolve(engine, t).map_or(0.0, |id| level_of(engine, id)))
                    }
                    Action::Gm => Some(engine.grand_master()),
                    _ => None,
                };
                let values = (k.states.entry(input).or_default()).apply(
                    b,
                    reading,
                    current,
                    Resolution::Normal,
                );
                let cmds = tick.commands(b.action, &values, &mut self.keypad);
                out.extend(cmds.into_iter().map(|c| (name.clone(), c)));
            }
        }
    }

    /// After the tick and `feedback`: what the deck thread has to do (CTL-46, 52, 53).
    pub fn deck_out(&mut self, engine: &Engine) -> Vec<DeckCmd> {
        self.follow_fpp(engine.settings().faders_per_page);
        let mut wheels = None;
        let mut out = Vec::new();
        let mut changed = false;
        for (name, dev) in &mut self.devices {
            let Some(k) = dev.deck.as_mut() else { continue };
            out.append(&mut k.outbox);
            let mapping = dev.mapping.as_ref().and_then(|n| self.mappings.get(n));
            let (Some(model), Some(mapping)) = (k.model.as_ref(), mapping) else {
                continue;
            };
            if !(dev.enabled && dev.connected) {
                continue;
            }
            let pages = deck::deck_pages(mapping);
            if k.page > pages {
                k.page = pages;
                k.feedback.clear();
            }
            let px = f32::from(model.key_px.max(1));
            let looks: Vec<KeyLook> = (1..=model.keys())
                .map(|n| {
                    let b = mapping.bindings.iter().find(|b| {
                        deck::on_page(b, k.page)
                            && b.input
                                == Input::Deck(DeckInput {
                                    kind: DeckKind::Key,
                                    number: n,
                                })
                    });
                    let Some(b) = b else { return KeyLook::black() };
                    let subject = subject_of(engine, b, &mut wheels);
                    let mut look = key_look(Some(b), &subject);
                    // CTL-52: the level bar is `key_px` wide; only a moved bar changes the key.
                    look.level = look.level.map(|l| (l * px).round() / px);
                    look
                })
                .collect();
            if looks != k.looks {
                changed = true;
            }
            if k.thread == Some(DeckState::Ok) {
                let wanted = (1u16..).zip(looks.iter().cloned());
                let keys = k.feedback.tick(wanted, &[], &[]);
                if !keys.is_empty() {
                    out.push(DeckCmd::Looks {
                        name: name.clone(),
                        keys,
                    });
                }
            }
            k.looks = looks;
        }
        // CTL-57: the clients see changed looks at most 10 times a second.
        self.input_dirty |= changed;
        out
    }
}

mod osc;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod deck_tests;

#[cfg(test)]
mod osc_tests;
