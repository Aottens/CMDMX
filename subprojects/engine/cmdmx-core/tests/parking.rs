//! Parking (PRK-01): `specs/modules/patch.md` PAT-29..34.
//!
//! API under test:
//! - `Fixture.parked: bool`, serde `default`, left out of the JSON when false. `universe` and
//!   `address` stay the remembered place.
//! - `Engine::patch_park(&[FixtureId])` and `Engine::patch_unpark(&[FixtureId])`: range commands
//!   (PAT-19); a fixture already in the asked state stays as it is.
//! - `Engine::patch_move_parking(ids, universe, address)` and `Engine::patch_add_parking(&PatchAdd)`
//!   (PAT-32): as `patch_move_range` and `patch_add`, but a fixture outside the move whose channels
//!   it would overlap is parked instead of refusing; a notice names them.
//! - `CoreError::Parked(number)` ("201 is parked") and `CoreError::UnparkOverlap { number, other,
//!   universe, address }` ("201 would overlap 1201 at 1.37: give it a free address").
//! - A parked fixture's channels are free (`spans` skip it); it is skipped live (PAT-30).
//!
//! Church rig (`common`): washes 101–104 at 1, 6, 11, 16 (5 ch); dimmers 201–208 at 21–28,
//! 221–226 at 29–34, 301–306 at 35–40. Ids equal numbers.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

mod common;

use cmdmx_core::attribute::AttributeGroup;
use cmdmx_core::effect::{EffectSet, Shape, Wave};
use cmdmx_core::engine::Engine;
use cmdmx_core::palette::{Cell, Record, RecordMode, Window};
use cmdmx_core::patch::{FixtureId, PatchAdd};
use cmdmx_core::playback::PlaybackRecord;
use cmdmx_core::programmer::Item;
use cmdmx_core::show::{Meta, Show, SCHEMA_VERSION};
use cmdmx_core::value::Value;
use cmdmx_core::CoreError;
use common::{church, dmx, select};
use serde_json::{json, Value as Json};

const TICK: f64 = 0.025;

fn run(e: &mut Engine, line: &str) -> Result<(), CoreError> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    e.execute_command(&tokens)
}

fn parked(e: &Engine, id: FixtureId) -> bool {
    e.fixture(id).unwrap().parked
}

fn place(e: &Engine, id: FixtureId) -> (u16, u16) {
    let f = e.fixture(id).unwrap();
    (f.universe, f.address)
}

fn snapshot(e: &Engine) -> Vec<cmdmx_core::patch::Fixture> {
    e.fixtures().cloned().collect()
}

fn dimmers(count: u32, address: u16, number: u32) -> PatchAdd {
    PatchAdd {
        profile: "dimmer".into(),
        mode: None,
        count,
        universe: 1,
        address: Some(address),
        number: Some(number),
    }
}

fn meta() -> Meta {
    Meta {
        name: "Park".into(),
        created: "2026-11-06T09:00:00Z".into(),
        modified: "2026-11-06T09:00:00Z".into(),
        app_version: "0.1.0".into(),
    }
}

/// 201..203 at full, recorded as a cue on 1.1 and as group 1; the programmer is cleared.
fn cue_and_group(e: &mut Engine) -> u32 {
    run(e, "201 Thro 203 Enter").unwrap();
    e.record_group(1).unwrap();
    run(e, "201 Thro 203 @ 100 Enter").unwrap();
    let pb = e.record_playback(&PlaybackRecord::at(1, 1)).unwrap();
    e.clear();
    e.clear();
    pb
}

fn tick_to(e: &mut Engine, from: f64, to: f64) {
    let mut t = from;
    while t + TICK < to - 1e-9 {
        t += TICK;
        e.tick(t);
    }
    e.tick(to);
}

// ---- PAT-29 ---------------------------------------------------------------------------------

#[test]
fn pat_29_park_takes_fixtures_out_of_the_live_lists_and_darkens_them() {
    let mut e = church();
    run(&mut e, "201 Thro 203 Enter").unwrap();
    e.record_group(1).unwrap();
    e.locate(&[AttributeGroup::I]).unwrap();
    assert!(e.located().keys().any(|i| i.fixture == 202));
    run(&mut e, "201 Thro 203 @ 100 Enter").unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, 22), 255);

    e.patch_park(&[202]).unwrap();
    assert!(parked(&e, 202));
    assert_eq!(place(&e, 202), (1, 22), "the place is remembered");
    assert_eq!(e.programmer().selection(), [201, 203]);
    assert!(!e.programmer().in_programmer(202));
    assert!(!e.located().keys().any(|i| i.fixture == 202));
    assert_eq!(e.group(1), Some(vec![201, 202, 203]), "groups keep it");
    e.tick(TICK);
    assert_eq!(dmx(&e, 22), 0, "its channel drops to 0");
    assert_eq!(dmx(&e, 21), 255);

    // Already parked stays as it is; the rest of the range is parked.
    e.patch_park(&[202, 203]).unwrap();
    assert!(parked(&e, 202) && parked(&e, 203));
    assert_eq!(place(&e, 203), (1, 23));
}

#[test]
fn pat_29_park_is_a_range_command() {
    let mut e = church();
    let before = snapshot(&e);
    assert_eq!(e.patch_park(&[]), Err(CoreError::EmptySelection));
    assert_eq!(
        e.patch_park(&[201, 999]),
        Err(CoreError::UnknownFixture(999))
    );
    assert_eq!(
        e.patch_park(&[201, 202, 201]),
        Err(CoreError::RepeatedFixture(201))
    );
    assert_eq!(e.patch_unpark(&[]), Err(CoreError::EmptySelection));
    assert_eq!(e.patch_unpark(&[999]), Err(CoreError::UnknownFixture(999)));
    assert_eq!(snapshot(&e), before, "nothing changes");
}

#[test]
fn pat_29_park_changes_nothing_recorded() {
    let mut e = church();
    let pb = cue_and_group(&mut e);
    let cues = e.playback(pb).unwrap().cues.clone();
    e.patch_park(&[202]).unwrap();
    assert_eq!(e.playback(pb).unwrap().cues, cues);
    assert_eq!(e.group(1), Some(vec![201, 202, 203]));
}

// ---- PAT-30 ---------------------------------------------------------------------------------

#[test]
fn pat_30_a_parked_fixture_cannot_be_selected() {
    let mut e = church();
    e.patch_park(&[202]).unwrap();
    let refused = e.select_fixture(202);
    assert_eq!(refused, Err(CoreError::Parked(202)));
    assert_eq!(refused.unwrap_err().to_string(), "202 is parked");
    assert_eq!(run(&mut e, "202 Enter"), Err(CoreError::Parked(202)));
    assert!(e.programmer().selection().is_empty());
}

#[test]
fn pat_30_ranges_groups_all_and_rem_dim_leave_it_out() {
    let mut e = church();
    run(&mut e, "201 Thro 203 Enter").unwrap();
    e.record_group(1).unwrap();
    e.clear_selection();
    e.patch_park(&[202]).unwrap();

    run(&mut e, "201 Thro 204 Enter").unwrap();
    assert_eq!(e.programmer().selection(), [201, 203, 204]);
    e.clear_selection();

    run(&mut e, "201 And 202 And 203 Enter").unwrap();
    assert_eq!(e.programmer().selection(), [201, 203]);
    e.clear_selection();

    e.select_group(1).unwrap();
    assert_eq!(e.programmer().selection(), [201, 203]);
    e.clear_selection();

    // PRG-06: All selects what is in the programmer.
    e.patch_unpark(&[202]).unwrap();
    run(&mut e, "201 And 202 And 306 @ 50 Enter").unwrap();
    e.clear_selection();
    e.patch_park(&[202]).unwrap();
    e.select_all();
    assert_eq!(e.programmer().selection(), [201, 306]);
    e.clear_selection();

    select(&mut e, &[201]);
    e.rem_dim().unwrap();
    assert!(!e.programmer().in_programmer(202), "Rem Dim leaves it out");
    assert!(e.programmer().in_programmer(203));
}

#[test]
fn pat_30_a_group_of_only_parked_fixtures_selects_nothing_and_warns() {
    let mut e = church();
    select(&mut e, &[202, 203]);
    e.record_group(3).unwrap();
    e.clear_selection();
    e.patch_park(&[202, 203]).unwrap();
    e.drain_warnings();
    e.select_group(3).unwrap();
    assert!(e.programmer().selection().is_empty());
    let warnings: Vec<String> = e.drain_warnings().into_iter().map(|w| w.message).collect();
    assert_eq!(warnings, ["Group 3: every fixture is parked"]);
    run(&mut e, "Group 3 Enter").unwrap();
    assert!(e.programmer().selection().is_empty());
    let warnings: Vec<String> = e.drain_warnings().into_iter().map(|w| w.message).collect();
    assert_eq!(warnings, ["Group 3: every fixture is parked"]);
}

#[test]
fn pat_30_removing_another_fixture_keeps_the_spread_over_what_hangs() {
    let mut e = church();
    let pb = spread_cue(&mut e, &[201, 202, 203, 204]);
    e.patch_park(&[202]).unwrap();
    e.patch_remove(&[204]).unwrap();
    let entry = &e.playback(pb).unwrap().cues[0].effects[0];
    assert_eq!(entry.count, 2, "201 and 203 hang");
}

/// A RampUp over `ids`, size 1, 60 BPM, spread 360, recorded on 1.1.
fn spread_cue(e: &mut Engine, ids: &[FixtureId]) -> u32 {
    select(e, ids);
    e.effect_apply(&cmdmx_core::effect::EffectApply::Shape {
        shape: Shape::Wave(Wave::RampUp),
        attribute: None,
    })
    .unwrap();
    e.effect_set(EffectSet::Size(1.0)).unwrap();
    e.effect_set(EffectSet::Bpm(60.0)).unwrap();
    e.effect_set(EffectSet::Spread(360.0)).unwrap();
    let pb = e.record_playback(&PlaybackRecord::at(1, 1)).unwrap();
    e.clear();
    e.clear();
    pb
}

#[test]
fn pat_30_an_effect_spreads_over_the_members_that_hang() {
    let mut parked_rig = church();
    let pb = spread_cue(&mut parked_rig, &[201, 202, 203]);
    parked_rig.patch_park(&[202]).unwrap();
    let mut removed_rig = church();
    spread_cue(&mut removed_rig, &[201, 202, 203]);
    removed_rig.patch_remove(&[202]).unwrap();

    let members = &parked_rig.playback(pb).unwrap().cues[0].effects[0].members;
    assert_eq!(members.len(), 3, "the parked member stays in the entry");

    for e in [&mut parked_rig, &mut removed_rig] {
        e.tick(0.0);
        e.set_fader(pb, 1.0, 0.0).unwrap();
        e.tick(0.0);
    }
    let mut from = 0.0;
    for to in [0.2, 0.35, 0.6, 0.95] {
        tick_to(&mut parked_rig, from, to);
        tick_to(&mut removed_rig, from, to);
        for address in [21, 23] {
            assert_eq!(
                dmx(&parked_rig, address),
                dmx(&removed_rig, address),
                "{address} at {to}"
            );
        }
        from = to;
    }
    assert_eq!(
        dmx(&parked_rig, 22),
        0,
        "the parked fixture outputs nothing"
    );
}

// ---- PAT-31 ---------------------------------------------------------------------------------

#[test]
fn pat_31_unpark_puts_it_back_and_it_plays_again() {
    let mut e = church();
    let pb = cue_and_group(&mut e);
    e.patch_park(&[202]).unwrap();
    e.tick(0.0);
    e.set_fader(pb, 1.0, 0.0).unwrap();
    tick_to(&mut e, 0.0, 0.1);
    assert_eq!((dmx(&e, 21), dmx(&e, 22)), (255, 0));

    e.patch_unpark(&[202]).unwrap();
    assert!(!parked(&e, 202));
    assert_eq!(place(&e, 202), (1, 22));
    tick_to(&mut e, 0.1, 0.2);
    assert_eq!(
        dmx(&e, 22),
        255,
        "plays its recorded data from the next tick"
    );
    e.select_fixture(202).unwrap();
    assert_eq!(e.programmer().selection(), [202]);

    // Not parked: stays as it is.
    e.patch_unpark(&[201]).unwrap();
    assert!(!parked(&e, 201));
}

#[test]
fn pat_31_parked_channels_are_free_and_unpark_onto_them_is_refused() {
    let mut e = church();
    e.patch_park(&[202, 203]).unwrap();
    let new = e.patch_add(&dimmers(1, 22, 1202)).unwrap();
    assert_eq!(
        place(&e, new[0]),
        (1, 22),
        "a parked fixture's channels are free"
    );

    let before = snapshot(&e);
    let refused = e.patch_unpark(&[203, 202]);
    assert_eq!(
        refused,
        Err(CoreError::UnparkOverlap {
            number: 202,
            other: 1202,
            universe: 1,
            address: 22
        })
    );
    assert_eq!(
        refused.unwrap_err().to_string(),
        "202 would overlap 1202 at 1.22: give it a free address"
    );
    assert_eq!(snapshot(&e), before, "the whole command changes nothing");
    assert!(parked(&e, 203));
}

#[test]
fn pat_31_move_unparks_at_the_new_address() {
    let mut e = church();
    e.patch_park(&[202]).unwrap();
    e.patch_move(202, 1, 200).unwrap();
    assert!(!parked(&e, 202));
    assert_eq!(place(&e, 202), (1, 200));

    e.patch_park(&[203, 204]).unwrap();
    e.patch_move_range(&[203, 204], 2, 1).unwrap();
    assert!(!parked(&e, 203) && !parked(&e, 204));
    assert_eq!((place(&e, 203), place(&e, 204)), ((2, 1), (2, 2)));
}

// ---- PAT-32 ---------------------------------------------------------------------------------

#[test]
fn pat_32_move_parks_the_fixtures_it_would_overlap() {
    let mut e = church();
    e.drain_notices();
    // Wash 101 (5 ch) onto 1.21 covers 201..205.
    e.patch_move_parking(&[101], 1, 21).unwrap();
    assert_eq!(place(&e, 101), (1, 21));
    for id in 201..=205 {
        assert!(parked(&e, id), "{id}");
    }
    assert!(!parked(&e, 206));
    assert_eq!(
        e.drain_notices(),
        ["Moved 101 to 1.21, parked 201, 202, 203, 204, 205"]
    );
}

#[test]
fn pat_32_a_range_move_does_not_park_its_own_fixtures() {
    let mut e = church();
    e.drain_notices();
    e.patch_move_parking(&[201, 202], 1, 22).unwrap();
    assert_eq!((place(&e, 201), place(&e, 202)), ((1, 22), (1, 23)));
    assert!(!parked(&e, 201) && !parked(&e, 202));
    assert!(parked(&e, 203));
    assert_eq!(e.drain_notices(), ["Moved 2 fixtures to 1.22, parked 203"]);
}

#[test]
fn pat_32_without_a_clash_it_is_a_plain_move() {
    let mut e = church();
    e.drain_notices();
    e.patch_move_parking(&[201], 1, 300).unwrap();
    assert_eq!(place(&e, 201), (1, 300));
    assert!(e.drain_notices().is_empty());
}

#[test]
fn pat_32_past_512_or_a_used_number_is_still_refused() {
    let mut e = church();
    let before = snapshot(&e);
    assert!(matches!(
        e.patch_move_parking(&[101], 1, 510),
        Err(CoreError::AddressOutOfRange { .. })
    ));
    assert_eq!(
        e.patch_add_parking(&dimmers(1, 21, 202)),
        Err(CoreError::NumberInUse(202))
    );
    assert_eq!(snapshot(&e), before);
}

#[test]
fn pat_32_add_parks_the_fixtures_it_would_overlap() {
    let mut e = church();
    e.drain_notices();
    let ids = e.patch_add_parking(&dimmers(2, 21, 1201)).unwrap();
    assert_eq!(ids.len(), 2);
    assert_eq!(place(&e, ids[0]), (1, 21));
    assert!(parked(&e, 201) && parked(&e, 202) && !parked(&e, 203));
    assert_eq!(
        e.drain_notices(),
        ["Added 2 fixtures at 1.21, parked 201, 202"]
    );
}

// ---- PAT-33 ---------------------------------------------------------------------------------

#[test]
fn pat_33_rerecording_a_group_keeps_its_parked_fixtures() {
    let mut e = church();
    run(&mut e, "201 Thro 203 Enter").unwrap();
    e.record_group(1).unwrap();
    e.clear_selection();
    e.patch_park(&[202]).unwrap();
    select(&mut e, &[204, 201]);
    e.record_group(1).unwrap();
    assert_eq!(e.group(1), Some(vec![204, 201, 202]));
}

#[test]
fn pat_33_recording_keeps_a_parked_fixtures_cue_data() {
    let mut e = church();
    let pb = cue_and_group(&mut e);
    let kept = e.playback(pb).unwrap().cues[0].values[&Item::fixture(202)].clone();
    e.patch_park(&[202]).unwrap();
    run(&mut e, "201 @ 50 Enter").unwrap();
    e.record_playback(&PlaybackRecord {
        mode: Some(RecordMode::Replace),
        ..PlaybackRecord::at(1, 1)
    })
    .unwrap();
    let values = &e.playback(pb).unwrap().cues[0].values;
    assert_eq!(values.get(&Item::fixture(202)), Some(&kept), "{values:?}");
}

#[test]
fn pat_33_recording_a_palette_keeps_a_parked_fixtures_value() {
    let mut e = church();
    let cell = Cell::new(Window::Positions, 1);
    run(&mut e, "201 Thro 202 Enter").unwrap();
    e.set_programmer_value(201, "Dimmer", 0.3).unwrap();
    e.set_programmer_value(202, "Dimmer", 0.6).unwrap();
    let id = e
        .record_palette(&Record {
            mask: Some(vec![AttributeGroup::I]),
            ..Record::at(cell)
        })
        .unwrap();
    e.clear();
    e.clear();
    e.patch_park(&[202]).unwrap();
    run(&mut e, "201 @ 50 Enter").unwrap();
    e.record_palette(&Record {
        mask: Some(vec![AttributeGroup::I]),
        mode: Some(RecordMode::Replace),
        ..Record::at(cell)
    })
    .unwrap();
    let palette = e.palettes().find(|p| p.id == id).unwrap();
    assert_eq!(
        palette.fixtures[&Item::fixture(202)]["Dimmer"],
        Value::Level(0.6)
    );
}

#[test]
fn pat_33_a_parked_fixture_can_be_removed() {
    let mut e = church();
    cue_and_group(&mut e);
    e.patch_park(&[202]).unwrap();
    e.patch_remove(&[202]).unwrap();
    assert!(e.fixture(202).is_none());
    assert_eq!(e.group(1), Some(vec![201, 203]));
}

// ---- PAT-34 ---------------------------------------------------------------------------------

#[test]
fn pat_34_the_show_file_marks_parked_fixtures_only() {
    assert_eq!(SCHEMA_VERSION, 13);
    let mut e = church();
    e.patch_park(&[202]).unwrap();
    let v: Json = serde_json::from_str(&e.to_show(meta()).to_json()).unwrap();
    let fixtures = v["patch"]["fixtures"].as_array().unwrap();
    for f in fixtures {
        let expected = if f["id"] == json!(202) {
            Some(&json!(true))
        } else {
            None
        };
        assert_eq!(f.get("parked"), expected, "{f}");
    }
    assert_eq!(v["schema_version"], json!(13));
}

#[test]
fn pat_34_a_parked_fixture_loads_parked_without_a_clash() {
    let mut e = church();
    e.patch_park(&[202]).unwrap();
    e.patch_add(&dimmers(1, 22, 1202)).unwrap();
    let (show, migrations) = Show::from_json(&e.to_show(meta()).to_json()).unwrap();
    assert!(migrations.is_empty(), "{migrations:?}");
    let (loaded, warnings) = Engine::from_show(&show);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(loaded.fixture(202).unwrap().parked);
    assert_eq!(place(&loaded, 202), (1, 22));
    assert!(loaded.fixture_by_number(1202).is_some());
}

#[test]
fn pat_34_schema_11_migrates_and_parks_nothing() {
    let e = church();
    let mut v: Json = serde_json::from_str(&e.to_show(meta()).to_json()).unwrap();
    v["schema_version"] = json!(11);
    let (show, migrations) = Show::from_json(&v.to_string()).unwrap();
    assert_eq!(migrations.len(), 2, "{migrations:?}");
    assert!(migrations[0].starts_with("11 -> 12"), "{migrations:?}");
    let (loaded, _) = Engine::from_show(&show);
    assert!(loaded.fixtures().all(|f| !f.parked));
}
