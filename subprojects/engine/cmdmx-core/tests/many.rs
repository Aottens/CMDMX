//! Working on many at once (IDEA-35 batch D, P1-36): `playbacks.md` PBK-50, 51 (MUL-01).
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
use cmdmx_core::playback::{Layout, Paging, Playback, Values};

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
