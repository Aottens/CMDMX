//! JSON scenario format and runner (see `specs/modules/engine-merge.md`,
//! "Scenario test format").
//!
//! A scenario is: profiles, patch, playbacks, timestamped actions, and
//! expected DMX channel values at sampled timestamps. The runner drives the
//! engine with a virtual 40 Hz clock, applies actions whose time has come
//! before each tick, and compares frames. Used by `tests/scenarios.rs` and
//! available to a future `cmdmx-cli replay` command.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::engine::{Engine, Settings};
use crate::patch::{Fixture, FixtureId};
use crate::playback::{Playback, PlaybackId};
use crate::profile::Profile;
use crate::programmer::Resolution;
use crate::{CoreError, Timestamp, UniverseId};

pub const TICK_SECONDS: f64 = 1.0 / 40.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    /// Criterion id this scenario proves, e.g. "ENG-01".
    pub spec: String,
    #[serde(default)]
    pub settings: ScenarioSettings,
    pub profiles: BTreeMap<String, Profile>,
    pub patch: Vec<Fixture>,
    #[serde(default)]
    pub playbacks: Vec<Playback>,
    #[serde(default)]
    pub actions: Vec<Action>,
    pub expect: Vec<Expectation>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScenarioSettings {
    pub ltp_trigger_level: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub t: Timestamp,
    #[serde(flatten)]
    pub kind: ActionKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    Fader { playback: PlaybackId, level: f32 },
    GrandMaster(f32),
    Prg(PrgAction),
}

/// Programmer actions in a scenario (PRG-12/13).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrgAction {
    Select(Vec<FixtureId>),
    Wheel {
        attribute: String,
        steps: i32,
        resolution: Resolution,
    },
    Command(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Expectation {
    pub t: Timestamp,
    pub universe: UniverseId,
    /// 1-based DMX address → expected raw value 0..=255.
    pub channels: BTreeMap<String, u8>,
}

/// One failed expectation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mismatch {
    pub t_milli: u64,
    pub universe: UniverseId,
    pub address: u16,
    pub expected: u8,
    pub actual: Option<u8>,
}

impl std::fmt::Display for Mismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "t={}ms universe {} address {}: expected {}, got {:?}",
            self.t_milli, self.universe, self.address, self.expected, self.actual
        )
    }
}

/// Build the engine a scenario describes.
pub fn build_engine(scenario: &Scenario) -> Result<Engine, CoreError> {
    let mut settings = Settings::default();
    if let Some(t) = scenario.settings.ltp_trigger_level {
        settings.ltp_trigger_level = t;
    }
    let mut engine = Engine::new(settings);
    for (key, profile) in &scenario.profiles {
        engine.add_profile(key.clone(), profile.clone());
    }
    for fixture in &scenario.patch {
        engine.patch(fixture.clone())?;
    }
    for playback in &scenario.playbacks {
        engine.add_playback(playback.clone());
    }
    Ok(engine)
}

/// Replay a scenario and return every mismatch (empty = pass).
///
/// Actions are applied in file order whenever `action.t <= now` of the
/// upcoming tick; expectations are checked at the first tick whose time is
/// `>= expectation.t`. Both are tolerant to the 25 ms tick grid.
pub fn run(scenario: &Scenario) -> Result<Vec<Mismatch>, CoreError> {
    let mut engine = build_engine(scenario)?;

    let mut actions: Vec<&Action> = scenario.actions.iter().collect();
    actions.sort_by(|a, b| a.t.total_cmp(&b.t));
    let mut expectations: Vec<&Expectation> = scenario.expect.iter().collect();
    expectations.sort_by(|a, b| a.t.total_cmp(&b.t));

    let end = expectations
        .iter()
        .map(|e| e.t)
        .chain(actions.iter().map(|a| a.t))
        .fold(0.0_f64, f64::max)
        + TICK_SECONDS;

    let mut mismatches = Vec::new();
    let mut next_action = 0usize;
    let mut next_expect = 0usize;
    let mut tick_index: u64 = 0;

    loop {
        let now = tick_index as f64 * TICK_SECONDS;
        if now > end {
            break;
        }

        while let Some(action) = actions.get(next_action) {
            if action.t > now + 1e-9 {
                break;
            }
            match &action.kind {
                ActionKind::Fader { playback, level } => {
                    engine.set_fader(*playback, *level, now)?;
                }
                ActionKind::GrandMaster(level) => engine.set_grand_master(*level)?,
                ActionKind::Prg(PrgAction::Select(ids)) => {
                    for id in ids {
                        engine.select_fixture(*id)?;
                    }
                }
                ActionKind::Prg(PrgAction::Wheel {
                    attribute,
                    steps,
                    resolution,
                }) => engine.turn_wheel(attribute, *steps, *resolution)?,
                ActionKind::Prg(PrgAction::Command(tokens)) => {
                    let tokens: Vec<&str> = tokens.iter().map(String::as_str).collect();
                    engine.execute_command(&tokens)?;
                }
            }
            next_action += 1;
        }

        engine.tick(now);

        while let Some(expectation) = expectations.get(next_expect) {
            if expectation.t > now + 1e-9 {
                break;
            }
            let frame = engine.frame(expectation.universe);
            for (address_text, expected) in &expectation.channels {
                let address: u16 = address_text.parse().unwrap_or(0);
                let actual = frame.and_then(|f| f.channel(address));
                if actual != Some(*expected) {
                    mismatches.push(Mismatch {
                        t_milli: (now * 1000.0).round() as u64,
                        universe: expectation.universe,
                        address,
                        expected: *expected,
                        actual,
                    });
                }
            }
            next_expect += 1;
        }

        tick_index += 1;
    }

    Ok(mismatches)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_json_round_trips() {
        let json = r#"{
          "spec": "ENG-00",
          "profiles": { "d": { "attributes": [ { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 } ] } },
          "patch": [ { "id": 1, "profile": "d", "universe": 1, "address": 1 } ],
          "playbacks": [ { "id": 1, "values": { "1": { "Dimmer": 1.0 } } } ],
          "actions": [ { "t": 0.0, "fader": { "playback": 1, "level": 1.0 } }, { "t": 0.5, "grand_master": 0.5 } ],
          "expect": [ { "t": 0.1, "universe": 1, "channels": { "1": 255 } }, { "t": 0.6, "universe": 1, "channels": { "1": 128 } } ]
        }"#;
        let scenario: Scenario = serde_json::from_str(json)
            .map_err(|e| e.to_string())
            .unwrap_or_else(|e| panic_free_fail(&e));
        let mismatches = run(&scenario).unwrap_or_else(|e| panic_free_fail(&e.to_string()));
        assert!(mismatches.is_empty(), "{mismatches:?}");
    }

    /// Tests may fail loudly; production code may not. Keeps the lint intact.
    #[allow(clippy::panic)]
    fn panic_free_fail<T>(msg: &str) -> T {
        panic!("{msg}")
    }
}
