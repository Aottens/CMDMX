//! Command line (Titan keypad syntax): `specs/modules/programmer.md` PRG-08, 09, 11 (VS2-03).
//! PRG-10 (`Palette n`, ending on `Locate`) lands with palettes and Locate (VS2-05).

#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

mod common;

use cmdmx_core::engine::Engine;
use cmdmx_core::patch::FixtureId;
use cmdmx_core::value::Value;
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
        assert_eq!(e.programmer().value(id, "Dimmer"), Some(Value::Level(0.4)));
    }
    assert!(!e.programmer().in_programmer(201));
}

#[test]
fn prg_09_at_alone_uses_the_targets() {
    let mut e = church();
    select(&mut e, &[101, 102, 103]);
    e.next_fixture(); // sub-selection: 101
    run(&mut e, "@ 50 Enter").unwrap();
    assert_eq!(e.programmer().value(101, "Dimmer"), Some(Value::Level(0.5)));
    assert!(!e.programmer().in_programmer(102));
    assert_eq!(selection(&e), vec![101, 102, 103]);
}

#[test]
fn prg_09_level_is_clamped() {
    let mut e = church();
    run(&mut e, "301 @ 150 Enter").unwrap();
    assert_eq!(e.programmer().value(301, "Dimmer"), Some(Value::Level(1.0)));
    run(&mut e, "302 @ 0 Enter").unwrap();
    assert_eq!(e.programmer().value(302, "Dimmer"), Some(Value::Level(0.0)));
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

// ---- PRG-100: a relative level ----------------------------------------------

fn dimmer(e: &Engine, id: FixtureId) -> f32 {
    match e.programmer().value(id, "Dimmer") {
        Some(Value::Level(v)) => v,
        other => panic!("{id}: {other:?}"),
    }
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn prg_100_plus_and_minus_move_from_the_programmer_level() {
    let mut e = church();
    run(&mut e, "201 @ 20 Enter").unwrap();
    run(&mut e, "202 @ 60 Enter").unwrap();
    run(&mut e, "201 Thro 202 @ + 10 Enter").unwrap();
    assert!(near(dimmer(&e, 201), 0.3) && near(dimmer(&e, 202), 0.7));
    run(&mut e, "201 Thro 202 @ - 25 Enter").unwrap();
    assert!(near(dimmer(&e, 201), 0.05) && near(dimmer(&e, 202), 0.45));
    // The joined form an OSC or IPC client may type.
    run(&mut e, "201 @ +5 Enter").unwrap();
    run(&mut e, "202 @ -5 Enter").unwrap();
    assert!(near(dimmer(&e, 201), 0.1) && near(dimmer(&e, 202), 0.4));
}

#[test]
fn prg_100_without_a_selection_it_acts_on_the_targets() {
    let mut e = church();
    run(&mut e, "201 Thro 202 @ 50 Enter").unwrap();
    run(&mut e, "@ + 10 Enter").unwrap();
    assert!(near(dimmer(&e, 201), 0.6) && near(dimmer(&e, 202), 0.6));
}

#[test]
fn prg_100_clamps_to_0_and_100() {
    let mut e = church();
    run(&mut e, "201 @ 95 Enter").unwrap();
    run(&mut e, "201 @ + 10 Enter").unwrap();
    assert!(near(dimmer(&e, 201), 1.0));
    run(&mut e, "201 @ - 150 Enter").unwrap();
    assert!(near(dimmer(&e, 201), 0.0));
}

#[test]
fn prg_100_without_a_programmer_value_it_starts_from_the_stage() {
    use cmdmx_core::playback::{Playback, Values};
    let mut e = church();
    let mut v: Values = std::collections::BTreeMap::new();
    v.entry(202.into())
        .or_default()
        .insert("Dimmer".to_string(), 0.4.into());
    e.add_playback(Playback::new(1, 1, 1, v));
    e.set_fader(1, 1.0, 0.0).unwrap();
    e.tick(0.0);
    e.tick(0.025);
    run(&mut e, "202 @ + 10 Enter").unwrap();
    assert!(near(dimmer(&e, 202), 0.5), "{}", dimmer(&e, 202));
    // Nothing on stage and nothing in the programmer: from 0.
    run(&mut e, "203 @ - 10 Enter").unwrap();
    assert!(near(dimmer(&e, 203), 0.0));
    run(&mut e, "204 @ + 10 Enter").unwrap();
    assert!(near(dimmer(&e, 204), 0.1));
}

#[test]
fn prg_100_a_sign_without_a_number_is_refused_as_prg_11() {
    let mut e = church();
    run(&mut e, "201 @ 50 Enter").unwrap();
    for line in [
        "201 @ + Enter",
        "201 @ - Enter",
        "201 @ + + 5 Enter",
        "201 @ +x Enter",
    ] {
        assert!(
            matches!(run(&mut e, line), Err(CoreError::InvalidCommand(_))),
            "{line}"
        );
    }
    assert!(near(dimmer(&e, 201), 0.5));
}
