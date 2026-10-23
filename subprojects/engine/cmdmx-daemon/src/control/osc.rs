//! OSC on the control layer (VS7-03, CTL-34..38): the `/cmdmx` schema and custom addresses as
//! inputs of the same CTL-05 table the MIDI devices use, feedback per client, Learn.
//!
//! `osc_io` moves the datagrams; everything here runs on the tick thread.

use std::collections::{BTreeMap, HashMap};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use cmdmx_control::action::{Action, AttrGroup, Key, Target, ValueKind};
use cmdmx_control::convert::{ActionValue, BindingState, Reading, Resolution};
use cmdmx_control::feedback::Feedback;
use cmdmx_control::mapping::{Binding, Input};
use cmdmx_control::osc::{self, OscArg, OscMessage};
use cmdmx_core::engine::Engine;
use cmdmx_core::playback::CueNumber;
use serde::Serialize;
use serde_json::{json, Map, Value};

use super::{resolve, state_of, Control, LastInput, Shown, Tick, LOG_EVERY};
use crate::ipc::Command;
use crate::osc_io::OscIn;
use crate::shows::valid_name;

/// CTL-37: a sender not heard for this many seconds gets no feedback.
const HEARD_FOR: f64 = 600.0;

/// CTL-35: the settings kept in `state.json` under `control.osc`.
#[derive(Debug, Clone, Serialize)]
pub(super) struct Settings {
    enabled: bool,
    port: u16,
    feedback_port: u16,
    /// IPv4 addresses or subnets; empty = any sender.
    allow: Vec<String>,
    /// `IP:port` feedback clients.
    clients: Vec<String>,
    /// The mapping for custom addresses (CTL-38).
    mapping: String,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            enabled: false,
            port: 8000,
            feedback_port: 9000,
            allow: Vec::new(),
            clients: Vec::new(),
            mapping: "OSC".into(),
        }
    }
}

fn port(v: &Value, key: &str) -> Result<u16, String> {
    (v.as_u64().and_then(|p| u16::try_from(p).ok()))
        .filter(|p| *p >= 1)
        .ok_or_else(|| format!("\"{key}\" must be a port from 1 to 65535"))
}

fn strings(v: &Value, key: &str) -> Result<Vec<String>, String> {
    (v.as_array()
        .and_then(|a| a.iter().map(|x| x.as_str().map(String::from)).collect()))
    .ok_or_else(|| format!("\"{key}\" must be a list of text"))
}

/// `10.0.0.5` or `192.168.1.0/24` as (network, mask).
fn subnet(text: &str) -> Option<(u32, u32)> {
    let (ip, prefix) = match text.split_once('/') {
        Some((ip, prefix)) => (ip, prefix.parse::<u32>().ok()?),
        None => (text, 32),
    };
    let ip: Ipv4Addr = ip.parse().ok()?;
    let mask = u32::MAX
        .checked_shl(32u32.checked_sub(prefix)?)
        .unwrap_or(0);
    Some((u32::from(ip) & mask, mask))
}

impl Settings {
    /// These settings with `change` over them; one bad entry refuses the whole change.
    fn merged(&self, change: &Map<String, Value>) -> Result<Settings, String> {
        let mut s = self.clone();
        for (key, v) in change {
            match key.as_str() {
                "enabled" => {
                    s.enabled = (v.as_bool()).ok_or("\"enabled\" must be true or false")?;
                }
                "port" => s.port = port(v, key)?,
                "feedback_port" => s.feedback_port = port(v, key)?,
                "allow" => {
                    s.allow = strings(v, key)?;
                    if let Some(bad) = s.allow.iter().find(|a| subnet(a).is_none()) {
                        return Err(format!("'{bad}' is not an IPv4 address or subnet"));
                    }
                }
                "clients" => {
                    let list = strings(v, key)?;
                    s.clients = (list.iter())
                        .map(|c| match c.parse::<SocketAddr>() {
                            Ok(a) => Ok(a.to_string()),
                            Err(_) => Err(format!("'{c}' is not IP:port")),
                        })
                        .collect::<Result<_, _>>()?;
                }
                "mapping" => {
                    let name = v.as_str().ok_or("\"mapping\" must be a mapping name")?;
                    valid_name(name)?;
                    s.mapping = name.to_string();
                }
                other => return Err(format!("unknown OSC setting \"{other}\"")),
            }
        }
        Ok(s)
    }

    /// Whether packets from `ip` are accepted (CTL-35).
    fn allows(&self, ip: IpAddr) -> bool {
        let IpAddr::V4(ip) = ip else {
            return self.allow.is_empty();
        };
        self.allow.is_empty()
            || (self.allow.iter().filter_map(|a| subnet(a)))
                .any(|(net, mask)| u32::from(ip) & mask == net)
    }
}

/// One input waiting for the next `commands`.
struct Pending {
    from: IpAddr,
    address: String,
    /// The mapping's binding, or a default one for the built-in schema.
    binding: Binding,
    reading: Reading,
    custom: bool,
}

#[derive(Default)]
pub(super) struct Osc {
    pub(super) settings: Settings,
    errors: u64,
    logged: Option<f64>,
    error: Option<String>,
    /// When each sender IP was last heard.
    heard: BTreeMap<IpAddr, f64>,
    pub(super) feedback: HashMap<SocketAddr, Feedback<String, OscArg>>,
    pending: Vec<Pending>,
    /// Memory of the custom bindings, by address.
    states: HashMap<String, BindingState>,
    /// What inputs set this tick: sender, address, the value feedback would send (CTL-20).
    echo: Vec<(IpAddr, String, OscArg)>,
    /// The last warning, for `/cmdmx/message`.
    message: String,
}

impl Osc {
    /// The settings under `control.osc` of the saved state; entries that do not hold are skipped.
    pub(super) fn load(saved: Option<&Value>) -> Osc {
        let mut settings = Settings::default();
        let saved = saved.and_then(|s| s.get("control")?.get("osc")?.as_object());
        for (k, v) in saved.into_iter().flatten() {
            let one = Map::from_iter([(k.clone(), v.clone())]);
            if let Ok(s) = settings.merged(&one) {
                settings = s;
            }
        }
        Osc {
            settings,
            ..Osc::default()
        }
    }

    /// Everyone who gets feedback: the listed clients, then the senders heard.
    fn targets(&self) -> Vec<SocketAddr> {
        let mut all: Vec<SocketAddr> = (self.settings.clients.iter())
            .filter_map(|c| c.parse().ok())
            .collect();
        for ip in self.heard.keys() {
            let a = SocketAddr::new(*ip, self.settings.feedback_port);
            if !all.contains(&a) {
                all.push(a);
            }
        }
        all
    }
}

/// A custom binding's output range applied to a level (the other way round from `Binding::scale`).
fn level_out(b: &Binding, level: f32) -> f32 {
    let x = if b.invert { 1.0 - level } else { level };
    b.min + x * (b.max - b.min)
}

/// The OSC argument for what an action shows; `None` for a wheel the programmer lacks.
fn arg_of(b: &Binding, shown: Shown) -> Option<OscArg> {
    Some(OscArg::Float(match shown {
        Shown::Level(level) => level.map_or(0.0, |l| level_out(b, l)),
        Shown::On(on) => f32::from(u8::from(on)),
        Shown::Wheel(v) => v?,
    }))
}

/// CTL-38: what a custom address says to its binding, by the action's kind.
fn custom_reading(b: &Binding, args: &[OscArg]) -> Option<Reading> {
    let first = args.first();
    match (b.action.value_kind(), first) {
        (ValueKind::Button, None) => Some(Reading::Tap),
        (ValueKind::Button, Some(a)) => Some(Reading::Button(a.as_f64()? != 0.0)),
        (ValueKind::Continuous, a) => {
            let n = (a?.as_f64()? as f32).clamp(0.0, 1.0);
            Some(Reading::Level(b.scale(n)))
        }
        (ValueKind::Relative, a) => Some(Reading::Steps(a?.as_f64()? as f32)),
    }
}

fn arg_json(a: &OscArg) -> Value {
    match a {
        OscArg::Str(s) => json!(s),
        // The shortest text of the f32, so 0.7 shows as 0.7.
        OscArg::Float(f) => f
            .to_string()
            .parse::<f64>()
            .map_or(Value::Null, |v| json!(v)),
        a => a.as_f64().map_or(Value::Null, |v| json!(v)),
    }
}

/// CTL-37: `"<number> <legend>"`, the number alone without a legend.
fn cue_texts(engine: &Engine) -> (String, String) {
    let Some(id) = engine.connected() else {
        return Default::default();
    };
    let Some(live) = engine.list_live(id) else {
        return Default::default();
    };
    let text = |n: Option<CueNumber>| {
        n.map(|n| {
            let cue = engine
                .playback(id)
                .and_then(|p| p.cues.iter().find(|c| c.number == n));
            match cue.map_or("", |c| c.legend.as_str()) {
                "" => n.to_string(),
                legend => format!("{n} {legend}"),
            }
        })
        .unwrap_or_default()
    };
    (text(live.current), text(live.next))
}

impl Control {
    /// `osc_io` events, called while the tick drains its channels; never blocks (CTL-14).
    pub fn osc_event(&mut self, ev: OscIn, now: f64) {
        match ev {
            OscIn::Listening(Ok(_)) => {
                self.osc.error = None;
                self.dirty = true;
            }
            OscIn::Listening(Err(e)) => {
                self.dirty = true;
                let text = format!("OSC: cannot listen on port {}: {e}", self.osc.settings.port);
                self.messages.push(text.clone());
                self.osc.error = Some(text);
            }
            OscIn::Packet { from, bytes } => self.osc_packet(from.ip(), &bytes, now),
        }
    }

    /// The port to listen on, `None` while OSC is off (CTL-35).
    pub fn osc_listen(&self) -> Option<u16> {
        let s = &self.osc.settings;
        s.enabled.then_some(s.port)
    }

    /// The last warning clients saw, for `/cmdmx/message` (CTL-37).
    pub fn note_warning(&mut self, text: &str) {
        text.clone_into(&mut self.osc.message);
    }

    /// `control.osc` (CTL-35, 40): all or nothing.
    pub(super) fn osc_request(&mut self, change: &Map<String, Value>) -> Result<(), String> {
        let next = self.osc.settings.merged(change)?;
        if !next.enabled {
            self.osc.heard.clear();
        }
        self.osc.error = self.osc.error.take().filter(|_| {
            (next.enabled, next.port) == (self.osc.settings.enabled, self.osc.settings.port)
        });
        self.osc.settings = next;
        // Clients may have changed, or the port they listen on: everyone gets the full state.
        self.osc.feedback.clear();
        Ok(())
    }

    /// CTL-15, 35: an ignored packet is counted; the first per minute is logged.
    fn osc_error(&mut self, what: &str, now: f64) {
        self.osc.errors += 1;
        self.dirty = true;
        if self.osc.logged.is_none_or(|t| now - t >= LOG_EVERY) {
            self.osc.logged = Some(now);
            eprintln!("osc: {what}");
        }
    }

    fn osc_packet(&mut self, ip: IpAddr, bytes: &[u8], now: f64) {
        if !self.osc.settings.enabled {
            return;
        }
        if !self.osc.settings.allows(ip) {
            return self.osc_error(&format!("sender {ip} is not allowed"), now);
        }
        let messages = match osc::decode(bytes) {
            Ok(m) => m,
            Err(e) => return self.osc_error(&format!("malformed packet from {ip}: {e}"), now),
        };
        if self.osc.heard.insert(ip, now).is_none() {
            self.dirty = true;
        }
        for m in messages {
            self.osc_message(ip, m, now);
        }
    }

    fn osc_message(&mut self, from: IpAddr, m: OscMessage, now: f64) {
        let builtin = m.address == "/cmdmx" || m.address.starts_with("/cmdmx/");
        let schema = if builtin {
            let Some((action, reading)) = osc::schema(&m) else {
                return self.osc_error(&format!("unknown address {} from {from}", m.address), now);
            };
            Some((Binding::new(Input::Osc(m.address.clone()), action), reading))
        } else {
            None
        };
        self.last_input = Some(LastInput {
            device: format!("OSC {from}"),
            input: m.address.clone(),
            value: m.args.first().map_or(Value::Null, arg_json),
        });
        self.input_dirty = true;
        let (binding, reading, custom) = match schema {
            Some((b, r)) => (b, r, false),
            // CTL-38: Learn takes the next address outside /cmdmx and does not act on it.
            None if self.learn.is_some() => return self.learn_osc(m.address),
            None => {
                let mapping = self.mappings.get(&self.osc.settings.mapping);
                let found = mapping.and_then(|map| {
                    map.bindings
                        .iter()
                        .find(|b| matches!(&b.input, Input::Osc(a) if *a == m.address))
                });
                let Some(b) = found else { return };
                let Some(reading) = custom_reading(b, &m.args) else {
                    return;
                };
                (b.clone(), reading, true)
            }
        };
        self.osc.pending.push(Pending {
            from,
            address: m.address,
            binding,
            reading,
            custom,
        });
    }

    /// CTL-25, 27, 38: bind the address to the armed action.
    fn learn_osc(&mut self, address: String) {
        let Some(learn) = self.learn.take() else {
            return;
        };
        self.dirty = true;
        let name = learn
            .mapping
            .unwrap_or_else(|| self.osc.settings.mapping.clone());
        if !self.mappings.contains_key(&name) {
            if let Err(e) = self.add_mapping(name.clone(), "OSC".into()) {
                return self.messages.push(e);
            }
        }
        let mut binding = Binding::new(Input::Osc(address.clone()), learn.action);
        // A learned OSC fader jumps: it follows its feedback like a motor (CTL-13, 38).
        binding.pickup = learn.action.value_kind() != ValueKind::Continuous;
        let old = self.mappings.get_mut(&name).and_then(|m| m.bind(binding));
        self.messages
            .push(format!("Learned: OSC {address} → {}", learn.action));
        if let Some(old) = old {
            self.messages.push(format!("{address} was {}", old.action));
        }
        if let Err(e) = self.save(&name) {
            self.messages.push(e);
        }
        self.refresh();
    }

    /// Engine commands for the OSC inputs since the last call, device `OSC` (CTL-06). Levels per
    /// address coalesced to the last, steps summed, buttons never (CTL-14).
    pub(super) fn osc_commands(&mut self, tick: &mut Tick) -> Vec<(String, Command)> {
        let mut items: Vec<Pending> = Vec::new();
        let mut at: HashMap<String, usize> = HashMap::new();
        for p in std::mem::take(&mut self.osc.pending) {
            let button = p.binding.action.value_kind() == ValueKind::Button;
            let slot = at.get(&p.address).and_then(|i| items.get_mut(*i));
            match (slot, p.reading) {
                (Some(item), Reading::Level(_)) => (item.reading, item.from) = (p.reading, p.from),
                (Some(item), Reading::Steps(more)) => {
                    if let Reading::Steps(sum) = item.reading {
                        item.reading = Reading::Steps(sum + more);
                    }
                }
                _ => {
                    if !button && matches!(p.reading, Reading::Level(_) | Reading::Steps(_)) {
                        at.insert(p.address.clone(), items.len());
                    }
                    items.push(p);
                }
            }
        }
        let mut out = Vec::new();
        for item in items {
            let b = &item.binding;
            // OSC controls follow their feedback like motor faders: never pickup (CTL-13, 37).
            let mut fresh = BindingState::default();
            let state = match item.custom {
                true => self.osc.states.entry(item.address.clone()).or_default(),
                false => &mut fresh,
            };
            let values = state.apply(b, item.reading, None, Resolution::Normal);
            let cmds = tick.commands(b.action, &values, &mut self.keypad);
            if cmds.is_empty() {
                continue;
            }
            // CTL-20: the sender is not sent back what it just set.
            let level = values.iter().rev().find_map(|v| match v {
                ActionValue::Level(l) => Some(*l),
                _ => None,
            });
            if let (None, Reading::Level(_), Some(level)) = (&b.feedback_to, item.reading, level) {
                let arg = OscArg::Float(level_out(b, level));
                self.osc.echo.push((item.from, item.address.clone(), arg));
            }
            out.extend(cmds.into_iter().map(|c| ("OSC".to_string(), c)));
        }
        out
    }

    /// Feedback after the tick's frame, for every client (CTL-19..21, 37).
    pub fn osc_feedback(&mut self, engine: &Engine, now: f64) -> Vec<(SocketAddr, OscMessage)> {
        let echo = std::mem::take(&mut self.osc.echo);
        if !self.osc.settings.enabled {
            return Vec::new();
        }
        let before = self.osc.heard.len();
        self.osc.heard.retain(|_, t| now - *t <= HEARD_FOR);
        if self.osc.heard.len() != before {
            self.dirty = true;
        }
        let targets = self.osc.targets();
        // A client that comes back starts from nothing and gets the full state (CTL-21).
        self.osc.feedback.retain(|a, _| targets.contains(a));
        let wanted = self.osc_wanted(engine);
        let mut out = Vec::new();
        for to in targets {
            let mine: Vec<(String, OscArg)> = (echo.iter())
                .filter(|(ip, ..)| *ip == to.ip())
                .map(|(_, address, arg)| (address.clone(), arg.clone()))
                .collect();
            let feedback = self.osc.feedback.entry(to).or_default();
            for (address, arg) in feedback.tick(wanted.iter().cloned(), &mine, &[]) {
                let args = vec![arg];
                out.push((to, OscMessage { address, args }));
            }
        }
        out
    }

    /// What every client should show now: the built-in list (CTL-37) and the custom bindings.
    fn osc_wanted(&self, engine: &Engine) -> Vec<(String, OscArg)> {
        let mut wheels = None;
        let plain = Binding::new(Input::Osc(String::new()), Action::Gm);
        let mut state =
            |a| arg_of(&plain, state_of(engine, a, &mut wheels)).unwrap_or(OscArg::Float(0.0));
        let text = |s: &str| OscArg::Str(s.to_string());
        let mut out: Vec<(String, OscArg)> = Vec::new();
        for n in 1..=engine.settings().faders_per_page {
            let t = Target::Slot(n);
            let legend = (resolve(engine, t).and_then(|id| engine.playback(id)))
                .map_or("", |p| p.legend.as_str());
            let base = format!("/cmdmx/fader/{n}");
            out.push((format!("{base}/upper"), state(Action::Upper(t))));
            out.push((format!("{base}/lower"), state(Action::Lower(t))));
            out.push((format!("{base}/legend"), text(legend)));
            out.push((base, state(Action::Fader(t))));
        }
        let page = engine.page();
        out.push(("/cmdmx/page".into(), OscArg::Int(i32::from(page))));
        let legend = engine.page_legend(page).unwrap_or_default();
        out.push(("/cmdmx/page/legend".into(), text(legend)));
        for (address, action) in [
            ("/cmdmx/gm", Action::Gm),
            ("/cmdmx/dbo", Action::Dbo),
            ("/cmdmx/go", Action::Go(Target::Connected)),
            ("/cmdmx/key/highlight", Action::Key(Key::Highlight)),
        ] {
            out.push((address.into(), state(action)));
        }
        for g in [
            AttrGroup::I,
            AttrGroup::P,
            AttrGroup::C,
            AttrGroup::G,
            AttrGroup::B,
            AttrGroup::E,
            AttrGroup::S,
            AttrGroup::FX,
        ] {
            let address = format!("/cmdmx/attribute_group/{g}");
            out.push((address, state(Action::AttributeGroup(g))));
        }
        let (current, next) = cue_texts(engine);
        out.push(("/cmdmx/cue/current".into(), text(&current)));
        out.push(("/cmdmx/cue/next".into(), text(&next)));
        out.push(("/cmdmx/message".into(), text(&self.osc.message)));
        // CTL-38: custom addresses answer at their own address.
        let custom = self.mappings.get(&self.osc.settings.mapping);
        for b in custom
            .into_iter()
            .flat_map(|m| &m.bindings)
            .filter(|b| b.feedback)
        {
            let Input::Osc(address) = b.feedback_to.as_ref().unwrap_or(&b.input) else {
                continue;
            };
            let mut wheels = None;
            let shown = state_of(engine, b.action, &mut wheels);
            out.extend(arg_of(b, shown).map(|arg| (address.clone(), arg)));
        }
        out
    }

    /// The snapshot's `osc` section (CTL-39).
    pub(super) fn osc_snapshot(&self) -> Value {
        let (o, s) = (&self.osc, &self.osc.settings);
        let configured = s.clients.iter().map(|c| {
            let heard = c
                .parse::<SocketAddr>()
                .ok()
                .and_then(|a| o.heard.get(&a.ip()));
            json!({"address": c, "last_heard": heard, "configured": true})
        });
        let heard = o.heard.iter().filter_map(|(ip, t)| {
            let address = SocketAddr::new(*ip, s.feedback_port).to_string();
            (!s.clients.contains(&address))
                .then(|| json!({"address": address, "last_heard": t, "configured": false}))
        });
        json!({
            "enabled": s.enabled,
            "port": s.port,
            "feedback_port": s.feedback_port,
            "allow": s.allow,
            "clients": configured.chain(heard).collect::<Vec<_>>(),
            "mapping": s.mapping,
            "errors": o.errors,
            "error": o.error,
        })
    }
}
