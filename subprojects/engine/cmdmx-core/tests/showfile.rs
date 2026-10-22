//! SHW-01..08: the show file (`specs/modules/showfile.md`).

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

mod common;

use std::collections::BTreeMap;

use cmdmx_core::attribute::{AttributeDef, AttributeGroup, AttributeKind};
use cmdmx_core::colour::ColourModel;
use cmdmx_core::engine::Engine;
use cmdmx_core::output::NodeKind;
use cmdmx_core::patch::{Fixture, PatchAdd};
use cmdmx_core::playback::{FaderMode, GroupMaster, GroupMode, Master, Playback, Priority};
use cmdmx_core::profile::Profile;
use cmdmx_core::show::{Meta, Show, SCHEMA_VERSION};
use serde_json::{json, Value as Json};

const FIXTURE_SHOW: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/fixtures/shows/church-demo.cmdmx.json"
);
const SHOWTEC: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/fixtures/qlcplus/Showtec-LED-Par-56.expected.json"
);

fn meta() -> Meta {
    Meta {
        name: "Church demo".into(),
        created: "2026-10-18T09:00:00Z".into(),
        modified: "2026-10-18T09:00:00Z".into(),
        app_version: "0.1.0".into(),
    }
}

/// The church rig of `common::church()`, plus a Showtec LED Par 56 (QLC+ profile) at 2.1,
/// groups 1 (all, patch order) and 4 (spots, reversed), playback 1 (washes at full, wash 101
/// Pan at half) and playback 2 at High priority in fader mode 2, colour model RGB and 10 faders
/// per page, plus the default output nodes of universe 2 (OUT-25) next to those of a new engine.
/// `tests/fixtures/shows/church-demo.cmdmx.json` is exactly this rig written out.
fn rig() -> Engine {
    let mut e = common::church();
    let showtec = Profile::from_json(&std::fs::read_to_string(SHOWTEC).unwrap()).unwrap();
    e.add_profile("Showtec/LED Par 56", showtec).unwrap();
    e.patch(Fixture {
        id: 401,
        number: 401,
        name: "Organ par".into(),
        handle: None,
        profile: "Showtec/LED Par 56".into(),
        mode: Some("6 Channel".into()),
        universe: 2,
        address: 1,
    })
    .unwrap();
    let all: Vec<u32> = e.fixtures().map(|f| f.id).collect();
    common::select(&mut e, &all);
    e.record_group(1).unwrap();
    e.clear_selection();
    common::select(&mut e, &[306, 305, 304, 303, 302, 301]);
    e.record_group(4).unwrap();
    e.clear_selection();
    let mut washes: BTreeMap<u32, BTreeMap<String, cmdmx_core::value::Stored>> = (101..=104)
        .map(|id| (id, BTreeMap::from([("Dimmer".to_string(), 1.0.into())])))
        .collect();
    washes
        .get_mut(&101)
        .unwrap()
        .insert("Pan".into(), 0.5.into());
    e.add_playback(Playback::new(1, 1, 1, washes));
    e.add_playback(Playback {
        priority: Priority::High,
        fader_mode: FaderMode::Mode2,
        ..Playback::new(
            2,
            1,
            2,
            BTreeMap::from([(301, BTreeMap::from([("Dimmer".to_string(), 0.8.into())]))]),
        )
    });
    e.set_colour_model(ColourModel::Rgb);
    // OUT-27: what migrating the schema 5 file gives, so every older file migrates to this one.
    e.output_add(2, true, NodeKind::default_artnet(2)).unwrap();
    e.output_add(2, true, NodeKind::default_sacn(2)).unwrap();
    e
}

fn rig_with_settings() -> Engine {
    // Settings travel with the show: load the rig's show with 10 faders per page.
    let mut show = rig().to_show(meta());
    show.settings.faders_per_page = 10;
    let (e, warnings) = Engine::from_show(&show);
    assert!(warnings.is_empty(), "{warnings:?}");
    e
}

/// Top-level keys of the written file, in order (lines with exactly two spaces of indent).
fn top_level_keys(json: &str) -> Vec<String> {
    json.lines()
        .filter(|l| l.starts_with("  \"") && !l.starts_with("   "))
        .map(|l| l.trim_start().split('"').nth(1).unwrap().to_string())
        .collect()
}

fn as_json(show: &Show) -> Json {
    serde_json::from_str(&show.to_json()).unwrap()
}

fn load(json: &Json) -> (Engine, Vec<String>) {
    let (show, migrations) = Show::from_json(&json.to_string()).unwrap();
    assert!(migrations.is_empty());
    Engine::from_show(&show)
}

// ---- SHW-01 ------------------------------------------------------------------

#[test]
fn shw_01_top_level_keys_in_order() {
    let json = rig().to_show(meta()).to_json();
    assert_eq!(
        top_level_keys(&json),
        [
            "schema_version",
            "meta",
            "settings",
            "profiles",
            "patch",
            "groups",
            "palettes",
            "pages",
            "playbacks",
            "output"
        ]
    );
    let v: Json = serde_json::from_str(&json).unwrap();
    assert_eq!(v["schema_version"], json!(SCHEMA_VERSION));
    assert_eq!(SCHEMA_VERSION, 6); // OUT-13
    assert_eq!(
        v["meta"],
        json!({"name": "Church demo", "created": "2026-10-18T09:00:00Z",
               "modified": "2026-10-18T09:00:00Z", "app_version": "0.1.0"})
    );
}

#[test]
fn shw_01_settings_hold_engine_settings_and_colour_model() {
    let v = as_json(&rig_with_settings().to_show(meta()));
    let s = &v["settings"];
    assert_eq!(s["faders_per_page"], json!(10));
    assert_eq!(s["handles_per_page"], json!(40));
    assert_eq!(s["colour_model"], json!("Rgb"));
    for key in [
        "ltp_trigger_level",
        "wheel_step_normal",
        "wheel_step_fine",
        "wheel_step_coarse",
    ] {
        assert!(s[key].is_number(), "settings.{key} missing");
    }
}

#[test]
fn shw_01_patch_groups_and_playbacks_shape() {
    let v = as_json(&rig().to_show(meta()));
    let par = v["patch"]["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"] == json!(401))
        .unwrap();
    assert_eq!(
        *par,
        json!({"id": 401, "number": 401, "name": "Organ par", "profile": "Showtec/LED Par 56",
               "mode": "6 Channel", "universe": 2, "address": 1,
               "handle": {"page": 1, "slot": 25}})
    );
    assert_eq!(
        v["groups"][1],
        json!({"id": 4, "fixtures": [306, 305, 304, 303, 302, 301]})
    );
    assert_eq!(v["playbacks"]["playbacks"][1]["priority"], json!("High"));
    assert_eq!(v["playbacks"]["playbacks"][1]["fader_mode"], json!(2));
    assert_eq!(
        v["playbacks"]["playbacks"][0]["cues"][0]["values"]["101"]["Pan"],
        json!(0.5)
    );
}

// ---- SHW-02 ------------------------------------------------------------------

#[test]
fn shw_02_writing_twice_is_byte_identical() {
    let show = rig().to_show(meta());
    let a = show.to_json();
    assert_eq!(a, show.to_json());
    assert_eq!(a, rig().to_show(meta()).to_json());
    assert!(a.ends_with("}\n"));
    assert!(
        a.contains("\n  \"meta\": {\n    \"name\""),
        "two-space indents"
    );
}

#[test]
fn shw_02_load_then_write_changes_nothing() {
    let a = rig().to_show(meta()).to_json();
    let (show, _) = Show::from_json(&a).unwrap();
    assert_eq!(show.to_json(), a);
}

#[test]
fn shw_02_collections_sorted_by_id() {
    let mut e = rig();
    // Patched, grouped and added out of order.
    e.patch(Fixture {
        id: 50,
        number: 50,
        name: String::new(),
        handle: None,
        profile: "dimmer".into(),
        mode: None,
        universe: 3,
        address: 1,
    })
    .unwrap();
    common::select(&mut e, &[50]);
    e.record_group(2).unwrap();
    e.add_playback(Playback::new(0, 1, 3, BTreeMap::new()));
    let v = as_json(&e.to_show(meta()));
    let ids = |key: &str, list: &Json| -> Vec<u64> {
        list.as_array()
            .unwrap()
            .iter()
            .map(|x| x[key].as_u64().unwrap())
            .collect()
    };
    let fixtures = ids("id", &v["patch"]["fixtures"]);
    let mut sorted = fixtures.clone();
    sorted.sort();
    assert_eq!(fixtures, sorted);
    assert_eq!(ids("id", &v["groups"]), [1, 2, 4]);
    assert_eq!(ids("id", &v["playbacks"]["playbacks"]), [0, 1, 2]);
}

// ---- SHW-03 ------------------------------------------------------------------

#[test]
fn shw_03_engine_show_engine_round_trip() {
    let show = rig_with_settings().to_show(meta());
    let (engine, warnings) = Engine::from_show(&show);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(engine.to_show(meta()), show);
    assert_eq!(engine.settings().faders_per_page, 10);
    assert_eq!(engine.colour_model(), ColourModel::Rgb);
}

#[test]
fn shw_03_loaded_engine_outputs_the_same_frames() {
    let mut original = rig();
    let (mut loaded, _) = Engine::from_show(&original.to_show(meta()));
    let mut t = 0.0;
    for (pb, level) in [(1, 1.0), (2, 0.5), (1, 0.3), (2, 0.0)] {
        original.set_fader(pb, level, t).unwrap();
        loaded.set_fader(pb, level, t).unwrap();
        original.tick(t);
        loaded.tick(t);
        for u in [1, 2] {
            assert_eq!(
                original.frame(u).map(|f| f.slots().to_vec()),
                loaded.frame(u).map(|f| f.slots().to_vec()),
                "universe {u} after playback {pb} at {level}"
            );
        }
        t += 0.025;
    }
}

#[test]
fn shw_03_fixture_show_is_the_church_rig() {
    let file = std::fs::read_to_string(FIXTURE_SHOW).unwrap();
    assert_eq!(rig().to_show(meta()).to_json(), file);
    let (show, migrations) = Show::from_json(&file).unwrap();
    assert!(migrations.is_empty());
    let (engine, warnings) = Engine::from_show(&show);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(engine.to_show(meta()).to_json(), file);
}

// ---- SHW-04 ------------------------------------------------------------------

#[test]
fn shw_04_newer_schema_is_refused_with_a_clear_message() {
    let mut v = as_json(&rig().to_show(meta()));
    v["schema_version"] = json!(7);
    let err = Show::from_json(&v.to_string()).unwrap_err().to_string();
    assert!(err.contains("Church demo"), "{err}");
    assert!(err.contains("newer"), "{err}");
    assert!(err.contains("schema 7"), "{err}");
    assert!(err.contains("up to 6"), "{err}");
}

#[test]
fn shw_04_not_json_or_no_version_is_refused() {
    assert!(Show::from_json("this is not a show").is_err());
    let mut v = as_json(&rig().to_show(meta()));
    v.as_object_mut().unwrap().remove("schema_version");
    assert!(Show::from_json(&v.to_string()).is_err());
}

#[test]
fn shw_04_current_version_runs_no_migrations() {
    let (_, migrations) = Show::from_json(&rig().to_show(meta()).to_json()).unwrap();
    assert!(migrations.is_empty());
}

// ---- SHW-05 ------------------------------------------------------------------

#[test]
fn shw_05_only_used_profiles_are_saved() {
    let mut e = rig();
    e.add_profile("unused", common::wash_profile()).unwrap();
    let show = e.to_show(meta());
    let keys: Vec<&String> = show.profiles.keys().collect();
    assert_eq!(keys, ["Showtec/LED Par 56", "dimmer", "wash"]);
}

#[test]
fn shw_05_load_uses_the_embedded_profile() {
    let mut v = as_json(&rig().to_show(meta()));
    // The embedded wash differs from the rig's: Zoom moved from channel 5 to channel 4 and Tilt dropped.
    let attrs = &mut v["profiles"]["wash"]["modes"][0]["attributes"];
    let attrs = attrs.as_array_mut().unwrap();
    attrs.retain(|a| a["name"] != json!("Tilt"));
    for a in attrs.iter_mut() {
        if a["name"] == json!("Zoom") {
            a["channel"] = json!(4);
        }
    }
    let (engine, warnings) = load(&v);
    assert!(warnings.is_empty(), "{warnings:?}");
    let mode = engine.fixture_mode(101).unwrap();
    assert!(mode.attribute("Tilt").is_none());
    assert_eq!(mode.attribute("Zoom").unwrap().channel, 4);
}

// ---- SHW-06 ------------------------------------------------------------------

#[test]
fn shw_06_ids_are_never_reused_after_a_load() {
    let mut e = Engine::new(Default::default());
    e.add_profile(
        "dimmer",
        Profile::from_attributes(vec![AttributeDef::new(
            "Dimmer",
            AttributeGroup::I,
            AttributeKind::Htp,
            1,
        )]),
    )
    .unwrap();
    let add = PatchAdd {
        profile: "dimmer".into(),
        mode: None,
        count: 3,
        universe: 1,
        address: None,
        number: None,
    };
    assert_eq!(e.patch_add(&add).unwrap(), [1, 2, 3]);
    e.patch_remove(&[3]).unwrap();
    let show = e.to_show(meta());
    assert_eq!(show.patch.next_id, 4);
    let (mut loaded, _) = Engine::from_show(&show);
    let one = PatchAdd { count: 1, ..add };
    assert_eq!(loaded.patch_add(&one).unwrap(), [4]);
}

// ---- SHW-07 ------------------------------------------------------------------
// One warning per left-out item; references to a fixture that was left out for another
// reason (its group memberships, its playback values) go with it without a warning of their own.

fn fixture_ids(e: &Engine) -> Vec<u32> {
    e.fixtures().map(|f| f.id).collect()
}

fn set_fixture(v: &mut Json, id: u32, key: &str, value: Json) {
    let f = v["patch"]["fixtures"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|f| f["id"] == json!(id))
        .unwrap();
    f[key] = value;
}

#[test]
fn shw_07_invalid_profile_leaves_out_its_fixtures() {
    let mut v = as_json(&rig().to_show(meta()));
    v["profiles"]["wash"]["modes"][0]["footprint"] = json!(0);
    let (engine, warnings) = load(&v);
    assert!(warnings.iter().any(|w| w.contains("wash")), "{warnings:?}");
    let ids = fixture_ids(&engine);
    assert!(!ids.iter().any(|id| (101..=104).contains(id)));
    assert!(
        ids.contains(&201) && ids.contains(&401),
        "the rest still loads"
    );
}

#[test]
fn shw_07_missing_profile_or_mode_leaves_out_that_fixture() {
    let mut v = as_json(&rig().to_show(meta()));
    set_fixture(&mut v, 201, "profile", json!("Nobody/Nothing"));
    set_fixture(&mut v, 401, "mode", json!("99 Channel"));
    let (engine, warnings) = load(&v);
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(warnings.iter().any(|w| w.contains("201")), "{warnings:?}");
    assert!(warnings.iter().any(|w| w.contains("401")), "{warnings:?}");
    let ids = fixture_ids(&engine);
    assert!(!ids.contains(&201) && !ids.contains(&401));
    assert!(ids.contains(&202));
}

#[test]
fn shw_07_clashing_fixture_is_left_out_the_earlier_one_stays() {
    let mut v = as_json(&rig().to_show(meta()));
    // 302 onto 301's address.
    let address_301 = v["patch"]["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"] == json!(301))
        .unwrap()["address"]
        .clone();
    set_fixture(&mut v, 302, "address", address_301);
    let (engine, warnings) = load(&v);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("302"), "{warnings:?}");
    assert_eq!(engine.group(4), Some(&[306, 305, 304, 303, 301][..]));
    let ids = fixture_ids(&engine);
    assert!(ids.contains(&301) && !ids.contains(&302));
}

#[test]
fn shw_07_dangling_group_members_and_playback_values_are_left_out() {
    let mut v = as_json(&rig().to_show(meta()));
    v["groups"][1]["fixtures"]
        .as_array_mut()
        .unwrap()
        .push(json!(999));
    v["playbacks"]["playbacks"][0]["cues"][0]["values"]["998"] = json!({"Dimmer": 1.0});
    let (engine, warnings) = load(&v);
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(warnings.iter().any(|w| w.contains("999")), "{warnings:?}");
    assert!(warnings.iter().any(|w| w.contains("998")), "{warnings:?}");
    assert_eq!(engine.group(4), Some(&[306, 305, 304, 303, 302, 301][..]));
}

#[test]
fn shw_07_saving_after_a_remove_loads_without_warnings() {
    // Playback values of a removed fixture are not written, so the saved show loads clean.
    let mut e = rig();
    e.patch_remove(&[101]).unwrap();
    let show = e.to_show(meta());
    assert!(!show.to_json().contains("\"101\""));
    let (_, warnings) = Engine::from_show(&show);
    assert!(warnings.is_empty(), "{warnings:?}");
}

// ---- SHW-08 ------------------------------------------------------------------

#[test]
fn shw_08_no_machine_specific_data() {
    let json = rig().to_show(meta()).to_json();
    assert!(!json.contains("/Users/"));
    let keys = top_level_keys(&json);
    // OUT-13 amends SHW-08: `output` is the grouped key it foresaw; `control` stays out.
    assert!(!keys.iter().any(|k| k == "control"));
}

#[test]
fn shw_03_a_group_emptied_by_removes_survives_the_round_trip() {
    let mut e = rig();
    e.patch_remove(&[301, 302, 303, 304, 305, 306]).unwrap();
    let show = e.to_show(meta());
    let (loaded, warnings) = Engine::from_show(&show);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(loaded.to_show(meta()), show);
}

// ---- GRM-13, 14: group masters (`specs/modules/group-masters.md`) -------------

const V4_SHOW: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/fixtures/shows/church-demo-v4.cmdmx.json"
);

/// The rig with a Scale master of group 4 (the spots) on 1.8 at 25 % (playback 3).
fn rig_with_group_master() -> Engine {
    let mut e = rig();
    let id = e.record_group_master(1, 8, 4).unwrap();
    assert_eq!(id, 3);
    e.set_fader(2, 1.0, 0.0).unwrap();
    e.set_fader(id, 0.25, 0.0).unwrap();
    e
}

fn group_master_of(e: &Engine, id: u32) -> GroupMaster {
    match &e.playback(id).unwrap().master {
        Some(Master::Group(m)) => m.clone(),
        other => panic!("not a group master: {other:?}"),
    }
}

#[test]
fn grm_13_a_group_master_is_saved_with_its_level() {
    let e = rig_with_group_master();
    let v = as_json(&e.to_show(meta()));
    assert_eq!(v["schema_version"], json!(6));
    let pb = &v["playbacks"]["playbacks"][2];
    assert_eq!(pb["kind"], json!("GroupMaster"));
    assert_eq!(
        pb["master"],
        json!({"group": 4, "mode": "Scale", "scale": 100, "enabled": true, "level": 0.25})
    );
    assert_eq!(pb["cues"], json!([]));
}

#[test]
fn grm_13_a_restart_keeps_the_masters_level_and_the_stage() {
    let mut e = rig_with_group_master();
    e.tick(0.0);
    let show = e.to_show(meta());
    let (parsed, migrations) = Show::from_json(&show.to_json()).unwrap();
    assert!(migrations.is_empty(), "{migrations:?}");
    let (mut loaded, warnings) = Engine::from_show(&parsed);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(loaded.to_show(meta()), show);
    assert_eq!(loaded.playback_state(3).map(|s| s.level), Some(0.25));
    loaded.set_fader(2, 1.0, 0.0).unwrap(); // playback levels are live state (SHW decision 3)
    loaded.tick(0.0);
    assert_eq!(loaded.frame(1), e.frame(1));
    assert_eq!(common::dmx(&loaded, 35), 51, "301: 80 % × 25 %");
}

#[test]
fn grm_13_a_schema_4_show_is_migrated() {
    let file = std::fs::read_to_string(V4_SHOW).unwrap();
    let (show, migrations) = Show::from_json(&file).unwrap();
    assert_eq!(migrations.len(), 2, "{migrations:?}");
    assert!(migrations[0].starts_with("4 -> 5"), "{migrations:?}");
    assert!(migrations[1].starts_with("5 -> 6"), "{migrations:?}"); // OUT-27
    let (engine, warnings) = Engine::from_show(&show);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(
        engine.to_show(meta()).to_json(),
        std::fs::read_to_string(FIXTURE_SHOW).unwrap(),
        "4 -> 5 changes nothing but the version; 5 -> 6 adds the nodes of universes 1 and 2"
    );
}

/// Load the group-master rig with playback 3 (the master) edited by `f`.
fn load_edited(f: impl FnOnce(&mut Json)) -> (Engine, Vec<String>) {
    let mut v = as_json(&rig_with_group_master().to_show(meta()));
    f(&mut v["playbacks"]["playbacks"][2]);
    load(&v)
}

#[test]
fn grm_14_a_master_of_a_missing_group_is_kept_and_masters_nothing() {
    let (mut e, warnings) = load_edited(|pb| pb["master"]["group"] = json!(9));
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("group 9"), "{warnings:?}");
    assert_eq!(group_master_of(&e, 3).group, 9);
    e.set_fader(2, 1.0, 0.0).unwrap();
    e.tick(0.0);
    assert_eq!(common::dmx(&e, 35), 204);
}

#[test]
fn grm_14_options_out_of_range_get_their_default() {
    for (edit, expect) in [
        (json!({"scale": 300}), (GroupMode::Scale, 100, 0.25)),
        (json!({"level": 1.5}), (GroupMode::Scale, 100, 1.0)),
        (
            json!({"mode": "Htp", "level": -0.5}),
            (GroupMode::Htp, 100, 0.0),
        ),
        (json!({"mode": "Bogus"}), (GroupMode::Scale, 100, 0.25)),
    ] {
        let (e, warnings) = load_edited(|pb| {
            for (k, v) in edit.as_object().unwrap() {
                pb["master"][k] = v.clone();
            }
        });
        assert_eq!(warnings.len(), 1, "{edit}: {warnings:?}");
        let m = group_master_of(&e, 3);
        assert_eq!((m.mode, m.scale, m.level), expect, "{edit}");
        assert_eq!(
            e.playback_state(3).map(|s| s.level),
            Some(expect.2),
            "{edit}"
        );
    }
}

#[test]
fn grm_14_a_group_master_with_cues_loads_without_them() {
    let (e, warnings) = load_edited(|pb| {
        pb["cues"] = json!([{"delay": 0.0, "fade_in": 0.0, "fade_out": 0.0, "overlap": 100.0,
            "number": 1, "legend": "", "link": {"type": "Wait"}, "track": "Track",
            "group_times": {}, "values": {"301": {"Dimmer": 1.0}}}]);
    });
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(e.playback(3).unwrap().cues.is_empty());
}
