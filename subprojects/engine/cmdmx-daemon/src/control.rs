//! The daemon side of the generic MIDI control layer (VS7-02, `specs/modules/control-surfaces.md`).
//!
//! `midi_io` owns the CoreMIDI ports on its own thread and sends [`MidiEvent`]s; the tick thread
//! owns [`Control`]: devices and their states (CTL-08, 15, 16, 24), mapping files and the
//! `control` key of `state.json` (CTL-23), conversion of inputs to engine commands coalesced per
//! tick (CTL-06, 14), feedback after the tick (CTL-21), Learn (CTL-25), `last_input` (CTL-28) and
//! the `control` section of the snapshot and its delta (CTL-39).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::PathBuf;

use cmdmx_control::action::{Action, AttrGroup, Key, KeypadToken, Target, ValueKind};
use cmdmx_control::convert::{ActionValue, BindingState, Reading, Resolution};
use cmdmx_control::feedback::Feedback;
use cmdmx_control::learn;
use cmdmx_control::mapping::{device_matches, Binding, Input, Mapping};
use cmdmx_control::mcu::{self, Window};
use cmdmx_control::midi::{self, Decoder, Kind, MidiInput, MidiMessage};
use cmdmx_core::attribute::AttributeGroup;
use cmdmx_core::engine::Engine;
use cmdmx_core::palette::Timing;
use cmdmx_core::playback::{HandleButton, ListAction, PlaybackId, PAGES};
use serde_json::{json, Value};

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
}

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
        }
    }

    fn state(&self) -> &'static str {
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
    value: u16,
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
}

/// The text of an input, as the user sees it.
fn input_text(i: &Input) -> String {
    match i {
        Input::Midi(m) => m.to_string(),
        Input::Osc(a) => a.clone(),
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
    let on = |on: bool| Some(if on { b.on_value } else { b.off_value });
    let slot = |t: Target| resolve(engine, t).map(|id| level_of(engine, id));
    let continuous = |level: f32| {
        let x = if b.invert { 1.0 - level } else { level };
        let v = b.min + x * (b.max - b.min);
        Some(if win.is_some() && kind == Kind::PitchBend {
            mcu::position(v)
        } else {
            midi::to_raw(kind, v)
        })
    };
    match action {
        Action::Fader(t) => match slot(t) {
            Some(level) => continuous(level),
            None => Some(0),
        },
        Action::Gm => continuous(engine.grand_master()),
        Action::Upper(t) | Action::Lower(t) => on(resolve(engine, t)
            .and_then(|id| engine.playback_state(id))
            .is_some_and(|s| s.flash || s.latched)),
        Action::Go(t) => on(resolve(engine, t)
            .and_then(|id| engine.list_live(id))
            .is_some_and(|l| l.current.is_some())),
        Action::Page(n) => on(engine.page() == n),
        Action::Dbo => on(engine.dbo()),
        Action::Key(Key::Highlight) => on(engine.highlight()),
        Action::AttributeGroup(g) => on(engine.active_group() == core_group(g)),
        Action::Wheel(n) => {
            let all = wheels.get_or_insert_with(|| {
                (engine.programmer_view().wheels.iter())
                    .map(|w| w.value)
                    .collect()
            });
            let v = usize::from(n).checked_sub(1).and_then(|i| all.get(i));
            if win.is_some() {
                Some(mcu::ring(v.copied()))
            } else {
                Some(v.map_or(b.off_value, |v| midi::to_raw(kind, *v)))
            }
        }
        _ => on(false),
    }
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
            devices.insert(name.clone(), dev);
        }
        let mut control = Control {
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
        }
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
        for (name, dev) in &mut self.devices {
            if dev.connected && !list.iter().any(|p| &p.name == name) {
                (dev.connected, dev.input, dev.output) = (false, false, false);
                if dev.enabled {
                    self.messages.push(format!("{name} disconnected"));
                }
            }
        }
        // A device nobody enabled or assigned is forgotten once it is gone.
        self.devices
            .retain(|_, d| d.connected || d.enabled || d.mapping.is_some());
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
                value: m.value,
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
        let target = learn
            .mapping
            .or_else(|| self.devices.get(device)?.mapping.clone());
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
        let units: Vec<String> = (self.devices.iter())
            .filter(|(_, d)| d.connected && d.enabled && is_mackie(&self.mappings, d))
            .map(|(n, _)| n.clone())
            .collect();
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
            .filter(|(_, d)| d.enabled || d.mapping.is_some())
            .map(|(n, d)| {
                (
                    n.clone(),
                    json!({"enabled": d.enabled, "mapping": d.mapping}),
                )
            })
            .collect();
        update_state(&self.state_file, "control", json!({ "devices": devices }))
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
                let mapping = match self.devices.get(&device) {
                    None => return Err(format!("unknown device '{device}'")),
                    Some(d) => d.mapping.clone(),
                };
                if on && mapping.is_none() {
                    let name = self.first_mapping(&device)?;
                    if let Some(d) = self.devices.get_mut(&device) {
                        d.mapping = Some(name);
                    }
                }
                if let Some(d) = self.devices.get_mut(&device) {
                    d.enabled = on;
                }
                self.refresh();
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
                let mut merged = match m.bindings.iter().find(|b| b.input == input) {
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
            ControlRequest::Unbind { mapping, input } => {
                let m = self.mapping_mut(&mapping)?;
                let before = m.bindings.len();
                m.bindings.retain(|b| b.input != input);
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
                json!({"v": 0, "type": "control.mapping", "name": name, "mapping": mapping})
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
                json!({
                    "name": name,
                    "input": d.input,
                    "output": d.output,
                    "state": d.state(),
                    "enabled": d.enabled,
                    "mapping": d.mapping,
                    "errors": d.errors,
                })
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
            "learn": self.learn.as_ref().map(|l| l.action.to_string()),
            "last_input": last_input,
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

#[cfg(test)]
mod tests;

#[cfg(test)]
mod osc_tests;
