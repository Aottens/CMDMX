//! Update chooses its targets (PBK-49, IPC-57).
//!
//! API these tests fix:
//! - `Engine::update_targets() -> UpdateTargets { playbacks: Vec<UpdateTarget>, palettes: Vec<u32> }`
//!   and `UpdateTarget { id, cue }` (in `playback`): what the softkeys offer.
//! - `Engine::update(&[UpdateTarget], &[u32], Option<UpdateMode>) -> Result<(), CoreError>`: stores
//!   into all of them; a refusal changes nothing.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

mod common;

use std::collections::BTreeMap;

use cmdmx_core::attribute::AttributeGroup;
use cmdmx_core::engine::Engine;
use cmdmx_core::palette::{Cell, Record, RecordAs, Window};
use cmdmx_core::playback::{
    Cue, CueNumber, ListAction, ListOptions, Playback, PlaybackChange, UpdateMode, UpdateTarget,
    Values,
};
use cmdmx_core::programmer::Item;
use cmdmx_core::undo::History;
use cmdmx_core::value::{Reference, Stored, Value};
use common::{church, dmx, select};

fn q(n: f64) -> CueNumber {
    CueNumber::new(n).unwrap()
}

fn target(id: u32, n: f64) -> UpdateTarget {
    UpdateTarget { id, cue: q(n) }
}

fn cue(n: f64, entries: &[(u32, &str, Stored)]) -> Cue {
    let mut v: Values = BTreeMap::new();
    for (id, name, stored) in entries {
        v.entry((*id).into())
            .or_default()
            .insert((*name).to_string(), *stored);
    }
    Cue {
        number: q(n),
        ..Cue::new(v)
    }
}

fn level(l: f32) -> Stored {
    Stored::Value(Value::Level(l))
}

fn add(e: &mut Engine, id: u32, slot: u16, cues: Vec<Cue>) {
    e.add_playback(Playback {
        cues,
        ..Playback::new(id, 1, slot, Values::new())
    });
}

/// Palette on Beams `handle`, held per fixture for exactly `ids`, Dimmer at `l`. Gives (id, number).
fn dim_palette(e: &mut Engine, ids: &[u32], l: f32, handle: u16) -> (u32, u32) {
    clean(e);
    select(e, ids);
    for &id in ids {
        e.set_programmer_value(id, "Dimmer", l).unwrap();
    }
    let id = e
        .record_palette(&Record {
            mask: Some(vec![AttributeGroup::I]),
            record_as: RecordAs::Normal,
            ..Record::at(Cell::new(Window::Beams, handle))
        })
        .unwrap();
    clean(e);
    let number = (e.palettes().find(|p| p.id == id)).unwrap().number;
    (id, number)
}

fn reference(palette: u32, l: f32) -> Stored {
    Stored::Reference(Reference {
        palette,
        value: Value::Level(l),
    })
}

fn clean(e: &mut Engine) {
    e.clear_selection();
    e.clear();
}

fn program(e: &mut Engine, id: u32, attribute: &str, l: f32) {
    select(e, &[id]);
    e.set_programmer_value(id, attribute, l).unwrap();
}

fn go(e: &mut Engine, id: u32, times: u32) {
    for i in 0..times {
        let now = f64::from(i) * 0.1;
        e.cuelist(Some(id), ListAction::Go, now).unwrap();
        e.tick(now);
    }
}

fn stored(e: &Engine, id: u32, n: f64, fixture: u32, attribute: &str) -> Option<Stored> {
    let p = e.playback(id).unwrap();
    let cue = p.cues.iter().find(|c| c.number == q(n)).unwrap();
    let values = cue.values.get(&Item::fixture(fixture))?;
    values.get(attribute).copied()
}

fn no_tracking(e: &mut Engine, id: u32) {
    e.change_playback(
        id,
        &PlaybackChange {
            list: Some(ListOptions {
                tracking: false,
                ..ListOptions::default()
            }),
            ..PlaybackChange::default()
        },
    )
    .unwrap();
}

/// List 1 on 1.1: Q1 par 201 at 50 %, Q2 par 202 full, Q3 par 201 at 80 %.
fn list(e: &mut Engine) {
    add(
        e,
        1,
        1,
        vec![
            cue(1.0, &[(201, "Dimmer", level(0.5))]),
            cue(2.0, &[(202, "Dimmer", level(1.0))]),
            cue(3.0, &[(201, "Dimmer", level(0.8))]),
        ],
    );
}

#[test]
fn pbk_49_targets_are_the_active_playbacks_whose_current_cue_holds_a_touched_attribute() {
    let mut e = church();
    list(&mut e);
    add(&mut e, 2, 2, vec![cue(1.0, &[(201, "Dimmer", level(0.7))])]);
    add(&mut e, 3, 3, vec![cue(1.0, &[(204, "Dimmer", level(0.7))])]);
    add(&mut e, 4, 4, vec![cue(1.0, &[(201, "Dimmer", level(0.7))])]);
    go(&mut e, 1, 2); // Q2: 201 tracks from Q1
    e.set_fader(2, 1.0, 0.2).unwrap();
    e.set_fader(3, 1.0, 0.2).unwrap();
    e.tick(0.3);
    program(&mut e, 201, "Dimmer", 0.3);
    let got = e.update_targets();
    assert_eq!(got.playbacks, [target(1, 2.0), target(2, 1.0)]);
    assert!(got.palettes.is_empty());
}

#[test]
fn pbk_49_without_tracking_only_what_the_cue_itself_holds_and_never_what_is_off() {
    let mut e = church();
    list(&mut e);
    no_tracking(&mut e, 1);
    go(&mut e, 1, 2);
    program(&mut e, 201, "Dimmer", 0.3);
    assert!(
        e.update_targets().playbacks.is_empty(),
        "tracked is not held"
    );
    let mut e = church();
    list(&mut e);
    no_tracking(&mut e, 1);
    go(&mut e, 1, 1);
    e.set_cue_off(1, q(1.0), Item::fixture(201), Some("Dimmer"), true)
        .unwrap();
    program(&mut e, 201, "Dimmer", 0.3);
    assert!(
        e.update_targets().playbacks.is_empty(),
        "dormant is not held"
    );
}

#[test]
fn pbk_49_palettes_hold_it_for_that_fixture_and_those_the_cues_reference_come_first() {
    let mut e = church();
    let (_, plain) = dim_palette(&mut e, &[201], 0.2, 1);
    let (used, referenced) = dim_palette(&mut e, &[201], 0.3, 2);
    let (_, other) = dim_palette(&mut e, &[203], 0.4, 3);
    add(
        &mut e,
        1,
        1,
        vec![
            cue(1.0, &[(201, "Dimmer", reference(used, 0.3))]),
            cue(2.0, &[(202, "Dimmer", level(1.0))]),
        ],
    );
    go(&mut e, 1, 1);
    program(&mut e, 201, "Dimmer", 0.6);
    let got = e.update_targets();
    assert_eq!(got.playbacks, [target(1, 1.0)]);
    assert_eq!(got.palettes, [referenced, plain], "{other} holds 203 only");
}

#[test]
fn pbk_49_a_ticked_cue_takes_it_as_pbk_13_or_cue_52_says() {
    let mut e = church();
    list(&mut e);
    program(&mut e, 201, "Dimmer", 0.3);
    e.update(&[target(1, 2.0)], &[], None).unwrap();
    assert_eq!(
        stored(&e, 1, 2.0, 201, "Dimmer"),
        Some(level(0.3)),
        "Forwards"
    );
    let mut e = church();
    list(&mut e);
    program(&mut e, 201, "Dimmer", 0.3);
    e.update(&[target(1, 2.0)], &[], Some(UpdateMode::Backwards))
        .unwrap();
    assert_eq!(stored(&e, 1, 1.0, 201, "Dimmer"), Some(level(0.3)));
    assert_eq!(stored(&e, 1, 2.0, 201, "Dimmer"), None, "still tracks");
    // Update stores only what the cue holds: a fixture it never held stays out.
    let mut e = church();
    list(&mut e);
    program(&mut e, 205, "Dimmer", 0.3);
    also(&mut e, 201, "Dimmer", 0.4);
    e.update(&[target(1, 1.0)], &[], None).unwrap();
    assert_eq!(stored(&e, 1, 1.0, 205, "Dimmer"), None);
    assert_eq!(stored(&e, 1, 1.0, 201, "Dimmer"), Some(level(0.4)));
}

fn also(e: &mut Engine, id: u32, attribute: &str, l: f32) {
    select(e, &[id]);
    e.set_programmer_value(id, attribute, l).unwrap();
}

#[test]
fn pbk_49_a_ticked_palette_takes_what_it_holds_and_the_cue_keeps_its_reference() {
    let mut e = church();
    let (used, number) = dim_palette(&mut e, &[201], 0.3, 1);
    add(
        &mut e,
        1,
        1,
        vec![
            cue(
                1.0,
                &[
                    (201, "Dimmer", reference(used, 0.3)),
                    (202, "Dimmer", level(0.5)),
                ],
            ),
            cue(2.0, &[(203, "Dimmer", level(1.0))]),
        ],
    );
    go(&mut e, 1, 1);
    e.set_fader(1, 1.0, 0.1).unwrap();
    program(&mut e, 201, "Dimmer", 0.4);
    also(&mut e, 202, "Dimmer", 0.6);
    e.update(&[target(1, 1.0)], &[number], None).unwrap();
    let kept = stored(&e, 1, 1.0, 201, "Dimmer");
    assert!(
        matches!(kept, Some(Stored::Reference(r)) if r.palette == used),
        "{kept:?}"
    );
    assert_eq!(stored(&e, 1, 1.0, 202, "Dimmer"), Some(level(0.6)));
    let palette = e.palette(number).unwrap();
    let held = &palette.fixtures[&Item::fixture(201)];
    assert_eq!(held["Dimmer"], Value::Level(0.4));
    assert!(!palette.fixtures.contains_key(&Item::fixture(202)));
    clean(&mut e);
    e.tick(0.5);
    assert_eq!(dmx(&e, 21), 102, "on stage through the reference");
    assert_eq!(dmx(&e, 22), 153);
}

#[test]
fn pbk_49_a_palette_not_ticked_is_overwritten_by_a_value_in_the_cue() {
    let mut e = church();
    let (used, number) = dim_palette(&mut e, &[201], 0.3, 1);
    add(
        &mut e,
        1,
        1,
        vec![
            cue(1.0, &[(201, "Dimmer", reference(used, 0.3))]),
            cue(2.0, &[(203, "Dimmer", level(1.0))]),
        ],
    );
    program(&mut e, 201, "Dimmer", 0.4);
    e.update(&[target(1, 1.0)], &[], None).unwrap();
    assert_eq!(stored(&e, 1, 1.0, 201, "Dimmer"), Some(level(0.4)));
    let held = &e.palette(number).unwrap().fixtures[&Item::fixture(201)];
    assert_eq!(
        held["Dimmer"],
        Value::Level(0.3),
        "the palette keeps its own"
    );
}

#[test]
fn pbk_49_only_a_palette_is_enough() {
    let mut e = church();
    let (_, number) = dim_palette(&mut e, &[201], 0.3, 1);
    program(&mut e, 201, "Dimmer", 0.4);
    e.update(&[], &[number], None).unwrap();
    let held = &e.palette(number).unwrap().fixtures[&Item::fixture(201)];
    assert_eq!(held["Dimmer"], Value::Level(0.4));
}

#[test]
fn pbk_49_a_value_stored_comes_on_and_what_is_off_is_not_read() {
    let mut e = church();
    list(&mut e);
    e.set_cue_off(1, q(3.0), Item::fixture(201), Some("Dimmer"), true)
        .unwrap();
    // Tracking: 201 is in Q3's state through Q1, so it is stored and no longer off.
    program(&mut e, 201, "Dimmer", 0.3);
    e.update(&[target(1, 3.0)], &[], None).unwrap();
    assert_eq!(stored(&e, 1, 3.0, 201, "Dimmer"), Some(level(0.3)));
    let p = e.playback(1).unwrap();
    assert!(p.cues[2].dormant.is_empty(), "never both on and off");
    // Without tracking: Q3 does not hold it, so nothing goes in and it stays off.
    let mut e = church();
    list(&mut e);
    no_tracking(&mut e, 1);
    e.set_cue_off(1, q(3.0), Item::fixture(201), Some("Dimmer"), true)
        .unwrap();
    program(&mut e, 201, "Dimmer", 0.3);
    assert!(e.update(&[target(1, 3.0)], &[], None).is_err());
    assert_eq!(stored(&e, 1, 3.0, 201, "Dimmer"), None);
    assert!(!e.playback(1).unwrap().cues[2].dormant.is_empty());
}

#[test]
fn pbk_49_refusals_change_nothing() {
    let mut e = church();
    list(&mut e);
    let (_, number) = dim_palette(&mut e, &[201], 0.3, 1);
    assert!(e.update(&[target(1, 1.0)], &[], None).is_err(), "empty");
    program(&mut e, 201, "Dimmer", 0.4);
    assert!(e.update(&[], &[], None).is_err(), "no targets");
    for (cues, palettes) in [
        (vec![target(1, 1.0), target(99, 1.0)], vec![number]),
        (vec![target(1, 1.0), target(1, 9.0)], vec![number]),
        (vec![target(1, 1.0)], vec![number, 99]),
    ] {
        assert!(e.update(&cues, &palettes, None).is_err(), "{cues:?}");
        assert_eq!(stored(&e, 1, 1.0, 201, "Dimmer"), Some(level(0.5)));
        let held = &e.palette(number).unwrap().fixtures[&Item::fixture(201)];
        assert_eq!(held["Dimmer"], Value::Level(0.3));
    }
}

#[test]
fn pbk_49_it_is_one_undo_step() {
    let mut e = church();
    list(&mut e);
    let (_, number) = dim_palette(&mut e, &[202], 0.3, 1);
    program(&mut e, 201, "Dimmer", 0.4);
    also(&mut e, 202, "Dimmer", 0.9);
    let mut h = History::default();
    h.edit(&mut e, "Update", None, |e| {
        e.update(&[target(1, 1.0), target(1, 3.0)], &[number], None)
    })
    .unwrap();
    assert_eq!(stored(&e, 1, 1.0, 201, "Dimmer"), Some(level(0.4)));
    h.undo(&mut e).unwrap();
    assert_eq!(stored(&e, 1, 1.0, 201, "Dimmer"), Some(level(0.5)));
    assert_eq!(stored(&e, 1, 3.0, 201, "Dimmer"), Some(level(0.8)));
    let held = &e.palette(number).unwrap().fixtures[&Item::fixture(202)];
    assert_eq!(held["Dimmer"], Value::Level(0.3));
}
