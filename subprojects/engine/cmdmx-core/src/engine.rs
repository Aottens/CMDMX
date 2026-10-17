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
//! Not yet here, by design (each lands with its spec): release masks and
//! times (ENG-07..10), chases, masters other than GM.

use std::collections::BTreeMap;

use crate::attribute::{AttributeDef, AttributeGroup, AttributeKind};
use crate::colour::{ColourModel, TargetColour};
use crate::command::{self, Item, Kind};
use crate::patch::{Fixture, FixtureId};
use crate::playback::{FaderMode, Playback, PlaybackId, PlaybackState, Priority, Values};
use crate::profile::{ColourMixing, ColourSystem, LogicalAttribute, Mode, Profile, COLOUR, DIMMER};
use crate::programmer::{
    AttributeGroupState, GroupId, Programmer, ProgrammerView, Resolution, WheelState,
};
use crate::value::Value;
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
    /// Wheel step sizes as fractions of the full range (PRG-21).
    pub wheel_step_normal: f32,
    pub wheel_step_fine: f32,
    pub wheel_step_coarse: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            ltp_trigger_level: 0.02,
            faders_per_page: 8,
            wheel_step_normal: 1.0 / 255.0,
            wheel_step_fine: 1.0 / 65535.0,
            wheel_step_coarse: 1.0 / 25.0,
        }
    }
}

/// The engine. Deterministic: the same sequence of calls yields the same frames.
#[derive(Debug)]
pub struct Engine {
    settings: Settings,
    profiles: BTreeMap<String, Profile>,
    fixtures: BTreeMap<FixtureId, Fixture>,
    playbacks: BTreeMap<PlaybackId, Playback>,
    states: BTreeMap<PlaybackId, PlaybackState>,
    grand_master: f32,
    activation_seq: u64,
    frames: BTreeMap<UniverseId, Frame>,
    warnings: Vec<Warning>,
    last_tick: Option<Timestamp>,
    programmer: Programmer,
    groups: BTreeMap<GroupId, Vec<FixtureId>>,
    /// Attribute group the wheels show (PRG-17/18).
    active_group: AttributeGroup,
    /// How the Colour wheels are shown (PRG-17, PRG-30).
    colour_model: ColourModel,
    /// Last value of every LTP logical attribute per fixture: what it outputs while
    /// nothing proposes it, and where a wheel starts (LIB-03, PRG-20).
    held: BTreeMap<FixtureId, BTreeMap<String, Value>>,
}

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
            playbacks: BTreeMap::new(),
            states: BTreeMap::new(),
            grand_master: 1.0,
            activation_seq: 0,
            frames: BTreeMap::new(),
            warnings: Vec::new(),
            last_tick: None,
            programmer: Programmer::default(),
            groups: BTreeMap::new(),
            active_group: AttributeGroup::I,
            colour_model: ColourModel::default(),
            held: BTreeMap::new(),
        }
    }

    // ---- setup -----------------------------------------------------------

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

    /// Patch a fixture. Rejected (not panicking) when the profile or mode is unknown
    /// or the mode's footprint runs past slot 512. LTP attributes start at their
    /// default (LIB-03).
    pub fn patch(&mut self, fixture: Fixture) -> Result<(), CoreError> {
        let profile = self
            .profiles
            .get(&fixture.profile)
            .ok_or_else(|| CoreError::UnknownProfile(fixture.profile.clone()))?;
        let mode = profile
            .mode(fixture.mode.as_deref())
            .ok_or_else(|| CoreError::UnknownMode {
                profile: fixture.profile.clone(),
                mode: fixture.mode.clone().unwrap_or_default(),
            })?;
        if fixture.address == 0
            || u32::from(fixture.address) + u32::from(mode.footprint) > UNIVERSE_SIZE as u32 + 1
        {
            return Err(CoreError::AddressOutOfRange {
                fixture: fixture.id,
                universe: fixture.universe,
                address: fixture.address,
            });
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
        self.fixtures.insert(fixture.id, fixture);
        Ok(())
    }

    pub fn add_playback(&mut self, playback: Playback) {
        self.states.entry(playback.id).or_default();
        self.playbacks.insert(playback.id, playback);
    }

    // ---- actions ---------------------------------------------------------

    /// Move a playback fader. Activation happens when the level rises to the
    /// LTP trigger point; the playback is killed when the level returns to 0.
    pub fn set_fader(
        &mut self,
        id: PlaybackId,
        level: f32,
        _now: Timestamp,
    ) -> Result<(), CoreError> {
        if !(0.0..=1.0).contains(&level) || level.is_nan() {
            return Err(CoreError::InvalidLevel(level));
        }
        if !self.playbacks.contains_key(&id) {
            return Err(CoreError::UnknownPlayback(id));
        }
        let trigger = self.settings.ltp_trigger_level;
        let seq = &mut self.activation_seq;
        let state = self.states.entry(id).or_default();
        state.level = level;
        if level <= 0.0 {
            // Kill: the playback leaves the merge entirely. Release masks and
            // times (ENG-07..09) will refine what happens to its LTP values.
            state.activated = None;
            state.triggered = None;
            return Ok(());
        }
        if state.activated.is_none() {
            *seq += 1;
            state.activated = Some(*seq);
        }
        if level >= trigger && state.triggered.is_none() {
            *seq += 1;
            state.triggered = Some(*seq);
        }
        Ok(())
    }

    /// Grand Master, 0.0..=1.0. Scales all HTP output after the merge (ENG-06).
    pub fn set_grand_master(&mut self, level: f32) -> Result<(), CoreError> {
        if !(0.0..=1.0).contains(&level) || level.is_nan() {
            return Err(CoreError::InvalidLevel(level));
        }
        self.grand_master = level;
        Ok(())
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

    /// PRG-02/03: append a group's fixtures, keeping the ones already selected in place.
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
        self.programmer
            .set_value(fixture, attribute, value, self.activation_seq);
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

    /// PRG-20: programmer value, else what is on the output, else the held value.
    fn start_value(&self, id: FixtureId, l: &LogicalAttribute) -> Value {
        self.programmer
            .value(id, l.name)
            .or_else(|| merge_attribute(id, l, &self.playbacks, &self.states, &self.programmer))
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
                Some(WheelState {
                    value,
                    touched: self.programmer.value(id, name).is_some(),
                    sixteen_bit: l.sixteen_bit,
                    attribute,
                })
            })
            .collect();
        ProgrammerView {
            selection: self.programmer.selection().to_vec(),
            targets,
            active_group: self.active_group,
            attribute_groups,
            wheels,
            in_programmer: self.programmer.values().keys().copied().collect(),
            colour_model: self.colour_model,
        }
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

    /// Patched fixtures (or members of existing groups) named by one item, in typed order.
    /// Ranges walk only the entries that exist, so `1 Thro 4000000000` stays cheap.
    fn expand(&self, item: &Item) -> Vec<FixtureId> {
        let (lo, hi) = (item.from.min(item.to), item.from.max(item.to));
        let mut keys: Vec<u32> = match item.kind {
            Kind::Fixture => self.fixtures.range(lo..=hi).map(|(id, _)| *id).collect(),
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

    // ---- tick ------------------------------------------------------------

    /// Recompute every universe frame for the current state at `now`.
    ///
    /// Never fails: anything that cannot be resolved is skipped and reported
    /// through [`Engine::drain_warnings`], and the previous value of that
    /// slot survives (ENG-12).
    pub fn tick(&mut self, now: Timestamp) {
        self.last_tick = Some(now);
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
                let merged = merge_attribute(
                    *fixture_id,
                    &l,
                    &self.playbacks,
                    &self.states,
                    &self.programmer,
                );
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
                    (Some(v), AttributeKind::Htp) => v.scaled(self.grand_master),
                    (None, AttributeKind::Htp) => Value::Level(0.0),
                    (None, AttributeKind::Ltp) => held.get(l.name).copied().unwrap_or(l.default),
                };
                if virtual_dimmer && l.name == DIMMER {
                    scale = value.level();
                }
                write_value(frame, fixture.address, mode, &l, value, scale);
            }
        }
    }

    // ---- queries ---------------------------------------------------------

    pub fn frame(&self, universe: UniverseId) -> Option<&Frame> {
        self.frames.get(&universe)
    }

    pub fn universes(&self) -> impl Iterator<Item = (&UniverseId, &Frame)> {
        self.frames.iter()
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

/// The fixture's mode: the named one, or the profile's first.
fn resolve_mode<'a>(
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

/// Merge one attribute of one fixture across all active playbacks and the programmer.
///
/// Returns `None` when no active source proposes a value. `tick` then writes 0
/// for an HTP attribute and the held value for an LTP one (ENG-07, LIB-03).
fn merge_attribute(
    fixture: FixtureId,
    attr: &LogicalAttribute,
    playbacks: &BTreeMap<PlaybackId, Playback>,
    states: &BTreeMap<PlaybackId, PlaybackState>,
    programmer: &Programmer,
) -> Option<Value> {
    let mut best: Option<Candidate> = None;

    for (id, playback) in playbacks {
        let Some(state) = states.get(id) else {
            continue;
        };
        let Some(activated) = state.activated else {
            continue;
        };
        let Some(raw) = playback
            .values
            .get(&fixture)
            .and_then(|attrs| attrs.get(attr.name))
            .copied()
            // A value of the wrong kind (a level for Colour, say) is ignored, not output.
            .filter(|v| v.same_kind(&attr.default))
        else {
            continue;
        };

        let (value, order) = match (attr.kind, playback.fader_mode) {
            // ENG-05: HTP scales with the fader from 0 in every mode without times.
            (AttributeKind::Htp, _) => (raw.scaled(state.level), activated),
            // ENG-05: LTP snaps to the cue value once the fader has reached the
            // trigger point; until then this playback says nothing about it.
            (AttributeKind::Ltp, FaderMode::Mode0 | FaderMode::Mode1) => match state.triggered {
                Some(triggered) => (raw, triggered),
                None => continue,
            },
            // Mode 2: LTP follows the fader proportionally from 0.
            (AttributeKind::Ltp, FaderMode::Mode2) => (raw.scaled(state.level), activated),
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
    if let Some(value) = programmer.value(fixture, attr.name) {
        let candidate = Candidate {
            priority: Priority::Programmer,
            activated: programmer.activated(),
            value,
        };
        best = Some(match best {
            None => candidate,
            Some(current) => pick(attr.kind, current, candidate),
        });
    }

    best.map(|c| c.value)
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
        e.add_playback(Playback {
            id: 1,
            priority: Priority::Normal,
            fader_mode: FaderMode::Mode0,
            values: BTreeMap::from([(1, BTreeMap::from([("Dimmer".to_string(), 1.0.into())]))]),
        });
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
