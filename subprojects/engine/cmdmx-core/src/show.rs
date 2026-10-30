//! The show file: one JSON document holding everything the operator programmed
//! (`specs/modules/showfile.md` SHW-01..08). Pure: no I/O, no clock; times come in as parameters.
//!
//! `Engine::to_show` and `Engine::from_show` (in `engine.rs`, where the private state lives)
//! convert between an engine and a [`Show`]. Field order here is the key order in the file.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::attribute::AttributeGroup;
use crate::colour::ColourModel;
use crate::effect::Entry;
use crate::output::{Node, NodeId, NodeKind};
use crate::palette::{Palette, PaletteId, RecordBy};
use crate::patch::{Fixture, FixtureId};
use crate::playback::{HandleButtons, ListFader, Playback, PlaybackId, UpdateMode};
use crate::profile::Profile;
use crate::programmer::Grid;
use crate::programmer::{default_lowlight, ClearLtp, ClearPrecedence, GroupId, Item};
use crate::{CoreError, UniverseId};

/// SHW-04: every change to the schema raises this and comes with a step in [`migrate`].
pub const SCHEMA_VERSION: u32 = 9;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    pub name: String,
    pub created: String,
    pub modified: String,
    pub app_version: String,
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

fn is_default_lowlight(v: &f32) -> bool {
    *v == default_lowlight()
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
    /// CUE-38.
    pub auto_connect: bool,
    pub cue_list_fader: ListFader,
    pub update_mode: UpdateMode,
    pub handle_buttons: HandleButtons,
    /// PRG-42; absent in older files, so they load at the defaults, and left out while at them,
    /// so a show that never touched them saves as before (no schema bump).
    #[serde(default, skip_serializing_if = "is_default")]
    pub clear_precedence: ClearPrecedence,
    #[serde(default, skip_serializing_if = "is_default")]
    pub clear_ltp: ClearLtp,
    #[serde(
        default = "default_lowlight",
        skip_serializing_if = "is_default_lowlight"
    )]
    pub lowlight_level: f32,
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
    /// In recorded order: whole fixtures and cells (PRG-49).
    pub items: Vec<Item>,
    /// PRG-79: the grid the group was recorded with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid: Option<Grid>,
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

/// OUT-13: `next_node_id` is above every node id ever used, so ids are never reused.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShowOutput {
    pub next_node_id: NodeId,
    pub nodes: Vec<ShowNode>,
}

/// An output node as saved, or an entry that is not one (OUT-28: left out with a warning on load).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ShowNode {
    Node(Node),
    Unreadable(serde_json::Value),
}

impl<'de> Deserialize<'de> for ShowNode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(match serde_json::from_value(value.clone()) {
            Ok(node) => ShowNode::Node(node),
            Err(_) => ShowNode::Unreadable(value),
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
    /// OUT-13.
    pub output: ShowOutput,
}

impl Show {
    /// SHW-02: pretty JSON, two-space indents, keys in declaration order, trailing newline.
    pub fn to_json(&self) -> String {
        let mut json = serde_json::to_string_pretty(self).unwrap_or_default();
        json.push('\n');
        json
    }

    /// SHW-04: parse a show file, migrating an older schema step by step. Returns the show and
    /// the migrations that ran, then a line for each effect entry left out (FX-28). Not JSON, no
    /// `schema_version` or a newer schema is refused.
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
        let mut migrations = migrate(&mut value, version);
        migrations.extend(lenient_effects(&mut value));
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
    if version < 4 {
        migrations.push(migrate_3_to_4(value));
    }
    if version < 5 {
        if let Some(show) = value.as_object_mut() {
            show.insert("schema_version".into(), 5.into());
        }
        migrations.push("4 -> 5: group masters may be saved, nothing else changes".into());
    }
    if version < 6 {
        migrations.push(migrate_5_to_6(value));
    }
    if version < 7 {
        migrations.push(migrate_6_to_7(value));
    }
    if version < 8 {
        if let Some(show) = value.as_object_mut() {
            show.insert("schema_version".into(), 8.into());
        }
        migrations.push("7 -> 8: cues and palettes may hold effects, nothing else changes".into());
    }
    if version < 9 {
        if let Some(show) = value.as_object_mut() {
            show.insert("schema_version".into(), 9.into());
        }
        migrations
            .push("8 -> 9: groups may store a grid, cues an order, nothing else changes".into());
    }
    migrations
}

/// FX-28: leave out each effect entry of a cue or palette that does not read as one (an unknown
/// shape, parameters out of range, no attributes), so the rest of the show loads. One line each.
fn lenient_effects(value: &mut serde_json::Value) -> Vec<String> {
    use serde_json::Value;
    let mut lines = Vec::new();
    let mut scrub = |owner: String, holder: &mut Value| {
        let Some(list) = holder.get_mut("effects").and_then(Value::as_array_mut) else {
            return;
        };
        let mut n = 0;
        list.retain(|raw| {
            n += 1;
            let read = serde_json::from_value::<Entry>(raw.clone()).map_err(|e| e.to_string());
            let read = read.and_then(|e| match e.attributes.is_empty() {
                true => Err("no attributes".into()),
                false => e.params.validate().map_err(|e| e.to_string()),
            });
            if let Err(why) = &read {
                lines.push(format!("effect {n} of {owner} left out: {why}"));
            }
            read.is_ok()
        });
    };
    let playbacks = (value.get_mut("playbacks")).and_then(|p| p.get_mut("playbacks"));
    for playback in playbacks
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        let id = playback.get("id").map(Value::to_string).unwrap_or_default();
        let cues = playback.get_mut("cues").and_then(Value::as_array_mut);
        for cue in cues.into_iter().flatten() {
            let number = cue.get("number").map(Value::to_string).unwrap_or_default();
            scrub(format!("cue {number} of playback {id}"), cue);
        }
    }
    let palettes = (value.get_mut("palettes")).and_then(|p| p.get_mut("palettes"));
    for palette in palettes.and_then(Value::as_array_mut).into_iter().flatten() {
        let number = palette
            .get("number")
            .map(Value::to_string)
            .unwrap_or_default();
        scrub(format!("palette {number}"), palette);
    }
    lines
}

/// PRG-49, 52: a group holds items, not fixture ids: `[901, 902]` becomes `["901", "902"]`.
fn migrate_6_to_7(value: &mut serde_json::Value) -> String {
    if let Some(show) = value.as_object_mut() {
        let groups = show
            .get_mut("groups")
            .and_then(serde_json::Value::as_array_mut);
        for group in groups.into_iter().flatten() {
            let Some(group) = group.as_object_mut() else {
                continue;
            };
            if let Some(serde_json::Value::Array(fixtures)) = group.remove("fixtures") {
                let items: Vec<_> = fixtures
                    .iter()
                    .filter_map(serde_json::Value::as_u64)
                    .map(|f| serde_json::Value::String(f.to_string()))
                    .collect();
                group.insert("items".into(), items.into());
            }
        }
        show.insert("schema_version".into(), 7.into());
    }
    "6 -> 7: groups store items (whole fixtures and cells), nothing else changes".into()
}

/// OUT-27: line 1's defaults plus serial `auto`, then the defaults of every other universe the
/// patch uses, in ascending order.
fn migrate_5_to_6(value: &mut serde_json::Value) -> String {
    let Some(show) = value.as_object_mut() else {
        return "5 -> 6: not an object, left as it is".into();
    };
    let used: std::collections::BTreeSet<UniverseId> = show
        .get("patch")
        .and_then(|p| p.get("fixtures"))
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|f| f.get("universe")?.as_u64())
        .filter_map(|u| UniverseId::try_from(u).ok())
        .filter(|&u| u > 1)
        .collect();
    let mut kinds = vec![
        (1, NodeKind::default_artnet(1)),
        (1, NodeKind::default_sacn(1)),
        (
            1,
            NodeKind::Serial {
                port: "auto".into(),
            },
        ),
    ];
    for &line in &used {
        kinds.push((line, NodeKind::default_artnet(line)));
        kinds.push((line, NodeKind::default_sacn(line)));
    }
    let nodes: Vec<serde_json::Value> = (1..)
        .zip(kinds)
        .filter_map(|(id, (line, kind))| {
            serde_json::to_value(Node {
                id,
                line,
                enabled: true,
                kind,
            })
            .ok()
        })
        .collect();
    show.insert(
        "output".into(),
        serde_json::json!({ "next_node_id": nodes.len() + 1, "nodes": nodes }),
    );
    show.insert("schema_version".into(), 6.into());
    let lines: Vec<String> = std::iter::once(1)
        .chain(used)
        .map(|u| u.to_string())
        .collect();
    format!(
        "5 -> 6: output nodes added: Art-Net and sACN for universe {}, serial auto for universe 1",
        lines.join(", ")
    )
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

/// CUE-38: each playback's cues are numbered 1, 2, 3 … in their order and gain the cue fields;
/// the playbacks gain kind `Cues` and the default list and chase options.
fn migrate_3_to_4(value: &mut serde_json::Value) -> String {
    use serde_json::json;
    let Some(show) = value.as_object_mut() else {
        return "3 -> 4: not an object, left as it is".into();
    };
    if let Some(settings) = show
        .get_mut("settings")
        .and_then(serde_json::Value::as_object_mut)
    {
        for (key, default) in [
            ("auto_connect", json!(true)),
            ("cue_list_fader", json!("KillWithOff")),
            ("update_mode", json!("Forwards")),
            ("handle_buttons", json!("BestPractice")),
        ] {
            settings.insert(key.into(), default);
        }
    }
    let playbacks = show
        .get_mut("playbacks")
        .and_then(|p| p.get_mut("playbacks"))
        .and_then(serde_json::Value::as_array_mut);
    for playback in playbacks.into_iter().flatten() {
        let Some(playback) = playback.as_object_mut() else {
            continue;
        };
        playback.insert("kind".into(), json!("Cues"));
        playback.insert(
            "list".into(),
            json!({"tracking": true, "fader": "KillWithOff", "fire_first_cue": true,
                "loop": "StopOnFinal"}),
        );
        playback.insert(
            "chase".into(),
            json!({"bpm": 120.0, "crossfade": 0.0, "direction": "Forward", "loop": "Loop",
                "speed_source": "Free"}),
        );
        let cues = playback
            .get_mut("cues")
            .and_then(serde_json::Value::as_array_mut);
        for (n, cue) in (1..).zip(cues.into_iter().flatten()) {
            if let Some(cue) = cue.as_object_mut() {
                cue.insert("number".into(), json!(n));
                cue.insert("legend".into(), json!(""));
                cue.insert("link".into(), json!({"type": "Wait"}));
                cue.insert("track".into(), json!("Track"));
                cue.insert("group_times".into(), json!({}));
            }
        }
    }
    show.insert("schema_version".into(), 4.into());
    "3 -> 4: cues numbered 1, 2, 3 … with legends, links, tracking and group times at their defaults, playbacks are memories with default list and chase options, cue list settings added".into()
}
