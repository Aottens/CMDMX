//! Groups and cues keep the selection order (SEL-03, P2-02): `specs/modules/programmer.md`
//! PRG-79..81, 83 and `specs/modules/playbacks.md` PBK-15 as amended. Pars 201–208 are
//! one-channel dimmers at 21–28; washes 101–104 (Dimmer, Pan 16-bit, Tilt, Zoom) at 1, 6, 11, 16.
//!
//! API under test (new in SEL-03):
//! - `programmer::Grid` is `Serialize`/`Deserialize` (`{wings, groups, blocks, flip}`).
//! - `Engine::group_grid(id) -> Option<Grid>`: the grid recorded with a group (PRG-79), `None`
//!   when it was recorded with the grid off. `ShowGroup::grid: Option<Grid>`, absent when `None`.
//! - `Cue::order: BTreeMap<Item, u32>` (PRG-80): empty = no order, then absent in the file.
//!   Recording with an empty selection stores no order.
//! - `Engine::set_cue_order(id, cue, &BTreeMap<Item, i64>)` (PRG-81, `cue.order`): an edit;
//!   an item not in the cue's values or a negative position is refused and changes nothing.
//! - `SCHEMA_VERSION` 9 with the step "8 -> 9" (PRG-83).

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

mod common;

use std::collections::BTreeMap;

use cmdmx_core::effect::{EffectApply, EffectSet};
use cmdmx_core::engine::Engine;
use cmdmx_core::palette::RecordMode;
use cmdmx_core::patch::FixtureId;
use cmdmx_core::playback::{
    Cue, CueChange, CueNumber, ListAction, Playback, PlaybackRecord, Values,
};
use cmdmx_core::programmer::{Grid, GridShortcut, Item};
use cmdmx_core::show::{Meta, Show, SCHEMA_VERSION};
use cmdmx_core::undo::History;
use common::{church, dmx, select};

const PARS: [FixtureId; 8] = [201, 202, 203, 204, 205, 206, 207, 208];

fn items(ids: &[FixtureId]) -> Vec<Item> {
    ids.iter().map(|&id| Item::fixture(id)).collect()
}

fn order(pairs: &[(FixtureId, u32)]) -> BTreeMap<Item, u32> {
    pairs.iter().map(|&(f, k)| (Item::fixture(f), k)).collect()
}

fn edit(pairs: &[(FixtureId, i64)]) -> BTreeMap<Item, i64> {
    pairs.iter().map(|&(f, k)| (Item::fixture(f), k)).collect()
}

fn q(n: f64) -> CueNumber {
    CueNumber::new(n).unwrap()
}

fn outside_in() -> Grid {
    Grid {
        wings: 2,
        ..Grid::default()
    }
}

fn pars_dmx(e: &Engine) -> Vec<u8> {
    (21..=28).map(|a| dmx(e, a)).collect()
}

fn near(got: u8, want: u8) -> bool {
    got.abs_diff(want) <= 1
}

fn full(e: &mut Engine, ids: &[FixtureId]) {
    for &id in ids {
        e.set_programmer_value(id, "Dimmer", 1.0).unwrap();
    }
}

fn record(e: &mut Engine, page: u16, slot: u16) -> u32 {
    e.record_playback(&PlaybackRecord::at(page, slot)).unwrap()
}

fn record_mode(e: &mut Engine, page: u16, slot: u16, mode: RecordMode) -> u32 {
    let record = PlaybackRecord {
        mode: Some(mode),
        ..PlaybackRecord::at(page, slot)
    };
    e.record_playback(&record).unwrap()
}

fn cue_order(e: &Engine, pb: u32, n: usize) -> BTreeMap<Item, u32> {
    e.playback(pb).unwrap().cues[n].order.clone()
}

fn empty_programmer(e: &mut Engine) {
    e.clear_selection();
    e.clear();
}

/// Group 5 "Pars outside in": 201–208 with wings 2.
fn pars_outside_in(e: &mut Engine) {
    select(e, &PARS);
    e.grid_shortcut(GridShortcut::OutsideIn).unwrap();
    e.record_group(5).unwrap();
    e.clear_selection();
}

// ---- PRG-79 ---------------------------------------------------------------------------------

#[test]
fn prg_79_a_group_stores_the_grid_and_a_new_selection_restores_it() {
    let mut e = church();
    pars_outside_in(&mut e);
    assert_eq!(e.group_grid(5), Some(outside_in()));
    assert_eq!(
        e.programmer().grid(),
        None,
        "Clear of the selection ends it"
    );

    e.select_group(5).unwrap();
    assert_eq!(e.programmer().items(), items(&PARS).as_slice());
    assert_eq!(e.programmer().grid(), Some(outside_in()));
    assert_eq!(e.programmer().stage(), None, "no stage, as after All");
    assert_eq!(e.programmer().positions(), [0, 1, 2, 3, 3, 2, 1, 0]);
    e.next_fixture();
    assert_eq!(e.programmer().stage(), Some(0), "Fix +1 starts at stage 0");
}

#[test]
fn prg_79_a_group_recorded_with_the_grid_off_stores_none() {
    let mut e = church();
    select(&mut e, &[203, 201]);
    e.record_group(6).unwrap();
    assert_eq!(e.group_grid(6), None);
    e.clear_selection();
    e.select_group(6).unwrap();
    assert_eq!(e.programmer().grid(), None);
    assert_eq!(e.programmer().items(), items(&[203, 201]).as_slice());
}

#[test]
fn prg_79_re_recording_without_the_grid_drops_it() {
    let mut e = church();
    pars_outside_in(&mut e);
    select(&mut e, &PARS);
    e.record_group(5).unwrap();
    assert_eq!(e.group_grid(5), None);
}

#[test]
fn prg_79_a_typed_group_restores_its_grid() {
    let mut e = church();
    pars_outside_in(&mut e);
    select(&mut e, &[301, 302]);
    e.execute_command(&["Group", "5", "Enter"]).unwrap();
    assert_eq!(e.programmer().items(), items(&PARS).as_slice());
    assert_eq!(e.programmer().grid(), Some(outside_in()));
    assert_eq!(e.programmer().stage(), None);
}

#[test]
fn prg_79_a_group_after_a_change_starts_a_new_selection_with_its_grid() {
    let mut e = church();
    pars_outside_in(&mut e);
    select(&mut e, &[301]);
    e.set_programmer_value(301, "Dimmer", 0.5).unwrap(); // PRG-03: next select starts afresh
    e.select_group(5).unwrap();
    assert_eq!(e.programmer().items(), items(&PARS).as_slice());
    assert_eq!(e.programmer().grid(), Some(outside_in()));
}

#[test]
fn prg_79_into_an_existing_selection_the_group_appends_and_leaves_the_grid() {
    let mut e = church();
    pars_outside_in(&mut e);
    select(&mut e, &[301]);
    e.select_group(5).unwrap();
    let mut want = items(&[301]);
    want.extend(items(&PARS));
    assert_eq!(e.programmer().items(), want.as_slice());
    assert_eq!(e.programmer().grid(), None, "grid off stays off");

    e.clear_selection();
    select(&mut e, &[301, 302]);
    e.grid_shortcut(GridShortcut::OddEven).unwrap();
    e.select_group(5).unwrap();
    let odd_even = Grid {
        groups: 2,
        ..Grid::default()
    };
    assert_eq!(
        e.programmer().grid(),
        Some(odd_even),
        "a grid that is on stays"
    );
}

#[test]
fn prg_79_the_dimmer_chase_of_the_group_runs_inwards() {
    let mut e = church();
    pars_outside_in(&mut e);
    e.execute_command(&["Group", "5", "Enter"]).unwrap();
    e.effect_apply(&EffectApply::Look("Dimmer chase".into()))
        .unwrap();
    e.tick(0.0);
    assert_eq!(pars_dmx(&e), [255, 0, 0, 0, 0, 0, 0, 255], "from both ends");
}

#[test]
fn prg_79_undo_puts_the_old_group_and_grid_back() {
    let mut e = church();
    select(&mut e, &PARS);
    e.record_group(5).unwrap();
    let mut history = History::default();
    e.grid_shortcut(GridShortcut::OutsideIn).unwrap();
    history
        .edit(&mut e, "Record group", None, |e| e.record_group(5))
        .unwrap();
    assert_eq!(e.group_grid(5), Some(outside_in()));
    history.undo(&mut e).unwrap();
    assert_eq!(e.group_grid(5), None);
}

// ---- PRG-80 ---------------------------------------------------------------------------------

#[test]
fn prg_80_record_stores_the_positions_under_the_grid() {
    let mut e = church();
    select(&mut e, &PARS);
    full(&mut e, &PARS);
    e.grid_shortcut(GridShortcut::OutsideIn).unwrap(); // stage 0 selected: does not matter
    let pb = record(&mut e, 1, 1);
    assert_eq!(
        cue_order(&e, pb, 0),
        order(&[
            (201, 0),
            (208, 0),
            (202, 1),
            (207, 1),
            (203, 2),
            (206, 2),
            (204, 3),
            (205, 3)
        ])
    );
}

#[test]
fn prg_80_recorded_items_outside_the_selection_follow_in_user_number_order() {
    let mut e = church();
    full(&mut e, &[302, 301, 101, 203]);
    e.clear_selection();
    select(&mut e, &[203, 201]); // 201 has no value: not recorded, so not in the order
    let pb = record(&mut e, 1, 1);
    assert_eq!(
        cue_order(&e, pb, 0),
        order(&[(203, 0), (101, 2), (301, 3), (302, 4)]),
        "N = 2 positions in the selection, then user numbers"
    );
}

#[test]
fn prg_80_recording_without_a_selection_stores_no_order() {
    let mut e = church();
    select(&mut e, &[202, 201]);
    full(&mut e, &[202, 201]);
    e.clear_selection();
    let pb = record(&mut e, 1, 1);
    assert!(cue_order(&e, pb, 0).is_empty());
}

#[test]
fn prg_80_merge_keeps_positions_and_puts_new_items_after_the_highest() {
    let mut e = church();
    select(&mut e, &[201, 202]);
    full(&mut e, &[201, 202]);
    let pb = record(&mut e, 1, 1);
    assert_eq!(cue_order(&e, pb, 0), order(&[(201, 0), (202, 1)]));
    empty_programmer(&mut e);

    select(&mut e, &[204, 201, 203]);
    full(&mut e, &[204, 201, 203]);
    record_mode(&mut e, 1, 1, RecordMode::Merge);
    assert_eq!(
        cue_order(&e, pb, 0),
        order(&[(201, 0), (202, 1), (204, 2), (203, 3)]),
        "201 keeps 0; the new items in their recorded order"
    );
}

#[test]
fn prg_80_merge_keeps_ties_among_new_items() {
    let mut e = church();
    select(&mut e, &[201]);
    full(&mut e, &[201]);
    let pb = record(&mut e, 1, 1);
    empty_programmer(&mut e);

    select(&mut e, &[202, 203, 204, 205]);
    full(&mut e, &[202, 203, 204, 205]);
    e.grid_shortcut(GridShortcut::OutsideIn).unwrap(); // 0 1 1 0
    record_mode(&mut e, 1, 1, RecordMode::Merge);
    assert_eq!(
        cue_order(&e, pb, 0),
        order(&[(201, 0), (202, 1), (205, 1), (203, 2), (204, 2)])
    );
}

#[test]
fn prg_80_update_keeps_the_order() {
    let mut e = church();
    select(&mut e, &[202, 201]);
    full(&mut e, &[202, 201]);
    let pb = record(&mut e, 1, 1);
    empty_programmer(&mut e);

    select(&mut e, &[201, 202, 203]);
    full(&mut e, &[201, 202, 203]);
    record_mode(&mut e, 1, 1, RecordMode::QuickMerge);
    assert_eq!(cue_order(&e, pb, 0), order(&[(202, 0), (201, 1)]));
}

#[test]
fn prg_80_replace_records_a_fresh_order() {
    let mut e = church();
    select(&mut e, &[202, 201]);
    full(&mut e, &[202, 201]);
    let pb = record(&mut e, 1, 1);
    empty_programmer(&mut e);

    select(&mut e, &[201, 203]);
    full(&mut e, &[201, 203]);
    record_mode(&mut e, 1, 1, RecordMode::Replace);
    assert_eq!(cue_order(&e, pb, 0), order(&[(201, 0), (203, 1)]));
}

/// List 1 on 1.1: Q1 par 221 at full, Q2 pars 201–208 at full recorded Outside in, fade 2 s,
/// overlap 0 %. The fader is up, the programmer empty.
fn outside_in_list() -> (Engine, u32) {
    let mut e = church();
    select(&mut e, &[221]);
    full(&mut e, &[221]);
    let pb = record(&mut e, 1, 1);
    empty_programmer(&mut e);
    select(&mut e, &PARS);
    full(&mut e, &PARS);
    e.grid_shortcut(GridShortcut::OutsideIn).unwrap();
    let record = PlaybackRecord {
        cue: Some(q(2.0)),
        ..PlaybackRecord::at(1, 1)
    };
    e.record_playback(&record).unwrap();
    let times = CueChange {
        fade_in: Some(2.0),
        overlap: Some(0.0),
        ..CueChange::default()
    };
    e.change_cue(pb, q(2.0), &times).unwrap();
    empty_programmer(&mut e);
    e.set_fader(pb, 1.0, 0.0).unwrap();
    (e, pb)
}

fn go(e: &mut Engine, pb: u32, now: f64) {
    e.cuelist(Some(pb), ListAction::Go, now).unwrap();
    e.tick(now);
}

#[test]
fn prg_80_overlap_on_go_staggers_by_the_order() {
    let (mut e, pb) = outside_in_list();
    go(&mut e, pb, 0.0); // Q1
    go(&mut e, pb, 10.0); // Q2
    e.tick(11.0);
    let d = pars_dmx(&e);
    assert!(
        near(d[0], 128) && near(d[7], 128),
        "201 and 208 first: {d:?}"
    );
    assert_eq!(&d[1..7], &[0; 6]);
    e.tick(13.0);
    let d = pars_dmx(&e);
    assert!(
        near(d[1], 128) && near(d[6], 128),
        "202 and 207 at 2 s: {d:?}"
    );
    assert_eq!(&d[2..6], &[0; 4]);
    e.tick(17.0);
    let d = pars_dmx(&e);
    assert!(
        near(d[3], 128) && near(d[4], 128),
        "204 and 205 last: {d:?}"
    );
    e.tick(18.0);
    assert_eq!(pars_dmx(&e), [255; 8]);
}

#[test]
fn prg_80_a_cue_without_order_keeps_user_number_order() {
    let mut e = church();
    let mut values = Values::new();
    for &id in &[203, 202, 201] {
        values
            .entry(Item::fixture(id))
            .or_default()
            .insert("Dimmer".into(), 1.0.into());
    }
    let first = Cue {
        number: q(1.0),
        ..Cue::new(Values::from([(
            Item::fixture(221),
            BTreeMap::from([("Dimmer".to_string(), 1.0.into())]),
        )]))
    };
    let second = Cue {
        number: q(2.0),
        fade_in: 2.0,
        overlap: 0.0,
        ..Cue::new(values)
    };
    assert!(second.order.is_empty());
    e.add_playback(Playback {
        cues: vec![first, second],
        ..Playback::new(1, 1, 1, Values::new())
    });
    e.set_fader(1, 1.0, 0.0).unwrap();
    go(&mut e, 1, 0.0);
    go(&mut e, 1, 10.0);
    e.tick(13.0);
    let d = pars_dmx(&e);
    assert_eq!(d[0], 255, "201 first: {d:?}");
    assert!(near(d[1], 128), "202 second: {d:?}");
    assert_eq!(d[2], 0);
}

#[test]
fn prg_80_a_memory_fires_its_ltp_values_in_order() {
    // Washes 101–104 Pan 0 → 1 recorded Outside in on 1.1 (fade 2 s, overlap 0 %): fired, 101
    // and 104 move first, 102 and 103 start at 2 s.
    let mut e = church();
    let washes = [101, 102, 103, 104];
    select(&mut e, &washes);
    for &id in &washes {
        e.set_programmer_value(id, "Pan", 1.0).unwrap();
    }
    e.grid_shortcut(GridShortcut::OutsideIn).unwrap();
    let pb = record(&mut e, 1, 1);
    let times = CueChange {
        fade_in: Some(2.0),
        overlap: Some(0.0),
        ..CueChange::default()
    };
    e.change_cue(pb, q(1.0), &times).unwrap();
    empty_programmer(&mut e);
    e.tick(0.0);
    e.set_fader(pb, 1.0, 1.0).unwrap();
    e.tick(1.0);
    e.tick(3.0);
    let pan = |a: u16| dmx(&e, a + 1); // Pan coarse
    assert_eq!((pan(1), pan(16)), (255, 255), "101 and 104 first");
    assert_eq!((pan(6), pan(11)), (0, 0), "102 and 103 start at 2 s");
}

// ---- PRG-81 ---------------------------------------------------------------------------------

/// Pars 201–208 recorded in user-number order on 1.1 (order 201:0 … 208:7).
fn pars_cue() -> (Engine, u32) {
    let mut e = church();
    select(&mut e, &PARS);
    full(&mut e, &PARS);
    let pb = record(&mut e, 1, 1);
    empty_programmer(&mut e);
    (e, pb)
}

#[test]
fn prg_81_cue_order_replaces_it_and_the_rest_follow_in_their_old_order() {
    let (mut e, pb) = pars_cue();
    e.set_cue_order(pb, q(1.0), &edit(&[(205, 0), (204, 0), (206, 1)]))
        .unwrap();
    assert_eq!(
        cue_order(&e, pb, 0),
        order(&[
            (205, 0),
            (204, 0),
            (206, 1),
            (201, 2),
            (202, 3),
            (203, 4),
            (207, 5),
            (208, 6)
        ])
    );
}

#[test]
fn prg_81_an_item_not_in_the_cue_or_a_negative_position_is_refused() {
    let (mut e, pb) = pars_cue();
    let before = cue_order(&e, pb, 0);
    assert!(e
        .set_cue_order(pb, q(1.0), &edit(&[(205, 0), (301, 1)]))
        .is_err());
    assert!(e.set_cue_order(pb, q(1.0), &edit(&[(205, -1)])).is_err());
    assert!(e.set_cue_order(pb, q(9.0), &edit(&[(205, 0)])).is_err());
    assert_eq!(cue_order(&e, pb, 0), before);
}

#[test]
fn prg_81_editing_the_order_is_an_undoable_edit() {
    let (mut e, pb) = pars_cue();
    let before = cue_order(&e, pb, 0);
    let mut history = History::default();
    history
        .edit(&mut e, "Fixture order", None, |e| {
            e.set_cue_order(pb, q(1.0), &edit(&[(208, 0)]))
        })
        .unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(cue_order(&e, pb, 0)[&Item::fixture(208)], 0);
    history.undo(&mut e).unwrap();
    assert_eq!(cue_order(&e, pb, 0), before);
}

// ---- PRG-83 ---------------------------------------------------------------------------------

fn meta() -> Meta {
    Meta {
        name: "Orders".into(),
        created: "2026-10-30T09:00:00Z".into(),
        modified: "2026-10-30T09:00:00Z".into(),
        app_version: "0.1.0".into(),
    }
}

#[test]
fn prg_83_schema_9_round_trips_grid_order_and_flip() {
    assert_eq!(SCHEMA_VERSION, 9);
    let mut e = church();
    pars_outside_in(&mut e);
    e.select_group(5).unwrap();
    full(&mut e, &PARS);
    e.effect_apply(&EffectApply::Look("Dimmer chase".into()))
        .unwrap();
    e.effect_set(EffectSet::Flip(true)).unwrap();
    let pb = record(&mut e, 1, 1);

    let json = e.to_show(meta()).to_json();
    assert!(json.contains("\"schema_version\": 9"));
    assert!(json.contains("\"grid\""), "{json}");
    assert!(json.contains("\"order\""), "{json}");
    assert!(json.contains("\"flip\": true"), "{json}");

    let (show, migrations) = Show::from_json(&json).unwrap();
    assert!(migrations.is_empty(), "{migrations:?}");
    let (loaded, warnings) = Engine::from_show(&show);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(loaded.group_grid(5), Some(outside_in()));
    let cue = &loaded.playback(pb).unwrap().cues[0];
    assert_eq!(cue.order, cue_order(&e, pb, 0));
    assert!(cue.effects[0].params.flip);
    assert_eq!(loaded.to_show(meta()).to_json(), json);
}

#[test]
fn prg_83_a_show_without_them_saves_as_schema_8_did() {
    let mut e = church();
    select(&mut e, &[202, 201]);
    e.record_group(6).unwrap();
    full(&mut e, &[202, 201]);
    e.effect_apply(&EffectApply::Look("Dimmer chase".into()))
        .unwrap();
    e.clear_selection();
    record(&mut e, 1, 1); // no selection: no order
    let json = e.to_show(meta()).to_json();
    for key in ["\"grid\"", "\"order\"", "\"flip\""] {
        assert!(!json.contains(key), "{key} in {json}");
    }

    let eight = json.replace("\"schema_version\": 9", "\"schema_version\": 8");
    let (show, migrations) = Show::from_json(&eight).unwrap();
    assert!(
        migrations.last().is_some_and(|m| m.starts_with("8 -> 9")),
        "{migrations:?}"
    );
    let (loaded, warnings) = Engine::from_show(&show);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(loaded.to_show(show.meta.clone()).to_json(), json);
}

#[test]
fn prg_83_the_schema_8_church_show_loads_unchanged() {
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/fixtures/shows/"
    );
    let v8 = std::fs::read_to_string(format!("{dir}church-demo-v8.cmdmx.json")).unwrap();
    let v9 = std::fs::read_to_string(format!("{dir}church-demo.cmdmx.json")).unwrap();
    assert_eq!(
        v8.replace("\"schema_version\": 8", "\"schema_version\": 9"),
        v9
    );
    let (show, _) = Show::from_json(&v8).unwrap();
    let (loaded, warnings) = Engine::from_show(&show);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(loaded.to_show(show.meta.clone()).to_json(), v9);
}
