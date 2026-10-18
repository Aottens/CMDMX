//! What an attribute holds: a level, or for Colour a whole colour (ADR-005).

use serde::{Deserialize, Serialize};

use crate::colour::TargetColour;
use crate::palette::PaletteId;

/// A programmer, playback or output value. On the wire a number is a `Level` and an
/// `{ "r", "g", "b" }` object is a `Colour`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Level(f32),
    Colour(TargetColour),
}

impl From<f32> for Value {
    fn from(level: f32) -> Self {
        Value::Level(level)
    }
}

impl From<TargetColour> for Value {
    fn from(colour: TargetColour) -> Self {
        Value::Colour(colour)
    }
}

impl Value {
    /// Both are levels, or both are colours.
    pub fn same_kind(&self, other: &Value) -> bool {
        matches!(
            (self, other),
            (Value::Level(_), Value::Level(_)) | (Value::Colour(_), Value::Colour(_))
        )
    }

    /// The level, or 0 for a colour.
    pub fn level(&self) -> f32 {
        match self {
            Value::Level(v) => *v,
            Value::Colour(_) => 0.0,
        }
    }

    pub fn scaled(self, factor: f32) -> Value {
        match self {
            Value::Level(v) => Value::Level(v * factor),
            Value::Colour(c) => Value::Colour(c.scaled(factor)),
        }
    }

    /// `self` towards `to` by `k` (0.0..=1.0, exact `to` at 1.0); Colour per component in linear
    /// target RGB (PAL-13, PBK-15). Mixed kinds give `to`.
    pub fn lerp(self, to: Value, k: f32) -> Value {
        if k >= 1.0 {
            return to;
        }
        match (self, to) {
            (Value::Level(a), Value::Level(b)) => Value::Level(a + (b - a) * k),
            (Value::Colour(a), Value::Colour(b)) => Value::Colour(TargetColour {
                r: a.r + (b.r - a.r) * k,
                g: a.g + (b.g - a.g) * k,
                b: a.b + (b.b - a.b) * k,
            }),
            _ => to,
        }
    }

    /// The first component outside 0.0..=1.0 (or NaN), if any.
    pub fn out_of_range(&self) -> Option<f32> {
        let bad = |v: &f32| !(0.0..=1.0).contains(v);
        match self {
            Value::Level(v) => Some(*v).filter(bad),
            Value::Colour(c) => [c.r, c.g, c.b].iter().copied().find(bad),
        }
    }
}

/// A palette reference (PAL, `specs/modules/palettes.md`): the palette's id plus the value it
/// gave when the reference was made, which is used when the palette resolves nothing (PAL-11)
/// or is deleted (PAL-18).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    pub palette: PaletteId,
    /// The fallback.
    pub value: Value,
}

/// What the programmer and playbacks store for an attribute: a value, or a palette reference.
/// On the wire a reference is `{ "palette": id, "value": … }`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Stored {
    Value(Value),
    Reference(Reference),
}

impl Stored {
    /// The value, or a reference's fallback.
    pub fn value(&self) -> Value {
        match self {
            Stored::Value(v) => *v,
            Stored::Reference(r) => r.value,
        }
    }

    /// PAL-18: a reference to `palette` becomes its fallback value.
    pub fn without_palette(self, palette: PaletteId) -> Stored {
        match self {
            Stored::Reference(r) if r.palette == palette => Stored::Value(r.value),
            other => other,
        }
    }
}

impl From<Value> for Stored {
    fn from(value: Value) -> Self {
        Stored::Value(value)
    }
}

impl From<f32> for Stored {
    fn from(level: f32) -> Self {
        Stored::Value(Value::Level(level))
    }
}

impl From<TargetColour> for Stored {
    fn from(colour: TargetColour) -> Self {
        Stored::Value(Value::Colour(colour))
    }
}
