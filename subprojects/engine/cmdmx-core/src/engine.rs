//! The merge: active sources → one frame per universe per tick.
//!
//! Implements the phase-1 subset of `specs/modules/engine-merge.md`:
//! ENG-01 (HTP highest wins), ENG-02 (LTP latest wins), ENG-03 (priority),
//! ENG-04 (programmer at `Priority::Programmer`), ENG-05 (fader mode 0
//! scaling / LTP snap), ENG-06 (Grand Master), ENG-11 (determinism), ENG-12
//! (never stops on bad input).
//!
//! Colour is one value mapped nominally onto each fixture's mixing channels, with the
//! virtual dimmer and cells (ADR-005, LIB-03/04/07).
//!
//! Palette references (PAL-07, 11) resolve in the merge, every tick.
//!
//! Playback behaviour on the frame (VS2-21, PBK-15..23, 25..30): fader modes with times, kill
//! point, release, Release, playback groups, Flash/Swop/Latch and DBO. Actions change the
//! inputs; `tick` starts the timers and moves the intensity master before the merge.
//!
//! Locate, Highlight, Off, Rem Dim and Clear (VS2-05, PRG-23..29, 38..44, ENG-10): the located
//! set joins the merge under the programmer value, Highlight replaces the merged value before the
//! group masters, and a timed Clear leaves fades behind in the programmer.
//!
//! Not yet here, by design (each lands with its spec): chases, masters other than GM.

use std::collections::{BTreeMap, BTreeSet};

use crate::attribute::{AttributeDef, AttributeGroup, AttributeKind};
use crate::colour::{self, ColourModel, CtStep, TargetColour};
use crate::command::{self, Cells, Kind};
use crate::cuelist::{Ctx, Env, ListRun};
use crate::effect::{self, Clock, EffectApply, EffectSet, EntryId};
use crate::gel::{GelRef, GelTable};
use crate::output::{self, Node, NodeId, NodeKind};
use crate::palette::{
    self, Cell, Detected, Entry, GlobalValue, Limit, Palette, PaletteId, Record, RecordBy,
    RecordMode, Timing,
};
use crate::patch::{Fixture, FixtureId, Handle, PatchAdd};
use crate::playback::{
    check_bpm, fade_progress, merge_order, stagger_ranks, AddCue, BpmFader, BpmMaster, Button,
    ButtonKind, ChaseOptions, Cue, CueChange, CueEffect, CueNumber, CueValue, CueView, FaderMode,
    GroupMaster, GroupMode, GroupTime, HandleButton, HandleButtons, KillPoint,
    Kind as PlaybackKind, Layout, Link, ListAction, ListFader, ListLive, Master, MasterKind,
    Origin, Origins, Paging, Playback, PlaybackChange, PlaybackId, PlaybackRecord, PlaybackState,
    Priority, Release, SpeedSource, UpdateMode, UpdateTarget, UpdateTargets, Values, GROUP_SCALES,
    PAGES,
};
use crate::profile::{ColourMixing, ColourSystem, LogicalAttribute, Mode, Profile, COLOUR, DIMMER};
use crate::programmer::{
    default_lowlight, fan_factors, AttributeGroupState, ClearLtp, ClearPrecedence, ColourView,
    EditPart, EffectView, Fade, FanCurve, FixtureValues, Grid, GridShortcut, GroupId,
    GroupRecordMode, Item, Order, PaletteUse, Programmer, ProgrammerSettingsChange, ProgrammerView,
    Resolution, WheelState,
};
use crate::show::{
    Meta, Show, ShowGroup, ShowNode, ShowOutput, ShowPage, ShowPalette, ShowPalettes, ShowPatch,
    ShowPlaybacks, ShowSettings, SCHEMA_VERSION,
};
use crate::value::{Reference, Stored, Value};
use crate::{CoreError, Timestamp, UniverseId, Warning, UNIVERSE_SIZE};

/// One universe's output, 512 slots. Start code is added by the transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    slots: [u8; UNIVERSE_SIZE],
}

impl Default for Frame {
    fn default() -> Self {
        Frame {
            slots: [0; UNIVERSE_SIZE],
        }
    }
}

impl Frame {
    pub fn slots(&self) -> &[u8; UNIVERSE_SIZE] {
        &self.slots
    }

    /// Slot by 1-based DMX address. Out-of-range reads give `None`, never a panic.
    pub fn channel(&self, address: u16) -> Option<u8> {
        address
            .checked_sub(1)
            .and_then(|i| self.slots.get(usize::from(i)).copied())
    }

    /// Write one attribute (coarse and optional fine byte) of a fixture at `address`.
    fn set_attribute(&mut self, address: u16, attr: &AttributeDef, value: f32) {
        let (coarse, fine) = attr.to_dmx(value);
        self.set_bytes(address, attr.channel, attr.fine_channel, coarse, fine);
    }

    /// Write a colour onto one mixing system, nominally (ADR-005): RGB as is, CMY inverted.
    fn set_mixing(&mut self, address: u16, mixing: &ColourMixing, colour: TargetColour) {
        let rgb = [colour.r, colour.g, colour.b];
        for (component, value) in mixing.components.iter().zip(rgb) {
            let value = match mixing.system {
                ColourSystem::Rgb => value,
                ColourSystem::Cmy => 1.0 - value,
            };
            let (coarse, fine) = component.to_dmx(value);
            self.set_bytes(
                address,
                component.channel,
                component.fine_channel,
                coarse,
                fine,
            );
        }
    }

    fn set_bytes(
        &mut self,
        address: u16,
        channel: u16,
        fine_channel: Option<u16>,
        coarse: u8,
        fine: Option<u8>,
    ) {
        let base = address.saturating_sub(1);
        self.set(base.saturating_add(channel), coarse);
        if let (Some(fine_ch), Some(fine_v)) = (fine_channel, fine) {
            self.set(base.saturating_add(fine_ch), fine_v);
        }
    }

    /// Zero `footprint` slots from `address` (a fixture left this place).
    fn clear(&mut self, address: u16, footprint: u16) {
        for a in address..address.saturating_add(footprint) {
            self.set(a, 0);
        }
    }

    fn set(&mut self, address: u16, value: u8) {
        if let Some(slot) = address
            .checked_sub(1)
            .and_then(|i| self.slots.get_mut(usize::from(i)))
        {
            *slot = value;
        }
    }
}

/// IPC-54: what deleting a palette touches.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PaletteUsage {
    pub cues: u32,
    pub playbacks: u32,
}

/// IPC-54: what removing a set of fixtures from the patch touches.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FixtureUsage {
    pub groups: u32,
    pub cues: u32,
    pub playbacks: u32,
    pub palettes: u32,
    pub empty_groups: u32,
    pub empty_cues: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Fader level at which LTP attributes of a mode-0/1 playback are asserted.
    pub ltp_trigger_level: f32,
    /// Faders per page (configuration, never hard-coded; default 8).
    pub faders_per_page: u16,
    /// Fixture buttons per page of the Fixtures window (PAT-14).
    pub handles_per_page: u16,
    /// Wheel step sizes as fractions of the full range (PRG-21).
    pub wheel_step_normal: f32,
    pub wheel_step_fine: f32,
    pub wheel_step_coarse: f32,
    /// Master fade time (s) and overlap (%) of palettes (PAL-15).
    pub palette_time: f32,
    pub palette_overlap: f32,
    /// Default Record By of palettes (PAL-02).
    pub record_by: RecordBy,
    /// Default Record By of playbacks, and the times of a new cue (PBK-10, PBK-12).
    pub cue_record_by: RecordBy,
    pub cue_delay: f32,
    pub cue_fade_in: f32,
    /// CUE-57: `None` = As In.
    pub cue_fade_out: Option<f32>,
    pub cue_delay_out: Option<f32>,
    pub cue_overlap: f32,
    /// The global release mask and time (PBK-20).
    pub release_mask: Vec<AttributeGroup>,
    pub release_time: f32,
    /// CUE-26, CUE-15, CUE-52, CUE-25.
    pub auto_connect: bool,
    pub cue_list_fader: ListFader,
    pub update_mode: UpdateMode,
    pub handle_buttons: HandleButtons,
    /// PRG-29, PRG-43, PRG-26 (`settings.programmer`, PRG-42).
    pub clear_precedence: ClearPrecedence,
    pub clear_ltp: ClearLtp,
    pub lowlight_level: f32,
}

/// PBK-12, PBK-20 (`settings.playback`): every field optional, all or nothing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlaybackSettingsChange {
    pub record_by: Option<RecordBy>,
    pub delay: Option<f32>,
    pub fade_in: Option<f32>,
    /// CUE-57: `Some(None)` = As In.
    pub fade_out: Option<Option<f32>>,
    pub delay_out: Option<Option<f32>>,
    pub overlap: Option<f32>,
    pub release_mask: Option<Vec<AttributeGroup>>,
    pub release_time: Option<f32>,
    pub auto_connect: Option<bool>,
    pub cue_list_fader: Option<ListFader>,
    pub update_mode: Option<UpdateMode>,
    pub handle_buttons: Option<HandleButtons>,
}

/// PBK-09: the default record mask, every group.
pub const ALL_GROUPS: [AttributeGroup; 8] = [
    AttributeGroup::I,
    AttributeGroup::P,
    AttributeGroup::C,
    AttributeGroup::G,
    AttributeGroup::B,
    AttributeGroup::E,
    AttributeGroup::S,
    AttributeGroup::Fx,
];

impl Default for Settings {
    fn default() -> Self {
        Settings {
            ltp_trigger_level: 0.02,
            faders_per_page: 8,
            handles_per_page: 40,
            wheel_step_normal: 1.0 / 255.0,
            wheel_step_fine: 1.0 / 65535.0,
            wheel_step_coarse: 1.0 / 25.0,
            palette_time: 0.0,
            palette_overlap: 100.0,
            record_by: RecordBy::Mixed,
            cue_record_by: RecordBy::Channel,
            cue_delay: 0.0,
            cue_fade_in: 0.0,
            cue_fade_out: None,
            cue_delay_out: None,
            cue_overlap: 100.0,
            release_mask: Vec::new(),
            release_time: 2.0,
            auto_connect: true,
            cue_list_fader: ListFader::KillWithOff,
            update_mode: UpdateMode::Forwards,
            handle_buttons: HandleButtons::BestPractice,
            clear_precedence: ClearPrecedence::default(),
            clear_ltp: ClearLtp::default(),
            lowlight_level: default_lowlight(),
        }
    }
}

/// The engine. Deterministic: the same sequence of calls yields the same frames.
#[derive(Debug)]
pub struct Engine {
    settings: Settings,
    profiles: BTreeMap<String, Profile>,
    fixtures: BTreeMap<FixtureId, Fixture>,
    /// User number -> id (PAT-06, PAT-12).
    numbers: BTreeMap<u32, FixtureId>,
    /// Above every id ever patched, so ids are never reused (PAT-12).
    next_id: FixtureId,
    playbacks: BTreeMap<PlaybackId, Playback>,
    states: BTreeMap<PlaybackId, PlaybackState>,
    /// Above every playback id ever used, so ids are never reused (PBK, Definitions).
    playback_next_id: PlaybackId,
    /// Legends of the `PAGES` pages (PBK-08).
    page_legends: Vec<String>,
    grand_master: f32,
    /// PBK-30: Dead Black Out. Live state, not in the show.
    dbo: bool,
    /// PBK-27: the playback whose Swop wins, set by the last tick.
    swop_owner: Option<PlaybackId>,
    /// Current page of playbacks, 1..=PAGES (PBK-02). Live state, not in the show.
    page: u16,
    activation_seq: u64,
    /// PRG-72: seeds the next Shuffle. Live state: not saved, not undone.
    shuffle_seq: u64,
    frames: BTreeMap<UniverseId, Frame>,
    warnings: Vec<Warning>,
    notices: Vec<String>,
    last_tick: Option<Timestamp>,
    programmer: Programmer,
    /// PRG-49: a group holds items, whole fixtures and cells.
    groups: BTreeMap<GroupId, Vec<Item>>,
    /// PRG-79: the grid a group was recorded with; absent = grid off.
    group_grids: BTreeMap<GroupId, Grid>,
    /// PRG-90: a group's legend; an empty legend has no entry.
    group_legends: BTreeMap<GroupId, String>,
    palettes: BTreeMap<PaletteId, Palette>,
    /// Above every palette id ever used, so ids are never reused (PAL, Definitions).
    palette_next_id: PaletteId,
    /// PRG-67: the gel table `Lee n` reads.
    gels: GelTable,
    /// OUT-13: where each line is sent, by id.
    nodes: BTreeMap<NodeId, Node>,
    /// Above every node id ever used, so ids are never reused (OUT-13).
    node_next_id: NodeId,
    /// Attribute group the wheels show (PRG-17/18).
    active_group: AttributeGroup,
    /// How the Colour wheels are shown (PRG-17, PRG-30).
    colour_model: ColourModel,
    /// Last value of every LTP logical attribute per fixture: what it outputs while
    /// nothing proposes it, and where a wheel starts (LIB-03, PRG-20).
    /// PRG-51: a cell has its own entry only when the tick merges it on its own (a cell without
    /// an entry follows the fixture's held value).
    held: BTreeMap<Item, BTreeMap<String, Value>>,
    /// CUE-26: the list the Go block acts on. Live state, not in the show.
    connected: Option<PlaybackId>,
    /// PRG-26: Highlight. Live state, not in the show or the edit state.
    highlight: bool,
    /// PRG-56: Fan mode and the last curve; live state, not undoable, not saved.
    fan: Option<FanCurve>,
    fan_curve: FanCurve,
    /// Above every programmer effect entry id ever made, so ids and seeds are never reused
    /// (FX-01, FX-14). Not in the edit state.
    effect_next_id: EntryId,
    /// FX-09: the clocks of the programmer entries. Live state: undo does not rewind them (FX-20).
    effect_live: BTreeMap<EntryId, EffectLive>,
    /// FX-17: the wheels act on the current effect entry. Live state.
    effect_wheels: bool,
    /// FX-24: the effects of each active playback's cue. Live state.
    cue_fx: BTreeMap<PlaybackId, CueFx>,
}

/// FX-14: what [`Engine::effect_make`] makes an entry from.
struct EntrySpec {
    look: Option<String>,
    shape: effect::Shape,
    attributes: Vec<String>,
    params: Option<effect::Params>,
    one_per_position: bool,
    colours: Vec<TargetColour>,
    palette: Option<PaletteId>,
}

/// FX-24: the index of the cue a playback runs now, if any.
fn cue_index(playback: &Playback, state: Option<&PlaybackState>) -> Option<usize> {
    if is_list(playback) {
        let current = state.and_then(|s| s.run.current);
        current.and_then(|n| playback.cues.iter().position(|c| c.number == n))
    } else {
        (!playback.cues.is_empty() && !playback.kind.is_master()).then_some(0)
    }
}

/// FX-24: the effects a playback's current cue runs. Live state, never saved.
#[derive(Debug, Default)]
struct CueFx {
    /// The playback's activation this was built for: a new one starts over.
    activated: u64,
    /// The cue the runs were built for; `None` before the first.
    cue: Option<CueNumber>,
    runs: Vec<Run>,
}

/// FX-24, 26: one entry of a playback's cue, with its own clock.
#[derive(Debug)]
struct Run {
    /// The entry as the cue state gives it, members cut to those this run drives.
    entry: effect::Entry,
    clock: Clock,
    /// The clock runs from here (the cue's start plus its delay).
    start: Timestamp,
    last: Timestamp,
    /// FX-26: the parameters of the cue before, fading to the entry's from `(start, seconds)`.
    blend: Option<(effect::Params, Timestamp, f32)>,
    /// FX-26: the output fades in (weight 0 to 1) or, `leaving`, out over `(start, seconds)`.
    ramp: Option<(Timestamp, f32)>,
    leaving: bool,
}

impl CueFx {
    /// PAT-26: the runs that are not leaving take members, positions and count from the cue state
    /// `incoming` as it is now; clocks, starts, blends and ramps stay. A run left empty stays, so
    /// an undo brings its entry back on its clock; a member no run holds joins the first run of
    /// its entry (none: it shows at the next start).
    fn follow(&mut self, incoming: &[effect::Entry]) {
        let same = |a: &effect::Entry, b: &effect::Entry| {
            a.id == b.id && a.shape == b.shape && a.attributes == b.attributes
        };
        for r in self.runs.iter_mut().filter(|r| !r.leaving) {
            let hold: Vec<&effect::Entry> = incoming.iter().filter(|e| same(&r.entry, e)).collect();
            r.entry
                .drop_members(|m| !hold.iter().any(|e| e.members.contains(m)));
        }
        for e in incoming {
            let live: Vec<usize> = (0..self.runs.len())
                .filter(|&i| {
                    self.runs
                        .get(i)
                        .is_some_and(|r| !r.leaving && same(&r.entry, e))
                })
                .collect();
            let held = |m: &Item| {
                live.iter().any(|&i| {
                    self.runs
                        .get(i)
                        .is_some_and(|r| r.entry.members.contains(m))
                })
            };
            let missing: Vec<Item> = e.members.iter().filter(|m| !held(m)).cloned().collect();
            let sharing = live.iter().copied().find(|&i| {
                self.runs
                    .get(i)
                    .is_some_and(|r| r.entry.members.iter().any(|m| e.members.contains(m)))
            });
            let target = sharing.or(live.first().copied());
            if let Some(run) = target.and_then(|i| self.runs.get_mut(i)) {
                run.entry.members.extend(missing);
            }
        }
        for r in self.runs.iter_mut().filter(|r| !r.leaving) {
            let hold: Vec<&effect::Entry> = incoming.iter().filter(|e| same(&r.entry, e)).collect();
            let at = |m: &Item| {
                hold.iter()
                    .find_map(|e| e.members.iter().position(|x| x == m).map(|k| (*e, k)))
            };
            r.entry.positions = (r.entry.members.iter())
                .map(|m| {
                    at(m)
                        .and_then(|(e, k)| e.positions.get(k).copied())
                        .unwrap_or(0)
                })
                .collect();
            if let Some((e, _)) = r.entry.members.first().and_then(at) {
                r.entry.count = e.count;
            }
        }
    }

    /// FX-26: the runs of `cue` (its state `incoming`) at `now`, from the runs of the cue before.
    /// Per member and attribute: an entry of the same shape continues the clock and fades size,
    /// top and BPM over the fade time; otherwise the outputs crossfade (an effect the cue lacks
    /// fades out, one it starts fades in), and the first cue of a playback just starts.
    fn start(
        &mut self,
        incoming: Vec<effect::Entry>,
        cue: &Cue,
        palettes: &BTreeMap<PaletteId, Palette>,
        now: Timestamp,
    ) {
        let fresh = self.cue.is_none();
        // CUE-56: delay, fade in, delay out, fade out.
        let times = |group: AttributeGroup| match cue.group_times.get(&group) {
            Some(g) => (g.delay, g.fade, g.delay, g.fade),
            None => (
                cue.delay,
                cue.fade_in,
                cue.delay_out_in_force(),
                cue.fade_out_in_force(),
            ),
        };
        let mut before = std::mem::take(&mut self.runs);
        let mut runs: Vec<Run> = Vec::new();
        for entry in incoming {
            let (delay, fade, _, _) = times(entry.group);
            let start = now + f64::from(delay);
            let mut rest = entry;
            for a in &mut before {
                let same = a.entry.shape == rest.shape && a.entry.attributes == rest.attributes;
                if a.leaving || !same || !rest.members.iter().any(|m| a.entry.members.contains(m)) {
                    continue;
                }
                let mut taken = rest.clone();
                taken.drop_members(|m| !a.entry.members.contains(m));
                rest.drop_members(|m| a.entry.members.contains(m));
                let (was, to) = (a.params(palettes, now), pointed(palettes, &taken));
                let differs = (was.size, was.top, was.bpm) != (to.size, to.top, to.bpm);
                let hold = taken.members.clone();
                runs.push(Run {
                    blend: (differs && fade > 0.0).then_some((was, start, fade)),
                    entry: taken,
                    clock: a.clock,
                    start: a.start,
                    last: a.last,
                    ramp: a.ramp,
                    leaving: false,
                });
                a.entry.drop_members(|m| hold.contains(m));
            }
            if !rest.members.is_empty() {
                runs.push(Run {
                    entry: rest,
                    clock: Clock::new(),
                    start,
                    last: start,
                    blend: None,
                    ramp: (!fresh && fade > 0.0).then_some((start, fade)),
                    leaving: false,
                });
            }
        }
        for mut a in before {
            if a.entry.members.is_empty() {
                continue;
            }
            if !a.leaving {
                let (_, _, delay, fade) = times(a.entry.group);
                if fade <= 0.0 {
                    continue;
                }
                a.leaving = true;
                a.ramp = Some((now + f64::from(delay), fade));
            }
            runs.push(a);
        }
        self.runs = runs;
        self.cue = Some(cue.number);
    }
}

impl Run {
    /// FX-23, 26: the parameters now: the palette's while it exists, size, top and BPM faded.
    fn params(&self, palettes: &BTreeMap<PaletteId, Palette>, now: Timestamp) -> effect::Params {
        let mut p = pointed(palettes, &self.entry);
        if let Some((from, start, fade)) = self.blend {
            let k = fade_progress((now - start) as f32, fade);
            p.size = from.size + (p.size - from.size) * k;
            p.top = from.top + (p.top - from.top) * k;
            p.bpm = from.bpm + (p.bpm - from.bpm) * k;
        }
        p
    }

    /// Weight of the run's output: 0 to 1 coming in, 1 to 0 leaving.
    fn weight(&self, now: Timestamp) -> f32 {
        let Some((start, fade)) = self.ramp else {
            return 1.0;
        };
        let k = fade_progress((now - start) as f32, fade);
        if self.leaving {
            1.0 - k
        } else {
            k
        }
    }

    fn index(&self, item: Item, attribute: &str) -> Option<usize> {
        self.entry
            .attributes
            .iter()
            .any(|a| a == attribute)
            .then_some(())?;
        self.entry.members.iter().position(|m| *m == item)
    }
}

/// FX-06: the master an entry takes its tempo from.
fn speed_source(source: effect::SpeedSource) -> Option<(MasterKind, u8)> {
    match source {
        effect::SpeedSource::Free => None,
        effect::SpeedSource::Bpm(n) => Some((MasterKind::Bpm, n)),
        effect::SpeedSource::Rate(n) => Some((MasterKind::Rate, n)),
    }
}

/// FX-23: the entry's parameters, or those of the palette entry it points at while that exists.
fn pointed(palettes: &BTreeMap<PaletteId, Palette>, e: &effect::Entry) -> effect::Params {
    (e.palette.and_then(|id| palettes.get(&id)))
        .and_then(|p| {
            (p.effects.iter()).find(|x| x.shape == e.shape && x.attributes == e.attributes)
        })
        .map_or(e.params, |x| x.params)
}

/// FX-09, 10, 19: what runs for a programmer entry besides its parameters.
#[derive(Debug, Default)]
struct EffectLive {
    clock: Clock,
    /// The tick that last advanced the clock; `None` before the entry's first tick.
    last: Option<Timestamp>,
    /// FX-10: the current tap series.
    taps: Vec<Timestamp>,
    /// FX-19: a timed Clear fades the entry out; at the end the entry goes.
    fade: Option<Fade>,
}

/// UNDO (ADR-008): everything an edit command changes, as one snapshot. Not in it, so live or
/// never-reused: fades, `activated`, id counters, playback states, GM, DBO, page, held values,
/// the wheels' group and colour model, the connected list, frames.
#[derive(Debug, Clone, PartialEq)]
pub struct EditState {
    settings: Settings,
    profiles: BTreeMap<String, Profile>,
    fixtures: BTreeMap<FixtureId, Fixture>,
    numbers: BTreeMap<u32, FixtureId>,
    playbacks: BTreeMap<PlaybackId, Playback>,
    page_legends: Vec<String>,
    groups: BTreeMap<GroupId, Vec<Item>>,
    group_grids: BTreeMap<GroupId, Grid>,
    /// PRG-90: a group's legend; an empty legend has no entry.
    group_legends: BTreeMap<GroupId, String>,
    palettes: BTreeMap<PaletteId, Palette>,
    nodes: BTreeMap<NodeId, Node>,
    programmer: EditPart,
}

/// PAL-08: what applying a palette puts in the programmer: fixture, attribute, resolved value.
type PalettePlan = Vec<(Item, String, Value)>;

/// A candidate value for one attribute from one source, during a merge.
#[derive(Clone, Copy)]
struct Candidate {
    priority: Priority,
    activated: u64,
    value: Value,
    via: Via,
}

/// ENG-25: where a candidate came from. `Copy`, so the merge carries it at no cost.
#[derive(Clone, Copy, Default)]
struct Via {
    /// The playback and whether it is a list; `None`: the programmer.
    playback: Option<(PlaybackId, bool)>,
    /// The palette the stored value referred to.
    palette: Option<PaletteId>,
    /// The programmer's winning value was a located one (PRG-25).
    located: bool,
}

/// ENG-25: what the shared attribute code reports to `explain` (the tick passes none).
#[derive(Default)]
struct Trace {
    /// The merge winner, `None` when nobody proposed.
    winner: Option<Candidate>,
    /// The HTP proposals that lost.
    others: Vec<Candidate>,
    /// What the attribute gives while nobody proposes: the held value or default (LTP), 0 (HTP).
    unproposed: Option<Value>,
    modifiers: Vec<Modifier>,
}

/// ENG-25: the value `explain` gives for one attribute and how it got there.
#[derive(Debug, Clone, PartialEq)]
pub struct Explanation {
    pub name: String,
    /// `Some(n)` for the attribute of cell `n` (LIB-07), `None` for the fixture's own.
    pub cell: Option<u16>,
    /// What the tick writes for the attribute (before a virtual dimmer scales it).
    pub value: Value,
    pub base: Base,
    /// HTP sources that proposed a value and lost.
    pub others: Vec<Proposal>,
    pub modifiers: Vec<Modifier>,
}

/// ENG-25: the merge winner (ENG-01..10) and its value before any modifier.
#[derive(Debug, Clone, PartialEq)]
pub struct Base {
    pub source: Source,
    pub value: Value,
    /// The value is not hard in the list's current cue (CUE-50); false for anything but a list.
    pub tracked: bool,
    /// The palette the value refers to (PAL-07).
    pub palette: Option<PaletteId>,
}

/// ENG-25: who gave a value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Source {
    Programmer,
    /// A playback; `cue` is the current cue of a list or chase, `None` for a memory.
    Playback {
        id: PlaybackId,
        cue: Option<CueNumber>,
    },
    /// An LTP attribute nobody proposes keeps its last value.
    Held,
    /// An HTP attribute nobody proposes is 0.
    Default,
}

/// ENG-25: an HTP source that proposed a value and lost.
#[derive(Debug, Clone, PartialEq)]
pub struct Proposal {
    pub source: Source,
    pub value: Value,
}

/// ENG-25: what changed the value after the merge, in the order the tick applies it.
#[derive(Debug, Clone, PartialEq)]
pub enum Modifier {
    Effect,
    /// A group master holding the fixture (GRM-06..10), also at full; `level` is what it applies.
    Master {
        playback: PlaybackId,
        mode: GroupMode,
        level: f32,
        enabled: bool,
    },
    /// The Grand Master, below full only.
    GrandMaster(f32),
    Dbo,
    /// The programmer's value is a located one (PRG-25).
    Locate,
    Highlight,
    /// ENG-21: the colour mix wrote this emitter or RGB attribute.
    AutoMix,
}

/// A programmer or located value that was taken out (Off, Clear), for PRG-43 and ENG-10.
struct Gone {
    item: Item,
    name: String,
    kind: AttributeKind,
    default: Value,
    value: Value,
}

impl Engine {
    pub fn new(settings: Settings) -> Self {
        Engine {
            settings,
            profiles: BTreeMap::new(),
            fixtures: BTreeMap::new(),
            numbers: BTreeMap::new(),
            next_id: 1,
            playbacks: BTreeMap::new(),
            states: BTreeMap::new(),
            playback_next_id: 1,
            page_legends: vec![String::new(); usize::from(PAGES)],
            grand_master: 1.0,
            dbo: false,
            swop_owner: None,
            page: 1,
            activation_seq: 0,
            shuffle_seq: 1,
            frames: BTreeMap::new(),
            warnings: Vec::new(),
            notices: Vec::new(),
            last_tick: None,
            programmer: Programmer::default(),
            groups: BTreeMap::new(),
            group_grids: BTreeMap::new(),
            group_legends: BTreeMap::new(),
            palettes: BTreeMap::new(),
            palette_next_id: 1,
            gels: GelTable::lee(),
            // OUT-25: line 1 goes out on Art-Net, sACN and the first serial interface.
            nodes: [
                NodeKind::default_artnet(1),
                NodeKind::default_sacn(1),
                NodeKind::Serial {
                    port: "auto".into(),
                },
            ]
            .into_iter()
            .zip(1..)
            .map(|(kind, id)| {
                let node = Node {
                    id,
                    line: 1,
                    enabled: true,
                    kind,
                };
                (id, node)
            })
            .collect(),
            node_next_id: 4,
            active_group: AttributeGroup::I,
            colour_model: ColourModel::default(),
            held: BTreeMap::new(),
            connected: None,
            highlight: false,
            fan: None,
            fan_curve: FanCurve::default(),
            effect_next_id: 1,
            effect_live: BTreeMap::new(),
            effect_wheels: false,
            cue_fx: BTreeMap::new(),
        }
    }

    // ---- setup -----------------------------------------------------------

    /// PAT-09: does the show already hold a profile under this key?
    pub fn has_profile(&self, key: &str) -> bool {
        self.profiles.contains_key(key)
    }

    /// PAT-09: the profile the show holds under this key (its modes feed Change Mode).
    pub fn profile(&self, key: &str) -> Option<&Profile> {
        self.profiles.get(key)
    }

    /// LIB-02: an invalid profile is refused and never reaches the patch.
    pub fn add_profile(
        &mut self,
        key: impl Into<String>,
        profile: Profile,
    ) -> Result<(), CoreError> {
        profile.validate()?;
        self.profiles.insert(key.into(), profile);
        Ok(())
    }

    /// LIB-25: replace the show's copy of a profile, all or nothing. A mode a fixture uses keeps
    /// its name and footprint, and an attribute the programmer, a cue or a palette stores for a
    /// fixture of this type stays. Returns how many patched fixtures use the profile.
    pub fn save_profile(&mut self, key: &str, profile: Profile) -> Result<usize, CoreError> {
        profile.validate_for_save()?;
        let users: Vec<&Fixture> = self
            .fixtures
            .values()
            .filter(|f| f.profile == key)
            .collect();
        let mut modes: BTreeMap<FixtureId, &Mode> = BTreeMap::new();
        for f in &users {
            let old = lookup_mode(&self.profiles, key, f.mode.as_deref())?;
            let same = profile
                .mode(Some(&old.name))
                .filter(|m| m.footprint == old.footprint)
                .filter(|m| {
                    profile
                        .mode(f.mode.as_deref())
                        .is_some_and(|n| n.name == m.name)
                });
            let Some(new) = same else {
                return Err(CoreError::ProfileInUse(format!(
                    "mode '{}' is used by fixture {} and must keep {} channels",
                    old.name, f.number, old.footprint
                )));
            };
            modes.insert(f.id, new);
        }
        // PRG-53: a cell the new mode lacks is kept, so only a cell that stays can lose a name.
        let gone = |item: &Item, name: &String| {
            (modes.get(&item.fixture)).is_some_and(|m| {
                item.cell.is_none_or(|c| m.cell(c).is_some())
                    && m.item_logical(item.cell, name).is_none()
            })
        };
        let used = |name: &str, by: String| {
            Err(CoreError::ProfileInUse(format!("{name} is used by {by}")))
        };
        let programmer = (self.programmer.values().iter())
            .flat_map(|(id, v)| v.keys().map(move |n| (id, n)))
            .chain(
                (self.programmer.located().iter())
                    .flat_map(|(id, v)| v.keys().map(move |n| (id, n))),
            );
        for (id, name) in programmer {
            if gone(id, name) {
                return used(name, "the programmer".into());
            }
        }
        for playback in self.playbacks.values() {
            for cue in &playback.cues {
                for (id, values) in cue.stored_items() {
                    if let Some(name) = values.keys().find(|n| gone(id, n)) {
                        return used(name, format!("cue {} of list {}", cue.number, playback.id));
                    }
                }
            }
        }
        for palette in self.palettes.values() {
            let by_fixture = palette
                .fixtures
                .iter()
                .find_map(|(id, v)| v.keys().find(|n| gone(id, n)));
            let shared = palette.shared.get(key).and_then(|v| {
                (v.keys()).find(|n| modes.keys().any(|&id| gone(&Item::fixture(id), n)))
            });
            let shared_cells = (palette.shared_cells.get(key).into_iter().flatten())
                .flat_map(|(&c, v)| v.keys().map(move |n| (c, n)))
                .find(|&(c, n)| modes.keys().any(|&id| gone(&Item::cell(id, c), n)))
                .map(|(_, n)| n);
            if let Some(name) = by_fixture.or(shared).or(shared_cells) {
                return used(name, format!("palette {}", palette.number));
            }
        }
        let ids: Vec<FixtureId> = modes.keys().copied().collect();
        let old = self.profiles.insert(key.to_string(), profile);
        let old: BTreeMap<String, Profile> =
            old.map(|p| (key.to_string(), p)).into_iter().collect();
        for id in &ids {
            let old_mode = self.fixtures.get(id).and_then(|f| resolve_mode(&old, f));
            self.refresh_held(*id, old_mode);
        }
        Ok(ids.len())
    }

    /// LIB-25: after a profile copy changed, a fixture's held values follow it: names the mode
    /// lost go, new LTP attributes start at their default, and a value still at the old default
    /// takes the new one (a value a source proposed stays).
    fn refresh_held(&mut self, id: FixtureId, old: Option<&Mode>) {
        let (Some(fixture), Some(held)) =
            (self.fixtures.get(&id), self.held.get(&Item::fixture(id)))
        else {
            return;
        };
        let Some(new) = resolve_mode(&self.profiles, fixture) else {
            return;
        };
        let next: BTreeMap<String, Value> = new
            .logical_attributes()
            .filter(|l| l.kind == AttributeKind::Ltp)
            .map(|l| {
                let old_default = old.and_then(|m| m.logical(l.name)).map(|o| o.default);
                let value = (held.get(l.name).copied())
                    .filter(|v| l.default.same_kind(v) && Some(*v) != old_default)
                    .unwrap_or(l.default);
                (l.name.to_string(), value)
            })
            .collect();
        self.held.insert(Item::fixture(id), next);
    }

    /// Patch a fixture under its own id (scenario files, test rigs, the show file later).
    /// Rejected (not panicking) when the id or number is taken, the profile or mode is
    /// unknown, or the footprint runs past slot 512 or onto another fixture (PAT-02).
    /// `number == 0` means "same as the id". LTP attributes start at their default (LIB-03).
    pub fn patch(&mut self, mut fixture: Fixture) -> Result<(), CoreError> {
        if fixture.number == 0 {
            fixture.number = fixture.id;
        }
        if self.fixtures.contains_key(&fixture.id) {
            return Err(CoreError::DuplicateFixture(fixture.id));
        }
        let footprint =
            lookup_mode(&self.profiles, &fixture.profile, fixture.mode.as_deref())?.footprint;
        // PAT-34: a parked fixture holds no channels, so it has no clash to check.
        if !fixture.parked {
            self.check_place(
                fixture.number,
                fixture.universe,
                u32::from(fixture.address),
                footprint,
                &[],
            )?;
        } else if fixture.address == 0 || u32::from(fixture.address) > UNIVERSE_SIZE as u32 {
            return Err(CoreError::AddressOutOfRange {
                fixture: fixture.number,
                universe: fixture.universe,
                address: fixture.address,
            });
        }
        if self.numbers.contains_key(&fixture.number) {
            return Err(CoreError::NumberInUse(fixture.number));
        }
        self.install(fixture)
    }

    /// PAT-01..03, 07: patch `count` fixtures back to back. The whole placement is computed and
    /// checked first; a refused add changes nothing. Returns the new ids in patch order.
    pub fn patch_add(&mut self, add: &PatchAdd) -> Result<Vec<FixtureId>, CoreError> {
        self.add_fixtures(add, false)
    }

    /// PAT-32: as [`Engine::patch_add`], but the fixtures the new ones overlap are parked in the
    /// same command and a notice names them; a used number still refuses with nothing parked.
    pub fn patch_add_parking(&mut self, add: &PatchAdd) -> Result<Vec<FixtureId>, CoreError> {
        self.add_fixtures(add, true)
    }

    fn add_fixtures(&mut self, add: &PatchAdd, park: bool) -> Result<Vec<FixtureId>, CoreError> {
        if add.count == 0 {
            return Err(CoreError::InvalidCount(0));
        }
        // ponytail: a zero-channel mode is treated as one channel wide when placing.
        let footprint = lookup_mode(&self.profiles, &add.profile, add.mode.as_deref())?
            .footprint
            .max(1);
        if add.number == Some(0) {
            return Err(CoreError::InvalidNumber);
        }
        let first_number = match (add.number, self.numbers.keys().next_back()) {
            (Some(n), _) => n,
            (None, None) => 1,
            (None, Some(max)) => max.checked_add(1).ok_or(CoreError::NoFreeNumbers)?,
        };
        let first_id = self.next_id;
        first_id
            .checked_add(add.count)
            .ok_or(CoreError::NoFreeNumbers)?;
        let first_address = match add.address {
            Some(a) => u32::from(a),
            None => self
                .first_free_run(add.universe, u64::from(add.count) * u64::from(footprint))
                .ok_or(CoreError::NoFreeAddresses {
                    universe: add.universe,
                })?,
        };
        // Refused at the first fixture that does not fit, so this stays within a universe.
        let mut placed = Vec::new();
        let mut address = first_address;
        for i in 0..add.count {
            let number = first_number
                .checked_add(i)
                .ok_or(CoreError::NoFreeNumbers)?;
            if park {
                self.check_range(number, add.universe, address, footprint)?;
            } else {
                self.check_place(number, add.universe, address, footprint, &[])?;
            }
            if self.numbers.contains_key(&number) {
                return Err(CoreError::NumberInUse(number));
            }
            placed.push((first_id + i, number, address));
            address += u32::from(footprint);
        }
        let clashing = if park {
            self.overlapped(add.universe, first_address, address, &[])
        } else {
            Vec::new()
        };
        self.park_ids(&clashing);
        let first_on_universe = self.universe_is_empty(add.universe);
        let mut ids = Vec::new();
        for (id, number, address) in placed {
            self.install(Fixture {
                parked: false,
                id,
                number,
                name: String::new(),
                profile: add.profile.clone(),
                mode: add.mode.clone(),
                universe: add.universe,
                address: u16::try_from(address).unwrap_or(u16::MAX),
                handle: None,
            })?;
            ids.push(id);
        }
        self.add_default_nodes(add.universe, first_on_universe);
        if let (Some(first), false) = (ids.first(), clashing.is_empty()) {
            let parked = self.numbers_of(&clashing);
            let place = self.fixtures.get(first).map_or(0, |f| f.address);
            let what = match ids.len() {
                1 => format!("Added {}", self.fixtures.get(first).map_or(0, |f| f.number)),
                many => format!("Added {many} fixtures"),
            };
            self.notices.push(format!(
                "{what} at {}.{place}, parked {parked}",
                add.universe
            ));
        }
        Ok(ids)
    }

    fn universe_is_empty(&self, universe: UniverseId) -> bool {
        self.fixtures.values().all(|f| f.universe != universe)
    }

    /// OUT-26: the first fixture on a universe that has no node yet adds that line's default
    /// Art-Net and sACN nodes (each only if its destination is free), with one warning.
    fn add_default_nodes(&mut self, line: UniverseId, was_empty: bool) {
        if !was_empty || self.nodes.values().any(|n| n.line == line) {
            return;
        }
        let mut names = Vec::new();
        let port_address = line.saturating_sub(1);
        for (kind, label) in [
            (
                NodeKind::default_artnet(line),
                format!("Art-Net {}", output::artnet_address(port_address)),
            ),
            (NodeKind::default_sacn(line), format!("sACN {line}")),
        ] {
            if self.output_add(line, true, kind).is_ok() {
                names.push(label);
            }
        }
        if !names.is_empty() {
            let at = self.last_tick.unwrap_or(0.0);
            self.warnings.push(Warning {
                at,
                message: format!("Universe {line} now goes out on {}", names.join(" and ")),
            });
        }
    }

    /// OUT-28: add a node; refused with a reason, nothing changes.
    pub fn output_add(
        &mut self,
        line: UniverseId,
        enabled: bool,
        kind: NodeKind,
    ) -> Result<NodeId, CoreError> {
        let id = self.node_next_id;
        let node = Node {
            id,
            line,
            enabled,
            kind,
        };
        output::check(&node, self.nodes.values())?;
        self.node_next_id = id.saturating_add(1);
        self.nodes.insert(id, node);
        Ok(id)
    }

    /// OUT-28: replace a node by id; its kind cannot change.
    pub fn output_edit(&mut self, node: Node) -> Result<(), CoreError> {
        let old = self
            .nodes
            .get(&node.id)
            .ok_or(CoreError::UnknownNode(node.id))?;
        if std::mem::discriminant(&old.kind) != std::mem::discriminant(&node.kind) {
            return Err(CoreError::NodeKindChanged(node.id));
        }
        output::check(&node, self.nodes.values())?;
        self.nodes.insert(node.id, node);
        Ok(())
    }

    /// OUT-28: remove nodes; all ids are checked first.
    pub fn output_remove(&mut self, ids: &[NodeId]) -> Result<(), CoreError> {
        if let Some(&unknown) = ids.iter().find(|id| !self.nodes.contains_key(id)) {
            return Err(CoreError::UnknownNode(unknown));
        }
        for id in ids {
            self.nodes.remove(id);
        }
        Ok(())
    }

    /// OUT-13: the nodes sorted by id.
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.values()
    }

    /// PAT-04: give a fixture a new place. Its own old channels do not count as a clash; they
    /// drop to 0 and the new ones are written at the next tick. References follow the id.
    pub fn patch_move(
        &mut self,
        id: FixtureId,
        universe: UniverseId,
        address: u16,
    ) -> Result<(), CoreError> {
        let fixture = self
            .fixtures
            .get(&id)
            .ok_or(CoreError::UnknownFixture(id))?;
        let footprint = self.footprint_of(fixture)?;
        self.check_place(
            fixture.number,
            universe,
            u32::from(address),
            footprint,
            &[id],
        )?;
        let (old_universe, old_address, was_parked) =
            (fixture.universe, fixture.address, fixture.parked);
        if !was_parked {
            self.clear_channels(old_universe, old_address, footprint);
        }
        self.frames.entry(universe).or_default();
        let was_empty = self.universe_is_empty(universe);
        if let Some(f) = self.fixtures.get_mut(&id) {
            f.universe = universe;
            f.address = address;
            f.parked = false; // PAT-31
        }
        if was_parked {
            self.parking_changed();
        }
        self.add_default_nodes(universe, was_empty);
        Ok(())
    }

    /// PAT-29: take fixtures off the output map; the place is remembered. A range command
    /// (PAT-19); one already parked stays as it is. They leave the selection, the programmer, the
    /// located set and the programmer's effects as in PAT-05; groups, palettes and cues are
    /// untouched and their channels drop to 0.
    pub fn patch_park(&mut self, ids: &[FixtureId]) -> Result<(), CoreError> {
        self.range_ids(ids)?;
        self.park_ids(ids);
        Ok(())
    }

    fn park_ids(&mut self, ids: &[FixtureId]) {
        for &id in ids {
            let Some(f) = self.fixtures.get(&id).filter(|f| !f.parked) else {
                continue;
            };
            let (universe, address) = (f.universe, f.address);
            let footprint = self.footprint_of(f).unwrap_or(0);
            if let Some(f) = self.fixtures.get_mut(&id) {
                f.parked = true;
            }
            self.clear_channels(universe, address, footprint);
            self.programmer.remove_fixture(id);
        }
        self.parking_changed();
        self.follow_active_group();
    }

    /// PAT-30: after a fixture was parked or unparked, effect positions are worked out over the
    /// members that hang, in every cue and palette, and the running cues follow.
    fn parking_changed(&mut self) {
        let parked: BTreeSet<FixtureId> = (self.fixtures.values())
            .filter(|f| f.parked)
            .map(|f| f.id)
            .collect();
        let is_parked = |m: &Item| parked.contains(&m.fixture);
        let cue_effects = (self.playbacks.values_mut())
            .flat_map(|pb| &mut pb.cues)
            .flat_map(|c| &mut c.effects);
        let palette_effects = (self.palettes.values_mut()).flat_map(|p| &mut p.effects);
        for e in cue_effects.chain(palette_effects) {
            e.refresh_over(is_parked);
        }
        self.follow_cue_effects();
    }

    /// PAT-31: put parked fixtures back at their remembered place. Each is checked against the
    /// fixtures that hang (and against the others being unparked); the first clash refuses the
    /// whole command. One already unparked stays as it is.
    pub fn patch_unpark(&mut self, ids: &[FixtureId]) -> Result<(), CoreError> {
        let wake: Vec<&Fixture> = (self.range_ids(ids)?.iter())
            .filter_map(|id| self.fixtures.get(id))
            .filter(|f| f.parked)
            .collect();
        for (i, f) in wake.iter().enumerate() {
            let footprint = self.footprint_of(f)?;
            let clash = match self.check_place(
                f.number,
                f.universe,
                u32::from(f.address),
                footprint,
                &[],
            ) {
                Err(CoreError::PatchOverlap {
                    number,
                    other,
                    universe,
                    address,
                }) => Some((number, other, universe, address)),
                Err(e) => return Err(e),
                Ok(()) => wake.iter().take(i).find_map(|o| {
                    let (a, b) = (u32::from(f.address), u32::from(o.address));
                    let (wa, wb) = (footprint, self.footprint_of(o).ok()?);
                    (o.universe == f.universe && a < b + u32::from(wb) && b < a + u32::from(wa))
                        .then_some((f.number, o.number, f.universe, f.address.max(o.address)))
                }),
            };
            if let Some((number, other, universe, address)) = clash {
                return Err(CoreError::UnparkOverlap {
                    number,
                    other,
                    universe,
                    address,
                });
            }
        }
        let wake: Vec<FixtureId> = wake.iter().map(|f| f.id).collect();
        for id in wake {
            if let Some(f) = self.fixtures.get_mut(&id) {
                f.parked = false;
            }
        }
        self.parking_changed();
        Ok(())
    }

    /// PAT-32: as [`Engine::patch_move_range`], but a fixture outside the range whose channels
    /// the new places overlap is parked in the same command and a notice says so. A move past
    /// slot 512 is still refused, with nothing parked.
    pub fn patch_move_parking(
        &mut self,
        ids: &[FixtureId],
        universe: UniverseId,
        address: u16,
    ) -> Result<(), CoreError> {
        let sorted = self.range_ids(ids)?;
        let mut end = u32::from(address);
        for f in sorted.iter().filter_map(|id| self.fixtures.get(id)) {
            let footprint = self.footprint_of(f)?;
            self.check_range(f.number, universe, end, footprint)?;
            end += u32::from(footprint.max(1));
        }
        let clashing = self.overlapped(universe, u32::from(address), end, &sorted);
        self.park_ids(&clashing);
        self.patch_move_range(&sorted, universe, address)?;
        if !clashing.is_empty() {
            let what = match sorted[..] {
                [id] => format!("Moved {}", self.fixtures.get(&id).map_or(0, |f| f.number)),
                _ => format!("Moved {} fixtures", sorted.len()),
            };
            let parked = self.numbers_of(&clashing);
            self.notices
                .push(format!("{what} to {universe}.{address}, parked {parked}"));
        }
        Ok(())
    }

    /// The non-parked fixtures outside `own` with a channel in `first..end` on `universe`.
    fn overlapped(
        &self,
        universe: UniverseId,
        first: u32,
        end: u32,
        own: &[FixtureId],
    ) -> Vec<FixtureId> {
        (self.fixtures.values())
            .filter(|f| f.universe == universe && !f.parked && !own.contains(&f.id))
            .filter(|f| {
                let at = u32::from(f.address);
                let width = self.footprint_of(f).map_or(0, u32::from);
                at < end && first < at + width
            })
            .map(|f| f.id)
            .collect()
    }

    /// The user numbers of `ids` in number order, for a notice.
    fn numbers_of(&self, ids: &[FixtureId]) -> String {
        let mut numbers: Vec<u32> = (ids.iter())
            .filter_map(|id| self.fixtures.get(id))
            .map(|f| f.number)
            .collect();
        numbers.sort_unstable();
        let words: Vec<String> = numbers.iter().map(u32::to_string).collect();
        words.join(", ")
    }

    /// PAT-05: remove fixtures from the patch, the selection, the programmer, every group and
    /// every palette (PAL-19) and every cue of every playback (PBK-14). PAT-26: the fixture and
    /// its cells also leave the effects of every cue and palette (positions worked out again, an
    /// entry this emptied goes) and the runs of the cues playing. All ids are checked first.
    pub fn patch_remove(&mut self, ids: &[FixtureId]) -> Result<(), CoreError> {
        self.range_ids(ids)?;
        for &id in ids {
            let footprint = self
                .fixtures
                .get(&id)
                .and_then(|f| self.footprint_of(f).ok())
                .unwrap_or(0);
            let Some(fixture) = self.fixtures.remove(&id) else {
                continue;
            };
            if !fixture.parked {
                self.clear_channels(fixture.universe, fixture.address, footprint);
            }
            self.numbers.remove(&fixture.number);
            self.held.retain(|i, _| i.fixture != id);
            self.programmer.remove_fixture(id);
            for members in self.groups.values_mut() {
                members.retain(|i| i.fixture != id);
            }
            for palette in self.palettes.values_mut() {
                palette.fixtures.retain(|i, _| i.fixture != id); // PAL-19
                effect::Entry::retire(&mut palette.effects, |m| m.fixture == id);
                // PAT-26
            }
            for cue in self.playbacks.values_mut().flat_map(|pb| &mut pb.cues) {
                cue.values.retain(|i, _| i.fixture != id); // PBK-14
                cue.dormant.retain(|i, _| i.fixture != id); // CUE-61
                cue.order.retain(|i, _| i.fixture != id);
                effect::Entry::retire(&mut cue.effects, |m| m.fixture == id); // PAT-26
            }
        }
        self.parking_changed(); // PAT-30: positions over the members that hang; runs follow
        self.follow_active_group();
        Ok(())
    }

    /// PAT-06: change the user number; everything else refers to the id.
    pub fn patch_renumber(&mut self, id: FixtureId, number: u32) -> Result<(), CoreError> {
        if number == 0 {
            return Err(CoreError::InvalidNumber);
        }
        let old = self
            .fixtures
            .get(&id)
            .ok_or(CoreError::UnknownFixture(id))?
            .number;
        if self.numbers.get(&number).is_some_and(|&other| other != id) {
            return Err(CoreError::NumberInUse(number));
        }
        self.numbers.remove(&old);
        self.numbers.insert(number, id);
        if let Some(f) = self.fixtures.get_mut(&id) {
            f.number = number;
        }
        Ok(())
    }

    /// PAT-09: the legend only; id, number and place stay.
    pub fn patch_name(&mut self, id: FixtureId, name: &str) -> Result<(), CoreError> {
        let f = self
            .fixtures
            .get_mut(&id)
            .ok_or(CoreError::UnknownFixture(id))?;
        f.name = name.to_string();
        Ok(())
    }

    /// PAT-13: give a fixture another mode of its profile. Attributes are matched by name;
    /// programmer and playback values the new mode cannot take are dropped with a warning,
    /// held values of attributes that remain are kept. The fixture leaves effect entries the new
    /// mode has no attribute of (PAT-27); palettes keep theirs and warn (PAT-28). A bigger
    /// footprint is checked like a move.
    pub fn patch_change_mode(&mut self, id: FixtureId, mode: &str) -> Result<(), CoreError> {
        let fixture = self
            .fixtures
            .get(&id)
            .ok_or(CoreError::UnknownFixture(id))?;
        let new = lookup_mode(&self.profiles, &fixture.profile, Some(mode))?;
        self.check_place(
            fixture.number,
            fixture.universe,
            u32::from(fixture.address),
            new.footprint,
            &[id],
        )?;
        let at = self.last_tick.unwrap_or(0.0);
        let palette_lines = self.palette_mode_warnings(&[id], mode);
        self.warnings.extend(
            palette_lines
                .into_iter()
                .map(|message| Warning { at, message }),
        );
        let dropped = self.apply_mode(id, mode)?;
        self.warnings.extend(
            dropped
                .into_iter()
                .map(|(message, _)| Warning { at, message }),
        );
        Ok(())
    }

    /// The value- and effect-dropping half of a mode change (PAT-13, 27); returns
    /// `(warning, attribute)` per dropped value or effect attribute.
    fn apply_mode(
        &mut self,
        id: FixtureId,
        mode: &str,
    ) -> Result<Vec<(String, String)>, CoreError> {
        let fixture = self
            .fixtures
            .get(&id)
            .ok_or(CoreError::UnknownFixture(id))?;
        let old_footprint = self.footprint_of(fixture)?;
        let new = lookup_mode(&self.profiles, &fixture.profile, Some(mode))?;
        let (number, universe, address) = (fixture.number, fixture.universe, fixture.address);
        let parked = fixture.parked;
        // A value stays only when the new mode has that attribute with the same kind of value.
        let keeps = |name: &str, value: &Stored| {
            new.logical(name)
                .is_some_and(|l| l.default.same_kind(&value.value()))
        };
        let mut warnings: Vec<(String, String)> = Vec::new();
        let mut dropped_programmer = Vec::new();
        // PRG-53: a cell item's values stay in every mode; only the fixture's own are checked.
        for (name, value) in (self.programmer.values_of(Item::fixture(id)))
            .into_iter()
            .flatten()
        {
            if !keeps(name, value) {
                dropped_programmer.push(name.clone());
                warnings.push((
                    format!(
                        "fixture {number}: programmer value '{name}' dropped, mode '{mode}' has no such attribute"
                    ),
                    name.clone(),
                ));
            }
        }
        for playback in self.playbacks.values_mut() {
            for cue in &mut playback.cues {
                for values in [&mut cue.values, &mut cue.dormant] // CUE-61
                    .into_iter()
                    .filter_map(|v| v.get_mut(&Item::fixture(id)))
                {
                    values.retain(|name, value| {
                        let keep = keeps(name, value);
                        if !keep {
                            warnings.push((
                                format!(
                                    "fixture {number}: playback {} value '{name}' dropped, mode '{mode}' has no such attribute",
                                    playback.id
                                ),
                                name.clone(),
                            ));
                        }
                        keep
                    });
                }
            }
        }
        // PAT-27: a whole-fixture member leaves an entry the new mode has none of the attributes of.
        let item = Item::fixture(id);
        let gone = |e: &effect::Entry| e.attributes.iter().all(|a| new.logical(a).is_none());
        for playback in self.playbacks.values_mut() {
            let pid = playback.id;
            for cue in &mut playback.cues {
                cue.effects.retain_mut(|e| {
                    if !gone(e) || !e.members.contains(&item) {
                        return true;
                    }
                    for a in &e.attributes {
                        warnings.push((
                            format!(
                                "fixture {number}: playback {pid} effect '{a}' dropped, mode '{mode}' has no such attribute"
                            ),
                            a.clone(),
                        ));
                    }
                    e.drop_members(|m| *m == item);
                    e.refresh();
                    !e.members.is_empty()
                });
            }
        }
        let leaving: Vec<_> = (self.programmer.effects.iter())
            .filter(|e| gone(e) && e.members.contains(&item))
            .map(|e| (e.id, e.attributes.clone()))
            .collect();
        for (_, attributes) in &leaving {
            for a in attributes {
                warnings.push((
                    format!(
                        "fixture {number}: programmer effect '{a}' dropped, mode '{mode}' has no such attribute"
                    ),
                    a.clone(),
                ));
            }
        }
        self.held.retain(|i, _| i.cell.is_none() || i.fixture != id);
        let old_held = self.held.remove(&Item::fixture(id)).unwrap_or_default();
        let held: BTreeMap<String, Value> = new
            .logical_attributes()
            .filter(|l| l.kind == AttributeKind::Ltp)
            .map(|l| {
                let value = old_held
                    .get(l.name)
                    .copied()
                    .filter(|v| l.default.same_kind(v))
                    .unwrap_or(l.default);
                (l.name.to_string(), value)
            })
            .collect();
        for name in dropped_programmer {
            self.programmer.remove_value(Item::fixture(id), &name);
        }
        self.held.insert(Item::fixture(id), held);
        if !parked {
            self.clear_channels(universe, address, old_footprint);
        }
        if let Some(f) = self.fixtures.get_mut(&id) {
            f.mode = Some(mode.to_string());
        }
        if !leaving.is_empty() {
            self.effects_leave(|e, m| *m == item && leaving.iter().any(|(i, _)| *i == e.id));
        }
        self.follow_cue_effects();
        Ok(warnings)
    }

    /// PAT-28: per palette (number order) the attributes it holds for the `ids` that `mode` lacks
    /// while their old mode has them; global values, cells and memberless entries say nothing.
    fn palette_mode_warnings(&self, ids: &[FixtureId], mode: &str) -> Vec<String> {
        let mut palettes: Vec<_> = self.palettes.values().collect();
        palettes.sort_by_key(|p| p.number);
        let mut lines = Vec::new();
        for p in palettes {
            let (mut lost, mut count) = (BTreeSet::new(), 0);
            for &id in ids {
                let Some(f) = self.fixtures.get(&id) else {
                    continue;
                };
                let (Some(old), Ok(new)) = (
                    resolve_mode(&self.profiles, f),
                    lookup_mode(&self.profiles, &f.profile, Some(mode)),
                ) else {
                    continue;
                };
                let item = Item::fixture(id);
                let held = (p.fixtures.get(&item).into_iter())
                    .chain(p.shared.get(&f.profile))
                    .flat_map(|m| m.keys().cloned())
                    .chain(
                        (p.effects.iter().filter(|e| e.members.contains(&item)))
                            .flat_map(|e| e.attributes.iter().cloned()),
                    );
                let mut any = false;
                for a in held {
                    if old.logical(&a).is_some() && new.logical(&a).is_none() {
                        lost.insert(a);
                        any = true;
                    }
                }
                count += usize::from(any);
            }
            if count > 0 {
                let name = if p.legend.is_empty() {
                    format!("Palette {}", p.number)
                } else {
                    format!("Palette {} {}", p.number, p.legend)
                };
                let attrs: Vec<_> = lost.into_iter().collect();
                let s = if count == 1 { "" } else { "s" };
                lines.push(format!(
                    "{name}: no {} on {count} fixture{s} in this mode, the value stays",
                    attrs.join(", ")
                ));
            }
        }
        lines
    }

    /// PAT-19: the range's ids sorted by user number; empty, unknown or repeated ids refuse.
    fn range_ids(&self, ids: &[FixtureId]) -> Result<Vec<FixtureId>, CoreError> {
        if ids.is_empty() {
            return Err(CoreError::EmptySelection);
        }
        let mut seen = BTreeSet::new();
        for &id in ids {
            if !self.fixtures.contains_key(&id) {
                return Err(CoreError::UnknownFixture(id));
            }
            if !seen.insert(id) {
                return Err(CoreError::RepeatedFixture(id));
            }
        }
        let mut sorted = ids.to_vec();
        sorted.sort_by_key(|id| self.fixtures.get(id).map_or(0, |f| f.number));
        Ok(sorted)
    }

    /// PAT-20: the range back to back from `address` in number order; all or nothing.
    pub fn patch_move_range(
        &mut self,
        ids: &[FixtureId],
        universe: UniverseId,
        address: u16,
    ) -> Result<(), CoreError> {
        let sorted = self.range_ids(ids)?;
        if let [id] = sorted[..] {
            return self.patch_move(id, universe, address);
        }
        let mut places = Vec::new();
        let mut next = u32::from(address);
        for &id in &sorted {
            let fixture = self
                .fixtures
                .get(&id)
                .ok_or(CoreError::UnknownFixture(id))?;
            let footprint = self.footprint_of(fixture)?;
            self.check_place(fixture.number, universe, next, footprint, &sorted)?;
            places.push((id, next, fixture.universe, fixture.address, footprint));
            next += u32::from(footprint.max(1));
        }
        self.frames.entry(universe).or_default();
        let was_empty = self.universe_is_empty(universe);
        let mut unparked = false;
        for (id, new_address, old_universe, old_address, footprint) in places {
            if self.fixtures.get(&id).is_some_and(|f| f.parked) {
                unparked = true; // PAT-31: its remembered channels are not its own now
            } else {
                self.clear_channels(old_universe, old_address, footprint);
            }
            if let Some(f) = self.fixtures.get_mut(&id) {
                f.universe = universe;
                f.address = u16::try_from(new_address).unwrap_or(f.address);
                f.parked = false;
            }
        }
        if unparked {
            self.parking_changed();
        }
        self.add_default_nodes(universe, was_empty);
        Ok(())
    }

    /// PAT-21: consecutive user numbers from `number` in number order; all or nothing.
    pub fn patch_renumber_range(
        &mut self,
        ids: &[FixtureId],
        number: u32,
    ) -> Result<(), CoreError> {
        let sorted = self.range_ids(ids)?;
        if let [id] = sorted[..] {
            return self.patch_renumber(id, number);
        }
        if number == 0 {
            return Err(CoreError::InvalidNumber);
        }
        let mut new = Vec::new();
        for (i, &id) in sorted.iter().enumerate() {
            let n = u32::try_from(i)
                .ok()
                .and_then(|i| number.checked_add(i))
                .ok_or(CoreError::NoFreeNumbers)?;
            if self
                .numbers
                .get(&n)
                .is_some_and(|other| !sorted.contains(other))
            {
                return Err(CoreError::NumberInUse(n));
            }
            new.push((id, n));
        }
        for f in sorted.iter().filter_map(|id| self.fixtures.get(id)) {
            self.numbers.remove(&f.number);
        }
        for (id, n) in new {
            self.numbers.insert(n, id);
            if let Some(f) = self.fixtures.get_mut(&id) {
                f.number = n;
            }
        }
        Ok(())
    }

    /// PAT-23: a trailing number in `name` counts up in number order, zero padding kept.
    pub fn patch_name_range(&mut self, ids: &[FixtureId], name: &str) -> Result<(), CoreError> {
        let sorted = self.range_ids(ids)?;
        if let [id] = sorted[..] {
            return self.patch_name(id, name);
        }
        let prefix = name.trim_end_matches(|c: char| c.is_ascii_digit());
        let digits = name.get(prefix.len()..).unwrap_or_default();
        let start = digits.parse::<u64>().ok();
        for (i, &id) in sorted.iter().enumerate() {
            let label = match start.and_then(|n| n.checked_add(i as u64)) {
                Some(n) => format!("{prefix}{n:0width$}", width = digits.len()),
                None => name.to_string(),
            };
            if let Some(f) = self.fixtures.get_mut(&id) {
                f.name = label;
            }
        }
        Ok(())
    }

    /// PAT-22: every fixture of the range takes `mode` in place; one warning for dropped values.
    pub fn patch_change_mode_range(
        &mut self,
        ids: &[FixtureId],
        mode: &str,
    ) -> Result<(), CoreError> {
        let sorted = self.range_ids(ids)?;
        if let [id] = sorted[..] {
            return self.patch_change_mode(id, mode);
        }
        // (id, number, universe, first, end) of each fixture's new footprint.
        let mut spans = Vec::new();
        for &id in &sorted {
            let f = self
                .fixtures
                .get(&id)
                .ok_or(CoreError::UnknownFixture(id))?;
            let new = lookup_mode(&self.profiles, &f.profile, Some(mode)).map_err(|e| match e {
                CoreError::UnknownMode { .. } => CoreError::FixtureLacksMode {
                    fixture: f.number,
                    mode: mode.to_string(),
                },
                other => other,
            })?;
            let first = u32::from(f.address);
            spans.push((
                f.number,
                f.universe,
                first,
                first + u32::from(new.footprint),
            ));
        }
        for &(number, universe, first, end) in &spans {
            self.check_place(number, universe, first, (end - first) as u16, &sorted)?;
            let clash = spans
                .iter()
                .filter(|&&(other, u, f, e)| {
                    other != number && u == universe && first < e && f < end
                })
                .map(|&(other, _, f, _)| (first.max(f), other))
                .min();
            if let Some((channel, other)) = clash {
                return Err(CoreError::PatchOverlap {
                    number,
                    other,
                    universe,
                    address: u16::try_from(channel).unwrap_or(u16::MAX),
                });
            }
        }
        let at = self.last_tick.unwrap_or(0.0);
        let palette_lines = self.palette_mode_warnings(&sorted, mode);
        self.warnings.extend(
            palette_lines
                .into_iter()
                .map(|message| Warning { at, message }),
        );
        let mut attrs = BTreeSet::new();
        let mut affected = 0;
        for &id in &sorted {
            let dropped = self.apply_mode(id, mode)?;
            affected += usize::from(!dropped.is_empty());
            attrs.extend(dropped.into_iter().map(|(_, attr)| attr));
        }
        if affected > 0 {
            let attrs: Vec<_> = attrs.into_iter().collect();
            self.warnings.push(Warning {
                at,
                message: format!("Dropped {} on {affected} fixtures", attrs.join(", ")),
            });
        }
        Ok(())
    }

    /// PAT-14: move a fixture's button to a free slot, or swap with whoever holds it.
    pub fn patch_handle(&mut self, id: FixtureId, handle: Handle) -> Result<(), CoreError> {
        let old = self
            .fixtures
            .get(&id)
            .ok_or(CoreError::UnknownFixture(id))?
            .handle;
        if handle.page == 0 || handle.slot == 0 || handle.slot > self.settings.handles_per_page {
            return Err(CoreError::InvalidHandle {
                page: handle.page,
                slot: handle.slot,
            });
        }
        let occupant = self
            .fixtures
            .values()
            .find(|f| f.id != id && f.handle == Some(handle))
            .map(|f| f.id);
        if let Some(other) = occupant.and_then(|o| self.fixtures.get_mut(&o)) {
            other.handle = old;
        }
        if let Some(f) = self.fixtures.get_mut(&id) {
            f.handle = Some(handle);
        }
        Ok(())
    }

    /// Store a checked fixture: number index, handle, held defaults and frame (LIB-03).
    fn install(&mut self, mut fixture: Fixture) -> Result<(), CoreError> {
        lookup_mode(&self.profiles, &fixture.profile, fixture.mode.as_deref())?;
        if fixture.handle.is_none() {
            fixture.handle = self.first_free_handle();
        }
        self.init_held(&fixture)?;
        self.numbers.insert(fixture.number, fixture.id);
        self.next_id = self.next_id.max(fixture.id.saturating_add(1));
        self.fixtures.insert(fixture.id, fixture);
        Ok(())
    }

    /// Held LTP values and frame slots of a fixture start at the mode defaults (LIB-03).
    fn init_held(&mut self, fixture: &Fixture) -> Result<(), CoreError> {
        let mode = lookup_mode(&self.profiles, &fixture.profile, fixture.mode.as_deref())?;
        let frame = self.frames.entry(fixture.universe).or_default();
        let held = self.held.entry(Item::fixture(fixture.id)).or_default();
        held.clear();
        // A virtual dimmer starts dark: its Dimmer is HTP, so nothing proposes it yet.
        let scale = if mode.has_virtual_dimmer() { 0.0 } else { 1.0 };
        for l in mode
            .logical_attributes()
            .filter(|l| l.kind == AttributeKind::Ltp)
        {
            held.insert(l.name.to_string(), l.default);
            if !fixture.parked {
                write_value(frame, fixture.address, mode, &l, l.default, scale);
            }
        }
        Ok(())
    }

    fn footprint_of(&self, fixture: &Fixture) -> Result<u16, CoreError> {
        lookup_mode(&self.profiles, &fixture.profile, fixture.mode.as_deref()).map(|m| m.footprint)
    }

    fn clear_channels(&mut self, universe: UniverseId, address: u16, footprint: u16) {
        if let Some(frame) = self.frames.get_mut(&universe) {
            frame.clear(address, footprint);
        }
    }

    /// Channel spans `(first, last, number)` of the fixtures on a universe, except `own`.
    fn spans(&self, universe: UniverseId, own: &[FixtureId]) -> Vec<(u32, u32, u32)> {
        self.fixtures
            .values()
            .filter(|f| f.universe == universe && !f.parked && !own.contains(&f.id))
            .filter_map(|f| {
                let first = u32::from(f.address);
                let width = u32::from(self.footprint_of(f).ok()?);
                Some((first, (first + width).saturating_sub(1), f.number))
            })
            .collect()
    }

    /// PAT-02: `footprint` channels from `address` must lie within 1..=512.
    fn check_range(
        &self,
        number: u32,
        universe: UniverseId,
        address: u32,
        footprint: u16,
    ) -> Result<(), CoreError> {
        let end = address.saturating_add(u32::from(footprint)); // first channel after
        if address == 0 || end > UNIVERSE_SIZE as u32 + 1 {
            return Err(CoreError::AddressOutOfRange {
                fixture: number,
                universe,
                address: u16::try_from(address).unwrap_or(u16::MAX),
            });
        }
        Ok(())
    }

    /// PAT-02: `footprint` channels from `address` must lie within 1..=512 and clear every other
    /// fixture; the error names the first clashing channel.
    fn check_place(
        &self,
        number: u32,
        universe: UniverseId,
        address: u32,
        footprint: u16,
        own: &[FixtureId],
    ) -> Result<(), CoreError> {
        self.check_range(number, universe, address, footprint)?;
        let at = u16::try_from(address).unwrap_or(u16::MAX);
        let end = address.saturating_add(u32::from(footprint)); // first channel after
        let clash = self
            .spans(universe, own)
            .into_iter()
            .filter(|&(first, last, _)| address <= last && first < end)
            .map(|(first, _, other)| (address.max(first), other))
            .min();
        match clash {
            Some((channel, other)) => Err(CoreError::PatchOverlap {
                number,
                other,
                universe,
                address: u16::try_from(channel).unwrap_or(at),
            }),
            None => Ok(()),
        }
    }

    /// PAT-03: the first address on the universe with `total` free channels in a row.
    fn first_free_run(&self, universe: UniverseId, total: u64) -> Option<u32> {
        let total = u32::try_from(total)
            .ok()
            .filter(|t| *t <= UNIVERSE_SIZE as u32)?;
        let mut spans = self.spans(universe, &[]);
        spans.sort();
        let mut candidate = 1;
        for (first, last, _) in spans {
            if first >= candidate + total {
                break;
            }
            candidate = candidate.max(last + 1);
        }
        (candidate + total <= UNIVERSE_SIZE as u32 + 1).then_some(candidate)
    }

    /// PAT-14: the first free slot, page by page.
    fn first_free_handle(&self) -> Option<Handle> {
        let taken: BTreeSet<(u16, u16)> = self
            .fixtures
            .values()
            .filter_map(|f| f.handle)
            .map(|h| (h.page, h.slot))
            .collect();
        (1..=u16::MAX)
            .flat_map(|page| (1..=self.settings.handles_per_page).map(move |slot| (page, slot)))
            .find(|key| !taken.contains(key))
            .map(|(page, slot)| Handle { page, slot })
    }

    /// Place a playback as it is (tests, scenarios, the show file): nothing is checked, and the
    /// next playback id is raised above its id.
    pub fn add_playback(&mut self, mut playback: Playback) {
        for cue in &mut playback.cues {
            self.settle_entries(&mut cue.effects);
        }
        self.playback_next_id = self.playback_next_id.max(playback.id.saturating_add(1));
        let state = self.states.entry(playback.id).or_default();
        if let Some(Master::Group(m)) = &playback.master {
            state.level = m.level; // GRM-05: the fader starts at the saved level
        }
        self.playbacks.insert(playback.id, playback);
    }

    // ---- undo (UNDO, ADR-008) ----------------------------------------------

    pub fn edit_state(&self) -> EditState {
        EditState {
            settings: self.settings.clone(),
            profiles: self.profiles.clone(),
            fixtures: self.fixtures.clone(),
            numbers: self.numbers.clone(),
            playbacks: self.playbacks.clone(),
            page_legends: self.page_legends.clone(),
            groups: self.groups.clone(),
            group_grids: self.group_grids.clone(),
            group_legends: self.group_legends.clone(),
            palettes: self.palettes.clone(),
            nodes: self.nodes.clone(),
            programmer: self.programmer.edit_part(),
        }
    }

    /// UNDO-10..12. Infallible: a whole-snapshot swap cannot fail to apply (UNDO-17). PRG-45: an
    /// LTP value the restored programmer no longer holds is let go of as Clear does.
    pub(crate) fn restore_edit_state(&mut self, s: EditState) {
        let same_place = |a: &Fixture, b: &Fixture| {
            (a.universe, a.address, &a.profile, &a.mode, a.parked)
                == (b.universe, b.address, &b.profile, &b.mode, b.parked)
        };
        // Old channels of removed or changed fixtures go dark (old profiles still in place).
        let gone: Vec<(UniverseId, u16, u16)> = self
            .fixtures
            .values()
            .filter(|f| !f.parked && s.fixtures.get(&f.id).is_none_or(|n| !same_place(f, n)))
            .map(|f| (f.universe, f.address, self.footprint_of(f).unwrap_or(0)))
            .collect();
        for (universe, address, footprint) in gone {
            self.clear_channels(universe, address, footprint);
        }
        self.settings = s.settings;
        let old_profiles = std::mem::replace(&mut self.profiles, s.profiles);
        self.numbers = s.numbers;
        self.page_legends = s.page_legends;
        self.groups = s.groups;
        self.group_grids = s.group_grids;
        self.group_legends = s.group_legends;
        self.palettes = s.palettes;
        self.nodes = s.nodes;
        let old_fixtures = std::mem::replace(&mut self.fixtures, s.fixtures);
        let patched = old_fixtures.len() != self.fixtures.len()
            || old_fixtures.iter().any(|(id, o)| {
                self.fixtures
                    .get(id)
                    .is_none_or(|n| (&o.profile, &o.mode) != (&n.profile, &n.mode))
            });
        self.held
            .retain(|item, _| self.fixtures.contains_key(&item.fixture));
        let fixtures: Vec<Fixture> = self.fixtures.values().cloned().collect();
        for f in fixtures {
            self.frames.entry(f.universe).or_default();
            let kept = old_fixtures
                .get(&f.id)
                .is_some_and(|o| (&o.profile, &o.mode) == (&f.profile, &f.mode));
            if !kept && self.init_held(&f).is_err() {
                self.held.remove(&Item::fixture(f.id));
            } else if kept {
                // LIB-25: undoing a profile edit moves the defaults back on the output too.
                let old = old_fixtures
                    .get(&f.id)
                    .and_then(|o| resolve_mode(&old_profiles, o));
                self.refresh_held(f.id, old);
            }
        }
        // Playbacks: removed ones are killed, returned ones start fresh, changed ones keep
        // running and snap to their contents; a group master keeps its current level.
        let mut new = s.playbacks;
        self.states.retain(|id, _| new.contains_key(id));
        for pb in new.values_mut() {
            let old = self.playbacks.get(&pb.id);
            if let (Some(Master::Group(m)), Some(Master::Group(o))) =
                (&mut pb.master, old.and_then(|o| o.master.as_ref()))
            {
                m.level = o.level;
            }
            let state = self.states.entry(pb.id).or_default();
            match old {
                Some(o) => state.run.edited |= o != &*pb,
                None => {
                    if let Some(Master::Group(m)) = &pb.master {
                        state.level = m.level;
                    }
                }
            }
        }
        self.playbacks = new;
        if patched {
            self.follow_cue_effects(); // PAT-26: a fixture came, went or changed mode
        }
        let p = &self.programmer;
        let mut held_before: Vec<(Item, String)> = Vec::new();
        for item in p.touched_items() {
            let values = p.values_of(item).into_iter().flat_map(|a| a.keys());
            let located = p.located_of(item).into_iter().flat_map(|a| a.keys());
            held_before.extend(values.chain(located).map(|n| (item, n.clone())));
        }
        self.programmer.restore_edit_part(s.programmer);
        let gone: Vec<Gone> = held_before
            .into_iter()
            .filter(|(i, n)| {
                self.programmer.stored_item(*i, n).is_none()
                    && self.programmer.located_value(*i, n).is_none()
            })
            .filter_map(|(item, name)| {
                let l = self.mode_of(item.fixture)?.item_logical(item.cell, &name)?;
                Some(Gone {
                    item,
                    name,
                    kind: l.kind,
                    default: l.default,
                    value: l.default, // unused: give_up(.., 0.0) never fades
                })
            })
            .collect();
        self.give_up(gone, 0.0);
        self.follow_active_group();
    }

    // ---- show file (SHW-01..08) --------------------------------------------

    /// SHW-01..06: the show data as a `Show`; live state (faders, GM, programmer, selection,
    /// held values, the wheels' group) is not part of it. Collections come out sorted by id,
    /// only profiles in use are kept (SHW-05), and playback values for fixtures that are no
    /// longer patched are left out, so what is saved loads clean (SHW-07).
    pub fn to_show(&self, meta: Meta) -> Show {
        let s = &self.settings;
        Show {
            schema_version: SCHEMA_VERSION,
            meta,
            settings: ShowSettings {
                faders_per_page: s.faders_per_page,
                handles_per_page: s.handles_per_page,
                ltp_trigger_level: s.ltp_trigger_level,
                wheel_step_normal: s.wheel_step_normal,
                wheel_step_fine: s.wheel_step_fine,
                wheel_step_coarse: s.wheel_step_coarse,
                colour_model: self.colour_model,
                palette_time: s.palette_time,
                palette_overlap: s.palette_overlap,
                record_by: s.record_by,
                cue_record_by: s.cue_record_by,
                cue_delay: s.cue_delay,
                cue_fade_in: s.cue_fade_in,
                cue_fade_out: s.cue_fade_out,
                cue_delay_out: s.cue_delay_out,
                cue_overlap: s.cue_overlap,
                release_mask: s.release_mask.clone(),
                release_time: s.release_time,
                auto_connect: s.auto_connect,
                cue_list_fader: s.cue_list_fader,
                update_mode: s.update_mode,
                handle_buttons: s.handle_buttons.clone(),
                clear_precedence: s.clear_precedence,
                clear_ltp: s.clear_ltp,
                lowlight_level: s.lowlight_level,
            },
            profiles: self
                .profiles
                .iter()
                .filter(|(key, _)| self.fixtures.values().any(|f| &f.profile == *key))
                .map(|(key, profile)| (key.clone(), profile.clone()))
                .collect(),
            patch: ShowPatch {
                next_id: self.next_id,
                fixtures: self.fixtures.values().cloned().collect(),
            },
            groups: self
                .groups
                .iter()
                .map(|(&id, items)| ShowGroup {
                    id,
                    items: items.clone(),
                    grid: self.group_grids.get(&id).copied(),
                    legend: self.group_legends.get(&id).cloned().unwrap_or_default(),
                })
                .collect(),
            palettes: ShowPalettes {
                next_id: self.palette_next_id,
                palettes: self
                    .palettes
                    .values()
                    .cloned()
                    .map(ShowPalette::Palette)
                    .collect(),
            },
            pages: (1..=PAGES)
                .zip(&self.page_legends)
                .filter(|(_, legend)| !legend.is_empty())
                .map(|(number, legend)| ShowPage {
                    number,
                    legend: legend.clone(),
                })
                .collect(),
            playbacks: ShowPlaybacks {
                next_id: self.playback_next_id,
                playbacks: self
                    .playbacks
                    .values()
                    .map(|pb| {
                        let mut pb = pb.clone();
                        for cue in &mut pb.cues {
                            cue.values
                                .retain(|item, _| self.fixtures.contains_key(&item.fixture));
                            cue.dormant
                                .retain(|item, _| self.fixtures.contains_key(&item.fixture));
                        }
                        pb
                    })
                    .collect(),
            },
            output: ShowOutput {
                next_node_id: self.node_next_id,
                nodes: self.nodes.values().cloned().map(ShowNode::Node).collect(),
            },
        }
    }

    /// SHW-07: build an engine from a show. Never fails: a profile that does not validate, a
    /// fixture the patch refuses, a group member or playback value without a fixture is left
    /// out with one warning. A fixture left out for another reason takes its references with it
    /// silently; a reference to an id the file never patched gets its own warning.
    pub fn from_show(show: &Show) -> (Engine, Vec<String>) {
        let s = &show.settings;
        let mut engine = Engine::new(Settings {
            ltp_trigger_level: s.ltp_trigger_level,
            faders_per_page: s.faders_per_page,
            handles_per_page: s.handles_per_page,
            wheel_step_normal: s.wheel_step_normal,
            wheel_step_fine: s.wheel_step_fine,
            wheel_step_coarse: s.wheel_step_coarse,
            palette_time: s.palette_time,
            palette_overlap: s.palette_overlap,
            record_by: s.record_by,
            cue_record_by: s.cue_record_by,
            cue_delay: s.cue_delay,
            cue_fade_in: s.cue_fade_in,
            cue_fade_out: s.cue_fade_out,
            cue_delay_out: s.cue_delay_out,
            cue_overlap: s.cue_overlap,
            release_mask: s.release_mask.clone(),
            release_time: s.release_time,
            auto_connect: s.auto_connect,
            cue_list_fader: s.cue_list_fader,
            update_mode: s.update_mode,
            handle_buttons: s.handle_buttons.clone(),
            clear_precedence: s.clear_precedence,
            clear_ltp: s.clear_ltp,
            lowlight_level: s.lowlight_level,
        });
        engine.colour_model = s.colour_model;
        let mut warnings = Vec::new();
        // PRG-42: a lowlight level out of range is left at its default.
        if !(0.0..=1.0).contains(&s.lowlight_level) {
            engine.settings.lowlight_level = default_lowlight();
            warnings.push(format!(
                "lowlight_level {} is out of range, set to {}",
                s.lowlight_level,
                default_lowlight()
            ));
        }
        // CUE-39: a button a kind cannot have becomes the kind's BestPractice button.
        if let HandleButtons::Custom(pairs) = &mut engine.settings.handle_buttons {
            for (kind, pair) in pairs {
                let mut fixed = false;
                for (button, best) in pair.iter_mut().zip(kind.best_practice()) {
                    if !kind.allows(*button) {
                        (*button, fixed) = (best, true);
                    }
                }
                if fixed {
                    warnings.push(format!(
                        "handle_buttons: a button {kind:?} cannot have is set to its best practice"
                    ));
                }
            }
        }
        let mut invalid = BTreeSet::new();
        for (key, profile) in &show.profiles {
            if let Err(e) = engine.add_profile(key.clone(), profile.clone()) {
                warnings.push(format!("profile '{key}' left out with its fixtures: {e}"));
                invalid.insert(key.as_str());
            }
        }
        let mut fixtures: Vec<&Fixture> = show.patch.fixtures.iter().collect();
        fixtures.sort_by_key(|f| f.id);
        let in_file: BTreeSet<FixtureId> = fixtures.iter().map(|f| f.id).collect();
        for f in fixtures {
            if invalid.contains(f.profile.as_str()) {
                continue;
            }
            if let Err(e) = engine.patch(f.clone()) {
                warnings.push(format!("fixture {} left out: {e}", f.id));
            }
        }
        engine.next_id = engine.next_id.max(show.patch.next_id);
        for group in &show.groups {
            let mut members = Vec::new();
            for &item in &group.items {
                let id = item.fixture;
                if engine.fixtures.contains_key(&id) {
                    members.push(item);
                } else if !in_file.contains(&id) {
                    warnings.push(format!(
                        "group {}: fixture {id} is not in the patch, left out",
                        group.id
                    ));
                }
            }
            engine.groups.insert(group.id, members);
            match group.grid.map(|g| g.validate().map(|()| g)) {
                Some(Ok(grid)) => {
                    engine.group_grids.insert(group.id, grid);
                }
                Some(Err(e)) => warnings.push(format!("group {}: grid left out, {e}", group.id)),
                None => {}
            }
            if !group.legend.is_empty() {
                match check_legend(&group.legend) {
                    Ok(()) => {
                        engine.group_legends.insert(group.id, group.legend.clone());
                    }
                    Err(e) => warnings.push(format!("group {}: legend left out, {e}", group.id)),
                }
            }
        }
        for entry in &show.palettes.palettes {
            let mut palette = match entry {
                ShowPalette::Palette(p) => p.clone(),
                ShowPalette::Unreadable(v) => {
                    warnings.push(format!("palette entry left out, not readable: {v}"));
                    continue;
                }
            };
            palette.fixtures.retain(|item, _| {
                let id = item.fixture;
                let keep = engine.fixtures.contains_key(&id);
                if !keep && !in_file.contains(&id) {
                    warnings.push(format!(
                        "palette {}: fixture {id} is not in the patch, its values are left out",
                        palette.number
                    ));
                }
                keep
            });
            // PAT-26: like cue effects, members that are not patched are left out.
            effect::Entry::retire(&mut palette.effects, |m| {
                !engine.fixtures.contains_key(&m.fixture)
            });
            let number = palette.number;
            if let Err(e) = engine.add_palette(palette) {
                warnings.push(format!("palette {number} left out: {e}"));
            }
        }
        engine.palette_next_id = engine.palette_next_id.max(show.palettes.next_id);
        engine.nodes.clear();
        for entry in &show.output.nodes {
            let node = match entry {
                ShowNode::Node(node) => node,
                ShowNode::Unreadable(v) => {
                    warnings.push(format!("output node left out, not readable: {v}"));
                    continue;
                }
            };
            if engine.nodes.contains_key(&node.id) {
                warnings.push(format!(
                    "output node {} left out: the id is already used",
                    node.id
                ));
            } else if let Err(e) = output::check(node, engine.nodes.values()) {
                warnings.push(format!("output node {} left out: {e}", node.id));
            } else {
                engine.nodes.insert(node.id, node.clone());
            }
        }
        let highest = engine
            .nodes
            .keys()
            .next_back()
            .map_or(1, |id| id.saturating_add(1));
        engine.node_next_id = show.output.next_node_id.max(highest);
        for page in &show.pages {
            if let Err(e) = engine.set_page_legend(page.number, &page.legend) {
                warnings.push(format!("page {} legend left out: {e}", page.number));
            }
        }
        for playback in &show.playbacks.playbacks {
            if let Some(playback) = engine.lenient_playback(playback, &in_file, &mut warnings) {
                engine.add_playback(playback);
            }
        }
        engine.playback_next_id = engine.playback_next_id.max(show.playbacks.next_id);
        engine.parking_changed();
        engine.notice_holding_masters();
        (engine, warnings)
    }

    /// PBK-44: a playback of a show file as the engine takes it: left out (`None`) for an id or
    /// home handle an earlier one holds or a handle outside the pages; an option or time out of
    /// range at its default, a second `Locked` on a slot `Unlocked`, cue values for a fixture
    /// that is not patched or a palette that does not exist as in SHW-07 and PAL-24; one
    /// warning for each.
    fn lenient_playback(
        &self,
        playback: &Playback,
        in_file: &BTreeSet<FixtureId>,
        warnings: &mut Vec<String>,
    ) -> Option<Playback> {
        let Playback { id, page, slot, .. } = *playback;
        if self.playbacks.contains_key(&id) {
            warnings.push(format!("playback {id} left out: the id is already used"));
            return None;
        }
        if !(1..=PAGES).contains(&page) || slot == 0 {
            warnings.push(format!(
                "playback {id} left out: handle {page}.{slot} does not exist"
            ));
            return None;
        }
        if let Some(other) = self
            .playbacks
            .values()
            .find(|p| (p.page, p.slot) == (page, slot))
        {
            warnings.push(format!(
                "playback {id} left out: handle {page}.{slot} is used by playback {}",
                other.id
            ));
            return None;
        }
        let mut playback = playback.clone();
        for cue in &mut playback.cues {
            // Like cue values (SHW-07): members that are not patched are left out.
            cue.effects.retain_mut(|e| {
                e.drop_members(|m| !self.fixtures.contains_key(&m.fixture));
                !e.members.is_empty()
            });
        }
        if playback
            .release_time
            .is_some_and(|t| check_time(t).is_err())
        {
            warnings.push(format!(
                "playback {id}: release time is out of range, set to the global one"
            ));
            playback.release_time = None;
        }
        if playback.group.is_some_and(|g| !(1..=99).contains(&g)) {
            warnings.push(format!("playback {id}: group is out of range, set to none"));
            playback.group = None;
        }
        if playback.paging == Paging::Locked
            && self
                .playbacks
                .values()
                .any(|p| p.slot == slot && p.paging == Paging::Locked)
        {
            warnings.push(format!(
                "playback {id}: slot {slot} already holds a Locked playback, loaded Unlocked"
            ));
            playback.paging = Paging::Unlocked;
        }
        let master_kind = match playback.kind {
            PlaybackKind::BpmMaster => Some(MasterKind::Bpm),
            PlaybackKind::RateMaster => Some(MasterKind::Rate),
            _ => None,
        };
        if playback.kind == PlaybackKind::GroupMaster {
            // GRM-14: a missing group is kept (it masters nothing), anything out of range gets
            // its default, cues are dropped; one warning each.
            let Some(Master::Group(mut m)) = playback.master.take() else {
                warnings.push(format!(
                    "playback {id} left out: it is a master without its options"
                ));
                return None;
            };
            if !self.groups.contains_key(&m.group) {
                warnings.push(format!(
                    "playback {id}: group {} is not in the show, the master masters nothing",
                    m.group
                ));
            }
            if m.mode == GroupMode::Unknown {
                warnings.push(format!(
                    "playback {id}: unknown group master mode, set to Scale"
                ));
                m.mode = GroupMode::Scale;
            }
            if !(0.0..=1.0).contains(&m.level) {
                m.level = GroupMaster::neutral_level(m.mode);
                warnings.push(format!(
                    "playback {id}: group master level is out of range, set to {}",
                    m.level
                ));
            }
            if !GROUP_SCALES.contains(&m.scale) {
                m.scale = 100;
                warnings.push(format!(
                    "playback {id}: group master scale is out of range, set to 100"
                ));
            }
            playback.master = Some(Master::Group(m));
            if !playback.cues.is_empty() {
                warnings.push(format!("playback {id}: a master has no cues, left out"));
                playback.cues.clear();
            }
        } else {
            match (master_kind, playback.master.take()) {
                (None, None) => {}
                (None, Some(_)) => {
                    warnings.push(format!(
                        "playback {id}: only a master has master options, left out"
                    ));
                }
                (Some(kind), Some(mut master)) if master.kind() == Some(kind) => {
                    let number = master.number();
                    if let Err(e) = kind.check_number(number) {
                        warnings.push(format!("playback {id} left out: {e}"));
                        return None;
                    }
                    if self.master_in_use(kind, number) {
                        warnings.push(format!(
                            "playback {id} left out: {master} is already used by an earlier master"
                        ));
                        return None;
                    }
                    if master.check().is_err() {
                        warnings.push(format!(
                            "playback {id}: master options are out of range, set to their defaults"
                        ));
                        master = Master::new(kind, number);
                    }
                    playback.master = Some(master);
                    if !playback.cues.is_empty() {
                        warnings.push(format!("playback {id}: a master has no cues, left out"));
                        playback.cues.clear();
                    }
                }
                (Some(_), _) => {
                    warnings.push(format!(
                        "playback {id} left out: it is a master without its options"
                    ));
                    return None;
                }
            }
        }
        if playback.chase.check().is_err() {
            warnings.push(format!(
                "playback {id}: chase options are out of range, set to their defaults"
            ));
            playback.chase = ChaseOptions::default();
        }
        let mut position = 0;
        let mut seen = BTreeSet::new();
        playback.cues.retain(|cue| {
            position += 1;
            let keep = cue.number.is_valid() && seen.insert(cue.number);
            if !keep {
                warnings.push(format!(
                    "playback {id}: cue {position} left out, its number is out of range or used before"
                ));
            }
            keep
        });
        playback.cues.sort_by_key(|cue| cue.number);
        for cue in &mut playback.cues {
            if check_link(&cue.link).is_err() {
                warnings.push(format!(
                    "playback {id}: Q{} link is out of range, set to Wait",
                    cue.number
                ));
                cue.link = Link::Wait;
            }
            if check_group_times(&cue.group_times).is_err() {
                warnings.push(format!(
                    "playback {id}: Q{} group times are out of range, left out",
                    cue.number
                ));
                cue.group_times.clear();
            }
            for (name, value, max, default) in [
                ("delay", &mut cue.delay, 600.0, 0.0),
                ("fade in", &mut cue.fade_in, 600.0, 0.0),
                ("overlap", &mut cue.overlap, 100.0, 100.0),
            ] {
                if !(0.0..=max).contains(value) {
                    warnings.push(format!(
                        "playback {id}: {name} {value} is out of range, set to {default}"
                    ));
                    *value = default;
                }
            }
            // CUE-56, 58: an out-of-range out time becomes As In.
            for (name, value) in [
                ("fade out", &mut cue.fade_out),
                ("delay out", &mut cue.delay_out),
            ] {
                if let Some(v) = value.filter(|v| !(0.0..=600.0).contains(v)) {
                    warnings.push(format!(
                        "playback {id}: {name} {v} is out of range, set to As In"
                    ));
                    *value = None;
                }
            }
            for values in [&mut cue.values, &mut cue.dormant] {
                // CUE-61: what is off is left out and checked alike.
                values.retain(|item, _| {
                    let fixture = item.fixture;
                    let keep = self.fixtures.contains_key(&fixture);
                    if !keep && !in_file.contains(&fixture) {
                        warnings.push(format!(
                            "playback {id}: fixture {fixture} is not in the patch, its values are left out"
                        ));
                    }
                    keep
                });
            }
            for stored in (cue.values.values_mut().chain(cue.dormant.values_mut()))
                .flat_map(|a| a.values_mut())
            {
                if let Stored::Reference(r) = *stored {
                    if !self.palettes.contains_key(&r.palette) {
                        warnings.push(format!(
                            "playback {id}: palette {} does not exist, its value is kept",
                            r.palette
                        ));
                        *stored = Stored::Value(r.value);
                    }
                }
            }
        }
        Some(playback)
    }

    // ---- actions ---------------------------------------------------------

    /// Move a playback fader. The playback fires when the effective level rises above 0 and its
    /// LTP values trigger at the trigger point (PBK, Definitions); it is killed by the next tick
    /// once its kill point is reached (PBK-19). Every time counts from that tick (PBK, Timing),
    /// so `_now` is not needed. A master never fires (CUE-37): its level is read when asked.
    pub fn set_fader(
        &mut self,
        id: PlaybackId,
        level: f32,
        _now: Timestamp,
    ) -> Result<(), CoreError> {
        if !(0.0..=1.0).contains(&level) || level.is_nan() {
            return Err(CoreError::InvalidLevel(level));
        }
        let master = self.playbacks.get(&id).is_some_and(|p| p.kind.is_master());
        let state = self.live_state(id)?;
        state.level = level;
        if master {
            state.fader_moved = true; // CUE-35: the fader's tempo counts again
        } else {
            self.settle(id);
        }
        // GRM-05, 11: an enabled group master saves the fader; a disabled one is locked.
        if let Some(Master::Group(m)) =
            (self.playbacks.get_mut(&id)).and_then(|p| p.master.as_mut())
        {
            if m.enabled {
                m.level = level;
            }
        }
        Ok(())
    }

    /// PBK-26: Flash held (`on`) or released. Pressing it fires the playback at full.
    pub fn set_flash(
        &mut self,
        id: PlaybackId,
        on: bool,
        _now: Timestamp,
    ) -> Result<(), CoreError> {
        self.not_a_master(id, on)?;
        let state = self.live_state(id)?;
        state.flash = on;
        state.blocked &= !on; // a press is a deliberate fire
        self.settle(id);
        Ok(())
    }

    /// PBK-27: Swop held or released; the later of two held Swops wins.
    pub fn set_swop(&mut self, id: PlaybackId, on: bool, _now: Timestamp) -> Result<(), CoreError> {
        self.not_a_master(id, on)?;
        self.activation_seq = self.activation_seq.saturating_add(1);
        let seq = self.activation_seq;
        let state = self.live_state(id)?;
        state.swop = on;
        state.blocked &= !on;
        if on {
            state.swop_seq = seq;
        }
        self.settle(id);
        Ok(())
    }

    /// PBK-28: latch Flash on, or off again (the engine toggles; arming Latch is the UI's).
    pub fn latch_flash(&mut self, id: PlaybackId, _now: Timestamp) -> Result<(), CoreError> {
        let latches = !self.states.get(&id).is_some_and(|s| s.latched);
        self.not_a_master(id, latches)?;
        let state = self.live_state(id)?;
        state.latched = !state.latched;
        state.blocked &= !state.latched;
        self.settle(id);
        Ok(())
    }

    /// PBK-22, 23, 47: kill the playback at once, ignoring `fade_out` and the kill point, and
    /// release every LTP attribute it holds over its release time; each HTP attribute it
    /// outputs falls to 0 over the same time. A fader still up has to go to 0 before it fires
    /// again. An inactive playback is left alone.
    pub fn release_playback(&mut self, id: PlaybackId, now: Timestamp) -> Result<(), CoreError> {
        self.live_state(id)?;
        self.start_htp_fall(id, now);
        self.kill_playback(id, true, None);
        Ok(())
    }

    /// PBK-47: before a Release kills the playback, note each HTP level it outputs now (master
    /// included); `merge_attribute` lets them fall to 0 beside the LTP release.
    fn start_htp_fall(&mut self, id: PlaybackId, now: Timestamp) {
        let (Some(state), Some(playback)) = (self.states.get(&id), self.playbacks.get(&id)) else {
            return;
        };
        if !state.is_active() {
            return;
        }
        let stored: Vec<(Item, String, Stored)> = if is_list(playback) {
            let at = |item: Item, name: &str| {
                let fixture = self.fixtures.get(&item.fixture)?;
                let mode = resolve_mode(&self.profiles, fixture)?;
                let resolve = |s| resolve_stored(&self.palettes, item, fixture, mode, name, s);
                let (v, _) = state.run.fade_value(item, name, now, resolve)?;
                Some(Stored::Value(v))
            };
            (state.run.attributes())
                .filter_map(|(item, name)| Some((item, name.to_string(), at(item, name)?)))
                .collect()
        } else {
            (playback.values().into_iter())
                .flat_map(|v| v.iter())
                .flat_map(|(item, a)| a.iter().map(|(n, s)| (*item, n.clone(), *s)))
                .collect()
        };
        let mut entries: BTreeMap<Item, BTreeMap<String, (Value, Value)>> = BTreeMap::new();
        for (item, name, stored) in stored {
            let Some(fixture) = self.fixtures.get(&item.fixture) else {
                continue;
            };
            let Some(mode) = resolve_mode(&self.profiles, fixture) else {
                continue;
            };
            let Some(l) = mode
                .item_logical(item.cell, &name)
                .filter(|l| l.kind == AttributeKind::Htp)
            else {
                continue;
            };
            let value = resolve_stored(&self.palettes, item, fixture, mode, l.name, stored);
            if !value.same_kind(&l.default) {
                continue;
            }
            let value = value.scaled(state.master);
            if value.level() > 0.0 {
                entries
                    .entry(item)
                    .or_default()
                    .insert(name, (value, Value::Level(0.0)));
            }
        }
        if entries.is_empty() {
            return;
        }
        let duration = (playback.release_time).unwrap_or(self.settings.release_time);
        if let Some(state) = self.states.get_mut(&id) {
            state.falls.push(Release {
                start: None,
                duration,
                entries,
            });
        }
    }

    /// CUE-10..12, 18, 19, 27, 32: Go, Back, Stop, Go to cue, Set next or Release on a cue list or
    /// a chase; `None` is the connected list. Go and Go to cue fire a list that is off at once
    /// (even at fader 0);
    /// the others only record what to do and the next tick performs it. A refused action changes
    /// nothing (CUE-49).
    pub fn cuelist(
        &mut self,
        id: Option<PlaybackId>,
        action: ListAction,
        now: Timestamp,
    ) -> Result<(), CoreError> {
        let id = match id {
            Some(id) => id,
            None => self.connected().ok_or(CoreError::NoCueListConnected)?,
        };
        let playback = (self.playbacks.get(&id)).ok_or(CoreError::UnknownPlayback(id))?;
        if !is_list(playback) {
            return Err(CoreError::NotACueList(id));
        }
        let idle = ListRun::default();
        let run = self.states.get(&id).map_or(&idle, |s| &s.run);
        let cue_name = |n: CueNumber| match playback.cues.iter().find(|c| c.number == n) {
            Some(c) if !c.legend.is_empty() => format!("Q{n} {}", c.legend),
            _ => format!("Q{n}"),
        };
        let list_name = || match playback.legend.as_str() {
            "" => format!("{}.{}", playback.page, playback.slot),
            legend => legend.to_string(),
        };
        let name = |current: Option<CueNumber>| current.map_or_else(list_name, cue_name);
        match action {
            ListAction::Release => return self.release_playback(id, now),
            ListAction::Go if !run.paused() && run.next_cue(playback).is_none() => {
                let last = format!("{} is the last cue", name(run.current));
                return Err(CoreError::ListRefused(last));
            }
            ListAction::Back if run.current.is_none() => {
                let off = format!("{} is not running", name(None));
                return Err(CoreError::ListRefused(off));
            }
            ListAction::Back if run.back_cue(playback).is_none() => {
                let first = format!("{} is the first cue", name(run.current));
                return Err(CoreError::ListRefused(first));
            }
            ListAction::Goto(n) | ListAction::Next(n)
                if !playback.cues.iter().any(|c| c.number == n) =>
            {
                return Err(CoreError::UnknownCue(n));
            }
            _ => {}
        }
        let connects = matches!(
            action,
            ListAction::Go | ListAction::Goto(_) | ListAction::Next(_)
        );
        if connects && self.settings.auto_connect {
            self.connected = Some(id);
        }
        if matches!(action, ListAction::Go | ListAction::Goto(_)) {
            self.fire(id);
        }
        let state = self.live_state(id)?;
        match action {
            ListAction::Next(n) => state.run.next = Some(n),
            // Back and Stop need a list that runs.
            ListAction::Back | ListAction::Stop if !state.is_active() => {}
            _ => state.run.pending.push(action),
        }
        Ok(())
    }

    /// CUE-26: the Go block follows this list (a cue list or a chase). Nothing on stage changes.
    pub fn connect_list(&mut self, id: PlaybackId) -> Result<(), CoreError> {
        let playback = (self.playbacks.get(&id)).ok_or(CoreError::UnknownPlayback(id))?;
        if !is_list(playback) {
            return Err(CoreError::NotACueList(id));
        }
        self.connected = Some(id);
        Ok(())
    }

    /// CUE-26: the connected list, else the first list in handle order (page, then slot).
    pub fn connected(&self) -> Option<PlaybackId> {
        let stored = self
            .connected
            .filter(|id| self.playbacks.get(id).is_some_and(is_list));
        stored.or_else(|| {
            (self.playbacks.values().filter(|p| is_list(p)))
                .min_by_key(|p| (p.page, p.slot))
                .map(|p| p.id)
        })
    }

    /// CUE-40: the live state of a cue list or a chase; `None` for anything else.
    pub fn list_live(&self, id: PlaybackId) -> Option<ListLive> {
        let playback = self.playbacks.get(&id).filter(|p| is_list(p))?;
        let run = self.states.get(&id).map(|s| &s.run);
        Some(run.unwrap_or(&ListRun::default()).live(playback))
    }

    /// CUE-25: the two functions of a playback's handle buttons now; `None` for an unknown id.
    pub fn handle_buttons(&self, id: PlaybackId) -> Option<[Button; 2]> {
        self.playbacks.get(&id).map(|p| self.button_pair(p))
    }

    fn button_pair(&self, playback: &Playback) -> [Button; 2] {
        let kind = button_kind(playback);
        let best = kind.best_practice();
        match &self.settings.handle_buttons {
            HandleButtons::BestPractice => best,
            HandleButtons::Titan => match kind {
                ButtonKind::BpmMaster | ButtonKind::RateMaster | ButtonKind::GroupMaster => best,
                _ => [Button::Swop, Button::Flash],
            },
            HandleButtons::Custom(pairs) => pairs.get(&kind).copied().unwrap_or(best),
        }
    }

    /// CUE-55: press or release a handle button; it does what `handle_buttons` assigns to the
    /// kind of playback on the handle (CUE-25). Flash and Swop follow `on`, the others act on the
    /// press, on this playback's own list.
    pub fn press_button(
        &mut self,
        id: PlaybackId,
        button: HandleButton,
        on: bool,
        now: Timestamp,
    ) -> Result<(), CoreError> {
        let playback = (self.playbacks.get(&id)).ok_or(CoreError::UnknownPlayback(id))?;
        let pair = self.button_pair(playback);
        let function = match button {
            HandleButton::Upper => pair[0],
            HandleButton::Lower => pair[1],
        };
        match function {
            Button::Flash => self.set_flash(id, on, now),
            Button::Swop => self.set_swop(id, on, now),
            _ if !on => Ok(()),
            Button::Go => self.cuelist(Some(id), ListAction::Go, now),
            Button::Back => self.cuelist(Some(id), ListAction::Back, now),
            Button::Stop => self.cuelist(Some(id), ListAction::Stop, now),
            Button::Release => self.release_playback(id, now),
            Button::Tap => self.tap(id, now),
            Button::None => Ok(()),
        }
    }

    /// IPC-53: whether the handle button is down now (Flash or Swop held).
    pub fn button_held(&self, id: PlaybackId, button: HandleButton) -> bool {
        let Some(playback) = self.playbacks.get(&id) else {
            return false;
        };
        let pair = self.button_pair(playback);
        let function = match button {
            HandleButton::Upper => pair[0],
            HandleButton::Lower => pair[1],
        };
        let Some(state) = self.playback_state(id) else {
            return false;
        };
        match function {
            Button::Flash => state.flash,
            Button::Swop => state.swop,
            _ => false,
        }
    }

    /// CUE-33: a tap of a chase or a BPM master, timed `at` the moment the daemon received it.
    /// The first tap, or one more than 2 s after the last, starts a series and changes nothing;
    /// each further tap sets the BPM from the mean of the series' last three intervals (1–600,
    /// rounded to 0.1) and beat-syncs. Refused on a memory, a cue list and a Rate master.
    pub fn tap(&mut self, id: PlaybackId, at: Timestamp) -> Result<(), CoreError> {
        if !at.is_finite() {
            return Err(CoreError::InvalidCommand(format!("tap time {at}")));
        }
        let playback = (self.playbacks.get(&id)).ok_or(CoreError::UnknownPlayback(id))?;
        if !has_tempo(playback) {
            return Err(CoreError::InvalidCommand(format!(
                "playback {id} has no tempo to tap"
            )));
        }
        let state = self.live_state(id)?;
        let Some(bpm) = tap_bpm(&mut state.taps, at) else {
            return Ok(());
        };
        self.set_tempo(id, bpm, Some(at))
    }

    /// CUE-34: type a tempo (1–600) for a chase or a BPM master. It changes no running step.
    pub fn set_bpm(&mut self, id: PlaybackId, bpm: f32) -> Result<(), CoreError> {
        check_bpm(bpm)?;
        self.set_tempo(id, bpm, None)
    }

    /// CUE-33, 34: a tempo for `id` goes to the BPM master a chase follows, else to the chase's
    /// own `bpm`, or to the BPM master itself; it clears the master's "fader moved last" (CUE-35).
    /// With `sync` the running chases on it start their next step at that tap time.
    fn set_tempo(
        &mut self,
        id: PlaybackId,
        bpm: f32,
        sync: Option<Timestamp>,
    ) -> Result<(), CoreError> {
        let playback = (self.playbacks.get(&id)).ok_or(CoreError::UnknownPlayback(id))?;
        let master = match (&playback.master, playback.kind) {
            (Some(Master::Bpm(m)), _) => Some((id, m.number)),
            (_, PlaybackKind::Chase) => match playback.chase.speed_source {
                SpeedSource::Bpm(n) => self.find_master(MasterKind::Bpm, n).map(|(m, _)| (m, n)),
                _ => None,
            },
            _ => {
                return Err(CoreError::InvalidCommand(format!(
                    "playback {id} has no tempo to set"
                )))
            }
        };
        if let Some((master, number)) = master {
            let stored = (self.playbacks.get_mut(&master)).and_then(|p| p.master.as_mut());
            if let Some(Master::Bpm(m)) = stored {
                m.bpm = bpm;
            }
            self.live_state(master)?.fader_moved = false;
            if let Some(at) = sync {
                for (other, state) in &mut self.states {
                    let follows = self.playbacks.get(other).is_some_and(|p| {
                        p.kind == PlaybackKind::Chase
                            && p.chase.speed_source == SpeedSource::Bpm(number)
                    });
                    if follows && state.is_active() && !state.run.paused() {
                        state.run.sync = Some(at);
                    }
                }
                // FX-10: so do the programmer entries that follow this master.
                for e in &self.programmer.effects {
                    if e.params.speed_source == effect::SpeedSource::Bpm(number) {
                        if let Some(live) = self.effect_live.get_mut(&e.id) {
                            live.clock.beat_sync(e.params.beats);
                        }
                    }
                }
            }
        } else {
            self.playback_mut(id)?.chase.bpm = bpm;
            let state = self.live_state(id)?;
            if sync.is_some() && state.is_active() && !state.run.paused() {
                state.run.sync = sync;
            }
        }
        Ok(())
    }

    /// CUE-29, 35: a chase's tempo after its speed source, a BPM master's tempo; `None` for
    /// anything else.
    pub fn effective_bpm(&self, id: PlaybackId) -> Option<f32> {
        let playback = self.playbacks.get(&id)?;
        match &playback.master {
            Some(Master::Bpm(m)) => Some(self.master_tempo(id, m)),
            Some(Master::Rate(_) | Master::Group(_)) => None,
            None => (playback.kind == PlaybackKind::Chase).then(|| self.chase_bpm(&playback.chase)),
        }
    }

    /// CUE-44: the multiplier of a BPM master; `None` for anything else.
    pub fn effective_multiplier(&self, id: PlaybackId) -> Option<f32> {
        match &self.playbacks.get(&id)?.master {
            Some(Master::Bpm(m)) => Some(self.master_multiplier(id, m)),
            _ => None,
        }
    }

    /// CUE-29: a source whose master does not exist runs as `Free`.
    fn chase_bpm(&self, chase: &ChaseOptions) -> f32 {
        self.speed(
            chase.bpm,
            match chase.speed_source {
                SpeedSource::Free => None,
                SpeedSource::Bpm(n) => Some((MasterKind::Bpm, n)),
                SpeedSource::Rate(n) => Some((MasterKind::Rate, n)),
            },
        )
    }

    /// CUE-29, FX-09: `bpm` after its speed master; a missing master runs as `Free`.
    fn speed(&self, bpm: f32, source: Option<(MasterKind, u8)>) -> f32 {
        let Some((kind, number)) = source else {
            return bpm;
        };
        match self.find_master(kind, number) {
            Some((id, Master::Bpm(m))) => self.master_tempo(id, m),
            Some((id, Master::Rate(m))) => {
                let level = self.states.get(&id).map_or(0.0, |s| s.level);
                bpm * (level * f32::from(m.rate_scale) / 100.0) // CUE-36
            }
            None | Some((_, Master::Group(_))) => bpm,
        }
    }

    /// CUE-35: the BPM master's tempo: its fader's or its `bpm`, times its multiplier (or the
    /// fader's zone).
    fn master_tempo(&self, id: PlaybackId, m: &BpmMaster) -> f32 {
        let state = self.states.get(&id);
        let level = state.map_or(0.0, |s| s.level);
        let bpm = match m.fader {
            BpmFader::Bpm if state.is_some_and(|s| s.fader_moved) => level * m.fader_max,
            _ => m.bpm,
        };
        bpm * self.master_multiplier(id, m)
    }

    /// CUE-44: the multiplier a BPM master applies: its own, or the fader's zone.
    fn master_multiplier(&self, id: PlaybackId, m: &BpmMaster) -> f32 {
        match m.fader {
            BpmFader::Multiplier => {
                let level = self.states.get(&id).map_or(0.0, |s| s.level);
                let zone = ((level * 5.0) as usize).min(4); // /4 /2 ×1 ×2 ×4
                [0.25, 0.5, 1.0, 2.0, 4.0].get(zone).copied().unwrap_or(1.0)
            }
            _ => m.multiplier,
        }
    }

    /// The playback holding speed master `kind` `number`.
    fn find_master(&self, kind: MasterKind, number: u8) -> Option<(PlaybackId, &Master)> {
        self.playbacks.values().find_map(|p| {
            let master = p.master.as_ref();
            master
                .filter(|m| m.kind() == Some(kind) && m.number() == number)
                .map(|m| (p.id, m))
        })
    }

    /// PBK-30: Dead Black Out on or off. Live state: not in the show.
    pub fn set_dbo(&mut self, on: bool) {
        self.dbo = on;
    }

    pub fn dbo(&self) -> bool {
        self.dbo
    }

    /// The live state of a playback that exists.
    fn live_state(&mut self, id: PlaybackId) -> Result<&mut PlaybackState, CoreError> {
        if !self.playbacks.contains_key(&id) {
            return Err(CoreError::UnknownPlayback(id));
        }
        Ok(self.states.entry(id).or_default())
    }

    /// CUE-55, GRM-12: Flash, Swop and Latch are for playbacks that output something, not for
    /// speed masters; a group master has them unless it is in Take Over or disabled. `on` is
    /// false for a release, which is never refused (nothing stays stuck).
    fn not_a_master(&self, id: PlaybackId, on: bool) -> Result<(), CoreError> {
        match self.playbacks.get(&id) {
            Some(p) if p.kind.is_master() => match &p.master {
                Some(Master::Group(m)) if !on || (m.enabled && m.mode != GroupMode::TakeOver) => {
                    Ok(())
                }
                Some(Master::Group(_)) => Err(CoreError::InvalidCommand(format!(
                    "playback {id} is a group master in Take Over or disabled: \
                     Flash, Swop and Latch are refused"
                ))),
                _ => Err(CoreError::InvalidCommand(format!(
                    "playback {id} is a master: Flash, Swop and Latch are not for it"
                ))),
            },
            _ => Ok(()),
        }
    }

    /// After an input changed: fire an inactive playback whose effective level is above 0
    /// (killing the rest of its group, PBK-25), trigger its LTP values at the trigger point.
    /// An effective level of 0 lets a blocked playback fire again (PBK-22, 25) and kills a cue
    /// list with `KillAtZero` (CUE-15) or a chase (CUE-31).
    fn settle(&mut self, id: PlaybackId) {
        // CUE-37, GRM-03: a master never fires.
        if (self.playbacks.get(&id)).is_some_and(|p| p.kind.is_master()) {
            return;
        }
        let trigger = self.settings.ltp_trigger_level;
        let Some(state) = self.states.get_mut(&id) else {
            return;
        };
        let level = state.effective_level();
        let was = std::mem::replace(&mut state.settled, level);
        if level <= 0.0 {
            state.blocked = false;
            // CUE-15, 31: a chase or a `KillAtZero` list is killed when its level falls to 0, not
            // by a 0 sent again (a touched fader) after Go at fader 0.
            let list = self.playbacks.get(&id).filter(|p| is_list(p));
            let kills = |p: &Playback| {
                p.kind == PlaybackKind::Chase || p.list.fader == ListFader::KillAtZero
            };
            if was > 0.0 && list.is_some_and(kills) {
                self.kill_playback(id, false, None);
            }
            return;
        }
        if !state.is_active() && state.blocked {
            return;
        }
        let fired = self.fire(id);
        if let Some(state) = self.states.get_mut(&id) {
            if state.triggered.is_none() && level >= trigger {
                self.activation_seq = self.activation_seq.saturating_add(1);
                state.triggered = Some(self.activation_seq);
            }
        }
        if fired {
            self.list_fired(id);
        }
    }

    /// Put an inactive playback in the merge, killing the rest of its group (PBK-25). False when
    /// it was active already.
    fn fire(&mut self, id: PlaybackId) -> bool {
        let Some(state) = self.states.get_mut(&id).filter(|s| !s.is_active()) else {
            return false;
        };
        self.activation_seq = self.activation_seq.saturating_add(1);
        state.activated = Some(self.activation_seq);
        state.release = None; // PBK-21: firing again stops the release
        let group = self.playbacks.get(&id).and_then(|p| p.group);
        if let Some(group) = group {
            let others: Vec<PlaybackId> = self
                .playbacks
                .iter()
                .filter(|(other, p)| **other != id && p.group == Some(group))
                .map(|(other, _)| *other)
                .collect();
            for other in others {
                self.kill_playback(other, false, None);
            }
        }
        true
    }

    /// CUE-16, 26, 28: a cue list or chase fired by its fader or Flash connects and starts its
    /// next cue (a chase always starts its first step).
    fn list_fired(&mut self, id: PlaybackId) {
        let Some(list) = self.playbacks.get(&id).filter(|p| is_list(p)) else {
            return;
        };
        let fire_first = list.kind == PlaybackKind::Chase || list.list.fire_first_cue;
        if self.settings.auto_connect {
            self.connected = Some(id);
        }
        if let Some(state) = self.states.get_mut(&id) {
            state.run.fire_first = fire_first;
        }
    }

    /// PBK-19, 20, 22: the playback leaves the merge: its HTP values are gone, its LTP values
    /// are held except those that release (the release mask's groups, or all when `all`), which
    /// fade from their current output to what is underneath. `start` is the tick the release
    /// clock starts at; `None` = the next tick.
    fn kill_playback(&mut self, id: PlaybackId, all: bool, start: Option<Timestamp>) {
        let (Some(state), Some(playback)) = (self.states.get(&id), self.playbacks.get(&id)) else {
            return;
        };
        if !state.is_active() {
            return;
        }
        let mask =
            (playback.release_mask.as_deref()).unwrap_or(self.settings.release_mask.as_slice());
        let mut entries: BTreeMap<Item, BTreeMap<String, (Value, Value)>> = BTreeMap::new();
        for (item, attributes) in &state.from {
            for (name, origin) in attributes {
                if !all && !mask.contains(&self.attribute_group(name)) {
                    continue;
                }
                let current = held_value(&self.held, *item, name).unwrap_or(origin.value);
                entries
                    .entry(*item)
                    .or_default()
                    .insert(name.clone(), (current, origin.value));
            }
        }
        let release = (!entries.is_empty()).then(|| Release {
            start,
            duration: (playback.release_time).unwrap_or(self.settings.release_time),
            entries,
        });
        if let Some(state) = self.states.get_mut(&id) {
            state.blocked = state.effective_level() > 0.0;
            state.activated = None;
            state.triggered = None;
            state.fired_at = None;
            state.trigger_at = None;
            state.master = 0.0;
            state.fall = None;
            state.faded_in = false;
            state.was_holding = false;
            state.from.clear();
            state.release = release;
            state.run = ListRun::default(); // CUE-17
        }
    }

    /// Before the merge of a tick: start the timers of what the actions since the last tick did,
    /// move every active playback's intensity master (PBK-15..17, 26) and kill what has reached
    /// its kill point (PBK-19).
    fn update_playbacks(&mut self, now: Timestamp) {
        let mut kills: Vec<PlaybackId> = Vec::new();
        let mut swop: Option<(u64, PlaybackId)> = None;
        // The tempo of every chase, before the states are borrowed (CUE-29).
        let tempos: BTreeMap<PlaybackId, f32> = (self.playbacks.values())
            .filter(|p| p.kind == PlaybackKind::Chase)
            .map(|p| (p.id, self.chase_bpm(&p.chase)))
            .collect();
        for (id, state) in &mut self.states {
            if let Some(release) = &mut state.release {
                release.start.get_or_insert(now);
            }
            for fall in &mut state.falls {
                fall.start.get_or_insert(now);
            }
            let playback = self.playbacks.get(id);
            // PBK-27, GRM-12: a held Swop counts for an active playback and a group master.
            let swops =
                state.is_active() || playback.is_some_and(|p| p.kind == PlaybackKind::GroupMaster);
            if state.swop && swops && swop.is_none_or(|(seq, _)| state.swop_seq > seq) {
                swop = Some((state.swop_seq, *id));
            }
            let (true, Some(playback)) = (state.is_active(), playback) else {
                continue;
            };
            if is_list(playback) {
                // CUE-15, 31: the fader is the intensity master, at once; the cues do the fading.
                let mut cx = Ctx {
                    playback,
                    from: &mut state.from,
                    env: Env {
                        fixtures: &self.fixtures,
                        profiles: &self.profiles,
                        palettes: &self.palettes,
                        held: &self.held,
                    },
                    seq: &mut self.activation_seq,
                    now,
                    bpm: tempos.get(id).copied().unwrap_or(0.0),
                };
                state.run.tick(&mut cx);
                state.master = state.effective_level();
                continue;
            }
            let times = playback.times();
            let mode2 = playback.fader_mode == FaderMode::Mode2;
            if state.fired_at.is_none() {
                state.fired_at = Some(now);
                if mode2 {
                    state.from =
                        origins(playback, &self.fixtures, &self.profiles, &self.held, false);
                }
            }
            if state.triggered.is_some() && state.trigger_at.is_none() {
                state.trigger_at = Some(now);
                if !mode2 {
                    state.from =
                        origins(playback, &self.fixtures, &self.profiles, &self.held, true);
                }
            }
            let level = state.effective_level();
            let holding = state.flash || state.swop || state.latched;
            state.faded_in |= holding;
            let fired_at = state.fired_at.unwrap_or(now);
            let target = if mode2 || state.faded_in {
                level
            } else {
                level * fade_progress((now - fired_at) as f32 - times.delay, times.fade_in)
            };
            // Mode 1: m falls no faster than full scale per fade-out, from its value in the tick
            // the fall began; rises follow the target at once, and so does a released Flash.
            let falling = match state.fall {
                Some((since, from)) if times.fade_out > 0.0 => {
                    from - (now - since) as f32 / times.fade_out
                }
                _ => state.master,
            };
            let slow = playback.fader_mode == FaderMode::Mode1
                && times.fade_out > 0.0
                && !(state.was_holding && !holding)
                && target < falling;
            if slow {
                state.fall.get_or_insert((now, state.master));
                state.master = target.max(falling);
            } else {
                state.fall = None;
                state.master = target;
            }
            state.was_holding = holding;
            if level <= 0.0
                && (playback.kill_point == KillPoint::FaderAtZero || state.master <= 0.0)
            {
                kills.push(*id);
            }
        }
        self.swop_owner = swop.map(|(_, id)| id);
        for id in kills {
            self.kill_playback(id, false, Some(now));
        }
    }

    /// Grand Master, 0.0..=1.0. Scales all HTP output after the merge (ENG-06).
    pub fn set_grand_master(&mut self, level: f32) -> Result<(), CoreError> {
        if !(0.0..=1.0).contains(&level) || level.is_nan() {
            return Err(CoreError::InvalidLevel(level));
        }
        self.grand_master = level;
        Ok(())
    }

    // ---- pages and playbacks (PBK-02, PBK-06..14, 24, 31..33) ---------------

    /// PBK-02: show another page. 0 or a page above `PAGES` is refused and nothing changes;
    /// playback levels and active state are never touched.
    pub fn set_page(&mut self, page: u16) -> Result<(), CoreError> {
        if page == 0 || page > PAGES {
            return Err(CoreError::InvalidPage(page));
        }
        self.page = page;
        Ok(())
    }

    /// PBK-02: the current page, 1..=`PAGES`.
    pub fn page(&self) -> u16 {
        self.page
    }

    /// PBK-06, 07: what slot *s* (index *s* − 1) of the current page shows, for the
    /// `faders_per_page` slots.
    pub fn page_slots(&self) -> Vec<Option<PlaybackId>> {
        (1..=self.settings.faders_per_page)
            .map(|slot| self.playback_at(self.page, slot, true))
            .collect()
    }

    /// PBK-07: a `Locked` playback on the slot, else the playback whose home is the handle, else
    /// (`transparent`) a `TransparentLock` one from another page.
    fn playback_at(&self, page: u16, slot: u16, transparent: bool) -> Option<PlaybackId> {
        let find = |keep: &dyn Fn(&Playback) -> bool| {
            self.playbacks.values().find(|p| keep(p)).map(|p| p.id)
        };
        find(&|p| p.slot == slot && p.paging == Paging::Locked)
            .or_else(|| find(&|p| (p.page, p.slot) == (page, slot)))
            .or_else(|| {
                find(&|p| transparent && p.slot == slot && p.paging == Paging::TransparentLock)
            })
    }

    /// PBK-07, 13, 14: the playback a record, copy or move onto this handle addresses, if any.
    /// A handle outside the pages and `faders_per_page` is refused.
    fn handle_target(&self, page: u16, slot: u16) -> Result<Option<PlaybackId>, CoreError> {
        if !(1..=PAGES).contains(&page) || !(1..=self.settings.faders_per_page).contains(&slot) {
            return Err(CoreError::InvalidHandle { page, slot });
        }
        Ok(self.playback_at(page, slot, false))
    }

    fn check_empty_handle(&self, page: u16, slot: u16) -> Result<(), CoreError> {
        match self.handle_target(page, slot)? {
            Some(_) => Err(CoreError::HandleUsed { page, slot }),
            None => Ok(()),
        }
    }

    /// PBK-08: the page's legend; `None` for a page that does not exist.
    pub fn page_legend(&self, page: u16) -> Option<&str> {
        let index = usize::from(page).checked_sub(1)?;
        self.page_legends.get(index).map(String::as_str)
    }

    /// PBK-08: 0–32 characters.
    pub fn set_page_legend(&mut self, page: u16, legend: &str) -> Result<(), CoreError> {
        check_legend(legend)?;
        let slot = usize::from(page)
            .checked_sub(1)
            .and_then(|i| self.page_legends.get_mut(i))
            .ok_or(CoreError::InvalidPage(page))?;
        *slot = legend.to_string();
        Ok(())
    }

    /// PBK-12, 20: the cue time and release settings, refused as a whole when one is out of range.
    pub fn set_playback_settings(
        &mut self,
        change: &PlaybackSettingsChange,
    ) -> Result<(), CoreError> {
        for time in [
            change.delay,
            change.fade_in,
            change.fade_out.flatten(),
            change.delay_out.flatten(),
            change.release_time,
        ]
        .into_iter()
        .flatten()
        {
            check_time(time)?;
        }
        if let Some(overlap) = change.overlap {
            check_overlap(overlap)?;
        }
        if let Some(v) = &change.handle_buttons {
            v.check()?;
        }
        let s = &mut self.settings;
        if let Some(v) = change.record_by {
            s.cue_record_by = v;
        }
        if let Some(v) = change.auto_connect {
            s.auto_connect = v;
        }
        if let Some(v) = change.cue_list_fader {
            s.cue_list_fader = v;
        }
        if let Some(v) = change.update_mode {
            s.update_mode = v;
        }
        if let Some(v) = &change.handle_buttons {
            s.handle_buttons = v.clone();
        }
        if let Some(v) = change.delay {
            s.cue_delay = v;
        }
        if let Some(v) = change.fade_in {
            s.cue_fade_in = v;
        }
        if let Some(v) = change.fade_out {
            s.cue_fade_out = v;
        }
        if let Some(v) = change.delay_out {
            s.cue_delay_out = v;
        }
        if let Some(v) = change.overlap {
            s.cue_overlap = v;
        }
        if let Some(v) = &change.release_mask {
            s.release_mask = v.clone();
        }
        if let Some(v) = change.release_time {
            s.release_time = v;
        }
        Ok(())
    }

    /// PBK-09..13, CUE-03..06: record the programmer's contents in the mask onto a handle. An
    /// empty handle gets a new playback; a used one (or the playback a lock puts on it, PBK-07)
    /// needs a mode, or `add` (a new cue at the end), or a `cue` number (a new cue, or a mode
    /// for a held one). Palette references stay references with the value they resolve to now
    /// as fallback (PBK-11). Refused records change nothing.
    pub fn record_playback(&mut self, record: &PlaybackRecord) -> Result<PlaybackId, CoreError> {
        self.record_playback_except(record, &BTreeSet::new())
    }

    /// `record_playback` without the attributes `skip` names (PBK-49: a ticked palette takes them).
    fn record_playback_except(
        &mut self,
        record: &PlaybackRecord,
        skip: &BTreeSet<(Item, String)>,
    ) -> Result<PlaybackId, CoreError> {
        let (page, slot) = (record.page, record.slot);
        let existing = self.handle_target(page, slot)?;
        let held = existing.and_then(|id| self.playbacks.get(&id));
        if let Some(master) = held.and_then(|p| p.master.as_ref()) {
            return Err(CoreError::HoldsMaster {
                page,
                slot,
                master: master.to_string(),
            });
        }
        if record.add.is_some() && (record.mode.is_some() || record.cue.is_some()) {
            return Err(CoreError::InvalidCommand(
                "Append and Insert take no mode and no cue number".into(),
            ));
        }
        if record.cue.is_some_and(|n| !n.is_valid()) {
            return Err(CoreError::InvalidCueNumber);
        }
        // CUE-06: the cue of a running list that is current, by position.
        let running =
            (held.zip(existing).filter(|(pb, _)| pb.is_cue_list())).and_then(|(pb, id)| {
                let current = self.states.get(&id)?.run.current?;
                pb.cues.iter().position(|c| c.number == current)
            });
        // The cue a mode acts on; `None` is a new cue.
        let target = match (held, record.add, record.cue) {
            (Some(pb), None, Some(n)) => {
                let at = pb.cues.iter().position(|c| c.number == n);
                if at.is_some() && record.mode.is_none() {
                    return Err(CoreError::CueExists(n));
                }
                at
            }
            (Some(pb), None, None) => {
                if record.mode.is_none() {
                    return Err(CoreError::HandleUsedRecord { page, slot });
                }
                if pb.cues.len() > 1 && running.is_none() {
                    let name = if pb.legend.is_empty() {
                        format!("{page}.{slot}")
                    } else {
                        pb.legend.clone()
                    };
                    return Err(CoreError::NameACue(name));
                }
                Some(running.unwrap_or(0))
            }
            _ => None,
        };
        // CUE-03, 05: Append goes after the last cue, Insert after the current one.
        let insert_after = match (held, existing, record.add) {
            (Some(pb), Some(id), Some(AddCue::Insert)) if pb.is_cue_list() => {
                self.states.get(&id).and_then(|s| s.run.current)
            }
            _ => None,
        };
        let new_number = match (held, target) {
            (Some(pb), None) => Some(match (record.cue, insert_after, pb.cues.last()) {
                (Some(n), _, _) => n,
                (None, Some(current), _) => {
                    let after = pb.cues.iter().map(|c| c.number).find(|&n| n > current);
                    CueNumber::between(current, after).ok_or_else(|| {
                        CoreError::NoCueRoom(match after {
                            Some(after) => format!("between Q{current} and Q{after}"),
                            None => format!("after Q{current}"),
                        })
                    })?
                }
                (None, None, Some(last)) => CueNumber::between(last.number, None)
                    .ok_or_else(|| CoreError::NoCueRoom(format!("after Q{}", last.number)))?,
                (None, None, None) => CueNumber::default(),
            }),
            _ => None,
        };
        let was_list = held.is_some_and(Playback::is_cue_list);
        let mask = (record.mask.clone()).unwrap_or_else(|| ALL_GROUPS.to_vec());
        let mut values = self.contents(&mask);
        if !skip.is_empty() {
            for (item, attrs) in &mut values {
                attrs.retain(|name, _| !skip.contains(&(*item, name.clone())));
            }
            values.retain(|_, attrs| !attrs.is_empty());
        }
        let effects = self.recorded_effects(&mask);
        for (&item, attrs) in &mut values {
            for (name, stored) in attrs {
                if let Stored::Reference(r) = stored {
                    r.value = self.resolve_stored(item, name, Stored::Reference(*r));
                }
            }
        }
        if record.record_by.unwrap_or(self.settings.cue_record_by) == RecordBy::Mixed {
            let mut plain: BTreeMap<Item, BTreeMap<String, Value>> = values
                .iter()
                .map(|(&id, attrs)| {
                    let now = attrs.iter().map(|(n, s)| (n.clone(), s.value()));
                    (id, now.collect())
                })
                .collect();
            self.mix_groups(&mut plain);
            for (id, attrs) in plain {
                let entry = values.entry(id).or_default();
                for (name, value) in attrs {
                    entry.entry(name).or_insert(Stored::Value(value));
                }
            }
        }
        // Update takes what the cue holds; on a tracking list its state, tracked values included
        // (CUE-52).
        let tracking = held.is_some_and(|p| p.is_cue_list() && p.list.tracking);
        if let (Some(at), Some(RecordMode::QuickMerge)) = (target, record.mode) {
            let state = held.filter(|_| tracking).map(|p| p.cue_state(at));
            let own = held.and_then(|p| p.cues.get(at)).map(|c| &c.values);
            values.retain(|fixture, attrs| {
                attrs.retain(|name, _| match &state {
                    Some(state) => (state.get(fixture)).is_some_and(|a| a.contains_key(name)),
                    None => own
                        .and_then(|h| h.get(fixture))
                        .is_some_and(|h| h.contains_key(name)),
                });
                !attrs.is_empty()
            });
        }
        if values.is_empty() && effects.is_empty() {
            return Err(CoreError::NothingToRecordPlayback);
        }
        let order = self.recorded_order(&values);
        let Some(id) = existing else {
            let number = record.cue.unwrap_or_default();
            let mut playback = Playback::new(self.playback_next_id, page, slot, Values::new());
            if let Some(kind @ (PlaybackKind::Chase | PlaybackKind::Cues)) = record.kind {
                playback.kind = kind;
            }
            playback.cues = vec![Cue {
                effects,
                order,
                ..self.new_cue(values, number)
            }];
            let id = playback.id;
            self.add_playback(playback);
            return Ok(id);
        };
        if let Some(number) = new_number {
            // CUE-15: a playback that becomes a cue list takes the setting.
            let fader = self.settings.cue_list_fader;
            let cue = Cue {
                effects,
                order,
                ..self.new_cue(values, number)
            };
            if let Some(playback) = self.playbacks.get_mut(&id) {
                playback.cues.push(cue);
                playback.cues.sort_by_key(|c| c.number);
                if !was_list && playback.is_cue_list() {
                    playback.list.fader = fader;
                }
            }
        } else if let Some(playback) = self.playbacks.get_mut(&id) {
            if playback.cues.is_empty() {
                playback.cues.push(Cue::new(Values::new()));
            }
            let merging = matches!(
                record.mode,
                Some(RecordMode::Merge | RecordMode::QuickMerge)
            );
            if let Some(cue) = target.and_then(|at| playback.cues.get_mut(at)) {
                // PRG-80: Replace records a fresh order, Merge and Update extend the cue's.
                cue.order = if merging {
                    let numbered = |i: &Item| Some((self.fixtures.get(&i.fixture)?.number, *i));
                    let selected = !self.programmer.items().is_empty();
                    let base = match (cue.order.is_empty(), selected) {
                        (false, _) => cue.order.clone(),
                        (true, true) => {
                            stagger_ranks(&cue.order, cue.values.keys().filter_map(numbered))
                        }
                        (true, false) => BTreeMap::new(),
                    };
                    if base.is_empty() && !selected {
                        BTreeMap::new()
                    } else if selected {
                        merge_order(base, &order)
                    } else {
                        let rest =
                            stagger_ranks(&BTreeMap::new(), values.keys().filter_map(numbered));
                        merge_order(base, &rest)
                    }
                } else {
                    order
                };
            }
            let tracked = tracking && !matches!(record.mode, Some(RecordMode::Replace) | None);
            if let (true, Some(at)) = (tracked, target) {
                // CUE-52: each attribute goes where the update mode says.
                let mode = record.update_mode.unwrap_or(self.settings.update_mode);
                let (fixtures, profiles) = (&self.fixtures, &self.profiles);
                let htp = |item: Item, name: &str| {
                    let mode = resolve_mode(profiles, fixtures.get(&item.fixture)?)?;
                    Some(mode.item_logical(item.cell, name)?.kind == AttributeKind::Htp)
                };
                playback.store_tracked(at, values, mode, htp);
            } else if let Some(cue) = target.and_then(|at| playback.cues.get_mut(at)) {
                match record.mode {
                    Some(RecordMode::Replace) | None => {
                        // PAT-33: a parked fixture is not in the programmer; its data stays.
                        let parked =
                            |i: &Item| self.fixtures.get(&i.fixture).is_some_and(|f| f.parked);
                        let kept: Vec<_> = (cue.values.iter())
                            .filter(|(i, _)| parked(i))
                            .map(|(i, v)| (*i, v.clone()))
                            .collect();
                        cue.values = values;
                        for (item, attrs) in kept {
                            cue.values.entry(item).or_insert(attrs);
                        }
                        cue.dormant.retain(|i, _| parked(i)); // CUE-61
                    }
                    Some(RecordMode::Merge | RecordMode::QuickMerge) => {
                        for (fixture, attrs) in values {
                            cue.values.entry(fixture).or_default().extend(attrs);
                        }
                    }
                }
            }
            // CUE-61: a value stored now is on; an attribute is never both on and off.
            for cue in &mut playback.cues {
                let on = &cue.values;
                cue.dormant.retain(|item, attrs| {
                    if let Some(held) = on.get(item) {
                        attrs.retain(|name, _| !held.contains_key(name));
                    }
                    !attrs.is_empty()
                });
            }
            // FX-21: Merge and Update replace the effect per member and attribute, Replace all.
            if let Some(cue) = target.and_then(|at| playback.cues.get_mut(at)) {
                match record.mode {
                    Some(RecordMode::Replace) | None => cue.effects = effects,
                    Some(RecordMode::Merge | RecordMode::QuickMerge) => {
                        for e in &effects {
                            for o in &mut cue.effects {
                                if o.attributes.iter().any(|a| e.attributes.contains(a)) {
                                    o.drop_members(|m| e.members.contains(m));
                                    o.refresh();
                                }
                            }
                        }
                        cue.effects.retain(|o| !o.members.is_empty());
                        cue.effects.extend(effects);
                    }
                }
            }
        }
        // CUE-53: a running list shows the edit at the next tick; CUE-03, 05: the new cue is next.
        let added = new_number.filter(|_| record.cue.is_none());
        // CUE-01, 03: a memory on stage that becomes a list stands on its cue, so it keeps its output.
        let became = (self.playbacks.get(&id))
            .filter(|p| !was_list && p.is_cue_list())
            .and_then(|p| p.cues.first().map(|c| (c.number, p.cue_after(c.number))));
        if let Some(state) = self.states.get_mut(&id) {
            if let (Some((first, after)), true) = (became, state.is_active()) {
                state.run.current = Some(first);
                state.run.next = after;
            }
            let run = &mut state.run;
            run.edited = true;
            match (record.add, added) {
                (Some(AddCue::Insert), Some(n)) if run.current.is_some() => run.next = Some(n),
                (Some(AddCue::Append), Some(n)) if run.current.is_some() && run.next.is_none() => {
                    run.next = Some(n);
                }
                _ => {}
            }
        }
        Ok(id)
    }

    /// PBK-49: does the palette hold a value for this attribute of this item?
    fn palette_resolves(&self, palette: &Palette, item: Item, name: &str) -> bool {
        let (Some(fixture), Some(mode)) =
            (self.fixtures.get(&item.fixture), self.mode_of(item.fixture))
        else {
            return false;
        };
        palette.resolve(item, fixture, mode, name).is_some()
    }

    /// PBK-49: the active playbacks whose current cue holds something the programmer touches, and
    /// the palettes that hold it; those the listed cues reference come first. What the cue holds
    /// is what Update would merge into it (CUE-52), so what is off is never read.
    pub fn update_targets(&self) -> UpdateTargets {
        let touched = self.contents(&ALL_GROUPS);
        let mut out = UpdateTargets::default();
        let mut referenced = BTreeSet::new();
        let mut ordered: Vec<&Playback> = self.playbacks.values().collect();
        ordered.sort_by_key(|p| (p.page, p.slot));
        for p in ordered {
            let state = self.states.get(&p.id);
            if !state.is_some_and(PlaybackState::is_active) {
                continue;
            }
            let Some((at, cue)) = cue_index(p, state).and_then(|at| Some((at, p.cues.get(at)?)))
            else {
                continue;
            };
            let held: Vec<(Item, String, Stored)> = if p.is_cue_list() && p.list.tracking {
                (p.cue_state(at).into_iter())
                    .flat_map(|(i, a)| a.into_iter().map(move |(n, (s, _))| (i, n, s)))
                    .collect()
            } else {
                (cue.values.iter())
                    .flat_map(|(i, a)| a.iter().map(|(n, s)| (*i, n.clone(), *s)))
                    .collect()
            };
            if held
                .iter()
                .any(|(i, n, _)| touched.get(i).is_some_and(|a| a.contains_key(n)))
            {
                out.playbacks.push(UpdateTarget {
                    id: p.id,
                    cue: cue.number,
                });
                referenced.extend(held.iter().filter_map(|(_, _, s)| match s {
                    Stored::Reference(r) => Some(r.palette),
                    Stored::Value(_) => None,
                }));
            }
        }
        let mut palettes: Vec<&Palette> = (self.palettes.values())
            .filter(|p| {
                (touched.iter()).any(|(&i, a)| a.keys().any(|n| self.palette_resolves(p, i, n)))
            })
            .collect();
        palettes.sort_by_key(|p| (!referenced.contains(&p.id), p.number));
        out.palettes = palettes.into_iter().map(|p| p.number).collect();
        out
    }

    /// PBK-49: store what the programmer holds into the ticked palettes (what each holds for
    /// that fixture) and cues (QuickMerge, CUE-52), a palette first and its attributes left out of
    /// the cues so a reference stays. One undo step; a refusal changes nothing.
    pub fn update(
        &mut self,
        playbacks: &[UpdateTarget],
        palettes: &[u32],
        update_mode: Option<UpdateMode>,
    ) -> Result<(), CoreError> {
        let touched = self.contents(&ALL_GROUPS);
        if touched.is_empty() && self.programmer.effects.is_empty() {
            return Err(CoreError::NothingToRecordPlayback);
        }
        if playbacks.is_empty() && palettes.is_empty() {
            return Err(CoreError::ListRefused("Update: no targets".into()));
        }
        for &n in palettes {
            self.palette(n).ok_or(CoreError::UnknownPalette(n))?;
        }
        for t in playbacks {
            let p = (self.playbacks.get(&t.id)).ok_or(CoreError::UnknownPlayback(t.id))?;
            if !p.cues.iter().any(|c| c.number == t.cue) {
                return Err(CoreError::UnknownCue(t.cue));
            }
        }
        let mut taken = BTreeSet::new();
        for palette in palettes.iter().filter_map(|&n| self.palette(n)) {
            for (&item, attrs) in &touched {
                for name in attrs.keys() {
                    if self.palette_resolves(palette, item, name) {
                        taken.insert((item, name.clone()));
                    }
                }
            }
        }
        let before = self.edit_state();
        let result = self.update_stores(playbacks, palettes, update_mode, &taken);
        if result.is_err() {
            self.restore_edit_state(before);
        }
        result
    }

    fn update_stores(
        &mut self,
        playbacks: &[UpdateTarget],
        palettes: &[u32],
        update_mode: Option<UpdateMode>,
        taken: &BTreeSet<(Item, String)>,
    ) -> Result<(), CoreError> {
        let mut stored = false;
        let mut nothing = CoreError::NothingToRecordPlayback;
        let mut note = |r: Result<PlaybackId, CoreError>| match r {
            Ok(_) => {
                stored = true;
                Ok(())
            }
            Err(e @ (CoreError::NothingToRecord(_) | CoreError::NothingToRecordPlayback)) => {
                nothing = e;
                Ok(())
            }
            Err(e) => Err(e),
        };
        for &n in palettes {
            let Some(cell) = self.palette(n).map(Palette::cell) else {
                continue;
            };
            let record = Record {
                mask: Some(ALL_GROUPS.to_vec()),
                record_by: Some(RecordBy::Channel),
                mode: Some(RecordMode::QuickMerge),
                ..Record::at(cell)
            };
            note(self.record_palette_with(&record, true))?;
        }
        for t in playbacks {
            let Some((page, slot)) = self.playbacks.get(&t.id).map(|p| (p.page, p.slot)) else {
                continue;
            };
            let record = PlaybackRecord {
                mode: Some(RecordMode::QuickMerge),
                cue: Some(t.cue),
                update_mode,
                ..PlaybackRecord::at(page, slot)
            };
            note(self.record_playback_except(&record, taken))?;
        }
        if stored {
            Ok(())
        } else {
            Err(nothing)
        }
    }

    /// PBK-12: a new cue with the times of the settings and link `Wait`.
    fn new_cue(&self, values: Values, number: CueNumber) -> Cue {
        let s = &self.settings;
        Cue {
            delay: s.cue_delay,
            fade_in: s.cue_fade_in,
            fade_out: s.cue_fade_out,
            delay_out: s.cue_delay_out,
            overlap: s.cue_overlap,
            number,
            ..Cue::new(values)
        }
    }

    /// CUE-35, 36: is a master of this kind with this number on a handle?
    fn master_in_use(&self, kind: MasterKind, number: u8) -> bool {
        (self.playbacks.values())
            .filter_map(|p| p.master.as_ref())
            .any(|m| m.kind() == Some(kind) && m.number() == number)
    }

    /// CUE-35, 36, IPC-26: the lowest number of this kind no master uses; `None` when all are.
    pub fn free_master_number(&self, kind: MasterKind) -> Option<u8> {
        (1..=kind.max()).find(|&n| !self.master_in_use(kind, n))
    }

    /// CUE-35, 36: put a speed master on an empty handle. The number is 1–8 for BPM and 1–4 for
    /// Rate, and not used by another master of its kind.
    pub fn record_master(
        &mut self,
        page: u16,
        slot: u16,
        kind: MasterKind,
        number: u8,
    ) -> Result<PlaybackId, CoreError> {
        kind.check_number(number)?;
        let master = Master::new(kind, number);
        if self.master_in_use(kind, number) {
            return Err(CoreError::InvalidCommand(format!(
                "{master} is already in use"
            )));
        }
        self.check_empty_handle(page, slot)?;
        let id = self.playback_next_id;
        self.add_playback(Playback {
            kind: match kind {
                MasterKind::Bpm => PlaybackKind::BpmMaster,
                MasterKind::Rate => PlaybackKind::RateMaster,
            },
            master: Some(master),
            cues: Vec::new(),
            ..Playback::new(id, page, slot, Values::new())
        });
        Ok(id)
    }

    /// GRM-01: put a group master of `group` on an empty handle. A group may have several.
    pub fn record_group_master(
        &mut self,
        page: u16,
        slot: u16,
        group: GroupId,
    ) -> Result<PlaybackId, CoreError> {
        if !self.groups.contains_key(&group) {
            return Err(CoreError::UnknownGroup(group));
        }
        self.check_empty_handle(page, slot)?;
        let id = self.playback_next_id;
        self.add_playback(Playback {
            kind: PlaybackKind::GroupMaster,
            master: Some(Master::Group(GroupMaster::new(group))),
            cues: Vec::new(),
            ..Playback::new(id, page, slot, Values::new())
        });
        Ok(id)
    }

    /// GRM-21: point a group master at another group; everything else of it stays. Taken
    /// fixtures outside the new group are let go by the next tick (`follow_take_over`).
    pub fn set_master_group(&mut self, id: PlaybackId, group: GroupId) -> Result<(), CoreError> {
        let known = self.groups.contains_key(&group);
        let playback = self.playback_mut(id)?;
        let Some(Master::Group(m)) = &mut playback.master else {
            return Err(CoreError::NotAGroupMaster(id));
        };
        if !known {
            return Err(CoreError::UnknownGroup(group));
        }
        m.group = group;
        Ok(())
    }

    fn playback_mut(&mut self, id: PlaybackId) -> Result<&mut Playback, CoreError> {
        self.playbacks
            .get_mut(&id)
            .ok_or(CoreError::UnknownPlayback(id))
    }

    /// CUE-07: remove a cue; the last one stays.
    pub fn delete_cue(&mut self, id: PlaybackId, cue: CueNumber) -> Result<(), CoreError> {
        let playback = self.playback_mut(id)?;
        let at = (playback.cues.iter().position(|c| c.number == cue))
            .ok_or(CoreError::UnknownCue(cue))?;
        if playback.cues.len() == 1 {
            return Err(CoreError::LastCue(cue));
        }
        playback.cues.remove(at);
        // The cue after the deleted one is next when that was (CUE-07); tracking follows (CUE-53).
        let after = playback.cue_after(cue);
        if let Some(run) = self.states.get_mut(&id).map(|s| &mut s.run) {
            if run.next == Some(cue) {
                run.next = after;
            }
            run.edited = true;
        }
        Ok(())
    }

    /// CUE-63: remove several cues; refused as a whole when one is unknown or when they are
    /// every cue of the playback.
    pub fn delete_cues(&mut self, id: PlaybackId, cues: &[CueNumber]) -> Result<(), CoreError> {
        let playback = self.playback_mut(id)?;
        let cues: BTreeSet<_> = cues.iter().copied().collect();
        let first = *cues
            .first()
            .ok_or_else(|| CoreError::InvalidCommand("expected at least one cue".to_string()))?;
        if let Some(&c) = cues
            .iter()
            .find(|c| !playback.cues.iter().any(|x| x.number == **c))
        {
            return Err(CoreError::UnknownCue(c));
        }
        if cues.len() >= playback.cues.len() {
            return Err(CoreError::LastCue(cues.last().copied().unwrap_or(first)));
        }
        for c in cues {
            self.delete_cue(id, c)?;
        }
        Ok(())
    }

    /// CUE-08: give a cue a free number; it moves to its place in the order.
    pub fn renumber_cue(
        &mut self,
        id: PlaybackId,
        cue: CueNumber,
        to: CueNumber,
    ) -> Result<(), CoreError> {
        let playback = self.playback_mut(id)?;
        check_new_cue_number(playback, to)?;
        (playback.cues.iter_mut().find(|c| c.number == cue))
            .ok_or(CoreError::UnknownCue(cue))?
            .number = to;
        playback.cues.sort_by_key(|c| c.number);
        if let Some(run) = self.states.get_mut(&id).map(|s| &mut s.run) {
            run.renumbered(|n| if n == cue { to } else { n }); // CUE-08
            run.edited = true;
        }
        Ok(())
    }

    /// CUE-08: number every cue 1, 2, 3 … in their order.
    pub fn renumber_cues(&mut self, id: PlaybackId) -> Result<(), CoreError> {
        let playback = self.playback_mut(id)?;
        let numbers: Option<Vec<CueNumber>> = (1u32..)
            .map(|n| CueNumber::new(f64::from(n)))
            .take(playback.cues.len())
            .collect();
        let numbers = numbers.ok_or(CoreError::InvalidCueNumber)?;
        let old: Vec<CueNumber> = playback.cues.iter().map(|c| c.number).collect();
        for (cue, &number) in playback.cues.iter_mut().zip(&numbers) {
            cue.number = number;
        }
        if let Some(run) = self.states.get_mut(&id).map(|s| &mut s.run) {
            let to = |n: CueNumber| {
                old.iter()
                    .position(|&o| o == n)
                    .and_then(|i| numbers.get(i))
            };
            run.renumbered(|n| to(n).copied().unwrap_or(n)); // CUE-08
            run.edited = true;
        }
        Ok(())
    }

    /// CUE-08: copy a cue to a free number of the same playback.
    pub fn copy_cue(
        &mut self,
        id: PlaybackId,
        cue: CueNumber,
        to: CueNumber,
    ) -> Result<(), CoreError> {
        let playback = self.playback_mut(id)?;
        check_new_cue_number(playback, to)?;
        let copy = Cue {
            number: to,
            ..(playback.cues.iter().find(|c| c.number == cue))
                .ok_or(CoreError::UnknownCue(cue))?
                .clone()
        };
        playback.cues.push(copy);
        playback.cues.sort_by_key(|c| c.number);
        if let Some(state) = self.states.get_mut(&id) {
            state.run.edited = true; // CUE-53
        }
        Ok(())
    }

    /// CUE-54: the state of a cue per fixture and attribute, with the cue each value is hard in.
    pub fn cue_view(&self, id: PlaybackId, cue: CueNumber) -> Result<CueView, CoreError> {
        let playback = (self.playbacks.get(&id)).ok_or(CoreError::UnknownPlayback(id))?;
        let at = (playback.cues.iter().position(|c| c.number == cue))
            .ok_or(CoreError::UnknownCue(cue))?;
        let mut view = CueView::new();
        for (fixture, attributes) in playback.cue_state(at) {
            for (name, (value, i)) in attributes {
                let from = playback.cues.get(i).map_or(cue, |c| c.number);
                (view.entry(fixture).or_default()).insert(name, CueValue { value, from });
            }
        }
        Ok(view)
    }

    /// CUE-59: the effect entries of a cue's state (FX-27) with the cue each is hard in.
    pub fn cue_effects(&self, id: PlaybackId, cue: CueNumber) -> Result<Vec<CueEffect>, CoreError> {
        let playback = (self.playbacks.get(&id)).ok_or(CoreError::UnknownPlayback(id))?;
        let at = (playback.cues.iter().position(|c| c.number == cue))
            .ok_or(CoreError::UnknownCue(cue))?;
        let hard = |entry: &effect::Entry| {
            (playback.cues.iter().take(at + 1).rev())
                .find(|c| c.effects.iter().any(|e| e.id == entry.id))
                .map_or(cue, |c| c.number)
        };
        Ok(playback
            .effect_state(at)
            .into_iter()
            .map(|entry| CueEffect {
                from: hard(&entry),
                entry,
            })
            .collect())
    }

    /// CUE-59: take a hard effect entry out of a cue; refused, changing nothing, when the cue
    /// does not hold it. On stage at once (CUE-53).
    pub fn remove_cue_effect(
        &mut self,
        id: PlaybackId,
        cue: CueNumber,
        entry: EntryId,
    ) -> Result<(), CoreError> {
        let playback = self.playback_mut(id)?;
        let cue = (playback.cues.iter_mut())
            .find(|c| c.number == cue)
            .ok_or(CoreError::UnknownCue(cue))?;
        let before = cue.effects.len();
        cue.effects.retain(|e| e.id != entry);
        if cue.effects.len() == before {
            return Err(CoreError::InvalidCommand(format!(
                "Q{} holds no effect {entry} of its own",
                cue.number
            )));
        }
        if let Some(state) = self.states.get_mut(&id) {
            state.run.edited = true;
        }
        self.follow_cue_effects();
        Ok(())
    }

    /// CUE-54: take a hard value (or all of a fixture's) out of a cue so it tracks from earlier
    /// cues again; refused, changing nothing, when the cue does not hold it. On stage at once
    /// (CUE-53).
    pub fn remove_cue_value(
        &mut self,
        id: PlaybackId,
        cue: CueNumber,
        item: Item,
        attribute: Option<&str>,
    ) -> Result<(), CoreError> {
        let playback = self.playback_mut(id)?;
        let cue = (playback.cues.iter_mut())
            .find(|c| c.number == cue)
            .ok_or(CoreError::UnknownCue(cue))?;
        let held = cue.values.get_mut(&item);
        let removed = match (held, attribute) {
            (Some(_), None) => cue.values.remove(&item).is_some(),
            (Some(attributes), Some(name)) => {
                let removed = attributes.remove(name).is_some();
                if attributes.is_empty() {
                    cue.values.remove(&item);
                }
                removed
            }
            (None, _) => false,
        };
        // CUE-60: a whole item also leaves the members of the cue's own effect entries.
        let member = attribute.is_none() && cue.effects.iter().any(|e| e.members.contains(&item));
        if member {
            effect::Entry::retire(&mut cue.effects, |m| *m == item);
        }
        if !(removed || member) {
            let what = match (attribute, item.cell) {
                (Some(name), _) => name.to_string(),
                (None, Some(_)) => item.to_string(),
                (None, None) => format!("fixture {item}"),
            };
            return Err(CoreError::InvalidCommand(format!(
                "Q{} holds no {what} of its own",
                cue.number
            )));
        }
        if let Some(state) = self.states.get_mut(&id) {
            state.run.edited = true;
        }
        self.follow_cue_effects();
        Ok(())
    }

    /// CUE-61: move one attribute (or all of an item's) between the cue's values and what is off.
    pub fn set_cue_off(
        &mut self,
        id: PlaybackId,
        cue: CueNumber,
        item: Item,
        attribute: Option<&str>,
        off: bool,
    ) -> Result<(), CoreError> {
        let playback = self.playback_mut(id)?;
        let cue = (playback.cues.iter_mut())
            .find(|c| c.number == cue)
            .ok_or(CoreError::UnknownCue(cue))?;
        let (from, to) = match off {
            true => (&mut cue.values, &mut cue.dormant),
            false => (&mut cue.dormant, &mut cue.values),
        };
        let moved = match (from.get_mut(&item), attribute) {
            (Some(_), None) => from.remove(&item),
            (Some(attributes), Some(name)) => {
                let one = attributes.remove_entry(name);
                let rest = attributes.is_empty();
                if rest {
                    from.remove(&item);
                }
                one.map(|(k, v)| BTreeMap::from([(k, v)]))
            }
            (None, _) => None,
        };
        let Some(moved) = moved else {
            let what = match (attribute, item.cell) {
                (Some(name), _) => name.to_string(),
                (None, Some(_)) => item.to_string(),
                (None, None) => format!("fixture {item}"),
            };
            let tail = if off { "of its own" } else { "that is off" };
            return Err(CoreError::InvalidCommand(format!(
                "Q{} holds no {what} {tail}",
                cue.number
            )));
        };
        to.entry(item).or_default().extend(moved);
        if let Some(state) = self.states.get_mut(&id) {
            state.run.edited = true;
        }
        Ok(())
    }

    /// CUE-49, IPC-56: the same change to several cues; one unknown cue or bad value refuses all.
    pub fn change_cues(
        &mut self,
        id: PlaybackId,
        cues: &[CueNumber],
        change: &CueChange,
    ) -> Result<(), CoreError> {
        let pb = self.playback(id).ok_or(CoreError::UnknownPlayback(id))?;
        if let Some(n) = cues
            .iter()
            .find(|n| !pb.cues.iter().any(|c| c.number == **n))
        {
            return Err(CoreError::UnknownCue(*n));
        }
        // change_cue validates before it mutates and the change is the same each time, so a bad value refuses on the first cue.
        cues.iter()
            .try_for_each(|n| self.change_cue(id, *n, change))
    }

    /// CUE-09, 51: change one cue's legend, times, link, tracking and group times; refused as a
    /// whole when one is out of range.
    pub fn change_cue(
        &mut self,
        id: PlaybackId,
        cue: CueNumber,
        change: &CueChange,
    ) -> Result<(), CoreError> {
        if let Some(legend) = &change.legend {
            check_legend(legend)?;
        }
        for time in [
            change.delay,
            change.fade_in,
            change.fade_out.flatten(),
            change.delay_out.flatten(),
        ]
        .into_iter()
        .flatten()
        {
            check_time(time)?;
        }
        if let Some(overlap) = change.overlap {
            check_overlap(overlap)?;
        }
        if let Some(link) = &change.link {
            check_link(link)?;
        }
        if let Some(times) = &change.group_times {
            check_group_times(times)?;
        }
        let cue = (self.playback_mut(id)?.cues.iter_mut())
            .find(|c| c.number == cue)
            .ok_or(CoreError::UnknownCue(cue))?;
        if let Some(v) = &change.legend {
            cue.legend = v.clone();
        }
        if let Some(v) = change.delay {
            cue.delay = v;
        }
        if let Some(v) = change.fade_in {
            cue.fade_in = v;
        }
        if let Some(v) = change.fade_out {
            cue.fade_out = v;
        }
        if let Some(v) = change.delay_out {
            cue.delay_out = v;
        }
        if let Some(v) = change.overlap {
            cue.overlap = v;
        }
        if let Some(v) = change.link {
            cue.link = v;
        }
        if let Some(v) = change.track {
            cue.track = v;
        }
        if let Some(v) = &change.group_times {
            cue.group_times = v.clone();
        }
        if let Some(state) = self.states.get_mut(&id) {
            state.run.edited = true; // CUE-53: a track change shows on the next tick
        }
        Ok(())
    }

    /// PRG-81: the positions of the cue's items; its order, or the user-number order if it has none.
    fn cue_ranks(&self, cue: &Cue) -> BTreeMap<Item, u32> {
        stagger_ranks(
            &cue.order,
            (cue.values.keys()).filter_map(|i| Some((self.fixtures.get(&i.fixture)?.number, *i))),
        )
    }

    /// PRG-80: the order a record stores for `values`: the selection's positions under its grid,
    /// then the recorded items outside it in user-number order. No selection, no order.
    fn recorded_order(&self, values: &Values) -> BTreeMap<Item, u32> {
        let selection = self.programmer.items();
        if selection.is_empty() {
            return BTreeMap::new();
        }
        let mut order: BTreeMap<Item, u32> = (selection.iter().zip(self.programmer.positions()))
            .filter(|(i, _)| values.contains_key(i))
            .map(|(&i, &k)| (i, k))
            .collect();
        let mut rest: Vec<(u32, Item)> = (values.keys())
            .filter(|i| !order.contains_key(i))
            .filter_map(|i| Some((self.fixtures.get(&i.fixture)?.number, *i)))
            .collect();
        rest.sort_unstable();
        let n = self.programmer.position_count();
        for (k, (_, item)) in (n..).zip(rest) {
            order.insert(item, k);
        }
        order
    }

    /// PRG-81: put the cue's items in a new order. The given items take their positions, the
    /// others follow in their old order. An item the cue lacks or a negative position is refused.
    pub fn set_cue_order(
        &mut self,
        id: PlaybackId,
        cue: CueNumber,
        order: &BTreeMap<Item, i64>,
    ) -> Result<(), CoreError> {
        let found = (self.playbacks.get(&id))
            .ok_or(CoreError::UnknownPlayback(id))?
            .cues
            .iter()
            .find(|c| c.number == cue)
            .ok_or(CoreError::UnknownCue(cue))?;
        let mut new = BTreeMap::new();
        for (&item, &k) in order {
            if !found.values.contains_key(&item) {
                return Err(CoreError::InvalidCommand(format!(
                    "{item} is not in cue {cue}"
                )));
            }
            let k = u32::try_from(k).map_err(|_| {
                CoreError::InvalidCommand(format!("{item}: position {k} is out of range"))
            })?;
            new.insert(item, k);
        }
        let old = self.cue_ranks(found);
        let ranks: BTreeSet<u32> = (old.iter())
            .filter(|(i, _)| !new.contains_key(i))
            .map(|(_, &k)| k)
            .collect();
        let first = new.values().max().map_or(0, |m| m.saturating_add(1));
        for (&item, k) in &old {
            if let (false, Some(rank)) =
                (new.contains_key(&item), ranks.iter().position(|r| r == k))
            {
                new.insert(
                    item,
                    first.saturating_add(u32::try_from(rank).unwrap_or(u32::MAX)),
                );
            }
        }
        let target = (self.playback_mut(id)?.cues.iter_mut())
            .find(|c| c.number == cue)
            .ok_or(CoreError::UnknownCue(cue))?;
        target.order = new;
        if let Some(state) = self.states.get_mut(&id) {
            state.run.edited = true;
        }
        Ok(())
    }

    /// PBK-14, CUE-23, CUE-37: a new playback (new id) on an empty handle with the same cues,
    /// legend, icon, halo and options. A master gets the next free number of its kind.
    pub fn copy_playback(
        &mut self,
        id: PlaybackId,
        page: u16,
        slot: u16,
    ) -> Result<PlaybackId, CoreError> {
        let mut source = self
            .playbacks
            .get(&id)
            .ok_or(CoreError::UnknownPlayback(id))?
            .clone();
        self.check_empty_handle(page, slot)?;
        // A group master keeps its group and options; a speed master gets a free number.
        if let Some((kind, master)) = (source.master.as_mut()).and_then(|m| Some((m.kind()?, m))) {
            let free = self.free_master_number(kind);
            master.set_number(free.ok_or_else(|| {
                CoreError::InvalidCommand(format!("no free {master} number left"))
            })?);
        }
        let new = self.playback_next_id;
        self.add_playback(Playback {
            id: new,
            page,
            slot,
            ..source
        });
        Ok(new)
    }

    /// PBK-14: change only the home handle, so a running playback keeps running.
    pub fn move_playback(&mut self, id: PlaybackId, page: u16, slot: u16) -> Result<(), CoreError> {
        if !self.playbacks.contains_key(&id) {
            return Err(CoreError::UnknownPlayback(id));
        }
        self.check_empty_handle(page, slot)?;
        if let Some(playback) = self.playbacks.get_mut(&id) {
            playback.page = page;
            playback.slot = slot;
        }
        Ok(())
    }

    /// PBK-14: remove a playback; it leaves the merge with its state. The id is never reused.
    pub fn delete_playback(&mut self, id: PlaybackId) -> Result<(), CoreError> {
        self.playbacks
            .remove(&id)
            .ok_or(CoreError::UnknownPlayback(id))?;
        self.states.remove(&id);
        Ok(())
    }

    /// PBK-50: the sources in handle order with the handle each lands on. Refuses an empty or
    /// unknown list, `Retain` over several pages and any target outside the pages.
    fn playback_block(
        &self,
        ids: &[PlaybackId],
        page: u16,
        slot: u16,
        layout: Layout,
    ) -> Result<Vec<(PlaybackId, u16, u16)>, CoreError> {
        let mut src = Vec::new();
        for &id in ids {
            let p = self
                .playbacks
                .get(&id)
                .ok_or(CoreError::UnknownPlayback(id))?;
            src.push((p.page, p.slot, id));
        }
        src.sort_unstable();
        src.dedup_by_key(|s| s.2);
        let first = src.first().ok_or_else(|| {
            CoreError::InvalidCommand("expected at least one playback".to_string())
        })?;
        let pages = src.iter().map(|s| s.0).collect::<BTreeSet<_>>().len();
        if layout == Layout::Retain && pages > 1 {
            return Err(CoreError::SourcesOnPages(pages));
        }
        let lowest = first.1;
        let mut block = Vec::new();
        for (i, &(_, s, id)) in src.iter().enumerate() {
            let offset = match layout {
                Layout::Retain => s.saturating_sub(lowest),
                Layout::Bunch => u16::try_from(i).unwrap_or(u16::MAX),
            };
            let target = slot.saturating_add(offset);
            self.handle_target(page, target)?;
            block.push((id, page, target));
        }
        Ok(block)
    }

    /// PBK-50: copy several playbacks onto empty handles; the new ids come back in handle
    /// order. All or nothing.
    pub fn copy_playbacks(
        &mut self,
        ids: &[PlaybackId],
        page: u16,
        slot: u16,
        layout: Layout,
    ) -> Result<Vec<PlaybackId>, CoreError> {
        let block = self.playback_block(ids, page, slot, layout)?;
        for &(_, p, s) in &block {
            self.check_empty_handle(p, s)?;
        }
        let next = self.playback_next_id;
        let mut new = Vec::new();
        for (id, p, s) in block {
            match self.copy_playback(id, p, s) {
                Ok(n) => new.push(n),
                Err(e) => {
                    for n in new {
                        self.playbacks.remove(&n);
                        self.states.remove(&n);
                    }
                    self.playback_next_id = next;
                    return Err(e);
                }
            }
        }
        Ok(new)
    }

    /// PBK-50, 51: move several playbacks (ids and running state kept). With `swap` a playback
    /// in the way takes a handle the block leaves free. All or nothing.
    pub fn move_playbacks(
        &mut self,
        ids: &[PlaybackId],
        page: u16,
        slot: u16,
        layout: Layout,
        swap: bool,
    ) -> Result<(), CoreError> {
        let block = self.playback_block(ids, page, slot, layout)?;
        let moving: BTreeSet<_> = block.iter().map(|b| b.0).collect();
        let mut in_way = Vec::new();
        for &(_, p, s) in &block {
            let Some(o) = self.handle_target(p, s)?.filter(|o| !moving.contains(o)) else {
                continue;
            };
            let home = self.playbacks.get(&o).map_or((p, s), |x| (x.page, x.slot));
            if !swap {
                return Err(CoreError::HandleUsed { page: p, slot: s });
            }
            if home != (p, s) {
                return Err(CoreError::HandleLocked {
                    page: p,
                    slot: s,
                    by_page: home.0,
                    by_slot: home.1,
                });
            }
            in_way.push((home, o));
        }
        in_way.sort_unstable();
        let mut freed = Vec::new();
        for id in &moving {
            if let Some(h) = self.playbacks.get(id).map(|x| (x.page, x.slot)) {
                if !block.iter().any(|&(_, p, s)| (p, s) == h) {
                    freed.push(h);
                }
            }
        }
        freed.sort_unstable();
        if in_way.len() > freed.len() {
            let ((p, s), _) = in_way.get(freed.len()).copied().unwrap_or_default();
            return Err(CoreError::HandleUsed { page: p, slot: s });
        }
        let moves = block.into_iter().map(|(id, p, s)| (id, (p, s)));
        let swaps = in_way.into_iter().map(|(_, o)| o).zip(freed);
        for (id, (p, s)) in moves.chain(swaps) {
            if let Some(pb) = self.playbacks.get_mut(&id) {
                pb.page = p;
                pb.slot = s;
            }
        }
        Ok(())
    }

    /// PBK-50: delete several playbacks; refused as a whole when one is unknown.
    pub fn delete_playbacks(&mut self, ids: &[PlaybackId]) -> Result<(), CoreError> {
        let ids: BTreeSet<_> = ids.iter().copied().collect();
        if ids.is_empty() {
            return Err(CoreError::InvalidCommand(
                "expected at least one playback".to_string(),
            ));
        }
        if let Some(&id) = ids.iter().find(|id| !self.playbacks.contains_key(id)) {
            return Err(CoreError::UnknownPlayback(id));
        }
        for id in ids {
            self.delete_playback(id)?;
        }
        Ok(())
    }

    /// PBK-24, 36, CUE-01, 29, 35, 38: change options and the first cue's times; refused as a
    /// whole when one is out of range or a second `Locked` would share a slot (PBK-07).
    pub fn change_playback(
        &mut self,
        id: PlaybackId,
        change: &PlaybackChange,
    ) -> Result<(), CoreError> {
        let playback = self
            .playbacks
            .get(&id)
            .ok_or(CoreError::UnknownPlayback(id))?;
        let slot = playback.slot;
        if let Some(kind) = change.kind {
            if kind != playback.kind && (kind.is_master() || playback.kind.is_master()) {
                return Err(CoreError::InvalidOption(
                    "kind (a master stays one, and a playback cannot become one)".into(),
                ));
            }
        }
        if let Some(chase) = &change.chase {
            chase.check()?;
        }
        if let Some(master) = &change.master {
            master.check()?;
            let same = match (playback.master.as_ref(), master) {
                (Some(Master::Group(old)), Master::Group(new)) => old.group == new.group,
                (Some(old), new) => {
                    old.kind().is_some() && old.kind() == new.kind() && old.number() == new.number()
                }
                (None, _) => false,
            };
            if !same {
                return Err(CoreError::InvalidOption(
                    "master (it keeps its kind and number, a group master its group)".into(),
                ));
            }
        }
        for time in [
            change.delay,
            change.fade_in,
            change.fade_out.flatten(),
            change.delay_out.flatten(),
            change.release_time.flatten(),
        ]
        .into_iter()
        .flatten()
        {
            check_time(time)?;
        }
        if let Some(overlap) = change.overlap {
            check_overlap(overlap)?;
        }
        if let Some(group) = change.group.flatten().filter(|g| !(1..=99).contains(g)) {
            return Err(CoreError::InvalidGroup(group));
        }
        if change.paging == Some(Paging::Locked)
            && self
                .playbacks
                .values()
                .any(|p| p.id != id && p.slot == slot && p.paging == Paging::Locked)
        {
            return Err(CoreError::SlotAlreadyLocked { slot });
        }
        let Some(playback) = self.playbacks.get_mut(&id) else {
            return Ok(());
        };
        if let Some(v) = change.priority {
            playback.priority = v;
        }
        if let Some(v) = change.fader_mode {
            playback.fader_mode = v;
        }
        if let Some(v) = change.kill_point {
            playback.kill_point = v;
        }
        if let Some(v) = &change.release_mask {
            playback.release_mask = v.clone();
        }
        if let Some(v) = change.release_time {
            playback.release_time = v;
        }
        if let Some(v) = change.paging {
            playback.paging = v;
        }
        if let Some(v) = change.group {
            playback.group = v;
        }
        if let Some(v) = change.kind {
            playback.kind = v;
        }
        if let Some(v) = &change.list {
            playback.list = v.clone();
        }
        if let Some(v) = &change.chase {
            playback.chase = v.clone();
        }
        if let Some(v) = &change.master {
            // GRM-04, 11: the level in a change is ignored; enabling takes the fader.
            let new = match (v, playback.master.as_ref()) {
                (Master::Group(new), Some(Master::Group(old))) => {
                    let fader = self.states.get(&id).map_or(old.level, |s| s.level);
                    // GRM-20: a new mode puts the master where it changes nothing.
                    let level = if new.mode != old.mode {
                        let neutral = GroupMaster::neutral_level(new.mode);
                        if let Some(state) = self.states.get_mut(&id) {
                            state.level = neutral;
                        }
                        neutral
                    } else if new.enabled && !old.enabled {
                        fader
                    } else {
                        old.level
                    };
                    Master::Group(GroupMaster {
                        level,
                        ..new.clone()
                    })
                }
                _ => v.clone(),
            };
            playback.master = Some(new);
        }
        if let Some(cue) = playback.cues.first_mut() {
            if let Some(v) = change.delay {
                cue.delay = v;
            }
            if let Some(v) = change.fade_in {
                cue.fade_in = v;
            }
            if let Some(v) = change.fade_out {
                cue.fade_out = v;
            }
            if let Some(v) = change.delay_out {
                cue.delay_out = v;
            }
            if let Some(v) = change.overlap {
                cue.overlap = v;
            }
        }
        // GRM-12: Take Over and Disabled have no Flash, Swop or Latch; let go of what is held.
        let no_flash = matches!(&change.master, Some(Master::Group(m)) if !m.enabled || m.mode == GroupMode::TakeOver);
        if let (true, Some(state)) = (no_flash, self.states.get_mut(&id)) {
            (state.flash, state.swop, state.latched) = (false, false, false);
        }
        Ok(())
    }

    /// PBK-31: legend (at most 32 characters), icon id (`None` = automatic) and halo (`None` =
    /// automatic, components 0..=1).
    pub fn set_playback_legend(
        &mut self,
        id: PlaybackId,
        legend: &str,
        icon: Option<&str>,
        halo: Option<TargetColour>,
    ) -> Result<(), CoreError> {
        if !self.playbacks.contains_key(&id) {
            return Err(CoreError::UnknownPlayback(id));
        }
        check_legend(legend)?;
        if let Some(icon) = icon.filter(|i| !palette::valid_icon(i)) {
            return Err(CoreError::InvalidIcon(icon.to_string()));
        }
        if halo.is_some_and(|h| ![h.r, h.g, h.b].iter().all(|c| (0.0..=1.0).contains(c))) {
            return Err(CoreError::InvalidHalo);
        }
        if let Some(playback) = self.playbacks.get_mut(&id) {
            playback.legend = legend.to_string();
            playback.icon = icon.map(str::to_string);
            playback.halo = halo;
        }
        Ok(())
    }

    /// PBK-32: the icon drawn when none was picked, from the first cue; a reference counts with
    /// what it resolves to now.
    pub fn playback_auto_icon(&self, id: PlaybackId) -> Option<palette::AutoIcon> {
        let resolved: Vec<BTreeMap<String, Value>> = (self.playbacks.get(&id)?.values()?)
            .iter()
            .map(|(&item, attrs)| {
                (attrs.iter())
                    .map(|(name, stored)| (name.clone(), self.resolve_stored(item, name, *stored)))
                    .collect()
            })
            .collect();
        let sets: Vec<_> = resolved.iter().map(palette::as_set).collect();
        palette::auto_icon_of(&sets, |name| self.attribute_group(name))
    }

    /// PBK-33: the picked halo, else the first colour of the automatic icon.
    pub fn playback_halo(&self, id: PlaybackId) -> Option<TargetColour> {
        self.playbacks
            .get(&id)?
            .halo
            .or_else(|| match self.playback_auto_icon(id) {
                Some(palette::AutoIcon::Colours(colours)) => colours.first().copied(),
                _ => None,
            })
    }

    // ---- programmer (PRG-01..07) ------------------------------------------

    /// PRG-01/03: toggle a patched fixture in the selection.
    pub fn select_fixture(&mut self, id: FixtureId) -> Result<(), CoreError> {
        self.select_item(Item::fixture(id))
    }

    /// PRG-46: toggle a whole fixture or one of its cells; a cell the mode lacks is refused.
    pub fn select_item(&mut self, item: Item) -> Result<(), CoreError> {
        match self.fixtures.get(&item.fixture) {
            None => return Err(CoreError::UnknownFixture(item.fixture)),
            Some(f) if f.parked => return Err(CoreError::Parked(f.number)), // PAT-30
            Some(_) => {}
        }
        if let Some(cell) = item.cell {
            if cell == 0 || usize::from(cell) > self.cell_count(item.fixture) {
                return Err(CoreError::UnknownCell {
                    fixture: item.fixture,
                    cell,
                });
            }
        }
        self.fan = None; // PRG-56
        self.programmer.toggle(item);
        self.follow_active_group();
        self.select_entry_of_selection();
        Ok(())
    }

    /// PRG-02/03: append a group's fixtures, keeping the ones already selected in place;
    /// a fully selected group is removed instead (IDEA-11). Typed `Group n` replaces, see PRG-08.
    pub fn select_group(&mut self, id: GroupId) -> Result<(), CoreError> {
        let members = self.group_members(id).unwrap_or_default();
        if members.is_empty() {
            // PAT-30: a group of only parked fixtures selects nothing and says so.
            let hung_up = (self.groups.get(&id).into_iter().flatten())
                .any(|i| self.fixtures.get(&i.fixture).is_some_and(|f| f.parked));
            if !hung_up {
                return Err(CoreError::GroupEmpty(id));
            }
            let at = self.last_tick.unwrap_or(0.0);
            self.warnings.push(Warning {
                at,
                message: format!("Group {id}: every fixture is parked"),
            });
            return Ok(());
        }
        self.fan = None; // PRG-56
        self.programmer
            .add_group(&members, self.group_grids.get(&id).copied());
        self.follow_active_group();
        self.select_entry_of_selection();
        self.group_notice(id);
        Ok(())
    }

    /// PRG-96: tell the clients which group was selected, with its legend.
    fn group_notice(&mut self, id: GroupId) {
        let text = self.group_name(id);
        self.notices.push(text);
    }

    /// `Group 6` or `Group 6 Front`.
    fn group_name(&self, id: GroupId) -> String {
        match self.group_legend(id) {
            Some(l) => format!("Group {id} {l}"),
            None => format!("Group {id}"),
        }
    }

    /// GRM-22: a Scale or Limit master whose group is in the patch and whose level, with its
    /// scale, is below full. Disabled counts at its saved level.
    pub fn holding(&self, id: PlaybackId) -> bool {
        let Some(Master::Group(m)) = self.playbacks.get(&id).and_then(|p| p.master.as_ref()) else {
            return false;
        };
        let g = self.group_master_level(id, m);
        let level = match m.mode {
            GroupMode::Scale => g * f32::from(m.scale) / 100.0,
            GroupMode::Limit => g,
            _ => return false,
        };
        level < 1.0 && self.group_members(m.group).is_some_and(|f| !f.is_empty())
    }

    /// GRM-22: the level *g* of a group master, as `GroupMasters::holding` reads it.
    fn group_master_level(&self, id: PlaybackId, m: &GroupMaster) -> f32 {
        let fader = self
            .states
            .get(&id)
            .map_or(m.level, PlaybackState::effective_level);
        if m.enabled {
            fader
        } else {
            m.level
        }
    }

    /// GRM-23: one notice per holding master as a show opens, by handle.
    fn notice_holding_masters(&mut self) {
        let mut held: Vec<_> = self
            .playbacks
            .values()
            .filter(|p| self.holding(p.id))
            .collect();
        held.sort_by_key(|p| (p.page, p.slot));
        let texts: Vec<String> = held
            .iter()
            .filter_map(|p| {
                let Some(Master::Group(m)) = &p.master else {
                    return None;
                };
                let label = if p.legend.is_empty() {
                    "Master"
                } else {
                    &p.legend
                };
                let lock = if m.enabled { "" } else { ", locked" };
                let pct = (self.group_master_level(p.id, m) * 100.0).round();
                Some(format!(
                    "{label} ({}.{}) holds {} at {pct} %{lock}",
                    p.page,
                    p.slot,
                    self.group_name(m.group)
                ))
            })
            .collect();
        self.notices.extend(texts);
    }

    /// PRG-08: a typed selection.
    fn replace_selection(&mut self, list: Vec<Item>) {
        self.fan = None; // PRG-56
        self.programmer.replace_selection(list);
        self.follow_active_group();
        self.select_entry_of_selection();
    }

    /// PRG-79: a typed selection, with the grid of the one group it names.
    fn replace_typed(&mut self, list: Vec<Item>, grid: Option<Grid>) {
        self.replace_selection(list);
        if let Some(grid) = grid {
            self.programmer.restore_grid(grid);
        }
    }

    fn cell_count(&self, id: FixtureId) -> usize {
        self.mode_of(id).map_or(0, |m| m.cells.len())
    }

    /// PRG-49, 52: a group's items as stored, whole fixtures and cells.
    pub fn group_items(&self, id: GroupId) -> Option<Vec<Item>> {
        self.groups.get(&id).cloned()
    }

    /// PRG-53: a group's items that exist now; a cell the mode lacks is kept in the group but
    /// not selected.
    fn group_members(&self, id: GroupId) -> Option<Vec<Item>> {
        // PAT-30: a parked fixture is left out of every live list.
        let exists = |i: &Item| {
            let hangs = self.fixtures.get(&i.fixture).is_some_and(|f| !f.parked);
            hangs
                && match i.cell {
                    None => true,
                    Some(c) => self.mode_of(i.fixture).is_some_and(|m| m.cell(c).is_some()),
                }
        };
        Some(
            self.groups
                .get(&id)?
                .iter()
                .copied()
                .filter(exists)
                .collect(),
        )
    }

    pub fn clear_selection(&mut self) {
        self.fan = None; // PRG-56
        self.programmer.clear_selection();
        self.follow_active_group();
    }

    /// PRG-29, PRG-42: Clear by `clear_precedence` (default: the first Clear deselects, the next
    /// empties the programmer and the located set).
    pub fn clear(&mut self) {
        let selected = !self.programmer.items().is_empty();
        let has_content = !self.programmer.is_empty()
            || self.programmer.has_located()
            || !self.programmer.effects.is_empty();
        match self.settings.clear_precedence {
            ClearPrecedence::SelectionThenProgrammer => {
                if selected {
                    self.clear_selection();
                } else {
                    self.clear_matching(None, 0.0);
                }
            }
            ClearPrecedence::SelectionWithProgrammer => {
                self.clear_selection();
                self.clear_matching(None, 0.0);
            }
            ClearPrecedence::ProgrammerThenSelection => {
                if has_content {
                    self.clear_matching(None, 0.0);
                } else {
                    self.clear_selection();
                }
            }
        }
    }

    /// PRG-29: a masked Clear removes the values and located values in `mask` for all fixtures
    /// and leaves the selection.
    pub fn clear_mask(&mut self, mask: &[AttributeGroup]) -> Result<(), CoreError> {
        self.clear_matching(Some(mask), 0.0);
        Ok(())
    }

    /// ENG-10: Clear with a time (0 = at once); `mask` `None` = everything. The selection stays.
    pub fn clear_timed(
        &mut self,
        time: f32,
        mask: Option<&[AttributeGroup]>,
    ) -> Result<(), CoreError> {
        check_time(time)?;
        self.clear_matching(mask, time);
        Ok(())
    }

    /// Take the values and located values in `mask` (all when `None`) out of the programmer and
    /// let go of them (PRG-43, ENG-10).
    fn clear_matching(&mut self, mask: Option<&[AttributeGroup]>, time: f32) {
        let items = self.programmer.touched_items();
        let gone = self.take_out(&items, |l| mask.is_none_or(|m| m.contains(&l.group)));
        if mask.is_none() {
            self.programmer.clear_values(); // also what no mode can name any more
        }
        self.give_up(gone, time);
        // FX-19: the entries in `mask` go, at once or fading out over `time`.
        let hit = |e: &effect::Entry| mask.is_none_or(|m| m.contains(&e.group));
        if time > 0.0 {
            for e in self.programmer.effects.iter().filter(|e| hit(e)) {
                let fade = Fade {
                    from: Value::Level(0.0),
                    delay: 0.0,
                    duration: time,
                    start: None,
                };
                self.effect_live.entry(e.id).or_default().fade = Some(fade);
            }
        } else {
            self.effects_leave(|e, _| hit(e));
        }
    }

    /// Remove the programmer values and located values of `items` (each one's own, PRG-51) whose
    /// attribute `keep` accepts; what was removed, with its value, is returned for
    /// [`Engine::give_up`].
    fn take_out(&mut self, items: &[Item], keep: impl Fn(&LogicalAttribute) -> bool) -> Vec<Gone> {
        let mut gone = Vec::new();
        for &item in items {
            let Some(mode) = self.mode_of(item.fixture) else {
                continue;
            };
            let values = self.programmer.values_of(item);
            let located = self.programmer.located_of(item);
            let names: BTreeSet<&String> = (values.into_iter().flat_map(|a| a.keys()))
                .chain(located.into_iter().flat_map(|a| a.keys()))
                .collect();
            for name in names {
                let Some(l) = mode.item_logical(item.cell, name).filter(|l| keep(l)) else {
                    continue;
                };
                let value = match self.programmer.stored_item(item, name) {
                    Some(stored) => self.resolve_stored(item, name, stored),
                    None => match self.programmer.located_value(item, name) {
                        Some(v) => v,
                        None => continue,
                    },
                };
                gone.push(Gone {
                    item,
                    name: name.clone(),
                    kind: l.kind,
                    default: l.default,
                    value,
                });
            }
        }
        for g in &gone {
            self.programmer.remove_value(g.item, &g.name);
            self.programmer.remove_located(g.item, &g.name);
        }
        gone
    }

    /// PRG-43, ENG-10: what the removed values output next. An HTP value fades to 0 over `time`
    /// (the playbacks still compete); an LTP one, with `Release`, goes to the default (the merge
    /// lets a playback take it) over `time`; with `Freeze` it keeps its held value.
    /// ponytail: an HTP value fades from its value, not from its output (a palette fade running
    /// at the moment of Clear is not continued).
    fn give_up(&mut self, gone: Vec<Gone>, time: f32) {
        for g in gone {
            let fade = |from| Fade {
                from,
                delay: 0.0,
                duration: time,
                start: None,
            };
            match g.kind {
                AttributeKind::Htp if time > 0.0 => {
                    let fade = fade(g.value);
                    self.programmer.set_clearing(g.item, &g.name, fade);
                }
                AttributeKind::Htp => {}
                AttributeKind::Ltp if self.settings.clear_ltp == ClearLtp::Freeze => {}
                // PRG-51: a cell lets go back to the fixture's value, not to the default.
                // ponytail: with Freeze a cell is not kept at its output; it follows the fixture.
                AttributeKind::Ltp if g.item.cell.is_some() => {
                    let current = held_value(&self.held, g.item, &g.name).unwrap_or(g.value);
                    if time > 0.0 {
                        self.programmer.set_clearing(g.item, &g.name, fade(current));
                    } else if let Some(held) = self.held.get_mut(&g.item) {
                        held.remove(&g.name);
                    }
                }
                AttributeKind::Ltp => {
                    let held = self.held.entry(g.item).or_default();
                    let current = held.get(&g.name).copied().unwrap_or(g.value);
                    if time > 0.0 {
                        let fade = fade(current);
                        self.programmer.set_clearing(g.item, &g.name, fade);
                    } else {
                        held.insert(g.name, g.default);
                    }
                }
            }
        }
    }

    /// PRG-42: all or nothing.
    pub fn set_programmer_settings(
        &mut self,
        change: ProgrammerSettingsChange,
    ) -> Result<(), CoreError> {
        if let Some(level) = change.lowlight_level {
            if !(0.0..=1.0).contains(&level) {
                return Err(CoreError::InvalidLevel(level));
            }
        }
        let s = &mut self.settings;
        s.clear_precedence = change.clear_precedence.unwrap_or(s.clear_precedence);
        s.clear_ltp = change.clear_ltp.unwrap_or(s.clear_ltp);
        s.lowlight_level = change.lowlight_level.unwrap_or(s.lowlight_level);
        Ok(())
    }

    /// PRG-23, 38: put the targets' attributes in `mask` at their locate values. They leave the
    /// programmer (PRG-24); the located values are not contents.
    pub fn locate(&mut self, mask: &[AttributeGroup]) -> Result<(), CoreError> {
        let targets = self.programmer.target_items();
        if targets.is_empty() {
            return Err(CoreError::EmptySelection);
        }
        let mut put = Vec::new();
        for item in targets {
            let Some(mode) = self.mode_of(item.fixture) else {
                continue;
            };
            // PRG-47: an item is located through its own attributes.
            for l in
                (mode.item_attributes(item.cell).into_iter()).filter(|l| mask.contains(&l.group))
            {
                put.push((item, l.name.to_string(), locate_value(mode, &l)));
            }
        }
        for (item, name, value) in put {
            self.programmer.remove_value(item, &name);
            self.programmer.set_located(item, &name, value);
        }
        self.programmer.mark_changed();
        Ok(())
    }

    /// PRG-23: the located set, item → attribute → value.
    pub fn located(&self) -> Values {
        (self.programmer.located().iter())
            .map(|(&id, attrs)| {
                let attrs = attrs.iter().map(|(n, v)| (n.clone(), Stored::Value(*v)));
                (id, attrs.collect())
            })
            .collect()
    }

    /// PRG-26, 39: Highlight; refused with nothing selected.
    pub fn set_highlight(&mut self, on: bool) -> Result<(), CoreError> {
        if on && self.programmer.items().is_empty() {
            return Err(CoreError::EmptySelection);
        }
        self.highlight = on;
        Ok(())
    }

    pub fn highlight(&self) -> bool {
        self.highlight
    }

    /// PRG-56: the Fan key; off → on with the last curve, on → off.
    pub fn fan_key(&mut self) {
        self.fan = match self.fan {
            Some(_) => None,
            None => Some(self.fan_curve),
        };
    }

    /// PRG-56: `Some` turns Fan on with that curve (it becomes the last curve), `None` turns it off.
    pub fn set_fan(&mut self, curve: Option<FanCurve>) {
        self.fan = curve;
        self.fan_curve = curve.unwrap_or(self.fan_curve);
    }

    pub fn fan(&self) -> Option<FanCurve> {
        self.fan
    }

    pub fn fan_curve(&self) -> FanCurve {
        self.fan_curve
    }

    /// PRG-57: the targets that have the wheel's attribute, in target order; a colour-model wheel
    /// means Colour. A cell has Colour or an attribute of its own, and always Dimmer (PRG-47).
    pub fn fan_list(&self, wheel: &str) -> Vec<Item> {
        let name = if self.colour_model.wheels().contains(&wheel) {
            COLOUR
        } else {
            wheel
        };
        (self.programmer.target_items().into_iter())
            .filter(|item| {
                let Some(mode) = self.mode_of(item.fixture) else {
                    return false;
                };
                let Some(n) = item.cell else {
                    return mode.logical(name).is_some();
                };
                let cell = usize::from(n)
                    .checked_sub(1)
                    .and_then(|i| mode.cells.get(i));
                name == DIMMER
                    || cell.is_some_and(|c| {
                        (name == COLOUR && c.colour.is_some())
                            || c.attributes.iter().any(|a| a.name == name)
                    })
            })
            .collect()
    }

    /// PRG-27: Off removes the touched and located values of the targets in `mask`.
    pub fn off(&mut self, mask: &[AttributeGroup]) -> Result<(), CoreError> {
        self.off_where(|l| mask.contains(&l.group))
    }

    /// PRG-40: Off for one attribute; a colour-model wheel name means Colour.
    pub fn off_attribute(&mut self, attribute: &str) -> Result<(), CoreError> {
        const WHEELS: [&str; 8] = [
            "Hue",
            "Saturation",
            "Red",
            "Green",
            "Blue",
            "Cyan",
            "Magenta",
            "Yellow",
        ];
        let name = if WHEELS.contains(&attribute) {
            COLOUR
        } else {
            attribute
        };
        self.off_where(|l| l.name == name)
    }

    fn off_where(&mut self, keep: impl Fn(&LogicalAttribute) -> bool) -> Result<(), CoreError> {
        let targets = self.programmer.target_items();
        if targets.is_empty() {
            return Err(CoreError::EmptySelection);
        }
        let gone = self.take_out(&targets, &keep);
        // FX-19: Off on an attribute takes its effect off too; a path is one effect on Pan and
        // Tilt, so Off on either takes the member out.
        let mut hits: Vec<(EntryId, Item)> = Vec::new();
        for e in &self.programmer.effects {
            for m in e.members.iter().filter(|m| targets.contains(m)) {
                let mode = self.mode_of(m.fixture);
                let off = e.attributes.iter().any(|a| {
                    (mode.and_then(|mode| mode.item_logical(m.cell, a))).is_some_and(|l| keep(&l))
                });
                if off {
                    hits.push((e.id, *m));
                }
            }
        }
        if gone.is_empty() && hits.is_empty() {
            return Err(CoreError::NothingToOff);
        }
        self.give_up(gone, 0.0);
        self.effects_leave(|e, m| hits.contains(&(e.id, *m)));
        Ok(())
    }

    /// PRG-28, 41: Dimmer 0 as a touched value on every patched fixture that is not selected and
    /// has an intensity attribute.
    pub fn rem_dim(&mut self) -> Result<(), CoreError> {
        if self.programmer.items().is_empty() {
            return Err(CoreError::EmptySelection);
        }
        // PRG-54: a fixture with any selected item, a cell included, keeps its intensity.
        let selection: Vec<FixtureId> = self.programmer.items().iter().map(|i| i.fixture).collect();
        let mut put = Vec::new();
        let hanging = self.fixtures.values().filter(|f| !f.parked).map(|f| &f.id);
        for &id in hanging.filter(|id| !selection.contains(id)) {
            let Some(mode) = self.mode_of(id) else {
                continue;
            };
            let intensity = mode
                .logical_attributes()
                .filter(|l| l.group == AttributeGroup::I && l.kind == AttributeKind::Htp);
            put.extend(intensity.map(|l| (id, l.name.to_string())));
        }
        for (id, name) in put {
            self.set_programmer_value(id, &name, 0.0)?;
        }
        self.programmer.mark_changed();
        Ok(())
    }

    /// PRG-04: step to the next fixture (or pattern stage).
    pub fn next_fixture(&mut self) {
        self.programmer.step(true);
        self.follow_active_group();
    }

    pub fn prev_fixture(&mut self) {
        self.programmer.step(false);
        self.follow_active_group();
    }

    /// PRG-05: a pattern of `n` stages is the grid `{groups: n}`.
    pub fn select_pattern(&mut self, n: usize) -> Result<(), CoreError> {
        match u8::try_from(n) {
            Ok(groups) if (2..=32).contains(&groups) => self.set_grid(Grid {
                groups,
                ..Grid::default()
            }),
            _ => Err(CoreError::InvalidPattern(n)),
        }
    }

    /// PRG-73/74: set the whole grid and select stage 0; out of range changes nothing.
    pub fn set_grid(&mut self, grid: Grid) -> Result<(), CoreError> {
        grid.validate()?;
        self.programmer.set_grid(grid);
        self.follow_active_group();
        Ok(())
    }

    /// PRG-75: a grid shortcut; each replaces the whole grid.
    pub fn grid_shortcut(&mut self, shortcut: GridShortcut) -> Result<(), CoreError> {
        let len = self.programmer.items().len();
        if len == 0 {
            return Err(CoreError::SelectFirst);
        }
        let off = Grid::default();
        let bad = |what: String| CoreError::InvalidGrid(format!("grid: {what}"));
        let grid = match shortcut {
            GridShortcut::OddEven => Grid { groups: 2, ..off },
            GridShortcut::OneIn(groups) => Grid { groups, ..off },
            GridShortcut::NInM(n, m) if n < m => Grid {
                groups: m,
                blocks: n,
                ..off
            },
            GridShortcut::NInM(n, m) => return Err(bad(format!("{n} in {m} needs n below m"))),
            GridShortcut::Blocks(blocks) => Grid { blocks, ..off },
            GridShortcut::Divide(d) if (2..=32).contains(&d) => Grid {
                // Too long a selection lands out of range and is refused by `validate`.
                blocks: u8::try_from(len.div_ceil(usize::from(d))).unwrap_or(u8::MAX),
                ..off
            },
            GridShortcut::Divide(d) => {
                return Err(bad(format!("divide {d} out of range (2 to 32)")))
            }
            GridShortcut::OutsideIn => Grid { wings: 2, ..off },
            GridShortcut::InsideOut => Grid {
                wings: 2,
                flip: true,
                ..off
            },
            GridShortcut::Off => off,
        };
        self.set_grid(grid)
    }

    /// PRG-71/72: reorder the selection; the grid and the stage number stay.
    pub fn order(&mut self, tool: Order) -> Result<(), CoreError> {
        let mut items = self.programmer.items().to_vec();
        if items.len() < 2 {
            return Err(CoreError::OrderTooShort);
        }
        match tool {
            Order::Reverse => items.reverse(),
            Order::Sort => items.sort_by_key(|i| {
                let number = self.fixtures.get(&i.fixture).map_or(0, |f| f.number);
                (number, i.cell)
            }),
            Order::Revert => items = self.programmer.select_order().to_vec(),
            Order::Shuffle => items = self.shuffled(items),
        }
        self.programmer.set_order(items);
        self.follow_active_group();
        Ok(())
    }

    /// PRG-72: Fisher–Yates seeded by the shuffle counter; a seed that gives back `current` is
    /// skipped, and after 64 of them two items swap by a rotation.
    fn shuffled(&mut self, current: Vec<Item>) -> Vec<Item> {
        for _ in 0..64 {
            let mut state = self.shuffle_seq;
            self.shuffle_seq = self.shuffle_seq.wrapping_add(1);
            let mut candidate = current.clone();
            for i in (1..candidate.len()).rev() {
                let j = (splitmix64(&mut state) % (i as u64 + 1)) as usize;
                candidate.swap(i, j);
            }
            if candidate != current {
                return candidate;
            }
        }
        let mut rotated = current;
        rotated.rotate_left(1);
        rotated
    }

    /// PRG-06.
    pub fn select_all(&mut self) {
        self.programmer.select_all();
        self.follow_active_group();
    }

    /// PRG-07: store the selection as a group, replacing any existing one.
    pub fn record_group(&mut self, id: GroupId) -> Result<(), CoreError> {
        let items = self.programmer.items().to_vec();
        if items.is_empty() {
            return Err(CoreError::EmptySelection);
        }
        let mut items = items;
        // PAT-33: the group's parked fixtures stay, after the selection, in their old order.
        for old in self.groups.get(&id).into_iter().flatten() {
            let parked = self.fixtures.get(&old.fixture).is_some_and(|f| f.parked);
            if parked && !items.contains(old) {
                items.push(*old);
            }
        }
        self.groups.insert(id, items);
        match self.programmer.grid() {
            Some(grid) => self.group_grids.insert(id, grid),
            None => self.group_grids.remove(&id),
        };
        Ok(())
    }

    /// PRG-97: Record over a group. A used group needs a mode; `Replace` is `record_group`,
    /// `Merge` appends the selected items it does not hold yet and keeps grid and legend.
    pub fn record_group_as(
        &mut self,
        id: GroupId,
        mode: Option<GroupRecordMode>,
    ) -> Result<(), CoreError> {
        if self.programmer.items().is_empty() {
            return Err(CoreError::EmptySelection);
        }
        match (self.groups.get_mut(&id), mode) {
            (None, _) | (Some(_), Some(GroupRecordMode::Replace)) => self.record_group(id),
            (Some(_), None) => Err(CoreError::GroupUsedRecord(id)),
            (Some(items), Some(GroupRecordMode::Merge)) => {
                for item in self.programmer.items() {
                    if !items.contains(item) {
                        items.push(*item);
                    }
                }
                Ok(())
            }
        }
    }

    /// PRG-79: the grid a group was recorded with, `None` when it was recorded with the grid off.
    pub fn group_grid(&self, id: GroupId) -> Option<Grid> {
        self.group_grids.get(&id).copied()
    }

    /// PRG-90: a group's legend, `None` when it has none.
    pub fn group_legend(&self, id: GroupId) -> Option<&str> {
        self.group_legends.get(&id).map(String::as_str)
    }

    /// PRG-85, 86: delete a group; refused while a group master uses it.
    pub fn delete_group(&mut self, id: GroupId) -> Result<(), CoreError> {
        if !self.groups.contains_key(&id) {
            return Err(CoreError::GroupEmpty(id));
        }
        for pb in self.playbacks.values() {
            if matches!(&pb.master, Some(Master::Group(m)) if m.group == id) {
                return Err(CoreError::GroupMastered {
                    group: id,
                    page: pb.page,
                    slot: pb.slot,
                });
            }
        }
        self.groups.remove(&id);
        self.group_grids.remove(&id);
        self.group_legends.remove(&id);
        Ok(())
    }

    /// PRG-89: the refusals of Copy and Move.
    fn check_group_target(&self, id: GroupId, to: GroupId) -> Result<(), CoreError> {
        if !self.groups.contains_key(&id) {
            return Err(CoreError::GroupEmpty(id));
        }
        if to == id {
            return Err(CoreError::GroupOntoItself(id));
        }
        if self.groups.contains_key(&to) {
            return Err(CoreError::GroupUsed(to));
        }
        Ok(())
    }

    /// PRG-87: copy a group (items, grid, legend) to an empty cell; group masters stay.
    pub fn copy_group(&mut self, id: GroupId, to: GroupId) -> Result<(), CoreError> {
        self.check_group_target(id, to)?;
        if let Some(items) = self.groups.get(&id).cloned() {
            self.groups.insert(to, items);
        }
        if let Some(grid) = self.group_grids.get(&id).copied() {
            self.group_grids.insert(to, grid);
        }
        if let Some(legend) = self.group_legends.get(&id).cloned() {
            self.group_legends.insert(to, legend);
        }
        Ok(())
    }

    /// PRG-88: move a group (items, grid, legend) to an empty cell; group masters follow it.
    pub fn move_group(&mut self, id: GroupId, to: GroupId) -> Result<(), CoreError> {
        self.check_group_target(id, to)?;
        if let Some(items) = self.groups.remove(&id) {
            self.groups.insert(to, items);
        }
        if let Some(grid) = self.group_grids.remove(&id) {
            self.group_grids.insert(to, grid);
        }
        if let Some(legend) = self.group_legends.remove(&id) {
            self.group_legends.insert(to, legend);
        }
        for pb in self.playbacks.values_mut() {
            if let Some(Master::Group(m)) = &mut pb.master {
                if m.group == id {
                    m.group = to;
                }
            }
        }
        Ok(())
    }

    /// PRG-98: the sources in group order with the number each lands on. Refuses an empty or
    /// empty-group list and a number past `u32`.
    fn group_block(
        &self,
        groups: &[GroupId],
        to: GroupId,
        layout: Layout,
    ) -> Result<Vec<(GroupId, GroupId)>, CoreError> {
        let src: BTreeSet<_> = groups.iter().copied().collect();
        if let Some(&g) = src.iter().find(|g| !self.groups.contains_key(g)) {
            return Err(CoreError::GroupEmpty(g));
        }
        let lowest = *src
            .first()
            .ok_or_else(|| CoreError::InvalidCommand("expected at least one group".to_string()))?;
        let mut block = Vec::new();
        for (i, &g) in src.iter().enumerate() {
            let offset = match layout {
                Layout::Retain => g - lowest,
                Layout::Bunch => u32::try_from(i).unwrap_or(u32::MAX),
            };
            let target = to
                .checked_add(offset)
                .ok_or_else(|| CoreError::InvalidCommand("group number out of range".into()))?;
            block.push((g, target));
        }
        if block.iter().all(|b| b.0 == b.1) {
            return Err(CoreError::GroupOntoItself(lowest));
        }
        Ok(block)
    }

    /// PRG-98: copy several groups (items, grid, legend) onto empty numbers. All or nothing.
    pub fn copy_groups(
        &mut self,
        groups: &[GroupId],
        to: GroupId,
        layout: Layout,
    ) -> Result<(), CoreError> {
        let block = self.group_block(groups, to, layout)?;
        if let Some(&(_, t)) = block.iter().find(|b| self.groups.contains_key(&b.1)) {
            return Err(CoreError::GroupUsed(t));
        }
        for (g, t) in block {
            self.copy_group(g, t)?;
        }
        Ok(())
    }

    /// PRG-98: move several groups; group masters follow every moved group. With `swap` a
    /// group in the way takes a number the block leaves free. All or nothing.
    pub fn move_groups(
        &mut self,
        groups: &[GroupId],
        to: GroupId,
        layout: Layout,
        swap: bool,
    ) -> Result<(), CoreError> {
        let block = self.group_block(groups, to, layout)?;
        let moving: BTreeSet<_> = block.iter().map(|b| b.0).collect();
        let targets: BTreeSet<_> = block.iter().map(|b| b.1).collect();
        let in_way: Vec<_> = (targets.iter())
            .filter(|t| self.groups.contains_key(t) && !moving.contains(t))
            .copied()
            .collect();
        if !in_way.is_empty() && !swap {
            return Err(CoreError::GroupUsed(in_way.first().copied().unwrap_or(to)));
        }
        let freed = moving.iter().filter(|g| !targets.contains(g)).copied();
        let map: BTreeMap<GroupId, GroupId> = block
            .into_iter()
            .chain(in_way.into_iter().zip(freed))
            .collect();
        let items: Vec<_> = (map.iter())
            .map(|(&g, &t)| {
                (
                    t,
                    self.groups.remove(&g),
                    self.group_grids.remove(&g),
                    self.group_legends.remove(&g),
                )
            })
            .collect();
        for (t, items, grid, legend) in items {
            if let Some(items) = items {
                self.groups.insert(t, items);
            }
            if let Some(grid) = grid {
                self.group_grids.insert(t, grid);
            }
            if let Some(legend) = legend {
                self.group_legends.insert(t, legend);
            }
        }
        for pb in self.playbacks.values_mut() {
            if let Some(Master::Group(m)) = &mut pb.master {
                if let Some(&t) = map.get(&m.group) {
                    m.group = t;
                }
            }
        }
        Ok(())
    }

    /// PRG-98: delete several groups; refused as a whole when one is empty or mastered.
    pub fn delete_groups(&mut self, groups: &[GroupId]) -> Result<(), CoreError> {
        let groups: BTreeSet<_> = groups.iter().copied().collect();
        if groups.is_empty() {
            return Err(CoreError::InvalidCommand(
                "expected at least one group".to_string(),
            ));
        }
        for &g in &groups {
            if !self.groups.contains_key(&g) {
                return Err(CoreError::GroupEmpty(g));
            }
            if let Some(pb) = (self.playbacks.values())
                .find(|pb| matches!(&pb.master, Some(Master::Group(m)) if m.group == g))
            {
                return Err(CoreError::GroupMastered {
                    group: g,
                    page: pb.page,
                    slot: pb.slot,
                });
            }
        }
        for g in groups {
            self.delete_group(g)?;
        }
        Ok(())
    }

    /// PRG-90: set a group's legend; an empty one clears it.
    pub fn set_group_legend(&mut self, id: GroupId, legend: &str) -> Result<(), CoreError> {
        check_legend(legend)?;
        if !self.groups.contains_key(&id) {
            return Err(CoreError::GroupEmpty(id));
        }
        if legend.is_empty() {
            self.group_legends.remove(&id);
        } else {
            self.group_legends.insert(id, legend.to_string());
        }
        Ok(())
    }

    /// Set a manual value; it joins the merge at `Priority::Programmer` (PRG-12/13).
    pub fn set_programmer_value(
        &mut self,
        fixture: FixtureId,
        attribute: &str,
        value: impl Into<Value>,
    ) -> Result<(), CoreError> {
        self.set_item_value(Item::fixture(fixture), attribute, value)
    }

    /// PRG-47: like [`Engine::set_programmer_value`] for a whole fixture or one of its cells; a
    /// cell takes only its own attributes.
    pub fn set_item_value(
        &mut self,
        item: Item,
        attribute: &str,
        value: impl Into<Value>,
    ) -> Result<(), CoreError> {
        let value = value.into();
        let fixture = item.fixture;
        let fx = self
            .fixtures
            .get(&fixture)
            .ok_or(CoreError::UnknownFixture(fixture))?;
        let mode = self
            .mode_of(fixture)
            .ok_or_else(|| CoreError::UnknownProfile(fx.profile.clone()))?;
        let Some(logical) = mode.item_logical(item.cell, attribute) else {
            return Err(CoreError::UnknownAttribute {
                fixture,
                attribute: attribute.to_string(),
            });
        };
        if !logical.default.same_kind(&value) {
            return Err(CoreError::WrongValueKind {
                fixture,
                attribute: attribute.to_string(),
            });
        }
        if let Some(bad) = value.out_of_range() {
            return Err(CoreError::InvalidLevel(bad));
        }
        self.activation_seq = self.activation_seq.saturating_add(1);
        self.programmer
            .set_value(item, attribute, Stored::Value(value), self.activation_seq);
        Ok(())
    }

    // ---- programmer effects (FX-09..11, 14..20, 31; ENG-14..19) -----------

    pub fn effects(&self) -> &[effect::Entry] {
        &self.programmer.effects
    }

    pub fn current_effect(&self) -> Option<EntryId> {
        self.programmer.current
    }

    fn current_entry(&self) -> Option<&effect::Entry> {
        let id = self.programmer.current?;
        self.programmer.effects.iter().find(|e| e.id == id)
    }

    /// FX-17.
    pub fn effect_wheels(&self) -> bool {
        self.effect_wheels
    }

    pub fn set_effect_wheels(&mut self, on: bool) {
        self.effect_wheels = on;
    }

    /// An entry's clock θ_total in cycles (FX-09); live state, 0 until its first tick.
    pub fn effect_theta(&self, id: EntryId) -> Option<f64> {
        self.programmer.effects.iter().find(|e| e.id == id)?;
        Some(self.effect_live.get(&id).map_or(0.0, |l| l.clock.theta()))
    }

    fn no_current_effect() -> CoreError {
        CoreError::InvalidCommand("no current effect".into())
    }

    /// FX-14, 22, 31: make an entry from the targets that have the shape's attribute(s), in
    /// target order; a palette makes one per stored entry and returns the last. Refusals change
    /// nothing.
    pub fn effect_apply(&mut self, apply: &EffectApply) -> Result<EntryId, CoreError> {
        if let EffectApply::Palette(number) = apply {
            let palette = (self.palette(*number)).ok_or(CoreError::UnknownPalette(*number))?;
            let (pid, stored) = (palette.id, palette.effects.clone());
            let mut made = Err(CoreError::InvalidCommand(format!(
                "palette {number} holds no effect"
            )));
            for e in stored {
                let spec = EntrySpec {
                    look: e.look,
                    shape: e.shape,
                    attributes: e.attributes,
                    params: Some(e.params),
                    one_per_position: false,
                    colours: e.colours,
                    palette: Some(pid),
                };
                made = match (self.effect_make(spec), made) {
                    (Ok(id), _) => Ok(id),
                    (Err(_), Ok(id)) => Ok(id),
                    (Err(e), Err(_)) => Err(e),
                };
            }
            return made;
        }
        let (look, shape, attribute, params, one_per_position) = match apply {
            EffectApply::Look(name) => {
                let l = (effect::LOOKS.iter().find(|l| l.name == name))
                    .ok_or_else(|| CoreError::InvalidOption(format!("look '{name}'")))?;
                (
                    Some(l.name.to_string()),
                    l.shape,
                    l.attribute,
                    Some(l.params),
                    l.width_one_per_position,
                )
            }
            EffectApply::Shape { shape, attribute } => {
                (None, *shape, attribute.as_deref(), None, false)
            }
            EffectApply::Palette(_) => return Err(Self::no_current_effect()),
        };
        let attributes = Self::shape_attributes(shape, attribute);
        self.effect_make(EntrySpec {
            look,
            shape,
            attributes,
            params,
            one_per_position,
            colours: Vec::new(),
            palette: None,
        })
    }

    /// FX-14: the attributes a shape drives (a wave: `attribute`, else Dimmer).
    fn shape_attributes(shape: effect::Shape, attribute: Option<&str>) -> Vec<String> {
        use effect::Shape;
        match shape {
            Shape::Path(_) => vec!["Pan".into(), "Tilt".into()],
            Shape::Colour(_) => vec![COLOUR.into()],
            Shape::Wave(_) => vec![attribute.unwrap_or(DIMMER).into()],
        }
    }

    /// FX-37: 1-based ids of the looks at least one target has the attribute(s) for (the FX-14
    /// test of `effect_make`), in catalogue order.
    fn runnable_looks(&self, targets: &[Item]) -> Vec<usize> {
        (effect::LOOKS.iter().enumerate())
            .filter(|(_, l)| {
                let attrs = Self::shape_attributes(l.shape, l.attribute);
                targets.iter().any(|i| {
                    self.mode_of(i.fixture)
                        .is_some_and(|m| attrs.iter().any(|a| m.item_logical(i.cell, a).is_some()))
                })
            })
            .map(|(n, _)| n + 1)
            .collect()
    }

    /// FX-42: a Hue shift whose first member's base colour is white (saturation ~0) shows nothing.
    fn white_base(&self, e: &effect::Entry) -> bool {
        use effect::{ColourShape, Shape};
        if e.shape != Shape::Colour(ColourShape::HueShift) {
            return false;
        }
        let Some(item) = e.members.first().copied() else {
            return false;
        };
        let Some(l) = (self.mode_of(item.fixture)).and_then(|m| m.item_logical(item.cell, COLOUR))
        else {
            return false;
        };
        match self.start_value(item, &l) {
            Value::Colour(c) => c.to_hsv().1 < 0.01,
            Value::Level(_) => false,
        }
    }

    /// FX-42: a non-intensity effect whose first member's intensity in its base is 0 shows nothing.
    fn dark(&self, e: &effect::Entry) -> bool {
        if e.group == AttributeGroup::I {
            return false;
        }
        let Some(item) = e.members.first().copied() else {
            return false;
        };
        let Some(l) = (self.mode_of(item.fixture)).and_then(|m| m.item_logical(item.cell, DIMMER))
        else {
            return false;
        };
        matches!(self.start_value(item, &l), Value::Level(v) if v < 0.001)
    }

    /// FX-14: the entry of `spec` on the targets that have its attributes.
    fn effect_make(&mut self, spec: EntrySpec) -> Result<EntryId, CoreError> {
        use effect::{ColourShape, Shape};
        let EntrySpec {
            look,
            shape,
            attributes,
            params,
            one_per_position,
            mut colours,
            palette,
        } = spec;
        let label = attributes.join("/");
        let targets = self.programmer.target_items();
        if targets.is_empty() {
            return Err(CoreError::EmptySelection);
        }
        let logical = |item: &Item| {
            let mode = self.mode_of(item.fixture)?;
            attributes
                .iter()
                .find_map(|a| mode.item_logical(item.cell, a))
        };
        let members: Vec<Item> = targets
            .iter()
            .filter(|i| logical(i).is_some())
            .copied()
            .collect();
        let Some((first_item, first)) = members.first().and_then(|i| Some((*i, logical(i)?)))
        else {
            return Err(CoreError::AttributeNotInTargets(label));
        };
        let mut params = params.unwrap_or(effect::Params {
            mode: effect::default_mode(shape, first.kind == AttributeKind::Htp),
            ..effect::Params::DEFAULT
        });
        // FX-14: a ColourMix runs the first member's colour and its complement.
        if shape == Shape::Colour(ColourShape::ColourMix) && colours.is_empty() {
            let base = match self.start_value(first_item, &first) {
                Value::Colour(c) => c,
                Value::Level(_) => TargetColour::WHITE,
            };
            colours = vec![base, effect::hue_shift(base, 0.5)];
        }
        let group = first.group;
        let id = self.effect_next_id;
        let mut entry = effect::Entry {
            id,
            look,
            shape,
            attributes,
            params,
            members,
            positions: Vec::new(),
            count: 0,
            seed: u64::from(id),
            colours,
            palette,
            group,
        };
        // PRG-77: a look or shape (no effect palette) over a grid spreads by the grid's positions.
        if entry.palette.is_none() && self.programmer.stage().is_none() {
            if let Some(g) = self.programmer.grid() {
                let p = &mut entry.params;
                (p.wings, p.groups, p.blocks, p.flip) = (g.wings, g.groups, g.blocks, g.flip);
                params = *p;
            }
        }
        entry.refresh();
        if one_per_position && entry.count > 0 {
            params.width = 1.0 / entry.count as f32; // FX-30
            entry.params = params;
        }
        self.effect_next_id = id.saturating_add(1);
        // One programmer effect per fixture and attribute: the members leave their other entries.
        let (members, names) = (entry.members.clone(), entry.attributes.clone());
        self.effects_leave(|e, m| {
            e.attributes.iter().any(|a| names.contains(a)) && members.contains(m)
        });
        self.programmer.effects.push(entry);
        self.programmer.current = Some(id);
        self.programmer.mark_changed(); // PRG-03
        Ok(id)
    }

    /// Take the members `hit` accepts out of their entries; an entry left empty goes. `true`
    /// when a member went.
    fn effects_leave(&mut self, hit: impl Fn(&effect::Entry, &Item) -> bool) -> bool {
        let mut any = false;
        for e in &mut self.programmer.effects {
            let mut members = std::mem::take(&mut e.members);
            let before = members.len();
            members.retain(|m| !hit(e, m));
            any |= members.len() != before;
            e.members = members;
            e.refresh();
        }
        let p = &mut self.programmer;
        p.effects.retain(|e| !e.members.is_empty());
        if p.current
            .is_some_and(|c| p.effects.iter().all(|e| e.id != c))
        {
            p.current = None;
        }
        any
    }

    /// FX-16: change a parameter of the current entry from the next tick. Out of range is
    /// refused and changes nothing; positions follow.
    pub fn effect_set(&mut self, set: EffectSet) -> Result<(), CoreError> {
        let id = self
            .programmer
            .current
            .ok_or_else(Self::no_current_effect)?;
        let entry = (self.programmer.effects.iter_mut().find(|e| e.id == id))
            .ok_or_else(Self::no_current_effect)?;
        // FX-16: a pointer copies the palette's parameters first, then lets go of it.
        let mut p = pointed(&self.palettes, entry);
        match set {
            EffectSet::Size(v) => p.size = v,
            EffectSet::Top(v) => p.top = v,
            EffectSet::Bpm(v) => p.bpm = v,
            EffectSet::Beats(v) => p.beats = v,
            EffectSet::Spread(v) => p.spread = v,
            EffectSet::Phase(v) => p.phase = v,
            EffectSet::Width(v) => p.width = v,
            EffectSet::Crossfade(v) => p.crossfade = v,
            EffectSet::SpeedSource(v) => p.speed_source = v,
            EffectSet::Direction(v) => p.direction = v,
            EffectSet::Mode(v) => p.mode = v,
            EffectSet::Wings(v) => p.wings = v,
            EffectSet::Groups(v) => p.groups = v,
            EffectSet::Blocks(v) => p.blocks = v,
            EffectSet::Flip(v) => p.flip = v,
            EffectSet::Order(tool) => {
                // FX-45: reorder the members; the parameters, palette and clock stay.
                let mut members = entry.members.clone();
                if tool == Order::Revert {
                    return Err(CoreError::InvalidOption(
                        "order Revert (expected Reverse, Shuffle or Sort)".into(),
                    ));
                }
                if members.len() < 2 {
                    return Err(CoreError::OrderTooShort);
                }
                match tool {
                    Order::Sort => members.sort_by_key(|i| {
                        let number = self.fixtures.get(&i.fixture).map_or(0, |f| f.number);
                        (number, i.cell)
                    }),
                    Order::Shuffle => members = self.shuffled(members),
                    Order::Reverse | Order::Revert => members.reverse(),
                }
                if let Some(entry) = self.programmer.effects.iter_mut().find(|e| e.id == id) {
                    entry.members = members;
                    entry.refresh();
                }
                return Ok(());
            }
            EffectSet::Colours(list) => {
                let mix = entry.shape == effect::Shape::Colour(effect::ColourShape::ColourMix);
                let ok = |c: &TargetColour| [c.r, c.g, c.b].iter().all(|v| (0.0..=1.0).contains(v));
                if !mix || !(2..=8).contains(&list.len()) || !list.iter().all(ok) {
                    return Err(CoreError::InvalidOption(format!(
                        "colours (expected 2 to 8 colours on a colour mix, got {})",
                        list.len()
                    )));
                }
                p.validate()?;
                entry.colours = list;
                entry.params = p;
                entry.palette = None;
                entry.refresh();
                return Ok(());
            }
        }
        p.validate()?;
        entry.params = p;
        entry.palette = None;
        entry.refresh();
        Ok(())
    }

    /// FX-18: make another entry current.
    pub fn effect_select(&mut self, id: EntryId) -> Result<(), CoreError> {
        if self.programmer.effects.iter().all(|e| e.id != id) {
            return Err(CoreError::InvalidCommand(format!("no effect {id}")));
        }
        self.programmer.current = Some(id);
        Ok(())
    }

    /// FX-18: selecting exactly the members of one entry makes it current.
    fn select_entry_of_selection(&mut self) {
        let items = self.programmer.items();
        if let Some(e) = self.programmer.effects.iter().find(|e| e.members == items) {
            self.programmer.current = Some(e.id);
        }
    }

    /// FX-19: take the targets out of every entry, or only those in `group`.
    pub fn effect_stop(&mut self, group: Option<AttributeGroup>) -> Result<(), CoreError> {
        let targets = self.programmer.target_items();
        if targets.is_empty() {
            return Err(CoreError::EmptySelection);
        }
        let hit =
            |e: &effect::Entry, m: &Item| group.is_none_or(|g| e.group == g) && targets.contains(m);
        if !self.effects_leave(hit) {
            return Err(CoreError::InvalidCommand(
                "no effect on the selection".into(),
            ));
        }
        Ok(())
    }

    /// FX-11: every programmer clock to 0.
    pub fn effect_restart(&mut self) {
        for live in self.effect_live.values_mut() {
            live.clock.restart();
        }
    }

    /// FX-10: tap the current entry's tempo. `Free` times the taps (CUE-33) and sets the entry's
    /// `bpm`, beat-syncing from the second tap; `Bpm n` taps that master.
    pub fn effect_tap(&mut self, at: Timestamp) -> Result<(), CoreError> {
        if !at.is_finite() {
            return Err(CoreError::InvalidCommand(format!("tap time {at}")));
        }
        let (id, params) = (self.current_entry())
            .map(|e| (e.id, pointed(&self.palettes, e)))
            .ok_or_else(Self::no_current_effect)?;
        if let effect::SpeedSource::Bpm(n) = params.speed_source {
            if let Some((master, _)) = self.find_master(MasterKind::Bpm, n) {
                return self.tap(master, at);
            }
        }
        let live = self.effect_live.entry(id).or_default();
        let Some(bpm) = tap_bpm(&mut live.taps, at) else {
            return Ok(());
        };
        live.clock.beat_sync(params.beats);
        if let Some(e) = self.programmer.effects.iter_mut().find(|e| e.id == id) {
            e.params = pointed(&self.palettes, e);
            e.params.bpm = bpm;
            e.palette = None;
        }
        Ok(())
    }

    /// FX-17: a wheel of the effect layout acts on the current entry; `None` when the wheels are
    /// off, no entry is current or `name` is not one of them.
    fn turn_effect_wheel(&mut self, name: &str, steps: i32) -> Option<Result<(), CoreError>> {
        if !self.effect_wheels {
            return None;
        }
        let p = pointed(&self.palettes, self.current_entry()?);
        let n = steps as f32;
        let set = match name {
            "Size" => EffectSet::Size((p.size + n * 0.01).clamp(0.0, 1.0)),
            "Top" => EffectSet::Top((p.top + n * 0.01).clamp(0.0, 1.0)),
            "Width" => EffectSet::Width((p.width + n * 0.01).clamp(0.01, 1.0)),
            "Crossfade" => EffectSet::Crossfade((p.crossfade + n * 0.01).clamp(0.0, 1.0)),
            "Speed" => EffectSet::Bpm((p.bpm + n).clamp(0.0, 600.0)),
            "Spread" => EffectSet::Spread((p.spread + n * 15.0).clamp(0.0, 1440.0)),
            "Phase" => EffectSet::Phase((p.phase + n * 15.0).clamp(0.0, 360.0)),
            "Wings" => EffectSet::Wings((i32::from(p.wings) + steps).clamp(1, 8) as u8),
            "Beats" => {
                let at = effect::BEATS
                    .iter()
                    .position(|b| *b == p.beats)
                    .unwrap_or(2);
                let to = (at as i64 + i64::from(steps)).clamp(0, effect::BEATS.len() as i64 - 1);
                EffectSet::Beats(effect::BEATS.get(to as usize).copied().unwrap_or(p.beats))
            }
            _ => return None,
        };
        Some(self.effect_set(set))
    }

    /// FX-09, 19: before the merge, advance every entry's clock by Δt · rate (an entry's first
    /// tick does not advance it), start the fades of a timed Clear and drop the finished ones.
    /// Allocates only when an entry gets its first clock.
    fn tick_effects(&mut self, now: Timestamp) {
        for i in 0..self.programmer.effects.len() {
            let Some(e) = self.programmer.effects.get(i) else {
                continue;
            };
            let (id, params) = (e.id, pointed(&self.palettes, e));
            let source = speed_source(params.speed_source);
            let rate = effect::rate(self.speed(params.bpm, source), params.beats);
            let live = self.effect_live.entry(id).or_default();
            if let Some(last) = live.last {
                live.clock.advance(now - last, rate);
            }
            live.last = Some(now);
            if let Some(fade) = &mut live.fade {
                fade.start.get_or_insert(now);
            }
        }
        let live = &self.effect_live;
        let done = |e: &effect::Entry| {
            (live.get(&e.id).and_then(|l| l.fade.as_ref())).is_some_and(|f| f.progress(now) >= 1.0)
        };
        let p = &mut self.programmer;
        p.effects.retain(|e| !done(e));
        if p.current
            .is_some_and(|c| p.effects.iter().all(|e| e.id != c))
        {
            p.current = None;
        }
        let effects = &p.effects;
        self.effect_live
            .retain(|id, _| effects.iter().any(|e| e.id == *id));
    }

    /// PAT-26: the runs of every playback follow the cue state as it is now (a fixture came or
    /// went); not per tick.
    fn follow_cue_effects(&mut self) {
        let mut fx = std::mem::take(&mut self.cue_fx);
        for (id, f) in &mut fx {
            let Some(playback) = self.playbacks.get(id) else {
                continue;
            };
            let Some(i) = cue_index(playback, self.states.get(id)) else {
                continue;
            };
            if f.cue.is_some() && playback.cues.get(i).map(|c| c.number) == f.cue {
                f.follow(&playback.effect_state(i));
            }
        }
        self.cue_fx = fx;
    }

    /// FX-24, 26: before the merge, build the runs of every active playback's current cue when
    /// the cue changed, move the clocks of all of them by Δt · rate and drop what has faded out.
    /// Allocates only when a cue starts. ponytail: an edit to the running cue's entries shows at
    /// its next start; a patch change (PAT-26) and a Remove (CUE-59, 60) are followed; a Back starts with the cue's delay.
    fn tick_cue_effects(&mut self, now: Timestamp) {
        let mut fx = std::mem::take(&mut self.cue_fx);
        fx.retain(|id, _| self.states.get(id).is_some_and(PlaybackState::is_active));
        for (id, playback) in &self.playbacks {
            let Some(activated) = (self.states.get(id)).and_then(|s| s.activated) else {
                continue;
            };
            let at = cue_index(playback, self.states.get(id));
            let Some(cue) = at.and_then(|i| playback.cues.get(i)) else {
                continue;
            };
            let known = fx
                .get(id)
                .is_some_and(|f| f.activated == activated && f.cue == Some(cue.number));
            if known {
                continue;
            }
            let current = fx.entry(*id).or_default();
            if current.activated != activated {
                *current = CueFx::default();
            }
            let incoming = playback.effect_state(at.unwrap_or(0));
            // The first cue of a non-list is the cue; its delay holds the start (FX-24).
            current.start(incoming, cue, &self.palettes, now);
            current.activated = activated;
        }
        for f in fx.values_mut() {
            for r in &mut f.runs {
                let p = r.params(&self.palettes, now);
                let rate = effect::rate(self.speed(p.bpm, speed_source(p.speed_source)), p.beats);
                let from = r.last.max(r.start);
                if now > from {
                    r.clock.advance(now - from, rate);
                }
                r.last = now;
                let over = |(start, fade): (Timestamp, f32)| (now - start) as f32 >= fade;
                if r.blend.is_some_and(|(_, s, f)| over((s, f))) {
                    r.blend = None;
                }
                if !r.leaving && r.ramp.is_some_and(over) {
                    r.ramp = None;
                }
            }
            f.runs
                .retain(|r| !(r.leaving && r.ramp.is_some_and(|(s, f)| (now - s) as f32 >= f)));
        }
        self.cue_fx = fx;
    }

    /// PRG-18: the attribute group the wheels show; always accepted.
    pub fn active_group(&self) -> AttributeGroup {
        self.active_group
    }

    pub fn set_active_group(&mut self, group: AttributeGroup) {
        self.active_group = group;
        self.effect_wheels = false; // FX-17: an attribute-group pick takes the wheels back
    }

    /// PRG-18: after a selection change, leave a group no target has for the
    /// first one (I P C G B E S) some target has. Stored, not computed lazily.
    fn follow_active_group(&mut self) {
        if self.programmer.items().is_empty() {
            self.highlight = false; // PRG-26: deselecting stops Highlight
        }
        let targets = self.programmer.target_items();
        let has = |g: AttributeGroup| {
            targets.iter().any(|i| {
                self.mode_of(i.fixture)
                    .is_some_and(|m| m.item_attributes(i.cell).iter().any(|l| l.group == g))
            })
        };
        if targets.is_empty() || has(self.active_group) {
            return;
        }
        use AttributeGroup::*;
        if let Some(g) = [I, P, C, G, B, E, S].into_iter().find(|g| has(*g)) {
            self.active_group = g;
        }
    }

    /// The mode a fixture is patched in; `None` when the fixture or its profile is gone.
    fn mode_of(&self, id: FixtureId) -> Option<&Mode> {
        self.fixtures
            .get(&id)
            .and_then(|f| resolve_mode(&self.profiles, f))
    }

    /// PRG-17: names of the active group's attributes that at least one
    /// target has, in target order then profile order. Colour is shown as the
    /// colour model's wheels, ahead of the group's other attributes.
    pub fn wheel_layout(&self) -> Vec<String> {
        // FX-17: while the effect wheels are on and an entry is current, they are the layout.
        if let Some(e) = self.current_entry().filter(|_| self.effect_wheels) {
            let width = if e.shape == effect::Shape::Colour(effect::ColourShape::ColourMix) {
                "Crossfade"
            } else {
                "Width"
            };
            let names = [
                "Size", "Speed", "Spread", "Phase", width, "Top", "Beats", "Wings",
            ];
            return names.iter().map(|n| n.to_string()).collect();
        }
        let mut names: Vec<String> = Vec::new();
        let mut colour = false;
        for item in self.programmer.target_items() {
            let attributes =
                (self.mode_of(item.fixture).into_iter()).flat_map(|m| m.item_attributes(item.cell));
            for l in attributes {
                if l.group != self.active_group {
                    continue;
                }
                if l.is_colour() {
                    colour = true;
                } else if !names.iter().any(|n| n == l.name) {
                    names.push(l.name.to_string());
                }
            }
        }
        if colour {
            let wheels = self.colour_model.wheels().iter().map(|w| w.to_string());
            names.splice(0..0, wheels);
        }
        names
    }

    /// PRG-19..21, 36: move `attribute` of every target that has it by
    /// `steps` wheel steps. A wheel of the current colour model moves Colour
    /// (PRG-32). Refused turns change nothing.
    pub fn turn_wheel(
        &mut self,
        attribute: &str,
        steps: i32,
        resolution: Resolution,
    ) -> Result<(), CoreError> {
        if let Some(turned) = self.turn_effect_wheel(attribute, steps) {
            return turned;
        }
        let targets = self.programmer.target_items();
        if targets.is_empty() {
            return Err(CoreError::EmptySelection);
        }
        let step = match resolution {
            Resolution::Normal => self.settings.wheel_step_normal,
            Resolution::Fine => self.settings.wheel_step_fine,
            Resolution::Coarse => self.settings.wheel_step_coarse,
        };
        let delta = steps as f32 * step;
        let colour_wheel = self.colour_model.wheels().contains(&attribute);
        let name = if colour_wheel { COLOUR } else { attribute };
        // PRG-59: with Fan on and two or more items, each moves by its own share of `delta`.
        let list = self.fan_list(attribute);
        // PRG-76: the share follows the item's position k in the list, over N positions.
        let (ks, n) = self.programmer.list_positions(list.len());
        let shares: Vec<(Item, f32)> = match self.fan {
            Some(curve) if n >= 2 => {
                let factors = fan_factors(curve, n as usize);
                (list.iter().zip(&ks))
                    .map(|(item, &k)| {
                        let share = factors.get(k as usize).copied().unwrap_or(0.0);
                        (*item, delta * share)
                    })
                    .collect()
            }
            _ => targets.into_iter().map(|item| (item, delta)).collect(),
        };
        let mut new = Vec::new();
        for (item, delta) in shares {
            let Some(l) =
                (self.mode_of(item.fixture)).and_then(|m| m.item_logical(item.cell, name))
            else {
                continue;
            };
            let moved = match (colour_wheel, self.start_value(item, &l)) {
                (true, Value::Colour(c)) => self
                    .colour_model
                    .turn(attribute, c, delta)
                    .map(Value::Colour),
                (false, Value::Level(v)) => Some(Value::Level((v + delta).clamp(0.0, 1.0))),
                _ => None,
            };
            if let Some(value) = moved {
                new.push((item, value));
            }
        }
        if new.is_empty() {
            return Err(CoreError::AttributeNotInTargets(attribute.to_string()));
        }
        for (item, value) in new {
            self.set_item_value(item, name, value)?;
        }
        Ok(())
    }

    /// IPC-39: put an absolute level on `attribute` of every target that has it. Refused
    /// requests change nothing; Colour is set through [`Engine::set_colour`].
    pub fn set_programmer_level(&mut self, attribute: &str, value: f32) -> Result<(), CoreError> {
        if !(0.0..=1.0).contains(&value) {
            return Err(CoreError::InvalidLevel(value));
        }
        if attribute == COLOUR {
            return Err(CoreError::InvalidOption(
                "level: Colour takes a hue and a saturation".into(),
            ));
        }
        let targets = self.programmer.target_items();
        if targets.is_empty() {
            return Err(CoreError::EmptySelection);
        }
        let with: Vec<Item> = (targets.into_iter())
            .filter(|i| {
                self.mode_of(i.fixture)
                    .is_some_and(|m| m.item_logical(i.cell, attribute).is_some())
            })
            .collect();
        if with.is_empty() {
            return Err(CoreError::AttributeNotInTargets(attribute.to_string()));
        }
        for item in with {
            self.set_item_value(item, attribute, Value::Level(value))?;
        }
        Ok(())
    }

    /// IPC-23: set Colour of every target that has it to `hue`/`saturation` (0.0-1.0), keeping
    /// each one's brightest component like the HSI wheels (PRG-31, PRG-32).
    pub fn set_colour(&mut self, hue: f32, saturation: f32) -> Result<(), CoreError> {
        if let Some(bad) = [hue, saturation]
            .into_iter()
            .find(|x| !(0.0..=1.0).contains(x))
        {
            return Err(CoreError::InvalidLevel(bad));
        }
        let targets = self.programmer.target_items();
        if targets.is_empty() {
            return Err(CoreError::EmptySelection);
        }
        let mut new = Vec::new();
        for item in targets {
            let mode = self.mode_of(item.fixture);
            if let Some(l) = mode.and_then(|m| m.item_logical(item.cell, COLOUR)) {
                if let Value::Colour(c) = self.start_value(item, &l) {
                    let (_, _, v) = c.to_hsv();
                    new.push((item, TargetColour::from_hsv(hue, saturation, v)));
                }
            }
        }
        if new.is_empty() {
            return Err(CoreError::AttributeNotInTargets(COLOUR.to_string()));
        }
        for (item, c) in new {
            self.set_item_value(item, COLOUR, c)?;
        }
        Ok(())
    }

    /// PRG-65/66: move the target colour along the locus by a CTO/CTB step (mired), keeping its
    /// brightest component; black starts from 6500 K at full. Result clamps to 1800..=10000 K.
    pub fn ct_step(&mut self, step: CtStep) -> Result<(), CoreError> {
        let targets = self.programmer.target_items();
        if targets.is_empty() {
            return Err(CoreError::EmptySelection);
        }
        let mut new = Vec::new();
        for item in targets {
            let mode = self.mode_of(item.fixture);
            if let Some(l) = mode.and_then(|m| m.item_logical(item.cell, COLOUR)) {
                if let Value::Colour(c) = self.start_value(item, &l) {
                    let k = colour::kelvin(c).unwrap_or(6500.0);
                    let k = (1e6 / (1e6 / k + step.mired())).clamp(1800.0, 10000.0);
                    let max = c.r.max(c.g).max(c.b);
                    let scale = if max > 0.0 { max } else { 1.0 };
                    new.push((item, colour::planck(k).scaled(scale)));
                }
            }
        }
        if new.is_empty() {
            return Err(CoreError::AttributeNotInTargets(COLOUR.to_string()));
        }
        for (item, c) in new {
            self.set_item_value(item, COLOUR, c)?;
        }
        Ok(())
    }

    /// PRG-67: replace the gel table (tests and the daemon; the default is LEE).
    pub fn set_gels(&mut self, gels: GelTable) {
        self.gels = gels;
    }

    /// PRG-68: set the target colour to gel `number`, keeping its brightest component (black
    /// comes up at full). Fixtures without Colour are skipped.
    pub fn gel(&mut self, number: u32) -> Result<(), CoreError> {
        let targets = self.programmer.target_items();
        self.apply_gel(self.gel_plan(number, &targets)?)
    }

    /// PRG-68: what `gel` would set on `targets`; refuses without changing anything.
    fn gel_plan(
        &self,
        number: u32,
        targets: &[Item],
    ) -> Result<Vec<(Item, TargetColour)>, CoreError> {
        let gel = self
            .gels
            .colour(number)
            .ok_or(CoreError::UnknownGel(number))?;
        if targets.is_empty() {
            return Err(CoreError::EmptySelection);
        }
        let mut new = Vec::new();
        for &item in targets {
            let mode = self.mode_of(item.fixture);
            if let Some(l) = mode.and_then(|m| m.item_logical(item.cell, COLOUR)) {
                if let Value::Colour(c) = self.start_value(item, &l) {
                    let max = c.r.max(c.g).max(c.b);
                    new.push((item, gel.scaled(if max > 0.0 { max } else { 1.0 })));
                }
            }
        }
        if new.is_empty() {
            return Err(CoreError::AttributeNotInTargets(COLOUR.to_string()));
        }
        Ok(new)
    }

    fn apply_gel(&mut self, plan: Vec<(Item, TargetColour)>) -> Result<(), CoreError> {
        for (item, c) in plan {
            self.set_item_value(item, COLOUR, c)?;
        }
        Ok(())
    }

    /// PRG-20: programmer value, else what is on the output, else the held value. PRG-47: a
    /// cell reads its own merged output and held value; the virtual cell Dimmer starts full.
    fn start_value(&self, item: Item, l: &LogicalAttribute) -> Value {
        let id = item.fixture;
        let own = match self.programmer.stored_item(item, l.name) {
            Some(Stored::Value(v)) => Some(v),
            _ => None,
        };
        own.or_else(|| {
            let (fixture, mode) = (self.fixtures.get(&id)?, self.mode_of(id)?);
            merge_attribute(
                &MergeSources {
                    playbacks: &self.playbacks,
                    states: &self.states,
                    programmer: &self.programmer,
                    palettes: &self.palettes,
                    swop_owner: self.swop_owner,
                },
                fixture,
                mode,
                l,
                self.last_tick.unwrap_or_default(),
                item.cell,
                None,
            )
            .map(|c| c.value)
        })
        .or_else(|| held_value(&self.held, item, l.name))
        .unwrap_or(match l.kind {
            AttributeKind::Htp if item.cell.is_some() => l.default,
            AttributeKind::Htp => Value::Level(0.0),
            AttributeKind::Ltp => l.default,
        })
    }

    /// PRG-35 / IPC-11: the programmer state for rendering.
    pub fn programmer_view(&self) -> ProgrammerView {
        use AttributeGroup::*;
        // PRG-47, PRG-55: wheels, groups, the picker and `targets` read the target items.
        let items = self.programmer.target_items();
        let targets = items.clone();
        let attribute_groups = [I, P, C, G, B, E, S, Fx]
            .into_iter()
            .map(|group| AttributeGroupState {
                group,
                available: items.iter().any(|i| {
                    self.mode_of(i.fixture)
                        .is_some_and(|m| m.item_attributes(i.cell).iter().any(|l| l.group == group))
                }),
                has_values: items.iter().any(|&i| {
                    let values = self.programmer.values_of(i);
                    values.zip(self.mode_of(i.fixture)).is_some_and(|(v, m)| {
                        v.keys()
                            .any(|n| m.item_logical(i.cell, n).is_some_and(|l| l.group == group))
                    })
                }),
            })
            .collect();
        // Per wheel: the first target that has the attribute speaks for all.
        let current = self.current_entry().filter(|_| self.effect_wheels);
        let wheels = self
            .wheel_layout()
            .into_iter()
            .filter_map(|attribute| {
                // FX-33: an effect wheel shows its parameter as a fraction of the FX-06 range.
                if let Some(e) = current {
                    let p = pointed(&self.palettes, e);
                    let beats = effect::BEATS.iter().position(|b| *b == p.beats);
                    let value = match attribute.as_str() {
                        "Size" => p.size,
                        "Top" => p.top,
                        "Width" => p.width,
                        "Crossfade" => p.crossfade,
                        "Speed" => p.bpm / 600.0,
                        "Spread" => p.spread / 1440.0,
                        "Phase" => p.phase / 360.0,
                        "Beats" => beats.unwrap_or(2) as f32 / 6.0,
                        "Wings" => f32::from(p.wings.saturating_sub(1)) / 7.0,
                        _ => 0.0,
                    };
                    return Some(WheelState {
                        attribute,
                        value,
                        touched: true,
                        sixteen_bit: false,
                        palette: None,
                        auto: false,
                    });
                }
                let colour_wheel = self.colour_model.wheels().contains(&attribute.as_str());
                let name = if colour_wheel {
                    COLOUR
                } else {
                    attribute.as_str()
                };
                let (item, l) = items.iter().find_map(|&i| {
                    Some((i, self.mode_of(i.fixture)?.item_logical(i.cell, name)?))
                })?;
                let start = self.start_value(item, &l);
                let value = match start {
                    Value::Colour(c) => self.colour_model.wheel_value(&attribute, c)?,
                    Value::Level(v) => v,
                };
                let stored = self.programmer.stored_item(item, name);
                // IPC-46: the frame writer mixes this emitter on some target (ENG-21).
                let auto = l.emitter
                    && items.iter().any(|i| {
                        let Some(mode) = self.mode_of(i.fixture).filter(|m| m.automix) else {
                            return false;
                        };
                        let defs = match i.cell {
                            None => Some(&mode.attributes),
                            Some(c) => mode.cell(c).map(|c| &c.attributes),
                        };
                        defs.is_some_and(|d| {
                            d.iter().any(|a| {
                                a.emitter
                                    && a.name == name
                                    && a.cie.and_then(colour::rgb_equivalent).is_some()
                            })
                        })
                    });
                Some(WheelState {
                    auto,
                    value,
                    touched: stored.is_some(),
                    palette: match stored {
                        Some(Stored::Reference(r)) => Some(r.palette),
                        _ => None,
                    },
                    sixteen_bit: l.sixteen_bit,
                    attribute,
                })
            })
            .collect();
        // IPC-22: the first target that has Colour speaks for the picker.
        let colour = items.iter().find_map(|&i| {
            let l = self.mode_of(i.fixture)?.item_logical(i.cell, COLOUR)?;
            let Value::Colour(c) = self.start_value(i, &l) else {
                return None;
            };
            let (hue, saturation, _) = c.to_hsv();
            Some(ColourView {
                hue,
                saturation,
                r: c.r,
                g: c.g,
                b: c.b,
                kelvin: colour::kelvin(c).map(|k| (k / 10.0).round() * 10.0),
                gel: self.gels.reading(c).map(|g| GelRef {
                    range: "Lee",
                    number: g.number,
                    name: g.name.clone(),
                }),
            })
        });
        let palettes = self.palette_use(&items);
        ProgrammerView {
            selection: self.programmer.items().to_vec(),
            targets,
            active_group: self.active_group,
            attribute_groups,
            wheels,
            in_programmer: self.programmer.values().keys().copied().collect(),
            colour_model: self.colour_model,
            contents: (self.programmer.values().iter())
                .map(|(&item, values)| FixtureValues {
                    fixture: item,
                    values: values
                        .iter()
                        .map(|(name, stored)| {
                            (name.clone(), self.resolve_stored(item, name, *stored))
                        })
                        .collect(),
                    palettes: values
                        .iter()
                        .filter_map(|(name, stored)| match stored {
                            Stored::Reference(r) => Some((name.clone(), r.palette)),
                            Stored::Value(_) => None,
                        })
                        .collect(),
                    fading: values
                        .keys()
                        .filter_map(|name| {
                            let fade = self.programmer.fade(item, name)?;
                            Some((
                                name.clone(),
                                fade.progress(self.last_tick.unwrap_or_default()),
                            ))
                        })
                        .collect(),
                })
                .collect(),
            colour,
            palettes,
            located: (self.programmer.located().iter())
                .map(|(&i, attrs)| (i, attrs.keys().cloned().collect()))
                .collect(),
            highlight: self.highlight,
            fan: self.fan,
            fan_curve: self.fan_curve,
            grid: self.programmer.grid(),
            stage: self.programmer.stage(),
            positions: self.programmer.positions().to_vec(),
            position_count: self.programmer.position_count(),
            effects: (self.programmer.effects.iter())
                .map(|e| EffectView {
                    id: e.id,
                    look: e.look.clone(),
                    shape: e.shape,
                    attributes: e.attributes.clone(),
                    members: e.members.clone(),
                    positions: (e.positions.iter())
                        .map(|&k| {
                            if e.count == 0 {
                                0.0
                            } else {
                                k as f32 / e.count as f32
                            }
                        })
                        .collect(),
                    params: pointed(&self.palettes, e),
                    palette: e.palette,
                    current: self.programmer.current == Some(e.id),
                    colours: e.colours.clone(),
                    white_base: self.white_base(e),
                    dark: self.dark(e),
                })
                .collect(),
            effect_wheels: self.effect_wheels,
            runnable_looks: self.runnable_looks(&items),
        }
    }

    /// PAL-26: palettes referenced in the programmer, and those that resolve something for the
    /// targets (all of them when nothing is selected).
    fn palette_use(&self, targets: &[Item]) -> PaletteUse {
        let mut in_use: Vec<PaletteId> = (self.programmer.values().values())
            .flat_map(|attrs| attrs.values())
            .filter_map(|s| match s {
                Stored::Reference(r) => Some(r.palette),
                Stored::Value(_) => None,
            })
            .collect();
        in_use.sort_unstable();
        in_use.dedup();
        let relevant = (self.palettes.values())
            .filter(|p| {
                // FX-22: an effect palette counts; FX-31 refuses it on apply when nobody fits.
                targets.is_empty()
                    || !p.effects.is_empty()
                    || self.palette_plan(p.number, targets).is_ok()
            })
            .map(|p| p.id)
            .collect();
        PaletteUse { in_use, relevant }
    }

    /// PRG-30: how Colour is shown on the wheels; the colour itself does not change.
    pub fn colour_model(&self) -> ColourModel {
        self.colour_model
    }

    pub fn set_colour_model(&mut self, model: ColourModel) {
        self.colour_model = model;
    }

    /// PRG-15: the programmer's manual values whose attribute is in `mask`.
    pub fn contents(&self, mask: &[AttributeGroup]) -> Values {
        let mut out = Values::new();
        for (item, attrs) in self.programmer.values() {
            let Some(mode) = self.mode_of(item.fixture) else {
                continue;
            };
            for (name, value) in attrs {
                let in_mask = mode.item_logical(item.cell, name);
                if in_mask.is_some_and(|l| mask.contains(&l.group)) {
                    out.entry(*item).or_default().insert(name.clone(), *value);
                }
            }
        }
        out
    }

    /// PRG-08/09/11: run one command line. Parse and resolve first, then apply,
    /// so a refused line changes nothing.
    pub fn execute_command(&mut self, tokens: &[&str]) -> Result<(), CoreError> {
        let cmd = command::parse(tokens)?;
        let group = single_group(&cmd);
        self.run_command(cmd, group)?;
        if let Some(id) = group {
            self.group_notice(id); // PRG-96: only when the whole line went through
        }
        Ok(())
    }

    fn run_command(
        &mut self,
        cmd: command::Command,
        group: Option<GroupId>,
    ) -> Result<(), CoreError> {
        let typed = if cmd.selection.is_empty() {
            None
        } else {
            // PRG-48: `.`-first forms read the selection as it is before this line applies.
            let mut selected: Vec<FixtureId> = Vec::new();
            for i in self.programmer.items() {
                if !selected.contains(&i.fixture) {
                    selected.push(i.fixture);
                }
            }
            let mut list: Vec<Item> = Vec::new();
            for (not, item) in &cmd.selection {
                let ids = self.expand_items(item, &selected);
                if *not {
                    list.retain(|f| !ids.contains(f));
                } else {
                    for id in ids {
                        if !list.contains(&id) {
                            list.push(id);
                        }
                    }
                }
            }
            if list.is_empty() {
                // PAT-30: a line naming one parked fixture is refused with its number.
                if let [(false, one)] = &cmd.selection[..] {
                    let parked = (one.kind == Kind::Fixture && one.from == one.to)
                        .then(|| self.numbers.get(&one.from))
                        .flatten()
                        .and_then(|id| self.fixtures.get(id))
                        .filter(|f| f.parked);
                    if let Some(f) = parked {
                        return Err(CoreError::Parked(f.number));
                    }
                }
                // PAT-30: a group of only parked fixtures selects nothing and warns.
                if let Some(g) = group.filter(|g| self.groups.get(g).is_some_and(|m| !m.is_empty()))
                {
                    return self.select_group(g);
                }
                // PRG-96: a line naming one group says so when that group has no fixtures.
                return Err(group.map_or(CoreError::EmptySelection, CoreError::GroupEmpty));
            }
            Some(list)
        };
        // PRG-79: `Group n` alone brings the group's grid back.
        let typed_grid = group.and_then(|g| self.group_grids.get(&g).copied());
        if let Some(number) = cmd.palette {
            // PAL-10: plan against the typed selection first, so a refusal changes nothing.
            let (time, overlap) = self.palette_timing(Timing::default())?;
            let targets = typed
                .clone()
                .unwrap_or_else(|| self.programmer.target_items());
            // FX-22: an effect-only palette resolves no value and is no misfit.
            let effects = self.palette(number).is_some_and(|p| !p.effects.is_empty());
            let plan = match self.palette_plan(number, &targets) {
                Err(CoreError::PaletteDoesNotFit(_)) if effects => None,
                plan => Some(plan?),
            };
            if let Some(list) = typed {
                self.replace_typed(list, typed_grid);
            }
            let values = plan.is_some();
            if let Some((id, plan)) = plan {
                self.put_palette_timed(id, plan, time, overlap);
            }
            if effects {
                let made = self.effect_apply(&EffectApply::Palette(number));
                if !values {
                    made?;
                }
            }
            return Ok(());
        }
        if let Some(number) = cmd.gel {
            // PRG-68: plan against the typed selection first, so a refusal changes nothing.
            let targets = typed
                .clone()
                .unwrap_or_else(|| self.programmer.target_items());
            let plan = self.gel_plan(number, &targets)?;
            if let Some(list) = typed {
                self.replace_typed(list, typed_grid);
            }
            return self.apply_gel(plan);
        }
        if cmd.locate {
            // PRG-10: select, then Locate every group. `typed` is never empty here.
            if let Some(list) = typed {
                self.replace_typed(list, typed_grid);
            }
            return self.locate(&ALL_GROUPS);
        }
        let mut touched = Vec::new();
        let level = cmd.level.map(|v| (v / 100.0).clamp(0.0, 1.0));
        if level.is_some() {
            let targets = typed
                .clone()
                .unwrap_or_else(|| self.programmer.target_items());
            if typed.is_none() && targets.is_empty() {
                return Err(CoreError::EmptySelection);
            }
            // PRG-47: `901 . 2 @ 50` sets cell 2's Dimmer, not the master.
            for item in targets {
                let mode = self.mode_of(item.fixture);
                for l in mode.into_iter().flat_map(|m| m.item_attributes(item.cell)) {
                    if l.group == AttributeGroup::I && l.kind == AttributeKind::Htp {
                        touched.push((item, l.name.to_string()));
                    }
                }
            }
        }
        if let Some(list) = typed {
            self.replace_typed(list, typed_grid);
        }
        if let Some(level) = level {
            for (item, name) in touched {
                self.set_item_value(item, &name, level)?;
            }
        }
        Ok(())
    }

    /// Patched fixtures (user numbers, PAT-12) or members of existing groups (ids) named by one
    /// item, in typed order. Ranges walk only the entries that exist, so `1 Thro 4000000000`
    /// stays cheap.
    fn expand_items(&self, item: &command::Item, selected: &[FixtureId]) -> Vec<Item> {
        let (lo, hi) = (item.from.min(item.to), item.from.max(item.to));
        let desc = item.from > item.to;
        let ordered = |mut keys: Vec<u32>| {
            if desc {
                keys.reverse();
            }
            keys
        };
        let fixtures: Vec<FixtureId> = match item.kind {
            Kind::Selected => selected.to_vec(),
            Kind::Fixture => ordered(
                (self.numbers.range(lo..=hi))
                    .map(|(_, id)| *id)
                    .filter(|id| self.fixtures.get(id).is_some_and(|f| !f.parked)) // PAT-30
                    .collect(),
            ),
            Kind::Group => {
                let ids = ordered(self.groups.range(lo..=hi).map(|(id, _)| *id).collect());
                return ids
                    .iter()
                    .filter_map(|&g| self.group_members(g))
                    .flatten()
                    .collect();
            }
        };
        let Some(cells) = item.cells else {
            return fixtures.into_iter().map(Item::fixture).collect();
        };
        let mut out = Vec::new();
        for id in fixtures {
            let n = u32::try_from(self.cell_count(id)).unwrap_or(0);
            let (from, to) = match cells {
                Cells::All => (1, n),
                Cells::Range { from, to } => (from, to),
            };
            let mut range: Vec<u32> = (from.min(to).max(1)..=from.max(to).min(n)).collect();
            if from > to {
                range.reverse();
            }
            out.extend(
                range
                    .into_iter()
                    .filter_map(|c| u16::try_from(c).ok())
                    .map(|c| Item::cell(id, c)),
            );
        }
        out
    }

    // ---- palettes (PAL-01..12, 17..21) --------------------------------------

    /// PAL-23, scenario files and tests: place a palette as it is. Refused (nothing changes) for
    /// an id or number in use, a number outside 1–9999 or a cell in use.
    pub fn add_palette(&mut self, mut palette: Palette) -> Result<(), CoreError> {
        if palette.id == 0 || self.palettes.contains_key(&palette.id) {
            return Err(CoreError::DuplicatePalette(palette.id));
        }
        check_palette_number(palette.number)?;
        if self.palette(palette.number).is_some() {
            return Err(CoreError::PaletteNumberInUse(palette.number));
        }
        self.check_free_cell(palette.cell())?;
        self.palette_next_id = self.palette_next_id.max(palette.id.saturating_add(1));
        self.settle_entries(&mut palette.effects);
        self.palettes.insert(palette.id, palette);
        Ok(())
    }

    /// FX-29: what a saved entry leaves out: its group and positions, and the entry ids stay
    /// below the counter.
    fn settle_entries(&mut self, entries: &mut [effect::Entry]) {
        for e in entries {
            if let Some(group) = e.attributes.first().map(|a| self.attribute_group(a)) {
                e.group = group;
            }
            e.refresh();
            self.effect_next_id = self.effect_next_id.max(e.id.saturating_add(1));
        }
    }

    /// FX-21: the programmer entries of the mask, cut to the fixtures that are patched, a pointer
    /// holding the parameters it reads now.
    fn recorded_effects(&self, mask: &[AttributeGroup]) -> Vec<effect::Entry> {
        let hit = self
            .programmer
            .effects
            .iter()
            .filter(|e| mask.contains(&e.group));
        hit.filter_map(|e| {
            let mut e = e.clone();
            e.params = pointed(&self.palettes, &e);
            e.drop_members(|m| self.mode_of(m.fixture).is_none());
            e.refresh();
            (!e.members.is_empty()).then_some(e)
        })
        .collect()
    }

    /// PAL-01: record the programmer's contents in the mask onto a cell. An empty cell gets a
    /// new palette; a used one needs `mode` (PAL-06). Refused records change nothing.
    pub fn record_palette(&mut self, record: &Record) -> Result<PaletteId, CoreError> {
        self.record_palette_with(record, false)
    }

    /// `only_held` (PBK-49): a used palette takes only what it resolves for that fixture, and no
    /// effects.
    fn record_palette_with(
        &mut self,
        record: &Record,
        only_held: bool,
    ) -> Result<PaletteId, CoreError> {
        let cell = record.cell;
        let existing = self
            .palettes
            .values()
            .find(|p| p.cell() == cell)
            .map(|p| (p.id, p.number, p.kind()));
        let number = match existing {
            Some((_, number, _)) => {
                if record.mode.is_none() {
                    return Err(CoreError::PaletteUsed(number));
                }
                number
            }
            None => {
                self.check_free_cell(cell)?;
                match record.number {
                    Some(n) => {
                        check_palette_number(n)?;
                        if self.palette(n).is_some() {
                            return Err(CoreError::PaletteNumberInUse(n));
                        }
                        n
                    }
                    None => self.next_free_palette_number()?,
                }
            }
        };
        let mask = record
            .mask
            .clone()
            .unwrap_or_else(|| cell.window.mask().to_vec());
        let mut values = self.record_values(&mask);
        // FX-22: the entries of the mask, without members, a pointer as its parameters.
        let effects: Vec<effect::Entry> = (self.recorded_effects(&mask))
            .into_iter()
            .filter(|_| !only_held)
            .map(|mut e| {
                e.palette = None;
                e.members.clear();
                e.refresh();
                e
            })
            .collect();
        if record.record_by.unwrap_or(self.settings.record_by) == RecordBy::Mixed {
            self.mix_groups(&mut values);
        }
        let mode = existing.and(record.mode);
        if let (Some((id, ..)), Some(RecordMode::QuickMerge)) = (existing, mode) {
            if let Some(palette) = self.palettes.get(&id) {
                for (item, attrs) in values.iter_mut() {
                    attrs.retain(|name, _| {
                        if only_held {
                            self.palette_resolves(palette, *item, name)
                        } else {
                            palette.holds(name)
                        }
                    });
                }
                values.retain(|_, attrs| !attrs.is_empty());
            }
        }
        if values.is_empty() && effects.is_empty() {
            let letters: Vec<&str> = mask.iter().map(|g| palette::letter(*g)).collect();
            return Err(CoreError::NothingToRecord(format!(
                "{:?} (mask {})",
                cell.window,
                letters.join("")
            )));
        }
        let mut limit = Limit::from_record_as(record.record_as);
        if let (Some((_, _, kind)), Some(RecordMode::Merge | RecordMode::QuickMerge)) =
            (existing, mode)
        {
            limit = limit.capped_by(kind);
        }
        let detected = {
            let entries: BTreeMap<Item, Entry> = values
                .into_iter()
                .filter_map(|(item, values)| {
                    let fixture = self.fixtures.get(&item.fixture)?;
                    let mode = self.mode_of(item.fixture)?;
                    let profile = fixture.profile.as_str();
                    Some((
                        item,
                        Entry {
                            profile,
                            mode,
                            values,
                        },
                    ))
                })
                .collect();
            palette::detect(&entries, limit)?
        };
        match existing {
            Some((id, ..)) => {
                if let Some(palette) = self.palettes.get_mut(&id) {
                    match mode {
                        Some(RecordMode::Replace) | None => {
                            palette.global = detected.global;
                            palette.shared = detected.shared;
                            palette.shared_cells = detected.shared_cells;
                            // PAT-33: a parked fixture's value stays.
                            let kept: Vec<_> = (palette.fixtures.iter())
                                .filter(|(i, _)| {
                                    (self.fixtures.get(&i.fixture)).is_some_and(|f| f.parked)
                                })
                                .map(|(i, v)| (*i, v.clone()))
                                .collect();
                            palette.fixtures = detected.fixtures;
                            for (item, attrs) in kept {
                                palette.fixtures.entry(item).or_insert(attrs);
                            }
                            palette.effects = effects;
                        }
                        Some(_) => {
                            palette.merge(detected);
                            let named = |o: &effect::Entry| {
                                effects
                                    .iter()
                                    .any(|e| e.attributes.iter().any(|a| o.attributes.contains(a)))
                            };
                            palette.effects.retain(|o| !named(o));
                            palette.effects.extend(effects);
                        }
                    }
                }
                Ok(id)
            }
            None => {
                let id = self.palette_next_id;
                self.palette_next_id = id.saturating_add(1);
                let Detected {
                    global,
                    shared,
                    shared_cells,
                    fixtures,
                } = detected;
                let mut palette = Palette {
                    effects,
                    id,
                    number,
                    window: cell.window,
                    handle: cell.handle,
                    legend: String::new(),
                    icon: None,
                    global,
                    shared,
                    shared_cells,
                    fixtures,
                };
                palette.legend = self.gel_legend(&palette);
                self.palettes.insert(id, palette);
                Ok(id)
            }
        }
    }

    /// PAL-33: `L<number> <name>` (32 characters at most) when every colour in `palette` reads
    /// as the same gel, else empty.
    fn gel_legend(&self, palette: &Palette) -> String {
        let global = palette.global.values().filter_map(|v| match v {
            GlobalValue::Value(v) => Some(v),
            GlobalValue::Degrees { .. } => None,
        });
        let shared = palette.shared.values().flat_map(|a| a.values());
        let cells = palette
            .shared_cells
            .values()
            .flat_map(|c| c.values().flat_map(|a| a.values()));
        let fixtures = palette.fixtures.values().flat_map(|a| a.values());
        let mut gel = None;
        for value in global.chain(shared).chain(cells).chain(fixtures) {
            let Value::Colour(c) = value else { continue };
            match (self.gels.reading(*c), gel) {
                (None, _) => return String::new(),
                (Some(g), None) => gel = Some(g),
                (Some(g), Some(first)) if g.number != first.number => return String::new(),
                _ => {}
            }
        }
        gel.map(|g| {
            format!("L{} {}", g.number, g.name)
                .chars()
                .take(32)
                .collect()
        })
        .unwrap_or_default()
    }

    /// PAL-01, 03: the programmer's values in the mask as plain values; a reference is
    /// recorded as the value it resolves to now.
    fn record_values(&self, mask: &[AttributeGroup]) -> BTreeMap<Item, BTreeMap<String, Value>> {
        self.contents(mask)
            .into_iter()
            .map(|(item, attrs)| {
                let values = attrs
                    .into_iter()
                    .map(|(name, stored)| {
                        let value = self.resolve_stored(item, &name, stored);
                        (name, value)
                    })
                    .collect();
                (item, values)
            })
            .collect()
    }

    /// PAL-02 Mixed: a fixture with a touched attribute in group P (or C) also stores the rest
    /// of that group, untouched ones at their current output value (PRG-20). An item takes the
    /// rest of the group from its own attributes (a cell: its cell's, PRG-47).
    fn mix_groups(&self, values: &mut BTreeMap<Item, BTreeMap<String, Value>>) {
        for (&item, attrs) in values.iter_mut() {
            let Some(mode) = self.mode_of(item.fixture) else {
                continue;
            };
            let own = mode.item_attributes(item.cell);
            let whole: Vec<&LogicalAttribute> = [AttributeGroup::P, AttributeGroup::C]
                .into_iter()
                .filter(|g| {
                    attrs
                        .keys()
                        .any(|n| own.iter().any(|l| l.name == n.as_str() && l.group == *g))
                })
                .flat_map(|g| own.iter().filter(move |l| l.group == g))
                .collect();
            for l in whole {
                if !attrs.contains_key(l.name) {
                    attrs.insert(l.name.to_string(), self.start_value(item, l));
                }
            }
        }
    }

    /// PAL-17: the lowest unused number.
    fn next_free_palette_number(&self) -> Result<u32, CoreError> {
        (1..=palette::MAX_NUMBER)
            .find(|n| self.palette(*n).is_none())
            .ok_or(CoreError::NoFreePaletteNumbers)
    }

    /// PAL-17: handles start at 1 and hold one palette.
    fn check_free_cell(&self, cell: Cell) -> Result<(), CoreError> {
        let window = format!("{:?}", cell.window);
        let handle = cell.handle;
        if handle == 0 {
            Err(CoreError::InvalidPaletteCell { window, handle })
        } else if self.palettes.values().any(|p| p.cell() == cell) {
            Err(CoreError::PaletteCellInUse { window, handle })
        } else {
            Ok(())
        }
    }

    /// PAL-08/09: the values palette `number` resolves for the targets, or for every patched
    /// fixture in user-number order when there are none (quick palette). Nothing is changed.
    fn palette_plan(
        &self,
        number: u32,
        targets: &[Item],
    ) -> Result<(PaletteId, PalettePlan), CoreError> {
        let palette = self
            .palette(number)
            .ok_or(CoreError::UnknownPalette(number))?;
        let all: Vec<Item>;
        let list = if targets.is_empty() {
            // PRG-52: the cells the palette holds values for itself come with the fixtures.
            let cells = palette.fixtures.keys().filter(|i| i.cell.is_some());
            all = (self.numbers.values().map(|&id| Item::fixture(id)))
                .chain(cells.copied())
                .collect();
            &all
        } else {
            targets
        };
        let mut plan = Vec::new();
        for &item in list {
            let (Some(fixture), Some(mode)) =
                (self.fixtures.get(&item.fixture), self.mode_of(item.fixture))
            else {
                continue;
            };
            // PRG-47, 52: an item takes the palette through its own attributes, a cell with its
            // own values first.
            for l in mode.item_attributes(item.cell) {
                if let Some(v) = palette
                    .resolve(item, fixture, mode, l.name)
                    .filter(|v| v.same_kind(&l.default))
                {
                    plan.push((item, l.name.to_string(), v));
                }
            }
        }
        if plan.is_empty() {
            return Err(CoreError::PaletteDoesNotFit(
                format!("{} {}", palette.number, palette.legend)
                    .trim()
                    .to_string(),
            ));
        }
        Ok((palette.id, plan))
    }

    /// PAL-08/12: put a plan into the programmer as references (with the value as fallback)
    /// or as manual values. Counts as a programmer change (PRG-03).
    fn put_palette(&mut self, palette: PaletteId, plan: PalettePlan, as_reference: bool) {
        self.activation_seq = self.activation_seq.saturating_add(1);
        for (item, name, value) in plan {
            let stored = if as_reference {
                Stored::Reference(Reference { palette, value })
            } else {
                Stored::Value(value)
            };
            self.programmer
                .set_value(item, &name, stored, self.activation_seq);
        }
    }

    /// PAL-08/09: apply palette `number` to the targets (selection or sub-selection), or as a
    /// quick palette when nothing is selected. Refused when it resolves nothing for any target.
    /// PAL-13..15: with a time > 0 each attribute fades from its current output (the fixtures
    /// staggered by the overlap); the programmer holds the reference from the start.
    pub fn apply_palette(&mut self, number: u32, timing: Timing) -> Result<(), CoreError> {
        let (time, overlap) = self.palette_timing(timing)?;
        let effects = self.palette(number).is_some_and(|p| !p.effects.is_empty());
        let apply = EffectApply::Palette(number);
        match self.palette_plan(number, &self.programmer.target_items()) {
            // FX-22: an effect-only palette resolves no value and is no misfit.
            Err(CoreError::PaletteDoesNotFit(_)) if effects => self.effect_apply(&apply).map(drop),
            Err(e) => Err(e),
            Ok((id, plan)) => {
                self.put_palette_timed(id, plan, time, overlap);
                if effects {
                    let _ = self.effect_apply(&apply); // no target with its attributes: values only
                }
                Ok(())
            }
        }
    }

    /// PAL-15: the time and overlap of an apply: typed, else the master; refused out of range.
    fn palette_timing(&self, timing: Timing) -> Result<(f32, f32), CoreError> {
        let time = timing.time.unwrap_or(self.settings.palette_time);
        let overlap = timing.overlap.unwrap_or(self.settings.palette_overlap);
        check_palette_timing(time, overlap)?;
        Ok((time, overlap))
    }

    /// PAL-13/14: put the plan in the programmer as references, with a fade per attribute when
    /// `time` > 0. The fade's `from` is the output before the references go in.
    fn put_palette_timed(&mut self, id: PaletteId, plan: PalettePlan, time: f32, overlap: f32) {
        let mut fades = Vec::new();
        if time > 0.0 {
            let step = time * (1.0 - overlap / 100.0);
            let mut distinct: Vec<Item> = Vec::new();
            for (item, _, _) in &plan {
                if distinct.last() != Some(item) {
                    distinct.push(*item);
                }
            }
            // PRG-78: a selection's positions order the fade; a quick palette keeps k = i.
            let ks = if self.programmer.items().is_empty() {
                (0..distinct.len() as u32).collect()
            } else {
                self.programmer.list_positions(distinct.len()).0
            };
            let mut index = 0;
            let mut previous = None;
            for (item, name, _) in &plan {
                if previous.is_some_and(|p| p != *item) {
                    index += 1;
                }
                previous = Some(*item);
                let k = ks.get(index).copied().unwrap_or(0) as f32;
                let mode = self.mode_of(item.fixture);
                let Some(l) = mode.and_then(|m| m.item_logical(item.cell, name)) else {
                    continue;
                };
                fades.push((
                    *item,
                    name.clone(),
                    Fade {
                        from: self.start_value(*item, &l),
                        delay: k * step,
                        duration: time,
                        start: None,
                    },
                ));
            }
        }
        self.put_palette(id, plan, true);
        for (item, name, fade) in fades {
            self.programmer.set_fade(item, &name, fade);
        }
    }

    /// PAL-15: set the master fade time (s) and/or overlap (%); `None` keeps the current value.
    /// Out of range refuses both.
    pub fn set_palette_master(
        &mut self,
        time: Option<f32>,
        overlap: Option<f32>,
    ) -> Result<(), CoreError> {
        let time = time.unwrap_or(self.settings.palette_time);
        let overlap = overlap.unwrap_or(self.settings.palette_overlap);
        check_palette_timing(time, overlap)?;
        self.settings.palette_time = time;
        self.settings.palette_overlap = overlap;
        Ok(())
    }

    /// PAL-12: like [`Engine::apply_palette`], but the programmer gets manual values.
    pub fn include_palette(&mut self, number: u32) -> Result<(), CoreError> {
        let (id, plan) = self.palette_plan(number, &self.programmer.target_items())?;
        self.put_palette(id, plan, false);
        Ok(())
    }

    /// PBK-48: put a cue's own (hard) values and effect entries in the programmer, with their
    /// fixtures selected. Tracked and off (dormant) values stay out. Refused, changing nothing,
    /// when nothing is left in the mask.
    pub fn include_playback(
        &mut self,
        id: PlaybackId,
        cue: Option<CueNumber>,
        mask: Option<&[AttributeGroup]>,
    ) -> Result<(), CoreError> {
        let mask = mask.unwrap_or(&ALL_GROUPS);
        let playback = (self.playbacks.get(&id)).ok_or(CoreError::UnknownPlayback(id))?;
        let at = match cue {
            Some(n) => playback.cues.iter().position(|c| c.number == n),
            None if is_list(playback) => {
                let current = self.states.get(&id).and_then(|s| s.run.current);
                let Some(n) = current else {
                    let name = if playback.legend.is_empty() {
                        let (page, slot) = (playback.page, playback.slot);
                        format!("{page}.{slot}")
                    } else {
                        playback.legend.clone()
                    };
                    return Err(CoreError::NameACue(name));
                };
                playback.cues.iter().position(|c| c.number == n)
            }
            None => (!playback.cues.is_empty()).then_some(0),
        };
        let held = at.and_then(|i| playback.cues.get(i));
        let Some(held) = held else {
            return Err(match cue {
                Some(n) => CoreError::UnknownCue(n),
                None => CoreError::ListRefused("The playback holds no cue to include".into()),
            });
        };
        let mut values = Values::new();
        for (item, attrs) in &held.values {
            let Some(mode) = self.mode_of(item.fixture) else {
                continue;
            };
            for (name, stored) in attrs {
                let in_mask = mode.item_logical(item.cell, name);
                if in_mask.is_some_and(|l| mask.contains(&l.group)) {
                    values
                        .entry(*item)
                        .or_default()
                        .insert(name.clone(), *stored);
                }
            }
        }
        let mut effects: Vec<effect::Entry> = Vec::new();
        if mask.contains(&AttributeGroup::Fx) {
            for entry in &held.effects {
                let mut e = entry.clone();
                e.drop_members(|m| self.mode_of(m.fixture).is_none());
                e.refresh();
                if !e.members.is_empty() {
                    effects.push(e);
                }
            }
        }
        if values.is_empty() && effects.is_empty() {
            return Err(CoreError::ListRefused(format!(
                "Q{} holds nothing to include",
                held.number
            )));
        }
        let mut items: Vec<Item> = values.keys().copied().collect();
        items.extend(effects.iter().flat_map(|e| e.members.iter().copied()));
        // The cue's order; what it does not rank goes after, by fixture number.
        items.sort_by_key(|i| {
            let rank = held.order.get(i).copied().unwrap_or(u32::MAX);
            let number = self.fixtures.get(&i.fixture).map(|f| f.number);
            (rank, number, *i)
        });
        items.dedup();
        self.programmer.replace_selection(items);
        self.activation_seq = self.activation_seq.saturating_add(1);
        for (item, attrs) in values {
            for (name, stored) in attrs {
                self.programmer
                    .set_value(item, &name, stored, self.activation_seq);
            }
        }
        for mut e in effects {
            let id = self.effect_next_id;
            self.effect_next_id = id.saturating_add(1);
            e.id = id;
            let (members, names) = (e.members.clone(), e.attributes.clone());
            self.effects_leave(|o, m| {
                o.attributes.iter().any(|a| names.contains(a)) && members.contains(m)
            });
            self.programmer.effects.push(e);
            self.programmer.current = Some(id);
            self.programmer.mark_changed(); // PRG-03
        }
        Ok(())
    }

    /// PAL-17: a new palette (new id, next free number) on an empty cell with the same values,
    /// legend and icon.
    pub fn copy_palette(&mut self, number: u32, to: Cell) -> Result<PaletteId, CoreError> {
        let source = self
            .palette(number)
            .ok_or(CoreError::UnknownPalette(number))?
            .clone();
        self.check_free_cell(to)?;
        let new_number = self.next_free_palette_number()?;
        let id = self.palette_next_id;
        self.palette_next_id = id.saturating_add(1);
        self.palettes.insert(
            id,
            Palette {
                id,
                number: new_number,
                window: to.window,
                handle: to.handle,
                ..source
            },
        );
        Ok(id)
    }

    /// PAL-34: the sources in cell order with the cell each lands on. Refuses an empty or
    /// unknown list, `Retain` over several windows and handle 0.
    fn palette_block(
        &self,
        numbers: &[u32],
        to: Cell,
        layout: Layout,
    ) -> Result<Vec<(PaletteId, Cell)>, CoreError> {
        let mut src = Vec::new();
        for &n in numbers {
            let p = self.palette(n).ok_or(CoreError::UnknownPalette(n))?;
            src.push((p.cell(), p.id));
        }
        src.sort_unstable();
        src.dedup_by_key(|s| s.1);
        let first = src.first().ok_or_else(|| {
            CoreError::InvalidCommand("expected at least one palette".to_string())
        })?;
        let windows = src
            .iter()
            .map(|s| s.0.window)
            .collect::<BTreeSet<_>>()
            .len();
        if layout == Layout::Retain && windows > 1 {
            return Err(CoreError::SourcesInWindows(windows));
        }
        let mut block = Vec::new();
        for (i, &(cell, id)) in src.iter().enumerate() {
            let offset = match layout {
                Layout::Retain => cell.handle.saturating_sub(first.0.handle),
                Layout::Bunch => u16::try_from(i).unwrap_or(u16::MAX),
            };
            let target = Cell::new(to.window, to.handle.saturating_add(offset));
            if target.handle == 0 || target.handle == u16::MAX {
                self.check_free_cell(Cell::new(target.window, 0))?; // InvalidPaletteCell
            }
            block.push((id, target));
        }
        Ok(block)
    }

    /// PAL-34: copy several palettes onto empty cells (new ids and numbers, in cell order).
    /// All or nothing.
    pub fn copy_palettes(
        &mut self,
        numbers: &[u32],
        to: Cell,
        layout: Layout,
    ) -> Result<Vec<PaletteId>, CoreError> {
        let block = self.palette_block(numbers, to, layout)?;
        for &(_, cell) in &block {
            self.check_free_cell(cell)?;
        }
        let next = self.palette_next_id;
        let mut new = Vec::new();
        for (id, cell) in block {
            let number = self.palettes.get(&id).map(|p| p.number);
            match number.map_or(Err(CoreError::UnknownPalette(0)), |n| {
                self.copy_palette(n, cell)
            }) {
                Ok(n) => new.push(n),
                Err(e) => {
                    for n in new {
                        self.palettes.remove(&n);
                    }
                    self.palette_next_id = next;
                    return Err(e);
                }
            }
        }
        Ok(new)
    }

    /// PAL-34: move several palettes (numbers and references kept). With `swap` a palette in
    /// the way takes a cell the block leaves free. All or nothing.
    pub fn move_palettes(
        &mut self,
        numbers: &[u32],
        to: Cell,
        layout: Layout,
        swap: bool,
    ) -> Result<(), CoreError> {
        let block = self.palette_block(numbers, to, layout)?;
        let moving: BTreeSet<_> = block.iter().map(|b| b.0).collect();
        let mut in_way = Vec::new();
        for &(_, cell) in &block {
            let Some(o) = self
                .palettes
                .values()
                .find(|p| p.cell() == cell && !moving.contains(&p.id))
            else {
                continue;
            };
            if !swap {
                return Err(CoreError::PaletteCellInUse {
                    window: format!("{:?}", cell.window),
                    handle: cell.handle,
                });
            }
            in_way.push((cell, o.id));
        }
        in_way.sort_unstable();
        let mut freed: Vec<Cell> = (self.palettes.values())
            .filter(|p| moving.contains(&p.id) && !block.iter().any(|b| b.1 == p.cell()))
            .map(Palette::cell)
            .collect();
        freed.sort_unstable();
        let swaps = in_way.into_iter().map(|(_, o)| o).zip(freed);
        for (id, cell) in block.into_iter().chain(swaps) {
            if let Some(p) = self.palettes.get_mut(&id) {
                p.window = cell.window;
                p.handle = cell.handle;
            }
        }
        Ok(())
    }

    /// PAL-34: delete several palettes; refused as a whole when one is unknown.
    pub fn delete_palettes(&mut self, numbers: &[u32]) -> Result<(), CoreError> {
        let numbers: BTreeSet<_> = numbers.iter().copied().collect();
        if numbers.is_empty() {
            return Err(CoreError::InvalidCommand(
                "expected at least one palette".to_string(),
            ));
        }
        if let Some(&n) = numbers.iter().find(|&&n| self.palette(n).is_none()) {
            return Err(CoreError::UnknownPalette(n));
        }
        for n in numbers {
            self.delete_palette(n)?;
        }
        Ok(())
    }

    /// PAL-17: change only the cell, so every reference follows.
    pub fn move_palette(&mut self, number: u32, to: Cell) -> Result<(), CoreError> {
        let id = self
            .palette(number)
            .ok_or(CoreError::UnknownPalette(number))?
            .id;
        self.check_free_cell(to)?;
        if let Some(p) = self.palettes.get_mut(&id) {
            p.window = to.window;
            p.handle = to.handle;
        }
        Ok(())
    }

    /// PAL-17: give a palette a free number; references use the id.
    pub fn renumber_palette(&mut self, number: u32, to: u32) -> Result<(), CoreError> {
        let id = self
            .palette(number)
            .ok_or(CoreError::UnknownPalette(number))?
            .id;
        check_palette_number(to)?;
        if self.palette(to).is_some() {
            return Err(CoreError::PaletteNumberInUse(to));
        }
        if let Some(p) = self.palettes.get_mut(&id) {
            p.number = to;
        }
        Ok(())
    }

    /// PAL-18: delete; references in the programmer and in every playback become their
    /// fallback values. Number and cell are free again, the id is never reused.
    pub fn delete_palette(&mut self, number: u32) -> Result<(), CoreError> {
        let id = self
            .palette(number)
            .ok_or(CoreError::UnknownPalette(number))?
            .id;
        let gone = self.palettes.remove(&id);
        self.programmer.dereference(id);
        // FX-23: an entry that pointed at it keeps the parameters it had.
        let release = |e: &mut effect::Entry| {
            if e.palette != Some(id) {
                return false;
            }
            let same = |x: &&effect::Entry| x.shape == e.shape && x.attributes == e.attributes;
            if let Some(x) = gone.iter().flat_map(|p| &p.effects).find(same) {
                e.params = x.params;
            }
            e.palette = None;
            true
        };
        for e in &mut self.programmer.effects {
            if release(e) {
                e.refresh();
            }
        }
        for e in (self.playbacks.values_mut())
            .flat_map(|pb| pb.cues.iter_mut())
            .flat_map(|cue| cue.effects.iter_mut())
        {
            if release(e) {
                e.refresh();
            }
        }
        for r in self.cue_fx.values_mut().flat_map(|f| f.runs.iter_mut()) {
            release(&mut r.entry);
        }
        for stored in self
            .playbacks
            .values_mut()
            .flat_map(|pb| pb.cues.iter_mut())
            .flat_map(|cue| cue.values.values_mut().chain(cue.dormant.values_mut()))
            .flat_map(|attrs| attrs.values_mut())
        {
            *stored = stored.without_palette(id);
        }
        Ok(())
    }

    /// IPC-54: the cues (and their playbacks) that `delete_palette` would change.
    pub fn palette_usage(&self, number: u32) -> Result<PaletteUsage, CoreError> {
        let id = self
            .palette(number)
            .ok_or(CoreError::UnknownPalette(number))?
            .id;
        let mut usage = PaletteUsage::default();
        for pb in self.playbacks.values() {
            let hits = pb
                .cues
                .iter()
                .filter(|cue| {
                    let value = (cue.stored_items().flat_map(|(_, a)| a.values()))
                        .any(|s| matches!(s, Stored::Reference(r) if r.palette == id));
                    value || cue.effects.iter().any(|e| e.palette == Some(id))
                })
                .count();
            usage.cues = usage.cues.saturating_add(hits as u32);
            usage.playbacks = usage.playbacks.saturating_add(u32::from(hits > 0));
        }
        Ok(usage)
    }

    /// IPC-54: what `patch_remove` of `ids` would touch, counted without changing anything.
    pub fn fixture_usage(&self, ids: &[FixtureId]) -> FixtureUsage {
        let gone = |i: &Item| ids.contains(&i.fixture);
        let mut u = FixtureUsage::default();
        for members in self.groups.values() {
            let hit = members.iter().any(gone);
            u.groups += u32::from(hit);
            u.empty_groups += u32::from(hit && members.iter().all(gone));
        }
        for p in self.palettes.values() {
            let hit =
                p.fixtures.keys().any(gone) || p.effects.iter().any(|e| e.members.iter().any(gone));
            u.palettes += u32::from(hit);
        }
        for pb in self.playbacks.values() {
            let mut cues = 0;
            for cue in &pb.cues {
                // `Entry::retire` keeps an entry that never had members.
                let kept = |e: &effect::Entry| e.members.is_empty() || !e.members.iter().all(gone);
                let hit = cue.stored_items().any(|(i, _)| gone(i))
                    || cue.effects.iter().any(|e| e.members.iter().any(gone));
                let after =
                    !cue.stored_items().all(|(i, _)| gone(i)) || cue.effects.iter().any(kept);
                cues += u32::from(hit);
                u.empty_cues += u32::from(hit && !after);
            }
            u.cues += cues;
            u.playbacks += u32::from(cues > 0);
        }
        u
    }

    /// PAL-20, 22: the legend (at most 32 characters) and the icon id (`None` = automatic).
    pub fn set_palette_legend(
        &mut self,
        number: u32,
        legend: &str,
        icon: Option<&str>,
    ) -> Result<(), CoreError> {
        let id = self
            .palette(number)
            .ok_or(CoreError::UnknownPalette(number))?
            .id;
        check_legend(legend)?;
        if let Some(icon) = icon.filter(|i| !palette::valid_icon(i)) {
            return Err(CoreError::InvalidIcon(icon.to_string()));
        }
        if let Some(p) = self.palettes.get_mut(&id) {
            p.legend = legend.to_string();
            p.icon = icon.map(str::to_string);
        }
        Ok(())
    }

    /// A stored value, or what its palette resolves for this fixture now (PAL-11).
    pub fn resolve_stored(&self, item: impl Into<Item>, attribute: &str, stored: Stored) -> Value {
        let item = item.into();
        match (self.fixtures.get(&item.fixture), self.mode_of(item.fixture)) {
            (Some(f), Some(mode)) => {
                resolve_stored(&self.palettes, item, f, mode, attribute, stored)
            }
            _ => stored.value(),
        }
    }

    /// The palette with this user number.
    pub fn palette(&self, number: u32) -> Option<&Palette> {
        self.palettes.values().find(|p| p.number == number)
    }

    /// Palettes, sorted by id.
    pub fn palettes(&self) -> impl Iterator<Item = &Palette> {
        self.palettes.values()
    }

    /// PAL-07: what palette `number` resolves for a fixture and attribute.
    pub fn palette_value(&self, number: u32, fixture: FixtureId, attribute: &str) -> Option<Value> {
        self.palette(number)?.resolve(
            Item::fixture(fixture),
            self.fixtures.get(&fixture)?,
            self.mode_of(fixture)?,
            attribute,
        )
    }

    /// PAL-21: the icon drawn when no icon was picked.
    pub fn palette_auto_icon(&self, number: u32) -> Option<palette::AutoIcon> {
        self.palette(number)?
            .auto_icon(|name| self.attribute_group(name))
    }

    /// PAL-25: the attribute groups palette `number` holds values in, in I P C G B E S FX order.
    pub fn palette_groups(&self, number: u32) -> Vec<AttributeGroup> {
        let groups: BTreeSet<AttributeGroup> = (self.palette(number).into_iter())
            .flat_map(|p| p.attribute_names())
            .map(|name| self.attribute_group(name))
            .collect();
        groups.into_iter().collect()
    }

    /// PBK-35, CUE-40: the attribute groups all the playback's cues hold values or effects in,
    /// in I P C G B E S FX order.
    pub fn playback_groups(&self, id: PlaybackId) -> Vec<AttributeGroup> {
        let cues = self.playback(id).map(|p| p.cues.as_slice()).unwrap_or(&[]);
        self.groups_of(cues.iter())
    }

    /// CUE-40: the attribute groups one cue holds values or effects in (empty for an unknown cue).
    pub fn cue_groups(&self, id: PlaybackId, cue: CueNumber) -> Vec<AttributeGroup> {
        let cues = self.playback(id).map(|p| p.cues.as_slice()).unwrap_or(&[]);
        self.groups_of(cues.iter().filter(|c| c.number == cue))
    }

    fn groups_of<'a>(&self, cues: impl Iterator<Item = &'a Cue>) -> Vec<AttributeGroup> {
        // FXB-09: the attributes of a cue's effects count as its values do.
        let groups: BTreeSet<AttributeGroup> = cues
            .flat_map(|c| {
                let values = c.values.values().flat_map(|attrs| attrs.keys());
                let effects = c.effects.iter().flat_map(|e| e.attributes.iter());
                values.chain(effects)
            })
            .map(|name| self.attribute_group(name))
            .collect();
        groups.into_iter().collect()
    }

    /// The group of an attribute name, from the profiles (FX when no profile has it). `Colour`
    /// is the built-in target colour (group C) whether or not a patched mode mixes it.
    fn attribute_group(&self, name: &str) -> AttributeGroup {
        if name == COLOUR {
            return AttributeGroup::C;
        }
        (self.profiles.values())
            .flat_map(|p| &p.modes)
            .find_map(|m| m.logical(name).map(|l| l.group))
            .unwrap_or(AttributeGroup::Fx)
    }

    /// PAL-27: the default Record By of palettes.
    pub fn set_record_by(&mut self, record_by: RecordBy) {
        self.settings.record_by = record_by;
    }

    // ---- tick ------------------------------------------------------------

    /// Recompute every universe frame for the current state at `now`.
    ///
    /// Never fails: anything that cannot be resolved is skipped and reported
    /// through [`Engine::drain_warnings`], and the previous value of that
    /// slot survives (ENG-12).
    pub fn tick(&mut self, now: Timestamp) {
        self.last_tick = Some(now);
        self.programmer.start_fades(now);
        self.update_playbacks(now);
        self.tick_effects(now);
        self.tick_cue_effects(now);
        let sources = MergeSources {
            playbacks: &self.playbacks,
            states: &self.states,
            programmer: &self.programmer,
            palettes: &self.palettes,
            swop_owner: self.swop_owner,
        };
        let masters = GroupMasters {
            sources: &sources,
            groups: &self.groups,
            any: self
                .playbacks
                .values()
                .any(|p| p.kind == PlaybackKind::GroupMaster),
        };
        let highlight = Highlight::new(
            self.highlight,
            &self.programmer,
            self.settings.lowlight_level,
        );
        let cells = self.cells_in_play(&highlight);
        let effects = EffectSources {
            programmer: &self.programmer.effects,
            live: &self.effect_live,
            programmer_activated: self.programmer.activated(),
            palettes: &self.palettes,
            playbacks: &self.playbacks,
            states: &self.states,
            cue_fx: &self.cue_fx,
        };
        let pass = Pass {
            sources: &sources,
            masters: &masters,
            highlight: &highlight,
            effects: &effects,
            grand_master: self.grand_master,
            dbo: self.dbo,
            now,
        };
        // GRM-10: (Take Over master, fixture, level) of the passes this tick.
        let mut pickups: Vec<(PlaybackId, FixtureId, f32)> = Vec::new();
        for (fixture_id, fixture) in &self.fixtures {
            if fixture.parked {
                continue; // PAT-29: off the output map
            }
            let Some(mode) = resolve_mode(&self.profiles, fixture) else {
                self.warnings.push(Warning {
                    at: now,
                    message: format!(
                        "fixture {fixture_id}: profile '{}' or its mode missing, holding last values",
                        fixture.profile
                    ),
                });
                continue;
            };
            let Some(frame) = self.frames.get_mut(&fixture.universe) else {
                continue;
            };
            let held = self.held.entry(Item::fixture(*fixture_id)).or_default();
            let round = effect::round_factor(mode.pan_degrees, mode.tilt_degrees);
            // The virtual Dimmer comes first, so `scale` is known for what follows.
            let virtual_dimmer = mode.has_virtual_dimmer();
            let mut scale = 1.0;
            let mut mix = Mix::default();
            for l in mode.logical_attributes() {
                let out = pass.fixture_value(fixture, mode, &l, round, held, None);
                if let Some(v) = out.held {
                    // ENG-18: `held` keeps the base, not base + offset.
                    match held.get_mut(l.name) {
                        Some(slot) => *slot = v,
                        None => {
                            held.insert(l.name.to_string(), v);
                        }
                    }
                }
                if let Some(level) = out.after_masters.filter(|_| l.name == DIMMER) {
                    masters.pickups(*fixture_id, level, &mut pickups);
                }
                let value = out.value;
                if virtual_dimmer && l.name == DIMMER {
                    scale = value.level();
                }
                if mode.automix {
                    mix.record(&l, value);
                }
                write_value(frame, fixture.address, mode, &l, value, scale);
            }
            // ENG-21: the mix overwrites RGB and the mixable emitters, whatever the write order.
            if mode.automix {
                write_mixed(
                    frame,
                    fixture.address,
                    mode.colour.as_ref(),
                    &mode.attributes,
                    &mix,
                    scale,
                );
                for cell in &mode.cells {
                    write_mixed(
                        frame,
                        fixture.address,
                        cell.colour.as_ref(),
                        &cell.attributes,
                        &mix,
                        scale,
                    );
                }
            }
            // ponytail: effects play on whole fixtures only; an entry on a cell item waits for a
            // need (the cell loop below has no effect step).
            // PRG-51: a cell with a value, a located value or a clear fade of its own, or that
            // Highlight or an active playback speaks for, merges on its own and overwrites its
            // slots; the other cells keep the fixture's output. PRG-53: a cell the mode lacks
            // outputs nothing. ponytail: a merge per attribute of each such cell and tick;
            // fine for tens of cells, batch per fixture if a 1000-cell wall ever shows it.
            let own_cells =
                cells.range(Item::cell(*fixture_id, 0)..=Item::cell(*fixture_id, u16::MAX));
            for &item in own_cells {
                let Some((c, cell)) = item.cell.and_then(|c| Some((c, mode.cell(c)?))) else {
                    continue;
                };
                let mut cell_scale = 1.0;
                let mut cell_mix = Mix::default();
                for l in mode.cell_attributes(c) {
                    let virtual_dimmer = l.name == DIMMER && mode.virtual_cell_dimmer(c);
                    let out = pass.cell_value(fixture, mode, &l, c, &self.held, None);
                    if let Some(v) = out.held {
                        (self.held.entry(item).or_default()).insert(l.name.to_string(), v);
                    }
                    let value = out.value;
                    if virtual_dimmer {
                        cell_scale = value.level();
                    }
                    if mode.automix {
                        cell_mix.record(&l, value);
                    }
                    write_cell(frame, fixture.address, cell, &l, value, scale * cell_scale);
                }
                if mode.automix {
                    let scale = scale * cell_scale;
                    write_mixed(
                        frame,
                        fixture.address,
                        cell.colour.as_ref(),
                        &cell.attributes,
                        &cell_mix,
                        scale,
                    );
                }
            }
        }
        self.follow_take_over(&pickups);
        self.programmer.drop_finished_fades(now);
        for state in self.states.values_mut() {
            if state.release.as_ref().is_some_and(|r| r.finished(now)) {
                state.release = None; // PBK-20: this tick was its last
            }
            state.falls.retain(|f| !f.finished(now)); // PBK-47
        }
    }

    /// PRG-51, 52: the cell items some source speaks for: the programmer (a value, a located
    /// value or a clear fade), Highlight, an active playback's cue or list fades, a release in
    /// progress. The tick merges those cells on their own.
    fn cells_in_play(&self, highlight: &Highlight) -> BTreeSet<Item> {
        let mut items: BTreeSet<Item> = self.programmer.cell_items().collect();
        if highlight.on {
            items.extend(
                highlight
                    .targets
                    .iter()
                    .chain(&highlight.others)
                    .filter(|i| i.cell.is_some()),
            );
        }
        for (id, playback) in &self.playbacks {
            let Some(state) = self.states.get(id) else {
                continue;
            };
            if state.activated.is_some() {
                if is_list(playback) {
                    items.extend(state.run.items().filter(|i| i.cell.is_some()));
                } else {
                    let values = playback.values().into_iter().flat_map(|v| v.keys());
                    items.extend(values.filter(|i| i.cell.is_some()));
                }
            }
            if let Some(release) = &state.release {
                items.extend(release.entries.keys().filter(|i| i.cell.is_some()));
            }
            for fall in &state.falls {
                items.extend(fall.entries.keys().filter(|i| i.cell.is_some()));
            }
        }
        items
    }

    /// GRM-10: after the frame of a tick, put the fixtures a Take Over master has just passed in
    /// the programmer at its level, and keep every fixture it has taken at that level; a taken
    /// fixture that left the programmer or the group is let go.
    fn follow_take_over(&mut self, pickups: &[(PlaybackId, FixtureId, f32)]) {
        let put = |programmer: &mut Programmer, seq: &mut u64, fixture: FixtureId, g: f32| {
            *seq = seq.saturating_add(1);
            let level = Stored::Value(Value::Level(g));
            programmer.set_value(fixture, DIMMER, level, *seq);
        };
        for &(id, fixture, g) in pickups {
            if let Some(state) = self.states.get_mut(&id) {
                state.taken.insert(fixture);
                put(&mut self.programmer, &mut self.activation_seq, fixture, g);
            }
        }
        for (id, state) in &mut self.states {
            let Some(Master::Group(m)) = (self.playbacks.get(id)).and_then(|p| p.master.as_ref())
            else {
                continue;
            };
            if m.mode != GroupMode::TakeOver {
                state.taken.clear();
                state.takeover_prev = None;
                continue;
            }
            let g = if m.enabled {
                state.effective_level()
            } else {
                m.level
            };
            state.takeover_prev = Some(g);
            let members = self.groups.get(&m.group);
            let level = Some(Stored::Value(Value::Level(g)));
            state.taken.retain(|&f| {
                let current = self.programmer.stored(f, DIMMER);
                let keep =
                    current.is_some() && members.is_some_and(|g| g.contains(&Item::fixture(f)));
                if keep && current != level {
                    put(&mut self.programmer, &mut self.activation_seq, f, g);
                }
                keep
            });
        }
    }

    // ---- queries ---------------------------------------------------------

    /// ENG-25: why each attribute of the fixture (and each cell in play) is what it is, as of
    /// the last tick: the value the tick writes, the merge winner and the HTP sources that
    /// lost, and what changed the value after. `None` for an unknown fixture. It runs the same
    /// code as the tick ([`Pass`]) and changes nothing.
    pub fn explain(&self, id: FixtureId) -> Option<Vec<Explanation>> {
        let fixture = self.fixtures.get(&id)?;
        let mode = resolve_mode(&self.profiles, fixture)?;
        let sources = MergeSources {
            playbacks: &self.playbacks,
            states: &self.states,
            programmer: &self.programmer,
            palettes: &self.palettes,
            swop_owner: self.swop_owner,
        };
        let masters = GroupMasters {
            sources: &sources,
            groups: &self.groups,
            any: (self.playbacks.values()).any(|p| p.kind == PlaybackKind::GroupMaster),
        };
        let highlight = Highlight::new(
            self.highlight,
            &self.programmer,
            self.settings.lowlight_level,
        );
        let effects = EffectSources {
            programmer: &self.programmer.effects,
            live: &self.effect_live,
            programmer_activated: self.programmer.activated(),
            palettes: &self.palettes,
            playbacks: &self.playbacks,
            states: &self.states,
            cue_fx: &self.cue_fx,
        };
        let pass = Pass {
            sources: &sources,
            masters: &masters,
            highlight: &highlight,
            effects: &effects,
            grand_master: self.grand_master,
            dbo: self.dbo,
            now: self.last_tick.unwrap_or(0.0),
        };
        let round = effect::round_factor(mode.pan_degrees, mode.tilt_degrees);
        let none = BTreeMap::new();
        let held = self.held.get(&Item::fixture(id)).unwrap_or(&none);
        let mut out = Vec::new();
        for l in mode.logical_attributes() {
            let mut trace = Trace::default();
            let value = pass.fixture_value(fixture, mode, &l, round, held, Some(&mut trace));
            out.push(self.explanation(&l, Item::fixture(id), value.value, trace));
        }
        let cells = self.cells_in_play(&highlight);
        let own = cells.range(Item::cell(id, 0)..=Item::cell(id, u16::MAX));
        for &item in own {
            let Some(c) = item.cell.filter(|&c| mode.cell(c).is_some()) else {
                continue;
            };
            for l in mode.cell_attributes(c) {
                let mut trace = Trace::default();
                let value = pass.cell_value(fixture, mode, &l, c, &self.held, Some(&mut trace));
                out.push(self.explanation(&l, item, value.value, trace));
            }
        }
        Some(out)
    }

    fn explanation(
        &self,
        l: &LogicalAttribute,
        item: Item,
        value: Value,
        trace: Trace,
    ) -> Explanation {
        let source = |via: Via| match via.playback {
            None => Source::Programmer,
            Some((id, list)) => Source::Playback {
                id,
                cue: self
                    .list_live(id)
                    .filter(|_| list)
                    .and_then(|live| live.current),
            },
        };
        let base = match trace.winner {
            Some(c) => Base {
                source: source(c.via),
                value: c.value,
                tracked: self.tracked(c.via, item, l.name),
                palette: c.via.palette,
            },
            None => Base {
                source: match l.kind {
                    AttributeKind::Htp => Source::Default,
                    AttributeKind::Ltp => Source::Held,
                },
                value: trace.unproposed.unwrap_or(l.default),
                tracked: false,
                palette: None,
            },
        };
        Explanation {
            name: l.name.to_string(),
            cell: item.cell,
            value,
            base,
            others: (trace.others.iter())
                .map(|c| Proposal {
                    source: source(c.via),
                    value: c.value,
                })
                .collect(),
            modifiers: trace.modifiers,
        }
    }

    /// ENG-25, CUE-50: a list's value that is not hard in its current cue.
    fn tracked(&self, via: Via, item: Item, name: &str) -> bool {
        let Some((id, true)) = via.playback else {
            return false;
        };
        let current = self.list_live(id).and_then(|live| live.current);
        let Some((playback, current)) = self.playbacks.get(&id).zip(current) else {
            return false;
        };
        let Some(at) = playback.cues.iter().position(|c| c.number == current) else {
            return false;
        };
        let state = playback.cue_state(at);
        [item, Item::fixture(item.fixture)]
            .iter()
            .find_map(|i| state.get(i)?.get(name))
            .is_some_and(|&(_, hard)| hard != at)
    }

    pub fn frame(&self, universe: UniverseId) -> Option<&Frame> {
        self.frames.get(&universe)
    }

    pub fn universes(&self) -> impl Iterator<Item = (&UniverseId, &Frame)> {
        self.frames.iter()
    }

    pub fn playback(&self, id: PlaybackId) -> Option<&Playback> {
        self.playbacks.get(&id)
    }

    /// Every playback, sorted by id.
    pub fn playbacks(&self) -> impl Iterator<Item = &Playback> {
        self.playbacks.values()
    }

    pub fn playback_state(&self, id: PlaybackId) -> Option<&PlaybackState> {
        self.states.get(&id)
    }

    /// Every playback's state, sorted by id.
    pub fn playback_states(&self) -> impl Iterator<Item = (&PlaybackId, &PlaybackState)> {
        self.states.iter()
    }

    pub fn programmer(&self) -> &Programmer {
        &self.programmer
    }

    /// Patched fixtures, sorted by id.
    pub fn fixtures(&self) -> impl Iterator<Item = &Fixture> {
        self.fixtures.values()
    }

    pub fn fixture(&self, id: FixtureId) -> Option<&Fixture> {
        self.fixtures.get(&id)
    }

    /// PAT-09: the fixture's mode (its profile's first when none was named).
    pub fn fixture_mode(&self, id: FixtureId) -> Option<&Mode> {
        resolve_mode(&self.profiles, self.fixtures.get(&id)?)
    }

    pub fn fixture_by_number(&self, number: u32) -> Option<&Fixture> {
        self.numbers
            .get(&number)
            .and_then(|id| self.fixtures.get(id))
    }

    /// Recorded groups, sorted by id, with their items in group order (IPC-42).
    pub fn groups(&self) -> BTreeMap<GroupId, Vec<Item>> {
        self.groups.clone()
    }

    pub fn group(&self, id: GroupId) -> Option<Vec<FixtureId>> {
        self.groups.get(&id).map(|items| whole_fixtures(items))
    }

    pub fn grand_master(&self) -> f32 {
        self.grand_master
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// PRG-96: take the group notices (for the IPC layer; not warnings, not in the edit state).
    pub fn drain_notices(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notices)
    }

    /// Take accumulated warnings (for the IPC layer to forward to the UI).
    pub fn drain_warnings(&mut self) -> Vec<Warning> {
        std::mem::take(&mut self.warnings)
    }
}

/// The whole-fixture items of a list, as fixture ids.
fn whole_fixtures(items: &[Item]) -> Vec<FixtureId> {
    (items.iter())
        .filter(|i| i.cell.is_none())
        .map(|i| i.fixture)
        .collect()
}

/// CUE-26: a cue list or a chase.
fn is_list(playback: &Playback) -> bool {
    playback.is_cue_list() || playback.kind == PlaybackKind::Chase
}

/// CUE-33: add a tap to the series and give the BPM it implies; `None` for the first tap of a
/// series (more than 2 s after the last starts a new one). The mean of the last three intervals,
/// 1–600, rounded to 0.1. Shared by playbacks (CUE-33) and programmer entries (FX-10).
fn tap_bpm(series: &mut Vec<Timestamp>, at: Timestamp) -> Option<f32> {
    if series
        .last()
        .is_some_and(|&last| !(0.0..=2.0).contains(&(at - last)))
    {
        series.clear();
    }
    series.push(at);
    series.drain(..series.len().saturating_sub(4));
    let (first, last) = (*series.first()?, *series.last()?);
    let intervals = series.len().saturating_sub(1);
    if intervals == 0 {
        return None;
    }
    let mean = (last - first) / intervals as f64;
    let bpm = ((60.0 / mean) as f32).clamp(1.0, 600.0);
    Some((bpm * 10.0).round() / 10.0)
}

/// CUE-33, 34: a chase or a BPM master has a tempo to tap or type.
fn has_tempo(playback: &Playback) -> bool {
    playback.kind == PlaybackKind::Chase || matches!(playback.master, Some(Master::Bpm(_)))
}

/// CUE-25: the kind of handle the buttons are assigned for.
fn button_kind(playback: &Playback) -> ButtonKind {
    match playback.kind {
        PlaybackKind::Chase => ButtonKind::Chase,
        PlaybackKind::BpmMaster => ButtonKind::BpmMaster,
        PlaybackKind::RateMaster => ButtonKind::RateMaster,
        PlaybackKind::GroupMaster => ButtonKind::GroupMaster,
        PlaybackKind::Cues if playback.is_cue_list() => ButtonKind::CueList,
        PlaybackKind::Cues => ButtonKind::Memory,
    }
}

/// PAL-15: times 0-600 s, overlaps 0-100 %; NaN is refused too.
fn check_palette_timing(time: f32, overlap: f32) -> Result<(), CoreError> {
    if !(0.0..=600.0).contains(&time) {
        return Err(CoreError::InvalidPaletteTime(time));
    }
    if !(0.0..=100.0).contains(&overlap) {
        return Err(CoreError::InvalidPaletteOverlap(overlap));
    }
    Ok(())
}

/// PBK-12, PBK-24: times run from 0 to 600 s; NaN is refused too.
fn check_time(time: f32) -> Result<(), CoreError> {
    if (0.0..=600.0).contains(&time) {
        Ok(())
    } else {
        Err(CoreError::InvalidTime(time))
    }
}

/// PBK-12, PBK-24: overlaps run from 0 to 100 %.
fn check_overlap(overlap: f32) -> Result<(), CoreError> {
    if (0.0..=100.0).contains(&overlap) {
        Ok(())
    } else {
        Err(CoreError::InvalidOverlap(overlap))
    }
}

/// CUE-08: a number a cue can move or be copied to: valid and not used in the playback.
fn check_new_cue_number(playback: &Playback, number: CueNumber) -> Result<(), CoreError> {
    if !number.is_valid() {
        Err(CoreError::InvalidCueNumber)
    } else if playback.cues.iter().any(|c| c.number == number) {
        Err(CoreError::CueNumberInUse(number))
    } else {
        Ok(())
    }
}

/// CUE-09: a link offset runs from 0 to 600 s.
fn check_link(link: &Link) -> Result<(), CoreError> {
    match *link {
        Link::Wait => Ok(()),
        Link::After { offset } | Link::With { offset } => check_time(offset),
    }
}

/// CUE-09: group delays and fades run from 0 to 600 s.
fn check_group_times(times: &BTreeMap<AttributeGroup, GroupTime>) -> Result<(), CoreError> {
    times
        .values()
        .try_for_each(|t| check_time(t.delay).and_then(|()| check_time(t.fade)))
}

/// PAL-20, PBK-31: a legend is at most 32 characters.
fn check_legend(legend: &str) -> Result<(), CoreError> {
    if legend.chars().count() > palette::MAX_LEGEND {
        return Err(CoreError::InvalidLegend);
    }
    Ok(())
}

/// PAL-17: user numbers run from 1 to 9999.
fn check_palette_number(number: u32) -> Result<(), CoreError> {
    if (1..=palette::MAX_NUMBER).contains(&number) {
        Ok(())
    } else {
        Err(CoreError::InvalidPaletteNumber(number))
    }
}

/// The named mode of a profile, or its first when `mode` is `None`.
fn lookup_mode<'a>(
    profiles: &'a BTreeMap<String, Profile>,
    profile: &str,
    mode: Option<&str>,
) -> Result<&'a Mode, CoreError> {
    profiles
        .get(profile)
        .ok_or_else(|| CoreError::UnknownProfile(profile.to_string()))?
        .mode(mode)
        .ok_or_else(|| CoreError::UnknownMode {
            profile: profile.to_string(),
            mode: mode.unwrap_or_default().to_string(),
        })
}

/// The fixture's mode: the named one, or the profile's first.
pub(crate) fn resolve_mode<'a>(
    profiles: &'a BTreeMap<String, Profile>,
    fixture: &Fixture,
) -> Option<&'a Mode> {
    profiles
        .get(&fixture.profile)
        .and_then(|p| p.mode(fixture.mode.as_deref()))
}

/// PRG-38: the value Locate puts an attribute at.
fn locate_value(mode: &Mode, l: &LogicalAttribute) -> Value {
    let def = mode
        .attribute(l.name)
        .or_else(|| mode.cells.iter().find_map(|c| c.attribute(l.name)));
    if let Some(v) = def.and_then(|a| a.locate) {
        Value::Level(v)
    } else if l.is_colour() {
        Value::Colour(TargetColour::WHITE)
    } else if l.emitter {
        Value::Level(0.0) // white comes from the target colour (ADR-005)
    } else if l.group == AttributeGroup::I && l.kind == AttributeKind::Htp {
        Value::Level(1.0)
    } else if l.name == "Pan" || l.name == "Tilt" {
        Value::Level(0.5)
    } else {
        l.default
    }
}

/// PRG-26, 39, 54: who Highlight lifts and who it lowlights, per item, as the tick sees it.
struct Highlight {
    on: bool,
    targets: Vec<Item>,
    /// The rest of the selection while a sub-selection is active.
    others: Vec<Item>,
    lowlight: f32,
}

impl Highlight {
    fn new(on: bool, programmer: &Programmer, lowlight: f32) -> Self {
        let targets = programmer.target_items();
        let others = (programmer.items().iter())
            .filter(|i| !targets.contains(i))
            .copied()
            .collect();
        Highlight {
            on,
            targets,
            others,
            lowlight,
        }
    }

    /// The value Highlight puts the attribute of the item at, `None` when it leaves it alone.
    fn value(&self, item: Item, mode: &Mode, l: &LogicalAttribute) -> Option<Value> {
        if !self.on {
            return None;
        }
        if self.others.contains(&item) {
            return (l.group == AttributeGroup::I && l.kind == AttributeKind::Htp)
                .then_some(Value::Level(self.lowlight));
        }
        if !self.targets.contains(&item) {
            return None;
        }
        let def = match item.cell {
            Some(c) => mode.cell(c).and_then(|c| c.attribute(l.name)),
            None => mode
                .attribute(l.name)
                .or_else(|| mode.cells.iter().find_map(|c| c.attribute(l.name))),
        };
        let own = def.and_then(|a| a.highlight).map(Value::Level);
        if l.group == AttributeGroup::I && l.kind == AttributeKind::Htp {
            own.or(Some(Value::Level(1.0)))
        } else if l.is_colour() {
            Some(Value::Colour(TargetColour::WHITE))
        } else if l.emitter {
            Some(Value::Level(0.0))
        } else {
            own
        }
    }
}

/// Write one logical attribute's value into the fixture's slots. A level goes to the mode's
/// attribute of that name and to the same-named attribute of every cell; Colour goes to the
/// mode's and every cell's mixing channels (LIB-07, ADR-005). `scale` is the virtual dimmer
/// (LIB-04): it multiplies colour and emitters, before CMY inverts.
fn write_value(
    frame: &mut Frame,
    address: u16,
    mode: &Mode,
    l: &LogicalAttribute,
    value: Value,
    scale: f32,
) {
    match value {
        Value::Level(v) => {
            let v = if l.emitter { v * scale } else { v };
            let own = mode.attribute(l.name);
            let cells = mode.cells.iter().filter_map(|c| c.attribute(l.name));
            for attr in own.into_iter().chain(cells) {
                frame.set_attribute(address, attr, v);
            }
        }
        Value::Colour(c) => {
            let c = c.scaled(scale);
            let cells = mode.cells.iter().filter_map(|cell| cell.colour.as_ref());
            for mixing in mode.colour.iter().chain(cells) {
                frame.set_mixing(address, mixing, c);
            }
        }
    }
}

/// Most emitters whose own level is kept for the mix; further ones are written directly.
const MIX_EMITTERS: usize = 16;

/// The merged Colour and emitter levels of one fixture or cell, kept for [`write_mixed`].
#[derive(Default)]
struct Mix<'a> {
    colour: Option<TargetColour>,
    n: usize,
    levels: [(&'a str, f32); MIX_EMITTERS],
}

impl<'a> Mix<'a> {
    fn record(&mut self, l: &LogicalAttribute<'a>, value: Value) {
        match value {
            Value::Colour(c) => self.colour = Some(c),
            Value::Level(v) if l.emitter => {
                if let Some(slot) = self.levels.get_mut(self.n) {
                    *slot = (l.name, v);
                    self.n += 1;
                }
            }
            Value::Level(_) => {}
        }
    }

    fn level(&self, name: &str) -> f32 {
        self.levels
            .iter()
            .take(self.n)
            .find(|(n, _)| *n == name)
            .map_or(0.0, |&(_, v)| v)
    }
}

/// ENG-21..23: take the emitters out of the target colour in attribute order and write the
/// remainder to RGB; each mixable emitter gets its auto level plus its own value (clamped), and
/// the virtual dimmer `scale` applies after the mix. Unmixable emitters are left as written.
fn write_mixed(
    frame: &mut Frame,
    address: u16,
    mixing: Option<&ColourMixing>,
    attributes: &[AttributeDef],
    mix: &Mix,
    scale: f32,
) {
    let (Some(mixing), Some(target)) = (mixing, mix.colour) else {
        return;
    };
    if mixing.system != ColourSystem::Rgb {
        return;
    }
    let mut rem = [target.r, target.g, target.b].map(|c| c.clamp(0.0, 1.0));
    for a in attributes.iter().filter(|a| a.emitter) {
        let Some(eq) = a.cie.and_then(crate::colour::rgb_equivalent) else {
            continue;
        };
        let k = rem
            .iter()
            .zip(eq)
            .filter(|(_, e)| *e > 0.0)
            .map(|(r, e)| r / e)
            .fold(f32::INFINITY, f32::min);
        let level = (k / a.output).clamp(0.0, 1.0);
        for (r, e) in rem.iter_mut().zip(eq) {
            *r = (*r - level * a.output * e).max(0.0);
        }
        frame.set_attribute(address, a, (level + mix.level(&a.name)).min(1.0) * scale);
    }
    let [r, g, b] = rem;
    frame.set_mixing(address, mixing, TargetColour { r, g, b }.scaled(scale));
}

/// PRG-51: write one attribute of one cell into that cell's own slots. `scale` is the mode's
/// virtual dimmer times the cell's (PRG-47), applied as in [`write_value`].
fn write_cell(
    frame: &mut Frame,
    address: u16,
    cell: &crate::profile::Cell,
    l: &LogicalAttribute,
    value: Value,
    scale: f32,
) {
    match value {
        Value::Level(v) => {
            let v = if l.emitter { v * scale } else { v };
            if let Some(attr) = cell.attribute(l.name) {
                frame.set_attribute(address, attr, v);
            }
        }
        Value::Colour(c) => {
            if let Some(mixing) = &cell.colour {
                frame.set_mixing(address, mixing, c.scaled(scale));
            }
        }
    }
}

/// What a merge reads: every source of attribute values.
struct MergeSources<'a> {
    playbacks: &'a BTreeMap<PlaybackId, Playback>,
    states: &'a BTreeMap<PlaybackId, PlaybackState>,
    programmer: &'a Programmer,
    palettes: &'a BTreeMap<PaletteId, Palette>,
    /// PBK-27: the Swop that is held, if any; every other playback's HTP is silent.
    swop_owner: Option<PlaybackId>,
}

/// GRM-06..10: the group masters, read while a tick merges.
struct GroupMasters<'a> {
    sources: &'a MergeSources<'a>,
    groups: &'a BTreeMap<GroupId, Vec<Item>>,
    /// False when no playback is a group master: nothing to look for.
    any: bool,
}

impl GroupMasters<'_> {
    /// The masters of `mode` holding `fixture` with their level *g*: the fader (or 1.0 while
    /// Flash or Swop is held or latched), the saved level while disabled (GRM-11). GRM-19: an
    /// iterator over the playbacks, no list per fixture. ponytail: O(playbacks × group size)
    /// per fixture and attribute; index the masters per tick if a big rig ever shows it.
    fn holding(
        &self,
        fixture: FixtureId,
        mode: GroupMode,
    ) -> impl Iterator<Item = (PlaybackId, f32)> + '_ {
        let (playbacks, states) = (self.sources.playbacks, self.sources.states);
        playbacks.iter().filter_map(move |(id, p)| {
            let Some(Master::Group(m)) = &p.master else {
                return None;
            };
            let holds = self
                .groups
                .get(&m.group)
                .is_some_and(|f| f.contains(&Item::fixture(fixture)));
            if m.mode != mode || !holds {
                return None;
            }
            let fader = states
                .get(id)
                .map_or(m.level, PlaybackState::effective_level);
            Some((*id, if m.enabled { fader } else { m.level }))
        })
    }

    /// GRM-06: Scale (× *g* × scale), then HTP (the highest), then Limit (the lowest). Only
    /// Scale reaches a colour. The Grand Master comes after.
    fn apply(&self, fixture: FixtureId, value: Value) -> Value {
        if !self.any {
            return value;
        }
        let owner = self.sources.swop_owner;
        let scale = |v: Value, (id, g): (PlaybackId, f32)| {
            let percent = match self
                .sources
                .playbacks
                .get(&id)
                .and_then(|p| p.master.as_ref())
            {
                Some(Master::Group(m)) => f32::from(m.scale),
                _ => 100.0,
            };
            match v {
                Value::Level(l) => Value::Level((l * g * percent / 100.0).min(1.0)),
                colour => colour.scaled((g * percent / 100.0).min(1.0)),
            }
        };
        let value = self.holding(fixture, GroupMode::Scale).fold(value, scale);
        let Value::Level(level) = value else {
            return value;
        };
        // GRM-08: during another playback's Swop an HTP master adds nothing.
        let htp = self.holding(fixture, GroupMode::Htp);
        let level = htp
            .filter(|&(id, _)| owner.is_none_or(|o| o == id))
            .fold(level, |l, (_, g)| l.max(g));
        let limit = self.holding(fixture, GroupMode::Limit);
        Value::Level(limit.fold(level, |l, (_, g)| l.min(g)))
    }

    /// ENG-25: the masters holding `fixture` in the order `apply` uses them, also at full.
    /// ponytail: an HTP master silenced by another playback's Swop (GRM-08) is still listed.
    fn describe(&self, fixture: FixtureId, out: &mut Vec<Modifier>) {
        if !self.any {
            return;
        }
        for mode in [GroupMode::Scale, GroupMode::Htp, GroupMode::Limit] {
            for (playback, level) in self.holding(fixture, mode) {
                let master =
                    (self.sources.playbacks.get(&playback)).and_then(|p| p.master.as_ref());
                let enabled = match master {
                    Some(Master::Group(m)) => m.enabled,
                    _ => true,
                };
                out.push(Modifier::Master {
                    playback,
                    mode,
                    level,
                    enabled,
                });
            }
        }
    }

    /// GRM-10: the Take Over masters whose level just passed `output` (the fixture's Dimmer
    /// after the masters, before the Grand Master). Only a pass allocates.
    fn pickups(
        &self,
        fixture: FixtureId,
        output: f32,
        out: &mut Vec<(PlaybackId, FixtureId, f32)>,
    ) {
        if !self.any {
            return;
        }
        for (id, g) in self.holding(fixture, GroupMode::TakeOver) {
            let Some(state) = self.sources.states.get(&id) else {
                continue;
            };
            let passed = state
                .takeover_prev
                .is_some_and(|a| a != g && output >= a.min(g) && output <= a.max(g));
            if passed && !state.taken.contains(&fixture) {
                out.push((id, fixture, g));
            }
        }
    }
}

/// ENG-25: what the value of one attribute needs, for the tick and for `explain` alike, so the
/// two cannot drift apart.
struct Pass<'a> {
    sources: &'a MergeSources<'a>,
    masters: &'a GroupMasters<'a>,
    highlight: &'a Highlight,
    effects: &'a EffectSources<'a>,
    grand_master: f32,
    dbo: bool,
    now: Timestamp,
}

/// What the tick takes from one attribute.
struct Out {
    value: Value,
    /// LTP: the merged value `held` keeps (ENG-18).
    held: Option<Value>,
    /// HTP: the level after the group masters, before the Grand Master (GRM-10).
    after_masters: Option<f32>,
}

impl Pass<'_> {
    /// One logical attribute of a fixture: merge, Highlight, effect, masters, Grand Master. With a
    /// `trace` it also says how (ENG-25); the tick passes `None` and allocates nothing for it.
    fn fixture_value(
        &self,
        fixture: &Fixture,
        mode: &Mode,
        l: &LogicalAttribute,
        round: f32,
        held: &BTreeMap<String, Value>,
        mut trace: Option<&mut Trace>,
    ) -> Out {
        let item = Item::fixture(fixture.id);
        let others = trace.as_deref_mut().map(|t| &mut t.others);
        let merged = merge_attribute(self.sources, fixture, mode, l, self.now, None, others);
        // PRG-26 (ENG-14): Highlight replaces the merged value, never the held one.
        let lit = self.highlight.value(item, mode, l);
        // ENG-14: the programmer effect sits on the base value, before the masters.
        let effected = std::cell::Cell::new(false);
        let fx = |base: Value| {
            let htp = l.kind == AttributeKind::Htp;
            let v = with_effect(self.effects, item, l.name, htp, base, round, self.now);
            effected.set(v.is_some());
            v.unwrap_or(base)
        };
        let winner = merged.map(|c| c.value);
        // ENG-07: an HTP attribute nobody proposes is 0; LTP holds its last value (LIB-03).
        let held_or_default = held.get(l.name).copied().unwrap_or(l.default);
        let out = match (winner, l.kind) {
            (Some(v), AttributeKind::Ltp) => Out {
                value: lit.unwrap_or_else(|| fx(v)),
                held: Some(v),
                after_masters: None,
            },
            (winner, AttributeKind::Htp) => {
                // ENG-07: nobody proposing is 0. GRM-06: the group masters, then the GM.
                let base = lit.unwrap_or_else(|| fx(winner.unwrap_or(Value::Level(0.0))));
                let v = self.masters.apply(fixture.id, base);
                Out {
                    value: v.scaled(self.grand_master()),
                    held: None,
                    after_masters: Some(v.level()),
                }
            }
            (None, AttributeKind::Ltp) => Out {
                value: lit.unwrap_or_else(|| fx(held_or_default)),
                held: None,
                after_masters: None,
            },
        };
        if let Some(t) = trace {
            t.winner = merged;
            t.unproposed = Some(match l.kind {
                AttributeKind::Htp => Value::Level(0.0),
                AttributeKind::Ltp => held_or_default,
            });
            let htp = l.kind == AttributeKind::Htp;
            self.modifiers(t, fixture.id, mode, l, lit.is_some(), effected.get(), htp);
        }
        out
    }

    /// PRG-51: one attribute of a cell, merged on its own. No effect step (see the tick).
    fn cell_value(
        &self,
        fixture: &Fixture,
        mode: &Mode,
        l: &LogicalAttribute,
        c: u16,
        held: &BTreeMap<Item, BTreeMap<String, Value>>,
        mut trace: Option<&mut Trace>,
    ) -> Out {
        let item = Item::cell(fixture.id, c);
        let virtual_dimmer = l.name == DIMMER && mode.virtual_cell_dimmer(c);
        let others = trace.as_deref_mut().map(|t| &mut t.others);
        let merged = merge_attribute(self.sources, fixture, mode, l, self.now, Some(c), others);
        // A fixture's highlight counts for every cell but its own virtual Dimmer.
        let lit = self.highlight.value(item, mode, l).or_else(|| {
            let fixture_l = mode.logical(l.name).filter(|_| !virtual_dimmer)?;
            self.highlight
                .value(Item::fixture(fixture.id), mode, &fixture_l)
        });
        let winner = merged.map(|c| c.value);
        let held_or_default = held_value(held, item, l.name).unwrap_or(l.default);
        let (value, held, unproposed, masters) = match (winner, l.kind) {
            (Some(v), AttributeKind::Ltp) => (lit.unwrap_or(v), Some(v), held_or_default, false),
            // The virtual cell Dimmer is no master: no group masters, no Grand Master (they
            // scale the master Dimmer), and full while nobody proposes.
            (winner, AttributeKind::Htp) if virtual_dimmer => {
                let value = lit.or(winner).unwrap_or(l.default);
                (value, None, l.default, false)
            }
            (winner, AttributeKind::Htp) => {
                let base = lit.or(winner).unwrap_or(Value::Level(0.0));
                let value = self.masters.apply(fixture.id, base);
                (
                    value.scaled(self.grand_master()),
                    None,
                    Value::Level(0.0),
                    true,
                )
            }
            (None, AttributeKind::Ltp) => {
                (lit.unwrap_or(held_or_default), None, held_or_default, false)
            }
        };
        if let Some(t) = trace {
            t.winner = merged;
            t.unproposed = Some(unproposed);
            self.modifiers(t, fixture.id, mode, l, lit.is_some(), false, masters);
        }
        Out {
            value,
            held,
            after_masters: None,
        }
    }

    /// PBK-30: DBO is a Grand Master of 0 for the HTP output, whoever proposes it.
    fn grand_master(&self) -> f32 {
        if self.dbo {
            0.0
        } else {
            self.grand_master
        }
    }

    /// ENG-25: what changed the merged value, in the order it is applied.
    #[allow(clippy::too_many_arguments)]
    fn modifiers(
        &self,
        t: &mut Trace,
        fixture: FixtureId,
        mode: &Mode,
        l: &LogicalAttribute,
        highlighted: bool,
        effected: bool,
        masters: bool,
    ) {
        if t.winner.is_some_and(|c| c.via.located) {
            t.modifiers.push(Modifier::Locate);
        }
        if highlighted {
            t.modifiers.push(Modifier::Highlight);
        }
        if effected {
            t.modifiers.push(Modifier::Effect);
        }
        if masters {
            self.masters.describe(fixture, &mut t.modifiers);
            if self.dbo {
                t.modifiers.push(Modifier::Dbo);
            } else if self.grand_master < 1.0 {
                t.modifiers.push(Modifier::GrandMaster(self.grand_master));
            }
        }
        if mode.automix && (l.emitter || l.is_colour()) {
            t.modifiers.push(Modifier::AutoMix);
        }
    }
}

/// What the effect layer reads while a tick merges (ENG-14, 15).
struct EffectSources<'a> {
    programmer: &'a [effect::Entry],
    live: &'a BTreeMap<EntryId, EffectLive>,
    programmer_activated: u64,
    palettes: &'a BTreeMap<PaletteId, Palette>,
    playbacks: &'a BTreeMap<PlaybackId, Playback>,
    states: &'a BTreeMap<PlaybackId, PlaybackState>,
    cue_fx: &'a BTreeMap<PlaybackId, CueFx>,
}

/// ENG-14..16: `base` with the effect on `item`'s `attribute`, `None` when none acts: at most
/// one source acts (ENG-15): the highest priority, then the latest activation; the programmer
/// sits at `Priority::Programmer`. The programmer's fades out while a timed Clear runs (FX-19). A
/// playback's follows its master *m* (FX-25). ponytail: O(playbacks × runs × members) per
/// attribute and tick, no allocation; index the members per tick if 500 members ever show it
/// (FX-13).
fn with_effect(
    src: &EffectSources,
    item: Item,
    attribute: &str,
    htp: bool,
    base: Value,
    round: f32,
    now: Timestamp,
) -> Option<Value> {
    let mut best: Option<((Priority, u64), &CueFx, f32)> = None;
    for (id, fx) in src.cue_fx {
        let (Some(playback), Some(state)) = (src.playbacks.get(id), src.states.get(id)) else {
            continue;
        };
        let Some(activated) = state.activated else {
            continue;
        };
        let key = (playback.priority, activated);
        let drives = fx.runs.iter().any(|r| r.index(item, attribute).is_some());
        if drives && best.as_ref().is_none_or(|b| key > b.0) {
            best = Some((key, fx, state.master));
        }
    }
    let found = src.programmer.iter().find_map(|e| {
        e.attributes.iter().any(|a| a == attribute).then_some(())?;
        Some((e, e.members.iter().position(|m| *m == item)?))
    });
    if let Some(((priority, activated), fx, m)) = best {
        let programmer_wins = found.is_some()
            && (priority < Priority::Programmer
                || (priority == Priority::Programmer && activated <= src.programmer_activated));
        if !programmer_wins {
            return Some(playback_effect(
                src,
                fx,
                m,
                (item, attribute, htp),
                base,
                round,
                now,
            ));
        }
    }
    let (entry, i) = found?;
    let live = src.live.get(&entry.id)?;
    let p = pointed(src.palettes, entry);
    let v = entry.at(&p, i, live.clock.theta(), attribute, base, round);
    Some(match &live.fade {
        Some(fade) => base.lerp(v, 1.0 - fade.progress(now)),
        None => v,
    })
}

/// FX-25, 26: a playback's effect on one attribute: an absolute effect on an `Htp` attribute
/// scales like an intensity, any other has its size multiplied by *m*; the run of the cue before
/// fades out as the run of this cue fades in.
fn playback_effect(
    src: &EffectSources,
    fx: &CueFx,
    m: f32,
    (item, attribute, htp): (Item, &str, bool),
    base: Value,
    round: f32,
    now: Timestamp,
) -> Value {
    let (mut leaving, mut coming) = (None, None);
    for r in &fx.runs {
        let Some(i) = r.index(item, attribute) else {
            continue;
        };
        let mut p = r.params(src.palettes, now);
        let absolute = htp && p.mode == effect::Mode::Absolute;
        if !absolute {
            p.size *= m;
        }
        let mut v = r.entry.at(&p, i, r.clock.theta(), attribute, base, round);
        if absolute {
            v = v.scaled(m);
        }
        let slot = if r.leaving { &mut leaving } else { &mut coming };
        *slot = Some((v, r.weight(now)));
    }
    match (leaving, coming) {
        (None, None) => base,
        (Some((a, w)), None) | (None, Some((a, w))) => base.lerp(a, w),
        (Some((a, _)), Some((b, w))) => a.lerp(b, w),
    }
}

/// PRG-51: the held value of an item's attribute; a cell without an entry of its own for it
/// follows its fixture.
pub(crate) fn held_value(
    held: &BTreeMap<Item, BTreeMap<String, Value>>,
    item: Item,
    name: &str,
) -> Option<Value> {
    let own = held.get(&item).and_then(|a| a.get(name));
    let fixture = item
        .cell
        .and_then(|_| held.get(&Item::fixture(item.fixture))?.get(name));
    own.or(fixture).copied()
}

/// Where the LTP attributes of a playback's cue fade from (PBK-15, 17): the output of the tick
/// before the playback asserted them; `staggered`: the fixtures start one after the other in
/// user number order by the cue's overlap (PAL-14), each attribute by its fade (CUE-14).
fn origins(
    playback: &Playback,
    fixtures: &BTreeMap<FixtureId, Fixture>,
    profiles: &BTreeMap<String, Profile>,
    held: &BTreeMap<Item, BTreeMap<String, Value>>,
    staggered: bool,
) -> Origins {
    let mut out = Origins::new();
    let Some(values) = playback.values() else {
        return out;
    };
    // The stagger runs over the fixtures, a fixture's cells start with it, or by the cue's order.
    let ranks = stagger_ranks(
        playback.cues.first().map_or(&BTreeMap::new(), |c| &c.order),
        values
            .keys()
            .filter_map(|item| Some((fixtures.get(&item.fixture)?.number, *item))),
    );
    let overlap = playback.times().overlap;
    for (&id, &rank) in &ranks {
        let (Some(fixture), Some(attributes)) = (fixtures.get(&id.fixture), values.get(&id)) else {
            continue;
        };
        let Some(mode) = resolve_mode(profiles, fixture) else {
            continue;
        };
        for name in attributes.keys() {
            let Some(l) = mode
                .item_logical(id.cell, name)
                .filter(|l| l.kind == AttributeKind::Ltp)
            else {
                continue;
            };
            let value = held_value(held, id, l.name).unwrap_or(l.default);
            let (_, fade) = playback.ltp_times(l.group);
            let stagger = if staggered {
                fade * (1.0 - overlap / 100.0)
            } else {
                0.0
            };
            out.entry(id).or_default().insert(
                name.clone(),
                Origin {
                    value,
                    lag: rank as f32 * stagger,
                },
            );
        }
    }
    out
}

/// Merge one attribute of one fixture across all active playbacks and the programmer.
/// A palette reference is resolved here, every tick (PAL-11).
///
/// PRG-51: with `cell` the merge is that cell's. The sources see the fixture's value as the cell's
/// value; the programmer's entry for the cell beats its entry for the fixture. The virtual cell
/// Dimmer (PRG-47) is proposed by cell values only, never by a playback or a fixture value.
///
/// Returns `None` when no active source proposes a value. `tick` then writes 0
/// for an HTP attribute and the held value for an LTP one (ENG-07, LIB-03).
fn merge_attribute(
    src: &MergeSources,
    fixture: &Fixture,
    mode: &Mode,
    attr: &LogicalAttribute,
    now: Timestamp,
    cell: Option<u16>,
    // ENG-25: the losing HTP proposals go here when `explain` asks; the tick passes `None`.
    mut others: Option<&mut Vec<Candidate>>,
) -> Option<Candidate> {
    let mut best: Option<Candidate> = None;
    let own_only = cell.is_some_and(|c| attr.name == DIMMER && mode.virtual_cell_dimmer(c));
    // The items that speak for this attribute in every source, the most specific first.
    let items = (cell.map(|c| Item::cell(fixture.id, c)).into_iter())
        .chain((!own_only).then_some(Item::fixture(fixture.id)));
    // A reference gives what its palette resolves for the item, else its fallback (PAL-07,
    // PAL-11, PRG-52); a value of the wrong kind (a level for Colour, say) is ignored, not output.
    // ENG-25: the palette of the last reference resolved. ponytail: a fade between two
    // references reports the palette it resolved last (the target); fine for a label.
    let palette: std::cell::Cell<Option<PaletteId>> = std::cell::Cell::new(None);
    let resolve_for = |item: Item, stored: Stored| {
        palette.set(match stored {
            Stored::Reference(r) => Some(r.palette),
            Stored::Value(_) => None,
        });
        resolve_stored(src.palettes, item, fixture, mode, attr.name, stored)
    };

    for (id, playback) in src.playbacks.iter() {
        let Some(state) = src.states.get(id) else {
            continue;
        };
        let Some(activated) = state.activated else {
            continue;
        };
        // A cue list or chase proposes what its fades give now (CUE-13); anything else its first cue.
        palette.set(None);
        let raw = if is_list(playback) {
            items.clone().find_map(|item| {
                (state.run).fade_value(item, attr.name, now, |s| resolve_for(item, s))
            })
        } else {
            items.clone().find_map(|item| {
                let stored = (playback.values())?.get(&item)?.get(attr.name)?;
                Some((resolve_for(item, *stored), 0))
            })
        };
        let Some((raw, list_order)) = raw.filter(|(v, _)| v.same_kind(&attr.default)) else {
            continue;
        };

        let (value, order) = match attr.kind {
            // PBK-15..17: HTP scales with the intensity master. A playback at 0 (in its delay)
            // proposes nothing; a Swop silences every other playback (PBK-27).
            AttributeKind::Htp => {
                if state.master <= 0.0 || src.swop_owner.is_some_and(|owner| owner != *id) {
                    continue;
                }
                (raw.scaled(state.master), activated)
            }
            // CUE-15, 31: a list's LTP values are not scaled and not gated by the trigger level.
            AttributeKind::Ltp if is_list(playback) => (raw, list_order),
            AttributeKind::Ltp => {
                let from = items
                    .clone()
                    .find_map(|item| state.from.get(&item)?.get(attr.name));
                match playback.fader_mode {
                    // PBK-17: crossfades with the fader from where it was, from the fire.
                    FaderMode::Mode2 => (
                        from.map_or(raw, |o| o.value.lerp(raw, state.effective_level())),
                        activated,
                    ),
                    // PBK-15: asserted from the trigger point, fading in over the cue's times.
                    FaderMode::Mode0 | FaderMode::Mode1 => {
                        let Some(triggered) = state.triggered else {
                            continue;
                        };
                        let value = match (state.trigger_at, from) {
                            (Some(at), Some(o)) => {
                                let (delay, fade) = playback.ltp_times(attr.group); // CUE-14
                                let elapsed = (now - at) as f32 - delay - o.lag;
                                o.value.lerp(raw, fade_progress(elapsed, fade))
                            }
                            _ => raw,
                        };
                        (value, triggered)
                    }
                }
            }
        };

        let candidate = Candidate {
            priority: playback.priority,
            activated: order,
            value,
            via: Via {
                playback: Some((*id, is_list(playback))),
                palette: palette.get(),
                located: false,
            },
        };
        offer(&mut best, attr.kind, candidate, &mut others);
    }

    // PBK-47: a Released playback's HTP levels fall to 0 (the virtual cell Dimmer towards full,
    // PRG-47) over its release time, merging HTP; a Swop silences them (PBK-27).
    if attr.kind == AttributeKind::Htp {
        for (id, state) in src.states {
            if src.swop_owner.is_some_and(|owner| owner != *id) {
                continue;
            }
            for fall in &state.falls {
                let Some((from, _)) = items
                    .clone()
                    .find_map(|item| fall.entries.get(&item)?.get(attr.name))
                    .copied()
                else {
                    continue;
                };
                let to = if own_only {
                    attr.default
                } else {
                    Value::Level(0.0)
                };
                let candidate = Candidate {
                    priority: (src.playbacks.get(id)).map_or(Priority::Normal, |p| p.priority),
                    activated: 0,
                    value: from.lerp(to, fall.progress(now)),
                    via: Via {
                        playback: src.playbacks.get(id).map(|p| (*id, is_list(p))),
                        ..Via::default()
                    },
                };
                offer(&mut best, attr.kind, candidate, &mut others);
            }
        }
    }

    // ENG-04: manual values are absolute and sit at Priority::Programmer.
    palette.set(None);
    let proposal = items.clone().find_map(|item| {
        (src.programmer.stored_item(item, attr.name))
            .map(|s| resolve_for(item, s))
            .filter(|v| v.same_kind(&attr.default))
            .map(|to| match src.programmer.fade(item, attr.name) {
                Some(fade) => (fade.at(to, now), false), // PAL-13
                None => (to, false),
            })
            // PRG-25: a located value sits under the programmer value, at the same priority.
            .or_else(|| {
                let located = src.programmer.located_value(item, attr.name)?;
                Some((located, true)).filter(|(v, _)| v.same_kind(&attr.default))
            })
    });
    if let Some((value, located)) = proposal {
        let candidate = Candidate {
            priority: Priority::Programmer,
            activated: src.programmer.activated(),
            value,
            via: Via {
                playback: None,
                palette: palette.get().filter(|_| !located),
                located,
            },
        };
        offer(&mut best, attr.kind, candidate, &mut others);
    }

    // ENG-10: a value a timed Clear removed fades out. HTP: from its value to 0, the highest of it
    // and the other sources wins (ENG-01). LTP: from the output then to what the other sources
    // give, else the default. A source of higher priority owns the attribute outright.
    if let Some(fade) = items
        .clone()
        .find_map(|item| src.programmer.clearing(item, attr.name))
    {
        if !best
            .as_ref()
            .is_some_and(|c| c.priority > Priority::Programmer)
        {
            best = Some(match (attr.kind, best) {
                (AttributeKind::Htp, Some(c)) => {
                    let value = fade.at(Value::Level(0.0), now);
                    if value.level() > c.value.level() {
                        Candidate { value, ..c }
                    } else {
                        c
                    }
                }
                (AttributeKind::Ltp, Some(c)) => Candidate {
                    value: fade.at(c.value, now),
                    ..c
                },
                (kind, None) => Candidate {
                    priority: Priority::Programmer,
                    activated: src.programmer.activated(),
                    via: Via::default(),
                    value: match kind {
                        // PRG-47: the virtual cell Dimmer lets go towards full, its unproposed level.
                        AttributeKind::Htp if own_only => fade.at(attr.default, now),
                        AttributeKind::Htp => fade.at(Value::Level(0.0), now),
                        AttributeKind::Ltp => fade.at(attr.default, now),
                    },
                },
            });
        }
    }

    // PBK-20: a killed playback's LTP attribute fades from where it was to what is underneath:
    // the other sources, else the value from before the playback. A source of higher priority
    // owns the attribute outright. ponytail: several releases of one attribute chain in
    // playback id order; a timeline per attribute if that ever looks wrong on a console.
    if attr.kind == AttributeKind::Ltp {
        for (id, state) in src.states {
            let Some(release) = &state.release else {
                continue;
            };
            let Some((from, pre)) = items
                .clone()
                .find_map(|item| release.entries.get(&item)?.get(attr.name))
                .copied()
            else {
                continue;
            };
            let priority = src
                .playbacks
                .get(id)
                .map_or(Priority::Normal, |p| p.priority);
            if best.as_ref().is_some_and(|c| c.priority > priority) {
                continue;
            }
            let to = best.as_ref().map_or(pre, |c| c.value);
            let value = from.lerp(to, release.progress(now));
            best = Some(match best {
                Some(c) => Candidate { value, ..c },
                None => Candidate {
                    priority,
                    activated: 0,
                    value,
                    via: Via {
                        playback: src.playbacks.get(id).map(|p| (*id, is_list(p))),
                        ..Via::default()
                    },
                },
            });
        }
    }

    best
}

/// ENG-25: `candidate` against the best so far; an HTP loser is handed to `others`.
fn offer(
    best: &mut Option<Candidate>,
    kind: AttributeKind,
    candidate: Candidate,
    others: &mut Option<&mut Vec<Candidate>>,
) {
    let (won, lost) = match *best {
        None => (candidate, None),
        Some(current) => {
            let (won, lost) = pick(kind, current, candidate);
            (won, Some(lost))
        }
    };
    if let (AttributeKind::Htp, Some(lost), Some(others)) = (kind, lost, others) {
        others.push(lost);
    }
    *best = Some(won);
}

/// A stored value, or what its palette resolves for this fixture and attribute (PAL-07),
/// falling back to the reference's own value (PAL-11).
pub(crate) fn resolve_stored(
    palettes: &BTreeMap<PaletteId, Palette>,
    item: Item,
    fixture: &Fixture,
    mode: &Mode,
    attribute: &str,
    stored: Stored,
) -> Value {
    match stored {
        Stored::Value(v) => v,
        Stored::Reference(r) => palettes
            .get(&r.palette)
            .and_then(|p| p.resolve(item, fixture, mode, attribute))
            .unwrap_or(r.value),
    }
}

/// ENG-01..03: higher priority always wins; at equal priority HTP takes the
/// highest value and LTP the latest activation.
/// Returns the winner and the loser.
fn pick(kind: AttributeKind, current: Candidate, candidate: Candidate) -> (Candidate, Candidate) {
    let candidate_wins = if candidate.priority != current.priority {
        candidate.priority > current.priority
    } else {
        match kind {
            AttributeKind::Htp => candidate.value.level() > current.value.level(),
            AttributeKind::Ltp => candidate.activated > current.activated,
        }
    };
    if candidate_wins {
        (candidate, current)
    } else {
        (current, candidate)
    }
}

/// PRG-72: SplitMix64, the fixed generator behind Shuffle.
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// PRG-79, 96: the group a line names when it names exactly one, whole.
fn single_group(cmd: &command::Command) -> Option<GroupId> {
    match cmd.selection.as_slice() {
        [(false, i)] if i.kind == command::Kind::Group && i.from == i.to && i.cells.is_none() => {
            Some(i.from)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribute::{AttributeGroup, AttributeKind};

    fn dimmer_profile() -> Profile {
        Profile::from_attributes(vec![AttributeDef::new(
            "Dimmer",
            AttributeGroup::I,
            AttributeKind::Htp,
            1,
        )])
    }

    fn engine_with_dimmer() -> Engine {
        let mut e = Engine::new(Settings::default());
        assert_eq!(e.add_profile("dimmer", dimmer_profile()), Ok(()));
        let patched = e.patch(Fixture {
            parked: false,
            id: 1,
            number: 0,
            name: String::new(),
            handle: None,
            profile: "dimmer".into(),
            mode: None,
            universe: 1,
            address: 1,
        });
        assert_eq!(patched, Ok(()));
        e
    }

    #[test]
    fn patch_rejects_unknown_profile_without_panicking() {
        let mut e = Engine::new(Settings::default());
        let r = e.patch(Fixture {
            parked: false,
            id: 1,
            number: 0,
            name: String::new(),
            handle: None,
            profile: "nope".into(),
            mode: None,
            universe: 1,
            address: 1,
        });
        assert_eq!(r, Err(CoreError::UnknownProfile("nope".into())));
    }

    #[test]
    fn patch_rejects_address_past_universe_end() {
        let mut e = Engine::new(Settings::default());
        assert_eq!(e.add_profile("dimmer", dimmer_profile()), Ok(()));
        let r = e.patch(Fixture {
            parked: false,
            id: 1,
            number: 0,
            name: String::new(),
            handle: None,
            profile: "dimmer".into(),
            mode: None,
            universe: 1,
            address: 513,
        });
        assert!(matches!(r, Err(CoreError::AddressOutOfRange { .. })));
    }

    #[test]
    fn eng_12_bad_level_is_rejected_and_frame_survives() {
        let mut e = engine_with_dimmer();
        e.add_playback(Playback::new(
            1,
            1,
            1,
            BTreeMap::from([(
                Item::fixture(1),
                BTreeMap::from([("Dimmer".to_string(), 1.0.into())]),
            )]),
        ));
        assert_eq!(e.set_fader(1, 1.0, 0.0), Ok(()));
        e.tick(0.025);
        assert_eq!(e.frame(1).and_then(|f| f.channel(1)), Some(255));
        assert_eq!(e.set_fader(1, 7.0, 0.05), Err(CoreError::InvalidLevel(7.0)));
        e.tick(0.05);
        assert_eq!(e.frame(1).and_then(|f| f.channel(1)), Some(255));
    }

    #[test]
    fn frame_channel_out_of_range_is_none_not_panic() {
        let f = Frame::default();
        assert_eq!(f.channel(0), None);
        assert_eq!(f.channel(513), None);
        assert_eq!(f.channel(512), Some(0));
    }
}
