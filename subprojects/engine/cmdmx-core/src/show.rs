//! The show file: one JSON document holding everything the operator programmed
//! (`specs/modules/showfile.md` SHW-01..08). Pure: no I/O, no clock; times come in as parameters.
//!
//! `Engine::to_show` and `Engine::from_show` (in `engine.rs`, where the private state lives)
//! convert between an engine and a [`Show`]. Field order here is the key order in the file.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::attribute::AttributeGroup;
use crate::colour::ColourModel;
use crate::palette::{Palette, PaletteId, RecordBy};
use crate::patch::{Fixture, FixtureId};
use crate::playback::{Playback, PlaybackId};
use crate::profile::Profile;
use crate::programmer::GroupId;
use crate::CoreError;

/// SHW-04: every change to the schema raises this and comes with a step in [`migrate`].
pub const SCHEMA_VERSION: u32 = 3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    pub name: String,
    pub created: String,
    pub modified: String,
    pub app_version: String,
}

/// The engine's settings plus the colour model (SHW-01).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShowSettings {
    pub faders_per_page: u16,
    pub handles_per_page: u16,
    pub ltp_trigger_level: f32,
    pub wheel_step_normal: f32,
    pub wheel_step_fine: f32,
    pub wheel_step_coarse: f32,
    pub colour_model: ColourModel,
    /// PAL-15, PAL-23: master fade time (s) and overlap (%) of palettes.
    pub palette_time: f32,
    pub palette_overlap: f32,
    /// PAL-02.
    pub record_by: RecordBy,
    /// PBK-10, PBK-12: Record By and the times of a new cue.
    pub cue_record_by: RecordBy,
    pub cue_delay: f32,
    pub cue_fade_in: f32,
    pub cue_fade_out: f32,
    pub cue_overlap: f32,
    /// PBK-20: the global release mask and time.
    pub release_mask: Vec<AttributeGroup>,
    pub release_time: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShowPatch {
    /// SHW-06: above every id ever patched, so ids are never reused across a save and load.
    pub next_id: FixtureId,
    pub fixtures: Vec<Fixture>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShowGroup {
    pub id: GroupId,
    /// In recorded order.
    pub fixtures: Vec<FixtureId>,
}

/// PAL-23: above every palette id ever used, so ids are never reused across a save and load.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShowPalettes {
    pub next_id: PaletteId,
    pub palettes: Vec<ShowPalette>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShowPage {
    pub number: u16,
    pub legend: String,
}

/// PBK-34: `next_id` is above every playback id ever used, so ids are never reused.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShowPlaybacks {
    pub next_id: PlaybackId,
    pub playbacks: Vec<Playback>,
}

/// A palette as saved, or an entry that is not one (PAL-24: left out with a warning on load).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ShowPalette {
    Palette(Palette),
    Unreadable(serde_json::Value),
}

impl<'de> Deserialize<'de> for ShowPalette {
    /// Through `serde_json::Value`, not an untagged enum: that would buffer the entry and then
    /// fail on the fixture ids that are map keys.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(match serde_json::from_value(value.clone()) {
            Ok(palette) => ShowPalette::Palette(palette),
            Err(_) => ShowPalette::Unreadable(value),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Show {
    pub schema_version: u32,
    pub meta: Meta,
    pub settings: ShowSettings,
    /// SHW-05: exactly the profiles the patched fixtures use, by engine profile key.
    pub profiles: BTreeMap<String, Profile>,
    pub patch: ShowPatch,
    pub groups: Vec<ShowGroup>,
    pub palettes: ShowPalettes,
    /// PBK-34: only pages with a legend, sorted by number.
    pub pages: Vec<ShowPage>,
    pub playbacks: ShowPlaybacks,
}

impl Show {
    /// SHW-02: pretty JSON, two-space indents, keys in declaration order, trailing newline.
    pub fn to_json(&self) -> String {
        let mut json = serde_json::to_string_pretty(self).unwrap_or_default();
        json.push('\n');
        json
    }

    /// SHW-04: parse a show file, migrating an older schema step by step. Returns the show and
    /// the migrations that ran. Not JSON, no `schema_version` or a newer schema is refused.
    pub fn from_json(json: &str) -> Result<(Show, Vec<String>), CoreError> {
        let mut value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| CoreError::InvalidShow(e.to_string()))?;
        let version = value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .filter(|&v| v >= 1)
            .ok_or_else(|| CoreError::InvalidShow("no schema_version".into()))?;
        if version > u64::from(SCHEMA_VERSION) {
            let name = value
                .get("meta")
                .and_then(|m| m.get("name"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("the show");
            return Err(CoreError::ShowTooNew {
                name: name.to_string(),
                found: version,
                supported: SCHEMA_VERSION,
            });
        }
        let migrations = migrate(&mut value, version);
        let show =
            serde_json::from_value(value).map_err(|e| CoreError::InvalidShow(e.to_string()))?;
        Ok((show, migrations))
    }
}

/// SHW-04: bring `value` from schema `version` up to [`SCHEMA_VERSION`], one step per version,
/// and name each step that ran. A schema bump adds
/// `if version < n { /* edit value from n-1 to n */ migrations.push("n-1 -> n: what changed".into()); }`.
fn migrate(value: &mut serde_json::Value, version: u64) -> Vec<String> {
    let mut migrations = Vec::new();
    if version < 2 {
        if let Some(settings) = value
            .get_mut("settings")
            .and_then(serde_json::Value::as_object_mut)
        {
            settings.insert("palette_time".into(), 0.0.into());
            settings.insert("palette_overlap".into(), 100.0.into());
            settings.insert("record_by".into(), "Mixed".into());
        }
        if let Some(show) = value.as_object_mut() {
            show.insert(
                "palettes".into(),
                serde_json::json!({ "next_id": 1, "palettes": [] }),
            );
            show.insert("schema_version".into(), 2.into());
        }
        migrations
            .push("1 -> 2: palettes added (none yet), palette settings at their defaults".into());
    }
    if version < 3 {
        migrations.push(migrate_2_to_3(value));
    }
    migrations
}

/// PBK-45: each playback keeps the place PBK-01 gave it (id *n* on page ⌈n / fpp⌉, slot
/// ((n − 1) mod fpp) + 1, with the file's `faders_per_page`), its `values` become its only cue.
fn migrate_2_to_3(value: &mut serde_json::Value) -> String {
    use serde_json::json;
    let Some(show) = value.as_object_mut() else {
        return "2 -> 3: not an object, left as it is".into();
    };
    let per_page = show
        .get_mut("settings")
        .and_then(serde_json::Value::as_object_mut)
        .map_or(8, |settings| {
            let per_page = settings
                .get("faders_per_page")
                .and_then(serde_json::Value::as_u64)
                .filter(|&n| n > 0)
                .unwrap_or(8);
            for (key, default) in [
                ("cue_record_by", json!("Channel")),
                ("cue_delay", json!(0.0)),
                ("cue_fade_in", json!(0.0)),
                ("cue_fade_out", json!(0.0)),
                ("cue_overlap", json!(100.0)),
                ("release_mask", json!([])),
                ("release_time", json!(2.0)),
            ] {
                settings.insert(key.into(), default);
            }
            per_page
        });
    let old = match show.remove("playbacks") {
        Some(serde_json::Value::Array(list)) => list,
        _ => Vec::new(),
    };
    let mut next_id = 1;
    let mut playbacks = Vec::new();
    for old in old {
        let id = old
            .get("id")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        next_id = next_id.max(id.saturating_add(1));
        let n = id.saturating_sub(1);
        playbacks.push(json!({
            "id": id,
            "page": n / per_page + 1,
            "slot": n % per_page + 1,
            "legend": "",
            "icon": null,
            "halo": null,
            "priority": old.get("priority").cloned().unwrap_or(json!("Normal")),
            "fader_mode": old.get("fader_mode").cloned().unwrap_or(json!(0)),
            "kill_point": "FadeOutComplete",
            "release_mask": null,
            "release_time": null,
            "paging": "Unlocked",
            "group": null,
            "cues": [{
                "delay": 0.0,
                "fade_in": 0.0,
                "fade_out": 0.0,
                "overlap": 100.0,
                "values": old.get("values").cloned().unwrap_or(json!({})),
            }],
        }));
    }
    show.insert("pages".into(), json!([]));
    show.insert(
        "playbacks".into(),
        json!({ "next_id": next_id, "playbacks": playbacks }),
    );
    show.insert("schema_version".into(), 3.into());
    "2 -> 3: playbacks stored on handles with one cue each, playback settings at their defaults, no page legends".into()
}
