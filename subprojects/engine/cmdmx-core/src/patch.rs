//! The patch: which fixture sits where.
//!
//! Phase 1 scope: profile + universe + address. User numbers, handles, modes
//! and cells come with `specs/modules/patch.md`.

use serde::{Deserialize, Serialize};

use crate::UniverseId;

pub type FixtureId = u32;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fixture {
    pub id: FixtureId,
    /// Key into the show's profile map.
    pub profile: String,
    pub universe: UniverseId,
    /// 1-based DMX start address.
    pub address: u16,
}
