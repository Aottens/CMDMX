//! Playbacks: a handle with content, a priority, a fader mode and a live level.
//!
//! Storage and options (`specs/modules/playbacks.md` PBK-06..14, 24, 31..34); the engine owns
//! the state and the actions. Behaviour of times, kill and release on the frame follows with
//! VS2-21.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::attribute::AttributeGroup;
use crate::colour::TargetColour;
use crate::palette::{RecordBy, RecordMode};
use crate::patch::FixtureId;
use crate::value::Stored;

pub type PlaybackId = u32;

/// PBK-06: the number of pages of playbacks.
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

/// PBK-19: when a playback leaves the merge (behaviour on the frame is VS2-21).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum KillPoint {
    /// Effective level 0 and the intensity master has reached 0.
    #[default]
    FadeOutComplete,
    /// As soon as the effective level is 0.
    FaderAtZero,
}

/// PBK-07: Titan's Handle Paging.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Paging {
    /// Shown only on its home page.
    #[default]
    Unlocked,
    /// Shown at its slot on every page.
    Locked,
    /// Shown at its slot on every page whose own slot is empty.
    TransparentLock,
}

/// PBK-34: one cue, its times (seconds 0–600, overlap 0–100 %) and its values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cue {
    #[serde(default)]
    pub delay: f32,
    #[serde(default)]
    pub fade_in: f32,
    #[serde(default)]
    pub fade_out: f32,
    #[serde(default = "full_overlap")]
    pub overlap: f32,
    pub values: Values,
}

fn full_overlap() -> f32 {
    100.0
}

impl Cue {
    /// Zero times, full overlap.
    pub fn new(values: Values) -> Self {
        Cue {
            delay: 0.0,
            fade_in: 0.0,
            fade_out: 0.0,
            overlap: full_overlap(),
            values,
        }
    }
}

/// PBK-34: a handle's content and options. Field order is the key order in the show file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Playback {
    pub id: PlaybackId,
    /// Home handle (PBK-06): page 1..=`PAGES`, slot 1-based.
    pub page: u16,
    pub slot: u16,
    /// 0–32 characters (PBK-31).
    pub legend: String,
    /// A built-in icon id, else the automatic icon (PBK-31).
    pub icon: Option<String>,
    /// `None` = automatic (PBK-33).
    pub halo: Option<TargetColour>,
    pub priority: Priority,
    pub fader_mode: FaderMode,
    pub kill_point: KillPoint,
    /// `None` = the global setting (PBK-20).
    pub release_mask: Option<Vec<AttributeGroup>>,
    pub release_time: Option<f32>,
    pub paging: Paging,
    /// 1–99 (PBK-25).
    pub group: Option<u8>,
    /// Phase 1 records exactly one; the merge reads the first.
    pub cues: Vec<Cue>,
}

impl Playback {
    /// One cue with zero times; everything else at its default.
    pub fn new(id: PlaybackId, page: u16, slot: u16, values: Values) -> Self {
        Playback {
            id,
            page,
            slot,
            legend: String::new(),
            icon: None,
            halo: None,
            priority: Priority::Normal,
            fader_mode: FaderMode::Mode0,
            kill_point: KillPoint::default(),
            release_mask: None,
            release_time: None,
            paging: Paging::default(),
            group: None,
            cues: vec![Cue::new(values)],
        }
    }

    /// The first cue's values (phase 1 has one cue).
    pub fn values(&self) -> Option<&Values> {
        self.cues.first().map(|c| &c.values)
    }
}

/// `playback.options` and `playback.times` (PBK-36): every field optional; `Option<Option<_>>`
/// for the nullable ones (`Some(None)` clears). The times are the first cue's.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlaybackChange {
    pub priority: Option<Priority>,
    pub fader_mode: Option<FaderMode>,
    pub kill_point: Option<KillPoint>,
    pub release_mask: Option<Option<Vec<AttributeGroup>>>,
    pub release_time: Option<Option<f32>>,
    pub paging: Option<Paging>,
    pub group: Option<Option<u8>>,
    pub delay: Option<f32>,
    pub fade_in: Option<f32>,
    pub fade_out: Option<f32>,
    pub overlap: Option<f32>,
}

/// `playback.record` (PBK-36): the handle, the rest optional.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackRecord {
    pub page: u16,
    pub slot: u16,
    /// Default: every group.
    pub mask: Option<Vec<AttributeGroup>>,
    /// Default: the setting `cue_record_by`.
    pub record_by: Option<RecordBy>,
    /// Only for a used handle; `QuickMerge` is the Update function (PBK-13).
    pub mode: Option<RecordMode>,
}

impl PlaybackRecord {
    pub fn at(page: u16, slot: u16) -> Self {
        PlaybackRecord {
            page,
            slot,
            mask: None,
            record_by: None,
            mode: None,
        }
    }
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
