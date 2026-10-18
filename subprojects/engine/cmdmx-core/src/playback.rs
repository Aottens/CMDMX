//! Playbacks: a handle with content, a priority, a fader mode and a live level.
//!
//! Phase 1 scope (see `specs/modules/playbacks.md`, still a skeleton): fader
//! modes 0/1/2, priority, level, activation order. Release masks, kill points,
//! times and chases follow once that spec is written.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::patch::FixtureId;
use crate::value::Stored;

pub type PlaybackId = u32;

/// PBK-01: the number of pages of playbacks.
pub const PAGES: u16 = 10;

/// Titan's five priority levels, lowest first. Ordering is meaningful.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Normal,
    High,
    /// Same level as manually set attributes in the programmer.
    Programmer,
    VeryHigh,
}

/// Titan cue fader modes. Mode 3 (crossfade) is phase 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "u8", into = "u8")]
pub enum FaderMode {
    /// HTP follows the fader; LTP snaps at the trigger point. Fade-out time ignored.
    Mode0,
    /// HTP fades in/out with times; LTP fades in then stays.
    Mode1,
    /// Both HTP and LTP follow the fader proportionally.
    Mode2,
}

impl From<u8> for FaderMode {
    fn from(v: u8) -> Self {
        match v {
            1 => FaderMode::Mode1,
            2 => FaderMode::Mode2,
            _ => FaderMode::Mode0,
        }
    }
}

impl From<FaderMode> for u8 {
    fn from(m: FaderMode) -> Self {
        match m {
            FaderMode::Mode0 => 0,
            FaderMode::Mode1 => 1,
            FaderMode::Mode2 => 2,
        }
    }
}

/// Attribute values a playback proposes: fixture → attribute name → value or palette reference.
pub type Values = BTreeMap<FixtureId, BTreeMap<String, Stored>>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Playback {
    pub id: PlaybackId,
    #[serde(default = "default_priority")]
    pub priority: Priority,
    #[serde(default = "default_fader_mode")]
    pub fader_mode: FaderMode,
    pub values: Values,
}

fn default_priority() -> Priority {
    Priority::Normal
}

fn default_fader_mode() -> FaderMode {
    FaderMode::Mode0
}

/// Live state of a playback inside the engine.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlaybackState {
    /// Fader position 0.0..=1.0.
    pub level: f32,
    /// Set (to a monotonic counter) as soon as the fader leaves 0: from here
    /// on the playback's HTP values are in the merge, scaled by `level`.
    /// A counter, not a timestamp, so two actions in the same tick still have
    /// a defined order.
    pub activated: Option<u64>,
    /// Set when the fader first reaches the LTP trigger point: from here on
    /// the playback's LTP values are asserted (fader modes 0/1). Later trigger
    /// wins for LTP at equal priority. Cleared together with `activated`.
    pub triggered: Option<u64>,
}

impl PlaybackState {
    pub fn is_active(&self) -> bool {
        self.activated.is_some()
    }
}
