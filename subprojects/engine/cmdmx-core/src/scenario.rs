//! JSON scenario format and runner (see `specs/modules/engine-merge.md`,
//! "Scenario test format").
//!
//! A scenario is: profiles, patch, playbacks, timestamped actions, and
//! expected DMX channel values at sampled timestamps. The runner drives the
//! engine with a virtual 40 Hz clock, applies actions whose time has come
//! before each tick, and compares frames. Used by `tests/scenarios.rs` and
//! available to a future `cmdmx-cli replay` command.

use std::collections::BTreeMap;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

use crate::attribute::AttributeGroup;
use crate::colour::TargetColour;
use crate::engine::{Engine, Settings};
use crate::palette::{Palette, Record, Timing};
use crate::patch::{Fixture, FixtureId};
use crate::playback::{
    ChaseOptions, Cue, CueNumber, FaderMode, HandleButton, HandleButtons, KillPoint, Kind,
    ListAction, ListOptions, Master, Paging, Playback, PlaybackId, Priority, UpdateMode, Values,
    PAGES,
};
use crate::profile::Profile;
use crate::programmer::Resolution;
use crate::show::ShowGroup;
use crate::undo::History;
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
    /// PAL-23: palettes as in the show file, placed after the patch.
    #[serde(default)]
    pub palettes: Vec<Palette>,
    /// GRM: groups as in the show file, placed before the playbacks.
    #[serde(default)]
    pub groups: Vec<ShowGroup>,
    #[serde(default)]
    pub playbacks: Vec<ScenarioPlayback>,
    #[serde(default)]
    pub actions: Vec<Action>,
    pub expect: Vec<Expectation>,
}

/// A playback of a scenario (`specs/modules/playbacks.md`, "Scenario test format"): the
/// show-file fields, all optional but the id. `page` and `slot` default to where PBK-01 puts
/// the id; a plain `values` is one cue with zero times.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScenarioPlayback {
    pub id: PlaybackId,
    pub page: Option<u16>,
    pub slot: Option<u16>,
    pub legend: Option<String>,
    pub icon: Option<String>,
    pub halo: Option<TargetColour>,
    pub priority: Option<Priority>,
    pub fader_mode: Option<FaderMode>,
    pub kill_point: Option<KillPoint>,
    pub release_mask: Option<Vec<AttributeGroup>>,
    pub release_time: Option<f32>,
    pub paging: Option<Paging>,
    pub group: Option<u8>,
    pub values: Option<Values>,
    #[serde(default, deserialize_with = "numbered_cues")]
    pub cues: Option<Vec<Cue>>,
    /// CUE-38.
    pub kind: Option<Kind>,
    pub list: Option<ListOptions>,
    pub chase: Option<ChaseOptions>,
    pub master: Option<Master>,
}

/// CUE-38: a cue without a `number` counts 1, 2, 3 … in file order.
fn numbered_cues<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Vec<Cue>>, D::Error> {
    let Some(raw) = Option::<Vec<serde_json::Value>>::deserialize(deserializer)? else {
        return Ok(None);
    };
    let numbered = raw.into_iter().zip(1u32..);
    let cues = numbered.map(|(value, position)| {
        let has_number = value.get("number").is_some();
        let mut cue: Cue = serde_json::from_value(value).map_err(D::Error::custom)?;
        if !has_number {
            cue.number = CueNumber::from(f64::from(position));
        }
        Ok(cue)
    });
    cues.collect::<Result<Vec<_>, _>>().map(Some)
}

impl ScenarioPlayback {
    fn build(&self, faders_per_page: u16) -> Playback {
        // PBK-01: page = ⌈id / fpp⌉ (beyond `PAGES` it is out of reach, as a stored handle).
        let n = self.id.saturating_sub(1);
        let per_page = u32::from(faders_per_page.max(1));
        let page = u16::try_from(n / per_page + 1).unwrap_or(PAGES + 1);
        let slot = u16::try_from(n % per_page + 1).unwrap_or(1);
        let mut pb = Playback::new(
            self.id,
            self.page.unwrap_or(page),
            self.slot.unwrap_or(slot),
            self.values.clone().unwrap_or_default(),
        );
        if let Some(cues) = &self.cues {
            pb.cues = cues.clone();
        }
        pb.kind = self.kind.unwrap_or(pb.kind);
        pb.list = self.list.clone().unwrap_or(pb.list);
        pb.chase = self.chase.clone().unwrap_or(pb.chase);
        pb.master = self.master.clone();
        pb.legend = self.legend.clone().unwrap_or_default();
        pb.icon = self.icon.clone();
        pb.halo = self.halo;
        pb.priority = self.priority.unwrap_or(pb.priority);
        pb.fader_mode = self.fader_mode.unwrap_or(pb.fader_mode);
        pb.kill_point = self.kill_point.unwrap_or(pb.kill_point);
        pb.release_mask = self.release_mask.clone();
        pb.release_time = self.release_time;
        pb.paging = self.paging.unwrap_or(pb.paging);
        pb.group = self.group;
        pb
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScenarioSettings {
    pub ltp_trigger_level: Option<f32>,
    /// CUE-25, CUE-52.
    pub handle_buttons: Option<HandleButtons>,
    pub update_mode: Option<UpdateMode>,
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
    Fader {
        playback: PlaybackId,
        level: f32,
    },
    /// PBK-26..28, 22, 30.
    Flash {
        playback: PlaybackId,
        on: bool,
    },
    Swop {
        playback: PlaybackId,
        on: bool,
    },
    Latch {
        playback: PlaybackId,
    },
    Release {
        playback: PlaybackId,
    },
    /// CUE-10..12, 18, 19, 26, 27: no `playback` is the connected list.
    Go {
        playback: Option<PlaybackId>,
    },
    Back {
        playback: Option<PlaybackId>,
    },
    Stop {
        playback: Option<PlaybackId>,
    },
    Goto {
        playback: Option<PlaybackId>,
        cue: f64,
    },
    Next {
        playback: Option<PlaybackId>,
        cue: f64,
    },
    Connect {
        playback: PlaybackId,
    },
    /// CUE-33: `at` is the time the tap was received.
    Tap {
        playback: PlaybackId,
        at: Timestamp,
    },
    /// CUE-34.
    Bpm {
        playback: PlaybackId,
        bpm: f32,
    },
    /// CUE-55.
    Button {
        playback: PlaybackId,
        button: HandleButton,
        on: bool,
    },
    Dbo {
        on: bool,
    },
    GrandMaster(f32),
    Prg(PrgAction),
    Pal(PalAction),
    /// UNDO: `{"undo": {}}`.
    Undo {},
    Redo {},
}

/// Palette actions in a scenario (`specs/modules/palettes.md`, "Scenario test format").
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PalAction {
    Record(Record),
    Apply {
        number: u32,
        #[serde(default)]
        time: Option<f32>,
        #[serde(default)]
        overlap: Option<f32>,
    },
    Delete(u32),
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
    #[serde(default)]
    pub universe: UniverseId,
    /// 1-based DMX address → expected raw value 0..=255.
    #[serde(default)]
    pub channels: BTreeMap<String, u8>,
    /// UNDO: the labels the next Undo and Redo would act on.
    pub history: Option<HistoryExpectation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryExpectation {
    pub undo: Option<String>,
    pub redo: Option<String>,
}

/// One failed expectation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mismatch {
    Channel {
        t_milli: u64,
        universe: UniverseId,
        address: u16,
        expected: u8,
        actual: Option<u8>,
    },
    History {
        t_milli: u64,
        expected: (Option<String>, Option<String>),
        actual: (Option<String>, Option<String>),
    },
}

impl std::fmt::Display for Mismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mismatch::Channel {
                t_milli,
                universe,
                address,
                expected,
                actual,
            } => write!(
                f,
                "t={t_milli}ms universe {universe} address {address}: expected {expected}, got {actual:?}"
            ),
            Mismatch::History {
                t_milli,
                expected,
                actual,
            } => write!(
                f,
                "t={t_milli}ms history (undo, redo): expected {expected:?}, got {actual:?}"
            ),
        }
    }
}

/// Build the engine a scenario describes.
pub fn build_engine(scenario: &Scenario) -> Result<Engine, CoreError> {
    let mut settings = Settings::default();
    if let Some(t) = scenario.settings.ltp_trigger_level {
        settings.ltp_trigger_level = t;
    }
    if let Some(buttons) = &scenario.settings.handle_buttons {
        settings.handle_buttons = buttons.clone();
    }
    if let Some(mode) = scenario.settings.update_mode {
        settings.update_mode = mode;
    }
    let mut engine = Engine::new(settings);
    for (key, profile) in &scenario.profiles {
        engine.add_profile(key.clone(), profile.clone())?;
    }
    for fixture in &scenario.patch {
        engine.patch(fixture.clone())?;
    }
    for palette in &scenario.palettes {
        engine.add_palette(palette.clone())?;
    }
    for group in &scenario.groups {
        engine.clear_selection();
        for fixture in &group.fixtures {
            engine.select_fixture(*fixture)?;
        }
        engine.record_group(group.id)?;
    }
    engine.clear_selection();
    for playback in &scenario.playbacks {
        engine.add_playback(playback.build(engine.settings().faders_per_page));
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
    let mut history = History::default();

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
            if !matches!(
                action.kind,
                ActionKind::Prg(_) | ActionKind::Pal(_) | ActionKind::Undo {} | ActionKind::Redo {}
            ) {
                history.live();
            }
            match &action.kind {
                ActionKind::Fader { playback, level } => {
                    engine.set_fader(*playback, *level, now)?;
                }
                ActionKind::Flash { playback, on } => engine.set_flash(*playback, *on, now)?,
                ActionKind::Swop { playback, on } => engine.set_swop(*playback, *on, now)?,
                ActionKind::Latch { playback } => engine.latch_flash(*playback, now)?,
                ActionKind::Release { playback } => engine.release_playback(*playback, now)?,
                ActionKind::Go { playback } => engine.cuelist(*playback, ListAction::Go, now)?,
                ActionKind::Back { playback } => {
                    engine.cuelist(*playback, ListAction::Back, now)?
                }
                ActionKind::Stop { playback } => {
                    engine.cuelist(*playback, ListAction::Stop, now)?
                }
                ActionKind::Goto { playback, cue } => {
                    let cue = CueNumber::new(*cue).ok_or(CoreError::InvalidCueNumber)?;
                    engine.cuelist(*playback, ListAction::Goto(cue), now)?;
                }
                ActionKind::Next { playback, cue } => {
                    let cue = CueNumber::new(*cue).ok_or(CoreError::InvalidCueNumber)?;
                    engine.cuelist(*playback, ListAction::Next(cue), now)?;
                }
                ActionKind::Connect { playback } => engine.connect_list(*playback)?,
                ActionKind::Tap { playback, at } => engine.tap(*playback, *at)?,
                ActionKind::Bpm { playback, bpm } => engine.set_bpm(*playback, *bpm)?,
                ActionKind::Button {
                    playback,
                    button,
                    on,
                } => engine.press_button(*playback, *button, *on, now)?,
                ActionKind::Dbo { on } => engine.set_dbo(*on),
                ActionKind::GrandMaster(level) => engine.set_grand_master(*level)?,
                ActionKind::Prg(PrgAction::Select(ids)) => {
                    history.edit(&mut engine, "Select", Some("select"), |e| {
                        ids.iter().try_for_each(|id| e.select_fixture(*id))
                    })?;
                }
                ActionKind::Prg(PrgAction::Wheel {
                    attribute,
                    steps,
                    resolution,
                }) => history.edit(
                    &mut engine,
                    &format!("Wheel {attribute}"),
                    Some(&format!("wheel:{attribute}")),
                    |e| e.turn_wheel(attribute, *steps, *resolution),
                )?,
                ActionKind::Prg(PrgAction::Command(tokens)) => {
                    let words: Vec<&str> = tokens.iter().map(String::as_str).collect();
                    history.edit(&mut engine, &tokens.join(" "), None, |e| {
                        e.execute_command(&words)
                    })?;
                }
                ActionKind::Pal(PalAction::Record(record)) => {
                    history.edit(&mut engine, "Record palette", None, |e| {
                        e.record_palette(record).map(drop)
                    })?;
                }
                ActionKind::Pal(PalAction::Apply {
                    number,
                    time,
                    overlap,
                }) => history.edit(&mut engine, &format!("Palette {number}"), None, |e| {
                    e.apply_palette(
                        *number,
                        Timing {
                            time: *time,
                            overlap: *overlap,
                        },
                    )
                })?,
                ActionKind::Pal(PalAction::Delete(number)) => history.edit(
                    &mut engine,
                    &format!("Delete palette {number}"),
                    None,
                    |e| e.delete_palette(*number),
                )?,
                ActionKind::Undo {} => {
                    history.undo(&mut engine)?;
                }
                ActionKind::Redo {} => {
                    history.redo(&mut engine)?;
                }
            }
            next_action += 1;
        }

        engine.tick(now);

        while let Some(expectation) = expectations.get(next_expect) {
            if expectation.t > now + 1e-9 {
                break;
            }
            let t_milli = (now * 1000.0).round() as u64;
            if let Some(h) = &expectation.history {
                let actual = (
                    history.undo_label().map(str::to_string),
                    history.redo_label().map(str::to_string),
                );
                if actual != (h.undo.clone(), h.redo.clone()) {
                    mismatches.push(Mismatch::History {
                        t_milli,
                        expected: (h.undo.clone(), h.redo.clone()),
                        actual,
                    });
                }
            }
            let frame = engine.frame(expectation.universe);
            for (address_text, expected) in &expectation.channels {
                let address: u16 = address_text.parse().unwrap_or(0);
                let actual = frame.and_then(|f| f.channel(address));
                if actual != Some(*expected) {
                    mismatches.push(Mismatch::Channel {
                        t_milli,
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
