//! Playbacks: a handle with content, a priority, a fader mode and a live level.
//!
//! Storage and options (`specs/modules/playbacks.md` PBK-06..14, 24, 31..34); the engine owns
//! the state and the actions.
//!
//! Behaviour on the frame (VS2-21, PBK-15..23, 25..30): the live [`PlaybackState`] holds the
//! fader, Flash/Swop/Latch, the intensity master *m* and its timers. An action (fader, Flash,
//! Swop, Latch, Release) changes the inputs and fires or blocks the playback at once; the next
//! `Engine::tick` starts every timer from its own timestamp (as palette fades do, PAL-13), moves
//! *m*, captures the values LTP attributes fade from, and kills what has reached its kill
//! point. A kill turns the masked LTP attributes into a [`Release`] that fades back to what is
//! underneath. Nothing here reads a clock.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::attribute::AttributeGroup;
use crate::colour::TargetColour;
use crate::palette::{RecordBy, RecordMode};
use crate::patch::FixtureId;
use crate::value::{Stored, Value};
use crate::Timestamp;

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

/// PBK-19: when a playback leaves the merge.
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

/// The first cue's times (phase 1 has one cue); zero times and full overlap without a cue.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Times {
    pub(crate) delay: f32,
    pub(crate) fade_in: f32,
    pub(crate) fade_out: f32,
    pub(crate) overlap: f32,
}

impl Playback {
    pub(crate) fn times(&self) -> Times {
        self.cues.first().map_or(
            Times {
                delay: 0.0,
                fade_in: 0.0,
                fade_out: 0.0,
                overlap: full_overlap(),
            },
            |c| Times {
                delay: c.delay,
                fade_in: c.fade_in,
                fade_out: c.fade_out,
                overlap: c.overlap,
            },
        )
    }
}

/// Where an LTP attribute of a playback fades from (PBK-15, 17), and what a release returns to
/// when nothing else proposes it (PBK-20, "underneath"): its output in the tick before the
/// playback first asserted it. `lag` is the fixture's overlap stagger in seconds (PAL-14).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Origin {
    pub(crate) value: Value,
    pub(crate) lag: f32,
}

/// fixture → attribute → origin.
pub(crate) type Origins = BTreeMap<FixtureId, BTreeMap<String, Origin>>;

/// PBK-20: the LTP attributes of a killed playback fading back, per fixture and attribute from
/// the output at the kill to what is underneath (`to` is the playback's origin, used when no
/// other source proposes the attribute). Live state, never saved.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Release {
    /// Set by the first tick after the kill (PBK, Timing).
    pub(crate) start: Option<Timestamp>,
    pub(crate) duration: f32,
    pub(crate) entries: BTreeMap<FixtureId, BTreeMap<String, (Value, Value)>>,
}

impl Release {
    /// 0.0–1.0 at `now`; a zero release time is done as soon as the clock has started.
    pub(crate) fn progress(&self, now: Timestamp) -> f32 {
        let elapsed = self.start.map_or(0.0, |s| (now - s) as f32);
        fade_progress(elapsed, self.duration)
    }

    pub(crate) fn finished(&self, now: Timestamp) -> bool {
        self.start
            .is_some_and(|s| (now - s) as f32 >= self.duration)
    }
}

/// Linear 0.0–1.0 progress `elapsed` seconds into a fade of `fade` seconds. Before it starts
/// (`elapsed` < 0) it is 0; a zero fade is a jump at 0.
pub(crate) fn fade_progress(elapsed: f32, fade: f32) -> f32 {
    if elapsed < 0.0 {
        0.0
    } else if fade <= 0.0 {
        1.0
    } else {
        (elapsed / fade).min(1.0)
    }
}

/// Live state of a playback inside the engine.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlaybackState {
    /// Fader position 0.0..=1.0.
    pub level: f32,
    /// Set (to a monotonic counter) as soon as the effective level rises above 0: from here
    /// on the playback is in the merge (fired). A counter, not a timestamp, so two actions in
    /// the same tick still have a defined order. Cleared by a kill.
    pub activated: Option<u64>,
    /// Set when the effective level first reaches the LTP trigger point: from here on
    /// the playback's LTP values are asserted (fader modes 0/1). Later trigger
    /// wins for LTP at equal priority. Cleared together with `activated`.
    pub triggered: Option<u64>,
    /// PBK-26: Flash held.
    pub flash: bool,
    /// PBK-27: Swop held.
    pub swop: bool,
    /// PBK-28: Flash latched on.
    pub latched: bool,
    /// Order of the Swop presses: the later one wins (PBK-27).
    pub(crate) swop_seq: u64,
    /// Killed by a group or Release with the fader still up: it has to see an effective level
    /// of 0 before it fires again (PBK-22, 25).
    pub(crate) blocked: bool,
    /// The tick in which the fire / trigger reached the engine (PBK, Timing).
    pub(crate) fired_at: Option<Timestamp>,
    pub(crate) trigger_at: Option<Timestamp>,
    /// Intensity master *m* (PBK-15..17).
    pub(crate) master: f32,
    /// Mode 1: a fall of *m* in progress, from `(tick it started, m then)` (PBK-16).
    pub(crate) fall: Option<(Timestamp, f32)>,
    /// Flash/Swop/Latch ended the fade-in (PBK-26); reset by a kill.
    pub(crate) faded_in: bool,
    /// Flash/Swop/Latch was on at the last tick: its release ignores `fade_out` (PBK-26).
    pub(crate) was_holding: bool,
    pub(crate) from: Origins,
    pub(crate) release: Option<Release>,
}

impl PlaybackState {
    /// In the merge.
    pub fn is_active(&self) -> bool {
        self.activated.is_some()
    }

    /// PBK-20: killed, with LTP attributes still fading back.
    pub fn is_releasing(&self) -> bool {
        self.release.is_some()
    }

    /// PBK, Definitions: the fader level, or 1.0 while Flash or Swop is held or Flash is latched.
    pub fn effective_level(&self) -> f32 {
        if self.flash || self.swop || self.latched {
            1.0
        } else {
            self.level
        }
    }
}
