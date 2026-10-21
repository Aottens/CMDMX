//! Group masters, core (VS3-06): `specs/modules/group-masters.md` GRM-01..12, 19. What reaches
//! the frame is also in `tests/scenarios/GRM-*.json`; the show file (GRM-13, 14) is in
//! `tests/showfile.rs`.
//!
//! API these tests fix (`cmdmx_core::playback` and `Engine`), on top of `tests/cuelists.rs`:
//! - `Kind::GroupMaster` (`is_master()` is true for it, CUE-37).
//! - `GroupMode::{Scale, Htp, Limit, TakeOver}` and `GroupMaster { group: GroupId, mode,
//!   scale: u16, enabled: bool, level: f32 }` (JSON keys in that order), `GroupMaster::new(group)`
//!   with the defaults Scale, 100, true, 1.0; `Master::Group(GroupMaster)`, shown as "the master
//!   of group 6".
//! - `Engine::record_group_master(page, slot, group) -> Result<PlaybackId, _>` (GRM-01).
//! - `change_playback` with `master: Some(Master::Group(..))` changes `mode`, `scale` and
//!   `enabled`; `group` is not an option (a different one is refused) and `level` in a change is
//!   ignored (the live level stays).
//! - The master level is the fader (`set_fader`), or 1.0 while Flash or Swop is held or Flash is
//!   latched; `GroupMaster.level` follows the fader while enabled (the level saved, GRM-13) and
//!   stays where it was while disabled (GRM-11). `add_playback` and `from_show` start the fader of
//!   a group master at its `level`.
//! - `ButtonKind::GroupMaster`: `[None, Flash]` in BestPractice and Titan; Custom allows Flash,
//!   Swop and None.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

mod common;

use std::collections::BTreeMap;

use cmdmx_core::attribute::{AttributeDef, AttributeGroup, AttributeKind};
use cmdmx_core::engine::{Engine, PlaybackSettingsChange};
use cmdmx_core::patch::Fixture;
use cmdmx_core::playback::{
    Button, ButtonKind, GroupMaster, GroupMode, HandleButton, HandleButtons, Kind, Master,
    MasterKind, Paging, Playback, PlaybackChange, PlaybackRecord, Values,
};
use cmdmx_core::profile::Profile;
use cmdmx_core::value::{Stored, Value};

use common::dmx;

const SPOTS: [u32; 6] = [301, 302, 303, 304, 305, 306];

/// DMX address of a spot in `common::church()` (301 = 35 … 306 = 40) or par 201 (21).
fn addr(id: u32) -> u16 {
    match id {
        201 => 21,
        _ => 35 + (id - 301) as u16,
    }
}

fn record_group(e: &mut Engine, group: u32, ids: &[u32]) {
    e.clear_selection();
    common::select(e, ids);
    e.record_group(group).unwrap();
    e.clear_selection();
}

fn dimmers(ids: &[u32], level: f32) -> Values {
    let mut v: Values = BTreeMap::new();
    for &id in ids {
        v.entry(id)
            .or_default()
            .insert("Dimmer".to_string(), level.into());
    }
    v
}

/// The church rig with group 6 = the front spots and playback 1 on 1.1 putting the spots and
/// par 201 at 80 %, fader full. Nothing on 1.8.
fn church() -> Engine {
    let mut e = common::church();
    record_group(&mut e, 6, &SPOTS);
    let mut values = dimmers(&SPOTS, 0.8);
    values.extend(dimmers(&[201], 0.8));
    e.add_playback(Playback::new(1, 1, 1, values));
    e.set_fader(1, 1.0, 0.0).unwrap();
    e
}

fn group_master(e: &Engine, id: u32) -> GroupMaster {
    match &e.playback(id).unwrap().master {
        Some(Master::Group(m)) => m.clone(),
        other => panic!("not a group master: {other:?}"),
    }
}

fn change(e: &mut Engine, id: u32, f: impl FnOnce(&mut GroupMaster)) -> Result<(), String> {
    let mut m = group_master(e, id);
    f(&mut m);
    e.change_playback(
        id,
        &PlaybackChange {
            master: Some(Master::Group(m)),
            ..PlaybackChange::default()
        },
    )
    .map_err(|err| err.to_string())
}

fn level(e: &Engine, id: u32) -> f32 {
    e.playback_state(id).map_or(0.0, |s| s.level)
}

// ---- GRM-01: a group master on a handle -------------------------------------------------------

#[test]
fn grm_01_record_a_group_master_on_an_empty_handle() {
    let mut e = church();
    let id = e.record_group_master(1, 8, 6).unwrap();
    let pb = e.playback(id).unwrap();
    assert_eq!((pb.page, pb.slot, pb.kind), (1, 8, Kind::GroupMaster));
    assert!(pb.kind.is_master());
    assert!(pb.cues.is_empty());
    assert_eq!(
        pb.master,
        Some(Master::Group(GroupMaster {
            group: 6,
            mode: GroupMode::Scale,
            scale: 100,
            enabled: true,
            level: 1.0,
        }))
    );
    assert_eq!(GroupMaster::new(6), group_master(&e, id));
    assert_eq!(
        Master::Group(GroupMaster::new(6)).to_string(),
        "the master of group 6"
    );
}

#[test]
fn grm_01_a_used_handle_or_a_missing_group_is_refused() {
    let mut e = church();
    assert!(e.record_group_master(1, 1, 6).is_err(), "used handle");
    assert!(e.record_group_master(1, 8, 7).is_err(), "no group 7");
    assert_eq!(e.playbacks().count(), 1, "nothing changed");
}

#[test]
fn grm_01_a_group_may_have_several_masters() {
    let mut e = church();
    e.record_group_master(1, 7, 6).unwrap();
    e.record_group_master(1, 8, 6).unwrap();
    e.record_group_master(2, 1, 6).unwrap();
    assert_eq!(e.playbacks().count(), 4);
}

// ---- GRM-02: the group, not a copy of it ---------------------------------------------------------

#[test]
fn grm_02_re_recording_the_group_changes_what_it_masters() {
    let mut e = church();
    let id = e.record_group_master(1, 8, 6).unwrap();
    e.set_fader(id, 0.0, 0.0).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, addr(301)), 0);
    record_group(&mut e, 6, &[201]);
    e.tick(0.025);
    assert_eq!(dmx(&e, addr(301)), 204, "301 left the group");
    assert_eq!(dmx(&e, addr(201)), 0, "201 joined it");
}

#[test]
fn grm_02_a_master_whose_fixtures_are_gone_masters_nothing_and_is_kept() {
    let mut e = church();
    let id = e.record_group_master(1, 8, 6).unwrap();
    e.set_fader(id, 0.0, 0.0).unwrap();
    e.patch_remove(&SPOTS).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, addr(201)), 204);
    assert!(e.playback(id).is_some());
}

// ---- GRM-03: a playback that never enters the merge ---------------------------------------------

#[test]
fn grm_03_a_group_master_never_fires() {
    let mut e = church();
    let id = e.record_group_master(1, 8, 6).unwrap();
    e.set_fader(id, 0.5, 0.0).unwrap();
    e.tick(0.0);
    assert!(!e.playback_state(id).unwrap().is_active());
    e.release_playback(id, 0.1).unwrap();
    e.tick(0.1);
    assert_eq!(level(&e, id), 0.5, "not killed or released");
    assert_eq!(dmx(&e, addr(301)), 102);
}

#[test]
fn grm_03_recording_onto_its_handle_is_refused() {
    let mut e = church();
    let id = e.record_group_master(1, 8, 6).unwrap();
    common::select(&mut e, &[201]);
    e.set_programmer_value(201, "Dimmer", 1.0).unwrap();
    let err = e.record_playback(&PlaybackRecord::at(1, 8)).unwrap_err();
    assert_eq!(err.to_string(), "Handle 1.8 holds the master of group 6");
    assert!(e.playback(id).unwrap().cues.is_empty());
}

#[test]
fn grm_03_kind_cannot_change_to_or_from_a_group_master() {
    let mut e = church();
    let id = e.record_group_master(1, 8, 6).unwrap();
    for (pb, kind) in [(id, Kind::Cues), (1, Kind::GroupMaster)] {
        let to = PlaybackChange {
            kind: Some(kind),
            ..PlaybackChange::default()
        };
        assert!(e.change_playback(pb, &to).is_err(), "{pb} to {kind:?}");
    }
}

#[test]
fn grm_03_locks_copies_moves_and_deletes_as_a_playback() {
    let mut e = church();
    let id = e.record_group_master(1, 8, 6).unwrap();
    e.change_playback(
        id,
        &PlaybackChange {
            paging: Some(Paging::Locked),
            ..PlaybackChange::default()
        },
    )
    .unwrap();
    change(&mut e, id, |m| m.mode = GroupMode::Limit).unwrap();
    let copy = e.copy_playback(id, 2, 8).unwrap();
    let m = group_master(&e, copy);
    assert_eq!(
        (m.group, m.mode),
        (6, GroupMode::Limit),
        "same group, same options"
    );
    e.move_playback(copy, 3, 8).unwrap();
    assert_eq!(e.playback(copy).unwrap().page, 3);

    e.set_fader(id, 0.0, 0.0).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, addr(301)), 0);
    e.delete_playback(id).unwrap();
    e.tick(0.025);
    assert_eq!(dmx(&e, addr(301)), 204, "deleting ends the mastering");
}

// ---- GRM-04: options -----------------------------------------------------------------------------

#[test]
fn grm_04_options_are_checked() {
    let mut e = church();
    let id = e.record_group_master(1, 8, 6).unwrap();
    for scale in [100, 200, 400, 600, 1000] {
        change(&mut e, id, |m| m.scale = scale).unwrap();
    }
    for scale in [0, 150, 300, 1001] {
        assert!(change(&mut e, id, |m| m.scale = scale).is_err(), "{scale}");
    }
    assert!(
        change(&mut e, id, |m| m.group = 7).is_err(),
        "the group is not an option"
    );
    let bpm = PlaybackChange {
        master: Some(Master::new(MasterKind::Bpm, 1)),
        ..PlaybackChange::default()
    };
    assert!(
        e.change_playback(id, &bpm).is_err(),
        "a group master stays one"
    );
    assert_eq!(group_master(&e, id).scale, 1000);
}

#[test]
fn grm_04_a_change_applies_from_the_next_tick_without_a_fade() {
    let mut e = church();
    let id = e.record_group_master(1, 8, 6).unwrap();
    e.set_fader(id, 0.5, 0.0).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, addr(301)), 102);
    change(&mut e, id, |m| m.mode = GroupMode::Limit).unwrap();
    e.tick(0.025);
    assert_eq!(dmx(&e, addr(301)), 128, "min(0.8, 0.5)");
    change(&mut e, id, |m| m.level = 0.0).unwrap();
    assert_eq!(
        group_master(&e, id).level,
        0.5,
        "the level is not an option"
    );
}

// ---- GRM-05: start level -------------------------------------------------------------------------

#[test]
fn grm_05_a_new_master_starts_where_it_changes_nothing() {
    let mut e = church();
    let id = e.record_group_master(1, 8, 6).unwrap();
    assert_eq!(level(&e, id), 1.0);
    e.tick(0.0);
    assert_eq!(dmx(&e, addr(301)), 204);
}

#[test]
fn grm_05_add_playback_starts_the_fader_at_the_saved_level() {
    let mut e = church();
    e.add_playback(Playback {
        kind: Kind::GroupMaster,
        master: Some(Master::Group(GroupMaster {
            mode: GroupMode::Htp,
            level: 0.25,
            ..GroupMaster::new(6)
        })),
        cues: Vec::new(),
        ..Playback::new(2, 1, 8, Values::new())
    });
    assert_eq!(level(&e, 2), 0.25);
}

#[test]
fn grm_05_the_fader_is_the_saved_level() {
    let mut e = church();
    let id = e.record_group_master(1, 8, 6).unwrap();
    e.set_fader(id, 0.3, 0.0).unwrap();
    assert_eq!(group_master(&e, id).level, 0.3);
    e.set_flash(id, true, 0.0).unwrap();
    assert_eq!(group_master(&e, id).level, 0.3, "a held Flash is not saved");
}

// ---- GRM-06..09: the modes on the output ---------------------------------------------------------

#[test]
fn grm_06_the_programmer_is_mastered_too() {
    let mut e = church();
    common::select(&mut e, &[301]);
    e.set_programmer_value(301, "Dimmer", 1.0).unwrap();
    let id = e.record_group_master(1, 8, 6).unwrap();
    e.set_fader(id, 0.0, 0.0).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, addr(301)), 0);
}

#[test]
fn grm_06_a_fixture_with_no_source_gets_the_htp_level() {
    let mut e = church();
    e.set_fader(1, 0.0, 0.0).unwrap();
    let id = e.record_group_master(1, 8, 6).unwrap();
    change(&mut e, id, |m| m.mode = GroupMode::Htp).unwrap();
    e.set_fader(id, 0.4, 0.0).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, addr(301)), 102);
    assert_eq!(dmx(&e, addr(201)), 0, "not in the group");
}

#[test]
fn grm_06_scale_then_htp_then_limit_then_the_grand_master() {
    let mut e = church();
    let scale = e.record_group_master(1, 6, 6).unwrap();
    let htp = e.record_group_master(1, 7, 6).unwrap();
    let limit = e.record_group_master(1, 8, 6).unwrap();
    change(&mut e, htp, |m| m.mode = GroupMode::Htp).unwrap();
    change(&mut e, limit, |m| m.mode = GroupMode::Limit).unwrap();
    e.set_fader(scale, 0.25, 0.0).unwrap(); // 0.8 → 0.2
    e.set_fader(htp, 0.6, 0.0).unwrap(); // → 0.6
    e.set_fader(limit, 0.4, 0.0).unwrap(); // → 0.4
    e.set_grand_master(0.5).unwrap(); // → 0.2
    e.tick(0.0);
    assert_eq!(dmx(&e, addr(301)), 51);
    assert_eq!(dmx(&e, addr(201)), 102, "only the Grand Master");
}

#[test]
fn grm_06_dbo_takes_out_an_htp_master() {
    let mut e = church();
    let id = e.record_group_master(1, 8, 6).unwrap();
    change(&mut e, id, |m| m.mode = GroupMode::Htp).unwrap();
    e.set_dbo(true);
    e.tick(0.0);
    assert_eq!(dmx(&e, addr(301)), 0);
}

// ---- GRM-10: Take Over ---------------------------------------------------------------------------

fn take_over(e: &mut Engine) -> u32 {
    let id = e.record_group_master(1, 8, 6).unwrap();
    change(e, id, |m| m.mode = GroupMode::TakeOver).unwrap();
    e.set_fader(id, 0.0, 0.0).unwrap();
    e.tick(0.0);
    id
}

#[test]
fn grm_10_take_over_does_not_act_until_it_passes() {
    let mut e = church();
    let id = take_over(&mut e);
    assert_eq!(dmx(&e, addr(301)), 204, "at 0 it changes nothing");
    e.set_fader(id, 0.5, 0.025).unwrap();
    e.tick(0.025);
    e.tick(0.05);
    assert_eq!(dmx(&e, addr(301)), 204);
    assert!(!e.programmer().in_programmer(301));
}

#[test]
fn grm_10_passing_puts_dimmer_in_the_programmer_and_it_follows() {
    let mut e = church();
    let id = take_over(&mut e);
    e.set_fader(id, 1.0, 0.025).unwrap(); // passes 0.8
    e.tick(0.025);
    e.tick(0.05);
    for &spot in &SPOTS {
        assert_eq!(
            e.programmer().stored(spot, "Dimmer"),
            Some(Stored::Value(Value::Level(1.0))),
            "{spot}"
        );
    }
    assert!(
        e.programmer().selection().is_empty(),
        "the selection does not change"
    );
    assert_eq!(dmx(&e, addr(301)), 255);
    e.set_fader(id, 0.2, 0.075).unwrap();
    e.tick(0.075);
    e.tick(0.1);
    assert_eq!(dmx(&e, addr(301)), 51, "follows the fader");
    assert!(!e.programmer().in_programmer(201));
}

#[test]
fn grm_10_cleared_fixtures_are_picked_up_on_the_next_pass() {
    let mut e = church();
    let id = take_over(&mut e);
    e.set_fader(id, 1.0, 0.025).unwrap();
    e.tick(0.025);
    e.tick(0.05);
    e.clear();
    e.tick(0.075);
    assert_eq!(dmx(&e, addr(301)), 204, "back to the playback");
    e.set_fader(id, 0.9, 0.1).unwrap(); // 1.0 → 0.9 does not pass 0.8
    e.tick(0.1);
    e.tick(0.125);
    assert_eq!(dmx(&e, addr(301)), 204);
    assert!(!e.programmer().in_programmer(301));
    e.set_fader(id, 0.6, 0.15).unwrap(); // passes 0.8
    e.tick(0.15);
    e.tick(0.175);
    assert_eq!(dmx(&e, addr(301)), 153);
}

#[test]
fn grm_10_a_fixture_without_a_dimmer_is_skipped() {
    let mut e = church();
    e.add_profile(
        "mover",
        Profile::from_attributes(vec![AttributeDef::new(
            "Pan",
            AttributeGroup::P,
            AttributeKind::Ltp,
            1,
        )]),
    )
    .unwrap();
    e.patch(Fixture {
        id: 900,
        number: 0,
        name: String::new(),
        handle: None,
        profile: "mover".into(),
        mode: None,
        universe: 1,
        address: 100,
    })
    .unwrap();
    record_group(&mut e, 6, &[301, 900]);
    let id = take_over(&mut e);
    e.set_fader(id, 1.0, 0.025).unwrap();
    e.tick(0.025);
    e.tick(0.05);
    assert!(e.programmer().in_programmer(301));
    assert!(!e.programmer().in_programmer(900));
}

// ---- GRM-11: Disabled ----------------------------------------------------------------------------

#[test]
fn grm_11_disabled_locks_the_last_effect_and_enabling_jumps_to_the_fader() {
    let mut e = church();
    let id = e.record_group_master(1, 8, 6).unwrap();
    e.set_fader(id, 0.5, 0.0).unwrap();
    change(&mut e, id, |m| m.enabled = false).unwrap();
    e.set_fader(id, 0.0, 0.0).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, addr(301)), 102, "locked at 50 %");
    assert_eq!(
        group_master(&e, id).level,
        0.5,
        "the locked level is the saved one"
    );
    assert!(e.set_flash(id, true, 0.0).is_err(), "Flash is refused");
    change(&mut e, id, |m| m.enabled = true).unwrap();
    e.tick(0.025);
    assert_eq!(dmx(&e, addr(301)), 0, "the fader counts again");
    assert_eq!(group_master(&e, id).level, 0.0);
}

// ---- GRM-12: buttons -----------------------------------------------------------------------------

#[test]
fn grm_12_default_buttons_are_none_and_flash() {
    let mut e = church();
    let id = e.record_group_master(1, 8, 6).unwrap();
    assert_eq!(
        ButtonKind::GroupMaster.best_practice(),
        [Button::None, Button::Flash]
    );
    assert_eq!(e.handle_buttons(id), Some([Button::None, Button::Flash]));
    e.set_playback_settings(&PlaybackSettingsChange {
        handle_buttons: Some(HandleButtons::Titan),
        ..PlaybackSettingsChange::default()
    })
    .unwrap();
    assert_eq!(e.handle_buttons(id), Some([Button::None, Button::Flash]));
}

#[test]
fn grm_12_custom_buttons_are_flash_swop_or_none() {
    for (pair, ok) in [
        ([Button::Swop, Button::Flash], true),
        ([Button::None, Button::Swop], true),
        ([Button::Go, Button::Flash], false),
        ([Button::Tap, Button::None], false),
        ([Button::Release, Button::None], false),
    ] {
        let mut custom = BTreeMap::new();
        custom.insert(ButtonKind::GroupMaster, pair);
        assert_eq!(
            HandleButtons::Custom(custom).check().is_ok(),
            ok,
            "{pair:?}"
        );
    }
}

#[test]
fn grm_12_flash_puts_the_master_at_full() {
    let mut e = church();
    let id = e.record_group_master(1, 8, 6).unwrap();
    e.set_fader(id, 0.0, 0.0).unwrap();
    e.press_button(id, HandleButton::Lower, true, 0.0).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, addr(301)), 204);
    e.press_button(id, HandleButton::Lower, false, 0.025)
        .unwrap();
    e.tick(0.025);
    assert_eq!(dmx(&e, addr(301)), 0);
}

#[test]
fn grm_12_flash_swop_and_latch_are_refused_on_take_over_and_disabled() {
    let mut e = church();
    let id = e.record_group_master(1, 8, 6).unwrap();
    for f in [
        (|m: &mut GroupMaster| m.mode = GroupMode::TakeOver) as fn(&mut GroupMaster),
        |m: &mut GroupMaster| m.enabled = false,
    ] {
        let mut fresh = church();
        let id = fresh.record_group_master(1, 8, 6).unwrap();
        change(&mut fresh, id, f).unwrap();
        let before = fresh.playback_state(id).cloned();
        assert!(fresh.set_flash(id, true, 0.0).is_err());
        assert!(fresh.set_swop(id, true, 0.0).is_err());
        assert!(fresh.latch_flash(id, 0.0).is_err());
        assert_eq!(
            fresh.playback_state(id).cloned(),
            before,
            "GRM-19: nothing changed"
        );
    }
    e.set_flash(id, true, 0.0).unwrap();
    e.set_flash(id, false, 0.0).unwrap();
    e.latch_flash(id, 0.0).unwrap();
    e.latch_flash(id, 0.0).unwrap();
    e.set_swop(id, true, 0.0).unwrap();
    e.set_swop(id, false, 0.0).unwrap();
}

#[test]
fn grm_12_speed_masters_still_refuse_flash() {
    let mut e = church();
    let id = e.record_master(1, 8, MasterKind::Rate, 1).unwrap();
    assert!(e.set_flash(id, true, 0.0).is_err());
}

// ---- GRM-19: determinism -------------------------------------------------------------------------

#[test]
fn grm_19_same_actions_give_the_same_frames() {
    let run = || {
        let mut e = church();
        let a = e.record_group_master(1, 7, 6).unwrap();
        let b = e.record_group_master(1, 8, 6).unwrap();
        change(&mut e, b, |m| m.mode = GroupMode::TakeOver).unwrap();
        let mut frames = Vec::new();
        for i in 0..40 {
            let t = f64::from(i) * 0.025;
            e.set_fader(a, (i % 7) as f32 / 7.0, t).unwrap();
            e.set_fader(b, (i % 5) as f32 / 5.0, t).unwrap();
            e.tick(t);
            frames.push(e.frame(1).unwrap().slots().to_vec());
        }
        frames
    };
    assert_eq!(run(), run());
}
