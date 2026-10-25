//! Colour as one value (VS2-06): PRG-17 (C), PRG-30..34, LIB-03 (Colour default and hold),
//! LIB-04 (virtual dimmer), LIB-07 (cells driven by the fixture's attributes); ADR-005.
//! Frame-level LIB-04 and PRG-12 (colour) are also scenarios in `tests/scenarios/`.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;

use cmdmx_core::attribute::AttributeGroup;
use cmdmx_core::colour::{ColourModel, TargetColour};
use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::patch::Fixture;
use cmdmx_core::playback::Playback;
use cmdmx_core::profile::Profile;
use cmdmx_core::programmer::Item;
use cmdmx_core::programmer::Resolution::{Coarse, Normal};
use cmdmx_core::value::Value;
use cmdmx_core::CoreError;

const RGBW: &str = r#"{ "manufacturer": "T", "model": "RGBW Par", "source": "channel-list", "modes": [ {
  "name": "5ch", "footprint": 5,
  "colour": { "system": "RGB", "components": [ { "channel": 2 }, { "channel": 3 }, { "channel": 4 } ] },
  "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "White", "group": "C", "kind": "Ltp", "channel": 5, "emitter": true } ] } ] }"#;

/// U'King RGBL: no dimmer channel.
const RGBL: &str = r#"{ "manufacturer": "T", "model": "RGBL Par", "source": "channel-list", "modes": [ {
  "name": "4ch", "footprint": 4, "virtual_dimmer": true,
  "colour": { "system": "RGB", "components": [ { "channel": 1 }, { "channel": 2 }, { "channel": 3 } ] },
  "attributes": [ { "name": "Lime", "group": "C", "kind": "Ltp", "channel": 4, "emitter": true } ] } ] }"#;

/// Showtec LED Par 56 "6 Channel": no dimmer channel.
const RGB: &str = r#"{ "manufacturer": "T", "model": "RGB Par", "source": "channel-list", "modes": [ {
  "name": "6ch", "footprint": 6, "virtual_dimmer": true,
  "colour": { "system": "RGB", "components": [ { "channel": 1 }, { "channel": 2 }, { "channel": 3 } ] },
  "attributes": [
    { "name": "Mode", "group": "S", "kind": "Ltp", "channel": 4 },
    { "name": "Speed", "group": "S", "kind": "Ltp", "channel": 5 },
    { "name": "Strobe", "group": "I", "kind": "Ltp", "channel": 6 } ] } ] }"#;

const CMY: &str = r#"{ "manufacturer": "T", "model": "CMY Spot", "source": "channel-list", "modes": [ {
  "name": "4ch", "footprint": 4,
  "colour": { "system": "CMY", "components": [ { "channel": 2 }, { "channel": 3 }, { "channel": 4 } ] },
  "attributes": [ { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 } ] } ] }"#;

/// Shehds-like three-cell RGBW wash with a master dimmer on 6.
const CELLS: &str = r#"{ "manufacturer": "T", "model": "Cells", "source": "channel-list", "modes": [ {
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

/// Universe 1: 201 @1, 202 @6 (RGBW), 211 @11 (RGBL), 221 @15 (RGB), 401 @21 (CMY),
/// 501 @25 (cells: Dimmer 30, cell RGBW 32–35, 36–39, 40–43), 301 @49 (dimmer).
fn rig() -> Engine {
    let mut e = Engine::new(Settings::default());
    for (key, json) in [
        ("rgbw", RGBW),
        ("rgbl", RGBL),
        ("rgb", RGB),
        ("cmy", CMY),
        ("cells", CELLS),
        ("dimmer", DIMMER),
    ] {
        e.add_profile(key, Profile::from_json(json).unwrap())
            .unwrap();
    }
    for (id, profile, address) in [
        (201, "rgbw", 1),
        (202, "rgbw", 6),
        (211, "rgbl", 11),
        (221, "rgb", 15),
        (401, "cmy", 21),
        (501, "cells", 25),
        (301, "dimmer", 49),
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

fn rgb(r: f32, g: f32, b: f32) -> TargetColour {
    TargetColour { r, g, b }
}

fn select(e: &mut Engine, ids: &[u32]) {
    for &id in ids {
        e.select_fixture(id).unwrap();
    }
}

fn dmx(e: &Engine, address: u16) -> u8 {
    e.frame(1).and_then(|f| f.channel(address)).unwrap()
}

fn dmx_range(e: &Engine, from: u16, to: u16) -> Vec<u8> {
    (from..=to).map(|a| dmx(e, a)).collect()
}

fn programmer_colour(e: &Engine, id: u32) -> TargetColour {
    match e.programmer().value(id, "Colour") {
        Some(Value::Colour(c)) => c,
        other => panic!("fixture {id}: expected a colour in the programmer, got {other:?}"),
    }
}

fn assert_colour(actual: TargetColour, expected: TargetColour) {
    let close = |a: f32, b: f32| (a - b).abs() < 1e-4;
    assert!(
        close(actual.r, expected.r) && close(actual.g, expected.g) && close(actual.b, expected.b),
        "expected {expected:?}, got {actual:?}"
    );
}

fn wheels(e: &Engine) -> Vec<(String, f32)> {
    e.programmer_view()
        .wheels
        .into_iter()
        .map(|w| (w.attribute, w.value))
        .collect()
}

fn assert_wheels(e: &Engine, expected: &[(&str, f32)]) {
    let actual = wheels(e);
    let names: Vec<&str> = actual.iter().map(|(n, _)| n.as_str()).collect();
    let want: Vec<&str> = expected.iter().map(|(n, _)| *n).collect();
    assert_eq!(names, want);
    for ((name, value), (_, want)) in actual.iter().zip(expected) {
        assert!(
            (value - want).abs() < 1e-4,
            "{name}: expected {want}, got {value}"
        );
    }
}

// ---- value type -------------------------------------------------------------------

#[test]
fn adr_005_colour_is_one_value_of_the_right_kind() {
    let mut e = rig();
    e.set_programmer_value(201, "Colour", rgb(1.0, 0.42, 0.0))
        .unwrap();
    assert_eq!(
        e.programmer().value(201, "Colour"),
        Some(Value::Colour(rgb(1.0, 0.42, 0.0)))
    );
    assert!(matches!(
        e.set_programmer_value(201, "Colour", 0.5),
        Err(CoreError::WrongValueKind { .. })
    ));
    assert!(matches!(
        e.set_programmer_value(201, "Dimmer", rgb(1.0, 0.0, 0.0)),
        Err(CoreError::WrongValueKind { .. })
    ));
    assert!(matches!(
        e.set_programmer_value(201, "Colour", rgb(1.5, 0.0, 0.0)),
        Err(CoreError::InvalidLevel(_))
    ));
    assert!(matches!(
        e.set_programmer_value(301, "Colour", rgb(1.0, 0.0, 0.0)),
        Err(CoreError::UnknownAttribute { .. })
    ));
}

#[test]
fn adr_005_values_serialise_as_a_number_or_an_rgb_object() {
    let values: BTreeMap<String, Value> =
        serde_json::from_str(r#"{ "Dimmer": 0.5, "Colour": { "r": 1.0, "g": 0.42, "b": 0.0 } }"#)
            .unwrap();
    assert_eq!(values["Dimmer"], Value::Level(0.5));
    assert_eq!(values["Colour"], Value::Colour(rgb(1.0, 0.42, 0.0)));
    assert_eq!(
        serde_json::to_string(&values).unwrap(),
        r#"{"Colour":{"r":1.0,"g":0.42,"b":0.0},"Dimmer":0.5}"#
    );
}

#[test]
fn prg_15_contents_carry_the_colour_value() {
    let mut e = rig();
    e.set_programmer_value(201, "Colour", rgb(0.0, 0.0, 1.0))
        .unwrap();
    e.set_programmer_value(201, "Dimmer", 1.0).unwrap();
    let c = e.contents(&[AttributeGroup::C]);
    assert_eq!(
        c,
        BTreeMap::from([(
            Item::fixture(201),
            BTreeMap::from([("Colour".to_string(), rgb(0.0, 0.0, 1.0).into())])
        )])
    );
}

// ---- PRG-17: colour wheel layout ---------------------------------------------------

#[test]
fn prg_17_colour_wheels_follow_the_model_then_emitters() {
    let mut e = rig();
    select(&mut e, &[201, 211]);
    e.set_active_group(AttributeGroup::C);
    assert_eq!(e.colour_model(), ColourModel::Hsi, "HSI is the default");
    let names = |e: &Engine| -> Vec<String> { wheels(e).into_iter().map(|(n, _)| n).collect() };
    assert_eq!(names(&e), ["Hue", "Saturation", "White", "Lime"]);
    e.set_colour_model(ColourModel::Rgb);
    assert_eq!(names(&e), ["Red", "Green", "Blue", "White", "Lime"]);
    e.set_colour_model(ColourModel::Cmy);
    assert_eq!(names(&e), ["Cyan", "Magenta", "Yellow", "White", "Lime"]);
    assert_eq!(e.programmer_view().colour_model, ColourModel::Cmy);
}

#[test]
fn prg_17_virtual_dimmer_is_an_intensity_wheel() {
    let mut e = rig();
    select(&mut e, &[221]);
    assert_eq!(e.active_group(), AttributeGroup::I);
    let names: Vec<String> = wheels(&e).into_iter().map(|(n, _)| n).collect();
    assert_eq!(names, ["Dimmer", "Strobe"]);
}

// ---- PRG-30: models are views ------------------------------------------------------

#[test]
fn prg_30_hsi_rgb_cmy_are_views_of_one_value() {
    let mut e = rig();
    e.set_programmer_value(201, "Colour", rgb(1.0, 0.0, 0.0))
        .unwrap();
    e.set_programmer_value(201, "Dimmer", 1.0).unwrap();
    select(&mut e, &[201]);
    e.set_active_group(AttributeGroup::C);
    e.tick(0.0);
    let frame = dmx_range(&e, 1, 5);
    assert_eq!(frame, [255, 255, 0, 0, 0]);
    assert_wheels(&e, &[("Hue", 0.0), ("Saturation", 1.0), ("White", 0.0)]);
    e.set_colour_model(ColourModel::Rgb);
    assert_wheels(
        &e,
        &[("Red", 1.0), ("Green", 0.0), ("Blue", 0.0), ("White", 0.0)],
    );
    e.set_colour_model(ColourModel::Cmy);
    assert_wheels(
        &e,
        &[
            ("Cyan", 0.0),
            ("Magenta", 1.0),
            ("Yellow", 1.0),
            ("White", 0.0),
        ],
    );
    e.tick(0.025);
    assert_eq!(
        dmx_range(&e, 1, 5),
        frame,
        "switching the model changes nothing"
    );
    assert_colour(programmer_colour(&e, 201), rgb(1.0, 0.0, 0.0));
}

#[test]
fn prg_30_hue_wheel_value_is_a_fraction_of_360() {
    let mut e = rig();
    e.set_programmer_value(201, "Colour", rgb(0.0, 0.0, 1.0))
        .unwrap();
    select(&mut e, &[201]);
    e.set_active_group(AttributeGroup::C);
    assert_wheels(
        &e,
        &[("Hue", 240.0 / 360.0), ("Saturation", 1.0), ("White", 0.0)],
    );
}

// ---- PRG-31: one colour wheel for every colour system --------------------------------

#[test]
fn prg_31_hue_applies_to_every_colour_system() {
    let mut e = rig();
    e.execute_command(&[
        "201", "And", "211", "And", "221", "And", "401", "@", "100", "Enter",
    ])
    .unwrap();
    e.set_active_group(AttributeGroup::C);
    // From the default open white: full saturation at hue 0 (red), then hue to 240° (blue).
    e.turn_wheel("Saturation", 255, Normal).unwrap();
    e.turn_wheel("Hue", 170, Normal).unwrap();
    e.tick(0.0);
    assert_eq!(
        dmx_range(&e, 1, 5),
        [255, 0, 0, 255, 0],
        "RGBW: dimmer, blue, white 0"
    );
    assert_eq!(
        dmx_range(&e, 11, 14),
        [0, 0, 255, 0],
        "RGBL, virtual dimmer at 100 %"
    );
    assert_eq!(
        dmx_range(&e, 15, 17),
        [0, 0, 255],
        "RGB, virtual dimmer at 100 %"
    );
    assert_eq!(
        dmx_range(&e, 21, 24),
        [255, 255, 255, 0],
        "CMY: C = 1 − r, M = 1 − g, Y = 1 − b"
    );
}

#[test]
fn prg_31_colour_wheel_skips_targets_without_colour_and_refuses_when_none_has_it() {
    let mut e = rig();
    select(&mut e, &[201, 301]);
    e.turn_wheel("Red", -255, Normal).unwrap_err(); // Red is not a wheel in HSI
    e.set_colour_model(ColourModel::Rgb);
    e.turn_wheel("Red", -255, Normal).unwrap();
    assert_colour(programmer_colour(&e, 201), rgb(0.0, 1.0, 1.0));
    assert_eq!(e.programmer().value(301, "Colour"), None);
    e.clear();
    e.clear();
    select(&mut e, &[301]);
    assert_eq!(
        e.turn_wheel("Red", 1, Normal),
        Err(CoreError::AttributeNotInTargets("Red".into()))
    );
}

// ---- PRG-32: relative per target -------------------------------------------------------

#[test]
fn prg_32_hue_keeps_offsets_and_wraps() {
    let mut e = rig();
    e.set_programmer_value(201, "Colour", rgb(1.0, 0.0, 0.0))
        .unwrap(); // 0°
    e.set_programmer_value(202, "Colour", rgb(0.0, 1.0, 0.0))
        .unwrap(); // 120°
    select(&mut e, &[201, 202]); // a new selection after the changes (PRG-03)
    e.turn_wheel("Hue", -85, Normal).unwrap(); // −120°
    assert_colour(programmer_colour(&e, 201), rgb(0.0, 0.0, 1.0)); // 240°, wrapped
    assert_colour(programmer_colour(&e, 202), rgb(1.0, 0.0, 0.0)); // 0°
}

#[test]
fn prg_32_hue_and_saturation_keep_the_brightest_component() {
    let mut e = rig();
    e.set_programmer_value(201, "Colour", rgb(0.5, 0.0, 0.0))
        .unwrap();
    select(&mut e, &[201]);
    e.turn_wheel("Hue", 170, Normal).unwrap();
    assert_colour(programmer_colour(&e, 201), rgb(0.0, 0.0, 0.5));
    e.turn_wheel("Saturation", -255, Normal).unwrap();
    assert_colour(programmer_colour(&e, 201), rgb(0.5, 0.5, 0.5));
}

#[test]
fn prg_32_rgb_and_cmy_wheels_clamp_per_target() {
    let mut e = rig();
    e.set_programmer_value(201, "Colour", rgb(0.2, 0.0, 0.0))
        .unwrap();
    e.set_programmer_value(202, "Colour", rgb(0.7, 0.0, 0.0))
        .unwrap();
    select(&mut e, &[201, 202]); // a new selection after the changes (PRG-03)
    e.set_colour_model(ColourModel::Rgb);
    e.turn_wheel("Red", 10, Coarse).unwrap(); // +0.40
    assert_colour(programmer_colour(&e, 201), rgb(0.6, 0.0, 0.0));
    assert_colour(programmer_colour(&e, 202), rgb(1.0, 0.0, 0.0));
    e.set_colour_model(ColourModel::Cmy);
    e.turn_wheel("Cyan", 5, Coarse).unwrap(); // C +0.20 = r −0.20
    assert_colour(programmer_colour(&e, 201), rgb(0.4, 0.0, 0.0));
    assert_colour(programmer_colour(&e, 202), rgb(0.8, 0.0, 0.0));
}

// ---- PRG-34: direct emitter wheels -------------------------------------------------------

#[test]
fn prg_34_emitter_wheel_touches_only_fixtures_with_that_emitter() {
    let mut e = rig();
    select(&mut e, &[201, 221]);
    e.set_active_group(AttributeGroup::C);
    e.turn_wheel("White", 10, Coarse).unwrap();
    match e.programmer().value(201, "White") {
        Some(Value::Level(v)) => assert!((v - 0.4).abs() < 1e-4, "White {v}"),
        other => panic!("expected White 0.4, got {other:?}"),
    }
    assert_eq!(
        e.programmer().value(201, "Colour"),
        None,
        "emitter is separate from Colour"
    );
    assert!(!e.programmer().in_programmer(221));
}

// ---- LIB-03: Colour default and hold ------------------------------------------------------

#[test]
fn lib_03_colour_defaults_to_open_white() {
    let mut e = rig();
    e.execute_command(&["201", "And", "221", "And", "401", "@", "100", "Enter"])
        .unwrap();
    e.tick(0.0);
    assert_eq!(
        dmx_range(&e, 1, 5),
        [255, 255, 255, 255, 0],
        "RGBW white from RGB, W 0"
    );
    assert_eq!(dmx_range(&e, 15, 17), [255, 255, 255]);
    assert_eq!(dmx_range(&e, 21, 24), [255, 0, 0, 0], "CMY open");
    select(&mut e, &[201]);
    e.set_active_group(AttributeGroup::C);
    assert_wheels(&e, &[("Hue", 0.0), ("Saturation", 0.0), ("White", 0.0)]);
}

#[test]
fn lib_03_released_colour_holds_its_last_value() {
    let mut e = rig();
    let red = BTreeMap::from([("Colour".to_string(), rgb(1.0, 0.0, 0.0).into())]);
    e.add_playback(Playback::new(
        1,
        1,
        1,
        BTreeMap::from([(Item::fixture(201), red.clone()), (Item::fixture(221), red)]),
    ));
    e.set_fader(1, 1.0, 0.0).unwrap();
    e.tick(0.0);
    e.set_fader(1, 0.0, 0.025).unwrap();
    e.execute_command(&["201", "And", "221", "@", "100", "Enter"])
        .unwrap();
    e.tick(0.025);
    assert_eq!(dmx_range(&e, 1, 4), [255, 255, 0, 0]);
    assert_eq!(
        dmx_range(&e, 15, 17),
        [255, 0, 0],
        "virtual dimmer × held red"
    );
    // A wheel touch starts from the held colour (PRG-20).
    select(&mut e, &[201]);
    e.set_active_group(AttributeGroup::C);
    assert_wheels(&e, &[("Hue", 0.0), ("Saturation", 1.0), ("White", 0.0)]);
}

// ---- LIB-04: virtual dimmer -----------------------------------------------------------------

#[test]
fn lib_04_virtual_dimmer_scales_colour_and_emitters_and_obeys_the_grand_master() {
    let mut e = rig();
    e.set_programmer_value(211, "Colour", rgb(1.0, 0.5, 0.0))
        .unwrap();
    e.set_programmer_value(211, "Lime", 1.0).unwrap();
    e.set_programmer_value(211, "Dimmer", 0.5).unwrap();
    e.tick(0.0);
    assert_eq!(dmx_range(&e, 11, 14), [128, 64, 0, 128]);
    e.set_grand_master(0.0).unwrap();
    e.tick(0.025);
    assert_eq!(
        dmx_range(&e, 11, 14),
        [0, 0, 0, 0],
        "blackout darkens the whole fixture"
    );
}

#[test]
fn prg_09_keypad_level_reaches_the_virtual_dimmer() {
    let mut e = rig();
    e.execute_command(&["221", "@", "40", "Enter"]).unwrap();
    assert_eq!(e.programmer().value(221, "Dimmer"), Some(Value::Level(0.4)));
}

// ---- LIB-07: cells follow the fixture -------------------------------------------------------

#[test]
fn lib_07_fixture_colour_and_emitters_drive_every_cell() {
    let mut e = rig();
    e.set_programmer_value(501, "Colour", rgb(0.0, 1.0, 0.0))
        .unwrap();
    e.set_programmer_value(501, "White", 0.6).unwrap();
    e.set_programmer_value(501, "Dimmer", 1.0).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, 30), 255, "master dimmer");
    for cell in [32, 36, 40] {
        assert_eq!(
            dmx_range(&e, cell, cell + 3),
            [0, 255, 0, 153],
            "cell at {cell}"
        );
    }
    select(&mut e, &[501]);
    e.set_active_group(AttributeGroup::C);
    let names: Vec<String> = wheels(&e).into_iter().map(|(n, _)| n).collect();
    assert_eq!(
        names,
        ["Hue", "Saturation", "White"],
        "one White for the fixture"
    );
}

// ---- IPC-21..23 (UX-02b): the Programmer window and the colour picker ---------------

#[test]
fn ipc_21_view_contents_are_the_programmer_values() {
    use cmdmx_core::programmer::FixtureValues;
    let mut e = rig();
    assert!(e.programmer_view().contents.is_empty());
    e.set_programmer_value(201, "Dimmer", 0.5).unwrap();
    e.set_programmer_value(201, "Colour", rgb(1.0, 0.2, 0.0))
        .unwrap();
    e.set_programmer_value(301, "Dimmer", 1.0).unwrap();
    assert_eq!(
        e.programmer_view().contents,
        vec![
            FixtureValues {
                fixture: 201,
                values: BTreeMap::from([
                    ("Colour".to_string(), Value::Colour(rgb(1.0, 0.2, 0.0))),
                    ("Dimmer".to_string(), Value::Level(0.5)),
                ]),
                palettes: BTreeMap::new(),
                fading: BTreeMap::new(),
            },
            FixtureValues {
                fixture: 301,
                values: BTreeMap::from([("Dimmer".to_string(), Value::Level(1.0))]),
                palettes: BTreeMap::new(),
                fading: BTreeMap::new(),
            },
        ]
    );
}

#[test]
fn ipc_22_view_colour_is_the_first_targets_colour() {
    let mut e = rig();
    assert_eq!(e.programmer_view().colour, None, "nothing selected");
    select(&mut e, &[301]);
    assert_eq!(e.programmer_view().colour, None, "a dimmer has no Colour");
    select(&mut e, &[202, 201]);
    // Nothing in the programmer: the output, open white (LIB-03, PRG-20).
    let c = e.programmer_view().colour.unwrap();
    assert_eq!((c.r, c.g, c.b, c.saturation), (1.0, 1.0, 1.0, 0.0));
    e.set_programmer_value(202, "Colour", rgb(0.0, 0.0, 1.0))
        .unwrap();
    let c = e.programmer_view().colour.unwrap();
    assert_eq!((c.r, c.g, c.b), (0.0, 0.0, 1.0));
    assert!((c.hue - 2.0 / 3.0).abs() < 1e-4, "hue {}", c.hue);
    assert!((c.saturation - 1.0).abs() < 1e-4);
}

#[test]
fn ipc_23_set_colour_sets_hue_and_saturation_on_every_colour_target() {
    let mut e = rig();
    assert_eq!(e.set_colour(0.5, 1.0), Err(CoreError::EmptySelection));
    select(&mut e, &[301]);
    assert_eq!(
        e.set_colour(0.5, 1.0),
        Err(CoreError::AttributeNotInTargets("Colour".into()))
    );
    e.clear_selection();
    select(&mut e, &[201, 221, 301]);
    e.set_colour(2.0 / 3.0, 1.0).unwrap();
    assert_colour(programmer_colour(&e, 201), rgb(0.0, 0.0, 1.0));
    assert_colour(programmer_colour(&e, 221), rgb(0.0, 0.0, 1.0));
    assert_eq!(e.programmer().value(301, "Colour"), None);
    // Like the HSI wheels, the brightest component stays (PRG-32).
    e.set_programmer_value(201, "Colour", rgb(0.5, 0.5, 0.5))
        .unwrap();
    e.set_colour(0.0, 1.0).unwrap();
    assert_colour(programmer_colour(&e, 201), rgb(0.5, 0.0, 0.0));
    assert_eq!(e.set_colour(1.5, 1.0), Err(CoreError::InvalidLevel(1.5)));
    assert_eq!(e.set_colour(0.5, -0.1), Err(CoreError::InvalidLevel(-0.1)));
}
