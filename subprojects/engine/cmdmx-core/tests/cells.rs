//! Cell items in the programmer: `specs/modules/programmer.md` PRG-46, 48..50 (VS2-30).
//!
//! Shehds-like washes 901–904 with three RGBW cells each and a master Dimmer; 201 is a plain
//! dimmer without cells. Cells only select here: the merge (PRG-47, 51), storage (PRG-52) and
//! the protocol (PRG-55) are later tasks.

#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::patch::{Fixture, FixtureId};
use cmdmx_core::profile::Profile;
use cmdmx_core::programmer::Item;
use cmdmx_core::value::Value;
use cmdmx_core::CoreError;

/// Three RGBW cells (8–11, 12–15, 16–19) and a master Dimmer on 6.
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

const DIMMER: &str =
    r#"{ "attributes": [ { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 } ] }"#;

/// Universe 1: 901 @1, 902 @25, 903 @49, 904 @73 (Shehds), 201 @97 (dimmer).
fn rig() -> Engine {
    let mut e = Engine::new(Settings::default());
    e.add_profile("shehds", Profile::from_json(SHEHDS).unwrap())
        .unwrap();
    e.add_profile("dimmer", Profile::from_json(DIMMER).unwrap())
        .unwrap();
    for (id, profile, address) in [
        (901, "shehds", 1),
        (902, "shehds", 25),
        (903, "shehds", 49),
        (904, "shehds", 73),
        (201, "dimmer", 97),
    ] {
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
    e
}

fn f(id: FixtureId) -> Item {
    Item::fixture(id)
}

fn c(id: FixtureId, cell: u16) -> Item {
    Item::cell(id, cell)
}

fn tap(e: &mut Engine, items: &[Item]) {
    for &item in items {
        e.select_item(item).unwrap();
    }
}

fn items(e: &Engine) -> Vec<Item> {
    e.programmer().items().to_vec()
}

fn targets(e: &Engine) -> Vec<Item> {
    e.programmer().target_items()
}

fn run(e: &mut Engine, line: &str) -> Result<(), CoreError> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    e.execute_command(&tokens)
}

// ---- PRG-46 -----------------------------------------------------------------

#[test]
fn prg_46_tapping_a_cell_twice_removes_it() {
    let mut e = rig();
    tap(&mut e, &[c(901, 2), c(901, 3), c(901, 2)]);
    assert_eq!(items(&e), vec![c(901, 3)]);
}

#[test]
fn prg_46_a_fixture_and_its_cell_are_different_items() {
    let mut e = rig();
    tap(&mut e, &[f(901), c(901, 2)]);
    assert_eq!(items(&e), vec![f(901), c(901, 2)]);
    // Whole-fixture views stay as they were until the protocol carries items (VS2-33).
    assert_eq!(e.programmer().selection().to_vec(), vec![901]);
    e.select_fixture(901).unwrap();
    assert_eq!(items(&e), vec![c(901, 2)]);
}

#[test]
fn prg_46_a_cell_that_does_not_exist_is_refused() {
    let mut e = rig();
    tap(&mut e, &[c(902, 1)]);
    for (fixture, cell) in [(901, 4), (901, 0), (201, 1)] {
        assert_eq!(
            e.select_item(c(fixture, cell)),
            Err(CoreError::UnknownCell { fixture, cell })
        );
    }
    assert_eq!(
        e.select_item(c(999, 1)),
        Err(CoreError::UnknownFixture(999))
    );
    assert_eq!(items(&e), vec![c(902, 1)]);
}

#[test]
fn prg_46_after_a_change_a_cell_tap_starts_a_new_selection() {
    let mut e = rig();
    tap(&mut e, &[f(201)]);
    e.set_programmer_value(201, "Dimmer", 0.5).unwrap();
    tap(&mut e, &[c(901, 1)]);
    assert_eq!(items(&e), vec![c(901, 1)]);
    assert_eq!(e.programmer().value(201, "Dimmer"), Some(Value::Level(0.5)));
}

#[test]
fn prg_46_typed_cells_that_do_not_exist_are_skipped() {
    let mut e = rig();
    run(&mut e, "901 . 2 Thro 5 Enter").unwrap();
    assert_eq!(items(&e), vec![c(901, 2), c(901, 3)]);
    // 201 has no cells: skipped; nothing left → refused, selection unchanged (PRG-08).
    assert_eq!(run(&mut e, "201 . 1 Enter"), Err(CoreError::EmptySelection));
    assert_eq!(run(&mut e, "901 . 4 Enter"), Err(CoreError::EmptySelection));
    assert_eq!(items(&e), vec![c(901, 2), c(901, 3)]);
}

#[test]
fn prg_46_removing_a_fixture_drops_its_cell_items() {
    let mut e = rig();
    tap(&mut e, &[c(901, 1), c(902, 1), f(902)]);
    e.record_group(5).unwrap();
    e.patch_remove(&[902]).unwrap();
    assert_eq!(items(&e), vec![c(901, 1)]);
    assert_eq!(e.group_items(5), Some(vec![c(901, 1)]));
}

#[test]
fn prg_54_rem_dim_keeps_a_fixture_with_a_selected_cell() {
    let mut e = rig();
    tap(&mut e, &[c(901, 2)]);
    e.rem_dim().unwrap();
    assert_eq!(e.programmer().value(901, "Dimmer"), None);
    assert_eq!(e.programmer().value(902, "Dimmer"), Some(Value::Level(0.0)));
}

// ---- PRG-48 -----------------------------------------------------------------

#[test]
fn prg_48_one_cell() {
    let mut e = rig();
    run(&mut e, "901 . 2 Enter").unwrap();
    assert_eq!(items(&e), vec![c(901, 2)]);
    run(&mut e, "Fixture 902 . 1 Enter").unwrap();
    assert_eq!(items(&e), vec![c(902, 1)]);
}

#[test]
fn prg_48_all_cells_of_a_fixture() {
    let mut e = rig();
    run(&mut e, "901 . Enter").unwrap();
    assert_eq!(items(&e), vec![c(901, 1), c(901, 2), c(901, 3)]);
}

#[test]
fn prg_48_a_cell_range_of_one_fixture() {
    let mut e = rig();
    run(&mut e, "901 . 2 Thro 3 Enter").unwrap();
    assert_eq!(items(&e), vec![c(901, 2), c(901, 3)]);
    run(&mut e, "901 . 1 Thro . 2 Enter").unwrap();
    assert_eq!(items(&e), vec![c(901, 1), c(901, 2)]);
}

#[test]
fn prg_48_one_cell_of_each_fixture_in_a_range() {
    let mut e = rig();
    run(&mut e, "901 Thro 904 . 2 Enter").unwrap();
    assert_eq!(items(&e), vec![c(901, 2), c(902, 2), c(903, 2), c(904, 2)]);
}

#[test]
fn prg_48_all_cells_of_a_fixture_range() {
    let mut e = rig();
    run(&mut e, "901 Thro 902 . Enter").unwrap();
    assert_eq!(
        items(&e),
        vec![
            c(901, 1),
            c(901, 2),
            c(901, 3),
            c(902, 1),
            c(902, 2),
            c(902, 3)
        ]
    );
}

#[test]
fn prg_48_a_cell_range_of_each_fixture_in_a_range() {
    let mut e = rig();
    run(&mut e, "901 . 2 Thro 902 . 3 Enter").unwrap();
    assert_eq!(items(&e), vec![c(901, 2), c(901, 3), c(902, 2), c(902, 3)]);
}

#[test]
fn prg_48_a_fixture_range_skips_fixtures_without_cells() {
    let mut e = rig();
    run(&mut e, "201 Thro 901 . 1 Enter").unwrap();
    assert_eq!(items(&e), vec![c(901, 1)]);
}

#[test]
fn prg_48_dot_first_takes_the_selected_fixtures_in_selection_order() {
    let mut e = rig();
    tap(&mut e, &[f(902), c(901, 3)]);
    run(&mut e, ". 1 Enter").unwrap();
    assert_eq!(items(&e), vec![c(902, 1), c(901, 1)]);

    tap(&mut e, &[f(903)]); // selection [902.1, 901.1, 903]
    run(&mut e, ". Enter").unwrap();
    assert_eq!(
        items(&e),
        vec![
            c(902, 1),
            c(902, 2),
            c(902, 3),
            c(901, 1),
            c(901, 2),
            c(901, 3),
            c(903, 1),
            c(903, 2),
            c(903, 3)
        ]
    );

    run(&mut e, "902 And 901 Enter").unwrap();
    run(&mut e, ". 1 Thro . 2 Enter").unwrap();
    assert_eq!(items(&e), vec![c(902, 1), c(902, 2), c(901, 1), c(901, 2)]);
}

#[test]
fn prg_48_dot_first_with_nothing_selected_is_refused() {
    let mut e = rig();
    assert_eq!(run(&mut e, ". 1 Enter"), Err(CoreError::EmptySelection));
    assert_eq!(run(&mut e, ". Enter"), Err(CoreError::EmptySelection));
    // A selected fixture without cells gives no cells either.
    tap(&mut e, &[f(201)]);
    assert_eq!(run(&mut e, ". Enter"), Err(CoreError::EmptySelection));
    assert_eq!(items(&e), vec![f(201)]);
}

#[test]
fn prg_48_not_removes_the_items_it_names() {
    let mut e = rig();
    run(&mut e, "901 . Not 901 . 2 Enter").unwrap();
    assert_eq!(items(&e), vec![c(901, 1), c(901, 3)]);
    // The whole fixture is another item: `Not 901` leaves its cells.
    run(&mut e, "901 . 1 And 901 Not 901 Enter").unwrap();
    assert_eq!(items(&e), vec![c(901, 1)]);
}

#[test]
fn prg_48_and_mixes_cells_and_fixtures() {
    let mut e = rig();
    run(&mut e, "901 . 1 And 902 And 201 Enter").unwrap();
    assert_eq!(items(&e), vec![c(901, 1), f(902), f(201)]);
}

#[test]
fn prg_48_at_level_combines_with_cells() {
    let mut e = rig();
    run(&mut e, "901 . 2 And 201 @ 50 Enter").unwrap();
    assert_eq!(items(&e), vec![c(901, 2), f(201)]);
    assert_eq!(e.programmer().value(201, "Dimmer"), Some(Value::Level(0.5)));
}

#[test]
fn prg_48_malformed_dot_lines_are_refused_and_change_nothing() {
    let mut e = rig();
    tap(&mut e, &[c(901, 1)]);
    for line in [
        "901 . . Enter",
        "901 . 2 . Enter",
        "901 . 2 Thro Enter",
        "901 . 2 Thro . Enter",
        "901 . Thro 3 Enter",
        ". 1 Thro 2 Enter",
        "Group 1 . 2 Enter",
        ". . Enter",
    ] {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(
            e.execute_command(&tokens),
            Err(CoreError::InvalidCommand(line.into())),
            "{line}"
        );
    }
    assert_eq!(items(&e), vec![c(901, 1)]);
}

// ---- PRG-49 -----------------------------------------------------------------

#[test]
fn prg_49_a_group_stores_and_selects_items() {
    let mut e = rig();
    tap(&mut e, &[c(901, 1), c(902, 1)]);
    e.record_group(5).unwrap();
    assert_eq!(e.group_items(5), Some(vec![c(901, 1), c(902, 1)]));
    e.clear_selection();
    e.select_group(5).unwrap();
    assert_eq!(items(&e), vec![c(901, 1), c(902, 1)]);
    e.clear_selection();
    run(&mut e, "Group 5 Enter").unwrap();
    assert_eq!(items(&e), vec![c(901, 1), c(902, 1)]);
}

#[test]
fn prg_49_the_group_toggle_compares_items() {
    let mut e = rig();
    tap(&mut e, &[c(901, 1), c(902, 1)]);
    e.record_group(5).unwrap();
    e.select_group(5).unwrap();
    assert!(items(&e).is_empty());
    // The whole fixture 901 is not the item 901.1: the group is appended.
    tap(&mut e, &[f(901)]);
    e.select_group(5).unwrap();
    assert_eq!(items(&e), vec![f(901), c(901, 1), c(902, 1)]);
    e.select_group(5).unwrap();
    assert_eq!(items(&e), vec![f(901)]);
}

#[test]
fn prg_49_a_group_of_whole_fixtures_is_unchanged() {
    let mut e = rig();
    tap(&mut e, &[f(902), f(901)]);
    e.record_group(1).unwrap();
    assert_eq!(e.group(1), Some(vec![902, 901]));
    assert_eq!(e.group_items(1), Some(vec![f(902), f(901)]));
    assert_eq!(e.group_items(2), None);
}

// ---- PRG-50 -----------------------------------------------------------------

#[test]
fn prg_50_fix_steps_through_items() {
    let mut e = rig();
    run(&mut e, "901 . Enter").unwrap();
    e.next_fixture();
    assert_eq!(targets(&e), vec![c(901, 1)]);
    e.next_fixture();
    assert_eq!(targets(&e), vec![c(901, 2)]);
    e.prev_fixture();
    e.prev_fixture();
    assert_eq!(targets(&e), vec![c(901, 3)]);
}

#[test]
fn prg_50_odd_even_over_items() {
    let mut e = rig();
    tap(&mut e, &[c(901, 1), c(901, 2), c(901, 3)]);
    e.select_pattern(2).unwrap();
    assert_eq!(targets(&e), vec![c(901, 1), c(901, 3)]);
    e.next_fixture();
    assert_eq!(targets(&e), vec![c(901, 2)]);
    e.select_all();
    assert_eq!(targets(&e), vec![c(901, 1), c(901, 2), c(901, 3)]);
}

#[test]
fn prg_50_a_pattern_counts_fixtures_and_cells_alike() {
    let mut e = rig();
    tap(&mut e, &[f(201), c(901, 1), f(902), c(901, 2)]);
    e.select_pattern(2).unwrap();
    assert_eq!(targets(&e), vec![f(201), f(902)]);
    // Whole-fixture targets drive the wheels until cells join the merge (VS2-31).
    assert_eq!(e.programmer().targets(), vec![201, 902]);
}

#[test]
fn prg_50_all_without_sub_selection_selects_the_programmer_items() {
    let mut e = rig();
    tap(&mut e, &[f(902), f(901)]);
    e.set_programmer_value(902, "Dimmer", 1.0).unwrap();
    e.set_programmer_value(901, "Dimmer", 0.4).unwrap();
    e.clear_selection();
    tap(&mut e, &[c(903, 1)]);
    e.select_all();
    assert_eq!(items(&e), vec![f(901), f(902)]);
}
