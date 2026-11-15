//! A typed value for an attribute: `specs/modules/programmer.md` PRG-104 (INP-01, IDEA-35 batch J).

#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use cmdmx_core::attribute::AttributeGroup;
use cmdmx_core::colour::{kelvin, planck, ColourModel, TargetColour};
use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::patch::Fixture;
use cmdmx_core::profile::Profile;
use cmdmx_core::value::Value;
use cmdmx_core::CoreError;

/// Pan over 540°, a gobo wheel of 7 slots (Open, then six gobos).
const SPOT_A: &str = r#"{ "manufacturer": "T", "model": "Spot A", "source": "channel-list", "modes": [
  { "name": "Std", "footprint": 5, "pan_degrees": 540, "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 2, "fine_channel": 3 },
    { "name": "Tilt", "group": "P", "kind": "Ltp", "channel": 4 },
    { "name": "Gobo 1", "group": "G", "kind": "Ltp", "channel": 5, "slots": [
      { "name": "Open", "from": 0, "to": 7 },
      { "name": "G1", "from": 8, "to": 15 },
      { "name": "G2", "from": 16, "to": 23 },
      { "name": "G3", "from": 24, "to": 31 },
      { "name": "G4", "from": 32, "to": 39 },
      { "name": "G5", "from": 40, "to": 47 },
      { "name": "G6", "from": 48, "to": 55 } ] } ] } ] }"#;

/// Pan over 630°, a gobo wheel of 9 slots on other DMX ranges.
const SPOT_B: &str = r#"{ "manufacturer": "T", "model": "Spot B", "source": "channel-list", "modes": [
  { "name": "Std", "footprint": 4, "pan_degrees": 630, "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 2 },
    { "name": "Tilt", "group": "P", "kind": "Ltp", "channel": 3 },
    { "name": "Gobo 1", "group": "G", "kind": "Ltp", "channel": 4, "slots": [
      { "name": "Open", "from": 0, "to": 9 },
      { "name": "S1", "from": 10, "to": 19 },
      { "name": "S2", "from": 20, "to": 29 },
      { "name": "S3", "from": 30, "to": 39 },
      { "name": "S4", "from": 40, "to": 49 },
      { "name": "S5", "from": 50, "to": 59 },
      { "name": "S6", "from": 60, "to": 69 },
      { "name": "S7", "from": 70, "to": 79 },
      { "name": "S8", "from": 80, "to": 89 } ] } ] } ] }"#;

const RGBW: &str = r#"{ "manufacturer": "T", "model": "RGBW Par", "source": "channel-list", "modes": [ {
  "name": "5ch", "footprint": 5,
  "colour": { "system": "RGB", "components": [ { "channel": 2 }, { "channel": 3 }, { "channel": 4 } ] },
  "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "White", "group": "C", "kind": "Ltp", "channel": 5, "emitter": true } ] } ] }"#;

const DIMMER: &str =
    r#"{ "attributes": [ { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 } ] }"#;

/// 401 (Spot A) @1, 402 (Spot B) @6, 201 and 202 (RGBW) @10 and @15, 301 (dimmer) @20.
fn rig() -> Engine {
    let mut e = Engine::new(Settings::default());
    for (name, json) in [
        ("spot-a", SPOT_A),
        ("spot-b", SPOT_B),
        ("rgbw", RGBW),
        ("dimmer", DIMMER),
    ] {
        e.add_profile(name, Profile::from_json(json).unwrap())
            .unwrap();
    }
    for (id, profile, address) in [
        (401, "spot-a", 1),
        (402, "spot-b", 6),
        (201, "rgbw", 10),
        (202, "rgbw", 15),
        (301, "dimmer", 20),
    ] {
        e.patch(Fixture {
            parked: false,
            behaviour: Default::default(),
            swap_pan_tilt: false,
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

fn select(e: &mut Engine, ids: &[u32]) {
    for &id in ids {
        e.select_fixture(id).unwrap();
    }
}

fn level(e: &Engine, id: u32, attribute: &str) -> Option<f32> {
    match e.programmer().value(id, attribute) {
        Some(Value::Level(v)) => Some(v),
        None => None,
        other => panic!("{id} {attribute}: expected a level, got {other:?}"),
    }
}

fn colour(e: &Engine, id: u32) -> TargetColour {
    match e.programmer().value(id, "Colour") {
        Some(Value::Colour(c)) => c,
        other => panic!("{id}: expected a colour, got {other:?}"),
    }
}

fn close(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() < tol
}

/// The middle of a slot's DMX range as a level, as PAT-38 and the profile editor's Test set it.
fn middle(from: u8, to: u8) -> f32 {
    (f32::from(from) + f32::from(to)) / 2.0 / 255.0
}

fn refused(e: &mut Engine, attribute: &str, typed: f32) -> String {
    match e.type_value(attribute, typed) {
        Err(err) => err.to_string(),
        Ok(()) => panic!("{attribute} = {typed} should be refused"),
    }
}

// ---- percent of the range --------------------------------------------------------------

#[test]
fn prg_104_pan_50_is_the_middle_of_each_fixtures_own_range() {
    let mut e = rig();
    select(&mut e, &[401, 402]);
    e.type_value("Pan", 50.0).unwrap();
    // 540° and 630° movers both go to the middle of their own range.
    assert_eq!(level(&e, 401, "Pan"), Some(0.5));
    assert_eq!(level(&e, 402, "Pan"), Some(0.5));
    // Touched (PRG-12) and nothing else: Tilt stays out of the programmer.
    assert_eq!(level(&e, 401, "Tilt"), None);
    e.type_value("Pan", 0.0).unwrap();
    assert_eq!(level(&e, 402, "Pan"), Some(0.0));
    e.type_value("Pan", 100.0).unwrap();
    assert_eq!(level(&e, 401, "Pan"), Some(1.0));
    e.type_value("Dimmer", 12.5).unwrap();
    assert!(close(level(&e, 401, "Dimmer").unwrap(), 0.125, 1e-6));
}

#[test]
fn prg_104_reaches_the_targets_only_and_keeps_the_active_group() {
    let mut e = rig();
    select(&mut e, &[401, 402, 301]);
    e.set_active_group(AttributeGroup::I);
    e.next_fixture(); // 401 alone is the target
    e.type_value("Pan", 25.0).unwrap();
    assert_eq!(level(&e, 401, "Pan"), Some(0.25));
    assert_eq!(level(&e, 402, "Pan"), None, "402 is not a target");
    assert_eq!(
        e.active_group(),
        AttributeGroup::I,
        "the wheels stay where they were"
    );
}

#[test]
fn prg_104_percent_out_of_range_is_refused_and_changes_nothing() {
    let mut e = rig();
    select(&mut e, &[401, 402]);
    e.type_value("Pan", 40.0).unwrap();
    for bad in [100.5, 150.0, -1.0, f32::NAN] {
        assert_eq!(refused(&mut e, "Pan", bad), "Pan takes 0–100 %", "{bad}");
    }
    assert_eq!(level(&e, 401, "Pan"), Some(0.4));
    assert_eq!(level(&e, 402, "Pan"), Some(0.4));
}

// ---- Hue in degrees, Kelvin in kelvin ---------------------------------------------------

#[test]
fn prg_104_hue_240_turns_red_to_blue_keeping_brightness() {
    let mut e = rig();
    e.set_programmer_value(
        201,
        "Colour",
        TargetColour {
            r: 0.8,
            g: 0.0,
            b: 0.0,
        },
    )
    .unwrap();
    e.set_programmer_value(
        202,
        "Colour",
        TargetColour {
            r: 0.0,
            g: 1.0,
            b: 0.0,
        },
    )
    .unwrap();
    select(&mut e, &[201, 202]);
    e.type_value("Hue", 240.0).unwrap();
    let a = colour(&e, 201);
    let b = colour(&e, 202);
    assert!(
        close(a.r, 0.0, 1e-3) && close(a.g, 0.0, 1e-3) && close(a.b, 0.8, 1e-3),
        "{a:?}"
    );
    assert!(
        close(b.r, 0.0, 1e-3) && close(b.g, 0.0, 1e-3) && close(b.b, 1.0, 1e-3),
        "{b:?}"
    );
    e.type_value("Hue", 360.0).unwrap();
    let a = colour(&e, 201);
    assert!(
        close(a.r, 0.8, 1e-3) && close(a.b, 0.0, 1e-3),
        "360° is red again: {a:?}"
    );
    assert_eq!(refused(&mut e, "Hue", 361.0), "Hue takes 0–360°");
    // Saturation is a percentage wheel of the same model.
    e.type_value("Saturation", 50.0).unwrap();
    let (_, s, _) = colour(&e, 201).to_hsv();
    assert!(close(s, 0.5, 1e-3), "{s}");
}

#[test]
fn prg_104_kelvin_3200_lands_on_the_locus() {
    let mut e = rig();
    e.set_programmer_value(201, "Colour", planck(5600.0).scaled(0.5))
        .unwrap();
    select(&mut e, &[201]);
    e.set_colour_model(ColourModel::Kelvin);
    e.type_value("Kelvin", 3200.0).unwrap();
    let c = colour(&e, 201);
    let k = kelvin(c).unwrap();
    assert!((k - 3200.0).abs() <= 5.0, "reads {k} K");
    assert!(
        close(c.r.max(c.g).max(c.b), 0.5, 1e-3),
        "brightness kept: {c:?}"
    );
    for bad in [1700.0, 10001.0] {
        assert_eq!(refused(&mut e, "Kelvin", bad), "Kelvin takes 1800–10000 K");
    }
}

#[test]
fn prg_104_a_colour_wheel_of_another_model_is_refused() {
    let mut e = rig();
    select(&mut e, &[201]);
    // HSI is the model: there is no Kelvin wheel to type into.
    assert!(matches!(
        e.type_value("Kelvin", 3200.0),
        Err(CoreError::AttributeNotInTargets(_))
    ));
    assert_eq!(e.programmer().value(201, "Colour"), None);
}

// ---- slots -----------------------------------------------------------------------------

#[test]
fn prg_104_gobo_slot_3_is_each_fixtures_own_third_slot() {
    let mut e = rig();
    select(&mut e, &[401, 402]);
    e.type_value("Gobo 1", 3.0).unwrap();
    assert!(close(
        level(&e, 401, "Gobo 1").unwrap(),
        middle(16, 23),
        1e-6
    ));
    assert!(close(
        level(&e, 402, "Gobo 1").unwrap(),
        middle(20, 29),
        1e-6
    ));
    e.type_value("Gobo 1", 1.0).unwrap();
    assert!(close(level(&e, 401, "Gobo 1").unwrap(), middle(0, 7), 1e-6));
    e.tick(0.0);
    let dmx = e.frame(1).and_then(|f| f.channel(5)).unwrap();
    assert!(dmx <= 7, "Open's range on 401: {dmx}");
}

#[test]
fn prg_104_slot_past_the_last_is_refused_for_all() {
    let mut e = rig();
    select(&mut e, &[401]);
    assert_eq!(refused(&mut e, "Gobo 1", 9.0), "Gobo 1 has 7 slots");
    assert_eq!(level(&e, 401, "Gobo 1"), None);
    // With 402 (9 slots) in the selection, slot 8 is past 401's last: nothing changes on either.
    select(&mut e, &[402]);
    assert_eq!(refused(&mut e, "Gobo 1", 8.0), "Gobo 1 has 7 slots");
    assert_eq!(level(&e, 401, "Gobo 1"), None);
    assert_eq!(level(&e, 402, "Gobo 1"), None);
    // A slot number is a whole number from 1.
    for bad in [0.0, 2.5, -3.0] {
        assert_eq!(
            refused(&mut e, "Gobo 1", bad),
            "Gobo 1 has 7 slots",
            "{bad}"
        );
    }
    e.type_value("Gobo 1", 7.0).unwrap();
    assert!(close(
        level(&e, 401, "Gobo 1").unwrap(),
        middle(48, 55),
        1e-6
    ));
    assert!(close(
        level(&e, 402, "Gobo 1").unwrap(),
        middle(60, 69),
        1e-6
    ));
}

// ---- nobody has it ---------------------------------------------------------------------

#[test]
fn prg_104_an_attribute_no_target_has_is_refused() {
    let mut e = rig();
    select(&mut e, &[301]);
    assert!(matches!(
        e.type_value("Gobo 1", 3.0),
        Err(CoreError::AttributeNotInTargets(_))
    ));
    assert!(matches!(
        e.type_value("Pan", 50.0),
        Err(CoreError::AttributeNotInTargets(_))
    ));
    assert_eq!(level(&e, 301, "Pan"), None);
    e.clear_selection();
    assert!(matches!(
        e.type_value("Dimmer", 50.0),
        Err(CoreError::EmptySelection)
    ));
}
