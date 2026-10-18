//! Playback behaviour (VS2-21): `specs/modules/playbacks.md` PBK-15..23, 25..30. What reaches
//! the frame is mostly in the scenarios `tests/scenarios/PBK-*.json`; these tests cover the
//! state the IPC will show (active, releasing, Flash, Latch, DBO), references while fading and
//! the cases a scenario cannot express.
//!
//! API these tests fix (on top of VS2-20):
//! - `Engine::set_flash(id, on, now)`, `set_swop(id, on, now)`, `latch_flash(id, now)` (toggles,
//!   PBK-28), `release_playback(id, now)` (PBK-22), `set_dbo(on)`, `dbo()`.
//! - `PlaybackState`: `is_active()` (in the merge), `is_releasing()` (killed, its masked
//!   attributes still fading back, PBK-20), `flash` and `swop` (held), `latched`.
//! - Scenario actions `flash`/`swop {playback, on}`, `latch {playback}`, `release {playback}`,
//!   `dbo {on}`.
//! - Time model: *m* = effective level · fade-in progress (PBK-15); in mode 1 a fall of the
//!   target makes *m* fall linearly from its value at that tick at full scale per `fade_out`
//!   (PBK-16); every time counts from the tick in which the action reached the engine.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

mod common;

use std::collections::BTreeMap;

use cmdmx_core::attribute::AttributeGroup::{B, P};
use cmdmx_core::engine::{Engine, PlaybackSettingsChange};
use cmdmx_core::palette::{Cell, Record, RecordMode, Window};
use cmdmx_core::playback::{FaderMode, KillPoint, Playback, Values};
use cmdmx_core::show::Meta;
use cmdmx_core::value::{Reference, Stored, Value};

const TICK: f64 = 0.025;

/// `common::church()`: wash 101 at 1 (Dimmer 1, Pan 2–3, Tilt 4, Zoom 5), dimmers 201… from 21.
fn rig() -> Engine {
    common::church()
}

fn values(entries: &[(u32, &str, f32)]) -> Values {
    let mut v: Values = BTreeMap::new();
    for (id, name, level) in entries {
        v.entry(*id)
            .or_default()
            .insert((*name).to_string(), (*level).into());
    }
    v
}

/// Tick from `from` to `to` (inclusive) on the 25 ms grid.
fn run(e: &mut Engine, from: f64, to: f64) {
    let mut i = (from / TICK).round() as u64;
    loop {
        let t = i as f64 * TICK;
        if t > to + 1e-9 {
            break;
        }
        e.tick(t);
        i += 1;
    }
}

fn ch(e: &Engine, address: u16) -> u8 {
    e.frame(1).unwrap().channel(address).unwrap()
}

fn active(e: &Engine, id: u32) -> bool {
    e.playback_state(id).unwrap().is_active()
}

fn releasing(e: &Engine, id: u32) -> bool {
    e.playback_state(id).unwrap().is_releasing()
}

// ---- PBK-19: kill point ---------------------------------------------------------------------

#[test]
fn pbk_19_fade_out_complete_keeps_the_playback_active_until_it_is_out() {
    let mut e = rig();
    let mut pb = Playback {
        fader_mode: FaderMode::Mode1,
        ..Playback::new(1, 1, 1, values(&[(101, "Dimmer", 1.0)]))
    };
    pb.cues[0].fade_out = 1.0;
    e.add_playback(pb);
    e.set_fader(1, 1.0, 0.0).unwrap();
    run(&mut e, 0.0, 0.5);
    e.set_fader(1, 0.0, 0.5).unwrap();
    run(&mut e, 0.5, 0.75);
    assert!(active(&e, 1), "still fading out");
    assert_eq!(ch(&e, 1), 191, "a quarter of the way down over 1 s");
    run(&mut e, 0.775, 1.6);
    assert!(!active(&e, 1), "killed once the fade-out is complete");
    assert_eq!(ch(&e, 1), 0);
}

#[test]
fn pbk_19_fader_at_zero_kills_at_once() {
    let mut e = rig();
    let mut pb = Playback {
        fader_mode: FaderMode::Mode1,
        kill_point: KillPoint::FaderAtZero,
        ..Playback::new(1, 1, 1, values(&[(101, "Dimmer", 1.0)]))
    };
    pb.cues[0].fade_out = 4.0;
    e.add_playback(pb);
    e.set_fader(1, 1.0, 0.0).unwrap();
    run(&mut e, 0.0, 0.5);
    e.set_fader(1, 0.0, 0.5).unwrap();
    run(&mut e, 0.5, 0.5);
    assert!(!active(&e, 1));
    assert_eq!(ch(&e, 1), 0);
}

// ---- PBK-20, 21: release ----------------------------------------------------------------------

/// Playback 1 on wash 101: Zoom 1.0, fader mode 0, no local mask or time.
fn zoom_playback() -> Engine {
    let mut e = rig();
    e.add_playback(Playback::new(1, 1, 1, values(&[(101, "Zoom", 1.0)])));
    e
}

#[test]
fn pbk_20_without_a_mask_ltp_values_are_held() {
    let mut e = zoom_playback();
    e.set_fader(1, 1.0, 0.0).unwrap();
    run(&mut e, 0.0, 0.5);
    e.set_fader(1, 0.0, 0.5).unwrap();
    run(&mut e, 0.5, 3.0);
    assert_eq!(ch(&e, 5), 255);
    assert!(!releasing(&e, 1), "nothing in the (empty, global) mask");
}

#[test]
fn pbk_20_the_global_mask_and_time_apply_without_local_ones() {
    let mut e = zoom_playback();
    e.set_playback_settings(&PlaybackSettingsChange {
        release_mask: Some(vec![B]),
        release_time: Some(1.0),
        ..PlaybackSettingsChange::default()
    })
    .unwrap();
    e.set_fader(1, 1.0, 0.0).unwrap();
    run(&mut e, 0.0, 0.5);
    e.set_fader(1, 0.0, 0.5).unwrap();
    run(&mut e, 0.5, 0.75);
    assert!(releasing(&e, 1));
    assert!(!active(&e, 1), "killed, but still releasing");
    assert_eq!(ch(&e, 5), 191, "a quarter of the way back over 1 s");
    run(&mut e, 0.775, 1.6);
    assert_eq!(
        ch(&e, 5),
        0,
        "back to the value before the playback took it"
    );
    assert!(!releasing(&e, 1), "done");
}

#[test]
fn pbk_20_a_local_mask_overrides_the_global_one() {
    let mut e = zoom_playback();
    e.set_playback_settings(&PlaybackSettingsChange {
        release_mask: Some(vec![B]),
        release_time: Some(0.0),
        ..PlaybackSettingsChange::default()
    })
    .unwrap();
    e.change_playback(
        1,
        &cmdmx_core::playback::PlaybackChange {
            release_mask: Some(Some(vec![P])),
            ..Default::default()
        },
    )
    .unwrap();
    e.set_fader(1, 1.0, 0.0).unwrap();
    run(&mut e, 0.0, 0.5);
    e.set_fader(1, 0.0, 0.5).unwrap();
    run(&mut e, 0.5, 1.0);
    assert_eq!(ch(&e, 5), 255, "the local mask (P) does not hold Zoom (B)");
}

#[test]
fn pbk_21_firing_again_ends_the_release() {
    let mut e = zoom_playback();
    e.change_playback(
        1,
        &cmdmx_core::playback::PlaybackChange {
            release_mask: Some(Some(vec![B])),
            release_time: Some(Some(4.0)),
            ..Default::default()
        },
    )
    .unwrap();
    e.set_fader(1, 1.0, 0.0).unwrap();
    run(&mut e, 0.0, 0.5);
    e.set_fader(1, 0.0, 0.5).unwrap();
    run(&mut e, 0.5, 1.5);
    assert!(releasing(&e, 1));
    e.set_fader(1, 1.0, 1.5).unwrap();
    run(&mut e, 1.5, 1.5);
    assert!(!releasing(&e, 1));
    assert!(active(&e, 1));
    assert_eq!(ch(&e, 5), 255);
}

// ---- PBK-22: Release + handle -------------------------------------------------------------------

#[test]
fn pbk_22_release_needs_the_fader_through_zero_to_fire_again() {
    let mut e = zoom_playback();
    e.set_fader(1, 1.0, 0.0).unwrap();
    run(&mut e, 0.0, 0.5);
    e.release_playback(1, 0.5).unwrap();
    run(&mut e, 0.5, 3.0);
    assert!(!active(&e, 1));
    assert_eq!(ch(&e, 5), 0, "everything released, over the global 2 s");
    e.set_fader(1, 0.5, 3.0).unwrap();
    run(&mut e, 3.0, 3.1);
    assert!(!active(&e, 1), "a fader still up does not fire it again");
    e.set_fader(1, 0.0, 3.1).unwrap();
    e.set_fader(1, 1.0, 3.2).unwrap();
    run(&mut e, 3.2, 3.2);
    assert!(active(&e, 1));
}

#[test]
fn pbk_22_release_on_an_inactive_playback_does_nothing() {
    let mut e = zoom_playback();
    run(&mut e, 0.0, 0.1);
    let before = e.frame(1).unwrap().slots().to_vec();
    e.release_playback(1, 0.1).unwrap();
    run(&mut e, 0.1, 0.2);
    assert_eq!(e.frame(1).unwrap().slots().to_vec(), before);
    assert!(!releasing(&e, 1));
    assert!(e.release_playback(9, 0.2).is_err(), "unknown playback");
}

// ---- PBK-18: references while fading ------------------------------------------------------------

#[test]
fn pbk_18_a_fade_runs_towards_the_references_current_value() {
    let mut e = rig();
    // Palette 1 on Beams cell 1: wash 101 Zoom 1.0.
    common::select(&mut e, &[101]);
    e.set_programmer_value(101, "Zoom", 1.0).unwrap();
    e.record_palette(&Record::at(Cell::new(Window::Beams, 1)))
        .unwrap();
    e.clear_selection();
    e.clear();
    let palette = e.palette(1).unwrap().id;
    let mut pb = Playback::new(
        1,
        1,
        1,
        BTreeMap::from([(
            101,
            BTreeMap::from([(
                "Zoom".to_string(),
                Stored::Reference(Reference {
                    palette,
                    value: Value::Level(1.0),
                }),
            )]),
        )]),
    );
    pb.cues[0].fade_in = 2.0;
    e.add_playback(pb);
    e.set_fader(1, 1.0, 0.0).unwrap();
    run(&mut e, 0.0, 1.0);
    assert_eq!(ch(&e, 5), 128, "half way to 1.0");
    // The palette becomes Zoom 0.6 half way through the fade.
    common::select(&mut e, &[101]);
    e.set_programmer_value(101, "Zoom", 0.6).unwrap();
    e.record_palette(&Record {
        mode: Some(RecordMode::Replace),
        ..Record::at(Cell::new(Window::Beams, 1))
    })
    .unwrap();
    e.clear_selection();
    e.clear();
    run(&mut e, 1.025, 2.0);
    assert_eq!(ch(&e, 5), 153, "the fade ends on the palette's new value");
}

// ---- PBK-25: playback groups --------------------------------------------------------------------

#[test]
fn pbk_25_the_other_playback_of_the_group_is_killed() {
    let mut e = rig();
    for (id, fixture) in [(1, 201), (2, 202), (3, 203)] {
        let mut pb = Playback::new(id, 1, id as u16, values(&[(fixture, "Dimmer", 1.0)]));
        pb.group = if id == 3 { Some(2) } else { Some(1) };
        e.add_playback(pb);
    }
    e.set_fader(1, 1.0, 0.0).unwrap();
    e.set_fader(3, 1.0, 0.0).unwrap();
    run(&mut e, 0.0, 0.5);
    e.set_fader(2, 1.0, 0.5).unwrap();
    run(&mut e, 0.5, 0.6);
    assert!(!active(&e, 1));
    assert!(active(&e, 2));
    assert!(active(&e, 3), "another group is not touched");
    assert_eq!(
        e.playback_state(1).unwrap().level,
        1.0,
        "its fader stays up"
    );
}

// ---- PBK-26..28: Flash, Swop, Latch -------------------------------------------------------------

#[test]
fn pbk_26_flash_state_and_unknown_playbacks() {
    let mut e = zoom_playback();
    e.set_flash(1, true, 0.0).unwrap();
    run(&mut e, 0.0, 0.0);
    assert!(e.playback_state(1).unwrap().flash);
    assert!(active(&e, 1));
    e.set_flash(1, false, 0.1).unwrap();
    run(&mut e, 0.025, 0.1);
    assert!(!e.playback_state(1).unwrap().flash);
    assert!(!active(&e, 1));
    assert!(e.set_flash(9, true, 0.1).is_err());
    assert!(e.set_swop(9, true, 0.1).is_err());
    assert!(e.latch_flash(9, 0.1).is_err());
}

#[test]
fn pbk_27_the_later_swop_wins() {
    let mut e = rig();
    for (id, fixture) in [(1, 201), (2, 202), (3, 203)] {
        e.add_playback(Playback::new(
            id,
            1,
            id as u16,
            values(&[(fixture, "Dimmer", 1.0)]),
        ));
    }
    e.set_fader(1, 1.0, 0.0).unwrap();
    e.set_swop(2, true, 0.5).unwrap();
    run(&mut e, 0.0, 0.5);
    e.set_swop(3, true, 0.6).unwrap();
    run(&mut e, 0.525, 0.6);
    assert_eq!((ch(&e, 21), ch(&e, 22), ch(&e, 23)), (0, 0, 255));
    assert!(e.playback_state(3).unwrap().swop);
}

#[test]
fn pbk_28_latch_toggles() {
    let mut e = zoom_playback();
    e.latch_flash(1, 0.0).unwrap();
    run(&mut e, 0.0, 0.1);
    assert!(e.playback_state(1).unwrap().latched);
    assert!(active(&e, 1));
    e.latch_flash(1, 0.1).unwrap();
    run(&mut e, 0.1, 0.2);
    assert!(!e.playback_state(1).unwrap().latched);
    assert!(!active(&e, 1));
}

// ---- PBK-30: DBO ------------------------------------------------------------------------------

#[test]
fn pbk_30_dbo_is_live_state_and_works_with_the_grand_master() {
    let mut e = rig();
    e.add_playback(Playback::new(1, 1, 1, values(&[(101, "Dimmer", 1.0)])));
    assert!(!e.dbo());
    e.set_fader(1, 1.0, 0.0).unwrap();
    e.set_grand_master(0.5).unwrap();
    run(&mut e, 0.0, 0.1);
    assert_eq!(ch(&e, 1), 128);
    let meta = Meta {
        name: "t".into(),
        created: String::new(),
        modified: String::new(),
        app_version: String::new(),
    };
    let show = e.to_show(meta.clone());
    e.set_dbo(true);
    assert!(e.dbo());
    run(&mut e, 0.125, 0.2);
    assert_eq!(ch(&e, 1), 0);
    assert_eq!(e.to_show(meta), show, "DBO is not in the show");
    assert!(
        !Engine::from_show(&show).0.dbo(),
        "off when a show is loaded"
    );
    e.set_dbo(false);
    run(&mut e, 0.225, 0.3);
    assert_eq!(ch(&e, 1), 128);
}

// ---- PBK-43: determinism ------------------------------------------------------------------------

#[test]
fn pbk_43_same_actions_same_frames() {
    let go = || {
        let mut e = rig();
        let mut pb = Playback {
            fader_mode: FaderMode::Mode1,
            ..Playback::new(1, 1, 1, values(&[(101, "Dimmer", 1.0), (101, "Zoom", 1.0)]))
        };
        pb.cues[0].fade_in = 1.0;
        pb.cues[0].fade_out = 1.5;
        pb.release_mask = Some(vec![B]);
        e.add_playback(pb);
        let mut frames = Vec::new();
        e.set_fader(1, 0.7, 0.0).unwrap();
        run(&mut e, 0.0, 0.5);
        e.set_flash(1, true, 0.5).unwrap();
        run(&mut e, 0.525, 0.8);
        e.set_flash(1, false, 0.8).unwrap();
        e.set_fader(1, 0.0, 0.9).unwrap();
        for i in 0..80 {
            let t = 0.825 + f64::from(i) * TICK;
            e.tick(t);
            frames.push(e.frame(1).unwrap().slots().to_vec());
        }
        frames
    };
    assert_eq!(go(), go());
}
