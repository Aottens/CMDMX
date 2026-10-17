//! The patch: which fixture sits where.
//!
//! Phase 1 scope: profile + mode + universe + address. User numbers and handles
//! come with `specs/modules/patch.md`.

use serde::{Deserialize, Serialize};

use crate::UniverseId;

pub type FixtureId = u32;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fixture {
    pub id: FixtureId,
    /// Key into the show's profile map.
    pub profile: String,
    /// Mode name within the profile; `None` is the profile's first mode.
    #[serde(default)]
    pub mode: Option<String>,
    pub universe: UniverseId,
    /// 1-based DMX start address.
    pub address: u16,
}
