//! What an attribute holds: a level, or for Colour a whole colour (ADR-005).

use serde::{Deserialize, Serialize};

use crate::colour::TargetColour;

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

    /// The first component outside 0.0..=1.0 (or NaN), if any.
    pub fn out_of_range(&self) -> Option<f32> {
        let bad = |v: &f32| !(0.0..=1.0).contains(v);
        match self {
            Value::Level(v) => Some(*v).filter(bad),
            Value::Colour(c) => [c.r, c.g, c.b].iter().copied().find(bad),
        }
    }
}
