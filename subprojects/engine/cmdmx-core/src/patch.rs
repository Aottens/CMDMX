//! The patch: which fixture sits where (`specs/modules/patch.md` PAT-01..07, 12..14).
//!
//! A fixture has an internal id (assigned by the engine, never reused: every reference in the
//! engine uses it) and a user number (what the operator types, changeable). It also has a name,
//! a handle (its button in the Fixtures window) and a place: profile + mode + universe + address.
//! The checks and the edits live on the engine (`patch_add`, `patch_move`, ...), because they
//! touch the programmer, the groups and the frames too.

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
