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
use crate::{CoreError, Timestamp};

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

/// CUE-02: a cue number in thousandths (0.001–9999.999, at most three decimals), written
/// "Q2.5"; a plain JSON number. Any JSON number parses (CUE-39): one that breaks the rules
/// becomes the invalid number 0, which `Engine::from_show` leaves out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(from = "f64", into = "f64")]
pub struct CueNumber(u32);

/// The largest cue number in thousandths.
const MAX_CUE_NUMBER: u32 = 9_999_999;

impl CueNumber {
    pub fn new(number: f64) -> Option<CueNumber> {
        let thousandths = (number * 1000.0).round();
        let exact = (number * 1000.0 - thousandths).abs() < 1e-6;
        if !(1.0..=f64::from(MAX_CUE_NUMBER)).contains(&thousandths) || !exact {
            return None;
        }
        // In range, so the cast is exact.
        Some(CueNumber(thousandths as u32))
    }

    pub fn as_f64(self) -> f64 {
        f64::from(self.0) / 1000.0
    }

    /// False for the number a show file's bad number was read as (CUE-39).
    pub fn is_valid(self) -> bool {
        (1..=MAX_CUE_NUMBER).contains(&self.0)
    }

    /// CUE-05: halfway between `current` and `next`, cut to three decimals; ⌊current⌋ + 1
    /// without a next cue. `None` when no number fits.
    pub fn between(current: CueNumber, next: Option<CueNumber>) -> Option<CueNumber> {
        let at = match next {
            Some(next) => current.0 + (next.0.saturating_sub(current.0)) / 2,
            None => (current.0 / 1000 + 1) * 1000,
        };
        let fits = at > current.0 && next.is_none_or(|n| at < n.0) && at <= MAX_CUE_NUMBER;
        fits.then_some(CueNumber(at))
    }
}

impl Default for CueNumber {
    fn default() -> Self {
        CueNumber(1000)
    }
}

impl From<f64> for CueNumber {
    fn from(number: f64) -> Self {
        CueNumber::new(number).unwrap_or(CueNumber(0))
    }
}

impl From<CueNumber> for f64 {
    fn from(number: CueNumber) -> Self {
        number.as_f64()
    }
}

impl std::fmt::Display for CueNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_f64())
    }
}

/// CUE-20: how a cue follows the one before it. Offsets are seconds, 0–600.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Link {
    /// Waits for Go.
    #[default]
    Wait,
    /// Starts `offset` seconds after the previous cue has finished.
    After { offset: f32 },
    /// Starts `offset` seconds after the previous cue started.
    With { offset: f32 },
}

/// CUE-51: what a cue does with the values before it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Track {
    #[default]
    Track,
    Block,
    CueOnly,
    Solo,
}

/// CUE-14: delay and fade (seconds, 0–600) of one attribute group in a cue.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GroupTime {
    pub delay: f32,
    pub fade: f32,
}

/// PBK-34, CUE-02: one cue, its number, times (seconds 0–600, overlap 0–100 %) and values.
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
    #[serde(default)]
    pub number: CueNumber,
    /// 0–32 characters.
    #[serde(default)]
    pub legend: String,
    #[serde(default)]
    pub link: Link,
    #[serde(default)]
    pub track: Track,
    #[serde(default)]
    pub group_times: BTreeMap<AttributeGroup, GroupTime>,
    pub values: Values,
}

fn full_overlap() -> f32 {
    100.0
}

impl Cue {
    /// Number 1, zero times, full overlap, link `Wait`, tracking.
    pub fn new(values: Values) -> Self {
        Cue {
            delay: 0.0,
            fade_in: 0.0,
            fade_out: 0.0,
            overlap: full_overlap(),
            number: CueNumber::default(),
            legend: String::new(),
            link: Link::Wait,
            track: Track::Track,
            group_times: BTreeMap::new(),
            values,
        }
    }
}

/// CUE-01: what a playback is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// One cue is a memory, two or more a cue list.
    #[default]
    Cues,
    Chase,
    BpmMaster,
    RateMaster,
}

impl Kind {
    pub fn is_master(self) -> bool {
        matches!(self, Kind::BpmMaster | Kind::RateMaster)
    }
}

/// CUE-15: what 0 means on a cue list.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListFader {
    #[default]
    KillWithOff,
    KillAtZero,
}

/// CUE-21, CUE-30: what a list does after its last cue or step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoopAction {
    StopOnFinal,
    Loop,
}

/// CUE-38: the options of a cue list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListOptions {
    pub tracking: bool,
    pub fader: ListFader,
    pub fire_first_cue: bool,
    #[serde(rename = "loop")]
    pub looping: LoopAction,
}

impl Default for ListOptions {
    fn default() -> Self {
        ListOptions {
            tracking: true,
            fader: ListFader::KillWithOff,
            fire_first_cue: true,
            looping: LoopAction::StopOnFinal,
        }
    }
}

/// CUE-30.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Forward,
    Backward,
    Bounce,
    Random,
}

/// CUE-29: where a chase gets its tempo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpeedSource {
    Free,
    /// BPM master 1–8.
    Bpm(u8),
    /// Rate master 1–4.
    Rate(u8),
}

/// CUE-29, CUE-38: the options of a chase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChaseOptions {
    /// 1–600.
    pub bpm: f32,
    /// 0–100 %.
    pub crossfade: f32,
    pub direction: Direction,
    #[serde(rename = "loop")]
    pub looping: LoopAction,
    pub speed_source: SpeedSource,
}

impl Default for ChaseOptions {
    fn default() -> Self {
        ChaseOptions {
            bpm: 120.0,
            crossfade: 0.0,
            direction: Direction::Forward,
            looping: LoopAction::Loop,
            speed_source: SpeedSource::Free,
        }
    }
}

impl ChaseOptions {
    pub fn check(&self) -> Result<(), CoreError> {
        check_bpm(self.bpm)?;
        if !(0.0..=100.0).contains(&self.crossfade) {
            return Err(CoreError::InvalidOption(format!(
                "crossfade {} (expected 0 to 100 %)",
                self.crossfade
            )));
        }
        match self.speed_source {
            SpeedSource::Free => Ok(()),
            SpeedSource::Bpm(n) => MasterKind::Bpm.check_number(n),
            SpeedSource::Rate(n) => MasterKind::Rate.check_number(n),
        }
    }
}

fn check_bpm(bpm: f32) -> Result<(), CoreError> {
    if (1.0..=600.0).contains(&bpm) {
        Ok(())
    } else {
        Err(CoreError::InvalidOption(format!(
            "BPM {bpm} (expected 1 to 600)"
        )))
    }
}

/// CUE-35, CUE-36.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MasterKind {
    Bpm,
    Rate,
}

impl MasterKind {
    /// BPM masters are 1–8, Rate masters 1–4.
    pub fn max(self) -> u8 {
        match self {
            MasterKind::Bpm => 8,
            MasterKind::Rate => 4,
        }
    }

    pub fn check_number(self, number: u8) -> Result<(), CoreError> {
        if (1..=self.max()).contains(&number) {
            Ok(())
        } else {
            Err(CoreError::InvalidOption(format!(
                "master number {number} (expected 1 to {})",
                self.max()
            )))
        }
    }
}

/// CUE-35: what the fader of a BPM master does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BpmFader {
    #[serde(rename = "None")]
    Unused,
    Bpm,
    Multiplier,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BpmMaster {
    /// 1–8.
    pub number: u8,
    /// 1–600.
    pub bpm: f32,
    /// 1/4, 1/2, 1, 2 or 4.
    pub multiplier: f32,
    pub fader: BpmFader,
    /// The BPM at fader 100 %, 1–600.
    pub fader_max: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RateMaster {
    /// 1–4.
    pub number: u8,
    /// 100 or 200 %.
    pub rate_scale: u16,
}

/// CUE-35, CUE-36: the master of a `BpmMaster` or `RateMaster` playback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Master {
    Bpm(BpmMaster),
    Rate(RateMaster),
}

impl Master {
    /// A master with its defaults.
    pub fn new(kind: MasterKind, number: u8) -> Master {
        match kind {
            MasterKind::Bpm => Master::Bpm(BpmMaster {
                number,
                bpm: 120.0,
                multiplier: 1.0,
                fader: BpmFader::Unused,
                fader_max: 240.0,
            }),
            MasterKind::Rate => Master::Rate(RateMaster {
                number,
                rate_scale: 200,
            }),
        }
    }

    pub fn kind(&self) -> MasterKind {
        match self {
            Master::Bpm(_) => MasterKind::Bpm,
            Master::Rate(_) => MasterKind::Rate,
        }
    }

    pub fn number(&self) -> u8 {
        match self {
            Master::Bpm(m) => m.number,
            Master::Rate(m) => m.number,
        }
    }

    pub fn set_number(&mut self, number: u8) {
        match self {
            Master::Bpm(m) => m.number = number,
            Master::Rate(m) => m.number = number,
        }
    }

    /// The options, not the number.
    pub fn check(&self) -> Result<(), CoreError> {
        let bad = |what: String| Err(CoreError::InvalidOption(what));
        match self {
            Master::Bpm(m) => {
                check_bpm(m.bpm)?;
                if ![0.25, 0.5, 1.0, 2.0, 4.0].contains(&m.multiplier) {
                    return bad(format!(
                        "multiplier {} (expected /4, /2, 1, 2 or 4)",
                        m.multiplier
                    ));
                }
                check_bpm(m.fader_max)
            }
            Master::Rate(m) if ![100, 200].contains(&m.rate_scale) => bad(format!(
                "rate scale {} (expected 100 or 200 %)",
                m.rate_scale
            )),
            Master::Rate(_) => Ok(()),
        }
    }
}

impl std::fmt::Display for Master {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Master::Bpm(m) => write!(f, "BPM {}", m.number),
            Master::Rate(m) => write!(f, "RATE {}", m.number),
        }
    }
}

/// CUE-52: Titan's *Update* modes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateMode {
    #[default]
    Forwards,
    CueOnly,
    Backwards,
    Both,
}

/// CUE-25: the kinds of handle the buttons are assigned for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ButtonKind {
    Memory,
    CueList,
    Chase,
    BpmMaster,
    RateMaster,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Button {
    Flash,
    Swop,
    Go,
    Back,
    Stop,
    Tap,
    Release,
    None,
}

impl ButtonKind {
    /// CUE-25: the upper and lower button of `BestPractice`.
    pub fn best_practice(self) -> [Button; 2] {
        match self {
            ButtonKind::Memory => [Button::Swop, Button::Flash],
            ButtonKind::CueList => [Button::Go, Button::Flash],
            ButtonKind::Chase => [Button::Tap, Button::Flash],
            ButtonKind::BpmMaster => [Button::None, Button::Tap],
            ButtonKind::RateMaster => [Button::None, Button::None],
        }
    }

    /// CUE-25: Go, Back and Stop are for lists, Tap for chases and BPM masters, Flash, Swop and
    /// Release for everything but masters.
    pub fn allows(self, button: Button) -> bool {
        use Button::{Back, Flash, Go, Release, Stop, Swop, Tap};
        matches!(
            (self, button),
            (_, Button::None)
                | (ButtonKind::Memory, Flash | Swop | Release)
                | (
                    ButtonKind::CueList,
                    Flash | Swop | Release | Go | Back | Stop
                )
                | (ButtonKind::Chase, Flash | Swop | Release | Tap)
                | (ButtonKind::BpmMaster, Tap)
        )
    }
}

/// CUE-25: the setting `handle_buttons`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub enum HandleButtons {
    #[default]
    BestPractice,
    Titan,
    /// A pair per kind; a kind left out uses `BestPractice`.
    Custom(BTreeMap<ButtonKind, [Button; 2]>),
}

impl HandleButtons {
    pub fn check(&self) -> Result<(), CoreError> {
        let Self::Custom(pairs) = self else {
            return Ok(());
        };
        for (kind, pair) in pairs {
            if let Some(button) = pair.iter().find(|&&b| !kind.allows(b)) {
                return Err(CoreError::InvalidOption(format!(
                    "button {button:?} (not one of a {kind:?} handle)"
                )));
            }
        }
        Ok(())
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
    /// CUE-01.
    pub kind: Kind,
    /// CUE-38: used while the playback is a cue list.
    pub list: ListOptions,
    /// CUE-29: used while it is a chase.
    pub chase: ChaseOptions,
    /// CUE-35, CUE-36: on a speed master only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master: Option<Master>,
    /// In cue-number order (CUE-02); the merge reads the first until VS3-03. None on a master.
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
            kind: Kind::Cues,
            list: ListOptions::default(),
            chase: ChaseOptions::default(),
            master: None,
            cues: vec![Cue::new(values)],
        }
    }

    /// CUE, Definitions: kind `Cues` with two or more cues.
    pub fn is_cue_list(&self) -> bool {
        self.kind == Kind::Cues && self.cues.len() >= 2
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
    /// CUE-01: `Cues` or `Chase`, never to or from a master.
    pub kind: Option<Kind>,
    /// CUE-38, CUE-29, CUE-35: each the whole struct, checked.
    pub list: Option<ListOptions>,
    pub chase: Option<ChaseOptions>,
    pub master: Option<Master>,
}

/// `cue.times`, `cue.legend`, `cue.track` (CUE-09, CUE-51): every field optional, all or nothing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CueChange {
    pub legend: Option<String>,
    pub delay: Option<f32>,
    pub fade_in: Option<f32>,
    pub fade_out: Option<f32>,
    pub overlap: Option<f32>,
    pub link: Option<Link>,
    pub track: Option<Track>,
    /// Replaces the cue's group times.
    pub group_times: Option<BTreeMap<AttributeGroup, GroupTime>>,
}

/// CUE-03, CUE-05: add a cue instead of recording to a held one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddCue {
    Append,
    Insert,
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
    /// CUE-04: record to this cue number.
    pub cue: Option<CueNumber>,
    /// CUE-03, CUE-05; not together with `mode` or `cue`.
    pub add: Option<AddCue>,
    /// CUE-52: for Update; its effect is VS3-03.
    pub update_mode: Option<UpdateMode>,
}

impl PlaybackRecord {
    pub fn at(page: u16, slot: u16) -> Self {
        PlaybackRecord {
            page,
            slot,
            mask: None,
            record_by: None,
            mode: None,
            cue: None,
            add: None,
            update_mode: None,
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
