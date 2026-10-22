//! Palettes, core part (VS2-17): `specs/modules/palettes.md` PAL-01..12, 17..21, 23, 24, 32.
//! Fades and overlap (PAL-13..16) are VS2-18; IPC and the app (PAL-25..31) are VS2-19.
//! Frame-level PAL-07 (emitter zeroing) and PAL-11 (live references) are also scenarios in
//! `tests/scenarios/`.
//!
//! API these tests fix (new module `cmdmx_core::palette`, plus `Stored`/`Reference` in `value`):
//! - `Engine::record_palette(&Record) -> Result<PaletteId, CoreError>`
//! - `Engine::apply_palette(number, Timing)`, `include_palette(number)`
//! - `Engine::copy_palette(number, Cell) -> Result<PaletteId, _>`, `move_palette(number, Cell)`,
//!   `renumber_palette(number, to)`, `delete_palette(number)`,
//!   `set_palette_legend(number, legend, icon)`, `add_palette(Palette)` (tests, scenarios)
//! - `Engine::palette(number)`, `palettes()` (by id), `palette_value(number, fixture, attribute)`,
//!   `palette_auto_icon(number)`, `playback(id)`; `Programmer::stored(fixture, attribute)`
//! - Playback and `contents` values are `Stored` (a value or a reference with its fallback).

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;

use cmdmx_core::attribute::AttributeGroup::{B, C, I, P};
use cmdmx_core::colour::TargetColour;
use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::palette::{
    AutoIcon, Cell, GlobalValue, Palette, PaletteId, PaletteKind, Record, RecordAs, RecordBy,
    RecordMode, Timing, Window,
};
use cmdmx_core::patch::Fixture;
use cmdmx_core::playback::Playback;
use cmdmx_core::profile::Profile;
use cmdmx_core::programmer::Resolution;
use cmdmx_core::show::{Meta, Show};
use cmdmx_core::value::{Reference, Stored, Value};

// ---- rig ---------------------------------------------------------------------

const RGBW: &str = r#"{ "manufacturer": "T", "model": "RGBW Par", "source": "channel-list", "modes": [ {
  "name": "5ch", "footprint": 5,
  "colour": { "system": "RGB", "components": [ { "channel": 2 }, { "channel": 3 }, { "channel": 4 } ] },
  "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "White", "group": "C", "kind": "Ltp", "channel": 5, "emitter": true } ] } ] }"#;

/// RGB+Lime, no dimmer channel.
const RGBL: &str = r#"{ "manufacturer": "T", "model": "RGBL Par", "source": "channel-list", "modes": [ {
  "name": "4ch", "footprint": 4, "virtual_dimmer": true,
  "colour": { "system": "RGB", "components": [ { "channel": 1 }, { "channel": 2 }, { "channel": 3 } ] },
  "attributes": [ { "name": "Lime", "group": "C", "kind": "Ltp", "channel": 4, "emitter": true } ] } ] }"#;

/// Plain RGB, no dimmer channel.
const RGB: &str = r#"{ "manufacturer": "T", "model": "RGB Par", "source": "channel-list", "modes": [ {
  "name": "3ch", "footprint": 3, "virtual_dimmer": true,
  "colour": { "system": "RGB", "components": [ { "channel": 1 }, { "channel": 2 }, { "channel": 3 } ] } } ] }"#;

/// Moving-head wash: 540° pan, 270° tilt.
const WASH: &str = r#"{ "manufacturer": "T", "model": "Wash", "source": "channel-list", "modes": [ {
  "name": "4ch", "footprint": 4, "pan_degrees": 540, "tilt_degrees": 270,
  "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 2 },
    { "name": "Tilt", "group": "P", "kind": "Ltp", "channel": 3 },
    { "name": "Zoom", "group": "B", "kind": "Ltp", "channel": 4 } ] } ] }"#;

/// A second head type: 360° pan, 180° tilt.
const HEAD: &str = r#"{ "manufacturer": "T", "model": "Head", "source": "channel-list", "modes": [ {
  "name": "3ch", "footprint": 3, "pan_degrees": 360, "tilt_degrees": 180,
  "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 2 },
    { "name": "Tilt", "group": "P", "kind": "Ltp", "channel": 3 } ] } ] }"#;

/// A head without physical ranges.
const SPOT: &str = r#"{ "manufacturer": "T", "model": "Spot", "source": "channel-list", "modes": [ {
  "name": "2ch", "footprint": 2,
  "attributes": [
    { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 1 },
    { "name": "Tilt", "group": "P", "kind": "Ltp", "channel": 2 } ] } ] }"#;

const DIMMER: &str =
    r#"{ "attributes": [ { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 } ] }"#;

/// Universe 1: 201 @1, 202 @6 (RGBW); 211 @11, 212 @15 (RGBL); 221 @19 (RGB);
/// washes 101 @22, 102 @26; head 111 @30; spot 121 @33; dimmer 301 @35.
fn rig_with(settings: Settings) -> Engine {
    let mut e = Engine::new(settings);
    for (key, json) in [
        ("rgbw", RGBW),
        ("rgbl", RGBL),
        ("rgb", RGB),
        ("wash", WASH),
        ("head", HEAD),
        ("spot", SPOT),
        ("dimmer", DIMMER),
    ] {
        e.add_profile(key, Profile::from_json(json).unwrap())
            .unwrap();
    }
    for (id, profile, address) in [
        (201, "rgbw", 1),
        (202, "rgbw", 6),
        (211, "rgbl", 11),
        (212, "rgbl", 15),
        (221, "rgb", 19),
        (101, "wash", 22),
        (102, "wash", 26),
        (111, "head", 30),
        (121, "spot", 33),
        (301, "dimmer", 35),
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

fn rig() -> Engine {
    rig_with(Settings::default())
}

fn rgb(r: f32, g: f32, b: f32) -> TargetColour {
    TargetColour { r, g, b }
}

fn red() -> Value {
    Value::Colour(rgb(1.0, 0.0, 0.0))
}

fn blue() -> Value {
    Value::Colour(rgb(0.0, 0.0, 1.0))
}

fn select(e: &mut Engine, ids: &[u32]) {
    for &id in ids {
        e.select_fixture(id).unwrap();
    }
}

fn set(e: &mut Engine, ids: &[u32], attribute: &str, value: impl Into<Value> + Copy) {
    for &id in ids {
        e.set_programmer_value(id, attribute, value).unwrap();
    }
}

fn cell(window: Window, handle: u16) -> Cell {
    Cell::new(window, handle)
}

fn record(e: &mut Engine, window: Window, handle: u16) -> PaletteId {
    e.record_palette(&Record::at(cell(window, handle))).unwrap()
}

fn channel_record(e: &mut Engine, window: Window, handle: u16) -> PaletteId {
    e.record_palette(&Record {
        record_by: Some(RecordBy::Channel),
        ..Record::at(cell(window, handle))
    })
    .unwrap()
}

fn level(v: f32) -> Value {
    Value::Level(v)
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

fn assert_level(actual: Option<Value>, expected: f32) {
    match actual {
        Some(Value::Level(v)) if close(v, expected) => {}
        other => panic!("expected level {expected}, got {other:?}"),
    }
}

fn dmx(e: &Engine, address: u16) -> u8 {
    e.frame(1).and_then(|f| f.channel(address)).unwrap()
}

fn tick(e: &mut Engine, t: f64) {
    e.tick(t);
}

fn reference(e: &Engine, fixture: u32, attribute: &str) -> Reference {
    match e.programmer().stored(fixture, attribute) {
        Some(Stored::Reference(r)) => r,
        other => panic!("{fixture} {attribute}: expected a palette reference, got {other:?}"),
    }
}

fn number_of(e: &Engine, id: PaletteId) -> u32 {
    e.palettes().find(|p| p.id == id).unwrap().number
}

/// Palette with only a global level, placed directly (as a show or scenario would).
fn global_palette(id: PaletteId, number: u32, values: &[(&str, GlobalValue)]) -> Palette {
    Palette {
        id,
        number,
        window: Window::Colours,
        handle: number as u16,
        legend: String::new(),
        icon: None,
        global: values
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect(),
        shared: BTreeMap::new(),
        fixtures: BTreeMap::new(),
    }
}

fn g(v: Value) -> GlobalValue {
    GlobalValue::Value(v)
}

fn meta() -> Meta {
    Meta {
        name: "Palettes".into(),
        created: "2026-10-18T09:00:00Z".into(),
        modified: "2026-10-18T09:00:00Z".into(),
        app_version: "0.1.0".into(),
    }
}

// ---- PAL-01 recording onto an empty cell ---------------------------------------

#[test]
fn pal_01_records_the_programmer_in_the_windows_mask() {
    let mut e = rig();
    select(&mut e, &[201]);
    set(&mut e, &[201], "Dimmer", 0.8);
    set(&mut e, &[201], "Colour", rgb(1.0, 0.0, 0.0));
    let id = record(&mut e, Window::Colours, 1);
    let p = e.palette(1).expect("palette 1");
    assert_eq!(p.id, id);
    assert_eq!((p.window, p.handle), (Window::Colours, 1));
    assert_eq!(e.palette_value(1, 201, "Colour"), Some(red()));
    assert_eq!(
        e.palette_value(1, 201, "Dimmer"),
        None,
        "Dimmer is outside mask C"
    );
}

#[test]
fn pal_01_records_every_fixture_in_the_programmer_not_only_the_selection() {
    let mut e = rig();
    select(&mut e, &[201]);
    set(&mut e, &[201], "Colour", rgb(1.0, 0.0, 0.0));
    select(&mut e, &[202]); // PRG-03: a new selection, 201 stays in the programmer
    assert_eq!(e.programmer().selection(), &[202]);
    set(&mut e, &[202], "Colour", rgb(0.0, 0.0, 1.0));
    record(&mut e, Window::Colours, 1);
    assert_eq!(e.palette_value(1, 201, "Colour"), Some(red()));
    assert_eq!(e.palette_value(1, 202, "Colour"), Some(blue()));
}

#[test]
fn pal_01_next_free_number_or_a_free_number_given() {
    let mut e = rig();
    set(&mut e, &[201], "Colour", rgb(1.0, 0.0, 0.0));
    record(&mut e, Window::Colours, 1);
    e.record_palette(&Record {
        number: Some(10),
        ..Record::at(cell(Window::Colours, 2))
    })
    .unwrap();
    record(&mut e, Window::Colours, 3);
    assert_eq!(e.palette(1).unwrap().handle, 1);
    assert_eq!(e.palette(10).unwrap().handle, 2);
    assert_eq!(e.palette(2).unwrap().handle, 3);

    let before: Vec<Palette> = e.palettes().cloned().collect();
    assert!(e
        .record_palette(&Record {
            number: Some(10),
            ..Record::at(cell(Window::Colours, 4))
        })
        .is_err());
    assert!(e
        .record_palette(&Record {
            number: Some(0),
            ..Record::at(cell(Window::Colours, 4))
        })
        .is_err());
    assert_eq!(e.palettes().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn pal_01_nothing_in_the_mask_is_refused() {
    let mut e = rig();
    set(&mut e, &[201], "Dimmer", 0.8);
    assert!(e
        .record_palette(&Record::at(cell(Window::Colours, 1)))
        .is_err());
    assert!(e
        .record_palette(&Record::at(cell(Window::Colours, 0)))
        .is_err());
    assert_eq!(e.palettes().count(), 0);
}

#[test]
fn pal_01_the_window_only_gives_the_default_mask() {
    // Owner decision 6: what a palette is comes from its values, not from its window.
    let mut e = rig();
    set(&mut e, &[201], "Colour", rgb(1.0, 0.0, 0.0));
    e.record_palette(&Record {
        mask: Some(vec![C]),
        ..Record::at(cell(Window::Positions, 1))
    })
    .unwrap();
    assert_eq!(e.palette(1).unwrap().window, Window::Positions);
    assert_eq!(e.palette_value(1, 201, "Colour"), Some(red()));
}

// ---- PAL-02 Record By ------------------------------------------------------------

#[test]
fn pal_02_mixed_is_the_default_and_stores_the_whole_position() {
    let mut e = rig();
    assert_eq!(Settings::default().record_by, RecordBy::Mixed);
    set(&mut e, &[101], "Pan", 0.25);
    tick(&mut e, 0.0);
    record(&mut e, Window::Positions, 1);
    assert_level(e.palette_value(1, 101, "Pan"), 0.25);
    assert!(
        e.palette_value(1, 101, "Tilt").is_some(),
        "Mixed stores the untouched Tilt at its output value"
    );
}

#[test]
fn pal_02_channel_stores_only_what_was_touched() {
    let mut e = rig();
    set(&mut e, &[101], "Pan", 0.25);
    channel_record(&mut e, Window::Positions, 1);
    assert_level(e.palette_value(1, 101, "Pan"), 0.25);
    assert_eq!(e.palette_value(1, 101, "Tilt"), None);
}

#[test]
fn pal_02_mixed_colour_stores_the_emitters_too() {
    let mut e = rig();
    set(&mut e, &[201], "Colour", rgb(1.0, 0.0, 0.0));
    tick(&mut e, 0.0);
    record(&mut e, Window::Colours, 1);
    assert_eq!(e.palette_value(1, 201, "Colour"), Some(red()));
    assert_level(e.palette_value(1, 201, "White"), 0.0);
}

#[test]
fn pal_02_the_setting_changes_the_default() {
    let mut e = rig_with(Settings {
        record_by: RecordBy::Channel,
        ..Settings::default()
    });
    set(&mut e, &[101], "Pan", 0.25);
    record(&mut e, Window::Positions, 1);
    assert_eq!(e.palette_value(1, 101, "Tilt"), None);
}

// ---- PAL-03 references are recorded as values -------------------------------------

#[test]
fn pal_03_a_reference_in_the_programmer_is_stored_as_its_value() {
    let mut e = rig();
    e.add_palette(global_palette(7, 1, &[("Colour", g(red()))]))
        .unwrap();
    select(&mut e, &[202]);
    e.apply_palette(1, Timing::default()).unwrap();
    channel_record(&mut e, Window::Colours, 2);
    let p = e.palette(2).unwrap();
    assert_eq!(p.global.get("Colour"), Some(&g(red())));
}

// ---- PAL-04 kind detection ----------------------------------------------------------

#[test]
fn pal_04_same_colour_on_every_type_is_global() {
    let mut e = rig();
    set(
        &mut e,
        &[201, 202, 211, 212, 221],
        "Colour",
        rgb(1.0, 0.0, 0.0),
    );
    channel_record(&mut e, Window::Colours, 1);
    let p = e.palette(1).unwrap();
    assert_eq!(p.kind(), PaletteKind::Global);
    assert_eq!(p.global, BTreeMap::from([("Colour".to_string(), g(red()))]));
    assert!(p.shared.is_empty() && p.fixtures.is_empty());
}

#[test]
fn pal_04_emitters_stay_shared_next_to_a_global_colour() {
    let mut e = rig();
    set(&mut e, &[201, 202], "Colour", rgb(1.0, 0.0, 0.0));
    set(&mut e, &[201, 202], "White", 0.3);
    channel_record(&mut e, Window::Colours, 1);
    let p = e.palette(1).unwrap();
    assert_eq!(p.kind(), PaletteKind::Global);
    assert_eq!(p.global.get("Colour"), Some(&g(red())));
    assert_eq!(
        p.shared.get("rgbw"),
        Some(&BTreeMap::from([("White".to_string(), level(0.3))]))
    );
}

#[test]
fn pal_04_different_positions_are_per_fixture() {
    let mut e = rig();
    set(&mut e, &[101], "Pan", 0.2);
    set(&mut e, &[102], "Pan", 0.7);
    channel_record(&mut e, Window::Positions, 1);
    let p = e.palette(1).unwrap();
    assert_eq!(p.kind(), PaletteKind::Normal);
    assert!(p.global.is_empty() && p.shared.is_empty());
    assert_eq!(p.fixtures.keys().copied().collect::<Vec<_>>(), [101, 102]);
}

#[test]
fn pal_04_same_zoom_on_one_type_is_shared() {
    let mut e = rig();
    set(&mut e, &[101, 102], "Zoom", 0.4);
    channel_record(&mut e, Window::Beams, 1);
    let p = e.palette(1).unwrap();
    assert_eq!(p.kind(), PaletteKind::Shared);
    assert_eq!(
        p.shared.get("wash"),
        Some(&BTreeMap::from([("Zoom".to_string(), level(0.4))]))
    );
}

#[test]
fn pal_04_one_fixture_counts_as_shared() {
    let mut e = rig();
    set(&mut e, &[101], "Zoom", 0.4);
    channel_record(&mut e, Window::Beams, 1);
    assert_eq!(e.palette(1).unwrap().kind(), PaletteKind::Shared);
}

#[test]
fn pal_04_intensity_can_be_global() {
    let mut e = rig();
    set(&mut e, &[201, 301], "Dimmer", 0.5);
    e.record_palette(&Record {
        mask: Some(vec![I]),
        record_by: Some(RecordBy::Channel),
        ..Record::at(cell(Window::Beams, 1))
    })
    .unwrap();
    let p = e.palette(1).unwrap();
    assert_eq!(p.global.get("Dimmer"), Some(&g(level(0.5))));
}

#[test]
fn pal_04_values_are_compared_at_16_bit() {
    let mut e = rig();
    set(&mut e, &[201], "Colour", rgb(1.0, 0.0, 0.0));
    set(&mut e, &[202], "Colour", rgb(1.0, 0.000_001, 0.0));
    channel_record(&mut e, Window::Colours, 1);
    assert_eq!(e.palette(1).unwrap().kind(), PaletteKind::Global);
}

#[test]
fn pal_04_straight_down_on_two_head_types_is_a_global_position_in_degrees() {
    let mut e = rig();
    // Tilt −90°: wash (270°) at 0.5 − 90/270, head (180°) at 0.5 − 90/180; Pan centred.
    set(&mut e, &[101, 102, 111], "Pan", 0.5);
    set(&mut e, &[101, 102], "Tilt", 0.5 - 90.0 / 270.0);
    set(&mut e, &[111], "Tilt", 0.0);
    channel_record(&mut e, Window::Positions, 1);
    let p = e.palette(1).unwrap();
    assert_eq!(p.kind(), PaletteKind::Global);
    match (p.global.get("Pan"), p.global.get("Tilt")) {
        (
            Some(GlobalValue::Degrees { degrees: pan }),
            Some(GlobalValue::Degrees { degrees: tilt }),
        ) => {
            assert!(pan.abs() < 0.01, "pan {pan}");
            assert!((tilt + 90.0).abs() < 0.01, "tilt {tilt}");
        }
        other => panic!("expected global Pan/Tilt in degrees, got {other:?}"),
    }
}

#[test]
fn pal_04_pan_tilt_without_a_physical_range_stays_shared() {
    let mut e = rig();
    set(&mut e, &[101, 121], "Pan", 0.5);
    set(&mut e, &[101, 121], "Tilt", 0.5);
    channel_record(&mut e, Window::Positions, 1);
    let p = e.palette(1).unwrap();
    assert_eq!(p.kind(), PaletteKind::Shared);
    assert!(p.global.is_empty());
    assert_eq!(
        p.shared.keys().map(String::as_str).collect::<Vec<_>>(),
        ["spot", "wash"]
    );
}

// ---- PAL-05 Record as ---------------------------------------------------------------

#[test]
fn pal_05_record_as_normal_stores_per_fixture() {
    let mut e = rig();
    set(&mut e, &[201, 202, 221], "Colour", rgb(1.0, 0.0, 0.0));
    e.record_palette(&Record {
        record_by: Some(RecordBy::Channel),
        record_as: RecordAs::Normal,
        ..Record::at(cell(Window::Colours, 1))
    })
    .unwrap();
    let p = e.palette(1).unwrap();
    assert_eq!(p.kind(), PaletteKind::Normal);
    assert_eq!(
        p.fixtures.keys().copied().collect::<Vec<_>>(),
        [201, 202, 221]
    );
}

#[test]
fn pal_05_record_as_shared_keeps_the_global_level_empty() {
    let mut e = rig();
    set(&mut e, &[201, 202, 221], "Colour", rgb(1.0, 0.0, 0.0));
    e.record_palette(&Record {
        record_by: Some(RecordBy::Channel),
        record_as: RecordAs::Shared,
        ..Record::at(cell(Window::Colours, 1))
    })
    .unwrap();
    let p = e.palette(1).unwrap();
    assert_eq!(p.kind(), PaletteKind::Shared);
    assert!(p.global.is_empty());
    assert_eq!(
        p.shared.keys().map(String::as_str).collect::<Vec<_>>(),
        ["rgb", "rgbw"]
    );
}

#[test]
fn pal_05_record_as_shared_with_differing_values_is_refused() {
    let mut e = rig();
    set(&mut e, &[201], "Colour", rgb(1.0, 0.0, 0.0));
    set(&mut e, &[202], "Colour", rgb(0.0, 0.0, 1.0));
    assert!(e
        .record_palette(&Record {
            record_by: Some(RecordBy::Channel),
            record_as: RecordAs::Shared,
            ..Record::at(cell(Window::Colours, 1))
        })
        .is_err());
    assert_eq!(e.palettes().count(), 0);
}

// ---- PAL-06 recording onto a used cell ----------------------------------------------

fn red_on_rgbw_shared(e: &mut Engine) {
    set(e, &[201, 202], "Colour", rgb(1.0, 0.0, 0.0));
    e.record_palette(&Record {
        record_by: Some(RecordBy::Channel),
        record_as: RecordAs::Shared,
        ..Record::at(cell(Window::Colours, 1))
    })
    .unwrap();
    e.clear(); // SelectionThenProgrammer: nothing selected, so this empties the programmer
    assert!(e.programmer().is_empty());
}

#[test]
fn pal_06_a_used_cell_needs_a_mode() {
    let mut e = rig();
    red_on_rgbw_shared(&mut e);
    let before = e.palette(1).unwrap().clone();
    set(&mut e, &[211], "Colour", rgb(0.0, 0.0, 1.0));
    assert!(e
        .record_palette(&Record::at(cell(Window::Colours, 1)))
        .is_err());
    assert_eq!(e.palette(1).unwrap(), &before);
}

#[test]
fn pal_06_replace_keeps_id_number_cell_legend_and_icon() {
    let mut e = rig();
    red_on_rgbw_shared(&mut e);
    e.set_palette_legend(1, "Warm", Some("colour.drop"))
        .unwrap();
    let id = e.palette(1).unwrap().id;
    set(&mut e, &[211], "Colour", rgb(0.0, 0.0, 1.0));
    e.record_palette(&Record {
        record_by: Some(RecordBy::Channel),
        mode: Some(RecordMode::Replace),
        ..Record::at(cell(Window::Colours, 1))
    })
    .unwrap();
    let p = e.palette(1).unwrap();
    assert_eq!((p.id, p.handle), (id, 1));
    assert_eq!(p.legend, "Warm");
    assert_eq!(p.icon.as_deref(), Some("colour.drop"));
    assert_eq!(e.palette_value(1, 211, "Colour"), Some(blue()));
    assert!(
        !p.shared.contains_key("rgbw"),
        "Replace drops the old contents"
    );
}

#[test]
fn pal_06_merge_adds_a_type_and_never_widens_a_shared_palette() {
    let mut e = rig();
    red_on_rgbw_shared(&mut e);
    let orange = rgb(1.0, 0.3, 0.0);
    set(&mut e, &[211, 212], "Colour", orange);
    e.record_palette(&Record {
        record_by: Some(RecordBy::Channel),
        mode: Some(RecordMode::Merge),
        ..Record::at(cell(Window::Colours, 1))
    })
    .unwrap();
    let p = e.palette(1).unwrap();
    assert_eq!(p.kind(), PaletteKind::Shared);
    assert!(p.global.is_empty());
    assert_eq!(
        p.shared.get("rgbl"),
        Some(&BTreeMap::from([(
            "Colour".to_string(),
            Value::Colour(orange)
        )]))
    );
    assert_eq!(
        p.shared.get("rgbw"),
        Some(&BTreeMap::from([("Colour".to_string(), red())]))
    );
}

#[test]
fn pal_06_quick_merge_takes_only_attributes_the_palette_holds() {
    let mut e = rig();
    set(&mut e, &[101], "Pan", 0.2);
    channel_record(&mut e, Window::Positions, 1);
    e.clear();
    set(&mut e, &[101], "Pan", 0.9);
    set(&mut e, &[101], "Zoom", 0.5);
    e.record_palette(&Record {
        mask: Some(vec![P, B]),
        record_by: Some(RecordBy::Channel),
        mode: Some(RecordMode::QuickMerge),
        ..Record::at(cell(Window::Positions, 1))
    })
    .unwrap();
    assert_level(e.palette_value(1, 101, "Pan"), 0.9);
    assert_eq!(e.palette_value(1, 101, "Zoom"), None);
}

// ---- PAL-07 resolving ----------------------------------------------------------------

#[test]
fn pal_07_per_fixture_beats_shared_beats_global() {
    let mut e = rig();
    let green = Value::Colour(rgb(0.0, 1.0, 0.0));
    let mut p = global_palette(1, 1, &[("Colour", g(red()))]);
    p.shared.insert(
        "rgbw".into(),
        BTreeMap::from([("Colour".to_string(), blue())]),
    );
    p.fixtures
        .insert(202, BTreeMap::from([("Colour".to_string(), green)]));
    e.add_palette(p).unwrap();
    assert_eq!(e.palette_value(1, 202, "Colour"), Some(green));
    assert_eq!(e.palette_value(1, 201, "Colour"), Some(blue()));
    assert_eq!(e.palette_value(1, 221, "Colour"), Some(red()));
}

#[test]
fn pal_07_a_colour_zeroes_the_emitters_it_does_not_mention() {
    let mut e = rig();
    e.add_palette(global_palette(1, 1, &[("Colour", g(red()))]))
        .unwrap();
    assert_eq!(e.palette_value(1, 221, "Colour"), Some(red()));
    assert_eq!(e.palette_value(1, 211, "Colour"), Some(red()));
    assert_level(e.palette_value(1, 211, "Lime"), 0.0);
    assert_level(e.palette_value(1, 201, "White"), 0.0);
    assert_eq!(e.palette_value(1, 301, "Colour"), None);
    assert_eq!(e.palette_value(1, 301, "Dimmer"), None);
}

#[test]
fn pal_07_emitter_only_palette_leaves_colour_and_other_emitters_alone() {
    let mut e = rig();
    let mut p = global_palette(1, 1, &[]);
    p.shared.insert(
        "rgbw".into(),
        BTreeMap::from([("White".to_string(), level(1.0))]),
    );
    e.add_palette(p).unwrap();
    assert_level(e.palette_value(1, 201, "White"), 1.0);
    assert_eq!(e.palette_value(1, 201, "Colour"), None);
    assert_eq!(e.palette_value(1, 211, "Lime"), None);
}

#[test]
fn pal_07_global_degrees_use_each_fixtures_own_range() {
    let mut e = rig();
    e.add_palette(global_palette(
        1,
        41,
        &[("Tilt", GlobalValue::Degrees { degrees: -90.0 })],
    ))
    .unwrap();
    assert_level(e.palette_value(41, 101, "Tilt"), 0.5 - 90.0 / 270.0);
    assert_level(e.palette_value(41, 111, "Tilt"), 0.0);
    assert_eq!(
        e.palette_value(41, 121, "Tilt"),
        None,
        "no physical range, no global degrees"
    );
}

// ---- PAL-08 applying -----------------------------------------------------------------

#[test]
fn pal_08_apply_puts_references_with_their_fallback() {
    let mut e = rig();
    let id = 5;
    e.add_palette(global_palette(id, 1, &[("Colour", g(red()))]))
        .unwrap();
    select(&mut e, &[201, 301]);
    e.apply_palette(1, Timing::default()).unwrap();
    assert_eq!(
        reference(&e, 201, "Colour"),
        Reference {
            palette: id,
            value: red()
        }
    );
    assert_eq!(
        reference(&e, 201, "White"),
        Reference {
            palette: id,
            value: level(0.0)
        }
    );
    assert!(
        !e.programmer().in_programmer(301),
        "nothing resolves for the spot"
    );
}

#[test]
fn pal_08_other_attributes_stay_as_they_are() {
    let mut e = rig();
    e.add_palette(global_palette(1, 1, &[("Colour", g(red()))]))
        .unwrap();
    select(&mut e, &[201]);
    set(&mut e, &[201], "Dimmer", 0.5);
    e.apply_palette(1, Timing::default()).unwrap();
    assert_eq!(
        e.programmer().stored(201, "Dimmer"),
        Some(Stored::Value(level(0.5)))
    );
}

#[test]
fn pal_08_nothing_fits_is_refused() {
    let mut e = rig();
    e.add_palette(global_palette(1, 1, &[("Colour", g(red()))]))
        .unwrap();
    select(&mut e, &[301]);
    assert!(e.apply_palette(1, Timing::default()).is_err());
    assert!(e.programmer().is_empty());
    assert!(e.apply_palette(99, Timing::default()).is_err());
}

#[test]
fn pal_08_applying_counts_as_a_programmer_change() {
    let mut e = rig();
    e.add_palette(global_palette(1, 1, &[("Colour", g(red()))]))
        .unwrap();
    select(&mut e, &[201]);
    e.apply_palette(1, Timing::default()).unwrap();
    select(&mut e, &[202]);
    assert_eq!(e.programmer().selection(), &[202]);
}

// ---- PAL-09 quick palette ------------------------------------------------------------

#[test]
fn pal_09_no_selection_applies_to_every_fixture_it_fits() {
    let mut e = rig();
    e.add_palette(global_palette(1, 1, &[("Colour", g(red()))]))
        .unwrap();
    e.apply_palette(1, Timing::default()).unwrap();
    let in_programmer: Vec<u32> = e.programmer().values().keys().copied().collect();
    assert_eq!(in_programmer, [201, 202, 211, 212, 221]);
    assert!(e.programmer().selection().is_empty());
}

// ---- PAL-10 command line -------------------------------------------------------------

#[test]
fn pal_10_palette_n_enter_applies_to_the_targets() {
    let mut e = rig();
    e.add_palette(global_palette(1, 3, &[("Colour", g(red()))]))
        .unwrap();
    select(&mut e, &[201]);
    e.execute_command(&["Palette", "3", "Enter"]).unwrap();
    assert_eq!(reference(&e, 201, "Colour").palette, 1);
}

#[test]
fn pal_10_with_a_typed_selection() {
    let mut e = rig();
    e.add_palette(global_palette(1, 3, &[("Colour", g(red()))]))
        .unwrap();
    e.execute_command(&["202", "Palette", "3", "Enter"])
        .unwrap();
    assert_eq!(e.programmer().selection(), &[202]);
    assert_eq!(reference(&e, 202, "Colour").palette, 1);
    assert!(!e.programmer().in_programmer(201));
}

#[test]
fn pal_10_unknown_or_missing_number_is_refused() {
    let mut e = rig();
    select(&mut e, &[201]);
    assert!(e.execute_command(&["Palette", "9", "Enter"]).is_err());
    assert!(e.execute_command(&["Palette", "Enter"]).is_err());
    assert!(e.programmer().is_empty());
}

// ---- PAL-11 references are live -------------------------------------------------------

fn playback_with_reference(e: &mut Engine, palette: PaletteId, fallback: Value) {
    e.add_playback(Playback::new(
        1,
        1,
        1,
        BTreeMap::from([(
            201,
            BTreeMap::from([
                ("Dimmer".to_string(), Stored::Value(level(1.0))),
                (
                    "Colour".to_string(),
                    Stored::Reference(Reference {
                        palette,
                        value: fallback,
                    }),
                ),
            ]),
        )]),
    ));
    e.set_fader(1, 1.0, 0.0).unwrap();
}

#[test]
fn pal_11_a_recorded_palette_changes_a_playback_on_the_next_tick() {
    let mut e = rig();
    set(&mut e, &[201, 202], "Colour", rgb(1.0, 0.0, 0.0));
    let id = channel_record(&mut e, Window::Colours, 1);
    e.clear();
    playback_with_reference(&mut e, id, red());
    tick(&mut e, 0.0);
    assert_eq!((dmx(&e, 2), dmx(&e, 4)), (255, 0));

    set(&mut e, &[201, 202], "Colour", rgb(0.0, 0.0, 1.0));
    e.record_palette(&Record {
        record_by: Some(RecordBy::Channel),
        mode: Some(RecordMode::Replace),
        ..Record::at(cell(Window::Colours, 1))
    })
    .unwrap();
    e.clear();
    tick(&mut e, 0.025);
    assert_eq!((dmx(&e, 2), dmx(&e, 4)), (0, 255), "blue, without a fade");
}

#[test]
fn pal_11_a_reference_that_resolves_nothing_uses_its_fallback() {
    let mut e = rig();
    let mut p = global_palette(1, 1, &[]);
    p.fixtures
        .insert(202, BTreeMap::from([("Colour".to_string(), blue())]));
    e.add_palette(p).unwrap();
    playback_with_reference(&mut e, 1, red());
    tick(&mut e, 0.0);
    assert_eq!((dmx(&e, 2), dmx(&e, 4)), (255, 0));
}

#[test]
fn pal_11_programmer_references_resolve_in_the_merge() {
    let mut e = rig();
    set(&mut e, &[202], "Colour", rgb(1.0, 0.0, 0.0));
    channel_record(&mut e, Window::Colours, 1);
    e.clear();
    select(&mut e, &[201]);
    set(&mut e, &[201], "Dimmer", 1.0);
    e.apply_palette(1, Timing::default()).unwrap();
    tick(&mut e, 0.0);
    assert_eq!((dmx(&e, 2), dmx(&e, 4)), (255, 0));
    // References use the id: renumbering or moving the palette changes nothing on stage.
    e.renumber_palette(1, 12).unwrap();
    e.move_palette(12, cell(Window::Beams, 4)).unwrap();
    tick(&mut e, 0.025);
    assert_eq!((dmx(&e, 2), dmx(&e, 4)), (255, 0));
}

// ---- PAL-12 Include and PRG-22 --------------------------------------------------------

#[test]
fn pal_12_include_loads_values_not_references() {
    let mut e = rig();
    e.add_palette(global_palette(1, 1, &[("Colour", g(red()))]))
        .unwrap();
    select(&mut e, &[201]);
    e.include_palette(1).unwrap();
    assert_eq!(
        e.programmer().stored(201, "Colour"),
        Some(Stored::Value(red()))
    );
    assert_eq!(
        e.programmer().stored(201, "White"),
        Some(Stored::Value(level(0.0)))
    );
}

#[test]
fn pal_12_a_wheel_makes_only_that_attribute_manual() {
    let mut e = rig();
    e.add_palette(global_palette(1, 1, &[("Colour", g(red()))]))
        .unwrap();
    select(&mut e, &[201]);
    e.apply_palette(1, Timing::default()).unwrap();
    e.turn_wheel("White", 10, Resolution::Normal).unwrap();
    assert!(matches!(
        e.programmer().stored(201, "White"),
        Some(Stored::Value(_))
    ));
    assert_eq!(reference(&e, 201, "Colour").palette, 1);
}

// ---- PAL-17 copy, move, renumber -------------------------------------------------------

#[test]
fn pal_17_copy_makes_an_independent_palette() {
    let mut e = rig();
    set(&mut e, &[201], "Colour", rgb(1.0, 0.0, 0.0));
    let id = channel_record(&mut e, Window::Colours, 1);
    e.set_palette_legend(1, "Warm", None).unwrap();
    select(&mut e, &[201]);
    e.apply_palette(1, Timing::default()).unwrap();
    let copy = e.copy_palette(1, cell(Window::Positions, 3)).unwrap();
    assert_ne!(copy, id);
    assert_eq!(number_of(&e, copy), 2);
    let p = e.palette(2).unwrap();
    assert_eq!((p.window, p.handle), (Window::Positions, 3));
    assert_eq!(p.legend, "Warm");
    assert_eq!(p.global, e.palette(1).unwrap().global);
    assert_eq!(
        reference(&e, 201, "Colour").palette,
        id,
        "references keep the original"
    );
}

#[test]
fn pal_17_move_changes_only_the_cell() {
    let mut e = rig();
    set(&mut e, &[201], "Colour", rgb(1.0, 0.0, 0.0));
    let id = channel_record(&mut e, Window::Colours, 1);
    e.move_palette(1, cell(Window::Beams, 8)).unwrap();
    let p = e.palette(1).unwrap();
    assert_eq!((p.id, p.window, p.handle), (id, Window::Beams, 8));
}

#[test]
fn pal_17_renumber_keeps_references() {
    let mut e = rig();
    set(&mut e, &[201], "Colour", rgb(1.0, 0.0, 0.0));
    let id = channel_record(&mut e, Window::Colours, 1);
    select(&mut e, &[201]);
    e.apply_palette(1, Timing::default()).unwrap();
    e.renumber_palette(1, 30).unwrap();
    assert!(e.palette(1).is_none());
    assert_eq!(e.palette(30).unwrap().id, id);
    assert_eq!(reference(&e, 201, "Colour").palette, id);
}

#[test]
fn pal_17_used_cells_and_bad_numbers_are_refused() {
    let mut e = rig();
    set(&mut e, &[201], "Colour", rgb(1.0, 0.0, 0.0));
    channel_record(&mut e, Window::Colours, 1);
    channel_record(&mut e, Window::Colours, 2);
    let before: Vec<Palette> = e.palettes().cloned().collect();
    assert!(e.copy_palette(1, cell(Window::Colours, 2)).is_err());
    assert!(e.move_palette(1, cell(Window::Colours, 2)).is_err());
    assert!(e.copy_palette(9, cell(Window::Colours, 5)).is_err());
    assert!(e.renumber_palette(1, 2).is_err());
    assert!(e.renumber_palette(1, 0).is_err());
    assert!(e.renumber_palette(1, 10_000).is_err());
    assert_eq!(e.palettes().cloned().collect::<Vec<_>>(), before);
}

// ---- PAL-18 delete ----------------------------------------------------------------------

#[test]
fn pal_18_delete_falls_back_to_the_values_at_record_time() {
    let mut e = rig();
    set(&mut e, &[201, 202], "Colour", rgb(0.0, 0.0, 1.0));
    let id = channel_record(&mut e, Window::Colours, 1);
    e.clear();
    // Playback 1 was recorded while the palette was red; it is blue now.
    playback_with_reference(&mut e, id, red());
    select(&mut e, &[202]);
    e.apply_palette(1, Timing::default()).unwrap();
    e.delete_palette(1).unwrap();
    assert!(e.palette(1).is_none());
    let values = &e.playback(1).unwrap().cues[0].values;
    assert_eq!(values[&201]["Colour"], Stored::Value(red()));
    assert_eq!(
        e.programmer().stored(202, "Colour"),
        Some(Stored::Value(blue()))
    );
    tick(&mut e, 0.0);
    assert_eq!((dmx(&e, 2), dmx(&e, 4)), (255, 0));
}

#[test]
fn pal_18_number_and_cell_are_free_again_the_id_is_never_reused() {
    let mut e = rig();
    set(&mut e, &[201], "Colour", rgb(1.0, 0.0, 0.0));
    let first = channel_record(&mut e, Window::Colours, 1);
    e.delete_palette(1).unwrap();
    let second = channel_record(&mut e, Window::Colours, 1);
    assert_eq!(number_of(&e, second), 1);
    assert!(second > first);
    assert!(e.delete_palette(7).is_err());
}

// ---- PAL-19 patch changes -----------------------------------------------------------------

#[test]
fn pal_19_removing_a_fixture_removes_its_entries() {
    let mut e = rig();
    set(&mut e, &[101], "Pan", 0.2);
    set(&mut e, &[102], "Pan", 0.7);
    channel_record(&mut e, Window::Positions, 1);
    e.clear();
    set(&mut e, &[102], "Pan", 0.7);
    e.record_palette(&Record {
        record_by: Some(RecordBy::Channel),
        record_as: RecordAs::Normal,
        ..Record::at(cell(Window::Positions, 2))
    })
    .unwrap();
    e.patch_remove(&[102]).unwrap();
    assert_eq!(
        e.palette(1)
            .unwrap()
            .fixtures
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        [101]
    );
    let left = e.palette(2).expect("an emptied palette stays");
    assert!(left.fixtures.is_empty() && left.shared.is_empty() && left.global.is_empty());
}

// ---- PAL-20 legend and icon -----------------------------------------------------------------

#[test]
fn pal_20_legend_and_icon_are_checked() {
    let mut e = rig();
    set(&mut e, &[201], "Colour", rgb(1.0, 0.0, 0.0));
    channel_record(&mut e, Window::Colours, 1);
    e.set_palette_legend(1, "Warm", Some("service.prayer"))
        .unwrap();
    assert!(e.set_palette_legend(1, &"x".repeat(33), None).is_err());
    assert!(e
        .set_palette_legend(1, "Warm", Some("Colour Drop"))
        .is_err());
    assert!(e
        .set_palette_legend(1, "Warm", Some(&"a".repeat(49)))
        .is_err());
    let p = e.palette(1).unwrap();
    assert_eq!(p.legend, "Warm");
    assert_eq!(p.icon.as_deref(), Some("service.prayer"));
    e.set_palette_legend(1, "", None).unwrap();
    let p = e.palette(1).unwrap();
    assert_eq!((p.legend.as_str(), p.icon.as_deref()), ("", None));
}

// ---- PAL-21 automatic icons -------------------------------------------------------------------

#[test]
fn pal_21_a_colour_palette_shows_its_colours() {
    let mut e = rig();
    set(&mut e, &[201, 202, 221], "Colour", rgb(1.0, 0.0, 0.0));
    channel_record(&mut e, Window::Colours, 1);
    e.set_palette_legend(1, "", Some("colour.drop")).unwrap();
    assert_eq!(
        e.palette_auto_icon(1),
        Some(AutoIcon::Colours(vec![rgb(1.0, 0.0, 0.0)])),
        "a picked icon still reports the automatic one"
    );
}

#[test]
fn pal_21_at_most_four_colours() {
    let mut e = rig();
    let colours = [
        rgb(1.0, 0.0, 0.0),
        rgb(0.0, 1.0, 0.0),
        rgb(0.0, 0.0, 1.0),
        rgb(1.0, 1.0, 0.0),
        rgb(0.0, 1.0, 1.0),
    ];
    for (id, c) in [201, 202, 211, 212, 221].into_iter().zip(colours) {
        set(&mut e, &[id], "Colour", c);
    }
    channel_record(&mut e, Window::Colours, 1);
    assert_eq!(
        e.palette_auto_icon(1),
        Some(AutoIcon::Colours(colours[..4].to_vec()))
    );
}

#[test]
fn pal_21_a_position_palette_shows_its_points() {
    let mut e = rig();
    set(&mut e, &[101], "Pan", 0.25);
    set(&mut e, &[101], "Tilt", 0.75);
    set(&mut e, &[102], "Pan", 0.8);
    channel_record(&mut e, Window::Positions, 1);
    match e.palette_auto_icon(1) {
        Some(AutoIcon::Position(points)) => {
            assert_eq!(points.len(), 2);
            assert!(close(points[0][0], 0.25) && close(points[0][1], 0.75));
            assert!(close(points[1][0], 0.8) && close(points[1][1], 0.5));
        }
        other => panic!("expected a position icon, got {other:?}"),
    }
}

#[test]
fn pal_21_other_palettes_show_their_first_value() {
    let mut e = rig();
    set(&mut e, &[101, 102], "Zoom", 0.4);
    channel_record(&mut e, Window::Beams, 1);
    assert_eq!(
        e.palette_auto_icon(1),
        Some(AutoIcon::Value {
            attribute: "Zoom".into(),
            value: 0.4
        })
    );
}

// ---- PAL-23 show file ---------------------------------------------------------------------------

fn show_rig() -> Engine {
    let mut e = rig();
    set(&mut e, &[101, 102, 111], "Pan", 0.5);
    set(&mut e, &[101, 102], "Tilt", 0.5 - 90.0 / 270.0);
    set(&mut e, &[111], "Tilt", 0.0);
    channel_record(&mut e, Window::Positions, 1);
    e.clear();
    set(&mut e, &[201, 202], "Colour", rgb(1.0, 0.0, 0.0));
    set(&mut e, &[201, 202], "White", 0.3);
    let id = channel_record(&mut e, Window::Colours, 1);
    e.set_palette_legend(2, "Warm", Some("colour.drop"))
        .unwrap();
    e.clear();
    playback_with_reference(&mut e, id, red());
    e
}

#[test]
fn pal_23_palettes_and_references_round_trip() {
    let e = show_rig();
    let show = e.to_show(meta());
    let json = show.to_json();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["schema_version"], serde_json::json!(5)); // GRM-13
                                                           // Top-level keys in file order: lines with exactly two spaces of indent.
    let keys: Vec<&str> = json
        .lines()
        .filter_map(|l| l.strip_prefix("  \""))
        .filter_map(|l| l.split('"').next())
        .collect();
    let groups = keys.iter().position(|k| *k == "groups").unwrap();
    assert_eq!(keys[groups + 1], "palettes");
    assert_eq!(v["palettes"]["next_id"], serde_json::json!(3));
    let warm = &v["palettes"]["palettes"][1];
    assert_eq!(warm["number"], serde_json::json!(2));
    assert_eq!(warm["window"], serde_json::json!("Colours"));
    assert_eq!(warm["handle"], serde_json::json!(1));
    assert_eq!(warm["legend"], serde_json::json!("Warm"));
    assert_eq!(warm["icon"], serde_json::json!("colour.drop"));
    assert_eq!(
        warm["global"]["Colour"],
        serde_json::json!({"r": 1.0, "g": 0.0, "b": 0.0})
    );
    let tilt = v["palettes"]["palettes"][0]["global"]["Tilt"]["degrees"]
        .as_f64()
        .unwrap();
    assert!((tilt + 90.0).abs() < 0.01);
    assert_eq!(
        v["playbacks"]["playbacks"][0]["cues"][0]["values"]["201"]["Colour"],
        serde_json::json!({"palette": 2, "value": {"r": 1.0, "g": 0.0, "b": 0.0}})
    );
    for (key, expected) in [
        ("palette_time", serde_json::json!(0.0)),
        ("palette_overlap", serde_json::json!(100.0)),
        ("record_by", serde_json::json!("Mixed")),
    ] {
        assert_eq!(v["settings"][key], expected, "settings.{key}");
    }

    let (loaded, migrations) = Show::from_json(&json).unwrap();
    assert!(migrations.is_empty());
    let (engine, warnings) = Engine::from_show(&loaded);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(engine.to_show(meta()).to_json(), json);
}

#[test]
fn pal_23_ids_are_never_reused_after_a_load() {
    let e = show_rig();
    let (mut loaded, _) = Engine::from_show(&e.to_show(meta()));
    loaded.delete_palette(2).unwrap();
    set(&mut loaded, &[201], "Colour", rgb(0.0, 0.0, 1.0));
    let id = channel_record(&mut loaded, Window::Colours, 5);
    assert_eq!(id, 3);
}

#[test]
fn pal_23_a_version_1_show_is_migrated() {
    let v1 = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/fixtures/shows/church-demo-v1.cmdmx.json"
    ))
    .unwrap();
    let (show, migrations) = Show::from_json(&v1).unwrap();
    assert_eq!(
        migrations.len(),
        5,
        "1 -> 2, 2 -> 3, 3 -> 4, 4 -> 5 and 5 -> 6: {migrations:?}"
    );
    let (engine, warnings) = Engine::from_show(&show);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(engine.palettes().count(), 0);
    assert_eq!(engine.settings().record_by, RecordBy::Mixed);
    assert_eq!(engine.settings().palette_overlap, 100.0);
}

// ---- PAL-24 lenient load ------------------------------------------------------------------------

#[test]
fn pal_24_bad_palette_entries_are_left_out_with_a_warning() {
    let e = show_rig();
    let mut v: serde_json::Value = serde_json::from_str(&e.to_show(meta()).to_json()).unwrap();
    let palettes = v["palettes"]["palettes"].as_array_mut().unwrap();
    // A second palette with number 2, one with number 0, a fixture entry for 999,
    // and a shared entry for a type no fixture uses.
    let mut dup = palettes[1].clone();
    dup["id"] = serde_json::json!(8);
    dup["handle"] = serde_json::json!(9);
    let mut zero = palettes[1].clone();
    zero["id"] = serde_json::json!(9);
    zero["number"] = serde_json::json!(0);
    zero["handle"] = serde_json::json!(10);
    palettes.push(dup);
    palettes.push(zero);
    palettes[0]["fixtures"]["999"] = serde_json::json!({"Pan": 0.5});
    palettes[0]["shared"]["gone"] = serde_json::json!({"Zoom": 0.5});
    let (show, _) = Show::from_json(&v.to_string()).unwrap();
    let (engine, warnings) = Engine::from_show(&show);
    assert_eq!(warnings.len(), 3, "{warnings:?}");
    assert_eq!(engine.palettes().count(), 2);
    assert!(!engine.palette(1).unwrap().fixtures.contains_key(&999));
    assert!(engine.palette(1).unwrap().shared.contains_key("gone"));
}

#[test]
fn pal_24_a_reference_to_a_missing_palette_becomes_its_fallback() {
    let e = show_rig();
    let mut v: serde_json::Value = serde_json::from_str(&e.to_show(meta()).to_json()).unwrap();
    v["playbacks"]["playbacks"][0]["cues"][0]["values"]["201"]["Colour"]["palette"] =
        serde_json::json!(77);
    let (show, _) = Show::from_json(&v.to_string()).unwrap();
    let (engine, warnings) = Engine::from_show(&show);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(
        engine.playback(1).unwrap().cues[0].values[&201]["Colour"],
        Stored::Value(red())
    );
}

// ---- PAL-32 determinism ---------------------------------------------------------------------------

#[test]
fn pal_32_same_actions_same_palettes() {
    let a = show_rig().to_show(meta()).to_json();
    let b = show_rig().to_show(meta()).to_json();
    assert_eq!(a, b);
}

#[test]
fn pal_32_contents_keep_references() {
    // PRG-15: Record and Update read references as references.
    let mut e = rig();
    e.add_palette(global_palette(4, 1, &[("Colour", g(red()))]))
        .unwrap();
    select(&mut e, &[201]);
    e.apply_palette(1, Timing::default()).unwrap();
    let contents = e.contents(&[C]);
    assert_eq!(
        contents[&201]["Colour"],
        Stored::Reference(Reference {
            palette: 4,
            value: red()
        })
    );
}
