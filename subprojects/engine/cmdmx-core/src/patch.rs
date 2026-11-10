//! The patch: which fixture sits where (`specs/modules/patch.md` PAT-01..07, 12..14).
//!
//! A fixture has an internal id (assigned by the engine, never reused: every reference in the
//! engine uses it) and a user number (what the operator types, changeable). It also has a name,
//! a handle (its button in the Fixtures window) and a place: profile + mode + universe + address.
//! The checks and the edits live on the engine (`patch_add`, `patch_move`, ...), because they
//! touch the programmer, the groups and the frames too.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::UniverseId;

pub type FixtureId = u32;

/// PAT-14: a fixture button's place in the Fixtures window. Both are 1-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Handle {
    pub page: u16,
    pub slot: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fixture {
    pub id: FixtureId,
    /// The number the operator types (PAT-12). `0` on input to `Engine::patch` means "same as
    /// the id"; a stored fixture always has its real number.
    #[serde(default)]
    pub number: u32,
    #[serde(default)]
    pub name: String,
    /// Key into the show's profile map.
    pub profile: String,
    /// Mode name within the profile; `None` is the profile's first mode.
    #[serde(default)]
    pub mode: Option<String>,
    pub universe: UniverseId,
    /// 1-based DMX start address.
    pub address: u16,
    /// `None` on input gets the first free slot (PAT-14).
    #[serde(default)]
    pub handle: Option<Handle>,
    /// PAT-29: off the output map and out of every live list; `universe` and `address` stay
    /// the remembered place.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub parked: bool,
    /// PAT-44: per attribute name, what the output stage does to a level.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub behaviour: BTreeMap<String, Behaviour>,
    /// PAT-46: Pan's value goes to Tilt's channels and the other way round.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub swap_pan_tilt: bool,
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}
fn is_one(v: &f32) -> bool {
    *v == 1.0
}
fn one() -> f32 {
    1.0
}

/// PAT-44: invert, limits and offset of one attribute. All off by default.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Behaviour {
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub invert: bool,
    #[serde(skip_serializing_if = "is_zero")]
    pub low: f32,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub high: f32,
    #[serde(skip_serializing_if = "is_zero")]
    pub offset: f32,
}

impl Default for Behaviour {
    fn default() -> Self {
        Behaviour {
            invert: false,
            low: 0.0,
            high: 1.0,
            offset: 0.0,
        }
    }
}

impl Behaviour {
    /// PAT-45: invert, then offset, then the limits. Not `f32::clamp`: it panics when
    /// `low > high`, and a hand-edited show can carry that.
    pub fn apply(&self, level: f32) -> f32 {
        let v = if self.invert { 1.0 - level } else { level };
        (v + self.offset).max(self.low).min(self.high)
    }
}

/// PAT-44: the fields given; `None` leaves the field as it is.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BehaviourEdit {
    pub invert: Option<bool>,
    pub low: Option<f32>,
    pub high: Option<f32>,
    pub offset: Option<f32>,
}

/// PAT-01: `count` fixtures of one profile and mode, patched back to back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatchAdd {
    pub profile: String,
    pub mode: Option<String>,
    pub count: u32,
    pub universe: UniverseId,
    /// `None`: the first free run of channels big enough for all (PAT-03).
    pub address: Option<u16>,
    /// `None`: after the highest number in use (PAT-03).
    pub number: Option<u32>,
}
