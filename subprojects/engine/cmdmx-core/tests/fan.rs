//! Fan in the core programmer: `specs/modules/programmer.md` PRG-56..62 (VS2-35).
//! PRG-59 on the frame is `tests/scenarios/PRG-59-*.json`.
//!
//! API under test:
//! - `programmer::FanCurve` (`Line`, `Mirror`, `Wings`, `Arrow`; default `Line`) and
//!   `programmer::fan_factors(curve, n) -> Vec<f32>` (PRG-58).
//! - `Engine::fan_key()` toggles; `Engine::set_fan(Some(curve))` turns Fan on with that curve and
//!   makes it the last curve, `set_fan(None)` turns it off (IPC-43's three forms);
//!   `Engine::fan() -> Option<FanCurve>`, `Engine::fan_curve() -> FanCurve` (PRG-56).
//! - `Engine::fan_list(wheel) -> Vec<Item>`: the targets that have the wheel's attribute, in
//!   target order, cells as items (PRG-57). A colour-model wheel means Colour.
//! - `ProgrammerView::{fan, fan_curve}` (PRG-62).

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

mod common;

use cmdmx_core::attribute::AttributeGroup;
use cmdmx_core::colour::TargetColour;
use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::patch::{Fixture, FixtureId};
use cmdmx_core::playback::{Playback, Values};
use cmdmx_core::profile::Profile;
use cmdmx_core::programmer::{fan_factors, FanCurve, Item, Resolution::Coarse};
use cmdmx_core::undo::History;
use cmdmx_core::value::Value;
use common::{church, select};

use FanCurve::*;

fn patch(e: &mut Engine, id: FixtureId, profile: &str, address: u16) {
    e.patch(Fixture {
        id,
        number: 0,
        name: String::new(),
        handle: None,
        profile: profile.into(),
        mode: None,
        universe: 1,
        address,
    })
    .unwrap();
}

/// The church rig plus a fifth wash 105 after the spots (address 41).
fn rig() -> Engine {
    let mut e = church();
    patch(&mut e, 105, "wash", 41);
    e
}

const WASHES: [FixtureId; 5] = [101, 102, 103, 104, 105];

fn level(e: &Engine, id: FixtureId, attribute: &str) -> f32 {
    match e.programmer().value(id, attribute) {
        Some(Value::Level(v)) => v,
        other => panic!("{id} {attribute}: expected a level in the programmer, got {other:?}"),
    }
}

fn levels(e: &Engine, ids: &[FixtureId], attribute: &str) -> Vec<f32> {
    ids.iter().map(|&id| level(e, id, attribute)).collect()
}

fn assert_close(actual: &[f32], expected: &[f32]) {
    assert_eq!(actual.len(), expected.len(), "{actual:?} vs {expected:?}");
    for (a, b) in actual.iter().zip(expected) {
        assert!(
            (a - b).abs() < 1e-4,
            "expected {expected:?}, got {actual:?}"
        );
    }
}

fn set_all(e: &mut Engine, ids: &[FixtureId], attribute: &str, v: f32) {
    for &id in ids {
        e.set_programmer_value(id, attribute, v).unwrap();
    }
}

/// Washes 101–105 at Pan 0.5 in the programmer, selected afresh.
fn washes_at_half() -> Engine {
    let mut e = rig();
    set_all(&mut e, &WASHES, "Pan", 0.5);
    select(&mut e, &WASHES);
    e
}

// ---- PRG-56 -----------------------------------------------------------------

#[test]
fn prg_56_fan_is_off_at_start_with_line_as_the_last_curve() {
    let e = rig();
    assert_eq!(e.fan(), None);
    assert_eq!(e.fan_curve(), Line);
}

#[test]
fn prg_56_the_fan_key_toggles_with_the_last_curve() {
    let mut e = rig();
    select(&mut e, &WASHES);
    e.fan_key();
    assert_eq!(e.fan(), Some(Line));
    e.fan_key();
    assert_eq!(e.fan(), None);
    e.set_fan(Some(Wings));
    assert_eq!((e.fan(), e.fan_curve()), (Some(Wings), Wings));
    e.fan_key();
    assert_eq!((e.fan(), e.fan_curve()), (None, Wings));
    e.fan_key();
    assert_eq!(e.fan(), Some(Wings));
    e.set_fan(None);
    assert_eq!((e.fan(), e.fan_curve()), (None, Wings));
}

#[test]
fn prg_56_choosing_a_curve_while_on_switches_the_curve() {
    let mut e = rig();
    select(&mut e, &WASHES);
    e.set_fan(Some(Mirror));
    e.set_fan(Some(Arrow));
    assert_eq!((e.fan(), e.fan_curve()), (Some(Arrow), Arrow));
}

#[test]
fn prg_56_a_new_selection_turns_fan_off_and_keeps_the_last_curve() {
    let mut e = rig();
    select(&mut e, &WASHES);
    let on = |e: &mut Engine| e.set_fan(Some(Mirror));

    on(&mut e);
    e.select_fixture(201).unwrap(); // PRG-01: a tap
    assert_eq!((e.fan(), e.fan_curve()), (None, Mirror));

    on(&mut e);
    e.execute_command(&["101", "Thro", "103", "Enter"]).unwrap(); // PRG-08
    assert_eq!(e.fan(), None);

    on(&mut e);
    e.record_group(7).unwrap();
    e.select_group(7).unwrap(); // a group select
    assert_eq!(e.fan(), None);

    on(&mut e);
    e.clear_selection();
    assert_eq!(e.fan(), None);

    select(&mut e, &WASHES);
    on(&mut e);
    e.clear(); // PRG-29: the first Clear deselects
    assert_eq!(e.fan(), None);
    assert_eq!(e.fan_curve(), Mirror);
}

#[test]
fn prg_56_fix_patterns_and_all_keep_fan_on() {
    let mut e = rig();
    select(&mut e, &WASHES);
    e.set_fan(Some(Line));
    e.next_fixture();
    e.prev_fixture();
    e.select_pattern(2).unwrap();
    e.next_fixture();
    e.select_all(); // ends the pattern
    assert_eq!(e.fan(), Some(Line));
    e.select_all(); // every fixture in the programmer (none yet: an empty selection is fine)
    assert_eq!(e.fan(), Some(Line));
}

#[test]
fn prg_56_fan_commands_are_not_undoable() {
    let mut e = rig();
    select(&mut e, &WASHES);
    let mut history = History::default();
    history
        .edit(&mut e, "Fan", None, |e| {
            e.fan_key();
            Ok(())
        })
        .unwrap();
    history
        .edit(&mut e, "Fan", None, |e| {
            e.set_fan(Some(Arrow));
            Ok(())
        })
        .unwrap();
    assert!(history.is_empty(), "Fan is live state, not an edit");
    assert_eq!(e.fan(), Some(Arrow));
}

// ---- PRG-57 -----------------------------------------------------------------

#[test]
fn prg_57_items_without_the_attribute_take_no_position() {
    let mut e = rig();
    set_all(&mut e, &[101, 102, 103], "Pan", 0.5);
    select(&mut e, &[101, 301, 102, 103]); // spot 301 has no Pan
    assert_eq!(
        e.fan_list("Pan"),
        vec![Item::fixture(101), Item::fixture(102), Item::fixture(103)]
    );
    e.set_fan(Some(Line));
    e.turn_wheel("Pan", 10, Coarse).unwrap();
    assert_close(&levels(&e, &[101, 102, 103], "Pan"), &[0.1, 0.5, 0.9]);
    assert!(e.programmer().stored(301, "Pan").is_none());
}

#[test]
fn prg_57_a_group_fans_in_its_stored_order() {
    let mut e = rig();
    set_all(&mut e, &[101, 102, 103], "Pan", 0.5);
    select(&mut e, &[103, 101, 102]);
    e.record_group(3).unwrap();
    e.clear_selection();
    e.select_group(3).unwrap();
    e.set_fan(Some(Line));
    e.turn_wheel("Pan", 10, Coarse).unwrap();
    assert_close(&levels(&e, &[103, 101, 102], "Pan"), &[0.1, 0.5, 0.9]);
}

#[test]
fn prg_57_the_fan_list_follows_the_sub_selection() {
    let mut e = washes_at_half();
    e.select_pattern(2).unwrap(); // 101, 103, 105
    e.set_fan(Some(Line));
    assert_eq!(
        e.fan_list("Pan"),
        vec![Item::fixture(101), Item::fixture(103), Item::fixture(105)]
    );
    e.turn_wheel("Pan", 10, Coarse).unwrap();
    assert_close(&levels(&e, &WASHES, "Pan"), &[0.1, 0.5, 0.5, 0.5, 0.9]);
}

#[test]
fn prg_57_one_item_turns_as_without_fan() {
    let mut e = rig();
    e.set_programmer_value(101, "Pan", 0.5).unwrap();
    select(&mut e, &[101, 301]); // only 101 has Pan
    e.set_fan(Some(Line));
    e.turn_wheel("Pan", 5, Coarse).unwrap();
    assert_close(&levels(&e, &[101], "Pan"), &[0.7]);
}

/// Three RGBW cells and a master Dimmer, as in `tests/cells.rs`.
const SHEHDS: &str = r#"{ "manufacturer": "T", "model": "Cells", "source": "channel-list", "modes": [ {
  "name": "24ch", "footprint": 24,
  "attributes": [ { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 6 } ],
  "cells": [
    { "colour": { "system": "RGB", "components": [ { "channel": 8 }, { "channel": 9 }, { "channel": 10 } ] },
      "attributes": [ { "name": "White", "group": "C", "kind": "Ltp", "channel": 11, "emitter": true } ] },
    { "colour": { "system": "RGB", "components": [ { "channel": 12 }, { "channel": 13 }, { "channel": 14 } ] },
      "attributes": [ { "name": "White", "group": "C", "kind": "Ltp", "channel": 15, "emitter": true } ] },
    { "colour": { "system": "RGB", "components": [ { "channel": 16 }, { "channel": 17 }, { "channel": 18 } ] },
      "attributes": [ { "name": "White", "group": "C", "kind": "Ltp", "channel": 19, "emitter": true } ] } ] } ] }"#;

fn shehds() -> Engine {
    let mut e = rig();
    e.add_profile("shehds", Profile::from_json(SHEHDS).unwrap())
        .unwrap();
    for (i, id) in (901..=904).enumerate() {
        patch(&mut e, id, "shehds", 101 + 24 * i as u16);
    }
    e
}

#[test]
fn prg_57_unfolded_cells_are_one_fan_of_twelve() {
    let mut e = shehds();
    e.execute_command(&["901", "Thro", "904", ".", "Enter"])
        .unwrap();
    let cells: Vec<Item> = (901..=904)
        .flat_map(|f| (1..=3).map(move |c| Item::cell(f, c)))
        .collect();
    assert_eq!(e.fan_list("Hue"), cells);
    assert_eq!(e.fan_list("White"), cells);
    assert_eq!(e.fan_list("Pan"), Vec::<Item>::new());
}

#[test]
fn prg_57_whole_fixtures_and_cells_share_one_list_in_selection_order() {
    let mut e = shehds();
    e.select_item(Item::cell(901, 2)).unwrap();
    e.select_fixture(902).unwrap();
    e.select_fixture(101).unwrap(); // a wash: no Colour
    e.select_item(Item::cell(903, 1)).unwrap();
    assert_eq!(
        e.fan_list("Hue"),
        vec![Item::cell(901, 2), Item::fixture(902), Item::cell(903, 1)]
    );
}

// ---- PRG-58 -----------------------------------------------------------------

#[test]
fn prg_58_line() {
    assert_close(&fan_factors(Line, 5), &[-1.0, -0.5, 0.0, 0.5, 1.0]);
    assert_close(&fan_factors(Line, 6), &[-1.0, -0.6, -0.2, 0.2, 0.6, 1.0]);
    assert_close(&fan_factors(Line, 2), &[-1.0, 1.0]);
}

#[test]
fn prg_58_mirror() {
    assert_close(&fan_factors(Mirror, 5), &[-1.0, 0.0, 1.0, 0.0, -1.0]);
    assert_close(&fan_factors(Mirror, 6), &[-1.0, 0.0, 1.0, 1.0, 0.0, -1.0]);
    assert_close(&fan_factors(Mirror, 3), &[-1.0, 1.0, -1.0]);
    assert_close(&fan_factors(Mirror, 2), &[0.0, 0.0]); // halves of one item
}

#[test]
fn prg_58_wings() {
    let t = 1.0 / 3.0;
    assert_close(&fan_factors(Wings, 6), &[-1.0, -0.5, 0.0, 0.0, 0.5, 1.0]);
    assert_close(
        &fan_factors(Wings, 9),
        &[-1.0, -2.0 * t, -t, 0.0, 0.0, 0.0, t, 2.0 * t, 1.0],
    );
    assert_close(&fan_factors(Wings, 5), &[-1.0, 0.0, 0.0, 0.0, 1.0]);
    assert_close(&fan_factors(Wings, 2), &[0.0, 0.0]);
}

#[test]
fn prg_58_arrow() {
    assert_close(&fan_factors(Arrow, 5), &[1.0, 0.5, 0.0, 0.5, 1.0]);
    assert_close(&fan_factors(Arrow, 6), &[1.0, 0.6, 0.2, 0.2, 0.6, 1.0]);
}

#[test]
fn prg_58_every_curve_gives_one_factor_per_item_in_range() {
    for curve in [Line, Mirror, Wings, Arrow] {
        for n in 2..=24 {
            let k = fan_factors(curve, n);
            assert_eq!(k.len(), n, "{curve:?} {n}");
            assert!(
                k.iter().all(|k| (-1.0..=1.0).contains(k)),
                "{curve:?} {n}: {k:?}"
            );
        }
    }
}

// ---- PRG-59 -----------------------------------------------------------------

#[test]
fn prg_59_line_spreads_pan_and_back() {
    let mut e = washes_at_half();
    e.set_fan(Some(Line));
    e.turn_wheel("Pan", 10, Coarse).unwrap();
    assert_close(&levels(&e, &WASHES, "Pan"), &[0.1, 0.3, 0.5, 0.7, 0.9]);
    e.turn_wheel("Pan", -10, Coarse).unwrap();
    assert_close(&levels(&e, &WASHES, "Pan"), &[0.5; 5]);
}

#[test]
fn prg_59_arrow_on_dimmer() {
    let mut e = rig();
    let pars = [201, 202, 203, 204, 205];
    set_all(&mut e, &pars, "Dimmer", 0.2);
    select(&mut e, &pars);
    e.set_fan(Some(Arrow));
    e.turn_wheel("Dimmer", 10, Coarse).unwrap();
    assert_close(&levels(&e, &pars, "Dimmer"), &[0.6, 0.4, 0.2, 0.4, 0.6]);
}

#[test]
fn prg_59_each_item_clamps_on_its_own() {
    let mut e = washes_at_half();
    e.set_fan(Some(Line));
    e.turn_wheel("Pan", 20, Coarse).unwrap(); // d = 0.8
    assert_close(&levels(&e, &WASHES, "Pan"), &[0.0, 0.1, 0.5, 0.9, 1.0]);
}

#[test]
fn prg_59_every_item_is_touched_from_its_output_value() {
    let mut e = rig();
    // Pan 0.5 comes from a playback; nothing is in the programmer yet (PRG-20).
    let mut values = Values::new();
    for id in [101, 102, 103] {
        values
            .entry(id)
            .or_default()
            .insert("Pan".into(), 0.5.into());
    }
    e.add_playback(Playback::new(1, 1, 1, values));
    e.set_fader(1, 1.0, 0.0).unwrap();
    e.tick(0.1);
    select(&mut e, &[101, 102, 103]);
    e.set_fan(Some(Line));
    e.turn_wheel("Pan", 5, Coarse).unwrap(); // d = 0.2
                                             // 102 has k = 0 and still enters the programmer, so recording stores the whole fan.
    assert_close(&levels(&e, &[101, 102, 103], "Pan"), &[0.3, 0.5, 0.7]);
}

#[test]
fn prg_59_mixed_starting_values_keep_their_offsets() {
    let mut e = rig();
    for (id, v) in [(101, 0.2), (102, 0.4), (103, 0.6)] {
        e.set_programmer_value(id, "Tilt", v).unwrap();
    }
    select(&mut e, &[101, 102, 103]);
    e.set_fan(Some(Mirror)); // 3 → −1, 1, −1
    e.turn_wheel("Tilt", 5, Coarse).unwrap();
    assert_close(&levels(&e, &[101, 102, 103], "Tilt"), &[0.0, 0.6, 0.4]);
}

// ---- PRG-60 -----------------------------------------------------------------

const RGBW: &str = r#"{ "manufacturer": "T", "model": "RGBW Par", "source": "channel-list", "modes": [ {
  "name": "5ch", "footprint": 5,
  "colour": { "system": "RGB", "components": [ { "channel": 2 }, { "channel": 3 }, { "channel": 4 } ] },
  "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "White", "group": "C", "kind": "Ltp", "channel": 5, "emitter": true } ] } ] }"#;

/// RGBW pars 201–208 at addresses 1, 6, …, 36, with a wheel step of 1/40 (10 Coarse = 0.25).
fn colour_pars() -> Engine {
    let mut e = Engine::new(Settings {
        wheel_step_coarse: 1.0 / 40.0,
        ..Settings::default()
    });
    e.add_profile("rgbw", Profile::from_json(RGBW).unwrap())
        .unwrap();
    for (i, id) in (201..=208).enumerate() {
        patch(&mut e, id, "rgbw", 1 + 5 * i as u16);
    }
    e
}

const PARS: [FixtureId; 8] = [201, 202, 203, 204, 205, 206, 207, 208];

fn hue(e: &Engine, id: FixtureId) -> f32 {
    match e.programmer().value(id, "Colour") {
        Some(Value::Colour(c)) => c.to_hsv().0,
        other => panic!("{id}: expected a colour in the programmer, got {other:?}"),
    }
}

#[test]
fn prg_60_a_hue_fan_is_a_rainbow_through_red() {
    let mut e = colour_pars();
    let red = TargetColour {
        r: 1.0,
        g: 0.0,
        b: 0.0,
    };
    for id in PARS {
        e.set_programmer_value(id, "Colour", red).unwrap();
    }
    select(&mut e, &PARS);
    e.set_active_group(AttributeGroup::C);
    assert_eq!(e.fan_list("Hue").len(), 8);
    e.set_fan(Some(Line));
    e.turn_wheel("Hue", 10, Coarse).unwrap(); // d = 0.25 = 90°
                                              // k = (2i − 7) / 7; hue = 0.25 k, wrapped: 270° … 90° through red.
    let expected: Vec<f32> = (0..8)
        .map(|i| (0.25 * (2.0 * i as f32 - 7.0) / 7.0).rem_euclid(1.0))
        .collect();
    let actual: Vec<f32> = PARS.iter().map(|&id| hue(&e, id)).collect();
    assert_close(&actual, &expected);
    assert!((actual[0] - 0.75).abs() < 1e-4 && (actual[7] - 0.25).abs() < 1e-4);
}

#[test]
fn prg_60_direct_emitters_fan() {
    let mut e = colour_pars();
    let five = &PARS[..5];
    set_all(&mut e, five, "White", 0.5);
    select(&mut e, five);
    e.set_fan(Some(Line));
    e.turn_wheel("White", 10, Coarse).unwrap(); // d = 0.25
    assert_close(&levels(&e, five, "White"), &[0.25, 0.375, 0.5, 0.625, 0.75]);
}

#[test]
fn prg_60_keypad_levels_and_absolute_sets_do_not_fan() {
    let mut e = colour_pars();
    select(&mut e, &PARS);
    e.set_fan(Some(Line));
    e.execute_command(&["@", "50", "Enter"]).unwrap(); // PRG-09 on the targets
    assert_close(&levels(&e, &PARS, "Dimmer"), &[0.5; 8]);
    e.set_programmer_level("White", 0.3).unwrap();
    assert_close(&levels(&e, &PARS, "White"), &[0.3; 8]);
    e.set_colour(0.5, 1.0).unwrap();
    let hues: Vec<f32> = PARS.iter().map(|&id| hue(&e, id)).collect();
    assert_close(&hues, &[0.5; 8]);
}

#[test]
fn prg_60_without_fan_a_wheel_moves_all_alike() {
    let mut e = washes_at_half();
    e.set_fan(Some(Line));
    e.fan_key(); // off
    e.turn_wheel("Pan", 5, Coarse).unwrap();
    assert_close(&levels(&e, &WASHES, "Pan"), &[0.7; 5]);
}

// ---- PRG-61 -----------------------------------------------------------------

#[test]
fn prg_61_fanned_values_are_plain_manual_values() {
    let mut e = washes_at_half();
    e.set_fan(Some(Line));
    e.turn_wheel("Pan", 10, Coarse).unwrap();
    let contents = e.contents(&[AttributeGroup::P]);
    let pans: Vec<Value> = WASHES
        .iter()
        .map(|id| contents.get(id).unwrap().get("Pan").unwrap().value())
        .collect();
    let expected = [0.1, 0.3, 0.5, 0.7, 0.9];
    for (v, x) in pans.iter().zip(expected) {
        match v {
            Value::Level(l) => assert!((l - x).abs() < 1e-4, "{pans:?}"),
            other => panic!("expected a level, got {other:?}"),
        }
    }
}

#[test]
fn prg_61_fanned_turns_coalesce_into_one_undo_step_and_fan_stays_on() {
    let mut e = washes_at_half();
    e.set_fan(Some(Line));
    let mut history = History::default();
    for _ in 0..3 {
        history
            .edit(&mut e, "Wheel Pan", Some("wheel:Pan"), |e| {
                e.turn_wheel("Pan", 2, Coarse)
            })
            .unwrap();
    }
    assert_eq!(history.len(), 1);
    history.undo(&mut e).unwrap();
    assert_close(&levels(&e, &WASHES, "Pan"), &[0.5; 5]);
    assert_eq!(e.fan(), Some(Line));
}

// ---- PRG-62 -----------------------------------------------------------------

#[test]
fn prg_62_engine_state_carries_fan_and_fan_curve() {
    let mut e = rig();
    select(&mut e, &WASHES);
    let view = e.programmer_view();
    assert_eq!((view.fan, view.fan_curve), (None, Line));
    let json = serde_json::to_value(&view).unwrap();
    assert_eq!(json["fan"], serde_json::Value::Null);
    assert_eq!(json["fan_curve"], "Line");

    e.set_fan(Some(Wings));
    let json = serde_json::to_value(e.programmer_view()).unwrap();
    assert_eq!(json["fan"], "Wings");
    assert_eq!(json["fan_curve"], "Wings");
}
