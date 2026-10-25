//! Cell values stored by item: `specs/modules/programmer.md` PRG-52, 53 (VS2-32), and the show
//! schema 6 → 7 migration (groups store items).
//!
//! Shehds-like washes 901–904 as in `tests/cell_merge.rs`: Pan, a master Dimmer and three RGBW
//! cells (no cell Dimmer channels). Mode `2cell` is the same fixture with only two cells.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use cmdmx_core::attribute::AttributeGroup::{C, I};
use cmdmx_core::engine::{Engine, Settings, ALL_GROUPS};
use cmdmx_core::palette::{Cell, Record, RecordAs, Window};
use cmdmx_core::patch::{Fixture, FixtureId};
use cmdmx_core::playback::PlaybackRecord;
use cmdmx_core::profile::Profile;
use cmdmx_core::programmer::Item;
use cmdmx_core::show::{Meta, Show};

/// Pan on 1, master Dimmer on 6, RGBW cells on 8–11, 12–15, 16–19 (mode `2cell`: the first two).
const SHEHDS: &str = r#"{ "manufacturer": "T", "model": "Cells", "source": "channel-list", "modes": [ {
  "name": "24ch", "footprint": 24,
  "attributes": [
    { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 1 },
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 6 } ],
  "cells": [
    { "colour": { "system": "RGB", "components": [ { "channel": 8 }, { "channel": 9 }, { "channel": 10 } ] },
      "attributes": [ { "name": "White", "group": "C", "kind": "Ltp", "channel": 11, "emitter": true } ] },
    { "colour": { "system": "RGB", "components": [ { "channel": 12 }, { "channel": 13 }, { "channel": 14 } ] },
      "attributes": [ { "name": "White", "group": "C", "kind": "Ltp", "channel": 15, "emitter": true } ] },
    { "colour": { "system": "RGB", "components": [ { "channel": 16 }, { "channel": 17 }, { "channel": 18 } ] },
      "attributes": [ { "name": "White", "group": "C", "kind": "Ltp", "channel": 19, "emitter": true } ] } ] }, {
  "name": "2cell", "footprint": 24,
  "attributes": [
    { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 1 },
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 6 } ],
  "cells": [
    { "colour": { "system": "RGB", "components": [ { "channel": 8 }, { "channel": 9 }, { "channel": 10 } ] },
      "attributes": [ { "name": "White", "group": "C", "kind": "Ltp", "channel": 11, "emitter": true } ] },
    { "colour": { "system": "RGB", "components": [ { "channel": 12 }, { "channel": 13 }, { "channel": 14 } ] },
      "attributes": [ { "name": "White", "group": "C", "kind": "Ltp", "channel": 15, "emitter": true } ] } ] } ] }"#;

const RED: f32 = 0.0;
const BLUE: f32 = 2.0 / 3.0;

/// Universe 1: 901 @1, 902 @25, 903 @49, 904 @73.
fn rig() -> Engine {
    let mut e = Engine::new(Settings::default());
    e.add_profile("shehds", Profile::from_json(SHEHDS).unwrap())
        .unwrap();
    for (id, address) in [(901, 1), (902, 25), (903, 49), (904, 73)] {
        e.patch(Fixture {
            id,
            number: 0,
            name: String::new(),
            handle: None,
            profile: "shehds".into(),
            mode: None,
            universe: 1,
            address,
        })
        .unwrap();
    }
    e
}

fn c(id: FixtureId, cell: u16) -> Item {
    Item::cell(id, cell)
}

fn f(id: FixtureId) -> Item {
    Item::fixture(id)
}

fn meta() -> Meta {
    Meta {
        name: "t".into(),
        created: String::new(),
        modified: String::new(),
        app_version: String::new(),
    }
}

fn run(e: &mut Engine, line: &str) {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    e.execute_command(&tokens).unwrap();
}

/// R, G, B of cell `n` (1-based) of the fixture at `address`.
fn rgb_at(e: &mut Engine, address: u16, n: u16) -> [u8; 3] {
    e.tick(0.0);
    let base = address + 7 + (n - 1) * 4;
    let frame = e.frame(1).unwrap();
    [0, 1, 2].map(|i| frame.channel(base + i).unwrap())
}

fn rgb(e: &mut Engine, n: u16) -> [u8; 3] {
    rgb_at(e, 1, n)
}

/// 901 at full and red as a whole fixture, cell 2 blue.
fn red_901_blue_cell_2(e: &mut Engine) {
    run(e, "901 @ 100 Enter");
    e.set_colour(RED, 1.0).unwrap();
    run(e, "901 . 2 Enter");
    e.set_colour(BLUE, 1.0).unwrap();
}

/// Deselect and empty the programmer (PRG-29 default precedence: two presses).
fn clear_all(e: &mut Engine) {
    e.clear();
    e.clear();
    assert!(e.contents(&ALL_GROUPS).is_empty());
}

fn record_colour(e: &mut Engine, record_as: RecordAs) {
    e.record_palette(&Record {
        record_as,
        ..Record::at(Cell::new(Window::Colours, 1))
    })
    .unwrap();
}

/// Save to JSON and load again.
fn reload(e: &Engine) -> Engine {
    let (show, migrations) = Show::from_json(&e.to_show(meta()).to_json()).unwrap();
    assert!(migrations.is_empty());
    Engine::from_show(&show).0
}

// ---- PRG-52 -----------------------------------------------------------------

#[test]
fn prg_52_contents_key_cell_values_by_item() {
    let mut e = rig();
    red_901_blue_cell_2(&mut e);
    let got = e.contents(&[C, I]);
    assert_eq!(got.keys().copied().collect::<Vec<_>>(), [f(901), c(901, 2)]);
    assert!(got[&f(901)].contains_key("Dimmer"));
    assert!(got[&f(901)].contains_key("Colour"));
    assert_eq!(
        got[&c(901, 2)]
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["Colour"]
    );
    assert!(
        !e.contents(&[I]).contains_key(&c(901, 2)),
        "the mask still filters"
    );
}

#[test]
fn prg_52_items_are_written_as_fixture_dot_cell() {
    let mut e = rig();
    red_901_blue_cell_2(&mut e);
    e.record_playback(&PlaybackRecord::at(1, 1)).unwrap();
    let json = e.to_show(meta()).to_json();
    assert!(json.contains("\"901.2\""), "cell key in the cue: {json}");
    assert!(json.contains("\"901\""), "whole fixture key unchanged");
}

#[test]
fn prg_52_a_cue_stores_and_plays_cell_values() {
    let mut e = rig();
    red_901_blue_cell_2(&mut e);
    let id = e.record_playback(&PlaybackRecord::at(1, 1)).unwrap();
    let values = &e.playback(id).unwrap().cues[0].values;
    assert_eq!(
        values.keys().copied().collect::<Vec<_>>(),
        [f(901), c(901, 2)]
    );
    clear_all(&mut e);
    for mut e in [reload(&e), e] {
        e.set_fader(id, 1.0, 0.0).unwrap();
        assert_eq!(rgb(&mut e, 1), [255, 0, 0]);
        assert_eq!(rgb(&mut e, 2), [0, 0, 255], "the cue's cell value");
        assert_eq!(rgb(&mut e, 3), [255, 0, 0]);
    }
}

#[test]
fn prg_52_a_group_with_cells_survives_save_and_load() {
    let mut e = rig();
    run(&mut e, "901 . 1 And 902 . 1 And 903 Enter");
    e.record_group(1).unwrap();
    let json = e.to_show(meta()).to_json();
    assert!(json.contains("\"901.1\""), "{json}");
    let mut l = reload(&e);
    assert_eq!(l.group_items(1), Some(vec![c(901, 1), c(902, 1), f(903)]));
    l.select_group(1).unwrap();
    assert_eq!(l.programmer().items(), [c(901, 1), c(902, 1), f(903)]);
}

#[test]
fn prg_52_a_palette_stores_cell_values_by_item_and_include_loads_them() {
    let mut e = rig();
    run(&mut e, "901 . 2 Enter");
    e.set_colour(BLUE, 1.0).unwrap();
    record_colour(&mut e, RecordAs::Normal);
    let p = e.palette(1).unwrap();
    assert_eq!(p.fixtures.keys().copied().collect::<Vec<_>>(), [c(901, 2)]);
    assert!(p.shared.is_empty() && p.global.is_empty());
    clear_all(&mut e);
    for mut e in [reload(&e), e] {
        e.include_palette(1).unwrap();
        assert_eq!(
            e.contents(&[C]).keys().copied().collect::<Vec<_>>(),
            [c(901, 2)]
        );
    }
}

#[test]
fn prg_52_a_cell_resolves_its_own_value_before_its_fixtures() {
    let mut e = rig();
    red_901_blue_cell_2(&mut e);
    record_colour(&mut e, RecordAs::Normal);
    clear_all(&mut e);
    run(&mut e, "901 @ 100 Enter");
    run(&mut e, "901 . 1 And 901 . 2 Enter");
    e.apply_palette(1, Default::default()).unwrap();
    assert_eq!(rgb(&mut e, 1), [255, 0, 0], "no 901.1 value: 901's");
    assert_eq!(rgb(&mut e, 2), [0, 0, 255], "901.2's own value");
}

#[test]
fn prg_52_a_shared_cell_value_colours_that_cell_of_another_fixture() {
    let mut e = rig();
    run(&mut e, "901 . 2 And 902 . 2 Enter");
    e.set_colour(BLUE, 1.0).unwrap();
    record_colour(&mut e, RecordAs::Shared);
    assert!(
        e.palette(1).unwrap().fixtures.is_empty(),
        "shared, not per item"
    );
    clear_all(&mut e);
    let mut l = reload(&e);
    for e in [&mut l, &mut e] {
        run(e, "903 @ 100 Enter");
        run(e, "903 . 1 And 903 . 2 Enter");
        e.apply_palette(1, Default::default()).unwrap();
        assert_eq!(rgb_at(e, 49, 2), [0, 0, 255], "the type's cell 2");
        let cells: Vec<Item> = e.contents(&[C]).keys().copied().collect();
        assert_eq!(cells, [c(903, 2)], "nothing for cell 1");
    }
}

// ---- PRG-53 -----------------------------------------------------------------

#[test]
fn prg_53_a_missing_cell_is_kept_in_a_group_and_skipped_on_select() {
    let mut e = rig();
    run(&mut e, "901 . 3 And 902 . 3 Enter");
    e.record_group(1).unwrap();
    e.patch_change_mode(901, "2cell").unwrap();
    assert_eq!(e.group_items(1), Some(vec![c(901, 3), c(902, 3)]));
    e.clear_selection();
    e.select_group(1).unwrap();
    assert_eq!(e.programmer().items(), [c(902, 3)]);
    assert_eq!(
        reload(&e).group_items(1),
        Some(vec![c(901, 3), c(902, 3)]),
        "kept in the show"
    );
}

#[test]
fn prg_53_a_missing_cell_in_a_cue_is_kept_and_outputs_nothing() {
    let mut e = rig();
    run(&mut e, "901 @ 100 Enter");
    e.set_colour(RED, 1.0).unwrap();
    run(&mut e, "901 . 3 Enter");
    e.set_colour(BLUE, 1.0).unwrap();
    let id = e.record_playback(&PlaybackRecord::at(1, 1)).unwrap();
    clear_all(&mut e);
    e.patch_change_mode(901, "2cell").unwrap();
    let values = &e.playback(id).unwrap().cues[0].values;
    assert!(values[&c(901, 3)].contains_key("Colour"), "kept");
    e.set_fader(id, 1.0, 0.0).unwrap();
    assert_eq!(rgb(&mut e, 1), [255, 0, 0]);
    assert_eq!(rgb(&mut e, 2), [255, 0, 0]);
    assert_eq!(rgb(&mut e, 3), [0, 0, 0], "no cell 3: nothing there");
    e.patch_change_mode(901, "24ch").unwrap();
    assert_eq!(rgb(&mut e, 3), [0, 0, 255], "back in a mode with cell 3");
}

#[test]
fn prg_53_a_missing_cell_in_a_palette_is_kept_and_resolves_nothing() {
    let mut e = rig();
    run(&mut e, "901 . 3 And 902 . 3 Enter");
    e.set_colour(BLUE, 1.0).unwrap();
    record_colour(&mut e, RecordAs::Normal);
    clear_all(&mut e);
    e.patch_change_mode(901, "2cell").unwrap();
    assert!(e.palette(1).unwrap().fixtures.contains_key(&c(901, 3)));
    run(&mut e, "901 And 902 @ 100 Enter");
    run(&mut e, "901 . 2 And 902 . 3 Enter");
    e.apply_palette(1, Default::default()).unwrap();
    assert_eq!(rgb_at(&mut e, 25, 3), [0, 0, 255], "902.3 still resolves");
    assert!(
        !e.contents(&[C]).keys().any(|i| i.fixture == 901),
        "901.3 is gone from the mode and nothing else resolves for 901.2"
    );
}

#[test]
fn prg_53_removing_a_fixture_removes_its_cell_items() {
    let mut e = rig();
    run(&mut e, "901 . 2 And 902 . 2 Enter");
    e.set_colour(BLUE, 1.0).unwrap();
    e.record_group(1).unwrap();
    let id = e.record_playback(&PlaybackRecord::at(1, 1)).unwrap();
    record_colour(&mut e, RecordAs::Normal);
    e.patch_remove(&[901]).unwrap();
    assert_eq!(e.group_items(1), Some(vec![c(902, 2)]));
    let cue = &e.playback(id).unwrap().cues[0].values;
    assert_eq!(cue.keys().copied().collect::<Vec<_>>(), [c(902, 2)]);
    let p = e.palette(1).unwrap();
    assert_eq!(p.fixtures.keys().copied().collect::<Vec<_>>(), [c(902, 2)]);
}

// ---- show schema 6 → 7 ------------------------------------------------------

#[test]
fn shw_04_schema_6_groups_migrate_to_items() {
    let mut e = rig();
    run(&mut e, "902 And 901 Enter");
    e.record_group(1).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&e.to_show(meta()).to_json()).unwrap();
    assert_eq!(value["schema_version"], 7);
    assert_eq!(
        value["groups"][0]["items"],
        serde_json::json!(["902", "901"])
    );
    // The same show as schema 6 wrote it: groups hold fixture ids.
    value["schema_version"] = 6.into();
    value["groups"] = serde_json::json!([{ "id": 1, "fixtures": [902, 901] }]);
    let (show, migrations) = Show::from_json(&value.to_string()).unwrap();
    assert_eq!(migrations.len(), 1);
    assert!(migrations[0].starts_with("6 -> 7"), "{migrations:?}");
    let (l, _) = Engine::from_show(&show);
    assert_eq!(l.group_items(1), Some(vec![f(902), f(901)]));
}

#[test]
fn shw_04_the_schema_6_church_demo_migrates_to_the_schema_7_file() {
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/fixtures/shows/"
    );
    let v6 = std::fs::read_to_string(format!("{dir}church-demo-v6.cmdmx.json")).unwrap();
    let v7 = std::fs::read_to_string(format!("{dir}church-demo.cmdmx.json")).unwrap();
    let (show, migrations) = Show::from_json(&v6).unwrap();
    assert_eq!(migrations.len(), 1, "{migrations:?}");
    assert_eq!(show.to_json(), v7);
}
