//! Working on many at once (IDEA-35 batch D, P1-36): `playbacks.md` PBK-50, 51 (MUL-01);
//! `palettes.md` PAL-34, `programmer.md` PRG-98, `cuelists.md` CUE-63 (MUL-02).
//!
//! API these tests fix:
//! - `cmdmx_core::playback::Layout::{Retain, Bunch}` (`Retain` is the default; serde lowercase).
//! - `Engine::copy_playbacks(&[PlaybackId], page, slot, Layout) -> Result<Vec<PlaybackId>, _>`:
//!   the new ids in source handle order (page, then slot).
//! - `Engine::move_playbacks(&[PlaybackId], page, slot, Layout, swap: bool) -> Result<(), _>`.
//! - `Engine::delete_playbacks(&[PlaybackId]) -> Result<(), _>`.
//! - Each is all or nothing: a refusal leaves `edit_state()` and every playback state unchanged.
//!   With one id they behave as `copy_playback`, `move_playback`, `delete_playback` (PBK-14),
//!   except that `swap` on a used target swaps the two homes (PBK-51).

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

mod common;

use std::collections::BTreeMap;

use cmdmx_core::engine::Engine;
use cmdmx_core::palette::{Cell, Record, Window};
use cmdmx_core::playback::{
    Cue, CueNumber, GroupMaster, Layout, ListAction, Master, Paging, Playback, PlaybackRecord,
    Values,
};
use cmdmx_core::programmer::Item;
use cmdmx_core::value::Stored;

fn dimmer(id: u32) -> Values {
    let mut v: Values = BTreeMap::new();
    v.entry(id.into())
        .or_default()
        .insert("Dimmer".to_string(), 1.0.into());
    v
}

/// The church rig with playback `id` on each `(id, page, slot)`.
fn with(playbacks: &[(u32, u16, u16)]) -> Engine {
    let mut e = common::church();
    for &(id, page, slot) in playbacks {
        e.add_playback(Playback::new(id, page, slot, dimmer(201)));
    }
    e
}

fn home(e: &Engine, id: u32) -> (u16, u16) {
    let pb = e.playback(id).unwrap();
    (pb.page, pb.slot)
}

// ---- PBK-50: Copy, Move and Delete several playbacks -------------------------------------

#[test]
fn pbk_50_copy_retain_keeps_the_gap() {
    let mut e = with(&[(1, 1, 1), (2, 1, 3)]);
    let new = e.copy_playbacks(&[2, 1], 2, 4, Layout::Retain).unwrap();
    assert_eq!(new.len(), 2);
    assert_eq!(
        home(&e, new[0]),
        (2, 4),
        "the lowest source lands on the target"
    );
    assert_eq!(home(&e, new[1]), (2, 6), "the gap of one slot is kept");
    assert_eq!((home(&e, 1), home(&e, 2)), ((1, 1), (1, 3)));
    assert_eq!(e.playbacks().count(), 4);
}

#[test]
fn pbk_50_retain_refuses_sources_on_two_pages_and_bunch_takes_them() {
    let mut e = with(&[(1, 1, 1), (3, 2, 2)]);
    let before = e.edit_state();
    let err = e.copy_playbacks(&[1, 3], 3, 1, Layout::Retain).unwrap_err();
    assert!(
        err.to_string().contains("Sources on 2 pages: use Bunch Up"),
        "{err}"
    );
    assert_eq!(e.edit_state(), before);
    let new = e.copy_playbacks(&[1, 3], 3, 1, Layout::Bunch).unwrap();
    assert_eq!((home(&e, new[0]), home(&e, new[1])), ((3, 1), (3, 2)));
}

#[test]
fn pbk_50_bunch_closes_the_gaps_in_handle_order() {
    let mut e = with(&[(1, 1, 1), (2, 1, 3), (3, 1, 6)]);
    e.move_playbacks(&[3, 1, 2], 2, 2, Layout::Bunch, false)
        .unwrap();
    assert_eq!(
        (home(&e, 1), home(&e, 2), home(&e, 3)),
        ((2, 2), (2, 3), (2, 4))
    );
}

#[test]
fn pbk_50_a_block_past_the_page_end_is_refused() {
    let mut e = with(&[(1, 1, 1), (2, 1, 3)]);
    let last = e.settings().faders_per_page;
    let before = e.edit_state();
    assert!(e
        .copy_playbacks(&[1, 2], 2, last - 1, Layout::Retain)
        .is_err());
    assert!(e
        .move_playbacks(&[1, 2], 2, last, Layout::Bunch, false)
        .is_err());
    assert_eq!(e.edit_state(), before);
    // It fits exactly.
    e.move_playbacks(&[1, 2], 2, last - 2, Layout::Retain, false)
        .unwrap();
    assert_eq!(home(&e, 2), (2, last));
}

#[test]
fn pbk_50_a_used_target_refuses_it_all_and_is_named() {
    let mut e = with(&[(1, 1, 1), (2, 1, 2), (5, 2, 2)]);
    let before = e.edit_state();
    let err = e.copy_playbacks(&[1, 2], 2, 1, Layout::Retain).unwrap_err();
    assert_eq!(err.to_string(), "Handle 2.2 is used");
    let err = e
        .move_playbacks(&[1, 2], 2, 1, Layout::Retain, false)
        .unwrap_err();
    assert_eq!(err.to_string(), "Handle 2.2 is used");
    assert_eq!(
        e.edit_state(),
        before,
        "the first item is not copied or moved"
    );
    assert_eq!(e.playbacks().count(), 3);
}

#[test]
fn pbk_50_copy_onto_its_own_sources_is_refused() {
    let mut e = with(&[(1, 1, 1), (2, 1, 2)]);
    let before = e.edit_state();
    assert!(e.copy_playbacks(&[1, 2], 1, 2, Layout::Retain).is_err());
    assert_eq!(e.edit_state(), before);
}

#[test]
fn pbk_50_a_block_moves_by_one_slot_and_keeps_running() {
    let mut e = with(&[(1, 1, 1), (2, 1, 2), (3, 1, 3)]);
    e.set_fader(2, 0.5, 0.0).unwrap();
    e.move_playbacks(&[1, 2, 3], 1, 2, Layout::Retain, false)
        .unwrap();
    assert_eq!(
        (home(&e, 1), home(&e, 2), home(&e, 3)),
        ((1, 2), (1, 3), (1, 4))
    );
    let state = e.playback_state(2).unwrap();
    assert!(state.is_active());
    assert_eq!(state.level, 0.5);
    e.set_page(1).unwrap();
    assert_eq!(&e.page_slots()[..4], &[None, Some(1), Some(2), Some(3)]);
}

#[test]
fn pbk_50_an_unknown_playback_refuses_it_all() {
    let mut e = with(&[(1, 1, 1), (2, 1, 2)]);
    let before = e.edit_state();
    assert!(e.copy_playbacks(&[1, 9], 2, 1, Layout::Retain).is_err());
    assert!(e
        .move_playbacks(&[1, 9], 2, 1, Layout::Retain, false)
        .is_err());
    assert!(e.delete_playbacks(&[1, 9]).is_err());
    assert!(e.delete_playbacks(&[]).is_err(), "nothing to delete");
    assert_eq!(e.edit_state(), before);
}

#[test]
fn pbk_50_delete_kills_a_running_list_and_removes_them_all() {
    let mut e = with(&[(1, 1, 1), (2, 1, 2), (3, 1, 3)]);
    e.set_fader(1, 1.0, 0.0).unwrap();
    e.tick(0.0);
    e.delete_playbacks(&[1, 3]).unwrap();
    assert!(e.playback(1).is_none() && e.playback(3).is_none());
    assert!(e.playback_state(1).is_none());
    assert!(e.playback(2).is_some());
    e.tick(0.025);
    assert_eq!(e.frame(1).unwrap().channel(21).unwrap(), 0, "201 is out");
}

#[test]
fn pbk_50_one_id_is_pbk_14() {
    let mut e = with(&[(1, 1, 1), (2, 2, 4)]);
    let new = e.copy_playbacks(&[1], 3, 3, Layout::Retain).unwrap();
    assert_eq!(home(&e, new[0]), (3, 3));
    assert!(e.move_playbacks(&[1], 2, 4, Layout::Retain, false).is_err());
    e.move_playbacks(&[1], 2, 5, Layout::Bunch, false).unwrap();
    assert_eq!(home(&e, 1), (2, 5));
}

// ---- PBK-51: Swap if required --------------------------------------------------------------

#[test]
fn pbk_51_one_id_swaps_the_two_homes() {
    let mut e = with(&[(1, 1, 1), (2, 1, 2)]);
    e.set_fader(2, 0.7, 0.0).unwrap();
    e.move_playbacks(&[1], 1, 2, Layout::Retain, true).unwrap();
    assert_eq!((home(&e, 1), home(&e, 2)), ((1, 2), (1, 1)));
    assert_eq!(e.playback_state(2).unwrap().level, 0.7, "it keeps running");
}

#[test]
fn pbk_51_a_block_of_three_over_two_used_slots() {
    let mut e = with(&[(1, 1, 1), (2, 1, 2), (3, 1, 3), (7, 1, 5), (8, 1, 6)]);
    e.move_playbacks(&[1, 2, 3], 1, 4, Layout::Retain, true)
        .unwrap();
    assert_eq!(
        (home(&e, 1), home(&e, 2), home(&e, 3)),
        ((1, 4), (1, 5), (1, 6))
    );
    // In handle order on both sides: 1.5 → 1.1, 1.6 → 1.2.
    assert_eq!((home(&e, 7), home(&e, 8)), ((1, 1), (1, 2)));
}

#[test]
fn pbk_51_swap_still_refuses_past_the_page_end_and_a_locked_slot() {
    let mut e = with(&[(1, 2, 1), (2, 2, 2)]);
    e.add_playback(Playback {
        paging: Paging::Locked,
        ..Playback::new(9, 1, 8, dimmer(202))
    });
    let last = e.settings().faders_per_page;
    let before = e.edit_state();
    let err = e
        .move_playbacks(&[1], 2, 8, Layout::Retain, true)
        .unwrap_err();
    assert_eq!(err.to_string(), "Handle 2.8 is locked by 1.8");
    let err = e
        .move_playbacks(&[1], 2, 8, Layout::Retain, false)
        .unwrap_err();
    assert_eq!(err.to_string(), "Handle 2.8 is used");
    assert!(e
        .move_playbacks(&[1, 2], 2, last, Layout::Retain, true)
        .is_err());
    assert_eq!(e.edit_state(), before);
}

// ---- PAL-34: Copy, Move and Delete several palettes (MUL-02) -------------------------------
//
// API: `Engine::copy_palettes(&[number], to: Cell, Layout) -> Result<Vec<PaletteId>, _>` (new
// ids in cell order), `move_palettes(&[number], to: Cell, Layout, swap: bool)`,
// `delete_palettes(&[number])`; all or nothing.

/// A Positions palette on `handle` holding wash 101's Tilt; returns its number.
fn position(e: &mut Engine, handle: u16, tilt: f32) -> u32 {
    common::select(e, &[101]);
    e.set_programmer_value(101, "Tilt", tilt).unwrap();
    let id = e
        .record_palette(&Record::at(Cell::new(Window::Positions, handle)))
        .unwrap();
    e.clear_selection();
    e.clear();
    e.palettes().find(|p| p.id == id).unwrap().number
}

fn cell_of(e: &Engine, number: u32) -> (Window, u16) {
    let p = e.palette(number).unwrap();
    (p.window, p.handle)
}

#[test]
fn pal_34_copy_retain_keeps_the_gap_with_new_numbers() {
    let mut e = common::church();
    let (a, b) = (position(&mut e, 1, 0.1), position(&mut e, 3, 0.3));
    let new = e
        .copy_palettes(&[a, b], Cell::new(Window::Positions, 7), Layout::Retain)
        .unwrap();
    assert_eq!(new.len(), 2);
    let cells: Vec<_> = new
        .iter()
        .map(|id| {
            let p = e.palettes().find(|p| p.id == *id).unwrap();
            (p.window, p.handle, p.number)
        })
        .collect();
    assert_eq!(cells[0].1, 7);
    assert_eq!(cells[1].1, 9);
    assert!(cells.iter().all(|c| c.0 == Window::Positions));
    assert!(cells.iter().all(|c| c.2 != a && c.2 != b), "new numbers");
    assert_eq!(
        (cell_of(&e, a), cell_of(&e, b)),
        ((Window::Positions, 1), (Window::Positions, 3))
    );
}

#[test]
fn pal_34_move_bunch_closes_the_gaps_into_another_window() {
    let mut e = common::church();
    let ns = [
        position(&mut e, 1, 0.1),
        position(&mut e, 3, 0.3),
        position(&mut e, 6, 0.6),
    ];
    e.move_palettes(
        &[ns[2], ns[0], ns[1]],
        Cell::new(Window::Beams, 2),
        Layout::Bunch,
        false,
    )
    .unwrap();
    assert_eq!(
        ns.map(|n| cell_of(&e, n)),
        [(Window::Beams, 2), (Window::Beams, 3), (Window::Beams, 4)]
    );
}

#[test]
fn pal_34_a_used_cell_refuses_and_swap_moves_it_out_of_the_way() {
    let mut e = common::church();
    let (a, b, c) = (
        position(&mut e, 1, 0.1),
        position(&mut e, 2, 0.2),
        position(&mut e, 5, 0.5),
    );
    let before = e.edit_state();
    let to = Cell::new(Window::Positions, 5);
    assert!(e.copy_palettes(&[a, b], to, Layout::Retain).is_err());
    assert!(e.move_palettes(&[a, b], to, Layout::Retain, false).is_err());
    assert!(e
        .move_palettes(
            &[a, 99],
            Cell::new(Window::Positions, 9),
            Layout::Retain,
            false
        )
        .is_err());
    assert_eq!(e.edit_state(), before, "nothing changes");
    e.move_palettes(&[a], to, Layout::Retain, true).unwrap();
    assert_eq!(cell_of(&e, a), (Window::Positions, 5));
    assert_eq!(
        cell_of(&e, c),
        (Window::Positions, 1),
        "into the cell a left"
    );
    assert_eq!(cell_of(&e, b), (Window::Positions, 2));
}

#[test]
fn pal_34_delete_turns_references_in_a_cue_into_fallback_values() {
    let mut e = common::church();
    let (a, b) = (position(&mut e, 1, 0.1), position(&mut e, 2, 0.2));
    common::select(&mut e, &[101]);
    e.apply_palette(a, Default::default()).unwrap();
    let pb = e.record_playback(&PlaybackRecord::at(1, 1)).unwrap();
    e.clear_selection();
    e.clear();
    let before = e.edit_state();
    assert!(e.delete_palettes(&[a, b, 99]).is_err());
    assert_eq!(e.edit_state(), before);
    e.delete_palettes(&[a, b]).unwrap();
    assert!(e.palette(a).is_none() && e.palette(b).is_none());
    let stored = e.playback(pb).unwrap().cues[0].values[&Item::fixture(101)]["Tilt"];
    assert!(matches!(stored, Stored::Value(_)), "{stored:?}");
}

// ---- PRG-98: Copy, Move and Delete several groups (MUL-02) ---------------------------------
//
// API: `Engine::copy_groups(&[GroupId], to: GroupId, Layout)`, `move_groups(&[GroupId], to,
// Layout, swap: bool)`, `delete_groups(&[GroupId])`; all or nothing.

fn group(e: &mut Engine, id: u32, fixtures: &[u32]) {
    common::select(e, fixtures);
    e.record_group(id).unwrap();
    e.clear_selection();
}

/// A group master for `group` on handle 1.8.
fn master(e: &mut Engine, group: u32) -> u32 {
    e.record_group_master(1, 8, group).unwrap()
}

fn mastered(e: &Engine, master: u32) -> u32 {
    match &e.playback(master).unwrap().master {
        Some(Master::Group(GroupMaster { group, .. })) => *group,
        other => panic!("not a group master: {other:?}"),
    }
}

fn items(ids: &[u32]) -> Vec<Item> {
    ids.iter().map(|&id| Item::fixture(id)).collect()
}

#[test]
fn prg_98_copy_retain_keeps_the_gap() {
    let mut e = common::church();
    group(&mut e, 1, &[201]);
    group(&mut e, 3, &[203]);
    e.copy_groups(&[3, 1], 10, Layout::Retain).unwrap();
    assert_eq!(e.group_items(10), Some(items(&[201])));
    assert_eq!(e.group_items(11), None);
    assert_eq!(e.group_items(12), Some(items(&[203])));
    assert_eq!(e.group_items(1), Some(items(&[201])));
}

#[test]
fn prg_98_move_bunch_closes_the_gaps_and_masters_follow() {
    let mut e = common::church();
    group(&mut e, 1, &[201]);
    group(&mut e, 3, &[203]);
    group(&mut e, 6, &[206]);
    let m = master(&mut e, 6);
    e.move_groups(&[1, 3, 6], 20, Layout::Bunch, false).unwrap();
    assert_eq!(e.group_items(20), Some(items(&[201])));
    assert_eq!(e.group_items(21), Some(items(&[203])));
    assert_eq!(e.group_items(22), Some(items(&[206])));
    assert!([1, 3, 6].iter().all(|&g| e.group_items(g).is_none()));
    assert_eq!(mastered(&e, m), 22);
}

#[test]
fn prg_98_a_used_group_refuses_and_swap_moves_it_out_of_the_way() {
    let mut e = common::church();
    group(&mut e, 1, &[201]);
    group(&mut e, 2, &[202]);
    group(&mut e, 5, &[205]);
    let m = master(&mut e, 5);
    let before = e.edit_state();
    assert!(
        e.copy_groups(&[1, 2], 4, Layout::Retain).is_err(),
        "5 is used"
    );
    assert!(e.move_groups(&[1, 2], 5, Layout::Retain, false).is_err());
    assert!(
        e.move_groups(&[1, 9], 30, Layout::Retain, false).is_err(),
        "9 is empty"
    );
    assert_eq!(e.edit_state(), before);
    e.move_groups(&[1], 5, Layout::Retain, true).unwrap();
    assert_eq!(e.group_items(5), Some(items(&[201])));
    assert_eq!(e.group_items(1), Some(items(&[205])));
    assert_eq!(mastered(&e, m), 1, "the master follows its group");
}

#[test]
fn prg_98_delete_refuses_it_all_while_one_group_is_mastered() {
    let mut e = common::church();
    group(&mut e, 1, &[201]);
    group(&mut e, 6, &[206]);
    let m = master(&mut e, 6);
    let before = e.edit_state();
    let err = e.delete_groups(&[1, 6]).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Group 6 is mastered by handle 1.8: delete the master first"
    );
    assert_eq!(e.edit_state(), before);
    e.delete_playback(m).unwrap();
    e.delete_groups(&[1, 6]).unwrap();
    assert!(e.group_items(1).is_none() && e.group_items(6).is_none());
}

// ---- CUE-63: Delete several cues (MUL-02) ----------------------------------------------------
//
// API: `Engine::delete_cues(id, &[CueNumber])`; all or nothing.

fn q(n: f64) -> CueNumber {
    CueNumber::new(n).unwrap()
}

fn cue(n: f64, fixture: u32, level: f32) -> Cue {
    let mut values: Values = BTreeMap::new();
    values
        .entry(fixture.into())
        .or_default()
        .insert("Dimmer".to_string(), level.into());
    Cue {
        number: q(n),
        ..Cue::new(values)
    }
}

/// List on 1.1: Q1 201 at 50 %, Q2 201 full, Q3 202 full, Q3.5 203 full, Q4 204 full.
fn five_cues() -> Engine {
    let mut e = common::church();
    e.add_playback(Playback {
        cues: vec![
            cue(1.0, 201, 0.5),
            cue(2.0, 201, 1.0),
            cue(3.0, 202, 1.0),
            cue(3.5, 203, 1.0),
            cue(4.0, 204, 1.0),
        ],
        ..Playback::new(1, 1, 1, Values::new())
    });
    e
}

fn numbers(e: &Engine) -> Vec<f64> {
    e.playback(1)
        .unwrap()
        .cues
        .iter()
        .map(|c| c.number.as_f64())
        .collect()
}

#[test]
fn cue_63_three_cues_out_of_five_and_tracking_after() {
    let mut e = five_cues();
    e.delete_cues(1, &[q(3.5), q(2.0), q(3.0)]).unwrap();
    assert_eq!(numbers(&e), [1.0, 4.0]);
    // Q4 tracks from Q1 now: 201 at 50 %.
    e.set_fader(1, 1.0, 0.0).unwrap();
    e.cuelist(Some(1), ListAction::Goto(q(4.0)), 0.0).unwrap();
    e.tick(0.0);
    e.tick(0.5);
    assert_eq!(common::dmx(&e, 21), 128);
}

#[test]
fn cue_63_every_cue_or_an_unknown_one_is_refused() {
    let mut e = five_cues();
    let before = e.edit_state();
    let all = [q(1.0), q(2.0), q(3.0), q(3.5), q(4.0)];
    let err = e.delete_cues(1, &all).unwrap_err();
    assert!(
        err.to_string().contains("Delete the playback instead"),
        "{err}"
    );
    assert!(e.delete_cues(1, &[q(2.0), q(9.0)]).is_err());
    assert!(e.delete_cues(1, &[]).is_err());
    assert!(e.delete_cues(7, &[q(1.0)]).is_err());
    assert_eq!(e.edit_state(), before);
    assert_eq!(numbers(&e).len(), 5);
}
