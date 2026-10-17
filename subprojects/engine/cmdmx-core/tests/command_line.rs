//! Command line (Titan keypad syntax): `specs/modules/programmer.md` PRG-08, 09, 11 (VS2-03).
//! PRG-10 (`Palette n`, ending on `Locate`) lands with palettes and Locate (VS2-05).

#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

mod common;

use cmdmx_core::engine::Engine;
use cmdmx_core::patch::FixtureId;
use cmdmx_core::CoreError;
use common::{church, select};

fn run(e: &mut Engine, line: &str) -> Result<(), CoreError> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    e.execute_command(&tokens)
}

fn selection(e: &Engine) -> Vec<FixtureId> {
    e.programmer().selection().to_vec()
}

// ---- PRG-08 -----------------------------------------------------------------

#[test]
fn prg_08_thro_not_and() {
    let mut e = church();
    run(&mut e, "201 Thro 205 Not 203 And 301 Enter").unwrap();
    assert_eq!(selection(&e), vec![201, 202, 204, 205, 301]);
}

#[test]
fn prg_08_thro_keeps_the_typed_direction() {
    let mut e = church();
    run(&mut e, "205 Thro 201 Enter").unwrap();
    assert_eq!(selection(&e), vec![205, 204, 203, 202, 201]);
}

#[test]
fn prg_08_fixture_keyword_is_optional() {
    let mut e = church();
    run(&mut e, "Fixture 202 And 201 Enter").unwrap();
    assert_eq!(selection(&e), vec![202, 201]);
}

#[test]
fn prg_08_not_removes_a_range() {
    let mut e = church();
    run(&mut e, "201 Thro 206 Not 202 Thro 204 Enter").unwrap();
    assert_eq!(selection(&e), vec![201, 205, 206]);
}

#[test]
fn prg_08_group_in_an_expression() {
    let mut e = church();
    select(&mut e, &[203, 201]);
    e.record_group(3).unwrap();
    e.clear_selection();
    run(&mut e, "Group 3 And Fixture 301 Enter").unwrap();
    assert_eq!(selection(&e), vec![203, 201, 301]);
}

#[test]
fn prg_08_duplicates_keep_their_first_place() {
    let mut e = church();
    run(&mut e, "202 And 201 Thro 203 Enter").unwrap();
    assert_eq!(selection(&e), vec![202, 201, 203]);
}

#[test]
fn prg_08_typed_selection_replaces_the_selection() {
    let mut e = church();
    select(&mut e, &[101, 102]);
    run(&mut e, "201 Enter").unwrap();
    assert_eq!(selection(&e), vec![201]);
}

#[test]
fn prg_08_unpatched_numbers_are_skipped() {
    let mut e = church();
    // church rig: 201..=208 then 221..=226
    run(&mut e, "206 Thro 222 Enter").unwrap();
    assert_eq!(selection(&e), vec![206, 207, 208, 221, 222]);
}

#[test]
fn prg_08_nothing_left_is_refused_and_keeps_the_selection() {
    let mut e = church();
    select(&mut e, &[101]);
    assert!(run(&mut e, "999 Enter").is_err());
    assert!(run(&mut e, "Group 42 Enter").is_err());
    assert!(run(&mut e, "201 Not 201 Enter").is_err());
    assert_eq!(selection(&e), vec![101]);
}

#[test]
fn prg_08_huge_range_is_cheap() {
    let mut e = church();
    run(&mut e, "1 Thro 4000000000 Enter").unwrap();
    assert_eq!(selection(&e).len(), 24);
}

// ---- PRG-09 -----------------------------------------------------------------

#[test]
fn prg_09_at_sets_intensity_of_the_expression() {
    let mut e = church();
    run(&mut e, "221 Thro 226 @ 40 Enter").unwrap();
    assert_eq!(selection(&e), vec![221, 222, 223, 224, 225, 226]);
    for id in 221..=226 {
        assert_eq!(e.programmer().value(id, "Dimmer"), Some(0.4));
    }
    assert!(!e.programmer().in_programmer(201));
}

#[test]
fn prg_09_at_alone_uses_the_targets() {
    let mut e = church();
    select(&mut e, &[101, 102, 103]);
    e.next_fixture(); // sub-selection: 101
    run(&mut e, "@ 50 Enter").unwrap();
    assert_eq!(e.programmer().value(101, "Dimmer"), Some(0.5));
    assert!(!e.programmer().in_programmer(102));
    assert_eq!(selection(&e), vec![101, 102, 103]);
}

#[test]
fn prg_09_level_is_clamped() {
    let mut e = church();
    run(&mut e, "301 @ 150 Enter").unwrap();
    assert_eq!(e.programmer().value(301, "Dimmer"), Some(1.0));
    run(&mut e, "302 @ 0 Enter").unwrap();
    assert_eq!(e.programmer().value(302, "Dimmer"), Some(0.0));
}

#[test]
fn prg_09_at_with_empty_selection_is_refused() {
    let mut e = church();
    assert_eq!(run(&mut e, "@ 50 Enter"), Err(CoreError::EmptySelection));
    assert!(e.programmer().is_empty());
}

#[test]
fn prg_09_at_counts_as_a_change_for_the_next_select() {
    let mut e = church();
    run(&mut e, "201 @ 50 Enter").unwrap();
    e.select_fixture(202).unwrap();
    assert_eq!(selection(&e), vec![202]);
}

// ---- PRG-11 -----------------------------------------------------------------

#[test]
fn prg_11_malformed_lines_are_refused_naming_the_line() {
    let mut e = church();
    select(&mut e, &[101]);
    for line in [
        "201 Banana Enter",
        "201 Thro 205",
        "201 Thro Enter",
        "Thro 205 Enter",
        "201 @ Enter",
        "201 @ 50 60 Enter",
        "Enter",
        "",
        "201 And Enter",
    ] {
        assert_eq!(
            run(&mut e, line),
            Err(CoreError::InvalidCommand(line.to_string())),
            "line {line:?}"
        );
    }
    assert_eq!(selection(&e), vec![101]);
    assert!(e.programmer().is_empty());
}
