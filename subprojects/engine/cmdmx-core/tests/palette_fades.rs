//! Palette fades and overlap (VS2-18): `specs/modules/palettes.md` PAL-13..16.
//! Frame-level overlap is also the scenario `tests/scenarios/PAL-14-overlap-staggers-fixtures.json`.
//!
//! A fade starts with the first tick after the apply (PAL-13). New API these tests fix:
//! `Engine::set_palette_master(time: Option<f32>, overlap: Option<f32>)` (PAL-15; `None` keeps
//! the current value). `apply_palette(number, Timing)` now honours `Timing` and the settings.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;

use cmdmx_core::attribute::AttributeGroup::I;
use cmdmx_core::colour::TargetColour;
use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::palette::{Cell, GlobalValue, Palette, Record, RecordBy, Timing, Window};
use cmdmx_core::patch::Fixture;
use cmdmx_core::profile::Profile;
use cmdmx_core::programmer::Resolution;
use cmdmx_core::value::{Stored, Value};

const WASH: &str = r#"{ "manufacturer": "T", "model": "Wash", "source": "channel-list", "modes": [ {
  "name": "4ch", "footprint": 4,
  "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 2 },
    { "name": "Tilt", "group": "P", "kind": "Ltp", "channel": 3 },
    { "name": "Zoom", "group": "B", "kind": "Ltp", "channel": 4 } ] } ] }"#;

const RGBW: &str = r#"{ "manufacturer": "T", "model": "RGBW Par", "source": "channel-list", "modes": [ {
  "name": "5ch", "footprint": 5,
  "colour": { "system": "RGB", "components": [ { "channel": 2 }, { "channel": 3 }, { "channel": 4 } ] },
  "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "White", "group": "C", "kind": "Ltp", "channel": 5, "emitter": true } ] } ] }"#;

/// Palette numbers in the rig.
const RED: u32 = 1;
const PAN_FULL: u32 = 41;
const FULL: u32 = 81;
const OFF: u32 = 82;

/// Washes 101 @1, 102 @5, 103 @9, 104 @13 (Dimmer, Pan, Tilt, Zoom); RGBW par 201 @17
/// (Dimmer 17, RGB 18–20, White 21). Palettes: 1 red (global Colour), 41 Pan full (shared for
/// the washes), 81 Dimmer full and 82 Dimmer off (global).
fn rig_with(settings: Settings) -> Engine {
    let mut e = Engine::new(settings);
    e.add_profile("wash", Profile::from_json(WASH).unwrap())
        .unwrap();
    e.add_profile("rgbw", Profile::from_json(RGBW).unwrap())
        .unwrap();
    for (id, profile, address) in [
        (101, "wash", 1),
        (102, "wash", 5),
        (103, "wash", 9),
        (104, "wash", 13),
        (201, "rgbw", 17),
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
    let colour = |r, g, b| Value::Colour(TargetColour { r, g, b });
    let palettes = [
        (
            1,
            RED,
            Window::Colours,
            global("Colour", colour(1.0, 0.0, 0.0)),
        ),
        (2, PAN_FULL, Window::Positions, BTreeMap::new()),
        (3, FULL, Window::Beams, global("Dimmer", Value::Level(1.0))),
        (4, OFF, Window::Beams, global("Dimmer", Value::Level(0.0))),
    ];
    for (id, number, window, global) in palettes {
        let shared = if number == PAN_FULL {
            BTreeMap::from([(
                "wash".to_string(),
                BTreeMap::from([("Pan".to_string(), Value::Level(1.0))]),
            )])
        } else {
            BTreeMap::new()
        };
        e.add_palette(Palette {
            id,
            number,
            window,
            handle: id as u16,
            legend: String::new(),
            icon: None,
            global,
            shared,
            shared_cells: BTreeMap::new(),
            fixtures: BTreeMap::new(),
        })
        .unwrap();
    }
    e
}

fn global(attribute: &str, value: Value) -> BTreeMap<String, GlobalValue> {
    BTreeMap::from([(attribute.to_string(), GlobalValue::Value(value))])
}

fn rig() -> Engine {
    rig_with(Settings::default())
}

fn select(e: &mut Engine, ids: &[u32]) {
    for &id in ids {
        e.select_fixture(id).unwrap();
    }
}

fn timed(time: f32) -> Timing {
    Timing {
        time: Some(time),
        overlap: None,
    }
}

fn staggered(time: f32, overlap: f32) -> Timing {
    Timing {
        time: Some(time),
        overlap: Some(overlap),
    }
}

fn dmx(e: &Engine, address: u16) -> u8 {
    e.frame(1).and_then(|f| f.channel(address)).unwrap()
}

/// DMX at `address` within one step of `expected` (a fade's halfway point is 127.5).
fn assert_near(e: &Engine, address: u16, expected: u8) {
    let actual = dmx(e, address);
    assert!(
        actual.abs_diff(expected) <= 1,
        "address {address}: expected about {expected}, got {actual}"
    );
}

/// Dimmer addresses of the washes.
fn dimmers(e: &Engine) -> [u8; 4] {
    [dmx(e, 1), dmx(e, 5), dmx(e, 9), dmx(e, 13)]
}

// ---- PAL-13 fades ----------------------------------------------------------------------

#[test]
fn pal_13_a_dimmer_palette_fades_linearly() {
    let mut e = rig();
    select(&mut e, &[101]);
    e.apply_palette(FULL, timed(2.0)).unwrap();
    e.tick(0.0);
    assert_eq!(
        dmx(&e, 1),
        0,
        "the fade starts with the first tick after the apply"
    );
    e.tick(1.0);
    assert_near(&e, 1, 128);
    e.tick(2.0);
    assert_eq!(dmx(&e, 1), 255);
    e.tick(3.0);
    assert_eq!(dmx(&e, 1), 255);
}

#[test]
fn pal_13_colour_fades_in_linear_rgb() {
    let mut e = rig();
    select(&mut e, &[201]);
    e.set_programmer_value(201, "Dimmer", 1.0).unwrap();
    e.tick(0.0); // Colour starts at its default, open white
    e.apply_palette(RED, timed(2.0)).unwrap();
    e.tick(0.5);
    e.tick(1.5);
    assert_eq!(dmx(&e, 18), 255);
    assert_near(&e, 19, 128);
    assert_near(&e, 20, 128);
    e.tick(2.5);
    assert_eq!((dmx(&e, 18), dmx(&e, 19), dmx(&e, 20)), (255, 0, 0));
    assert_eq!(dmx(&e, 21), 0, "the unmentioned White goes to 0 (ADR-005)");
}

#[test]
fn pal_13_ltp_attributes_fade_too() {
    let mut e = rig();
    select(&mut e, &[101]);
    e.apply_palette(PAN_FULL, timed(2.0)).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, 2), 0);
    e.tick(1.0);
    assert_near(&e, 2, 128);
    e.tick(2.0);
    assert_eq!(dmx(&e, 2), 255);
}

#[test]
fn pal_13_the_programmer_holds_the_reference_from_the_start() {
    let mut e = rig();
    select(&mut e, &[101]);
    e.apply_palette(FULL, timed(2.0)).unwrap();
    assert!(matches!(
        e.programmer().stored(101, "Dimmer"),
        Some(Stored::Reference(r)) if r.palette == 3
    ));
    e.tick(0.0);
    e.tick(1.0);
    // A Record halfway stores where the fade is going, not where it is.
    e.record_palette(&Record {
        mask: Some(vec![I]),
        record_by: Some(RecordBy::Channel),
        ..Record::at(Cell::new(Window::Beams, 9))
    })
    .unwrap();
    let recorded = e.palettes().last().unwrap().number;
    assert_eq!(
        e.palette_value(recorded, 101, "Dimmer"),
        Some(Value::Level(1.0))
    );
}

#[test]
fn pal_13_zero_time_snaps() {
    let mut e = rig();
    select(&mut e, &[101]);
    e.apply_palette(FULL, timed(0.0)).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, 1), 255);
}

// ---- PAL-14 overlap ----------------------------------------------------------------------

#[test]
fn pal_14_overlap_staggers_the_fixtures_in_target_order() {
    let mut e = rig();
    select(&mut e, &[101, 102, 103, 104]);
    e.apply_palette(FULL, staggered(2.0, 50.0)).unwrap();
    e.tick(0.0);
    assert_eq!(dimmers(&e), [0, 0, 0, 0]);
    e.tick(1.0);
    assert_near(&e, 1, 128);
    assert_eq!(dmx(&e, 5), 0, "102 starts at 1 s");
    e.tick(2.0);
    assert_eq!(dmx(&e, 1), 255);
    assert_near(&e, 5, 128);
    assert_eq!(dmx(&e, 9), 0);
    e.tick(3.0);
    assert_near(&e, 9, 128);
    assert_eq!(dmx(&e, 13), 0, "104 starts at 3 s");
    e.tick(5.0);
    assert_eq!(dimmers(&e), [255, 255, 255, 255], "the run takes 5 s");
}

#[test]
fn pal_14_the_order_is_the_selection_order() {
    let mut e = rig();
    select(&mut e, &[104, 103, 102, 101]);
    e.apply_palette(FULL, staggered(2.0, 50.0)).unwrap();
    e.tick(0.0);
    e.tick(1.0);
    assert_near(&e, 13, 128);
    assert_eq!(dmx(&e, 1), 0);
}

#[test]
fn pal_14_a_quick_palette_goes_in_user_number_order() {
    let mut e = rig();
    e.apply_palette(FULL, staggered(2.0, 50.0)).unwrap();
    e.tick(0.0);
    e.tick(1.0);
    assert_near(&e, 1, 128);
    assert_eq!(dmx(&e, 5), 0);
    assert_eq!(dmx(&e, 17), 0, "201 is fifth: it starts at 4 s");
    e.tick(5.0);
    assert_near(&e, 17, 128);
}

#[test]
fn pal_14_zero_overlap_waits_for_the_previous_fixture() {
    let mut e = rig();
    select(&mut e, &[101, 102]);
    e.apply_palette(FULL, staggered(1.0, 0.0)).unwrap();
    e.tick(0.0);
    e.tick(1.0);
    assert_eq!(dmx(&e, 1), 255);
    assert_eq!(dmx(&e, 5), 0);
    e.tick(1.5);
    assert_near(&e, 5, 128);
}

#[test]
fn pal_14_overlap_without_a_time_does_nothing() {
    let mut e = rig();
    select(&mut e, &[101, 102, 103, 104]);
    e.apply_palette(FULL, staggered(0.0, 50.0)).unwrap();
    e.tick(0.0);
    assert_eq!(dimmers(&e), [255, 255, 255, 255]);
}

// ---- PAL-15 master time and overlap --------------------------------------------------------

#[test]
fn pal_15_master_time_applies_without_a_typed_time() {
    let mut e = rig_with(Settings {
        palette_time: 2.0,
        ..Settings::default()
    });
    select(&mut e, &[101]);
    e.apply_palette(FULL, Timing::default()).unwrap();
    e.tick(0.0);
    e.tick(1.0);
    assert_near(&e, 1, 128);
}

#[test]
fn pal_15_a_typed_time_overrides_the_master_for_that_apply_only() {
    let mut e = rig_with(Settings {
        palette_time: 4.0,
        ..Settings::default()
    });
    select(&mut e, &[101]);
    e.apply_palette(FULL, timed(0.0)).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, 1), 255, "typed 0 s snaps");
    e.apply_palette(OFF, Timing::default()).unwrap();
    e.tick(1.0);
    e.tick(3.0);
    assert_near(&e, 1, 128);
    assert_eq!(e.settings().palette_time, 4.0);
}

#[test]
fn pal_15_master_overlap_applies_without_a_typed_overlap() {
    let mut e = rig_with(Settings {
        palette_overlap: 50.0,
        ..Settings::default()
    });
    select(&mut e, &[101, 102]);
    e.apply_palette(FULL, timed(2.0)).unwrap();
    e.tick(0.0);
    e.tick(1.0);
    assert_near(&e, 1, 128);
    assert_eq!(dmx(&e, 5), 0);
}

#[test]
fn pal_15_out_of_range_times_and_overlaps_are_refused() {
    let mut e = rig();
    select(&mut e, &[101]);
    for timing in [
        timed(-1.0),
        timed(600.5),
        timed(f32::NAN),
        staggered(2.0, -1.0),
        staggered(2.0, 101.0),
    ] {
        assert!(e.apply_palette(FULL, timing).is_err(), "{timing:?}");
        assert!(
            e.programmer().is_empty(),
            "{timing:?} changed the programmer"
        );
    }
    e.apply_palette(FULL, staggered(600.0, 100.0)).unwrap();
}

#[test]
fn pal_15_set_palette_master_checks_its_range() {
    let mut e = rig();
    e.set_palette_master(Some(3.0), None).unwrap();
    e.set_palette_master(None, Some(25.0)).unwrap();
    assert_eq!(e.settings().palette_time, 3.0);
    assert_eq!(e.settings().palette_overlap, 25.0);
    assert!(e.set_palette_master(Some(700.0), Some(50.0)).is_err());
    assert!(e.set_palette_master(Some(1.0), Some(120.0)).is_err());
    assert_eq!(e.settings().palette_time, 3.0, "a refusal changes nothing");
    assert_eq!(e.settings().palette_overlap, 25.0);
}

#[test]
fn pal_15_the_command_line_uses_the_master_time() {
    let mut e = rig_with(Settings {
        palette_time: 2.0,
        ..Settings::default()
    });
    e.execute_command(&["101", "Palette", "81", "Enter"])
        .unwrap();
    e.tick(0.0);
    e.tick(1.0);
    assert_near(&e, 1, 128);
}

// ---- PAL-16 a fade never blocks ----------------------------------------------------------------

fn halfway(e: &mut Engine) {
    select(e, &[101]);
    e.apply_palette(FULL, timed(2.0)).unwrap();
    e.tick(0.0);
    e.tick(1.0);
    assert_near(e, 1, 128);
}

#[test]
fn pal_16_a_wheel_takes_over_from_the_faded_value() {
    let mut e = rig();
    halfway(&mut e);
    e.turn_wheel("Dimmer", 10, Resolution::Normal).unwrap();
    assert!(matches!(
        e.programmer().stored(101, "Dimmer"),
        Some(Stored::Value(_))
    ));
    e.tick(1.025);
    assert_near(&e, 1, 138); // 0.5 + 10/255
    e.tick(2.0);
    assert_near(&e, 1, 138);
}

#[test]
fn pal_16_another_palette_starts_from_where_the_fade_is() {
    let mut e = rig();
    halfway(&mut e);
    e.apply_palette(OFF, timed(2.0)).unwrap();
    e.tick(1.5);
    assert_near(&e, 1, 128);
    e.tick(2.5);
    assert_near(&e, 1, 64);
    e.tick(3.5);
    assert_eq!(dmx(&e, 1), 0);
}

#[test]
fn pal_16_clear_ends_the_fade() {
    let mut e = rig();
    halfway(&mut e);
    e.clear(); // deselect
    e.clear(); // empty the programmer
    assert!(e.programmer().is_empty());
    e.tick(1.5);
    assert_eq!(dmx(&e, 1), 0);
}

#[test]
fn pal_16_same_actions_same_frames() {
    let run = || {
        let mut e = rig();
        select(&mut e, &[101, 102, 103, 104]);
        e.apply_palette(FULL, staggered(2.0, 50.0)).unwrap();
        (0..=12)
            .map(|i| {
                e.tick(f64::from(i) * 0.4);
                dimmers(&e)
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(run(), run());
}

// ---- PAL-26 (amended): fade progress in the programmer view -----------------------------------

fn fading(e: &Engine, fixture: u32) -> BTreeMap<String, f32> {
    e.programmer_view()
        .contents
        .into_iter()
        .find(|c| c.fixture == fixture)
        .map(|c| c.fading)
        .unwrap_or_default()
}

#[test]
fn pal_26_the_programmer_view_shows_fade_progress() {
    let mut e = rig();
    select(&mut e, &[101, 102]);
    e.apply_palette(FULL, staggered(2.0, 50.0)).unwrap();
    assert_eq!(
        fading(&e, 101),
        BTreeMap::from([("Dimmer".to_string(), 0.0)])
    );
    e.tick(0.0);
    e.tick(1.0);
    let p = fading(&e, 101)["Dimmer"];
    assert!((p - 0.5).abs() < 1e-3, "101 halfway, got {p}");
    assert_eq!(
        fading(&e, 102),
        BTreeMap::from([("Dimmer".to_string(), 0.0)]),
        "102 waits for its overlap turn"
    );
    e.tick(2.0);
    assert!(
        fading(&e, 101).is_empty(),
        "a finished fade leaves the view"
    );
    e.tick(3.0);
    assert!(fading(&e, 102).is_empty());
    let view = e.programmer_view();
    assert!(view.contents.iter().all(|c| c.fading.is_empty()));
}
