//! Colour temperature (VS2-45, IDEA-32, ADR-010): PRG-63..66.
//!
//! API these tests fix (`cmdmx_core::colour`):
//! - `ColourModel::Kelvin`, wheel `"Kelvin"`; its 0.0..=1.0 value is (555.6 − mired) / 455.6.
//! - `planck(kelvin: f32) -> TargetColour`: the locus point (Kim et al. spline) in linear BT.2020,
//!   max component 1.
//! - `kelvin(colour: TargetColour) -> Option<f32>`: the nearest locus point's temperature (CIE
//!   1960 uv), clamped to 1800..=10000; `None` for black.
//! - `CtStep` (`CtoFull`, `CtoHalf`, `CtoQuarter`, `CtoEighth`, `CtbFull`, `CtbHalf`,
//!   `CtbQuarter`, `CtbEighth`) with `mired() -> f32`, and `Engine::ct_step(CtStep)`.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use cmdmx_core::attribute::AttributeGroup;
use cmdmx_core::colour::{kelvin, planck, ColourModel, CtStep, TargetColour};
use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::patch::Fixture;
use cmdmx_core::profile::Profile;
use cmdmx_core::programmer::Resolution::Normal;
use cmdmx_core::value::Value;
use cmdmx_core::CoreError;

const RGBW: &str = r#"{ "manufacturer": "T", "model": "RGBW Par", "source": "channel-list", "modes": [ {
  "name": "5ch", "footprint": 5,
  "colour": { "system": "RGB", "components": [ { "channel": 2 }, { "channel": 3 }, { "channel": 4 } ] },
  "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "White", "group": "C", "kind": "Ltp", "channel": 5, "emitter": true } ] } ] }"#;

const DIMMER: &str =
    r#"{ "attributes": [ { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 } ] }"#;

/// 201 @1, 202 @6, 203 @11 (RGBW), 301 @16 (dimmer only).
fn rig() -> Engine {
    let mut e = Engine::new(Settings::default());
    e.add_profile("rgbw", Profile::from_json(RGBW).unwrap())
        .unwrap();
    e.add_profile("dimmer", Profile::from_json(DIMMER).unwrap())
        .unwrap();
    for (id, profile, address) in [
        (201, "rgbw", 1),
        (202, "rgbw", 6),
        (203, "rgbw", 11),
        (301, "dimmer", 16),
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

fn colour_of(e: &Engine, id: u32) -> TargetColour {
    match e.programmer().value(id, "Colour") {
        Some(Value::Colour(c)) => c,
        other => panic!("fixture {id}: expected a colour, got {other:?}"),
    }
}

fn close(actual: TargetColour, expected: TargetColour, tol: f32) {
    let ok = |a: f32, b: f32| (a - b).abs() < tol;
    assert!(
        ok(actual.r, expected.r) && ok(actual.g, expected.g) && ok(actual.b, expected.b),
        "expected {expected:?}, got {actual:?}"
    );
}

fn temperature(e: &Engine, id: u32) -> f32 {
    kelvin(colour_of(e, id)).unwrap()
}

fn mired_to_value(mired: f32) -> f32 {
    (555.6 - mired) / 455.6
}

fn select(e: &mut Engine, ids: &[u32]) {
    for &id in ids {
        e.select_fixture(id).unwrap();
    }
}

// ---- the locus -------------------------------------------------------------------------

#[test]
fn prg_64_locus_points_in_bt2020() {
    close(planck(3200.0), rgb(1.0, 0.678, 0.293), 0.005);
    close(planck(1800.0), rgb(1.0, 0.373, 0.041), 0.005);
    close(planck(10000.0), rgb(0.678, 0.717, 1.0), 0.005);
}

#[test]
fn prg_64_reading_round_trips_and_clamps() {
    let white = kelvin(rgb(1.0, 1.0, 1.0)).unwrap();
    assert!((white - 6500.0).abs() <= 10.0, "white reads {white}");
    for k in [1800.0, 2700.0, 3200.0, 4300.0, 5600.0, 8000.0, 10000.0] {
        let back = kelvin(planck(k)).unwrap();
        assert!((back - k).abs() <= 5.0, "{k} K reads back as {back}");
        let half = kelvin(planck(k).scaled(0.5)).unwrap();
        assert!(
            (half - k).abs() <= 5.0,
            "brightness does not change the reading"
        );
    }
    assert_eq!(kelvin(rgb(0.5, 0.0, 0.0)), Some(1800.0), "red clamps warm");
    assert_eq!(
        kelvin(rgb(0.0, 0.0, 1.0)),
        Some(10000.0),
        "blue clamps cool"
    );
    assert_eq!(kelvin(rgb(0.0, 0.0, 0.0)), None, "black has none");
}

// ---- PRG-63: the Kelvin model ----------------------------------------------------------

#[test]
fn prg_63_kelvin_is_a_view_that_changes_nothing() {
    let mut e = rig();
    e.set_programmer_value(201, "Colour", planck(3200.0))
        .unwrap();
    e.set_programmer_value(201, "Dimmer", 1.0).unwrap();
    select(&mut e, &[201]);
    e.set_active_group(AttributeGroup::C);
    e.tick(0.0);
    let before = e.frame(1).unwrap().slots().to_vec();
    e.set_colour_model(ColourModel::Kelvin);
    let wheels: Vec<(String, f32)> = e
        .programmer_view()
        .wheels
        .into_iter()
        .map(|w| (w.attribute, w.value))
        .collect();
    assert_eq!(wheels.len(), 2);
    assert_eq!(wheels[0].0, "Kelvin");
    assert!((wheels[0].1 - mired_to_value(1e6 / 3200.0)).abs() < 0.002);
    assert_eq!(wheels[1], ("White".to_string(), 0.0));
    e.tick(0.025);
    assert_eq!(e.frame(1).unwrap().slots().to_vec(), before);
    assert_eq!(ColourModel::Kelvin.wheels(), &["Kelvin"]);
    assert_eq!(
        serde_json::to_string(&ColourModel::Kelvin).unwrap(),
        "\"Kelvin\""
    );
}

// ---- PRG-65: turning Kelvin --------------------------------------------------------------

#[test]
fn prg_65_turning_kelvin_lands_on_the_locus() {
    let mut e = rig();
    e.set_programmer_value(201, "Colour", rgb(1.0, 1.0, 1.0))
        .unwrap();
    select(&mut e, &[201]);
    e.set_colour_model(ColourModel::Kelvin);
    // 89 Normal steps down ≈ +159 mired: 6504 K → about 3200 K.
    e.turn_wheel("Kelvin", -89, Normal).unwrap();
    let t = temperature(&e, 201);
    assert!((t - 3197.0).abs() <= 10.0, "reads {t}");
    close(colour_of(&e, 201), rgb(1.0, 0.68, 0.29), 0.01);
    close(colour_of(&e, 201), planck(t), 0.005);
}

#[test]
fn prg_65_relative_per_target_and_brightness_kept() {
    let mut e = rig();
    e.set_programmer_value(201, "Colour", planck(5600.0))
        .unwrap();
    e.set_programmer_value(202, "Colour", planck(3200.0).scaled(0.5))
        .unwrap();
    select(&mut e, &[201, 202]);
    e.set_colour_model(ColourModel::Kelvin);
    e.turn_wheel("Kelvin", 10, Normal).unwrap();
    let step = 10.0 * 455.6 / 255.0;
    for (id, k) in [(201, 5600.0), (202, 3200.0)] {
        let want = 1e6 / (1e6 / k - step);
        let t = temperature(&e, id);
        assert!((t - want).abs() <= 5.0, "{id}: {t} instead of {want}");
    }
    let c = colour_of(&e, 202);
    assert!((c.r.max(c.g).max(c.b) - 0.5).abs() < 1e-4, "max kept");
}

#[test]
fn prg_65_clamps_and_snaps_off_locus_colours() {
    let mut e = rig();
    e.set_programmer_value(201, "Colour", planck(10000.0))
        .unwrap();
    e.set_programmer_value(202, "Colour", rgb(0.5, 0.0, 0.0))
        .unwrap();
    select(&mut e, &[201, 202]);
    e.set_colour_model(ColourModel::Kelvin);
    e.turn_wheel("Kelvin", 1, Normal).unwrap();
    assert!(
        (temperature(&e, 201) - 10000.0).abs() <= 5.0,
        "clamped cool"
    );
    e.turn_wheel("Kelvin", -2, Normal).unwrap();
    // 202: red reads 1800 K (clamped), one step up then two down → 1800 K at half level.
    close(colour_of(&e, 202), rgb(0.5, 0.187, 0.020), 0.005);
}

#[test]
fn prg_65_black_starts_full_at_6500() {
    let mut e = rig();
    e.set_programmer_value(201, "Colour", rgb(0.0, 0.0, 0.0))
        .unwrap();
    select(&mut e, &[201]);
    e.set_colour_model(ColourModel::Kelvin);
    e.turn_wheel("Kelvin", -1, Normal).unwrap();
    let c = colour_of(&e, 201);
    assert!((c.r.max(c.g).max(c.b) - 1.0).abs() < 1e-4, "full");
    let t = temperature(&e, 201);
    // One step warmer than 6500 K (+1.8 mired) is about 6425 K.
    assert!((t - 6425.0).abs() <= 15.0, "got {t}");
}

// ---- PRG-66: CTO and CTB steps -----------------------------------------------------------

#[test]
fn prg_66_nominal_mired_values() {
    for (step, mired) in [
        (CtStep::CtoFull, 159.0),
        (CtStep::CtoHalf, 81.0),
        (CtStep::CtoQuarter, 42.0),
        (CtStep::CtoEighth, 20.0),
        (CtStep::CtbFull, -137.0),
        (CtStep::CtbHalf, -78.0),
        (CtStep::CtbQuarter, -35.0),
        (CtStep::CtbEighth, -18.0),
    ] {
        assert_eq!(step.mired(), mired, "{step:?}");
    }
}

#[test]
fn prg_66_steps_shift_each_target_like_a_gel() {
    let mut e = rig();
    e.set_programmer_value(201, "Colour", planck(5600.0))
        .unwrap();
    e.set_programmer_value(202, "Colour", planck(3200.0))
        .unwrap();
    e.set_programmer_value(203, "Colour", planck(5600.0))
        .unwrap();
    select(&mut e, &[201]);
    e.ct_step(CtStep::CtoHalf).unwrap();
    assert!((temperature(&e, 201) - 3853.0).abs() <= 5.0);
    e.clear_selection();
    select(&mut e, &[202, 203]);
    e.ct_step(CtStep::CtbFull).unwrap();
    assert!((temperature(&e, 202) - 5698.0).abs() <= 5.0);
    assert!(
        (temperature(&e, 203) - 10000.0).abs() <= 5.0,
        "clamped cool"
    );
}

#[test]
fn prg_66_works_in_any_model_and_keeps_brightness() {
    let mut e = rig();
    e.set_programmer_value(201, "Colour", planck(5600.0).scaled(0.4))
        .unwrap();
    select(&mut e, &[201]);
    assert_eq!(e.colour_model(), ColourModel::Hsi);
    e.ct_step(CtStep::CtoFull).unwrap();
    let c = colour_of(&e, 201);
    assert!((c.r.max(c.g).max(c.b) - 0.4).abs() < 1e-4);
    assert!((temperature(&e, 201) - 1e6 / (1e6 / 5600.0 + 159.0)).abs() <= 5.0);
}

#[test]
fn prg_66_refused_without_colour_targets() {
    let mut e = rig();
    assert_eq!(e.ct_step(CtStep::CtoFull), Err(CoreError::EmptySelection));
    select(&mut e, &[301]);
    assert_eq!(
        e.ct_step(CtStep::CtoFull),
        Err(CoreError::AttributeNotInTargets("Colour".into()))
    );
}
