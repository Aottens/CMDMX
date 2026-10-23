//! Mapping files: bindings from an input to an action (CTL-10, 11, 12, 22, 27).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::action::{Action, ValueKind};
use crate::convert::Reading;
use crate::midi::{Kind, MidiInput, MidiMessage};
use crate::ControlError;

/// Relative encoder encodings (CTL-11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Encoding {
    SignMagnitude,
    TwosComplement,
    BinaryOffset,
}

impl Encoding {
    /// Signed number of detents in a raw value.
    pub fn steps(self, value: u16) -> i32 {
        let v = i32::from(value.min(127));
        match self {
            Encoding::SignMagnitude => {
                if v & 0x40 != 0 {
                    -(v & 0x3F)
                } else {
                    v & 0x3F
                }
            }
            Encoding::TwosComplement => {
                if v >= 64 {
                    v - 128
                } else {
                    v
                }
            }
            Encoding::BinaryOffset => v - 64,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ButtonMode {
    #[default]
    Momentary,
    Toggle,
}

/// A MIDI input or an OSC address.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Input {
    Midi(MidiInput),
    Osc(String),
}

fn one() -> f32 {
    1.0
}
fn yes() -> bool {
    true
}
fn full() -> u16 {
    127
}
fn is_zero(v: &f32) -> bool {
    *v == 0.0
}
fn is_one(v: &f32) -> bool {
    *v == 1.0
}
fn is_yes(v: &bool) -> bool {
    *v
}
fn is_full(v: &u16) -> bool {
    *v == 127
}
fn is_zero_u16(v: &u16) -> bool {
    *v == 0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Binding {
    pub input: Input,
    pub action: Action,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub min: f32,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub max: f32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub invert: bool,
    #[serde(default, skip_serializing_if = "is_momentary")]
    pub button_mode: ButtonMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<Encoding>,
    #[serde(default = "yes", skip_serializing_if = "is_yes")]
    pub pickup: bool,
    #[serde(default = "yes", skip_serializing_if = "is_yes")]
    pub feedback: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_to: Option<Input>,
    #[serde(default = "full", skip_serializing_if = "is_full")]
    pub on_value: u16,
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub off_value: u16,
    /// Fields we do not know survive load and save (CTL-22).
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn is_momentary(m: &ButtonMode) -> bool {
    *m == ButtonMode::Momentary
}

impl Binding {
    pub fn new(input: Input, action: Action) -> Binding {
        Binding {
            input,
            action,
            min: 0.0,
            max: 1.0,
            invert: false,
            button_mode: ButtonMode::Momentary,
            encoding: None,
            pickup: true,
            feedback: true,
            feedback_to: None,
            on_value: 127,
            off_value: 0,
            extra: Map::new(),
        }
    }

    /// Message value to 0..=1: normalise, apply the input range min..max, then invert (CTL-10).
    pub fn level(&self, msg: &MidiMessage) -> f32 {
        let full = match msg.input.kind {
            Kind::Cc14 | Kind::PitchBend => 16383.0,
            _ => 127.0,
        };
        self.scale((f32::from(msg.value) / full).min(1.0))
    }

    /// A normalised input value `n` (0..=1) through the input range min..max and invert.
    pub fn scale(&self, n: f32) -> f32 {
        let x = if self.max > self.min {
            ((n - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
        } else if n >= self.max {
            1.0
        } else {
            0.0
        };
        if self.invert {
            1.0 - x
        } else {
            x
        }
    }

    /// What the message means for this binding (CTL-11, 12).
    pub fn read(&self, msg: &MidiMessage) -> Reading {
        let kind = msg.input.kind;
        match (self.encoding, kind) {
            (Some(e), Kind::Cc | Kind::Cc14) => Reading::Steps(e.steps(msg.value) as f32),
            (_, Kind::Note | Kind::Program) => Reading::Button(msg.value > 0),
            (_, Kind::Cc) if self.action.value_kind() == ValueKind::Button => {
                Reading::Button(msg.value > 0)
            }
            _ => Reading::Level(self.level(msg)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Mapping {
    pub name: String,
    pub device: String,
    pub bindings: Vec<Binding>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A binding that parses but is not valid: MIDI channels are 1..=16.
pub fn check(b: &Binding) -> Result<(), String> {
    for i in std::iter::once(&b.input).chain(b.feedback_to.as_ref()) {
        if let Input::Midi(m) = i {
            if !(1..=16).contains(&m.channel) {
                return Err(format!("channel {} outside 1..=16", m.channel));
            }
        }
    }
    Ok(())
}

impl Mapping {
    /// Load a mapping. Bad bindings are left out, one warning each (CTL-22).
    pub fn from_json(text: &str) -> Result<(Mapping, Vec<String>), ControlError> {
        let bad = |why: &str| ControlError::InvalidMapping(why.to_string());
        let Value::Object(mut obj) = serde_json::from_str(text).map_err(|e| bad(&e.to_string()))?
        else {
            return Err(bad("not a JSON object"));
        };
        let Some(Value::String(name)) = obj.remove("name") else {
            return Err(bad("missing name"));
        };
        let device = match obj.remove("device") {
            Some(Value::String(d)) => d,
            _ => "*".to_string(),
        };
        let raw = match obj.remove("bindings") {
            Some(Value::Array(a)) => a,
            _ => Vec::new(),
        };
        let mut warnings = Vec::new();
        let mut bindings = Vec::new();
        for (i, v) in raw.into_iter().enumerate() {
            let parsed = serde_json::from_value::<Binding>(v.clone())
                .map_err(|e| e.to_string())
                .and_then(|b| check(&b).map(|()| b));
            match parsed {
                Ok(b) => bindings.push(b),
                Err(e) => warnings.push(format!("binding {} left out ({e}): {v}", i + 1)),
            }
        }
        let mapping = Mapping {
            name,
            device,
            bindings,
            extra: obj,
        };
        Ok((mapping, warnings))
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// One input, one action: an equal input replaces the old binding (CTL-27).
    pub fn bind(&mut self, b: Binding) -> Option<Binding> {
        match self.bindings.iter_mut().find(|o| o.input == b.input) {
            Some(slot) => Some(std::mem::replace(slot, b)),
            None => {
                self.bindings.push(b);
                None
            }
        }
    }
}

/// Device name pattern: `*` matches any run of characters, otherwise exact. `|` separates
/// alternatives (CTL-22).
pub fn device_matches(pattern: &str, name: &str) -> bool {
    pattern.split('|').any(|p| matches_one(p, name))
}

fn matches_one(pattern: &str, name: &str) -> bool {
    let mut parts = pattern.split('*');
    let Some(first) = parts.next() else {
        return false;
    };
    let Some(mut rest) = name.strip_prefix(first) else {
        return false;
    };
    let mut parts = parts.peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            return rest.ends_with(part);
        }
        match rest.find(part) {
            Some(i) => rest = rest.get(i + part.len()..).unwrap_or(""),
            None => return false,
        }
    }
    rest.is_empty()
}
