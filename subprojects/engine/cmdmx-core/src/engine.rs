//! The merge: active sources → one frame per universe per tick.
//!
//! Implements the phase-1 subset of `specs/modules/engine-merge.md`:
//! ENG-01 (HTP highest wins), ENG-02 (LTP latest wins), ENG-03 (priority),
//! ENG-05 (fader mode 0 scaling / LTP snap), ENG-06 (Grand Master), ENG-11
//! (determinism), ENG-12 (never stops on bad input).
//!
//! Not yet here, by design (each lands with its spec): programmer (ENG-04),
//! release masks and times (ENG-07..10), chases, masters other than GM.

use std::collections::BTreeMap;

use crate::attribute::{AttributeDef, AttributeKind};
use crate::patch::{Fixture, FixtureId};
use crate::playback::{FaderMode, Playback, PlaybackId, PlaybackState, Priority};
use crate::profile::Profile;
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
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            ltp_trigger_level: 0.02,
            faders_per_page: 8,
        }
    }
}

/// The engine. Deterministic: the same sequence of calls yields the same frames.
#[derive(Debug, Default)]
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
            grand_master: 1.0,
            ..Default::default()
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
                let merged = merge_attribute(*fixture_id, attr, &self.playbacks, &self.states);
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

/// Merge one attribute of one fixture across all active playbacks.
///
/// Returns `None` when no active source proposes a value, in which case the
/// caller leaves the slot as it was (LTP "stays where it is" semantics; HTP
/// will be revisited by release handling, ENG-07).
fn merge_attribute(
    fixture: FixtureId,
    attr: &AttributeDef,
    playbacks: &BTreeMap<PlaybackId, Playback>,
    states: &BTreeMap<PlaybackId, PlaybackState>,
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
