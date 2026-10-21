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
//! Not yet here, by design (each lands with its spec): the programmer's Clear time (ENG-10),
//! chases, masters other than GM.

use std::collections::{BTreeMap, BTreeSet};

use crate::attribute::{AttributeDef, AttributeGroup, AttributeKind};
use crate::colour::{ColourModel, TargetColour};
use crate::command::{self, Item, Kind};
use crate::cuelist::{Ctx, Env, ListRun};
use crate::palette::{
    self, Cell, Detected, Entry, Limit, Palette, PaletteId, Record, RecordBy, RecordMode, Timing,
};
use crate::patch::{Fixture, FixtureId, Handle, PatchAdd};
use crate::playback::{
    check_bpm, fade_progress, AddCue, BpmFader, BpmMaster, Button, ButtonKind, ChaseOptions, Cue,
    CueChange, CueNumber, CueValue, CueView, FaderMode, GroupMaster, GroupMode, GroupTime,
    HandleButton, HandleButtons, KillPoint, Kind as PlaybackKind, Link, ListAction, ListFader,
    ListLive, Master, MasterKind, Origin, Origins, Paging, Playback, PlaybackChange, PlaybackId,
    PlaybackRecord, PlaybackState, Priority, Release, SpeedSource, UpdateMode, Values,
    GROUP_SCALES, PAGES,
};
use crate::profile::{ColourMixing, ColourSystem, LogicalAttribute, Mode, Profile, COLOUR, DIMMER};
use crate::programmer::{
    AttributeGroupState, ColourView, Fade, FixtureValues, GroupId, PaletteUse, Programmer,
    ProgrammerView, Resolution, WheelState,
};
use crate::show::{
    Meta, Show, ShowGroup, ShowPage, ShowPalette, ShowPalettes, ShowPatch, ShowPlaybacks,
    ShowSettings, SCHEMA_VERSION,
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
    pub cue_fade_out: f32,
    pub cue_overlap: f32,
    /// The global release mask and time (PBK-20).
    pub release_mask: Vec<AttributeGroup>,
    pub release_time: f32,
    /// CUE-26, CUE-15, CUE-52, CUE-25.
    pub auto_connect: bool,
    pub cue_list_fader: ListFader,
    pub update_mode: UpdateMode,
    pub handle_buttons: HandleButtons,
}

/// PBK-12, PBK-20 (`settings.playback`): every field optional, all or nothing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlaybackSettingsChange {
    pub record_by: Option<RecordBy>,
    pub delay: Option<f32>,
    pub fade_in: Option<f32>,
    pub fade_out: Option<f32>,
    pub overlap: Option<f32>,
    pub release_mask: Option<Vec<AttributeGroup>>,
    pub release_time: Option<f32>,
    pub auto_connect: Option<bool>,
    pub cue_list_fader: Option<ListFader>,
    pub update_mode: Option<UpdateMode>,
    pub handle_buttons: Option<HandleButtons>,
}

/// PBK-09: the default record mask, every group.
const ALL_GROUPS: [AttributeGroup; 8] = [
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
            cue_fade_out: 0.0,
            cue_overlap: 100.0,
            release_mask: Vec::new(),
            release_time: 2.0,
            auto_connect: true,
            cue_list_fader: ListFader::KillWithOff,
            update_mode: UpdateMode::Forwards,
            handle_buttons: HandleButtons::BestPractice,
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
    frames: BTreeMap<UniverseId, Frame>,
    warnings: Vec<Warning>,
    last_tick: Option<Timestamp>,
    programmer: Programmer,
    groups: BTreeMap<GroupId, Vec<FixtureId>>,
    palettes: BTreeMap<PaletteId, Palette>,
    /// Above every palette id ever used, so ids are never reused (PAL, Definitions).
    palette_next_id: PaletteId,
    /// Attribute group the wheels show (PRG-17/18).
    active_group: AttributeGroup,
    /// How the Colour wheels are shown (PRG-17, PRG-30).
    colour_model: ColourModel,
    /// Last value of every LTP logical attribute per fixture: what it outputs while
    /// nothing proposes it, and where a wheel starts (LIB-03, PRG-20).
    held: BTreeMap<FixtureId, BTreeMap<String, Value>>,
    /// CUE-26: the list the Go block acts on. Live state, not in the show.
    connected: Option<PlaybackId>,
}

/// PAL-08: what applying a palette puts in the programmer: fixture, attribute, resolved value.
type PalettePlan = Vec<(FixtureId, String, Value)>;

/// A candidate value for one attribute from one source, during a merge.
struct Candidate {
    priority: Priority,
    activated: u64,
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
            frames: BTreeMap::new(),
            warnings: Vec::new(),
            last_tick: None,
            programmer: Programmer::default(),
            groups: BTreeMap::new(),
            palettes: BTreeMap::new(),
            palette_next_id: 1,
            active_group: AttributeGroup::I,
            colour_model: ColourModel::default(),
            held: BTreeMap::new(),
            connected: None,
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
        self.check_place(
            fixture.number,
            fixture.universe,
            u32::from(fixture.address),
            footprint,
            None,
        )?;
        if self.numbers.contains_key(&fixture.number) {
            return Err(CoreError::NumberInUse(fixture.number));
        }
        self.install(fixture)
    }

    /// PAT-01..03, 07: patch `count` fixtures back to back. The whole placement is computed and
    /// checked first; a refused add changes nothing. Returns the new ids in patch order.
    pub fn patch_add(&mut self, add: &PatchAdd) -> Result<Vec<FixtureId>, CoreError> {
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
            self.check_place(number, add.universe, address, footprint, None)?;
            if self.numbers.contains_key(&number) {
                return Err(CoreError::NumberInUse(number));
            }
            placed.push((first_id + i, number, address));
            address += u32::from(footprint);
        }
        let mut ids = Vec::new();
        for (id, number, address) in placed {
            self.install(Fixture {
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
        Ok(ids)
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
            Some(id),
        )?;
        let (old_universe, old_address) = (fixture.universe, fixture.address);
        self.clear_channels(old_universe, old_address, footprint);
        self.frames.entry(universe).or_default();
        if let Some(f) = self.fixtures.get_mut(&id) {
            f.universe = universe;
            f.address = address;
        }
        Ok(())
    }

    /// PAT-05: remove fixtures from the patch, the selection, the programmer, every group and
    /// every palette (PAL-19) and every cue of every playback (PBK-14).
    /// All ids are checked first.
    pub fn patch_remove(&mut self, ids: &[FixtureId]) -> Result<(), CoreError> {
        if let Some(&unknown) = ids.iter().find(|id| !self.fixtures.contains_key(id)) {
            return Err(CoreError::UnknownFixture(unknown));
        }
        for &id in ids {
            let footprint = self
                .fixtures
                .get(&id)
                .and_then(|f| self.footprint_of(f).ok())
                .unwrap_or(0);
            let Some(fixture) = self.fixtures.remove(&id) else {
                continue; // listed twice
            };
            self.clear_channels(fixture.universe, fixture.address, footprint);
            self.numbers.remove(&fixture.number);
            self.held.remove(&id);
            self.programmer.remove_fixture(id);
            for members in self.groups.values_mut() {
                members.retain(|&f| f != id);
            }
            for palette in self.palettes.values_mut() {
                palette.fixtures.remove(&id); // PAL-19
            }
            for cue in self.playbacks.values_mut().flat_map(|pb| &mut pb.cues) {
                cue.values.remove(&id); // PBK-14
            }
        }
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
    /// held values of attributes that remain are kept. A bigger footprint is checked like a move.
    pub fn patch_change_mode(&mut self, id: FixtureId, mode: &str) -> Result<(), CoreError> {
        let fixture = self
            .fixtures
            .get(&id)
            .ok_or(CoreError::UnknownFixture(id))?;
        let old_footprint = self.footprint_of(fixture)?;
        let new = lookup_mode(&self.profiles, &fixture.profile, Some(mode))?;
        self.check_place(
            fixture.number,
            fixture.universe,
            u32::from(fixture.address),
            new.footprint,
            Some(id),
        )?;
        let (number, universe, address) = (fixture.number, fixture.universe, fixture.address);
        // A value stays only when the new mode has that attribute with the same kind of value.
        let keeps = |name: &str, value: &Stored| {
            new.logical(name)
                .is_some_and(|l| l.default.same_kind(&value.value()))
        };
        let mut warnings: Vec<String> = Vec::new();
        let mut dropped_programmer = Vec::new();
        for (name, value) in self.programmer.values().get(&id).into_iter().flatten() {
            if !keeps(name, value) {
                dropped_programmer.push(name.clone());
                warnings.push(format!(
                    "fixture {number}: programmer value '{name}' dropped, mode '{mode}' has no such attribute"
                ));
            }
        }
        for playback in self.playbacks.values_mut() {
            for cue in &mut playback.cues {
                if let Some(values) = cue.values.get_mut(&id) {
                    values.retain(|name, value| {
                        let keep = keeps(name, value);
                        if !keep {
                            warnings.push(format!(
                                "fixture {number}: playback {} value '{name}' dropped, mode '{mode}' has no such attribute",
                                playback.id
                            ));
                        }
                        keep
                    });
                }
            }
        }
        let old_held = self.held.remove(&id).unwrap_or_default();
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
            self.programmer.remove_value(id, &name);
        }
        self.held.insert(id, held);
        self.clear_channels(universe, address, old_footprint);
        if let Some(f) = self.fixtures.get_mut(&id) {
            f.mode = Some(mode.to_string());
        }
        let at = self.last_tick.unwrap_or(0.0);
        self.warnings
            .extend(warnings.into_iter().map(|message| Warning { at, message }));
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
        let mode = lookup_mode(&self.profiles, &fixture.profile, fixture.mode.as_deref())?;
        if fixture.handle.is_none() {
            fixture.handle = self.first_free_handle();
        }
        let frame = self.frames.entry(fixture.universe).or_default();
        let held = self.held.entry(fixture.id).or_default();
        held.clear();
        // A virtual dimmer starts dark: its Dimmer is HTP, so nothing proposes it yet.
        let scale = if mode.has_virtual_dimmer() { 0.0 } else { 1.0 };
        for l in mode
            .logical_attributes()
            .filter(|l| l.kind == AttributeKind::Ltp)
        {
            held.insert(l.name.to_string(), l.default);
            write_value(frame, fixture.address, mode, &l, l.default, scale);
        }
        self.numbers.insert(fixture.number, fixture.id);
        self.next_id = self.next_id.max(fixture.id.saturating_add(1));
        self.fixtures.insert(fixture.id, fixture);
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
    fn spans(&self, universe: UniverseId, own: Option<FixtureId>) -> Vec<(u32, u32, u32)> {
        self.fixtures
            .values()
            .filter(|f| f.universe == universe && Some(f.id) != own)
            .filter_map(|f| {
                let first = u32::from(f.address);
                let width = u32::from(self.footprint_of(f).ok()?);
                Some((first, (first + width).saturating_sub(1), f.number))
            })
            .collect()
    }

    /// PAT-02: `footprint` channels from `address` must lie within 1..=512 and clear every other
    /// fixture; the error names the first clashing channel.
    fn check_place(
        &self,
        number: u32,
        universe: UniverseId,
        address: u32,
        footprint: u16,
        own: Option<FixtureId>,
    ) -> Result<(), CoreError> {
        let at = u16::try_from(address).unwrap_or(u16::MAX);
        let end = address.saturating_add(u32::from(footprint)); // first channel after
        if address == 0 || end > UNIVERSE_SIZE as u32 + 1 {
            return Err(CoreError::AddressOutOfRange {
                fixture: number,
                universe,
                address: at,
            });
        }
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
        let mut spans = self.spans(universe, None);
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
    pub fn add_playback(&mut self, playback: Playback) {
        self.playback_next_id = self.playback_next_id.max(playback.id.saturating_add(1));
        let state = self.states.entry(playback.id).or_default();
        if let Some(Master::Group(m)) = &playback.master {
            state.level = m.level; // GRM-05: the fader starts at the saved level
        }
        self.playbacks.insert(playback.id, playback);
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
                cue_overlap: s.cue_overlap,
                release_mask: s.release_mask.clone(),
                release_time: s.release_time,
                auto_connect: s.auto_connect,
                cue_list_fader: s.cue_list_fader,
                update_mode: s.update_mode,
                handle_buttons: s.handle_buttons.clone(),
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
                .map(|(&id, fixtures)| ShowGroup {
                    id,
                    fixtures: fixtures.clone(),
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
                            cue.values.retain(|id, _| self.fixtures.contains_key(id));
                        }
                        pb
                    })
                    .collect(),
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
            cue_overlap: s.cue_overlap,
            release_mask: s.release_mask.clone(),
            release_time: s.release_time,
            auto_connect: s.auto_connect,
            cue_list_fader: s.cue_list_fader,
            update_mode: s.update_mode,
            handle_buttons: s.handle_buttons.clone(),
        });
        engine.colour_model = s.colour_model;
        let mut warnings = Vec::new();
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
            for &id in &group.fixtures {
                if engine.fixtures.contains_key(&id) {
                    members.push(id);
                } else if !in_file.contains(&id) {
                    warnings.push(format!(
                        "group {}: fixture {id} is not in the patch, left out",
                        group.id
                    ));
                }
            }
            engine.groups.insert(group.id, members);
        }
        for entry in &show.palettes.palettes {
            let mut palette = match entry {
                ShowPalette::Palette(p) => p.clone(),
                ShowPalette::Unreadable(v) => {
                    warnings.push(format!("palette entry left out, not readable: {v}"));
                    continue;
                }
            };
            palette.fixtures.retain(|&id, _| {
                let keep = engine.fixtures.contains_key(&id);
                if !keep && !in_file.contains(&id) {
                    warnings.push(format!(
                        "palette {}: fixture {id} is not in the patch, its values are left out",
                        palette.number
                    ));
                }
                keep
            });
            let number = palette.number;
            if let Err(e) = engine.add_palette(palette) {
                warnings.push(format!("palette {number} left out: {e}"));
            }
        }
        engine.palette_next_id = engine.palette_next_id.max(show.palettes.next_id);
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
                ("fade out", &mut cue.fade_out, 600.0, 0.0),
                ("overlap", &mut cue.overlap, 100.0, 100.0),
            ] {
                if !(0.0..=max).contains(value) {
                    warnings.push(format!(
                        "playback {id}: {name} {value} is out of range, set to {default}"
                    ));
                    *value = default;
                }
            }
            cue.values.retain(|&fixture, _| {
                let keep = self.fixtures.contains_key(&fixture);
                if !keep && !in_file.contains(&fixture) {
                    warnings.push(format!(
                        "playback {id}: fixture {fixture} is not in the patch, its values are left out"
                    ));
                }
                keep
            });
            for stored in cue.values.values_mut().flat_map(|a| a.values_mut()) {
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

    /// PBK-22, 23: kill the playback at once, ignoring `fade_out` and the kill point, and
    /// release every LTP attribute it holds over its release time. A fader still up has to go
    /// to 0 before it fires again. An inactive playback is left alone.
    pub fn release_playback(&mut self, id: PlaybackId, _now: Timestamp) -> Result<(), CoreError> {
        self.live_state(id)?;
        self.kill_playback(id, true, None);
        Ok(())
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
        let series = &mut state.taps;
        if series
            .last()
            .is_some_and(|&last| !(0.0..=2.0).contains(&(at - last)))
        {
            series.clear();
        }
        series.push(at);
        series.drain(..series.len().saturating_sub(4));
        let (Some(&first), Some(&last)) = (series.first(), series.last()) else {
            return Ok(());
        };
        let intervals = series.len().saturating_sub(1);
        if intervals == 0 {
            return Ok(());
        }
        let mean = (last - first) / intervals as f64;
        let bpm = ((60.0 / mean) as f32).clamp(1.0, 600.0);
        self.set_tempo(id, (bpm * 10.0).round() / 10.0, Some(at))
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

    /// CUE-29: a source whose master does not exist runs as `Free`.
    fn chase_bpm(&self, chase: &ChaseOptions) -> f32 {
        let (kind, number) = match chase.speed_source {
            SpeedSource::Free => return chase.bpm,
            SpeedSource::Bpm(n) => (MasterKind::Bpm, n),
            SpeedSource::Rate(n) => (MasterKind::Rate, n),
        };
        match self.find_master(kind, number) {
            Some((id, Master::Bpm(m))) => self.master_tempo(id, m),
            Some((id, Master::Rate(m))) => {
                let level = self.states.get(&id).map_or(0.0, |s| s.level);
                chase.bpm * (level * f32::from(m.rate_scale) / 100.0) // CUE-36
            }
            None | Some((_, Master::Group(_))) => chase.bpm,
        }
    }

    /// CUE-35: the BPM master's tempo: its fader's or its `bpm`, times its multiplier (or the
    /// fader's zone).
    fn master_tempo(&self, id: PlaybackId, m: &BpmMaster) -> f32 {
        let state = self.states.get(&id);
        let level = state.map_or(0.0, |s| s.level);
        let (bpm, multiplier) = match m.fader {
            BpmFader::Bpm if state.is_some_and(|s| s.fader_moved) => {
                (level * m.fader_max, m.multiplier)
            }
            BpmFader::Multiplier => {
                let zone = ((level * 5.0) as usize).min(4); // /4 /2 ×1 ×2 ×4
                (
                    m.bpm,
                    [0.25, 0.5, 1.0, 2.0, 4.0].get(zone).copied().unwrap_or(1.0),
                )
            }
            _ => (m.bpm, m.multiplier),
        };
        bpm * multiplier
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
        let mut entries: BTreeMap<FixtureId, BTreeMap<String, (Value, Value)>> = BTreeMap::new();
        for (fixture, attributes) in &state.from {
            for (name, origin) in attributes {
                if !all && !mask.contains(&self.attribute_group(name)) {
                    continue;
                }
                let current = (self.held.get(fixture))
                    .and_then(|held| held.get(name))
                    .copied()
                    .unwrap_or(origin.value);
                entries
                    .entry(*fixture)
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
            change.fade_out,
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
        for (&id, attrs) in &mut values {
            for (name, stored) in attrs {
                if let Stored::Reference(r) = stored {
                    r.value = self.resolve_stored(id, name, Stored::Reference(*r));
                }
            }
        }
        if record.record_by.unwrap_or(self.settings.cue_record_by) == RecordBy::Mixed {
            let mut plain: BTreeMap<FixtureId, BTreeMap<String, Value>> = values
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
        if values.is_empty() {
            return Err(CoreError::NothingToRecordPlayback);
        }
        let Some(id) = existing else {
            let number = record.cue.unwrap_or_default();
            let mut playback = Playback::new(self.playback_next_id, page, slot, Values::new());
            playback.cues = vec![self.new_cue(values, number)];
            let id = playback.id;
            self.add_playback(playback);
            return Ok(id);
        };
        if let Some(number) = new_number {
            // CUE-15: a playback that becomes a cue list takes the setting.
            let fader = self.settings.cue_list_fader;
            let cue = self.new_cue(values, number);
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
            let tracked = tracking && !matches!(record.mode, Some(RecordMode::Replace) | None);
            if let (true, Some(at)) = (tracked, target) {
                // CUE-52: each attribute goes where the update mode says.
                let mode = record.update_mode.unwrap_or(self.settings.update_mode);
                let (fixtures, profiles) = (&self.fixtures, &self.profiles);
                let htp = |fixture: FixtureId, name: &str| {
                    let mode = resolve_mode(profiles, fixtures.get(&fixture)?)?;
                    Some(mode.logical(name)?.kind == AttributeKind::Htp)
                };
                playback.store_tracked(at, values, mode, htp);
            } else if let Some(cue) = target.and_then(|at| playback.cues.get_mut(at)) {
                match record.mode {
                    Some(RecordMode::Replace) | None => cue.values = values,
                    Some(RecordMode::Merge | RecordMode::QuickMerge) => {
                        for (fixture, attrs) in values {
                            cue.values.entry(fixture).or_default().extend(attrs);
                        }
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

    /// PBK-12: a new cue with the times of the settings and link `Wait`.
    fn new_cue(&self, values: Values, number: CueNumber) -> Cue {
        let s = &self.settings;
        Cue {
            delay: s.cue_delay,
            fade_in: s.cue_fade_in,
            fade_out: s.cue_fade_out,
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

    /// CUE-54: take a hard value (or all of a fixture's) out of a cue so it tracks from earlier
    /// cues again; refused, changing nothing, when the cue does not hold it. On stage at once
    /// (CUE-53).
    pub fn remove_cue_value(
        &mut self,
        id: PlaybackId,
        cue: CueNumber,
        fixture: FixtureId,
        attribute: Option<&str>,
    ) -> Result<(), CoreError> {
        let playback = self.playback_mut(id)?;
        let cue = (playback.cues.iter_mut())
            .find(|c| c.number == cue)
            .ok_or(CoreError::UnknownCue(cue))?;
        let held = cue.values.get_mut(&fixture);
        let removed = match (held, attribute) {
            (Some(_), None) => cue.values.remove(&fixture).is_some(),
            (Some(attributes), Some(name)) => {
                let removed = attributes.remove(name).is_some();
                if attributes.is_empty() {
                    cue.values.remove(&fixture);
                }
                removed
            }
            (None, _) => false,
        };
        if !removed {
            let what = attribute.map_or(format!("fixture {fixture}"), str::to_string);
            return Err(CoreError::InvalidCommand(format!(
                "Q{} holds no {what} of its own",
                cue.number
            )));
        }
        if let Some(state) = self.states.get_mut(&id) {
            state.run.edited = true;
        }
        Ok(())
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
        for time in [change.delay, change.fade_in, change.fade_out]
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
            change.fade_out,
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
                    Master::Group(GroupMaster {
                        level: if new.enabled && !old.enabled {
                            fader
                        } else {
                            old.level
                        },
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
            .map(|(&fixture, attrs)| {
                (attrs.iter())
                    .map(|(name, stored)| {
                        (name.clone(), self.resolve_stored(fixture, name, *stored))
                    })
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
        if !self.fixtures.contains_key(&id) {
            return Err(CoreError::UnknownFixture(id));
        }
        self.programmer.toggle(id);
        self.follow_active_group();
        Ok(())
    }

    /// PRG-02/03: append a group's fixtures, keeping the ones already selected in place;
    /// a fully selected group is removed instead (IDEA-11). Typed `Group n` replaces, see PRG-08.
    pub fn select_group(&mut self, id: GroupId) -> Result<(), CoreError> {
        let members = self.groups.get(&id).ok_or(CoreError::UnknownGroup(id))?;
        self.programmer.add_group(members);
        self.follow_active_group();
        Ok(())
    }

    pub fn clear_selection(&mut self) {
        self.programmer.clear_selection();
        self.follow_active_group();
    }

    /// PRG-29 (default `SelectionThenProgrammer`): the first Clear deselects, the next
    /// empties the programmer.
    pub fn clear(&mut self) {
        if self.programmer.selection().is_empty() {
            self.programmer.clear_values();
        } else {
            self.clear_selection();
        }
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

    /// PRG-05: split the selection into `n` interleaved stages.
    pub fn select_pattern(&mut self, n: usize) -> Result<(), CoreError> {
        if n < 2 {
            return Err(CoreError::InvalidPattern(n));
        }
        self.programmer.set_pattern(n);
        self.follow_active_group();
        Ok(())
    }

    /// PRG-06.
    pub fn select_all(&mut self) {
        self.programmer.select_all();
        self.follow_active_group();
    }

    /// PRG-07: store the selection as a group, replacing any existing one.
    pub fn record_group(&mut self, id: GroupId) -> Result<(), CoreError> {
        let selection = self.programmer.selection();
        if selection.is_empty() {
            return Err(CoreError::EmptySelection);
        }
        self.groups.insert(id, selection.to_vec());
        Ok(())
    }

    /// Set a manual value; it joins the merge at `Priority::Programmer` (PRG-12/13).
    pub fn set_programmer_value(
        &mut self,
        fixture: FixtureId,
        attribute: &str,
        value: impl Into<Value>,
    ) -> Result<(), CoreError> {
        let value = value.into();
        let fx = self
            .fixtures
            .get(&fixture)
            .ok_or(CoreError::UnknownFixture(fixture))?;
        let mode = self
            .mode_of(fixture)
            .ok_or_else(|| CoreError::UnknownProfile(fx.profile.clone()))?;
        let Some(logical) = mode.logical(attribute) else {
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
        self.programmer.set_value(
            fixture,
            attribute,
            Stored::Value(value),
            self.activation_seq,
        );
        Ok(())
    }

    /// PRG-18: the attribute group the wheels show; always accepted.
    pub fn active_group(&self) -> AttributeGroup {
        self.active_group
    }

    pub fn set_active_group(&mut self, group: AttributeGroup) {
        self.active_group = group;
    }

    /// PRG-18: after a selection change, leave a group no target has for the
    /// first one (I P C G B E S) some target has. Stored, not computed lazily.
    fn follow_active_group(&mut self) {
        let targets = self.programmer.targets();
        let has = |g: AttributeGroup| {
            targets.iter().any(|id| {
                self.mode_of(*id)
                    .is_some_and(|m| m.logical_attributes().any(|l| l.group == g))
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
        let mut names: Vec<String> = Vec::new();
        let mut colour = false;
        for id in self.programmer.targets() {
            for l in self.mode_of(id).iter().flat_map(|m| m.logical_attributes()) {
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
        let targets = self.programmer.targets();
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
        let mut new = Vec::new();
        for id in targets {
            let Some(l) = self.mode_of(id).and_then(|m| m.logical(name)) else {
                continue;
            };
            let moved = match (colour_wheel, self.start_value(id, &l)) {
                (true, Value::Colour(c)) => self
                    .colour_model
                    .turn(attribute, c, delta)
                    .map(Value::Colour),
                (false, Value::Level(v)) => Some(Value::Level((v + delta).clamp(0.0, 1.0))),
                _ => None,
            };
            if let Some(value) = moved {
                new.push((id, value));
            }
        }
        if new.is_empty() {
            return Err(CoreError::AttributeNotInTargets(attribute.to_string()));
        }
        for (id, value) in new {
            self.set_programmer_value(id, name, value)?;
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
        let targets = self.programmer.targets();
        if targets.is_empty() {
            return Err(CoreError::EmptySelection);
        }
        let mut new = Vec::new();
        for id in targets {
            if let Some(l) = self.mode_of(id).and_then(|m| m.logical(COLOUR)) {
                if let Value::Colour(c) = self.start_value(id, &l) {
                    let (_, _, v) = c.to_hsv();
                    new.push((id, TargetColour::from_hsv(hue, saturation, v)));
                }
            }
        }
        if new.is_empty() {
            return Err(CoreError::AttributeNotInTargets(COLOUR.to_string()));
        }
        for (id, c) in new {
            self.set_programmer_value(id, COLOUR, c)?;
        }
        Ok(())
    }

    /// PRG-20: programmer value, else what is on the output, else the held value.
    fn start_value(&self, id: FixtureId, l: &LogicalAttribute) -> Value {
        self.programmer
            .value(id, l.name)
            .or_else(|| {
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
                )
            })
            .or_else(|| self.held.get(&id)?.get(l.name).copied())
            .unwrap_or(match l.kind {
                AttributeKind::Htp => Value::Level(0.0),
                AttributeKind::Ltp => l.default,
            })
    }

    /// PRG-35 / IPC-11: the programmer state for rendering.
    pub fn programmer_view(&self) -> ProgrammerView {
        use AttributeGroup::*;
        let targets = self.programmer.targets();
        let attribute_groups = [I, P, C, G, B, E, S, Fx]
            .into_iter()
            .map(|group| AttributeGroupState {
                group,
                available: targets.iter().any(|&id| {
                    self.mode_of(id)
                        .is_some_and(|m| m.logical_attributes().any(|l| l.group == group))
                }),
                has_values: targets.iter().any(|&id| {
                    let values = self.programmer.values().get(&id);
                    values.zip(self.mode_of(id)).is_some_and(|(v, m)| {
                        v.keys()
                            .any(|n| m.logical(n).is_some_and(|l| l.group == group))
                    })
                }),
            })
            .collect();
        // Per wheel: the first target that has the attribute speaks for all.
        let wheels = self
            .wheel_layout()
            .into_iter()
            .filter_map(|attribute| {
                let colour_wheel = self.colour_model.wheels().contains(&attribute.as_str());
                let name = if colour_wheel {
                    COLOUR
                } else {
                    attribute.as_str()
                };
                let (id, l) = targets
                    .iter()
                    .find_map(|&id| Some((id, self.mode_of(id)?.logical(name)?)))?;
                let start = self.start_value(id, &l);
                let value = match start {
                    Value::Colour(c) => self.colour_model.wheel_value(&attribute, c)?,
                    Value::Level(v) => v,
                };
                let stored = self.programmer.stored(id, name);
                Some(WheelState {
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
        let colour = targets.iter().find_map(|&id| {
            let l = self.mode_of(id)?.logical(COLOUR)?;
            let Value::Colour(c) = self.start_value(id, &l) else {
                return None;
            };
            let (hue, saturation, _) = c.to_hsv();
            Some(ColourView {
                hue,
                saturation,
                r: c.r,
                g: c.g,
                b: c.b,
            })
        });
        let palettes = self.palette_use(&targets);
        ProgrammerView {
            selection: self.programmer.selection().to_vec(),
            targets,
            active_group: self.active_group,
            attribute_groups,
            wheels,
            in_programmer: self.programmer.values().keys().copied().collect(),
            colour_model: self.colour_model,
            contents: (self.programmer.values().iter())
                .map(|(&fixture, values)| FixtureValues {
                    fixture,
                    values: values
                        .iter()
                        .map(|(name, stored)| {
                            (name.clone(), self.resolve_stored(fixture, name, *stored))
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
                            let fade = self.programmer.fade(fixture, name)?;
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
        }
    }

    /// PAL-26: palettes referenced in the programmer, and those that resolve something for the
    /// targets (all of them when nothing is selected).
    fn palette_use(&self, targets: &[FixtureId]) -> PaletteUse {
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
            .filter(|p| targets.is_empty() || self.palette_plan(p.number, targets).is_ok())
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
        for (id, attrs) in self.programmer.values() {
            let Some(mode) = self.mode_of(*id) else {
                continue;
            };
            for (name, value) in attrs {
                if mode.logical(name).is_some_and(|l| mask.contains(&l.group)) {
                    out.entry(*id).or_default().insert(name.clone(), *value);
                }
            }
        }
        out
    }

    /// PRG-08/09/11: run one command line. Parse and resolve first, then apply,
    /// so a refused line changes nothing.
    pub fn execute_command(&mut self, tokens: &[&str]) -> Result<(), CoreError> {
        let cmd = command::parse(tokens)?;
        let typed = if cmd.selection.is_empty() {
            None
        } else {
            let mut list: Vec<FixtureId> = Vec::new();
            for (not, item) in &cmd.selection {
                let ids = self.expand(item);
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
                return Err(CoreError::EmptySelection);
            }
            Some(list)
        };
        if let Some(number) = cmd.palette {
            // PAL-10: plan against the typed selection first, so a refusal changes nothing.
            let (time, overlap) = self.palette_timing(Timing::default())?;
            let targets = typed.clone().unwrap_or_else(|| self.programmer.targets());
            let (id, plan) = self.palette_plan(number, &targets)?;
            if let Some(list) = typed {
                self.programmer.replace_selection(list);
                self.follow_active_group();
            }
            self.put_palette_timed(id, plan, time, overlap);
            return Ok(());
        }
        let mut touched = Vec::new();
        let level = cmd.level.map(|v| (v / 100.0).clamp(0.0, 1.0));
        if level.is_some() {
            let targets = typed.clone().unwrap_or_else(|| self.programmer.targets());
            if targets.is_empty() {
                return Err(CoreError::EmptySelection);
            }
            for id in targets {
                for l in self.mode_of(id).iter().flat_map(|m| m.logical_attributes()) {
                    if l.group == AttributeGroup::I && l.kind == AttributeKind::Htp {
                        touched.push((id, l.name.to_string()));
                    }
                }
            }
        }
        if let Some(list) = typed {
            self.programmer.replace_selection(list);
            self.follow_active_group();
        }
        if let Some(level) = level {
            for (id, name) in touched {
                self.set_programmer_value(id, &name, level)?;
            }
        }
        Ok(())
    }

    /// Patched fixtures (user numbers, PAT-12) or members of existing groups (ids) named by one
    /// item, in typed order. Ranges walk only the entries that exist, so `1 Thro 4000000000`
    /// stays cheap.
    fn expand(&self, item: &Item) -> Vec<FixtureId> {
        let (lo, hi) = (item.from.min(item.to), item.from.max(item.to));
        let mut keys: Vec<u32> = match item.kind {
            Kind::Fixture => self.numbers.range(lo..=hi).map(|(_, id)| *id).collect(),
            Kind::Group => self.groups.range(lo..=hi).map(|(id, _)| *id).collect(),
        };
        if item.from > item.to {
            keys.reverse();
        }
        match item.kind {
            Kind::Fixture => keys,
            Kind::Group => keys
                .iter()
                .filter_map(|g| self.groups.get(g))
                .flatten()
                .copied()
                .collect(),
        }
    }

    // ---- palettes (PAL-01..12, 17..21) --------------------------------------

    /// PAL-23, scenario files and tests: place a palette as it is. Refused (nothing changes) for
    /// an id or number in use, a number outside 1–9999 or a cell in use.
    pub fn add_palette(&mut self, palette: Palette) -> Result<(), CoreError> {
        if palette.id == 0 || self.palettes.contains_key(&palette.id) {
            return Err(CoreError::DuplicatePalette(palette.id));
        }
        check_palette_number(palette.number)?;
        if self.palette(palette.number).is_some() {
            return Err(CoreError::PaletteNumberInUse(palette.number));
        }
        self.check_free_cell(palette.cell())?;
        self.palette_next_id = self.palette_next_id.max(palette.id.saturating_add(1));
        self.palettes.insert(palette.id, palette);
        Ok(())
    }

    /// PAL-01: record the programmer's contents in the mask onto a cell. An empty cell gets a
    /// new palette; a used one needs `mode` (PAL-06). Refused records change nothing.
    pub fn record_palette(&mut self, record: &Record) -> Result<PaletteId, CoreError> {
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
        if record.record_by.unwrap_or(self.settings.record_by) == RecordBy::Mixed {
            self.mix_groups(&mut values);
        }
        let mode = existing.and(record.mode);
        if let (Some((id, ..)), Some(RecordMode::QuickMerge)) = (existing, mode) {
            if let Some(palette) = self.palettes.get(&id) {
                for attrs in values.values_mut() {
                    attrs.retain(|name, _| palette.holds(name));
                }
                values.retain(|_, attrs| !attrs.is_empty());
            }
        }
        if values.is_empty() {
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
            let entries: BTreeMap<FixtureId, Entry> = values
                .into_iter()
                .filter_map(|(id, values)| {
                    let fixture = self.fixtures.get(&id)?;
                    let mode = self.mode_of(id)?;
                    let profile = fixture.profile.as_str();
                    Some((
                        id,
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
                            palette.fixtures = detected.fixtures;
                        }
                        Some(_) => palette.merge(detected),
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
                    fixtures,
                } = detected;
                self.palettes.insert(
                    id,
                    Palette {
                        id,
                        number,
                        window: cell.window,
                        handle: cell.handle,
                        legend: String::new(),
                        icon: None,
                        global,
                        shared,
                        fixtures,
                    },
                );
                Ok(id)
            }
        }
    }

    /// PAL-01, 03: the programmer's values in the mask as plain values; a reference is
    /// recorded as the value it resolves to now.
    fn record_values(
        &self,
        mask: &[AttributeGroup],
    ) -> BTreeMap<FixtureId, BTreeMap<String, Value>> {
        self.contents(mask)
            .into_iter()
            .map(|(id, attrs)| {
                let values = attrs
                    .into_iter()
                    .map(|(name, stored)| {
                        let value = self.resolve_stored(id, &name, stored);
                        (name, value)
                    })
                    .collect();
                (id, values)
            })
            .collect()
    }

    /// PAL-02 Mixed: a fixture with a touched attribute in group P (or C) also stores the rest
    /// of that group, untouched ones at their current output value (PRG-20).
    fn mix_groups(&self, values: &mut BTreeMap<FixtureId, BTreeMap<String, Value>>) {
        for (&id, attrs) in values.iter_mut() {
            let Some(mode) = self.mode_of(id) else {
                continue;
            };
            let whole: Vec<LogicalAttribute> = [AttributeGroup::P, AttributeGroup::C]
                .into_iter()
                .filter(|g| {
                    attrs
                        .keys()
                        .any(|n| mode.logical(n).is_some_and(|l| l.group == *g))
                })
                .flat_map(|g| mode.logical_attributes().filter(move |l| l.group == g))
                .collect();
            for l in whole {
                if !attrs.contains_key(l.name) {
                    attrs.insert(l.name.to_string(), self.start_value(id, &l));
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
        targets: &[FixtureId],
    ) -> Result<(PaletteId, PalettePlan), CoreError> {
        let palette = self
            .palette(number)
            .ok_or(CoreError::UnknownPalette(number))?;
        let all: Vec<FixtureId>;
        let list = if targets.is_empty() {
            all = self.numbers.values().copied().collect();
            &all
        } else {
            targets
        };
        let mut plan = Vec::new();
        for &id in list {
            let (Some(fixture), Some(mode)) = (self.fixtures.get(&id), self.mode_of(id)) else {
                continue;
            };
            for l in mode.logical_attributes() {
                if let Some(v) = palette
                    .resolve(fixture, mode, l.name)
                    .filter(|v| v.same_kind(&l.default))
                {
                    plan.push((id, l.name.to_string(), v));
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
        for (fixture, name, value) in plan {
            let stored = if as_reference {
                Stored::Reference(Reference { palette, value })
            } else {
                Stored::Value(value)
            };
            self.programmer
                .set_value(fixture, &name, stored, self.activation_seq);
        }
    }

    /// PAL-08/09: apply palette `number` to the targets (selection or sub-selection), or as a
    /// quick palette when nothing is selected. Refused when it resolves nothing for any target.
    /// PAL-13..15: with a time > 0 each attribute fades from its current output (the fixtures
    /// staggered by the overlap); the programmer holds the reference from the start.
    pub fn apply_palette(&mut self, number: u32, timing: Timing) -> Result<(), CoreError> {
        let (time, overlap) = self.palette_timing(timing)?;
        let (id, plan) = self.palette_plan(number, &self.programmer.targets())?;
        self.put_palette_timed(id, plan, time, overlap);
        Ok(())
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
            let mut index = 0.0_f32;
            let mut previous = None;
            for (fixture, name, _) in &plan {
                if previous.is_some_and(|p| p != *fixture) {
                    index += 1.0;
                }
                previous = Some(*fixture);
                let Some(l) = self.mode_of(*fixture).and_then(|m| m.logical(name)) else {
                    continue;
                };
                fades.push((
                    *fixture,
                    name.clone(),
                    Fade {
                        from: self.start_value(*fixture, &l),
                        delay: index * step,
                        duration: time,
                        start: None,
                    },
                ));
            }
        }
        self.put_palette(id, plan, true);
        for (fixture, name, fade) in fades {
            self.programmer.set_fade(fixture, &name, fade);
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
        let (id, plan) = self.palette_plan(number, &self.programmer.targets())?;
        self.put_palette(id, plan, false);
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
        self.palettes.remove(&id);
        self.programmer.dereference(id);
        for stored in self
            .playbacks
            .values_mut()
            .flat_map(|pb| pb.cues.iter_mut())
            .flat_map(|cue| cue.values.values_mut())
            .flat_map(|attrs| attrs.values_mut())
        {
            *stored = stored.without_palette(id);
        }
        Ok(())
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
    pub fn resolve_stored(&self, fixture: FixtureId, attribute: &str, stored: Stored) -> Value {
        match (self.fixtures.get(&fixture), self.mode_of(fixture)) {
            (Some(f), Some(mode)) => resolve_stored(&self.palettes, f, mode, attribute, stored),
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

    /// PBK-35, CUE-40: the attribute groups all the playback's cues hold values in, in
    /// I P C G B E S FX order.
    pub fn playback_groups(&self, id: PlaybackId) -> Vec<AttributeGroup> {
        let cues = self.playback(id).map(|p| p.cues.as_slice()).unwrap_or(&[]);
        self.groups_of(cues.iter())
    }

    /// CUE-40: the attribute groups one cue holds values in (empty for an unknown cue).
    pub fn cue_groups(&self, id: PlaybackId, cue: CueNumber) -> Vec<AttributeGroup> {
        let cues = self.playback(id).map(|p| p.cues.as_slice()).unwrap_or(&[]);
        self.groups_of(cues.iter().filter(|c| c.number == cue))
    }

    fn groups_of<'a>(&self, cues: impl Iterator<Item = &'a Cue>) -> Vec<AttributeGroup> {
        let groups: BTreeSet<AttributeGroup> = cues
            .flat_map(|c| c.values.values())
            .flat_map(|attrs| attrs.keys())
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
        // PBK-30: DBO is a Grand Master of 0 for the HTP output, whoever proposes it.
        let grand_master = if self.dbo { 0.0 } else { self.grand_master };
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
        // GRM-10: (Take Over master, fixture, level) of the passes this tick.
        let mut pickups: Vec<(PlaybackId, FixtureId, f32)> = Vec::new();
        for (fixture_id, fixture) in &self.fixtures {
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
            let held = self.held.entry(*fixture_id).or_default();
            // The virtual Dimmer comes first, so `scale` is known for what follows.
            let virtual_dimmer = mode.has_virtual_dimmer();
            let mut scale = 1.0;
            for l in mode.logical_attributes() {
                let merged = merge_attribute(&sources, fixture, mode, &l, now);
                // ENG-07: an HTP attribute nobody proposes is 0; LTP holds its last value (LIB-03).
                let value = match (merged, l.kind) {
                    (Some(v), AttributeKind::Ltp) => {
                        match held.get_mut(l.name) {
                            Some(slot) => *slot = v,
                            None => {
                                held.insert(l.name.to_string(), v);
                            }
                        }
                        v
                    }
                    (merged, AttributeKind::Htp) => {
                        // ENG-07: nobody proposing is 0. GRM-06: the group masters, then the GM.
                        let v = masters.apply(*fixture_id, merged.unwrap_or(Value::Level(0.0)));
                        if l.name == DIMMER {
                            masters.pickups(*fixture_id, v.level(), &mut pickups);
                        }
                        v.scaled(grand_master)
                    }
                    (None, AttributeKind::Ltp) => held.get(l.name).copied().unwrap_or(l.default),
                };
                if virtual_dimmer && l.name == DIMMER {
                    scale = value.level();
                }
                write_value(frame, fixture.address, mode, &l, value, scale);
            }
        }
        self.follow_take_over(&pickups);
        self.programmer.drop_finished_fades(now);
        for state in self.states.values_mut() {
            if state.release.as_ref().is_some_and(|r| r.finished(now)) {
                state.release = None; // PBK-20: this tick was its last
            }
        }
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
                let keep = current.is_some() && members.is_some_and(|g| g.contains(&f));
                if keep && current != level {
                    put(&mut self.programmer, &mut self.activation_seq, f, g);
                }
                keep
            });
        }
    }

    // ---- queries ---------------------------------------------------------

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

    /// Recorded groups, sorted by id, members in group order.
    pub fn groups(&self) -> &BTreeMap<GroupId, Vec<FixtureId>> {
        &self.groups
    }

    pub fn group(&self, id: GroupId) -> Option<&[FixtureId]> {
        self.groups.get(&id).map(Vec::as_slice)
    }

    pub fn grand_master(&self) -> f32 {
        self.grand_master
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Take accumulated warnings (for the IPC layer to forward to the UI).
    pub fn drain_warnings(&mut self) -> Vec<Warning> {
        std::mem::take(&mut self.warnings)
    }
}

/// CUE-26: a cue list or a chase.
fn is_list(playback: &Playback) -> bool {
    playback.is_cue_list() || playback.kind == PlaybackKind::Chase
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
    groups: &'a BTreeMap<GroupId, Vec<FixtureId>>,
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
                .is_some_and(|f| f.contains(&fixture));
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

/// Where the LTP attributes of a playback's cue fade from (PBK-15, 17): the output of the tick
/// before the playback asserted them; `staggered`: the fixtures start one after the other in
/// user number order by the cue's overlap (PAL-14), each attribute by its fade (CUE-14).
fn origins(
    playback: &Playback,
    fixtures: &BTreeMap<FixtureId, Fixture>,
    profiles: &BTreeMap<String, Profile>,
    held: &BTreeMap<FixtureId, BTreeMap<String, Value>>,
    staggered: bool,
) -> Origins {
    let mut out = Origins::new();
    let Some(values) = playback.values() else {
        return out;
    };
    let mut order: Vec<(u32, FixtureId)> = values
        .keys()
        .filter_map(|id| Some((fixtures.get(id)?.number, *id)))
        .collect();
    order.sort_unstable();
    let overlap = playback.times().overlap;
    for (rank, (_, id)) in order.into_iter().enumerate() {
        let (Some(fixture), Some(attributes)) = (fixtures.get(&id), values.get(&id)) else {
            continue;
        };
        let Some(mode) = resolve_mode(profiles, fixture) else {
            continue;
        };
        for name in attributes.keys() {
            let Some(l) = mode.logical(name).filter(|l| l.kind == AttributeKind::Ltp) else {
                continue;
            };
            let value = (held.get(&id))
                .and_then(|attributes| attributes.get(l.name))
                .copied()
                .unwrap_or(l.default);
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
/// Returns `None` when no active source proposes a value. `tick` then writes 0
/// for an HTP attribute and the held value for an LTP one (ENG-07, LIB-03).
fn merge_attribute(
    src: &MergeSources,
    fixture: &Fixture,
    mode: &Mode,
    attr: &LogicalAttribute,
    now: Timestamp,
) -> Option<Value> {
    let mut best: Option<Candidate> = None;
    // A reference gives what its palette resolves, else its fallback (PAL-07, PAL-11); a value
    // of the wrong kind (a level for Colour, say) is ignored, not output.
    let resolve = |stored: Stored| resolve_stored(src.palettes, fixture, mode, attr.name, stored);

    for (id, playback) in src.playbacks {
        let Some(state) = src.states.get(id) else {
            continue;
        };
        let Some(activated) = state.activated else {
            continue;
        };
        // A cue list or chase proposes what its fades give now (CUE-13); anything else its first cue.
        let from_list =
            is_list(playback).then(|| state.run.fade_value(fixture.id, attr.name, now, resolve));
        let raw = match from_list {
            Some(fade) => fade,
            None => (playback.values())
                .and_then(|values| values.get(&fixture.id))
                .and_then(|attrs| attrs.get(attr.name))
                .map(|s| (resolve(*s), 0)),
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
                let from = (state.from.get(&fixture.id)).and_then(|attrs| attrs.get(attr.name));
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
        };

        best = Some(match best {
            None => candidate,
            Some(current) => pick(attr.kind, current, candidate),
        });
    }

    // ENG-04: manual values are absolute and sit at Priority::Programmer.
    if let Some(value) = src
        .programmer
        .stored(fixture.id, attr.name)
        .map(resolve)
        .filter(|v| v.same_kind(&attr.default))
        .map(|to| match src.programmer.fade(fixture.id, attr.name) {
            Some(fade) => fade.at(to, now), // PAL-13
            None => to,
        })
    {
        let candidate = Candidate {
            priority: Priority::Programmer,
            activated: src.programmer.activated(),
            value,
        };
        best = Some(match best {
            None => candidate,
            Some(current) => pick(attr.kind, current, candidate),
        });
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
            let Some((from, pre)) = (release.entries.get(&fixture.id))
                .and_then(|attrs| attrs.get(attr.name))
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
                },
            });
        }
    }

    best.map(|c| c.value)
}

/// A stored value, or what its palette resolves for this fixture and attribute (PAL-07),
/// falling back to the reference's own value (PAL-11).
pub(crate) fn resolve_stored(
    palettes: &BTreeMap<PaletteId, Palette>,
    fixture: &Fixture,
    mode: &Mode,
    attribute: &str,
    stored: Stored,
) -> Value {
    match stored {
        Stored::Value(v) => v,
        Stored::Reference(r) => palettes
            .get(&r.palette)
            .and_then(|p| p.resolve(fixture, mode, attribute))
            .unwrap_or(r.value),
    }
}

/// ENG-01..03: higher priority always wins; at equal priority HTP takes the
/// highest value and LTP the latest activation.
fn pick(kind: AttributeKind, current: Candidate, candidate: Candidate) -> Candidate {
    if candidate.priority != current.priority {
        return if candidate.priority > current.priority {
            candidate
        } else {
            current
        };
    }
    match kind {
        AttributeKind::Htp => {
            if candidate.value.level() > current.value.level() {
                candidate
            } else {
                current
            }
        }
        AttributeKind::Ltp => {
            if candidate.activated > current.activated {
                candidate
            } else {
                current
            }
        }
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
            BTreeMap::from([(1, BTreeMap::from([("Dimmer".to_string(), 1.0.into())]))]),
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
