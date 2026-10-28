//! Gel numbers (VS2-48, IDEA-02, P1-23): PRG-67..70 and PAL-33.
//!
//! API these tests fix (`cmdmx_core::gel`):
//! - `Gel { number: u32, name: String, x: f32, y: f32 }`, a row of a range's table.
//! - `GelTable::from_json(&str) -> Result<GelTable, CoreError>`: a JSON array of rows; a row
//!   whose x, y does not convert or a duplicate number is `CoreError::InvalidGel(String)`.
//! - `LEE_JSON: &str`, the compiled-in LEE table (`subprojects/engine/gels/lee.json`; empty until
//!   VS2-50), and `GelTable::lee()`, that table parsed (empty if it does not parse).
//! - `GelTable::gels() -> &[Gel]` sorted by number, `colour(number) -> Option<TargetColour>`
//!   (linear BT.2020, negatives 0, max 1), `reading(TargetColour) -> Option<&Gel>` (PRG-69).
//! - `Engine::set_gels(GelTable)`, `Engine::gel(number) -> Result<(), CoreError>` (PRG-68),
//!   `CoreError::UnknownGel(u32)` ("LEE 999 is not in the gel table").
//! - The command line takes `Lee n` where it takes `Palette n`.
//! - `ColourView::gel: Option<GelRef { range: "Lee", number, name }>`.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use cmdmx_core::colour::{kelvin, planck, CtStep, TargetColour};
use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::gel::{GelTable, LEE_JSON};
use cmdmx_core::palette::{Cell, Record, Window};
use cmdmx_core::patch::Fixture;
use cmdmx_core::profile::Profile;
use cmdmx_core::programmer::Resolution::Normal;
use cmdmx_core::value::Value;
use cmdmx_core::CoreError;

/// A few LEE rows (tungsten x, y from leefilters.com), enough for the criteria. The full table
/// ships only with LEE's permission (VS2-50).
const LEE_ROWS: &str = r#"[
  { "number": 223, "name": "Eighth C.T. Orange", "x": 0.440, "y": 0.406 },
  { "number": 201, "name": "Full C.T. Blue", "x": 0.330, "y": 0.339 },
  { "number": 202, "name": "Half C.T. Blue", "x": 0.372, "y": 0.374 },
  { "number": 203, "name": "Quarter C.T. Blue", "x": 0.398, "y": 0.390 },
  { "number": 204, "name": "Full C.T. Orange", "x": 0.510, "y": 0.420 },
  { "number": 205, "name": "Half C.T. Orange", "x": 0.474, "y": 0.417 },
  { "number": 206, "name": "Quarter C.T. Orange", "x": 0.450, "y": 0.410 },
  { "number": 218, "name": "Eighth C.T. Blue", "x": 0.411, "y": 0.395 },
  { "number": 106, "name": "Primary Red", "x": 0.693, "y": 0.305 },
  { "number": 181, "name": "Congo Blue", "x": 0.187, "y": 0.058 },
  { "number": 79, "name": "Just Blue", "x": 0.145, "y": 0.072 }
]"#;

const RGBW: &str = r#"{ "manufacturer": "T", "model": "RGBW Par", "source": "channel-list", "modes": [ {
  "name": "5ch", "footprint": 5,
  "colour": { "system": "RGB", "components": [ { "channel": 2 }, { "channel": 3 }, { "channel": 4 } ] },
  "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "White", "group": "C", "kind": "Ltp", "channel": 5, "emitter": true } ] } ] }"#;

const DIMMER: &str =
    r#"{ "attributes": [ { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 } ] }"#;

fn table() -> GelTable {
    GelTable::from_json(LEE_ROWS).unwrap()
}

/// 201..=208 RGBW pars from @1 every 5 channels, 301 a dimmer @41; the test LEE rows.
fn rig() -> Engine {
    let mut e = Engine::new(Settings::default());
    e.add_profile("rgbw", Profile::from_json(RGBW).unwrap())
        .unwrap();
    e.add_profile("dimmer", Profile::from_json(DIMMER).unwrap())
        .unwrap();
    let mut fixtures: Vec<(u32, &str, u16)> = (0..8)
        .map(|i| (201 + i, "rgbw", 1 + 5 * i as u16))
        .collect();
    fixtures.push((301, "dimmer", 41));
    for (id, profile, address) in fixtures {
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
    e.set_gels(table());
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

fn select(e: &mut Engine, ids: &[u32]) {
    for &id in ids {
        e.select_fixture(id).unwrap();
    }
}

fn gel_reading(e: &mut Engine) -> Option<u32> {
    e.tick(0.0);
    e.programmer_view().colour.and_then(|c| c.gel).map(|g| {
        assert_eq!(g.range, "Lee");
        g.number
    })
}

// ---- PRG-67: the table ------------------------------------------------------------------

#[test]
fn prg_67_rows_convert_to_bt2020() {
    let t = table();
    close(t.colour(201).unwrap(), rgb(1.0, 0.920, 0.837), 0.003);
    close(t.colour(106).unwrap(), rgb(1.0, 0.029, 0.001), 0.003);
    close(
        t.colour(181).unwrap(),
        rgb(0.153, 0.0, 1.0),
        0.003, // green comes out below 0 and is set to 0
    );
    assert_eq!(t.colour(999), None);
}

#[test]
fn prg_67_rows_sorted_by_number() {
    let numbers: Vec<u32> = table().gels().iter().map(|g| g.number).collect();
    let mut sorted = numbers.clone();
    sorted.sort_unstable();
    assert_eq!(numbers, sorted);
    assert_eq!(table().gels()[0].name, "Just Blue");
}

#[test]
fn prg_67_bad_rows_are_refused() {
    for bad in [
        r#"[{ "number": 1, "name": "Flat", "x": 0.3, "y": 0.0 }]"#,
        r#"[{ "number": 1, "name": "Nowhere", "x": -5.0, "y": 0.2 }]"#,
        r#"[{ "number": 1, "name": "A", "x": 0.3, "y": 0.3 },
            { "number": 1, "name": "B", "x": 0.4, "y": 0.4 }]"#,
        r#"{ "number": 1 }"#,
    ] {
        assert!(
            matches!(GelTable::from_json(bad), Err(CoreError::InvalidGel(_))),
            "{bad} must be refused"
        );
    }
}

#[test]
fn prg_67_compiled_in_lee_table_parses() {
    GelTable::from_json(LEE_JSON).unwrap();
    assert_eq!(GelTable::lee().gels().len(), table_len(LEE_JSON));
}

fn table_len(json: &str) -> usize {
    GelTable::from_json(json).unwrap().gels().len()
}

// ---- PRG-68: applying a gel -------------------------------------------------------------

#[test]
fn prg_68_gel_sets_colour_keeping_brightness() {
    let mut e = rig();
    e.set_programmer_value(201, "Colour", rgb(1.0, 1.0, 1.0))
        .unwrap();
    e.set_programmer_value(202, "Colour", rgb(0.5, 0.0, 0.0))
        .unwrap();
    select(&mut e, &[201]);
    e.gel(106).unwrap();
    close(colour_of(&e, 201), rgb(1.0, 0.029, 0.001), 0.003);
    e.execute_command(&["202", "Lee", "201", "Enter"]).unwrap();
    close(colour_of(&e, 202), rgb(0.50, 0.46, 0.42), 0.005);
}

#[test]
fn prg_68_black_comes_up_at_full() {
    let mut e = rig();
    select(&mut e, &[203]);
    e.gel(201).unwrap();
    close(colour_of(&e, 203), rgb(1.0, 0.920, 0.837), 0.003);
}

#[test]
fn prg_68_command_line_selects_and_applies() {
    let mut e = rig();
    e.execute_command(&["201", "Thro", "208", "Lee", "106", "Enter"])
        .unwrap();
    for id in 201..=208 {
        close(colour_of(&e, id), rgb(1.0, 0.029, 0.001), 0.003);
    }
    e.execute_command(&["Lee", "079", "Enter"]).unwrap();
    close(colour_of(&e, 205), table().colour(79).unwrap(), 0.001);
}

#[test]
fn prg_68_skips_fixtures_without_colour() {
    let mut e = rig();
    select(&mut e, &[201, 301]);
    e.gel(201).unwrap();
    assert_eq!(e.programmer().value(301, "Colour"), None);
}

#[test]
fn prg_68_refusals_change_nothing() {
    let mut e = rig();
    select(&mut e, &[201]);
    let err = e.gel(999).unwrap_err();
    assert!(matches!(err, CoreError::UnknownGel(999)));
    assert_eq!(err.to_string(), "LEE 999 is not in the gel table");
    assert!(e.execute_command(&["Lee", "999", "Enter"]).is_err());
    assert_eq!(e.programmer().value(201, "Colour"), None);

    let mut e = rig();
    assert!(e.gel(201).is_err(), "no targets");
    select(&mut e, &[301]);
    assert!(e.gel(201).is_err(), "no targets with Colour");
    assert!(e.execute_command(&["Lee", "Enter"]).is_err(), "no number");
}

// ---- PRG-69: reading ---------------------------------------------------------------------

#[test]
fn prg_69_colour_reads_as_gel_at_any_level() {
    let mut e = rig();
    select(&mut e, &[201]);
    e.gel(201).unwrap();
    assert_eq!(gel_reading(&mut e), Some(201));
    let c = colour_of(&e, 201);
    e.set_programmer_value(201, "Colour", c.scaled(0.5))
        .unwrap();
    assert_eq!(gel_reading(&mut e), Some(201), "half level still reads");
    e.turn_wheel("Hue", 1, Normal).unwrap();
    assert_eq!(gel_reading(&mut e), None, "a hue step leaves the gel");
}

#[test]
fn prg_69_table_reading() {
    let t = table();
    let c = t.colour(204).unwrap();
    assert_eq!(t.reading(c).map(|g| g.number), Some(204));
    assert_eq!(t.reading(c.scaled(0.3)).map(|g| g.number), Some(204));
    assert_eq!(t.reading(rgb(1.0, 1.0, 1.0)), None);
    assert_eq!(t.reading(rgb(0.0, 0.0, 0.0)), None, "black reads none");
}

// ---- PRG-70: CT gels agree with CT steps -------------------------------------------------

#[test]
fn prg_70_ct_gels_agree_with_ct_steps() {
    let t = table();
    let base = 1e6 / 3200.0;
    for (number, step, tol) in [
        (223, CtStep::CtoEighth, 8.0),
        (206, CtStep::CtoQuarter, 8.0),
        (205, CtStep::CtoHalf, 8.0),
        (204, CtStep::CtoFull, 16.0),
        (218, CtStep::CtbEighth, 8.0),
        (203, CtStep::CtbQuarter, 8.0),
        (202, CtStep::CtbHalf, 8.0),
        (201, CtStep::CtbFull, 8.0),
    ] {
        let gel = 1e6 / kelvin(t.colour(number).unwrap()).unwrap();
        let stepped = base + step.mired();
        assert!(
            (gel - stepped).abs() <= tol,
            "LEE {number} reads {gel:.1} mired, {step:?} on 3200 K gives {stepped:.1}"
        );
    }
    // The same through the engine: 3200 K white plus Full CTB lands near LEE 201.
    let mut e = rig();
    e.set_programmer_value(201, "Colour", planck(3200.0))
        .unwrap();
    select(&mut e, &[201]);
    e.ct_step(CtStep::CtbFull).unwrap();
    let stepped = 1e6 / kelvin(colour_of(&e, 201)).unwrap();
    let gel = 1e6 / kelvin(t.colour(201).unwrap()).unwrap();
    assert!((stepped - gel).abs() <= 8.0);
}

// ---- PAL-33: the gel legend --------------------------------------------------------------

#[test]
fn pal_33_one_gel_gives_the_legend() {
    let mut e = rig();
    e.execute_command(&["201", "Thro", "208", "Lee", "201", "Enter"])
        .unwrap();
    e.record_palette(&Record::at(Cell::new(Window::Colours, 4)))
        .unwrap();
    let legend = &e.palettes().find(|p| p.handle == 4).unwrap().legend;
    assert_eq!(legend, "L201 Full C.T. Blue");
}

#[test]
fn pal_33_mixed_gels_or_no_gel_leave_it_empty() {
    let mut e = rig();
    e.execute_command(&["201", "Lee", "201", "Enter"]).unwrap();
    e.execute_command(&["202", "Lee", "106", "Enter"]).unwrap();
    e.record_palette(&Record::at(Cell::new(Window::Colours, 4)))
        .unwrap();
    e.set_programmer_value(203, "Colour", rgb(0.2, 0.9, 0.4))
        .unwrap();
    e.record_palette(&Record::at(Cell::new(Window::Colours, 5)))
        .unwrap();
    for handle in [4, 5] {
        let p = e.palettes().find(|p| p.handle == handle).unwrap();
        assert_eq!(p.legend, "", "palette on handle {handle}");
    }
}
