//! Spreading features read the target positions (SEL-02, P2-02): `specs/modules/programmer.md`
//! PRG-76..78 and the Definition *Target positions*; `specs/modules/effects.md` FX-45 and the
//! FX-06/07/14 amendments (`flip`). Pars 201–208 are one-channel dimmers at 21–28; washes
//! 101–104 (Dimmer, Pan 16-bit, Tilt, Zoom) at 1, 6, 11, 16, plus 105 @41 and 106 @46 here.
//!
//! Target positions: with a stage selected the list's items take *k* = *i*; otherwise
//! `effect::positions(len, grid)` over the list (grid off → *k* = *i*). A fan list or a palette
//! that skips items takes its positions over its own list ("over that list").
//!
//! API under test (new in SEL-02):
//! - `effect::Params::flip: bool` (FX-06, default false; serialised only when true until schema 9),
//!   `EffectSet::Flip(bool)`, FX-07's step 4 in `Entry` positions.
//! - `EffectSet::Order(programmer::Order)` (FX-45, name "order"): `Reverse`, `Shuffle` (the
//!   engine's shuffle counter, PRG-72), `Sort` (user number); `Revert` and fewer than two members
//!   are refused and change nothing.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

mod common;

use std::collections::BTreeMap;

use cmdmx_core::effect::{EffectApply, EffectSet, Entry, EntryId, Params, LOOKS};
use cmdmx_core::engine::Engine;
use cmdmx_core::palette::{GlobalValue, Palette, Timing, Window};
use cmdmx_core::patch::{Fixture, FixtureId};
use cmdmx_core::programmer::{FanCurve, GridShortcut, Item, Order, Resolution::Coarse};
use cmdmx_core::undo::History;
use cmdmx_core::value::Value;
use common::{church, dmx, select};

const PARS: [FixtureId; 8] = [201, 202, 203, 204, 205, 206, 207, 208];
const WASHES: [FixtureId; 6] = [101, 102, 103, 104, 105, 106];

fn rig() -> Engine {
    let mut e = church();
    for (id, address) in [(105, 41), (106, 46)] {
        e.patch(Fixture {
            id,
            number: 0,
            name: String::new(),
            handle: None,
            profile: "wash".into(),
            mode: None,
            universe: 1,
            address,
        })
        .unwrap();
    }
    e
}

fn level(e: &Engine, id: FixtureId, attribute: &str) -> f32 {
    match e.programmer().value(id, attribute) {
        Some(Value::Level(v)) => v,
        other => panic!("{id} {attribute}: expected a level, got {other:?}"),
    }
}

fn levels(e: &Engine, ids: &[FixtureId], attribute: &str) -> Vec<f32> {
    ids.iter().map(|&id| level(e, id, attribute)).collect()
}

fn assert_close(actual: &[f32], expected: &[f32]) {
    assert_eq!(actual.len(), expected.len(), "{actual:?} vs {expected:?}");
    for (a, b) in actual.iter().zip(expected) {
        assert!(
            (a - b).abs() < 1e-4,
            "expected {expected:?}, got {actual:?}"
        );
    }
}

fn set_all(e: &mut Engine, ids: &[FixtureId], attribute: &str, v: f32) {
    for &id in ids {
        e.set_programmer_value(id, attribute, v).unwrap();
    }
}

fn items(ids: &[FixtureId]) -> Vec<Item> {
    ids.iter().map(|&id| Item::fixture(id)).collect()
}

fn look(name: &str) -> EffectApply {
    EffectApply::Look(name.into())
}

fn entry(e: &Engine, id: EntryId) -> &Entry {
    e.effects().iter().find(|x| x.id == id).unwrap()
}

fn pars_dmx(e: &Engine) -> Vec<u8> {
    (21..=28).map(|a| dmx(e, a)).collect()
}

// ---- PRG-76 -----------------------------------------------------------------

#[test]
fn prg_76_outside_in_fans_both_ends_alike() {
    let mut e = rig();
    set_all(&mut e, &WASHES, "Pan", 0.5);
    select(&mut e, &WASHES);
    e.grid_shortcut(GridShortcut::OutsideIn).unwrap();
    e.select_all(); // ends the stage, keeps the grid
    e.set_fan(Some(FanCurve::Line));
    e.turn_wheel("Pan", 10, Coarse).unwrap(); // +0.40
    assert_close(
        &levels(&e, &WASHES, "Pan"),
        &[0.1, 0.5, 0.9, 0.9, 0.5, 0.1],
    );
}

#[test]
fn prg_76_odd_against_even() {
    let mut e = rig();
    set_all(&mut e, &PARS, "Dimmer", 0.5);
    select(&mut e, &PARS);
    e.grid_shortcut(GridShortcut::OddEven).unwrap();
    e.select_all();
    e.set_fan(Some(FanCurve::Line));
    e.turn_wheel("Dimmer", 10, Coarse).unwrap();
    assert_close(
        &levels(&e, &PARS, "Dimmer"),
        &[0.1, 0.9, 0.1, 0.9, 0.1, 0.9, 0.1, 0.9],
    );
}

#[test]
fn prg_76_grid_off_is_prg_57_unchanged() {
    let mut e = rig();
    let five = [101, 102, 103, 104, 105];
    set_all(&mut e, &five, "Pan", 0.5);
    select(&mut e, &five);
    e.set_fan(Some(FanCurve::Line));
    e.turn_wheel("Pan", 10, Coarse).unwrap();
    assert_close(&levels(&e, &five, "Pan"), &[0.1, 0.3, 0.5, 0.7, 0.9]);
}

#[test]
fn prg_76_a_stage_fans_over_its_items_plainly() {
    let mut e = rig();
    set_all(&mut e, &PARS, "Dimmer", 0.5);
    select(&mut e, &PARS);
    e.grid_shortcut(GridShortcut::OddEven).unwrap(); // stage 0: 201, 203, 205, 207
    e.set_fan(Some(FanCurve::Line));
    e.turn_wheel("Dimmer", 10, Coarse).unwrap();
    let third = 0.4 / 3.0;
    assert_close(
        &levels(&e, &[201, 203, 205, 207], "Dimmer"),
        &[0.1, 0.5 - third, 0.5 + third, 0.9],
    );
    assert_close(&levels(&e, &[202, 204, 206, 208], "Dimmer"), &[0.5; 4]);
}

#[test]
fn prg_76_one_position_acts_as_without_fan() {
    let mut e = rig();
    let four = [201, 202, 203, 204];
    set_all(&mut e, &four, "Dimmer", 0.5);
    select(&mut e, &four);
    e.grid_shortcut(GridShortcut::Blocks(4)).unwrap(); // N = 1
    e.select_all();
    e.set_fan(Some(FanCurve::Line));
    e.turn_wheel("Dimmer", 10, Coarse).unwrap();
    assert_close(&levels(&e, &four, "Dimmer"), &[0.9; 4]);
}

#[test]
fn prg_76_positions_run_over_the_fan_list() {
    // Pars have no Pan: the fan list is the four washes, Outside in over that list → 0 1 1 0.
    let mut e = rig();
    let washes = [101, 102, 103, 104];
    set_all(&mut e, &washes, "Pan", 0.5);
    select(&mut e, &[201, 101, 102, 103, 104, 202]);
    e.grid_shortcut(GridShortcut::OutsideIn).unwrap();
    e.select_all();
    e.set_fan(Some(FanCurve::Line));
    e.turn_wheel("Pan", 10, Coarse).unwrap();
    assert_close(&levels(&e, &washes, "Pan"), &[0.1, 0.9, 0.9, 0.1]);
}

// ---- PRG-77, FX-14 amended ----------------------------------------------------

#[test]
fn prg_77_members_are_the_targets_in_target_order() {
    let mut e = rig();
    select(&mut e, &[201, 202, 203, 204]);
    e.order(Order::Reverse).unwrap();
    let id = e.effect_apply(&look("Dimmer chase")).unwrap();
    assert_eq!(entry(&e, id).members, items(&[204, 203, 202, 201]));
    e.tick(0.0);
    assert_eq!(dmx(&e, 24), 255, "204 leads");
    assert_eq!(dmx(&e, 21), 0);
}

#[test]
fn prg_77_a_new_entry_takes_the_grid() {
    let mut e = rig();
    select(&mut e, &PARS);
    e.grid_shortcut(GridShortcut::OutsideIn).unwrap();
    e.select_all();
    let id = e.effect_apply(&look("Dimmer chase")).unwrap();
    let x = entry(&e, id);
    let p = x.params;
    assert_eq!((p.wings, p.groups, p.blocks, p.flip), (2, 0, 1, false));
    assert_eq!(
        (x.positions.clone(), x.count),
        (vec![0, 1, 2, 3, 3, 2, 1, 0], 4)
    );
    assert!((p.width - 0.25).abs() < 1e-6, "FX-30: 100 % / N after the grid");
    e.tick(0.0);
    assert_eq!(
        pars_dmx(&e),
        [255, 0, 0, 0, 0, 0, 0, 255],
        "from both ends"
    );
}

#[test]
fn prg_77_inside_out_takes_flip() {
    let mut e = rig();
    select(&mut e, &PARS);
    e.grid_shortcut(GridShortcut::InsideOut).unwrap();
    e.select_all();
    let id = e.effect_apply(&look("Dimmer chase")).unwrap();
    let x = entry(&e, id);
    assert!(x.params.flip);
    assert_eq!(x.positions, [3, 2, 1, 0, 0, 1, 2, 3]);
    e.tick(0.0);
    assert_eq!(pars_dmx(&e), [0, 0, 0, 255, 255, 0, 0, 0], "from the middle");
}

#[test]
fn prg_77_one_in_three() {
    let mut e = rig();
    select(&mut e, &PARS);
    e.grid_shortcut(GridShortcut::OneIn(3)).unwrap();
    e.select_all();
    let id = e.effect_apply(&look("Dimmer wave")).unwrap();
    let x = entry(&e, id);
    assert_eq!((x.params.groups, x.count), (3, 3));
    assert_eq!(x.positions, [0, 1, 2, 0, 1, 2, 0, 1]);
}

#[test]
fn prg_77_fan_sweep_swaps_its_wings_for_the_grid() {
    let mut e = rig();
    select(&mut e, &WASHES);
    e.grid_shortcut(GridShortcut::OddEven).unwrap();
    e.select_all();
    let id = e.effect_apply(&look("Fan sweep")).unwrap();
    let want = LOOKS.iter().find(|l| l.name == "Fan sweep").unwrap().params;
    let p = entry(&e, id).params;
    assert_eq!((p.wings, p.groups, p.blocks, p.flip), (1, 2, 1, false));
    assert_eq!(
        Params {
            wings: want.wings,
            groups: want.groups,
            blocks: want.blocks,
            flip: want.flip,
            ..p
        },
        want,
        "spread, size, bpm and the rest stay the look's"
    );
}

#[test]
fn prg_77_with_a_stage_selected_the_look_defaults_stay() {
    let mut e = rig();
    select(&mut e, &PARS);
    e.grid_shortcut(GridShortcut::OddEven).unwrap(); // stage 0
    let id = e.effect_apply(&look("Dimmer chase")).unwrap();
    let x = entry(&e, id);
    assert_eq!(x.members, items(&[201, 203, 205, 207]));
    assert_eq!((x.params.wings, x.params.groups), (1, 0));
    assert_eq!((x.positions.clone(), x.count), (vec![0, 1, 2, 3], 4));
}

#[test]
fn prg_77_grid_off_keeps_the_look_defaults() {
    let mut e = rig();
    select(&mut e, &WASHES);
    let id = e.effect_apply(&look("Fan sweep")).unwrap();
    let want = LOOKS.iter().find(|l| l.name == "Fan sweep").unwrap().params;
    assert_eq!(entry(&e, id).params, want);
}

// ---- FX-06 / FX-07 amended: flip ---------------------------------------------

#[test]
fn fx_06_flip_defaults_to_false_and_is_saved_only_when_on() {
    assert!(!Params::DEFAULT.flip);
    let off = serde_json::to_value(Params::DEFAULT).unwrap();
    assert!(off.get("flip").is_none(), "absent when false: {off}");
    let on = serde_json::to_value(Params {
        flip: true,
        ..Params::DEFAULT
    })
    .unwrap();
    assert_eq!(on.get("flip"), Some(&serde_json::Value::Bool(true)));
    let back: Params = serde_json::from_value(on).unwrap();
    assert!(back.flip);
}

#[test]
fn fx_07_flip_reverses_the_positions() {
    let mut e = rig();
    select(&mut e, &PARS);
    let id = e.effect_apply(&look("Dimmer chase")).unwrap();
    e.effect_set(EffectSet::Wings(2)).unwrap();
    e.effect_set(EffectSet::Flip(true)).unwrap();
    let x = entry(&e, id);
    assert!(x.params.flip);
    assert_eq!(
        (x.positions.clone(), x.count),
        (vec![3, 2, 1, 0, 0, 1, 2, 3], 4)
    );
    assert_eq!(EffectSet::Flip(true).name(), "flip");
}

// ---- PRG-78, PAL-14 amended ----------------------------------------------------

/// Palette 81: Dimmer full, global.
fn with_full(e: &mut Engine) {
    e.add_palette(Palette {
        id: 1,
        number: 81,
        window: Window::Beams,
        handle: 1,
        legend: String::new(),
        icon: None,
        global: BTreeMap::from([(
            "Dimmer".to_string(),
            GlobalValue::Value(Value::Level(1.0)),
        )]),
        shared: BTreeMap::new(),
        shared_cells: BTreeMap::new(),
        fixtures: BTreeMap::new(),
        effects: Vec::new(),
    })
    .unwrap();
}

fn timed(time: f32, overlap: f32) -> Timing {
    Timing {
        time: Some(time),
        overlap: Some(overlap),
    }
}

fn near(got: u8, want: u8) -> bool {
    got.abs_diff(want) <= 1
}

#[test]
fn prg_78_palette_overlap_runs_on_positions() {
    let mut e = rig();
    with_full(&mut e);
    select(&mut e, &PARS);
    e.grid_shortcut(GridShortcut::OutsideIn).unwrap();
    e.select_all();
    e.apply_palette(81, timed(2.0, 0.0)).unwrap();
    e.tick(0.0);
    e.tick(1.0);
    let d = pars_dmx(&e);
    assert!(near(d[0], 128) && near(d[7], 128), "201 and 208 start at 0 s: {d:?}");
    assert_eq!(&d[1..7], &[0; 6]);
    e.tick(3.0);
    let d = pars_dmx(&e);
    assert!(near(d[1], 128) && near(d[6], 128), "202 and 207 at 2 s: {d:?}");
    assert_eq!(&d[2..6], &[0; 4]);
    e.tick(7.0);
    let d = pars_dmx(&e);
    assert!(near(d[3], 128) && near(d[4], 128), "204 and 205 at 6 s: {d:?}");
    e.tick(8.0);
    assert_eq!(pars_dmx(&e), [255; 8], "all done at 8 s");
}

#[test]
fn prg_78_grid_off_is_pal_14_unchanged() {
    let mut e = rig();
    with_full(&mut e);
    select(&mut e, &[201, 202, 203]);
    e.apply_palette(81, timed(2.0, 0.0)).unwrap();
    e.tick(0.0);
    e.tick(3.0);
    let d = pars_dmx(&e);
    assert_eq!(d[0], 255);
    assert!(near(d[1], 128), "{d:?}");
    assert_eq!(d[2], 0);
}

#[test]
fn prg_78_a_stage_staggers_its_items_plainly() {
    let mut e = rig();
    with_full(&mut e);
    select(&mut e, &PARS);
    e.grid_shortcut(GridShortcut::OddEven).unwrap(); // stage 0: 201, 203, 205, 207
    e.apply_palette(81, timed(2.0, 0.0)).unwrap();
    e.tick(0.0);
    e.tick(3.0);
    let d = pars_dmx(&e);
    assert_eq!(d[0], 255, "201 first: {d:?}");
    assert!(near(d[2], 128), "203 second: {d:?}");
    assert_eq!((d[4], d[6]), (0, 0));
}

#[test]
fn prg_78_positions_run_over_the_items_the_palette_touches() {
    // 201 and 202 are Dimmer-only; a Pan palette touches only the washes, Outside in over
    // them → 0 1 1 0.
    let mut e = rig();
    set_all(&mut e, &[101, 102, 103, 104], "Pan", 0.0);
    e.add_palette(Palette {
        id: 2,
        number: 41,
        window: Window::Positions,
        handle: 2,
        legend: String::new(),
        icon: None,
        global: BTreeMap::from([("Pan".to_string(), GlobalValue::Value(Value::Level(1.0)))]),
        shared: BTreeMap::new(),
        shared_cells: BTreeMap::new(),
        fixtures: BTreeMap::new(),
        effects: Vec::new(),
    })
    .unwrap();
    select(&mut e, &[201, 101, 102, 103, 104, 202]);
    e.grid_shortcut(GridShortcut::OutsideIn).unwrap();
    e.select_all();
    e.apply_palette(41, timed(2.0, 0.0)).unwrap();
    e.tick(0.0);
    e.tick(2.0);
    let pan = |a: u16| dmx(&e, a + 1); // Pan coarse
    assert_eq!((pan(1), pan(16)), (255, 255), "101 and 104 first");
    assert_eq!((pan(6), pan(11)), (0, 0), "102 and 103 start at 2 s");
}

// ---- FX-45 -------------------------------------------------------------------

fn chase(e: &mut Engine, ids: &[FixtureId]) -> EntryId {
    select(e, ids);
    e.effect_apply(&look("Dimmer chase")).unwrap()
}

#[test]
fn fx_45_reverse_lets_the_last_member_lead() {
    let mut e = rig();
    let id = chase(&mut e, &[201, 202, 203, 204]);
    e.tick(0.0);
    e.tick(0.1);
    let theta = e.effect_theta(id).unwrap();
    e.effect_set(EffectSet::Order(Order::Reverse)).unwrap();
    assert_eq!(entry(&e, id).members, items(&[204, 203, 202, 201]));
    assert_eq!(entry(&e, id).positions, [0, 1, 2, 3]);
    assert_eq!(e.effect_theta(id).unwrap(), theta, "the clock runs on");
    let mut e = rig();
    chase(&mut e, &[201, 202, 203, 204]);
    e.effect_set(EffectSet::Order(Order::Reverse)).unwrap();
    e.tick(0.0);
    assert_eq!((dmx(&e, 24), dmx(&e, 21)), (255, 0), "204 leads");
}

#[test]
fn fx_45_sort_is_by_user_number() {
    let mut e = rig();
    let id = chase(&mut e, &[203, 201, 204, 202]);
    e.effect_set(EffectSet::Order(Order::Sort)).unwrap();
    assert_eq!(entry(&e, id).members, items(&[201, 202, 203, 204]));
}

#[test]
fn fx_45_shuffle_takes_the_programmer_counter() {
    // The selection's first Shuffle and an entry's first Shuffle use the same seed (1).
    let mut a = rig();
    select(&mut a, &[201, 202, 203, 204, 205]);
    a.order(Order::Shuffle).unwrap();
    let mut b = rig();
    let id = chase(&mut b, &[201, 202, 203, 204, 205]);
    b.effect_set(EffectSet::Order(Order::Shuffle)).unwrap();
    let once = entry(&b, id).members.clone();
    assert_eq!(once, a.programmer().items());
    // ... and the counter moved on: the next Shuffle of either kind gives a new order.
    b.effect_set(EffectSet::Order(Order::Shuffle)).unwrap();
    assert_ne!(entry(&b, id).members, once);
    let mut c = rig();
    let id = chase(&mut c, &[201, 202, 203, 204, 205]);
    c.effect_set(EffectSet::Order(Order::Shuffle)).unwrap();
    c.effect_set(EffectSet::Order(Order::Shuffle)).unwrap();
    assert_eq!(
        entry(&c, id).members,
        entry(&b, id).members,
        "same actions, same order"
    );
}

#[test]
fn fx_45_revert_and_short_entries_are_refused() {
    let mut e = rig();
    assert!(
        e.effect_set(EffectSet::Order(Order::Reverse)).is_err(),
        "no current entry"
    );
    let id = chase(&mut e, &[201, 202, 203]);
    let before = entry(&e, id).clone();
    assert!(e.effect_set(EffectSet::Order(Order::Revert)).is_err());
    assert_eq!(entry(&e, id), &before);
    let mut e = rig();
    let id = chase(&mut e, &[201]);
    let before = entry(&e, id).clone();
    for tool in [Order::Reverse, Order::Shuffle, Order::Sort] {
        assert!(e.effect_set(EffectSet::Order(tool)).is_err(), "{tool:?}");
    }
    assert_eq!(entry(&e, id), &before);
}

#[test]
fn fx_45_is_an_effect_set_for_undo() {
    let mut e = rig();
    let mut h = History::default();
    let id = chase(&mut e, &[201, 202, 203, 204]);
    assert_eq!(EffectSet::Order(Order::Reverse).name(), "order");
    let key = format!("effect:{id}:order");
    h.edit(&mut e, "Effect order", Some(&key), |e| {
        e.effect_set(EffectSet::Order(Order::Reverse))
    })
    .unwrap();
    assert_eq!(entry(&e, id).members, items(&[204, 203, 202, 201]));
    h.undo(&mut e).unwrap();
    assert_eq!(entry(&e, id).members, items(&[201, 202, 203, 204]));
}
