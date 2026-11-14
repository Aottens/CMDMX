//! Behaviour at the output (PTC-07): `specs/modules/patch.md` PAT-44..47.
//!
//! API these tests fix:
//! - `cmdmx_core::patch::Behaviour { invert, low, high, offset }` (`Default`: off, 0, 1, 0), and
//!   `Fixture.behaviour: BTreeMap<String, Behaviour>`, `Fixture.swap_pan_tilt: bool`.
//! - `cmdmx_core::patch::BehaviourEdit { invert, low, high, offset }`, each an `Option`.
//! - `Engine::patch_behaviour(ids, attribute, &edit)`, `patch_behaviour_reset(ids)`,
//!   `patch_swap_pan_tilt(ids, swapped)`.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use cmdmx_core::attribute::AttributeGroup;
use cmdmx_core::colour::TargetColour;
use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::patch::{Behaviour, BehaviourEdit, FixtureId, PatchAdd};
use cmdmx_core::profile::Profile;
use cmdmx_core::programmer::Item;
use cmdmx_core::show::{Meta, Show, SCHEMA_VERSION};
use cmdmx_core::undo::History;
use cmdmx_core::CoreError;
use serde_json::{json, Value as Json};

/// Dimmer 1, Pan 2/3, Tilt 4/5, Zoom 6 (default 0.25). "Short" has Dimmer and Pan only.
const MOVER: &str = r#"{ "manufacturer": "T", "model": "Mover", "source": "channel-list", "modes": [
  { "name": "Std", "footprint": 6, "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 2, "fine_channel": 3 },
    { "name": "Tilt", "group": "P", "kind": "Ltp", "channel": 4, "fine_channel": 5 },
    { "name": "Zoom", "group": "B", "kind": "Ltp", "channel": 6, "default": 0.25 } ] },
  { "name": "Short", "footprint": 2, "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 2 } ] } ] }"#;

/// Same shape as Mover's Std under another key, for an exchange.
const SPOT: &str = r#"{ "manufacturer": "T", "model": "Spot", "source": "channel-list", "modes": [
  { "name": "Two", "footprint": 2, "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "Iris", "group": "B", "kind": "Ltp", "channel": 2 } ] } ] }"#;

/// RGB 1–3 with a virtual dimmer.
const PAR: &str = r#"{ "manufacturer": "T", "model": "Par", "source": "channel-list", "modes": [ {
  "name": "3ch", "footprint": 3, "virtual_dimmer": true,
  "colour": { "system": "RGB", "components": [ { "channel": 1 }, { "channel": 2 }, { "channel": 3 } ] },
  "attributes": [] } ] }"#;

/// Dimmer 1, RGB 2–4, White 5 at D65, auto-mix on.
const RGBW: &str = r#"{ "manufacturer": "T", "model": "RGBW", "source": "channel-list", "modes": [ {
  "name": "5ch", "footprint": 5, "automix": true,
  "colour": { "system": "RGB", "components": [ { "channel": 2 }, { "channel": 3 }, { "channel": 4 } ] },
  "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "White", "group": "C", "kind": "Ltp", "channel": 5, "emitter": true,
      "cie": [0.3127, 0.3290] } ] } ] }"#;

/// Two cells, each with a White level (channels 1 and 2).
const BAR: &str = r#"{ "manufacturer": "T", "model": "Bar", "source": "channel-list", "modes": [ {
  "name": "2ch", "footprint": 2, "attributes": [],
  "cells": [
    { "attributes": [ { "name": "White", "group": "C", "kind": "Ltp", "channel": 1 } ] },
    { "attributes": [ { "name": "White", "group": "C", "kind": "Ltp", "channel": 2 } ] } ] } ] }"#;

fn engine() -> Engine {
    let mut e = Engine::new(Settings::default());
    for (key, json) in [
        ("mover", MOVER),
        ("spot", SPOT),
        ("par", PAR),
        ("rgbw", RGBW),
        ("bar", BAR),
    ] {
        e.add_profile(key, Profile::from_json(json).unwrap())
            .unwrap();
    }
    e
}

fn add(e: &mut Engine, profile: &str, count: u32, number: u32, address: u16) -> Vec<FixtureId> {
    e.patch_add(&PatchAdd {
        profile: profile.into(),
        mode: None,
        count,
        universe: 1,
        address: Some(address),
        number: Some(number),
    })
    .unwrap()
}

/// Movers 101 at 1 and 102 at 7.
fn rig() -> (Engine, FixtureId, FixtureId) {
    let mut e = engine();
    let ids = add(&mut e, "mover", 2, 101, 1);
    (e, ids[0], ids[1])
}

fn dmx(e: &Engine, address: u16) -> u8 {
    e.frame(1).and_then(|f| f.channel(address)).unwrap()
}

fn invert() -> BehaviourEdit {
    BehaviourEdit {
        invert: Some(true),
        ..Default::default()
    }
}

fn high(v: f32) -> BehaviourEdit {
    BehaviourEdit {
        high: Some(v),
        ..Default::default()
    }
}

fn low(v: f32) -> BehaviourEdit {
    BehaviourEdit {
        low: Some(v),
        ..Default::default()
    }
}

fn offset(v: f32) -> BehaviourEdit {
    BehaviourEdit {
        offset: Some(v),
        ..Default::default()
    }
}

fn meta() -> Meta {
    Meta {
        name: "Behaviour".into(),
        created: "2026-11-10T09:00:00Z".into(),
        modified: "2026-11-10T09:00:00Z".into(),
        app_version: "0.1.0".into(),
    }
}

fn reopen(e: &Engine) -> Engine {
    let (show, migrations) = Show::from_json(&e.to_show(meta()).to_json()).unwrap();
    assert!(migrations.is_empty(), "{migrations:?}");
    let (loaded, warnings) = Engine::from_show(&show);
    assert!(warnings.is_empty(), "{warnings:?}");
    loaded
}

// ---- PAT-44 -------------------------------------------------------------------------

#[test]
fn pat_44_sets_only_the_fields_given_on_the_whole_range() {
    let (mut e, a, b) = rig();
    e.patch_behaviour(&[a, b], "Pan", &invert()).unwrap();
    e.patch_behaviour(&[a, b], "Pan", &high(0.8)).unwrap();
    for id in [a, b] {
        let got = e.fixture(id).unwrap().behaviour["Pan"];
        assert_eq!(
            got,
            Behaviour {
                invert: true,
                high: 0.8,
                ..Behaviour::default()
            }
        );
    }
    e.patch_behaviour(
        &[a],
        "Pan",
        &BehaviourEdit {
            invert: Some(false),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!e.fixture(a).unwrap().behaviour["Pan"].invert);
    assert_eq!(e.fixture(a).unwrap().behaviour["Pan"].high, 0.8);
}

#[test]
fn pat_44_all_fields_back_at_their_default_leaves_no_entry() {
    let (mut e, a, _) = rig();
    e.patch_behaviour(&[a], "Zoom", &invert()).unwrap();
    let off = BehaviourEdit {
        invert: Some(false),
        ..Default::default()
    };
    e.patch_behaviour(&[a], "Zoom", &off).unwrap();
    assert!(e.fixture(a).unwrap().behaviour.is_empty());
}

#[test]
fn pat_44_a_fixture_without_the_attribute_refuses_the_whole_command() {
    let mut e = engine();
    let m = add(&mut e, "mover", 1, 101, 1)[0];
    let s = add(&mut e, "spot", 1, 201, 20)[0];
    let r = e.patch_behaviour(&[m, s], "Pan", &invert());
    assert!(
        matches!(&r, Err(CoreError::UnknownAttribute { fixture: 201, attribute }) if attribute == "Pan"),
        "{r:?}"
    );
    assert!(
        e.fixture(m).unwrap().behaviour.is_empty(),
        "nothing changes"
    );
}

#[test]
fn pat_44_low_above_high_or_out_of_range_refuses() {
    let (mut e, a, _) = rig();
    e.patch_behaviour(&[a], "Zoom", &high(0.5)).unwrap();
    let before = e.fixture(a).unwrap().behaviour.clone();
    assert!(
        e.patch_behaviour(&[a], "Zoom", &low(0.6)).is_err(),
        "low above the stored high"
    );
    for bad in [
        low(-0.1),
        low(1.1),
        high(-0.1),
        high(1.1),
        offset(-1.1),
        offset(1.1),
        BehaviourEdit {
            low: Some(0.9),
            high: Some(0.1),
            ..Default::default()
        },
    ] {
        assert!(e.patch_behaviour(&[a], "Zoom", &bad).is_err(), "{bad:?}");
    }
    assert_eq!(e.fixture(a).unwrap().behaviour, before);
}

#[test]
fn pat_44_colour_holds_no_level_and_refuses() {
    let mut e = engine();
    let p = add(&mut e, "par", 1, 301, 1)[0];
    let r = e.patch_behaviour(&[p], "Colour", &invert());
    assert_eq!(
        r.unwrap_err().to_string(),
        "Colour cannot be inverted: use its channels"
    );
}

#[test]
fn pat_44_a_virtual_dimmer_can_take_behaviour() {
    let mut e = engine();
    let p = add(&mut e, "par", 1, 301, 1)[0];
    e.patch_behaviour(&[p], "Dimmer", &high(0.5)).unwrap();
    assert_eq!(e.fixture(p).unwrap().behaviour["Dimmer"].high, 0.5);
}

#[test]
fn pat_44_a_cell_attribute_can_take_behaviour() {
    let mut e = engine();
    let bar = add(&mut e, "bar", 1, 401, 1)[0];
    e.patch_behaviour(&[bar], "White", &high(0.5)).unwrap();
    assert_eq!(e.fixture(bar).unwrap().behaviour["White"].high, 0.5);
}

#[test]
fn pat_44_reset_puts_every_field_back_and_leaves_the_swap_alone() {
    let (mut e, a, b) = rig();
    e.patch_behaviour(&[a, b], "Pan", &invert()).unwrap();
    e.patch_behaviour(&[a, b], "Zoom", &high(0.5)).unwrap();
    e.patch_swap_pan_tilt(&[a], true).unwrap();
    e.patch_behaviour_reset(&[a, b]).unwrap();
    for id in [a, b] {
        assert!(e.fixture(id).unwrap().behaviour.is_empty());
    }
    assert!(e.fixture(a).unwrap().swap_pan_tilt, "Titan leaves it alone");
}

#[test]
fn pat_44_a_bad_range_refuses() {
    let (mut e, a, _) = rig();
    assert!(matches!(
        e.patch_behaviour(&[], "Pan", &invert()),
        Err(CoreError::EmptySelection)
    ));
    assert!(matches!(
        e.patch_behaviour(&[a, a], "Pan", &invert()),
        Err(CoreError::RepeatedFixture(_))
    ));
    assert!(matches!(
        e.patch_behaviour_reset(&[9999]),
        Err(CoreError::UnknownFixture(9999))
    ));
    assert!(matches!(
        e.patch_swap_pan_tilt(&[9999], true),
        Err(CoreError::UnknownFixture(9999))
    ));
}

#[test]
fn pat_44_each_is_one_undo_step() {
    let (mut e, a, b) = rig();
    let before = e.edit_state();
    let mut h = History::default();
    h.edit(&mut e, "Behaviour 2 fixtures", None, |e| {
        e.patch_behaviour(&[a, b], "Pan", &invert())
    })
    .unwrap();
    assert_eq!(h.undo(&mut e).unwrap(), "Behaviour 2 fixtures");
    assert!(e.edit_state() == before);
    assert!(e.fixture(a).unwrap().behaviour.is_empty());
    h.redo(&mut e).unwrap();
    assert!(e.fixture(b).unwrap().behaviour["Pan"].invert);
}

// ---- PAT-45 -------------------------------------------------------------------------

#[test]
fn pat_45_pan_inverted_coarse_and_fine() {
    let (mut e, a, _) = rig();
    e.patch_behaviour(&[a], "Pan", &invert()).unwrap();
    e.set_programmer_value(a, "Pan", 0.25).unwrap();
    e.tick(0.0);
    // 0.75 * 65535 = 49151 = 0xBFFF
    assert_eq!((dmx(&e, 2), dmx(&e, 3)), (0xBF, 0xFF));
}

#[test]
fn pat_45_dimmer_under_a_high_limit_outputs_204() {
    let (mut e, a, _) = rig();
    e.patch_behaviour(&[a], "Dimmer", &high(0.8)).unwrap();
    e.set_programmer_value(a, "Dimmer", 1.0).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, 1), 204);
}

#[test]
fn pat_45_offset_clamps_at_full() {
    let (mut e, a, _) = rig();
    e.patch_behaviour(&[a], "Dimmer", &offset(0.1)).unwrap();
    e.set_programmer_value(a, "Dimmer", 0.95).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, 1), 255);
}

#[test]
fn pat_45_invert_then_offset_then_limits() {
    let (mut e, a, _) = rig();
    let edit = BehaviourEdit {
        invert: Some(true),
        offset: Some(-0.3),
        low: Some(0.6),
        ..Default::default()
    };
    e.patch_behaviour(&[a], "Dimmer", &edit).unwrap();
    e.set_programmer_value(a, "Dimmer", 0.2).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, 1), 153, "0.2 -> 0.8 -> 0.5 -> raised to 0.6");
}

#[test]
fn pat_45_a_profile_default_goes_through_it() {
    let (mut e, a, _) = rig();
    e.patch_behaviour(&[a], "Zoom", &invert()).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, 6), 191, "default 0.25 inverted");
}

#[test]
fn pat_45_locate_and_highlight_go_through_it() {
    let (mut e, a, _) = rig();
    e.patch_behaviour(&[a], "Dimmer", &high(0.8)).unwrap();
    e.select_item(Item::fixture(a)).unwrap();
    e.locate(&[AttributeGroup::I]).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, 1), 204, "Locate");
    e.clear();
    e.clear();
    e.select_item(Item::fixture(a)).unwrap();
    e.set_highlight(true).unwrap();
    e.tick(0.025);
    assert_eq!(dmx(&e, 1), 204, "Highlight");
}

#[test]
fn pat_45_an_effect_stays_inside_the_limits() {
    use cmdmx_core::effect::{EffectApply, EffectSet, Shape, Wave};
    let (mut e, a, _) = rig();
    e.patch_behaviour(&[a], "Dimmer", &high(0.5)).unwrap();
    e.select_item(Item::fixture(a)).unwrap();
    e.effect_apply(&EffectApply::Shape {
        shape: Shape::Wave(Wave::RampUp),
        attribute: None,
    })
    .unwrap();
    e.effect_set(EffectSet::Size(1.0)).unwrap();
    let mut highest = 0;
    for i in 0..400 {
        e.tick(f64::from(i) * 0.025);
        highest = highest.max(dmx(&e, 1));
    }
    assert_eq!(highest, 128, "reaches the limit and no further");
}

#[test]
fn pat_45_what_is_stored_never_changes() {
    let (mut e, a, _) = rig();
    e.patch_behaviour(&[a], "Dimmer", &high(0.5)).unwrap();
    e.set_programmer_value(a, "Dimmer", 1.0).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, 1), 128);
    e.patch_behaviour_reset(&[a]).unwrap();
    e.tick(0.025);
    assert_eq!(
        dmx(&e, 1),
        255,
        "the programmer may hold it above the limit"
    );
}

#[test]
fn pat_45_a_virtual_dimmers_scale_is_the_value_after_behaviour() {
    let mut e = engine();
    let p = add(&mut e, "par", 1, 301, 1)[0];
    e.patch_behaviour(&[p], "Dimmer", &high(0.5)).unwrap();
    e.set_programmer_value(p, "Dimmer", 1.0).unwrap();
    e.set_programmer_value(
        p,
        "Colour",
        TargetColour {
            r: 1.0,
            g: 0.0,
            b: 0.0,
        },
    )
    .unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, 1), 128);
}

#[test]
fn pat_45_a_cell_takes_the_fixtures_behaviour_once() {
    let mut e = engine();
    let bar = add(&mut e, "bar", 1, 401, 1)[0];
    e.patch_behaviour(&[bar], "White", &invert()).unwrap();
    // The whole fixture's level goes to every cell's slot.
    e.set_programmer_value(bar, "White", 0.25).unwrap();
    e.tick(0.0);
    assert_eq!((dmx(&e, 1), dmx(&e, 2)), (191, 191));
    // A cell with a value of its own merges alone.
    let cell = Item {
        fixture: bar,
        cell: Some(2),
    };
    e.set_item_value(cell, "White", 0.0).unwrap();
    e.tick(0.025);
    assert_eq!((dmx(&e, 1), dmx(&e, 2)), (191, 255));
}

#[test]
fn pat_45_an_auto_mixed_emitter_stays_inside_its_limit() {
    let mut e = engine();
    let w = add(&mut e, "rgbw", 1, 501, 1)[0];
    e.patch_behaviour(&[w], "White", &high(0.8)).unwrap();
    e.set_programmer_value(w, "Colour", TargetColour::WHITE)
        .unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, 5), 204, "the mix's own level is limited too");
}

#[test]
fn pat_45_low_above_high_from_a_hand_edited_file_does_not_panic() {
    let (mut e, a, _) = rig();
    e.patch_behaviour(&[a], "Dimmer", &high(0.5)).unwrap();
    let mut v: Json = serde_json::from_str(&e.to_show(meta()).to_json()).unwrap();
    v["patch"]["fixtures"][0]["behaviour"]["Dimmer"] = json!({"low": 0.9, "high": 0.1});
    let (show, _) = Show::from_json(&v.to_string()).unwrap();
    let (mut loaded, _) = Engine::from_show(&show);
    loaded.set_programmer_value(a, "Dimmer", 1.0).unwrap();
    loaded.tick(0.0); // show-proof: a value, whatever it is, never stops the tick
}

// ---- PAT-46 -------------------------------------------------------------------------

#[test]
fn pat_46_pan_goes_to_tilts_channels_and_tilt_to_pans_coarse_and_fine() {
    let (mut e, a, _) = rig();
    e.patch_swap_pan_tilt(&[a], true).unwrap();
    e.set_programmer_value(a, "Pan", 0.25).unwrap();
    e.set_programmer_value(a, "Tilt", 0.75).unwrap();
    e.tick(0.0);
    // Tilt 0.75 on Pan's 2/3, Pan 0.25 on Tilt's 4/5 (0.25 * 65535 = 16384 = 0x4000).
    assert_eq!((dmx(&e, 2), dmx(&e, 3)), (0xBF, 0xFF));
    assert_eq!((dmx(&e, 4), dmx(&e, 5)), (0x40, 0x00));
    e.patch_swap_pan_tilt(&[a], false).unwrap();
    e.tick(0.025);
    assert_eq!((dmx(&e, 2), dmx(&e, 3)), (0x40, 0x00));
    assert_eq!((dmx(&e, 4), dmx(&e, 5)), (0xBF, 0xFF));
}

#[test]
fn pat_46_the_swap_comes_after_each_attributes_own_behaviour() {
    let (mut e, a, _) = rig();
    e.patch_behaviour(&[a], "Pan", &invert()).unwrap();
    e.patch_swap_pan_tilt(&[a], true).unwrap();
    e.set_programmer_value(a, "Pan", 0.25).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, 4), 0xBF, "Pan inverted, then on Tilt's channel");
}

#[test]
fn pat_46_a_fixture_without_both_refuses_the_whole_command() {
    let mut e = engine();
    let m = add(&mut e, "mover", 1, 101, 1)[0];
    let s = add(&mut e, "spot", 1, 201, 20)[0];
    let r = e.patch_swap_pan_tilt(&[m, s], true);
    assert!(
        matches!(&r, Err(CoreError::UnknownAttribute { fixture: 201, attribute }) if attribute == "Pan"),
        "{r:?}"
    );
    assert!(!e.fixture(m).unwrap().swap_pan_tilt, "nothing changes");
}

#[test]
fn pat_46_is_one_undo_step() {
    let (mut e, a, _) = rig();
    let mut h = History::default();
    h.edit(&mut e, "Swap pan/tilt 101", None, |e| {
        e.patch_swap_pan_tilt(&[a], true)
    })
    .unwrap();
    assert_eq!(h.undo(&mut e).unwrap(), "Swap pan/tilt 101");
    assert!(!e.fixture(a).unwrap().swap_pan_tilt);
}

// ---- PAT-47 -------------------------------------------------------------------------

#[test]
fn pat_47_the_show_file_has_them_only_when_set_and_they_survive_a_reopen() {
    let (mut e, a, b) = rig();
    e.patch_behaviour(&[a], "Pan", &invert()).unwrap();
    e.patch_behaviour(&[a], "Zoom", &offset(-0.5)).unwrap();
    e.patch_swap_pan_tilt(&[a], true).unwrap();
    let v: Json = serde_json::from_str(&e.to_show(meta()).to_json()).unwrap();
    let text = v.to_string();
    assert!(
        text.contains(r#""behaviour":{"Pan":{"invert":true},"Zoom":{"offset":-0.5}}"#),
        "{text}"
    );
    assert_eq!(
        text.matches("swap_pan_tilt").count(),
        1,
        "absent at its default"
    );
    let mut reopened = reopen(&e);
    assert_eq!(
        reopened.fixture(a).unwrap().behaviour,
        e.fixture(a).unwrap().behaviour
    );
    assert!(reopened.fixture(a).unwrap().swap_pan_tilt);
    assert!(reopened.fixture(b).unwrap().behaviour.is_empty());
    reopened.tick(0.0);
    assert_eq!(
        dmx(&reopened, 6),
        0,
        "Zoom 0.25 with offset -0.5, after the reopen"
    );
}

#[test]
fn pat_47_schema_12_migrates_and_changes_nothing() {
    let (e, a, _) = rig();
    assert_eq!(SCHEMA_VERSION, 15);
    let mut v: Json = serde_json::from_str(&e.to_show(meta()).to_json()).unwrap();
    v["schema_version"] = json!(12);
    let (show, migrations) = Show::from_json(&v.to_string()).unwrap();
    assert_eq!(migrations.len(), 3, "{migrations:?}");
    assert!(migrations[0].starts_with("12 -> 13"), "{migrations:?}");
    let (loaded, _) = Engine::from_show(&show);
    assert!(loaded.fixture(a).unwrap().behaviour.is_empty());
    assert!(!loaded.fixture(a).unwrap().swap_pan_tilt);
}

#[test]
fn pat_47_a_mode_change_keeps_what_remains_and_drops_the_rest() {
    let (mut e, a, _) = rig();
    e.patch_behaviour(&[a], "Pan", &invert()).unwrap();
    e.patch_behaviour(&[a], "Zoom", &high(0.5)).unwrap();
    e.patch_swap_pan_tilt(&[a], true).unwrap();
    e.patch_change_mode(a, "Short").unwrap();
    let f = e.fixture(a).unwrap();
    assert!(f.behaviour["Pan"].invert);
    assert!(!f.behaviour.contains_key("Zoom"));
    assert!(!f.swap_pan_tilt, "Short has no Tilt to swap with");
    e.set_programmer_value(a, "Pan", 0.25).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, 2), 191, "Pan plays on its own channel, inverted");
}

#[test]
fn pat_47_an_exchange_keeps_what_remains_and_drops_the_rest() {
    let (mut e, a, _) = rig();
    e.patch_behaviour(&[a], "Dimmer", &high(0.5)).unwrap();
    e.patch_behaviour(&[a], "Pan", &invert()).unwrap();
    e.patch_exchange(&[a], "spot", "Two", None).unwrap();
    let f = e.fixture(a).unwrap();
    assert_eq!(f.behaviour["Dimmer"].high, 0.5);
    assert!(!f.behaviour.contains_key("Pan"));
}

#[test]
fn pat_47_a_copy_takes_it_along() {
    let (mut e, a, _) = rig();
    e.patch_behaviour(&[a], "Pan", &invert()).unwrap();
    e.patch_swap_pan_tilt(&[a], true).unwrap();
    let c = e.patch_copy(&[a], None).unwrap()[0];
    let f = e.fixture(c).unwrap();
    assert!(f.behaviour["Pan"].invert);
    assert!(f.swap_pan_tilt);
}
