//! Locate, Highlight, Off, Rem Dim and Clear (VS2-05): `specs/modules/programmer.md` PRG-10
//! (Locate ending), PRG-23..29, 38..44 and ENG-10 (`engine-merge.md`). Frame-level PRG-23/25/26
//! and ENG-10 are also scenarios in `tests/scenarios/`.
//!
//! API these tests fix (`Engine`):
//! - `locate(&[AttributeGroup]) -> Result<(), CoreError>` (PRG-23, 24, 38) and
//!   `located() -> Values` (the located set, fixture → attribute → value).
//! - `set_highlight(bool) -> Result<(), CoreError>`, `highlight() -> bool` (PRG-26, 39).
//! - `off(&[AttributeGroup])`, `off_attribute(&str)` → `Result<(), CoreError>`; an Off that removes
//!   nothing is `CoreError::NothingToOff` (PRG-27, 40).
//! - `rem_dim() -> Result<(), CoreError>` (PRG-28, 41).
//! - `clear()` follows `clear_precedence`; `clear_mask(&[AttributeGroup])`;
//!   `clear_timed(time: f32, mask: Option<&[AttributeGroup]>) -> Result<(), CoreError>`
//!   (PRG-29, 42, ENG-10).
//! - `Settings { clear_precedence: ClearPrecedence, clear_ltp: ClearLtp, lowlight_level: f32, .. }`
//!   (`cmdmx_core::programmer`), changed by
//!   `set_programmer_settings(ProgrammerSettingsChange { clear_precedence, clear_ltp,
//!   lowlight_level }: all Option)` all or nothing, `CoreError::InvalidLevel` out of range (PRG-42).
//! - `ProgrammerView { located: BTreeMap<FixtureId, Vec<String>>, highlight: bool, .. }` (IPC-33).

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;

use cmdmx_core::attribute::AttributeGroup;
use cmdmx_core::colour::TargetColour;
use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::patch::Fixture;
use cmdmx_core::playback::{Playback, Values};
use cmdmx_core::profile::Profile;
use cmdmx_core::programmer::{ClearLtp, ClearPrecedence, ProgrammerSettingsChange, Resolution};
use cmdmx_core::show::Meta;
use cmdmx_core::undo::History;
use cmdmx_core::value::{Stored, Value};
use cmdmx_core::CoreError;

use AttributeGroup::*;

const ALL: [AttributeGroup; 8] = [I, P, C, G, B, E, S, Fx];

/// No locate or highlight values: the PRG-38/39 fallbacks apply.
const WASH: &str = r#"{ "attributes": [
  { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
  { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 2 },
  { "name": "Tilt", "group": "P", "kind": "Ltp", "channel": 3 },
  { "name": "Zoom", "group": "B", "kind": "Ltp", "channel": 4, "default": 0.4 },
  { "name": "Gobo", "group": "G", "kind": "Ltp", "channel": 5, "default": 0.6 } ] }"#;

/// The profile's own locate and highlight values win.
const SPOT: &str = r#"{ "attributes": [
  { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1, "locate": 0.8, "highlight": 0.6 },
  { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 2, "locate": 0.2 },
  { "name": "Tilt", "group": "P", "kind": "Ltp", "channel": 3 },
  { "name": "Zoom", "group": "B", "kind": "Ltp", "channel": 4, "default": 0.4, "highlight": 1.0 },
  { "name": "Gobo", "group": "G", "kind": "Ltp", "channel": 5, "default": 0.6 } ] }"#;

const RGBW: &str = r#"{ "manufacturer": "T", "model": "RGBW Par", "source": "channel-list", "modes": [ {
  "name": "5ch", "footprint": 5,
  "colour": { "system": "RGB", "components": [ { "channel": 2 }, { "channel": 3 }, { "channel": 4 } ] },
  "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "White", "group": "C", "kind": "Ltp", "channel": 5, "emitter": true } ] } ] }"#;

const DIMMER: &str =
    r#"{ "attributes": [ { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 } ] }"#;

/// Universe 1: washes 101 @1, 102 @6 (Dimmer, Pan, Tilt, Zoom, Gobo); spot 111 @11;
/// RGBW pars 201 @16, 202 @21 (Dimmer, R, G, B, White); dimmers 301 @26, 302 @27, 303 @28.
fn rig() -> Engine {
    let mut e = Engine::new(Settings::default());
    for (key, json) in [
        ("wash", WASH),
        ("spot", SPOT),
        ("rgbw", RGBW),
        ("dimmer", DIMMER),
    ] {
        e.add_profile(key, Profile::from_json(json).unwrap())
            .unwrap();
    }
    for (id, profile, address) in [
        (101, "wash", 1),
        (102, "wash", 6),
        (111, "spot", 11),
        (201, "rgbw", 16),
        (202, "rgbw", 21),
        (301, "dimmer", 26),
        (302, "dimmer", 27),
        (303, "dimmer", 28),
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

fn playback(e: &mut Engine, id: u32, entries: &[(u32, &str, f32)]) {
    let mut v: Values = BTreeMap::new();
    for &(fixture, attr, val) in entries {
        v.entry(fixture)
            .or_default()
            .insert(attr.to_string(), val.into());
    }
    e.add_playback(Playback::new(id, 1, id as u16, v));
    e.set_fader(id, 1.0, 0.0).unwrap();
}

fn level(e: &Engine, fixture: u32, attribute: &str) -> Option<f32> {
    match e.programmer().value(fixture, attribute) {
        Some(Value::Level(v)) => Some(v),
        None => None,
        other => panic!("{fixture} {attribute}: expected a level, got {other:?}"),
    }
}

fn located_attrs(e: &Engine, fixture: u32) -> Vec<String> {
    e.located()
        .get(&fixture)
        .map(|a| a.keys().cloned().collect())
        .unwrap_or_default()
}

fn red() -> Value {
    Value::Colour(TargetColour {
        r: 1.0,
        g: 0.0,
        b: 0.0,
    })
}

fn meta() -> Meta {
    Meta {
        name: "VS2-05".into(),
        created: "2026-10-22T09:00:00Z".into(),
        modified: "2026-10-22T09:00:00Z".into(),
        app_version: "0.1.0".into(),
    }
}

// ---- PRG-23 / PRG-38: Locate values -----------------------------------------------------

#[test]
fn prg_23_locate_drives_the_targets_and_is_not_programmer_contents() {
    let mut e = rig();
    select(&mut e, &[101]);
    e.locate(&ALL).unwrap();
    e.tick(0.1);
    // Dimmer full, Pan/Tilt centred, Zoom and Gobo at their defaults.
    assert_eq!(dmx_range(&e, 1, 5), vec![255, 128, 128, 102, 153]);
    assert_eq!(e.contents(&ALL), Values::new());
    assert_eq!(
        located_attrs(&e, 101),
        vec!["Dimmer", "Gobo", "Pan", "Tilt", "Zoom"]
    );
}

#[test]
fn prg_38_profile_locate_values_win_and_colour_locates_to_open_white() {
    let mut e = rig();
    e.set_programmer_value(201, "Colour", red()).unwrap();
    e.set_programmer_value(201, "White", 0.6).unwrap();
    e.tick(0.05);
    select(&mut e, &[111, 201]);
    e.locate(&ALL).unwrap();
    e.tick(0.1);
    assert_eq!(dmx_range(&e, 11, 15), vec![204, 51, 128, 102, 153]);
    // Colour white, the extra emitter 0 (ADR-005).
    assert_eq!(dmx_range(&e, 16, 20), vec![255, 255, 255, 255, 0]);
}

#[test]
fn prg_23_locate_honours_the_mask() {
    let mut e = rig();
    select(&mut e, &[101]);
    e.locate(&[P]).unwrap();
    e.tick(0.1);
    assert_eq!(dmx_range(&e, 1, 5), vec![0, 128, 128, 102, 153]);
    assert_eq!(located_attrs(&e, 101), vec!["Pan", "Tilt"]);
}

#[test]
fn prg_38_locate_with_no_targets_is_refused() {
    let mut e = rig();
    assert_eq!(e.locate(&ALL), Err(CoreError::EmptySelection));
    assert!(e.located().is_empty());
}

#[test]
fn prg_38_locate_counts_as_a_programmer_change() {
    let mut e = rig();
    select(&mut e, &[101]);
    e.locate(&ALL).unwrap();
    select(&mut e, &[102]);
    assert_eq!(e.programmer().selection(), &[102]);
    assert_eq!(located_attrs(&e, 101).len(), 5);
}

#[test]
fn prg_38_locate_again_overwrites() {
    let mut e = rig();
    select(&mut e, &[101]);
    e.locate(&[P]).unwrap();
    e.locate(&ALL).unwrap();
    assert_eq!(located_attrs(&e, 101).len(), 5);
}

// ---- PRG-24 / PRG-25 ---------------------------------------------------------------------

#[test]
fn prg_24_locate_removes_the_located_attributes_from_the_programmer() {
    let mut e = rig();
    e.set_programmer_value(101, "Pan", 0.9).unwrap();
    e.set_programmer_value(101, "Dimmer", 0.3).unwrap();
    select(&mut e, &[101]);
    e.locate(&[P]).unwrap();
    e.tick(0.1);
    assert_eq!(level(&e, 101, "Pan"), None);
    assert_eq!(level(&e, 101, "Dimmer"), Some(0.3));
    assert_eq!(dmx(&e, 1), 77);
    assert_eq!(dmx(&e, 2), 128);
}

#[test]
fn prg_24_a_touch_starts_from_the_locate_value_and_only_that_attribute_leaves_the_located_set() {
    let mut e = rig();
    select(&mut e, &[101]);
    e.locate(&ALL).unwrap();
    e.tick(0.1);
    e.turn_wheel("Pan", 6, Resolution::Coarse).unwrap();
    e.tick(0.2);
    assert!((level(&e, 101, "Pan").unwrap() - 0.74).abs() < 1e-4);
    assert!(!located_attrs(&e, 101).contains(&"Pan".to_string()));
    assert!(located_attrs(&e, 101).contains(&"Tilt".to_string()));
    // PRG-25: Pan from the programmer, the rest from locate.
    assert_eq!(dmx_range(&e, 1, 5), vec![255, 189, 128, 102, 153]);
    assert_eq!(e.contents(&ALL).len(), 1);
}

#[test]
fn prg_25_located_values_beat_playbacks_below_programmer_priority() {
    let mut e = rig();
    playback(&mut e, 1, &[(101, "Pan", 0.2), (101, "Dimmer", 0.4)]);
    e.tick(0.05);
    select(&mut e, &[101]);
    e.locate(&ALL).unwrap();
    e.tick(0.1);
    assert_eq!(dmx(&e, 1), 255);
    assert_eq!(dmx(&e, 2), 128);
}

// ---- PRG-26 / PRG-39: Highlight ----------------------------------------------------------

#[test]
fn prg_26_highlight_lifts_the_targets_and_lowlights_the_rest_of_the_selection() {
    let mut e = rig();
    for id in [301, 302, 303] {
        e.set_programmer_value(id, "Dimmer", 0.2).unwrap();
    }
    e.tick(0.05);
    select(&mut e, &[301, 302, 303]);
    e.next_fixture(); // sub-selection [301]
    e.set_highlight(true).unwrap();
    e.tick(0.1);
    assert_eq!(dmx_range(&e, 26, 28), vec![255, 64, 64]);
    // Edits made meanwhile stay in the programmer.
    e.set_programmer_value(302, "Dimmer", 0.6).unwrap();
    e.set_highlight(false).unwrap();
    e.tick(0.2);
    assert_eq!(dmx_range(&e, 26, 28), vec![51, 153, 51]);
    assert_eq!(e.contents(&ALL).len(), 3);
}

#[test]
fn prg_26_without_a_sub_selection_the_whole_selection_is_highlighted() {
    let mut e = rig();
    select(&mut e, &[301, 302]);
    e.set_highlight(true).unwrap();
    e.tick(0.1);
    assert_eq!(dmx_range(&e, 26, 28), vec![255, 255, 0]);
    assert!(e.contents(&ALL).is_empty());
}

#[test]
fn prg_26_deselecting_switches_highlight_off() {
    let mut e = rig();
    select(&mut e, &[301]);
    e.set_highlight(true).unwrap();
    e.clear(); // SelectionThenProgrammer: deselects
    assert!(!e.highlight());
    e.tick(0.1);
    assert_eq!(dmx(&e, 26), 0);
}

#[test]
fn prg_39_highlight_values_colour_white_emitters_0_others_untouched() {
    let mut e = rig();
    e.set_programmer_value(201, "Colour", red()).unwrap();
    e.set_programmer_value(201, "White", 0.6).unwrap();
    e.set_programmer_value(111, "Pan", 0.2).unwrap();
    e.tick(0.05);
    select(&mut e, &[201, 111]);
    e.set_highlight(true).unwrap();
    e.tick(0.1);
    assert_eq!(dmx_range(&e, 16, 20), vec![255, 255, 255, 255, 0]);
    // Spot: Dimmer and Zoom have highlight values, Pan is left alone.
    assert_eq!(dmx_range(&e, 11, 15), vec![153, 51, 0, 255, 153]);
}

#[test]
fn prg_39_the_grand_master_still_scales_highlight() {
    let mut e = rig();
    select(&mut e, &[301]);
    e.set_highlight(true).unwrap();
    e.set_grand_master(0.4).unwrap();
    e.tick(0.1);
    assert_eq!(dmx(&e, 26), 102);
}

#[test]
fn prg_39_highlight_follows_fix_steps() {
    let mut e = rig();
    select(&mut e, &[301, 302]);
    e.next_fixture();
    e.set_highlight(true).unwrap();
    e.tick(0.1);
    assert_eq!(dmx_range(&e, 26, 27), vec![255, 64]);
    e.next_fixture();
    e.tick(0.2);
    assert_eq!(dmx_range(&e, 26, 27), vec![64, 255]);
}

#[test]
fn prg_39_highlight_with_no_selection_is_refused_and_is_not_edit_state() {
    let mut e = rig();
    assert_eq!(e.set_highlight(true), Err(CoreError::EmptySelection));
    assert!(!e.highlight());
    select(&mut e, &[301]);
    let before = e.edit_state();
    e.set_highlight(true).unwrap();
    assert_eq!(e.edit_state(), before);
}

// ---- PRG-27 / PRG-40: Off ----------------------------------------------------------------

#[test]
fn prg_27_off_removes_the_mask_from_the_targets() {
    let mut e = rig();
    e.set_programmer_value(201, "Dimmer", 0.5).unwrap();
    e.set_programmer_value(201, "Colour", red()).unwrap();
    e.set_programmer_value(202, "Dimmer", 0.5).unwrap();
    select(&mut e, &[201]);
    e.off(&[C]).unwrap();
    assert!(e.programmer().value(201, "Colour").is_none());
    assert_eq!(level(&e, 201, "Dimmer"), Some(0.5));
    e.off(&[I]).unwrap();
    assert!(!e.programmer().in_programmer(201));
    // 202 is not a target.
    assert_eq!(level(&e, 202, "Dimmer"), Some(0.5));
}

#[test]
fn prg_27_off_also_removes_located_values_in_the_mask() {
    let mut e = rig();
    select(&mut e, &[101]);
    e.locate(&ALL).unwrap();
    e.off(&[P]).unwrap();
    assert_eq!(located_attrs(&e, 101), vec!["Dimmer", "Gobo", "Zoom"]);
}

#[test]
fn prg_40_off_one_attribute_on_every_target() {
    let mut e = rig();
    for id in [201, 202] {
        e.set_programmer_value(id, "Colour", red()).unwrap();
        e.set_programmer_value(id, "White", 0.6).unwrap();
    }
    select(&mut e, &[201, 202]);
    e.off_attribute("Hue").unwrap();
    for id in [201, 202] {
        assert!(e.programmer().value(id, "Colour").is_none());
        assert_eq!(level(&e, id, "White"), Some(0.6));
    }
    e.off_attribute("White").unwrap();
    assert!(e.programmer().is_empty());
}

#[test]
fn prg_40_colour_model_wheel_names_all_mean_colour() {
    for name in [
        "Hue",
        "Saturation",
        "Red",
        "Green",
        "Blue",
        "Cyan",
        "Magenta",
        "Yellow",
    ] {
        let mut e = rig();
        e.set_programmer_value(201, "Colour", red()).unwrap();
        select(&mut e, &[201]);
        e.off_attribute(name).unwrap();
        assert!(e.programmer().value(201, "Colour").is_none(), "{name}");
    }
}

#[test]
fn prg_40_off_one_attribute_removes_its_located_value() {
    let mut e = rig();
    select(&mut e, &[101]);
    e.locate(&ALL).unwrap();
    e.off_attribute("Pan").unwrap();
    assert!(!located_attrs(&e, 101).contains(&"Pan".to_string()));
    assert!(located_attrs(&e, 101).contains(&"Tilt".to_string()));
}

#[test]
fn prg_40_an_off_that_removes_nothing_is_refused() {
    let mut e = rig();
    e.set_programmer_value(201, "Dimmer", 0.5).unwrap();
    assert_eq!(e.off(&[C]), Err(CoreError::EmptySelection));
    select(&mut e, &[201]);
    let before = e.edit_state();
    assert_eq!(e.off(&[C]), Err(CoreError::NothingToOff));
    assert_eq!(e.off_attribute("Pan"), Err(CoreError::NothingToOff));
    assert_eq!(e.edit_state(), before);
    assert_eq!(
        CoreError::NothingToOff.to_string(),
        "Off: nothing of the selection is in the programmer"
    );
}

// ---- PRG-28 / PRG-41: Rem Dim ------------------------------------------------------------

#[test]
fn prg_28_rem_dim_puts_dimmer_0_on_every_unselected_intensity_fixture() {
    let mut e = rig();
    playback(&mut e, 1, &[(302, "Dimmer", 1.0)]);
    select(&mut e, &[301, 201]);
    e.rem_dim().unwrap();
    e.tick(0.1);
    let contents = e.contents(&[I]);
    for id in [101, 102, 111, 202, 302, 303] {
        assert_eq!(level(&e, id, "Dimmer"), Some(0.0), "{id}");
    }
    assert!(!contents.contains_key(&301));
    assert!(!contents.contains_key(&201));
    // Recorded as values: the playback's 302 is held down by the programmer.
    assert_eq!(dmx(&e, 27), 0);
}

#[test]
fn prg_41_rem_dim_with_no_selection_is_refused_and_counts_as_a_change() {
    let mut e = rig();
    assert_eq!(e.rem_dim(), Err(CoreError::EmptySelection));
    assert!(e.programmer().is_empty());
    select(&mut e, &[301]);
    e.rem_dim().unwrap();
    select(&mut e, &[302]);
    assert_eq!(e.programmer().selection(), &[302]);
}

// ---- PRG-29 / PRG-42: Clear --------------------------------------------------------------

#[test]
fn prg_29_selection_then_programmer_empties_the_located_set_too() {
    let mut e = rig();
    select(&mut e, &[101]);
    e.locate(&ALL).unwrap();
    e.set_programmer_value(301, "Dimmer", 0.5).unwrap();
    e.clear();
    assert!(e.programmer().selection().is_empty());
    assert!(!e.located().is_empty());
    e.clear();
    assert!(e.programmer().is_empty());
    assert!(e.located().is_empty());
}

#[test]
fn prg_29_selection_with_programmer_does_both_at_once() {
    let mut e = rig();
    e.set_programmer_settings(ProgrammerSettingsChange {
        clear_precedence: Some(ClearPrecedence::SelectionWithProgrammer),
        ..Default::default()
    })
    .unwrap();
    select(&mut e, &[101]);
    e.locate(&ALL).unwrap();
    e.clear();
    assert!(e.programmer().selection().is_empty());
    assert!(e.located().is_empty());
}

#[test]
fn prg_42_programmer_then_selection() {
    let mut e = rig();
    e.set_programmer_settings(ProgrammerSettingsChange {
        clear_precedence: Some(ClearPrecedence::ProgrammerThenSelection),
        ..Default::default()
    })
    .unwrap();
    select(&mut e, &[301]);
    e.set_programmer_value(301, "Dimmer", 0.5).unwrap();
    e.clear();
    assert!(e.programmer().is_empty());
    assert_eq!(e.programmer().selection(), &[301]);
    e.clear();
    assert!(e.programmer().selection().is_empty());
    // With an empty programmer the first Clear deselects.
    select(&mut e, &[302]);
    e.clear();
    assert!(e.programmer().selection().is_empty());
}

#[test]
fn prg_29_masked_clear_removes_the_mask_for_all_fixtures_and_keeps_the_selection() {
    let mut e = rig();
    e.set_programmer_value(201, "Colour", red()).unwrap();
    e.set_programmer_value(201, "Dimmer", 0.5).unwrap();
    e.set_programmer_value(202, "Colour", red()).unwrap();
    select(&mut e, &[301]);
    e.clear_mask(&[C]).unwrap();
    assert_eq!(e.programmer().selection(), &[301]);
    assert!(e.contents(&[C]).is_empty());
    assert_eq!(level(&e, 201, "Dimmer"), Some(0.5));
}

#[test]
fn prg_42_settings_default_change_refuse_and_save() {
    let mut e = rig();
    let s = e.settings();
    assert_eq!(s.clear_precedence, ClearPrecedence::SelectionThenProgrammer);
    assert_eq!(s.clear_ltp, ClearLtp::Release);
    assert_eq!(s.lowlight_level, 0.25);

    let before = e.settings().clone();
    assert_eq!(
        e.set_programmer_settings(ProgrammerSettingsChange {
            clear_ltp: Some(ClearLtp::Freeze),
            lowlight_level: Some(1.5),
            ..Default::default()
        }),
        Err(CoreError::InvalidLevel(1.5))
    );
    assert_eq!(e.settings(), &before);

    e.set_programmer_settings(ProgrammerSettingsChange {
        clear_precedence: Some(ClearPrecedence::ProgrammerThenSelection),
        clear_ltp: Some(ClearLtp::Freeze),
        lowlight_level: Some(0.1),
    })
    .unwrap();
    let json = e.to_show(meta()).to_json();
    let (back, warnings) = Engine::from_show(&cmdmx_core::show::Show::from_json(&json).unwrap().0);
    assert!(warnings.is_empty(), "{warnings:?}");
    let s = back.settings();
    assert_eq!(s.clear_precedence, ClearPrecedence::ProgrammerThenSelection);
    assert_eq!(s.clear_ltp, ClearLtp::Freeze);
    assert_eq!(s.lowlight_level, 0.1);
}

#[test]
fn prg_42_lowlight_level_is_the_setting() {
    let mut e = rig();
    e.set_programmer_settings(ProgrammerSettingsChange {
        lowlight_level: Some(0.4),
        ..Default::default()
    })
    .unwrap();
    select(&mut e, &[301, 302]);
    e.next_fixture();
    e.set_highlight(true).unwrap();
    e.tick(0.1);
    assert_eq!(dmx_range(&e, 26, 27), vec![255, 102]);
}

// ---- PRG-43: what an LTP attribute outputs after Clear ----------------------------------

fn zoom_after_clear(clear_ltp: ClearLtp, with_playback: bool) -> u8 {
    let mut e = rig();
    e.set_programmer_settings(ProgrammerSettingsChange {
        clear_ltp: Some(clear_ltp),
        ..Default::default()
    })
    .unwrap();
    if with_playback {
        playback(&mut e, 1, &[(102, "Zoom", 0.2)]);
    }
    e.set_programmer_value(102, "Zoom", 0.8).unwrap();
    e.tick(0.1);
    assert_eq!(dmx(&e, 9), 204);
    e.clear(); // empty selection: empties the programmer
    e.tick(0.2);
    dmx(&e, 9)
}

#[test]
fn prg_43_a_playback_takes_the_attribute_back_in_both_settings() {
    assert_eq!(zoom_after_clear(ClearLtp::Release, true), 51);
    assert_eq!(zoom_after_clear(ClearLtp::Freeze, true), 51);
}

#[test]
fn prg_43_without_a_playback_release_goes_to_the_default_and_freeze_keeps() {
    // Zoom default 0.4.
    assert_eq!(zoom_after_clear(ClearLtp::Release, false), 102);
    assert_eq!(zoom_after_clear(ClearLtp::Freeze, false), 204);
}

#[test]
fn prg_43_release_returns_colour_to_open_white_and_off_follows_the_setting() {
    let mut e = rig();
    e.set_programmer_value(201, "Dimmer", 1.0).unwrap();
    e.set_programmer_value(201, "Colour", red()).unwrap();
    e.tick(0.1);
    assert_eq!(dmx_range(&e, 17, 19), vec![255, 0, 0]);
    select(&mut e, &[201]);
    e.off(&[C]).unwrap();
    e.tick(0.2);
    assert_eq!(dmx_range(&e, 17, 19), vec![255, 255, 255]);
}

// ---- ENG-10: timed Clear -----------------------------------------------------------------

#[test]
fn eng_10_a_timed_clear_empties_at_once_and_fades_the_output() {
    let mut e = rig();
    playback(&mut e, 1, &[(302, "Dimmer", 0.4)]);
    e.set_programmer_value(301, "Dimmer", 1.0).unwrap();
    e.set_programmer_value(302, "Dimmer", 1.0).unwrap();
    e.set_programmer_value(102, "Zoom", 1.0).unwrap();
    select(&mut e, &[101]);
    e.locate(&[I]).unwrap();
    e.tick(1.0);
    e.clear_timed(2.0, None).unwrap();
    assert!(e.programmer().is_empty());
    assert!(e.located().is_empty());
    // The selection stays.
    assert_eq!(e.programmer().selection(), &[101]);
    e.tick(1.0);
    e.tick(1.5);
    // p = 0.25: HTP 0.75; LTP Zoom from 1.0 towards its default 0.4 → 0.85.
    assert_eq!(dmx(&e, 26), 191);
    assert_eq!(dmx(&e, 27), 191);
    assert_eq!(dmx(&e, 1), 191);
    assert_eq!(dmx(&e, 9), 217);
    e.tick(2.6);
    // p = 0.8: 301 at 0.2, 302 at max(0.2, playback 0.4).
    assert_eq!(dmx(&e, 26), 51);
    assert_eq!(dmx(&e, 27), 102);
    e.tick(3.1);
    assert_eq!(dmx(&e, 26), 0);
    assert_eq!(dmx(&e, 27), 102);
    assert_eq!(dmx(&e, 1), 0);
    assert_eq!(dmx(&e, 9), 102);
}

#[test]
fn eng_10_freeze_keeps_ltp_as_an_untimed_clear() {
    let mut e = rig();
    e.set_programmer_settings(ProgrammerSettingsChange {
        clear_ltp: Some(ClearLtp::Freeze),
        ..Default::default()
    })
    .unwrap();
    e.set_programmer_value(102, "Zoom", 1.0).unwrap();
    e.set_programmer_value(102, "Dimmer", 1.0).unwrap();
    e.tick(1.0);
    e.clear_timed(2.0, None).unwrap();
    e.tick(1.0);
    e.tick(1.5);
    assert_eq!(dmx(&e, 9), 255);
    assert_eq!(dmx(&e, 6), 191);
}

#[test]
fn eng_10_a_masked_timed_clear_fades_only_the_mask() {
    let mut e = rig();
    e.set_programmer_value(102, "Zoom", 1.0).unwrap();
    e.set_programmer_value(102, "Dimmer", 1.0).unwrap();
    e.tick(1.0);
    e.clear_timed(2.0, Some(&[I])).unwrap();
    assert_eq!(e.contents(&ALL).len(), 1);
    assert_eq!(level(&e, 102, "Zoom"), Some(1.0));
    e.tick(1.0);
    e.tick(1.5);
    assert_eq!(dmx(&e, 6), 191);
    assert_eq!(dmx(&e, 9), 255);
}

#[test]
fn eng_10_a_touch_during_the_fade_starts_from_the_faded_value() {
    let mut e = rig();
    e.set_programmer_value(301, "Dimmer", 1.0).unwrap();
    e.tick(1.0);
    e.clear_timed(2.0, None).unwrap();
    e.tick(1.0);
    e.tick(2.0); // faded to 0.5
    select(&mut e, &[301]);
    e.turn_wheel("Dimmer", 1, Resolution::Normal).unwrap();
    e.tick(2.05);
    let v = level(&e, 301, "Dimmer").unwrap();
    assert!((v - 0.5).abs() < 0.02, "{v}");
    e.tick(5.0);
    assert!(dmx(&e, 26) > 120, "the fade ended at the touch");
}

#[test]
fn eng_10_a_negative_or_non_finite_time_is_refused() {
    let mut e = rig();
    e.set_programmer_value(301, "Dimmer", 1.0).unwrap();
    assert!(e.clear_timed(-1.0, None).is_err());
    assert!(e.clear_timed(f32::NAN, None).is_err());
    assert!(!e.programmer().is_empty());
}

// ---- PRG-44: undo ------------------------------------------------------------------------

#[test]
fn prg_44_undoing_a_locate_takes_the_located_values_away() {
    let mut e = rig();
    let mut h = History::default();
    select(&mut e, &[101]);
    h.edit(&mut e, "Locate", None, |e| e.locate(&ALL)).unwrap();
    assert!(!e.located().is_empty());
    h.undo(&mut e).unwrap();
    assert!(e.located().is_empty());
    e.tick(0.1);
    assert_eq!(dmx(&e, 1), 0);
}

#[test]
fn prg_44_undoing_a_clear_brings_the_located_values_back() {
    let mut e = rig();
    let mut h = History::default();
    select(&mut e, &[101]);
    e.locate(&ALL).unwrap();
    e.clear_selection();
    h.edit(&mut e, "Clear", None, |e| {
        e.clear();
        Ok(())
    })
    .unwrap();
    assert!(e.located().is_empty());
    h.undo(&mut e).unwrap();
    assert_eq!(located_attrs(&e, 101).len(), 5);
}

#[test]
fn prg_44_undo_during_a_timed_clear_puts_the_programmer_back_at_once() {
    let mut e = rig();
    let mut h = History::default();
    e.set_programmer_value(301, "Dimmer", 1.0).unwrap();
    e.tick(1.0);
    h.edit(&mut e, "Clear", None, |e| e.clear_timed(2.0, None))
        .unwrap();
    e.tick(1.0);
    e.tick(2.0);
    h.undo(&mut e).unwrap();
    e.tick(2.05);
    assert_eq!(dmx(&e, 26), 255);
    assert_eq!(
        e.programmer().stored(301, "Dimmer"),
        Some(Stored::Value(Value::Level(1.0)))
    );
}

// ---- PRG-10: a selection ending in Locate --------------------------------------------------

#[test]
fn prg_10_a_line_ending_in_locate_selects_and_locates() {
    let mut e = rig();
    e.execute_command(&["301", "Thro", "303", "Locate"])
        .unwrap();
    assert_eq!(e.programmer().selection(), &[301, 302, 303]);
    e.tick(0.1);
    assert_eq!(dmx_range(&e, 26, 28), vec![255, 255, 255]);
    assert!(e.contents(&ALL).is_empty());
}

// ---- IPC-33: the programmer view -------------------------------------------------------------

#[test]
fn prg_35_the_view_shows_the_located_set_and_highlight() {
    let mut e = rig();
    select(&mut e, &[101]);
    e.locate(&[P]).unwrap();
    e.set_highlight(true).unwrap();
    let view = e.programmer_view();
    assert_eq!(
        view.located.get(&101),
        Some(&vec!["Pan".to_string(), "Tilt".to_string()])
    );
    assert!(view.highlight);
}

// ---- PRG-41 note: an LTP attribute of the I group is not intensity ----------------------

const STROBE: &str = r#"{ "attributes": [
  { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
  { "name": "Strobe", "group": "I", "kind": "Ltp", "channel": 2, "default": 0.1 },
  { "name": "Shutter", "group": "I", "kind": "Ltp", "channel": 3, "locate": 0.9, "highlight": 0.8 } ] }"#;

/// 401 @31, 402 @34 (Dimmer, Strobe, Shutter).
fn strobe_rig() -> Engine {
    let mut e = rig();
    e.add_profile("strobe", Profile::from_json(STROBE).unwrap())
        .unwrap();
    for (id, address) in [(401, 31), (402, 34)] {
        e.patch(Fixture {
            id,
            number: 0,
            name: String::new(),
            handle: None,
            profile: "strobe".into(),
            mode: None,
            universe: 1,
            address,
        })
        .unwrap();
    }
    e
}

#[test]
fn prg_41_locate_highlight_and_rem_dim_never_start_a_strobe() {
    let mut e = strobe_rig();
    select(&mut e, &[401]);
    e.locate(&ALL).unwrap();
    e.tick(0.1);
    // Strobe at its default, Shutter at its locate value.
    assert_eq!(dmx_range(&e, 31, 33), vec![255, 26, 230]);

    e.clear();
    e.clear();
    select(&mut e, &[401, 402]);
    e.next_fixture();
    e.set_highlight(true).unwrap();
    e.tick(0.2);
    // Target 401: Dimmer full, Strobe left alone, Shutter its highlight value.
    assert_eq!(dmx_range(&e, 31, 33), vec![255, 26, 204]);
    // Lowlit 402: only the Dimmer.
    assert_eq!(dmx_range(&e, 34, 36), vec![64, 26, 0]);

    e.set_highlight(false).unwrap();
    e.clear_selection();
    select(&mut e, &[301]);
    e.rem_dim().unwrap();
    assert_eq!(level(&e, 401, "Dimmer"), Some(0.0));
    assert_eq!(level(&e, 401, "Strobe"), None);
    assert_eq!(level(&e, 401, "Shutter"), None);
}
