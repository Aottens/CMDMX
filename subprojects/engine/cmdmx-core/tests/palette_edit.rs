//! Edit a palette (BLD-04): `specs/modules/palettes.md` PAL-36, PAL-37.
//!
//! API these tests fix (`Engine`):
//! - `edit_palette(number: u32) -> Result<(), CoreError>` (PAL-36): sets the programmer's undo part
//!   aside, empties the programmer, selects the palette's fixtures (per fixture: its patched,
//!   unparked fixtures; shared: the first patched fixture of each type it holds; global: the
//!   selection set aside, refused with none) and includes the palette as PAL-12 does.
//! - `editing() -> Option<PaletteId>`: the palette being edited.
//! - `store_palette_edit() -> Result<(), CoreError>` (PAL-37): stores the programmer into the
//!   edited palette as `Merge` with its window's mask; the edit goes on. One undo step when run
//!   through `History::edit`, and PRG-108 guards it through `guarded`.
//! - `end_palette_edit()` (PAL-37): gives back the programmer set aside (Cancel, and after a
//!   store); a no-op when not editing. Not an undo step.
//! - While editing, `edit_state()` holds the programmer set aside, so programmer changes during
//!   the edit are no undo step and a guarded refusal leaves the edit as it was.
//! - Refusals while editing are `CoreError::EditRefused(sentence)`: a second `edit_palette` and
//!   `History::undo` / `redo` say "Editing palette 3 Rood: Save or Cancel first".

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;

use cmdmx_core::colour::TargetColour;
use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::palette::{GlobalValue, Palette, Timing, Window};
use cmdmx_core::patch::Fixture;
use cmdmx_core::profile::Profile;
use cmdmx_core::undo::History;
use cmdmx_core::value::Value;
use cmdmx_core::CoreError;

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

/// Global palette 81 "Full": Dimmer full.
const FULL: u32 = 81;
/// Per-fixture palette 5 "Links": Pan of 101 and 102.
const LINKS: u32 = 5;
/// Shared palette 6 "Mix": Zoom for washes, White for RGBW pars.
const MIX: u32 = 6;
/// Per-fixture colour palette 3 "Rood": 201 and 202 red.
const ROOD: u32 = 3;

fn palette(id: u32, number: u32, window: Window, legend: &str) -> Palette {
    Palette {
        id,
        number,
        window,
        handle: id as u16,
        legend: legend.into(),
        icon: None,
        global: BTreeMap::new(),
        shared: BTreeMap::new(),
        shared_cells: BTreeMap::new(),
        fixtures: BTreeMap::new(),
        effects: Vec::new(),
    }
}

fn attrs(list: &[(&str, Value)]) -> BTreeMap<String, Value> {
    list.iter().map(|(n, v)| (n.to_string(), *v)).collect()
}

fn red() -> Value {
    Value::Colour(TargetColour {
        r: 1.0,
        g: 0.0,
        b: 0.0,
    })
}

fn blue() -> Value {
    Value::Colour(TargetColour {
        r: 0.0,
        g: 0.0,
        b: 1.0,
    })
}

/// Washes 101 @1, 102 @5, 103 @9, 104 @13; RGBW pars 201 @17, 202 @22. Palettes FULL, LINKS,
/// MIX and ROOD.
fn rig() -> Engine {
    let mut e = Engine::new(Settings::default());
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
        (202, "rgbw", 22),
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
    let mut full = palette(1, FULL, Window::Beams, "Full");
    full.global = BTreeMap::from([("Dimmer".to_string(), GlobalValue::Value(Value::Level(1.0)))]);
    let mut links = palette(2, LINKS, Window::Positions, "Links");
    links.fixtures = BTreeMap::from([
        (101.into(), attrs(&[("Pan", Value::Level(0.2))])),
        (102.into(), attrs(&[("Pan", Value::Level(0.3))])),
    ]);
    let mut mix = palette(3, MIX, Window::Beams, "Mix");
    mix.shared = BTreeMap::from([
        ("wash".to_string(), attrs(&[("Zoom", Value::Level(0.4))])),
        ("rgbw".to_string(), attrs(&[("White", Value::Level(1.0))])),
    ]);
    let mut rood = palette(4, ROOD, Window::Colours, "Rood");
    rood.fixtures = BTreeMap::from([
        (201.into(), attrs(&[("Colour", red())])),
        (202.into(), attrs(&[("Colour", red())])),
    ]);
    for p in [full, links, mix, rood] {
        e.add_palette(p).unwrap();
    }
    e
}

fn select(e: &mut Engine, ids: &[u32]) {
    for &id in ids {
        e.select_fixture(id).unwrap();
    }
}

fn selection(e: &Engine) -> Vec<u32> {
    let mut s = e.programmer().selection();
    s.sort_unstable();
    s
}

fn level(e: &Engine, fixture: u32, attribute: &str) -> Option<f32> {
    match e.programmer().value(fixture, attribute) {
        Some(Value::Level(l)) => Some(l),
        _ => None,
    }
}

fn dmx(e: &Engine, address: u16) -> u8 {
    e.frame(1).and_then(|f| f.channel(address)).unwrap()
}

/// The operator's programmer before an edit: 103 selected at Dimmer 0.5.
fn before(e: &mut Engine) {
    select(e, &[103]);
    e.set_programmer_value(103, "Dimmer", 0.5).unwrap();
}

fn id_of(e: &Engine, number: u32) -> u32 {
    e.palette(number).unwrap().id
}

const BUSY: &str = "Editing palette 3 Rood: Save or Cancel first";

// ---- PAL-36: Edit a palette ------------------------------------------------------------------

#[test]
fn pal_36_a_per_fixture_palette_selects_its_fixtures_into_an_emptied_programmer() {
    let mut e = rig();
    before(&mut e);
    e.edit_palette(LINKS).unwrap();
    assert_eq!(e.editing(), Some(id_of(&e, LINKS)));
    assert_eq!(selection(&e), [101, 102]);
    assert_eq!(
        e.programmer().stored(103, "Dimmer"),
        None,
        "the programmer was set aside"
    );
    assert!(
        e.programmer().stored(101, "Pan").is_some(),
        "the palette is included"
    );
    assert!(e.programmer().stored(102, "Pan").is_some());
}

#[test]
fn pal_36_a_shared_palette_selects_the_first_fixture_of_each_type() {
    let mut e = rig();
    before(&mut e);
    e.edit_palette(MIX).unwrap();
    assert_eq!(selection(&e), [101, 201]);
    assert!(e.programmer().stored(101, "Zoom").is_some());
    assert!(e.programmer().stored(201, "White").is_some());
}

#[test]
fn pal_36_a_global_palette_keeps_the_selection_set_aside() {
    let mut e = rig();
    select(&mut e, &[103, 104]);
    e.set_programmer_value(103, "Dimmer", 0.5).unwrap();
    e.edit_palette(FULL).unwrap();
    assert_eq!(selection(&e), [103, 104]);
    assert!(e.programmer().stored(104, "Dimmer").is_some());
    assert_ne!(
        level(&e, 103, "Dimmer"),
        Some(0.5),
        "the old value was set aside"
    );
}

#[test]
fn pal_36_a_global_palette_with_a_shared_part_is_global() {
    // A colour recorded on RGB fixtures stores Colour globally and the extra emitter per type.
    let mut e = rig();
    let mut p = e.palette(FULL).unwrap().clone();
    p.shared = BTreeMap::from([("rgbw".to_string(), attrs(&[("White", Value::Level(0.0))]))]);
    e.delete_palette(FULL).unwrap();
    e.add_palette(p).unwrap();
    select(&mut e, &[103, 104, 201]);
    e.edit_palette(FULL).unwrap();
    assert_eq!(
        selection(&e),
        [103, 104, 201],
        "global: the selection set aside"
    );
}

#[test]
fn pal_36_a_global_palette_with_no_selection_is_refused() {
    let mut e = rig();
    let err = e.edit_palette(FULL).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Select fixtures to edit Global palette 81 Full"
    );
    assert_eq!(e.editing(), None);
}

#[test]
fn pal_36_an_unknown_palette_is_refused() {
    let mut e = rig();
    assert!(e.edit_palette(99).is_err());
    assert_eq!(e.editing(), None);
}

#[test]
fn pal_36_a_second_edit_undo_and_redo_are_refused_while_editing() {
    let mut e = rig();
    let mut history = History::default();
    history
        .edit(&mut e, "Dimmer", None, |e| {
            e.set_programmer_value(101, "Dimmer", 0.5)
        })
        .unwrap();
    e.edit_palette(ROOD).unwrap();
    let refused = |r: Result<(), CoreError>| match r {
        Err(CoreError::EditRefused(s)) => assert_eq!(s, BUSY),
        other => panic!("expected EditRefused, got {other:?}"),
    };
    refused(e.edit_palette(LINKS));
    refused(history.undo(&mut e).map(drop));
    refused(history.redo(&mut e).map(drop));
    assert_eq!(e.editing(), Some(id_of(&e, ROOD)), "still the first edit");
    assert_eq!(history.len(), 1, "the undo step is still there");
}

#[test]
fn pal_36_edit_is_on_stage_in_live_and_not_in_blind() {
    let mut e = rig();
    e.tick(0.0);
    let r_before = dmx(&e, 18);
    e.edit_palette(ROOD).unwrap();
    assert!(!e.blind(), "Edit does not turn Blind on");
    e.set_programmer_value(201, "Dimmer", 1.0).unwrap();
    e.tick(0.1);
    assert_eq!(dmx(&e, 17), 255, "in Live the edit is on stage");
    e.end_palette_edit();

    e.enter_blind();
    e.edit_palette(ROOD).unwrap();
    e.set_programmer_value(201, "Dimmer", 1.0).unwrap();
    e.tick(0.2);
    assert_eq!(dmx(&e, 17), 0, "in Blind it is not");
    assert_eq!(dmx(&e, 18), r_before);
}

// ---- PAL-37: Save or Cancel --------------------------------------------------------------------

#[test]
fn pal_37_save_stores_the_change_as_merge_and_gives_the_programmer_back() {
    let mut e = rig();
    before(&mut e);
    e.edit_palette(ROOD).unwrap();
    e.set_programmer_value(201, "Colour", blue()).unwrap();
    e.set_programmer_value(201, "Dimmer", 1.0).unwrap(); // outside the Colours mask
    e.store_palette_edit().unwrap();
    e.end_palette_edit();
    assert_eq!(e.palette_value(ROOD, 201, "Colour"), Some(blue()));
    assert_eq!(
        e.palette_value(ROOD, 202, "Colour"),
        Some(red()),
        "Merge keeps the rest"
    );
    assert_eq!(
        e.palette_value(ROOD, 201, "Dimmer"),
        None,
        "the window's mask"
    );
    assert_eq!(e.editing(), None);
    assert_eq!(selection(&e), [103], "the programmer is back as before");
    assert_eq!(level(&e, 103, "Dimmer"), Some(0.5));
    assert_eq!(e.programmer().stored(201, "Colour"), None);
}

#[test]
fn pal_37_cancel_stores_nothing() {
    let mut e = rig();
    before(&mut e);
    e.edit_palette(ROOD).unwrap();
    e.set_programmer_value(201, "Colour", blue()).unwrap();
    e.end_palette_edit();
    assert_eq!(e.palette_value(ROOD, 201, "Colour"), Some(red()));
    assert_eq!(e.editing(), None);
    assert_eq!(selection(&e), [103]);
    assert_eq!(level(&e, 103, "Dimmer"), Some(0.5));
    e.end_palette_edit(); // not editing: a no-op
}

#[test]
fn pal_37_save_is_one_undo_step_and_undo_keeps_the_programmer_given_back() {
    let mut e = rig();
    let mut history = History::default();
    before(&mut e);
    e.edit_palette(ROOD).unwrap();
    history
        .edit(&mut e, "Colour", None, |e| {
            e.set_programmer_value(201, "Colour", blue())
        })
        .unwrap();
    assert_eq!(history.len(), 0, "a change during the edit is no undo step");
    history
        .edit(&mut e, "Save palette", None, |e| e.store_palette_edit())
        .unwrap();
    e.end_palette_edit();
    assert_eq!(history.len(), 1);
    history.undo(&mut e).unwrap();
    assert_eq!(e.palette_value(ROOD, 201, "Colour"), Some(red()));
    assert_eq!(selection(&e), [103]);
    assert_eq!(level(&e, 103, "Dimmer"), Some(0.5));
    history.redo(&mut e).unwrap();
    assert_eq!(e.palette_value(ROOD, 201, "Colour"), Some(blue()));
}

#[test]
fn pal_37_the_edits_values_leave_the_stage_when_the_programmer_comes_back() {
    let mut e = rig();
    e.tick(0.0);
    let (d, r) = (dmx(&e, 17), dmx(&e, 18));
    e.edit_palette(ROOD).unwrap();
    e.set_programmer_value(201, "Dimmer", 1.0).unwrap();
    e.set_programmer_value(201, "Colour", blue()).unwrap();
    e.tick(0.1);
    assert_eq!(dmx(&e, 17), 255);
    e.end_palette_edit();
    e.tick(0.2);
    assert_eq!(
        (dmx(&e, 17), dmx(&e, 18)),
        (d, r),
        "as a restored undo step lets go"
    );
}

#[test]
fn pal_37_in_blind_save_asks_when_the_palette_is_on_stage_and_the_edit_stays() {
    let mut e = rig();
    select(&mut e, &[201]);
    e.apply_palette(ROOD, Timing::default()).unwrap();
    e.tick(0.0);
    e.enter_blind();
    e.edit_palette(ROOD).unwrap();
    e.set_programmer_value(201, "Colour", blue()).unwrap();
    match e.guarded(false, |e| e.store_palette_edit()) {
        Err(CoreError::OnStage(s)) => assert!(s.contains("Palette 3 Rood"), "{s}"),
        other => panic!("expected OnStage, got {other:?}"),
    }
    assert_eq!(
        e.palette_value(ROOD, 201, "Colour"),
        Some(red()),
        "nothing stored"
    );
    assert_eq!(e.editing(), Some(id_of(&e, ROOD)), "the edit goes on");
    assert_eq!(e.programmer().value(201, "Colour"), Some(blue()));
    e.guarded(true, |e| e.store_palette_edit()).unwrap();
    e.end_palette_edit();
    assert_eq!(e.palette_value(ROOD, 201, "Colour"), Some(blue()));
    assert!(
        e.programmer().stored(201, "Colour").is_some(),
        "the reference set aside"
    );
}
