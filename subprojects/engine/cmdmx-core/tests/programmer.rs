//! Programmer selection behaviour: `specs/modules/programmer.md` PRG-01..07, 14 (VS2-02).
//!
//! Church fixture numbers: washes 101–104, RGBW pars 201–208, RGB pars 221–226,
//! front spots 301–306. Every fixture uses a one-channel dimmer profile here;
//! selection does not care about attributes.

#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

mod common;

use cmdmx_core::engine::Engine;
use cmdmx_core::patch::FixtureId;
use cmdmx_core::value::Value;
use cmdmx_core::CoreError;
use common::{church, select};

fn selection(e: &Engine) -> Vec<FixtureId> {
    e.programmer().selection().to_vec()
}

fn targets(e: &Engine) -> Vec<FixtureId> {
    e.programmer().targets()
}

// ---- PRG-01 -----------------------------------------------------------------

#[test]
fn prg_01_selection_keeps_order_and_toggles() {
    let mut e = church();
    select(&mut e, &[203, 201, 202]);
    assert_eq!(selection(&e), vec![203, 201, 202]);
    e.select_fixture(201).unwrap();
    assert_eq!(selection(&e), vec![203, 202]);
}

#[test]
fn prg_01_selecting_an_unpatched_fixture_is_refused() {
    let mut e = church();
    select(&mut e, &[201]);
    assert_eq!(e.select_fixture(999), Err(CoreError::UnknownFixture(999)));
    assert_eq!(selection(&e), vec![201]);
}

// ---- PRG-02 -----------------------------------------------------------------

#[test]
fn prg_02_group_appends_in_group_order_without_duplicates() {
    let mut e = church();
    select(&mut e, &[201, 202, 203, 204, 205, 206, 207, 208]);
    e.record_group(3).unwrap(); // "Pars RGBW"
    e.clear_selection();
    select(&mut e, &[205]);
    e.select_group(3).unwrap();
    assert_eq!(selection(&e), vec![205, 201, 202, 203, 204, 206, 207, 208]);
}

#[test]
fn prg_02_unknown_group_is_refused() {
    let mut e = church();
    assert_eq!(e.select_group(42), Err(CoreError::UnknownGroup(42)));
    assert!(selection(&e).is_empty());
}

// ---- PRG-03 -----------------------------------------------------------------

#[test]
fn prg_03_selecting_after_a_change_starts_a_new_selection() {
    let mut e = church();
    select(&mut e, &[201]);
    e.set_programmer_value(201, "Dimmer", 0.5).unwrap();
    e.select_fixture(202).unwrap();
    assert_eq!(selection(&e), vec![202]);
    assert_eq!(e.programmer().value(201, "Dimmer"), Some(Value::Level(0.5)));
}

#[test]
fn prg_03_group_select_after_a_change_starts_a_new_selection() {
    let mut e = church();
    select(&mut e, &[301, 302]);
    e.record_group(7).unwrap();
    e.clear_selection();
    select(&mut e, &[201]);
    e.set_programmer_value(201, "Dimmer", 0.5).unwrap();
    e.select_group(7).unwrap();
    assert_eq!(selection(&e), vec![301, 302]);
}

#[test]
fn prg_03_only_the_first_select_after_a_change_resets() {
    let mut e = church();
    select(&mut e, &[201]);
    e.set_programmer_value(201, "Dimmer", 0.5).unwrap();
    select(&mut e, &[202, 203]);
    assert_eq!(selection(&e), vec![202, 203]);
}

// ---- PRG-04 -----------------------------------------------------------------

#[test]
fn prg_04_fix_next_and_previous_step_one_fixture_and_wrap() {
    let mut e = church();
    select(&mut e, &[101, 102, 103]);
    assert_eq!(targets(&e), vec![101, 102, 103]);
    e.next_fixture();
    assert_eq!(targets(&e), vec![101]);
    e.next_fixture();
    assert_eq!(targets(&e), vec![102]);
    e.prev_fixture();
    assert_eq!(targets(&e), vec![101]);
    e.prev_fixture();
    assert_eq!(targets(&e), vec![103]);
    e.next_fixture();
    assert_eq!(targets(&e), vec![101]);
    assert_eq!(selection(&e), vec![101, 102, 103]);
}

#[test]
fn prg_04_previous_from_the_whole_selection_starts_at_the_end() {
    let mut e = church();
    select(&mut e, &[101, 102, 103]);
    e.prev_fixture();
    assert_eq!(targets(&e), vec![103]);
}

#[test]
fn prg_04_stepping_an_empty_selection_does_nothing() {
    let mut e = church();
    e.next_fixture();
    e.prev_fixture();
    assert!(targets(&e).is_empty());
}

// ---- PRG-05 -----------------------------------------------------------------

#[test]
fn prg_05_odd_even_pattern_steps_through_stages() {
    let mut e = church();
    select(&mut e, &[201, 202, 203, 204, 205, 206]);
    e.select_pattern(2).unwrap();
    assert_eq!(targets(&e), vec![201, 203, 205]);
    e.next_fixture();
    assert_eq!(targets(&e), vec![202, 204, 206]);
    e.next_fixture();
    assert_eq!(targets(&e), vec![201, 203, 205]);
    e.prev_fixture();
    assert_eq!(targets(&e), vec![202, 204, 206]);
    assert_eq!(selection(&e), vec![201, 202, 203, 204, 205, 206]);
}

#[test]
fn prg_05_pattern_follows_selection_order_not_numbers() {
    let mut e = church();
    select(&mut e, &[206, 205, 204, 203]);
    e.select_pattern(2).unwrap();
    assert_eq!(targets(&e), vec![206, 204]);
}

#[test]
fn prg_05_three_stage_pattern() {
    let mut e = church();
    select(&mut e, &[201, 202, 203, 204, 205, 206]);
    e.select_pattern(3).unwrap();
    assert_eq!(targets(&e), vec![201, 204]);
    e.prev_fixture();
    assert_eq!(targets(&e), vec![203, 206]);
}

#[test]
fn prg_05_pattern_of_fewer_than_two_stages_is_refused() {
    let mut e = church();
    select(&mut e, &[201, 202]);
    assert_eq!(e.select_pattern(1), Err(CoreError::InvalidPattern(1)));
    assert_eq!(e.select_pattern(0), Err(CoreError::InvalidPattern(0)));
    assert_eq!(targets(&e), vec![201, 202]);
}

// ---- PRG-06 -----------------------------------------------------------------

#[test]
fn prg_06_all_ends_stepping_and_patterns() {
    let mut e = church();
    select(&mut e, &[101, 102, 103]);
    e.next_fixture();
    e.select_all();
    assert_eq!(targets(&e), vec![101, 102, 103]);

    e.select_pattern(2).unwrap();
    e.select_all();
    assert_eq!(targets(&e), vec![101, 102, 103]);
    assert_eq!(selection(&e), vec![101, 102, 103]);
}

#[test]
fn prg_06_all_without_sub_selection_selects_the_programmer_fixtures() {
    let mut e = church();
    select(&mut e, &[302, 201]);
    e.set_programmer_value(302, "Dimmer", 1.0).unwrap();
    e.set_programmer_value(201, "Dimmer", 0.4).unwrap();
    e.clear_selection();
    select(&mut e, &[101]);
    e.select_all();
    assert_eq!(selection(&e), vec![201, 302]);
    assert_eq!(targets(&e), vec![201, 302]);
}

// ---- PRG-07 -----------------------------------------------------------------

#[test]
fn prg_07_record_group_stores_selection_order() {
    let mut e = church();
    select(&mut e, &[203, 201, 202]);
    e.record_group(1).unwrap();
    assert_eq!(e.group(1), Some(&[203, 201, 202][..]));
    e.clear_selection();
    e.select_group(1).unwrap();
    assert_eq!(selection(&e), vec![203, 201, 202]);
}

#[test]
fn prg_07_record_group_with_empty_selection_is_refused_and_keeps_the_group() {
    let mut e = church();
    select(&mut e, &[301]);
    e.record_group(1).unwrap();
    e.clear_selection();
    assert_eq!(e.record_group(1), Err(CoreError::EmptySelection));
    assert_eq!(e.group(1), Some(&[301][..]));
    assert_eq!(e.record_group(2), Err(CoreError::EmptySelection));
    assert_eq!(e.group(2), None);
}

#[test]
fn prg_07_record_onto_a_used_group_replaces_it() {
    let mut e = church();
    select(&mut e, &[301, 302]);
    e.record_group(1).unwrap();
    e.clear_selection();
    select(&mut e, &[101]);
    e.record_group(1).unwrap();
    assert_eq!(e.group(1), Some(&[101][..]));
}

// ---- PRG-14 -----------------------------------------------------------------

#[test]
fn prg_14_selecting_puts_nothing_in_the_programmer() {
    let mut e = church();
    select(&mut e, &[201, 202]);
    e.record_group(1).unwrap();
    e.select_group(1).unwrap();
    e.next_fixture();
    e.select_pattern(2).unwrap();
    e.select_all();
    assert!(e.programmer().is_empty());
    assert!(!e.programmer().in_programmer(201));
}

// ---- minimal programmer values (full wheel/merge behaviour is VS2-04) ---------

#[test]
fn programmer_value_is_refused_for_unknown_fixture_attribute_or_level() {
    let mut e = church();
    assert_eq!(
        e.set_programmer_value(999, "Dimmer", 0.5),
        Err(CoreError::UnknownFixture(999))
    );
    assert_eq!(
        e.set_programmer_value(201, "Pan", 0.5),
        Err(CoreError::UnknownAttribute {
            fixture: 201,
            attribute: "Pan".into()
        })
    );
    assert_eq!(
        e.set_programmer_value(201, "Dimmer", 1.5),
        Err(CoreError::InvalidLevel(1.5))
    );
    assert!(e.programmer().is_empty());
}
