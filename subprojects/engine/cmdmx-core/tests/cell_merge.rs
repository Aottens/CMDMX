//! Cells in the merge: `specs/modules/programmer.md` PRG-47, 51, 54 (VS2-31).
//!
//! Shehds-like washes 901–902 with Pan, a master Dimmer and three RGBW cells each (no cell
//! Dimmer channels, so every cell has a virtual Dimmer). Between sources (playback against
//! programmer) on the frame is also in `tests/scenarios/PRG-47-*.json` and `PRG-51-*.json`.
//! Cell values are not recorded into cues, groups or palettes yet (VS2-32).

#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use cmdmx_core::attribute::AttributeGroup::{C, I, P};
use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::palette::{Cell, Record, Timing, Window};
use cmdmx_core::patch::{Fixture, FixtureId};
use cmdmx_core::profile::Profile;
use cmdmx_core::programmer::{Item, Resolution};
use cmdmx_core::CoreError;

/// Pan on 1, master Dimmer on 6, three RGBW cells (8–11, 12–15, 16–19).
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
      "attributes": [ { "name": "White", "group": "C", "kind": "Ltp", "channel": 19, "emitter": true } ] } ] } ] }"#;

const RED: f32 = 0.0;
const BLUE: f32 = 2.0 / 3.0;

/// Universe 1: 901 @1, 902 @25.
fn rig() -> Engine {
    let mut e = Engine::new(Settings::default());
    e.add_profile("shehds", Profile::from_json(SHEHDS).unwrap())
        .unwrap();
    for (id, address) in [(901, 1), (902, 25)] {
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

fn run(e: &mut Engine, line: &str) {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    e.execute_command(&tokens).unwrap();
}

fn ch(e: &mut Engine, address: u16) -> u8 {
    e.tick(0.0);
    e.frame(1).and_then(|f| f.channel(address)).unwrap()
}

/// R, G, B of cell `n` (1-based) of 901.
fn rgb(e: &mut Engine, n: u16) -> [u8; 3] {
    let base = 8 + (n - 1) * 4;
    [ch(e, base), ch(e, base + 1), ch(e, base + 2)]
}

/// 901 at full and red as a whole fixture.
fn red_901(e: &mut Engine) {
    run(e, "901 @ 100 Enter");
    e.set_colour(RED, 1.0).unwrap();
}

// ---- PRG-47 -----------------------------------------------------------------

#[test]
fn prg_47_a_fixture_value_still_drives_every_cell() {
    let mut e = rig();
    red_901(&mut e);
    assert_eq!(ch(&mut e, 6), 255);
    for n in 1..=3 {
        assert_eq!(rgb(&mut e, n), [255, 0, 0], "cell {n}");
    }
}

#[test]
fn prg_47_a_cell_level_sets_the_cell_dimmer_not_the_master() {
    let mut e = rig();
    red_901(&mut e);
    run(&mut e, "901 . 2 @ 50 Enter");
    assert_eq!(ch(&mut e, 6), 255, "master Dimmer untouched");
    assert_eq!(rgb(&mut e, 1), [255, 0, 0]);
    assert_eq!(rgb(&mut e, 2), [128, 0, 0], "virtual cell Dimmer scales its colour");
    assert_eq!(rgb(&mut e, 3), [255, 0, 0]);
}

#[test]
fn prg_47_the_cell_dimmer_scales_its_white_emitter() {
    let mut e = rig();
    run(&mut e, "901 . 2 Enter");
    e.set_programmer_level("White", 1.0).unwrap();
    assert_eq!(ch(&mut e, 15), 255);
    run(&mut e, "901 . 2 @ 50 Enter");
    assert_eq!(ch(&mut e, 15), 128);
    assert_eq!(ch(&mut e, 11), 0, "cell 1 White untouched");
}

#[test]
fn prg_47_the_cell_dimmer_is_not_mastered_again() {
    let mut e = rig();
    red_901(&mut e);
    run(&mut e, "901 . 2 @ 50 Enter");
    e.set_grand_master(0.5).unwrap();
    assert_eq!(ch(&mut e, 6), 128, "the Grand Master scales the master Dimmer");
    assert_eq!(rgb(&mut e, 2), [128, 0, 0], "not the cell Dimmer on top of it");
}

#[test]
fn prg_47_a_cell_target_has_only_cell_attributes() {
    let mut e = rig();
    e.set_active_group(P);
    e.select_item(c(901, 2)).unwrap();
    assert!([C, I].contains(&e.active_group()));
    e.set_active_group(P);
    assert!(e.wheel_layout().is_empty(), "Pan is not on the wheels");
    assert_eq!(
        e.turn_wheel("Pan", 10, Resolution::Normal),
        Err(CoreError::AttributeNotInTargets("Pan".into()))
    );
    e.set_active_group(I);
    assert_eq!(e.wheel_layout(), vec!["Dimmer".to_string()]);
}

#[test]
fn prg_47_a_wheel_on_a_cell_moves_only_that_cell() {
    let mut e = rig();
    red_901(&mut e);
    run(&mut e, "901 . 3 Enter");
    e.turn_wheel("Dimmer", -128, Resolution::Coarse).unwrap();
    assert_eq!(rgb(&mut e, 3), [0, 0, 0]);
    assert_eq!(rgb(&mut e, 2), [255, 0, 0]);
    assert_eq!(ch(&mut e, 6), 255);
}

#[test]
fn prg_47_locate_a_cell_locates_only_that_cell() {
    let mut e = rig();
    red_901(&mut e);
    run(&mut e, "901 . 2 Enter");
    e.locate(&[I, C]).unwrap();
    assert_eq!(rgb(&mut e, 2), [255, 255, 255], "located white beats the fixture's red");
    assert_eq!(rgb(&mut e, 1), [255, 0, 0]);
    assert_eq!(ch(&mut e, 6), 255, "master Dimmer is not a cell attribute");
}

#[test]
fn prg_47_a_palette_recorded_on_the_fixture_colours_one_cell() {
    let mut e = rig();
    red_901(&mut e);
    e.record_palette(&Record::at(Cell::new(Window::Colours, 1)))
        .unwrap();
    e.clear(); // deselect
    e.clear(); // empty the programmer
    run(&mut e, "901 . 2 Enter");
    e.apply_palette(1, Timing::default()).unwrap();
    assert_eq!(rgb(&mut e, 2), [255, 0, 0]);
    assert_eq!(rgb(&mut e, 1), [255, 255, 255], "cell 1 stays at the default");
}

// ---- PRG-51 -----------------------------------------------------------------

#[test]
fn prg_51_within_one_source_the_cell_value_beats_the_fixture_value() {
    let mut e = rig();
    red_901(&mut e);
    run(&mut e, "901 . 2 Enter");
    e.set_colour(BLUE, 1.0).unwrap();
    assert_eq!(rgb(&mut e, 1), [255, 0, 0]);
    assert_eq!(rgb(&mut e, 2), [0, 0, 255]);
    assert_eq!(rgb(&mut e, 3), [255, 0, 0]);
}

#[test]
fn prg_51_a_later_fixture_value_does_not_override_the_cell_value() {
    let mut e = rig();
    run(&mut e, "901 . 2 Enter");
    e.set_colour(BLUE, 1.0).unwrap();
    red_901(&mut e);
    assert_eq!(rgb(&mut e, 2), [0, 0, 255]);
    assert_eq!(rgb(&mut e, 1), [255, 0, 0]);
}

#[test]
fn prg_51_off_on_the_cell_gives_the_fixture_value_back() {
    let mut e = rig();
    red_901(&mut e);
    run(&mut e, "901 . 2 Enter");
    e.set_colour(BLUE, 1.0).unwrap();
    e.off(&[C]).unwrap();
    assert_eq!(rgb(&mut e, 2), [255, 0, 0]);
}

#[test]
fn prg_51_off_on_the_fixture_keeps_the_cell_value() {
    let mut e = rig();
    red_901(&mut e);
    run(&mut e, "901 . 2 Enter");
    e.set_colour(BLUE, 1.0).unwrap();
    run(&mut e, "901 Enter");
    e.off(&[C]).unwrap();
    assert_eq!(rgb(&mut e, 2), [0, 0, 255]);
    assert_eq!(rgb(&mut e, 1), [255, 255, 255]);
}

#[test]
fn prg_51_clear_removes_cell_values() {
    let mut e = rig();
    red_901(&mut e);
    run(&mut e, "901 . 2 @ 50 Enter");
    e.clear();
    e.clear();
    assert!(e.programmer().is_empty());
    run(&mut e, "901 @ 100 Enter");
    e.set_colour(RED, 1.0).unwrap();
    assert_eq!(rgb(&mut e, 2), [255, 0, 0]);
}

#[test]
fn prg_51_removing_the_fixture_drops_its_cell_values() {
    let mut e = rig();
    run(&mut e, "901 . 2 @ 50 Enter");
    e.patch_remove(&[901]).unwrap();
    assert!(e.programmer().is_empty());
}

// ---- PRG-54 -----------------------------------------------------------------

#[test]
fn prg_54_highlight_lowlights_the_other_selected_cells_on_their_own_intensity() {
    let mut e = rig();
    red_901(&mut e);
    run(&mut e, "901 . Enter");
    e.next_fixture(); // 901.1
    e.set_highlight(true).unwrap();
    assert_eq!(rgb(&mut e, 1), [255, 255, 255], "highlighted cell");
    let low = (Settings::default().lowlight_level * 255.0).round() as u8;
    assert_eq!(rgb(&mut e, 2), [low, 0, 0]);
    assert_eq!(rgb(&mut e, 3), [low, 0, 0]);
    assert_eq!(ch(&mut e, 6), 255, "the master Dimmer is no item here");
}

#[test]
fn prg_54_highlight_on_a_cell_leaves_the_rest_of_the_fixture() {
    let mut e = rig();
    red_901(&mut e);
    run(&mut e, "901 . 2 Enter");
    e.set_highlight(true).unwrap();
    assert_eq!(rgb(&mut e, 2), [255, 255, 255]);
    assert_eq!(rgb(&mut e, 1), [255, 0, 0]);
    e.set_highlight(false).unwrap();
    assert_eq!(rgb(&mut e, 2), [255, 0, 0]);
}
