//! The merge: active sources → one frame per universe per tick.
//!
//! Implements the phase-1 subset of `specs/modules/engine-merge.md`:
//! ENG-01 (HTP highest wins), ENG-02 (LTP latest wins), ENG-03 (priority),
//! ENG-04 (programmer at `Priority::Programmer`), ENG-05 (fader mode 0
//! scaling / LTP snap), ENG-06 (Grand Master), ENG-11 (determinism), ENG-12
//! (never stops on bad input).
//!
//! Not yet here, by design (each lands with its spec): release masks and
//! times (ENG-07..10), chases, masters other than GM.

use std::collections::BTreeMap;

use crate::attribute::{AttributeDef, AttributeGroup, AttributeKind};
use crate::command::{self, Item, Kind};
use crate::patch::{Fixture, FixtureId};
use crate::playback::{FaderMode, Playback, PlaybackId, PlaybackState, Priority, Values};
use crate::profile::Profile;
use crate::programmer::{GroupId, Programmer, Resolution};
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
}

/// A candidate value for one attribute from one source, during a merge.
struct Candidate {
    priority: Priority,
    activated: u64,
    value: f32,
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
        }
    }

    // ---- setup -----------------------------------------------------------

    pub fn add_profile(&mut self, key: impl Into<String>, profile: Profile) {
        self.profiles.insert(key.into(), profile);
    }

    /// Patch a fixture. Rejected (not panicking) when the profile is unknown or
    /// the footprint runs past slot 512.
    pub fn patch(&mut self, fixture: Fixture) -> Result<(), CoreError> {
        let profile = self
            .profiles
            .get(&fixture.profile)
            .ok_or_else(|| CoreError::UnknownProfile(fixture.profile.clone()))?;
        let last = u32::from(fixture.address) + u32::from(profile.footprint()) - 1;
        if fixture.address == 0 || last > UNIVERSE_SIZE as u32 {
            return Err(CoreError::AddressOutOfRange {
                fixture: fixture.id,
                universe: fixture.universe,
                address: fixture.address,
            });
        }
        self.frames.entry(fixture.universe).or_default();
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
        value: f32,
    ) -> Result<(), CoreError> {
        let fx = self
            .fixtures
            .get(&fixture)
            .ok_or(CoreError::UnknownFixture(fixture))?;
        let profile = self
            .profiles
            .get(&fx.profile)
            .ok_or_else(|| CoreError::UnknownProfile(fx.profile.clone()))?;
        if profile.attribute(attribute).is_none() {
            return Err(CoreError::UnknownAttribute {
                fixture,
                attribute: attribute.to_string(),
            });
        }
        if !(0.0..=1.0).contains(&value) || value.is_nan() {
            return Err(CoreError::InvalidLevel(value));
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
                self.profile_of(*id)
                    .is_some_and(|p| p.attributes.iter().any(|a| a.group == g))
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

    fn profile_of(&self, id: FixtureId) -> Option<&Profile> {
        self.fixtures
            .get(&id)
            .and_then(|f| self.profiles.get(&f.profile))
    }

    /// PRG-17: names of the active group's attributes that at least one
    /// target has, in target order then profile order.
    pub fn wheel_layout(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for id in self.programmer.targets() {
            for a in self
                .profile_of(id)
                .map(|p| p.attributes.as_slice())
                .unwrap_or_default()
            {
                if a.group == self.active_group && !names.contains(&a.name) {
                    names.push(a.name.clone());
                }
            }
        }
        names
    }

    /// PRG-19..21, 36: move `attribute` of every target that has it by
    /// `steps` wheel steps. Refused turns change nothing.
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
        let mut new = Vec::new();
        for id in targets {
            let Some(fixture) = self.fixtures.get(&id) else {
                continue;
            };
            let Some(attr) = self
                .profiles
                .get(&fixture.profile)
                .and_then(|p| p.attribute(attribute))
            else {
                continue;
            };
            // PRG-20: programmer value, else what is on the output, else 0.
            let start = self
                .programmer
                .value(id, attribute)
                .or_else(|| {
                    merge_attribute(id, attr, &self.playbacks, &self.states, &self.programmer)
                })
                .or_else(|| self.frame_value(fixture, attr))
                .unwrap_or(0.0);
            new.push((id, (start + steps as f32 * step).clamp(0.0, 1.0)));
        }
        if new.is_empty() {
            return Err(CoreError::AttributeNotInTargets(attribute.to_string()));
        }
        for (id, value) in new {
            self.set_programmer_value(id, attribute, value)?;
        }
        Ok(())
    }

    /// The fixture's current frame slot(s) decoded back to 0.0..=1.0.
    fn frame_value(&self, fixture: &Fixture, attr: &AttributeDef) -> Option<f32> {
        let frame = self.frames.get(&fixture.universe)?;
        let base = fixture.address.saturating_sub(1);
        let coarse = frame.channel(base.saturating_add(attr.channel))?;
        Some(match attr.fine_channel {
            None => f32::from(coarse) / 255.0,
            Some(fine_ch) => {
                let fine = frame.channel(base.saturating_add(fine_ch))?;
                f32::from(u16::from(coarse) << 8 | u16::from(fine)) / 65535.0
            }
        })
    }

    /// PRG-15: the programmer's manual values whose attribute is in `mask`.
    pub fn contents(&self, mask: &[AttributeGroup]) -> Values {
        let mut out = Values::new();
        for (id, attrs) in self.programmer.values() {
            let Some(profile) = self.profile_of(*id) else {
                continue;
            };
            for (name, value) in attrs {
                if profile
                    .attribute(name)
                    .is_some_and(|a| mask.contains(&a.group))
                {
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
                let profile = self
                    .fixtures
                    .get(&id)
                    .and_then(|f| self.profiles.get(&f.profile));
                for a in profile.map(|p| p.attributes.as_slice()).unwrap_or_default() {
                    if a.group == AttributeGroup::I && a.kind == AttributeKind::Htp {
                        touched.push((id, a.name.clone()));
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
            let Some(profile) = self.profiles.get(&fixture.profile) else {
                self.warnings.push(Warning {
                    at: now,
                    message: format!(
                        "fixture {fixture_id}: profile '{}' missing, holding last values",
                        fixture.profile
                    ),
                });
                continue;
            };
            for attr in &profile.attributes {
                let merged = merge_attribute(
                    *fixture_id,
                    attr,
                    &self.playbacks,
                    &self.states,
                    &self.programmer,
                );
                let Some(value) = merged else { continue };
                let scaled = match attr.kind {
                    AttributeKind::Htp => value * self.grand_master,
                    AttributeKind::Ltp => value,
                };
                let (coarse, fine) = attr.to_dmx(scaled);
                let base = fixture.address.saturating_sub(1);
                if let Some(frame) = self.frames.get_mut(&fixture.universe) {
                    frame.set(base.saturating_add(attr.channel), coarse);
                    if let (Some(fine_ch), Some(fine_v)) = (attr.fine_channel, fine) {
                        frame.set(base.saturating_add(fine_ch), fine_v);
                    }
                }
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

/// Merge one attribute of one fixture across all active playbacks and the programmer.
///
/// Returns `None` when no active source proposes a value, in which case the
/// caller leaves the slot as it was (LTP "stays where it is" semantics; HTP
/// will be revisited by release handling, ENG-07).
fn merge_attribute(
    fixture: FixtureId,
    attr: &AttributeDef,
    playbacks: &BTreeMap<PlaybackId, Playback>,
    states: &BTreeMap<PlaybackId, PlaybackState>,
    programmer: &Programmer,
) -> Option<f32> {
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
            .and_then(|attrs| attrs.get(&attr.name))
            .copied()
        else {
            continue;
        };

        let (value, order) = match (attr.kind, playback.fader_mode) {
            // ENG-05: HTP scales with the fader from 0 in every mode without times.
            (AttributeKind::Htp, _) => (raw * state.level, activated),
            // ENG-05: LTP snaps to the cue value once the fader has reached the
            // trigger point; until then this playback says nothing about it.
            (AttributeKind::Ltp, FaderMode::Mode0 | FaderMode::Mode1) => match state.triggered {
                Some(triggered) => (raw, triggered),
                None => continue,
            },
            // Mode 2: LTP follows the fader proportionally from 0.
            (AttributeKind::Ltp, FaderMode::Mode2) => (raw * state.level, activated),
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
    if let Some(value) = programmer.value(fixture, &attr.name) {
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
            if candidate.value > current.value {
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
        Profile {
            attributes: vec![AttributeDef {
                name: "Dimmer".into(),
                group: AttributeGroup::I,
                kind: AttributeKind::Htp,
                channel: 1,
                fine_channel: None,
            }],
        }
    }

    fn engine_with_dimmer() -> Engine {
        let mut e = Engine::new(Settings::default());
        e.add_profile("dimmer", dimmer_profile());
        let patched = e.patch(Fixture {
            id: 1,
            profile: "dimmer".into(),
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
            universe: 1,
            address: 1,
        });
        assert_eq!(r, Err(CoreError::UnknownProfile("nope".into())));
    }

    #[test]
    fn patch_rejects_address_past_universe_end() {
        let mut e = Engine::new(Settings::default());
        e.add_profile("dimmer", dimmer_profile());
        let r = e.patch(Fixture {
            id: 1,
            profile: "dimmer".into(),
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
            values: BTreeMap::from([(1, BTreeMap::from([("Dimmer".to_string(), 1.0)]))]),
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
