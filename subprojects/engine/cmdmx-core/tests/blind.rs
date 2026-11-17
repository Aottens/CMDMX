//! Blind (BLD-01): `specs/modules/programmer.md` PRG-105..109.
//!
//! API these tests fix (`Engine`):
//! - `enter_blind()`: the stage keeps the programmer as it is (PRG-105); again while Blind is a no-op.
//! - `leave_blind(fade: f32, clear: bool) -> Result<(), CoreError>` (PRG-107): a negative or
//!   non-finite fade is refused; in Live it is a no-op.
//! - `blind() -> bool`.
//! - `set_highlight` in Blind is `CoreError::BlindHighlight` (PRG-106), and so is
//!   `step_fixture(forward: bool) -> Result<(), CoreError>` (Fix ±1) while Highlight is on;
//!   `step_fixture` is `next_fixture` / `prev_fixture` otherwise. Deselecting in Blind does not
//!   switch Highlight off (PRG-26 would change the stage).
//! - `guarded(confirm: bool, f: FnOnce(&mut Engine) -> Result<T, CoreError>)` (PRG-108): in Blind
//!   without `confirm`, when `f` changes the stored values of a cue live on a playback or of a
//!   palette something on stage references, it is undone and `CoreError::OnStage(sentence)` is
//!   returned; otherwise `f`'s result.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;

use cmdmx_core::colour::TargetColour;
use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::palette::{GlobalValue, Palette, RecordMode, Timing, Window};
use cmdmx_core::patch::Fixture;
use cmdmx_core::playback::{Playback, PlaybackRecord, Values};
use cmdmx_core::profile::Profile;
use cmdmx_core::show::Meta;
use cmdmx_core::undo::History;
use cmdmx_core::value::{Stored, Value};
use cmdmx_core::CoreError;

const WASH: &str = r#"{ "manufacturer": "T", "model": "Wash", "source": "channel-list", "modes": [ {
  "name": "4ch", "footprint": 4,
  "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 2 },
    { "name": "Tilt", "group": "P", "kind": "Ltp", "channel": 3 },
    { "name": "Zoom", "group": "B", "kind": "Ltp", "channel": 4 } ] } ] }"#;

const RGBW: &str = r#"{ "manufacturer": "T", "model": "RGBW Par", "source": "channel-list", "modes": [ {
  "name": "5ch", "footprint": 5,
  "colour": { "system": "RGB", "components": [ { "channel": 2 }, { "channel": 3 }, { "channel": 4 } ] },
  "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "White", "group": "C", "kind": "Ltp", "channel": 5, "emitter": true } ] } ] }"#;

/// Palette 81: Dimmer full (global).
const FULL: u32 = 81;

/// Washes 101 @1, 102 @5, 103 @9, 104 @13 (Dimmer, Pan, Tilt, Zoom); RGBW par 201 @17
/// (Dimmer 17, RGB 18–20, White 21). Palette 81 Dimmer full.
fn rig() -> Engine {
    let mut e = Engine::new(Settings::default());
    e.add_profile("wash", Profile::from_json(WASH).unwrap())
        .unwrap();
    e.add_profile("rgbw", Profile::from_json(RGBW).unwrap())
        .unwrap();
    for (id, profile, address) in [
        (101, "wash", 1),
        (102, "wash", 5),
        (103, "wash", 9),
        (104, "wash", 13),
        (201, "rgbw", 17),
    ] {
        e.patch(Fixture {
            parked: false,
            behaviour: Default::default(),
            swap_pan_tilt: false,
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
    e.add_palette(Palette {
        id: 1,
        number: FULL,
        window: Window::Beams,
        handle: 1,
        legend: "Full".into(),
        icon: None,
        global: BTreeMap::from([("Dimmer".to_string(), GlobalValue::Value(Value::Level(1.0)))]),
        shared: BTreeMap::new(),
        shared_cells: BTreeMap::new(),
        fixtures: BTreeMap::new(),
        effects: Vec::new(),
    })
    .unwrap();
    e
}

fn select(e: &mut Engine, ids: &[u32]) {
    for &id in ids {
        e.select_fixture(id).unwrap();
    }
}

fn timed(time: f32) -> Timing {
    Timing {
        time: Some(time),
        overlap: None,
    }
}

/// A playback on page 1, slot `id`, its fader up when `up`.
fn playback(e: &mut Engine, id: u32, entries: &[(u32, &str, f32)], up: bool) {
    let mut v: Values = BTreeMap::new();
    for &(fixture, attr, val) in entries {
        v.entry(fixture.into())
            .or_default()
            .insert(attr.to_string(), val.into());
    }
    e.add_playback(Playback::new(id, 1, id as u16, v));
    if up {
        e.set_fader(id, 1.0, 0.0).unwrap();
    }
}

fn merge_into(e: &mut Engine, slot: u16) -> Result<u32, CoreError> {
    e.record_playback(&PlaybackRecord {
        mode: Some(RecordMode::Merge),
        ..PlaybackRecord::at(1, slot)
    })
}

fn cue_level(e: &Engine, playback: u32, fixture: u32) -> Option<f32> {
    let cue = e.playback(playback)?.cues.first()?;
    match cue.values.get(&fixture.into())?.get("Dimmer")? {
        Stored::Value(Value::Level(l)) => Some(*l),
        _ => None,
    }
}

fn dmx(e: &Engine, address: u16) -> u8 {
    e.frame(1).and_then(|f| f.channel(address)).unwrap()
}

fn assert_near(e: &Engine, address: u16, expected: u8) {
    let actual = dmx(e, address);
    assert!(
        actual.abs_diff(expected) <= 1,
        "address {address}: expected about {expected}, got {actual}"
    );
}

fn level(e: &Engine, fixture: u32, attribute: &str) -> Option<f32> {
    match e.programmer().value(fixture, attribute) {
        Some(Value::Level(l)) => Some(l),
        _ => None,
    }
}

fn red() -> Value {
    Value::Colour(TargetColour {
        r: 1.0,
        g: 0.0,
        b: 0.0,
    })
}

// ---- PRG-105: Blind freezes what the programmer gives the stage --------------------------

#[test]
fn prg_105_entering_blind_changes_nothing_and_is_live_state() {
    let mut e = rig();
    e.set_programmer_value(101, "Dimmer", 0.5).unwrap();
    e.tick(0.0);
    assert!(!e.blind());
    e.enter_blind();
    e.enter_blind(); // again: a no-op
    assert!(e.blind());
    e.tick(0.1);
    assert_near(&e, 1, 128);
}

#[test]
fn prg_105_changes_in_blind_reach_the_programmer_not_the_stage() {
    let mut e = rig();
    e.set_programmer_value(101, "Dimmer", 0.5).unwrap();
    e.tick(0.0);
    e.enter_blind();
    e.set_programmer_value(101, "Dimmer", 1.0).unwrap();
    e.set_programmer_value(102, "Dimmer", 1.0).unwrap();
    select(&mut e, &[103]);
    e.apply_palette(FULL, timed(0.0)).unwrap();
    e.tick(0.1);
    assert_near(&e, 1, 128);
    assert_eq!(dmx(&e, 5), 0);
    assert_eq!(dmx(&e, 9), 0);
    // The operator sees and records from the programmer as it is now.
    assert_eq!(level(&e, 101, "Dimmer"), Some(1.0));
    assert_eq!(level(&e, 102, "Dimmer"), Some(1.0));
    // A Clear empties that programmer, not the stage.
    e.clear();
    e.clear();
    assert!(e.programmer().is_empty());
    e.tick(0.2);
    assert_near(&e, 1, 128);
}

#[test]
fn prg_105_an_undo_in_blind_leaves_the_frame() {
    let mut e = rig();
    let mut history = History::default();
    e.set_programmer_value(101, "Dimmer", 0.5).unwrap();
    e.tick(0.0);
    e.enter_blind();
    history
        .edit(&mut e, "Dimmer", None, |e| {
            e.set_programmer_value(101, "Dimmer", 1.0)
        })
        .unwrap();
    history.undo(&mut e).unwrap();
    assert_eq!(level(&e, 101, "Dimmer"), Some(0.5));
    history.redo(&mut e).unwrap();
    assert_eq!(level(&e, 101, "Dimmer"), Some(1.0));
    e.tick(0.1);
    assert_near(&e, 1, 128);
}

#[test]
fn prg_105_a_playback_fired_in_blind_is_on_stage() {
    let mut e = rig();
    e.tick(0.0);
    e.enter_blind();
    playback(&mut e, 1, &[(102, "Dimmer", 0.6)], true);
    e.tick(0.1);
    assert_eq!(dmx(&e, 5), 153);
}

#[test]
fn prg_105_a_running_palette_fade_plays_to_its_end() {
    let mut e = rig();
    select(&mut e, &[101]);
    e.apply_palette(FULL, timed(2.0)).unwrap();
    e.tick(0.0);
    e.tick(1.0);
    assert_near(&e, 1, 128);
    e.enter_blind();
    e.tick(1.5);
    assert_near(&e, 1, 191);
    e.tick(2.0);
    assert_eq!(dmx(&e, 1), 255);
    e.tick(3.0);
    assert_eq!(dmx(&e, 1), 255);
}

// ---- PRG-106: Highlight in Blind ---------------------------------------------------------

#[test]
fn prg_106_highlight_keeps_what_it_lit_and_cannot_change_in_blind() {
    let mut e = rig();
    select(&mut e, &[101, 102]);
    e.set_highlight(true).unwrap();
    e.tick(0.0);
    assert_eq!((dmx(&e, 1), dmx(&e, 5)), (255, 255));
    e.enter_blind();
    assert!(matches!(
        e.set_highlight(false),
        Err(CoreError::BlindHighlight)
    ));
    assert!(e.highlight());
    assert!(matches!(
        e.step_fixture(true),
        Err(CoreError::BlindHighlight)
    ));
    // Deselecting in Blind does not switch Highlight off (PRG-26 would change the stage).
    e.clear_selection();
    assert!(e.highlight());
    e.tick(0.1);
    assert_eq!((dmx(&e, 1), dmx(&e, 5)), (255, 255));
}

#[test]
fn prg_106_highlight_cannot_start_in_blind() {
    let mut e = rig();
    select(&mut e, &[101]);
    e.enter_blind();
    assert!(matches!(
        e.set_highlight(true),
        Err(CoreError::BlindHighlight)
    ));
    assert!(!e.highlight());
    // Without Highlight the steps work in Blind.
    select(&mut e, &[102]);
    e.step_fixture(true).unwrap();
    e.step_fixture(false).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, 1), 0);
}

#[test]
fn prg_106_steps_with_highlight_work_in_live() {
    let mut e = rig();
    select(&mut e, &[101, 102]);
    e.set_highlight(true).unwrap();
    e.step_fixture(true).unwrap();
    e.step_fixture(false).unwrap();
}

// ---- PRG-107: Leaving Blind --------------------------------------------------------------

#[test]
fn prg_107_a_changed_level_fades_from_the_frozen_value() {
    let mut e = rig();
    e.set_programmer_value(101, "Dimmer", 0.2).unwrap();
    e.tick(0.0);
    e.enter_blind();
    e.set_programmer_value(101, "Dimmer", 1.0).unwrap();
    e.leave_blind(2.0, false).unwrap();
    assert!(!e.blind());
    e.tick(1.0); // the fade starts with the first tick
    assert_near(&e, 1, 51);
    e.tick(2.0); // 0.2 → 1.0, halfway: 0.6
    assert_near(&e, 1, 153);
    e.tick(3.0);
    assert_eq!(dmx(&e, 1), 255);
    assert_eq!(level(&e, 101, "Dimmer"), Some(1.0));
}

#[test]
fn prg_107_a_colour_fades_in_linear_rgb() {
    let mut e = rig();
    e.set_programmer_value(201, "Dimmer", 1.0).unwrap();
    e.tick(0.0); // Colour at its default, open white
    e.enter_blind();
    e.set_programmer_value(201, "Colour", red()).unwrap();
    e.tick(0.5);
    assert_eq!((dmx(&e, 18), dmx(&e, 19), dmx(&e, 20)), (255, 255, 255));
    e.leave_blind(2.0, false).unwrap();
    e.tick(1.0);
    e.tick(2.0);
    assert_eq!(dmx(&e, 18), 255);
    assert_near(&e, 19, 128);
    assert_near(&e, 20, 128);
    e.tick(3.0);
    assert_eq!((dmx(&e, 18), dmx(&e, 19), dmx(&e, 20)), (255, 0, 0));
}

#[test]
fn prg_107_a_value_only_the_frozen_programmer_held_fades_out() {
    let mut e = rig();
    playback(&mut e, 1, &[(103, "Dimmer", 0.4)], true);
    e.set_programmer_value(102, "Dimmer", 1.0).unwrap();
    e.set_programmer_value(103, "Dimmer", 1.0).unwrap();
    e.tick(0.0);
    e.enter_blind();
    e.clear(); // nothing selected: empties the programmer
    assert!(e.programmer().is_empty());
    e.leave_blind(2.0, false).unwrap();
    e.tick(1.0);
    e.tick(2.0);
    assert_near(&e, 5, 128);
    assert_near(&e, 9, 128);
    e.tick(3.0);
    assert_eq!(dmx(&e, 5), 0);
    assert_eq!(dmx(&e, 9), 102, "HTP: the playback's 0.4 stays");
}

#[test]
fn prg_107_fade_0_is_on_stage_at_the_next_tick() {
    let mut e = rig();
    e.set_programmer_value(101, "Dimmer", 0.2).unwrap();
    e.tick(0.0);
    e.enter_blind();
    e.set_programmer_value(101, "Dimmer", 1.0).unwrap();
    e.set_programmer_value(102, "Dimmer", 1.0).unwrap();
    e.leave_blind(0.0, false).unwrap();
    e.tick(0.1);
    assert_eq!((dmx(&e, 1), dmx(&e, 5)), (255, 255));
}

#[test]
fn prg_107_clear_gives_back_the_playbacks_state() {
    let mut e = rig();
    playback(&mut e, 1, &[(103, "Dimmer", 0.4)], true);
    e.set_programmer_value(101, "Dimmer", 1.0).unwrap();
    e.set_programmer_value(103, "Dimmer", 1.0).unwrap();
    e.tick(0.0);
    e.enter_blind();
    e.set_programmer_value(102, "Dimmer", 1.0).unwrap();
    e.leave_blind(0.0, true).unwrap();
    assert!(e.programmer().is_empty());
    e.tick(0.1);
    assert_eq!((dmx(&e, 1), dmx(&e, 5), dmx(&e, 9)), (0, 0, 102));
}

#[test]
fn prg_107_a_bad_fade_is_refused_and_leaving_live_is_a_no_op() {
    let mut e = rig();
    e.leave_blind(0.0, false).unwrap();
    e.enter_blind();
    assert!(e.leave_blind(-1.0, false).is_err());
    assert!(e.leave_blind(f32::NAN, false).is_err());
    assert!(e.blind());
}

// ---- PRG-108: A stored change that reaches the stage asks in Blind -----------------------

#[test]
fn prg_108_a_merge_into_the_live_cue_asks_in_blind() {
    let mut e = rig();
    playback(&mut e, 1, &[(101, "Dimmer", 0.5)], true);
    e.tick(0.0);
    e.enter_blind();
    select(&mut e, &[101]);
    e.set_programmer_value(101, "Dimmer", 1.0).unwrap();
    match e.guarded(false, |e| merge_into(e, 1)) {
        Err(CoreError::OnStage(sentence)) => {
            assert!(sentence.contains("live on playback 1"), "{sentence}")
        }
        other => panic!("expected OnStage, got {other:?}"),
    }
    assert_eq!(cue_level(&e, 1, 101), Some(0.5), "nothing changed");
    e.guarded(true, |e| merge_into(e, 1)).unwrap();
    assert_eq!(cue_level(&e, 1, 101), Some(1.0));
    e.tick(0.1);
    assert_eq!(dmx(&e, 1), 255, "the stored change shows");
}

#[test]
fn prg_108_a_cue_not_on_stage_needs_no_confirm() {
    let mut e = rig();
    playback(&mut e, 2, &[(101, "Dimmer", 0.5)], false);
    e.tick(0.0);
    e.enter_blind();
    select(&mut e, &[101]);
    e.set_programmer_value(101, "Dimmer", 1.0).unwrap();
    e.guarded(false, |e| merge_into(e, 2)).unwrap();
    assert_eq!(cue_level(&e, 2, 101), Some(1.0));
}

#[test]
fn prg_108_live_needs_no_confirm() {
    let mut e = rig();
    playback(&mut e, 1, &[(101, "Dimmer", 0.5)], true);
    e.tick(0.0);
    select(&mut e, &[101]);
    e.set_programmer_value(101, "Dimmer", 1.0).unwrap();
    e.guarded(false, |e| merge_into(e, 1)).unwrap();
    assert_eq!(cue_level(&e, 1, 101), Some(1.0));
}

#[test]
fn prg_108_deleting_a_palette_on_stage_asks_in_blind() {
    let mut e = rig();
    select(&mut e, &[101]);
    e.apply_palette(FULL, timed(0.0)).unwrap();
    e.tick(0.0);
    e.enter_blind();
    e.clear();
    e.clear();
    match e.guarded(false, |e| e.delete_palettes(&[FULL])) {
        Err(CoreError::OnStage(sentence)) => assert!(sentence.contains("81"), "{sentence}"),
        other => panic!("expected OnStage, got {other:?}"),
    }
    assert!(e.palette(FULL).is_some());
    e.guarded(true, |e| e.delete_palettes(&[FULL])).unwrap();
    assert!(e.palette(FULL).is_none());
}

// ---- PRG-109: Blind is live state --------------------------------------------------------

#[test]
fn prg_109_blind_is_never_saved_and_a_loaded_show_starts_live() {
    let mut e = rig();
    e.enter_blind();
    let show = e.to_show(Meta {
        name: "BLD-01".into(),
        created: "2026-11-17T09:00:00Z".into(),
        modified: "2026-11-17T09:00:00Z".into(),
        app_version: "0.1.0".into(),
    });
    let json = serde_json::to_string(&show).unwrap();
    assert!(!json.contains("blind"), "Blind is not in the show file");
    let (opened, _) = Engine::from_show(&show);
    assert!(!opened.blind());
}
