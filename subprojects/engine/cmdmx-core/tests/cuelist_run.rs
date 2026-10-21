//! Cue lists running (VS3-03): `specs/modules/cuelists.md` CUE-03, 05..08 (with a running list),
//! CUE-10..27, 49..53, 55. What reaches the frame is mostly in the scenarios
//! `tests/scenarios/CUE-*.json`; these tests cover the live state the IPC will show (current,
//! next, paused, progress), the connected list, handle buttons, refusals, tracking edits and the
//! cases a scenario cannot express. Chases, Tap and speed masters running are VS3-04.
//!
//! API these tests fix (on top of `tests/cuelists.rs`):
//! - `Engine::cuelist(id: Option<PlaybackId>, action: ListAction, now) -> Result<(), CoreError>`
//!   with `ListAction::{Go, Back, Stop, Goto(CueNumber), Next(CueNumber), Release}`
//!   (`cuelist.go/back/stop/goto/next` and `playback.release`, CUE-10..12, 18, 19, 27). `None`
//!   acts on the connected list; with none it is refused ("No cue list connected"). On a playback
//!   that is not a cue list (a memory, a master) it is refused. Go at the end is refused with
//!   "Q<n> … is the last cue".
//! - `Engine::connect_list(id)` (CUE-26; a memory or a master is refused) and
//!   `Engine::connected() -> Option<PlaybackId>`: the connected list, or the first list (cue list
//!   or chase) in handle order (page, then slot) when none is connected or it is no list any more.
//! - `Engine::list_live(id) -> Option<ListLive>` for a cue list (`None` for anything else):
//!   `ListLive { current: Option<CueNumber>, next: Option<CueNumber>, paused: bool, progress:
//!   f32 }` (`Debug, Clone, Copy, PartialEq`; the CUE-40 `cue` object). `progress` is the
//!   current transition's (0 while off).
//! - `Engine::press_button(id, HandleButton::{Upper, Lower}, on, now)` (CUE-55; `"upper"` /
//!   `"lower"` in JSON) does what `handle_buttons` assigns to the playback's kind (CUE-25).
//!   `set_flash` on a master is refused.
//! - `PlaybackRecord::update_mode` takes effect (CUE-52); Replace/Merge/Update without a `cue`
//!   act on a running list's current cue (CUE-06); Insert goes after the current cue (CUE-05).
//! - Scenario format (`src/scenario.rs`): playbacks gain `kind`, `list`, `chase`, `master`; a cue
//!   without `number` counts 1, 2, 3 … in file order; actions `go`, `back`, `stop`
//!   `{playback?}`, `goto` / `next` `{playback?, cue}`, `connect {playback}`, `button {playback,
//!   button, on}`; `settings` gains `handle_buttons` and `update_mode`.
//! - Time model: an action takes effect in the next tick, every timer counts from that tick's
//!   timestamp (PBK, Timing). A transition fades each attribute from its value in that tick.
//!   Stop and Go take effect in the tick they reach: the list's time stands still from the Stop
//!   tick to the Go tick, so the output of the Stop tick is what freezes. A link is due on the
//!   first tick at or after its time (allow 1e-6 s for float error).
//! - Owner, 2026-10-21 (spec amended): a transition's duration counts only the attributes it
//!   fades (a cue that changes nothing is complete at once); Update on a tracking list takes the
//!   cue's state, tracked values included; KillAtZero kills when the level falls to 0, so Go at
//!   fader 0 runs the list with its HTP out; a Solo cue before *n* is not there for *n*.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

mod common;

use std::collections::BTreeMap;

use cmdmx_core::engine::{Engine, PlaybackSettingsChange};
use cmdmx_core::palette::RecordMode;
use cmdmx_core::playback::{
    AddCue, Button, ButtonKind, Cue, CueChange, CueNumber, FaderMode, HandleButton, HandleButtons,
    Kind, Link, ListAction, ListLive, ListOptions, LoopAction, MasterKind, Playback,
    PlaybackChange, PlaybackRecord, Track, UpdateMode, Values,
};
use cmdmx_core::show::{Meta, Show};
use cmdmx_core::value::{Stored, Value};
use common::dmx;

const TICK: f64 = 0.025;

// ---- helpers -------------------------------------------------------------------------------

fn q(n: f64) -> CueNumber {
    CueNumber::new(n).unwrap()
}

fn levels(entries: &[(u32, &str, f32)]) -> Values {
    let mut v: Values = BTreeMap::new();
    for (id, name, level) in entries {
        v.entry(*id)
            .or_default()
            .insert((*name).to_string(), (*level).into());
    }
    v
}

fn cue(n: f64, entries: &[(u32, &str, f32)]) -> Cue {
    Cue {
        number: q(n),
        ..Cue::new(levels(entries))
    }
}

/// Playback `id` on handle `page.slot` with these cues.
fn add(e: &mut Engine, id: u32, page: u16, slot: u16, cues: Vec<Cue>) {
    e.add_playback(Playback {
        cues,
        ..Playback::new(id, page, slot, Values::new())
    });
}

/// The church rig with list 1 on 1.1: Q1 par 201, Q2 par 202, Q3 par 203, each at full, zero
/// times. Par 201 is DMX 21, 202 is 22, 203 is 23.
fn three() -> Engine {
    let mut e = common::church();
    add(
        &mut e,
        1,
        1,
        1,
        vec![
            cue(1.0, &[(201, "Dimmer", 1.0)]),
            cue(2.0, &[(202, "Dimmer", 1.0)]),
            cue(3.0, &[(203, "Dimmer", 1.0)]),
        ],
    );
    e
}

fn tick(e: &mut Engine, t: f64) {
    e.tick(t);
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

fn act(e: &mut Engine, action: ListAction, now: f64) {
    e.cuelist(Some(1), action, now).unwrap();
    e.tick(now);
}

fn live(e: &Engine) -> ListLive {
    e.list_live(1).unwrap()
}

fn current(e: &Engine) -> Option<f64> {
    live(e).current.map(CueNumber::as_f64)
}

fn next(e: &Engine) -> Option<f64> {
    live(e).next.map(CueNumber::as_f64)
}

fn frame(e: &Engine) -> Vec<u8> {
    e.frame(1).unwrap().slots().to_vec()
}

fn options(e: &mut Engine, id: u32, list: ListOptions) {
    e.change_playback(
        id,
        &PlaybackChange {
            list: Some(list),
            ..PlaybackChange::default()
        },
    )
    .unwrap();
}

fn change(e: &mut Engine, n: f64, change: CueChange) {
    e.change_cue(1, q(n), &change).unwrap();
}

fn settings(e: &mut Engine, change: PlaybackSettingsChange) {
    e.set_playback_settings(&change).unwrap();
}

/// Selection and programmer hold exactly `fixture` `attribute` at `level`.
fn program(e: &mut Engine, fixture: u32, attribute: &str, level: f32) {
    e.clear_selection();
    e.clear();
    common::select(e, &[fixture]);
    e.set_programmer_value(fixture, attribute, level).unwrap();
}

/// Add `fixture` `attribute` at `level` to what the programmer holds.
fn also(e: &mut Engine, fixture: u32, attribute: &str, level: f32) {
    common::select(e, &[fixture]);
    e.set_programmer_value(fixture, attribute, level).unwrap();
}

fn record(
    e: &mut Engine,
    cue: Option<f64>,
    mode: RecordMode,
    update: Option<UpdateMode>,
) -> Result<u32, cmdmx_core::CoreError> {
    e.record_playback(&PlaybackRecord {
        cue: cue.map(q),
        mode: Some(mode),
        update_mode: update,
        ..PlaybackRecord::at(1, 1)
    })
}

/// Record, then empty the programmer so it does not reach the frame.
fn record_clean(e: &mut Engine, cue: Option<f64>, mode: RecordMode, update: Option<UpdateMode>) {
    record(e, cue, mode, update).unwrap();
    e.clear_selection();
    e.clear();
}

/// The hard value of `fixture` `attribute` in cue `n` of list 1.
fn hard(e: &Engine, n: f64, fixture: u32, attribute: &str) -> Option<f32> {
    let cue = e
        .playback(1)
        .unwrap()
        .cues
        .iter()
        .find(|c| c.number == q(n))
        .unwrap();
    cue.values
        .get(&fixture)
        .and_then(|a| a.get(attribute))
        .map(|s| match s {
            Stored::Value(Value::Level(v)) => *v,
            other => panic!("not a level: {other:?}"),
        })
}

fn meta() -> Meta {
    Meta {
        name: "t".into(),
        created: String::new(),
        modified: String::new(),
        app_version: String::new(),
    }
}

// ---- CUE-10: Go ------------------------------------------------------------------------------

#[test]
fn cue_10_go_moves_the_current_and_the_next_cue() {
    let mut e = three();
    assert_eq!(live(&e).current, None);
    assert_eq!(next(&e), Some(1.0), "off: the first cue is next");
    assert!(!live(&e).paused);
    act(&mut e, ListAction::Go, 0.0);
    assert!(e.playback_state(1).unwrap().is_active());
    assert_eq!((current(&e), next(&e)), (Some(1.0), Some(2.0)));
    act(&mut e, ListAction::Go, 1.0);
    assert_eq!((current(&e), next(&e)), (Some(2.0), Some(3.0)));
    act(&mut e, ListAction::Go, 2.0);
    assert_eq!((current(&e), next(&e)), (Some(3.0), None));
}

#[test]
fn cue_10_go_at_the_end_is_refused_and_changes_nothing() {
    let mut e = three();
    e.set_fader(1, 1.0, 0.0).unwrap();
    for t in [0.0, 1.0, 2.0] {
        act(&mut e, ListAction::Go, t);
    }
    let (before, out) = (live(&e), frame(&e));
    let err = e.cuelist(Some(1), ListAction::Go, 3.0).unwrap_err();
    assert!(err.to_string().contains("is the last cue"), "{err}");
    run(&mut e, 3.0, 3.5);
    assert_eq!(live(&e), before);
    assert_eq!(frame(&e), out);
}

#[test]
fn cue_10_list_actions_on_a_memory_or_a_master_are_refused() {
    let mut e = three();
    add(&mut e, 2, 1, 2, vec![cue(1.0, &[(204, "Dimmer", 1.0)])]);
    let master = e.record_master(1, 3, MasterKind::Bpm, 1).unwrap();
    for id in [2, master] {
        assert!(e.cuelist(Some(id), ListAction::Go, 0.0).is_err());
        assert!(e.list_live(id).is_none());
    }
    tick(&mut e, 0.0);
    assert_eq!(dmx(&e, 24), 0);
}

// ---- CUE-11: Back ----------------------------------------------------------------------------

#[test]
fn cue_11_back_starts_the_previous_cue_and_makes_the_one_after_it_next() {
    let mut e = three();
    act(&mut e, ListAction::Go, 0.0);
    act(&mut e, ListAction::Go, 1.0);
    act(&mut e, ListAction::Go, 2.0);
    act(&mut e, ListAction::Back, 3.0);
    assert_eq!((current(&e), next(&e)), (Some(2.0), Some(3.0)));
}

#[test]
fn cue_11_back_on_the_first_cue_or_off_is_refused() {
    let mut e = three();
    assert!(e.cuelist(Some(1), ListAction::Back, 0.0).is_err(), "off");
    act(&mut e, ListAction::Go, 0.0);
    let before = live(&e);
    assert!(e.cuelist(Some(1), ListAction::Back, 1.0).is_err(), "first");
    tick(&mut e, 1.0);
    assert_eq!(live(&e), before, "never wraps to the last cue");
}

// ---- CUE-12: Stop ----------------------------------------------------------------------------

#[test]
fn cue_12_stop_pauses_and_go_resumes_without_starting_the_next_cue() {
    let mut e = three();
    change(
        &mut e,
        1.0,
        CueChange {
            fade_in: Some(4.0),
            ..CueChange::default()
        },
    );
    act(&mut e, ListAction::Go, 0.0);
    run(&mut e, 0.0, 1.0);
    act(&mut e, ListAction::Stop, 1.0);
    assert!(live(&e).paused);
    let progress = live(&e).progress;
    run(&mut e, 1.0, 3.0);
    assert_eq!(live(&e).progress, progress, "frozen");
    act(&mut e, ListAction::Go, 3.0);
    assert!(!live(&e).paused);
    assert_eq!(current(&e), Some(1.0));
    run(&mut e, 3.0, 4.0);
    assert!(
        (live(&e).progress - 0.5).abs() < 1e-3,
        "{}",
        live(&e).progress
    );
}

#[test]
fn cue_12_stop_on_a_paused_list_goes_back() {
    let mut e = three();
    change(
        &mut e,
        2.0,
        CueChange {
            fade_in: Some(4.0),
            ..CueChange::default()
        },
    );
    act(&mut e, ListAction::Go, 0.0);
    act(&mut e, ListAction::Go, 1.0);
    act(&mut e, ListAction::Stop, 2.0);
    act(&mut e, ListAction::Stop, 3.0);
    assert_eq!((current(&e), next(&e)), (Some(1.0), Some(2.0)));
    assert!(!live(&e).paused);
}

#[test]
fn cue_12_stop_with_nothing_running_does_nothing() {
    let mut e = three();
    act(&mut e, ListAction::Go, 0.0);
    run(&mut e, 0.0, 1.0);
    let before = live(&e);
    act(&mut e, ListAction::Stop, 1.0);
    assert_eq!(live(&e), before);
    act(&mut e, ListAction::Stop, 1.5);
    assert_eq!(live(&e), before, "not paused, so no Back either");
}

// ---- CUE-13: the transition ------------------------------------------------------------------

#[test]
fn cue_13_progress_runs_over_the_transition() {
    let mut e = three();
    change(
        &mut e,
        2.0,
        CueChange {
            delay: Some(1.0),
            fade_in: Some(2.0),
            ..CueChange::default()
        },
    );
    act(&mut e, ListAction::Go, 0.0);
    assert_eq!(live(&e).progress, 1.0, "zero times: complete at once");
    act(&mut e, ListAction::Go, 1.0);
    run(&mut e, 1.0, 2.5);
    assert!(
        (live(&e).progress - 0.5).abs() < 1e-3,
        "{}",
        live(&e).progress
    );
    run(&mut e, 2.5, 4.0);
    assert_eq!(live(&e).progress, 1.0);
}

#[test]
fn cue_13_a_cue_that_changes_nothing_is_complete_at_once() {
    // Owner, 2026-10-21: the duration counts only what the transition fades.
    let mut e = common::church();
    let mut q2 = cue(2.0, &[(201, "Dimmer", 1.0)]);
    q2.fade_in = 5.0;
    add(&mut e, 1, 1, 1, vec![cue(1.0, &[(201, "Dimmer", 1.0)]), q2]);
    act(&mut e, ListAction::Go, 0.0);
    act(&mut e, ListAction::Go, 1.0);
    assert_eq!(live(&e).progress, 1.0);
}

// ---- CUE-15: the fader -----------------------------------------------------------------------

#[test]
fn cue_15_fader_modes_and_the_kill_point_do_not_apply_to_a_cue_list() {
    let mut e = three();
    e.change_playback(
        1,
        &PlaybackChange {
            fader_mode: Some(FaderMode::Mode1),
            fade_out: Some(4.0),
            ..PlaybackChange::default()
        },
    )
    .unwrap();
    e.set_fader(1, 1.0, 0.0).unwrap();
    run(&mut e, 0.0, 0.5);
    assert_eq!(dmx(&e, 21), 255);
    e.set_fader(1, 0.0, 1.0).unwrap();
    tick(&mut e, 1.0);
    assert_eq!(dmx(&e, 21), 0, "m follows the fader at once");
    run(&mut e, 1.0, 6.0);
    assert!(
        e.playback_state(1).unwrap().is_active(),
        "KillWithOff: still on its cue"
    );
    assert_eq!(current(&e), Some(1.0));
}

// ---- CUE-16: fire first cue ------------------------------------------------------------------

#[test]
fn cue_16_without_fire_first_cue_the_fader_makes_the_list_active_without_a_cue() {
    let mut e = three();
    options(
        &mut e,
        1,
        ListOptions {
            fire_first_cue: false,
            ..ListOptions::default()
        },
    );
    e.set_fader(1, 1.0, 0.0).unwrap();
    tick(&mut e, 0.0);
    assert!(e.playback_state(1).unwrap().is_active());
    assert_eq!((current(&e), next(&e)), (None, Some(1.0)));
    act(&mut e, ListAction::Go, 1.0);
    assert_eq!(current(&e), Some(1.0));
}

// ---- CUE-17: kill and reset ------------------------------------------------------------------

#[test]
fn cue_17_release_resets_the_list_to_its_first_cue() {
    let mut e = three();
    e.set_fader(1, 1.0, 0.0).unwrap();
    tick(&mut e, 0.0);
    act(&mut e, ListAction::Go, 1.0);
    act(&mut e, ListAction::Release, 2.0);
    assert!(!e.playback_state(1).unwrap().is_active());
    assert_eq!(live(&e).current, None);
    assert_eq!(next(&e), Some(1.0));
    assert!(!live(&e).paused);
}

#[test]
fn cue_17_a_kill_cancels_the_link_timers() {
    let mut e = three();
    change(
        &mut e,
        2.0,
        CueChange {
            link: Some(Link::With { offset: 1.0 }),
            ..CueChange::default()
        },
    );
    act(&mut e, ListAction::Go, 0.0);
    act(&mut e, ListAction::Release, 0.5);
    run(&mut e, 0.5, 3.0);
    assert_eq!(live(&e).current, None);
    assert_eq!(dmx(&e, 22), 0);
}

// ---- CUE-18, 19: Go to cue, Set next ---------------------------------------------------------

#[test]
fn cue_18_goto_fires_a_list_that_is_off_at_that_cue() {
    let mut e = three();
    act(&mut e, ListAction::Goto(q(2.0)), 0.0);
    assert!(e.playback_state(1).unwrap().is_active());
    assert_eq!((current(&e), next(&e)), (Some(2.0), Some(3.0)));
    act(&mut e, ListAction::Goto(q(1.0)), 1.0);
    assert_eq!((current(&e), next(&e)), (Some(1.0), Some(2.0)));
}

#[test]
fn cue_18_19_an_unknown_cue_is_refused() {
    let mut e = three();
    for action in [ListAction::Goto(q(2.5)), ListAction::Next(q(7.0))] {
        assert!(e.cuelist(Some(1), action, 0.0).is_err());
    }
    tick(&mut e, 0.0);
    assert_eq!(live(&e).current, None);
    assert_eq!(next(&e), Some(1.0));
}

#[test]
fn cue_19_set_next_works_on_and_off_without_changing_the_stage() {
    let mut e = three();
    act(&mut e, ListAction::Next(q(3.0)), 0.0);
    assert_eq!((live(&e).current, next(&e)), (None, Some(3.0)));
    assert!(!e.playback_state(1).is_some_and(|s| s.is_active()));
    act(&mut e, ListAction::Go, 1.0);
    assert_eq!(current(&e), Some(3.0), "Go fires the list at the next cue");
    e.set_fader(1, 1.0, 2.0).unwrap();
    tick(&mut e, 2.0);
    let out = frame(&e);
    act(&mut e, ListAction::Next(q(1.0)), 3.0);
    assert_eq!(frame(&e), out);
    assert_eq!((current(&e), next(&e)), (Some(3.0), Some(1.0)));
}

// ---- CUE-20: links ---------------------------------------------------------------------------

#[test]
fn cue_20_a_looping_chain_of_zero_links_moves_one_cue_per_tick() {
    let mut e = three();
    options(
        &mut e,
        1,
        ListOptions {
            looping: LoopAction::Loop,
            ..ListOptions::default()
        },
    );
    for n in [1.0, 2.0, 3.0] {
        change(
            &mut e,
            n,
            CueChange {
                link: Some(Link::With { offset: 0.0 }),
                ..CueChange::default()
            },
        );
    }
    act(&mut e, ListAction::Go, 0.0);
    let order = [1.0, 2.0, 3.0];
    let mut last = current(&e).unwrap();
    for i in 1..=9 {
        tick(&mut e, f64::from(i) * TICK);
        let now = current(&e).unwrap();
        let at = order.iter().position(|&n| n == last).unwrap();
        assert_eq!(now, order[(at + 1) % 3], "tick {i}: one cue per tick");
        last = now;
    }
}

#[test]
fn cue_20_go_while_a_link_is_pending_starts_the_pending_cue_at_once() {
    let mut e = three();
    change(
        &mut e,
        2.0,
        CueChange {
            link: Some(Link::With { offset: 5.0 }),
            ..CueChange::default()
        },
    );
    act(&mut e, ListAction::Go, 0.0);
    act(&mut e, ListAction::Go, 1.0);
    assert_eq!(current(&e), Some(2.0));
    run(&mut e, 1.0, 7.0);
    assert_eq!(
        current(&e),
        Some(2.0),
        "the link did not fire a second time"
    );
}

#[test]
fn cue_20_stop_pauses_the_link_timers() {
    let mut e = three();
    change(
        &mut e,
        2.0,
        CueChange {
            link: Some(Link::With { offset: 1.0 }),
            ..CueChange::default()
        },
    );
    act(&mut e, ListAction::Go, 0.0);
    act(&mut e, ListAction::Stop, 0.5);
    run(&mut e, 0.5, 3.0);
    assert_eq!(current(&e), Some(1.0));
    act(&mut e, ListAction::Go, 3.0);
    run(&mut e, 3.0, 3.45);
    assert_eq!(current(&e), Some(1.0), "0.5 s left");
    run(&mut e, 3.475, 3.5);
    assert_eq!(current(&e), Some(2.0));
}

// ---- CUE-21: end of list ---------------------------------------------------------------------

#[test]
fn cue_21_loop_makes_the_first_cue_next_after_the_last() {
    let mut e = three();
    for t in [0.0, 1.0, 2.0] {
        act(&mut e, ListAction::Go, t);
    }
    assert_eq!(next(&e), None, "StopOnFinal");
    options(
        &mut e,
        1,
        ListOptions {
            looping: LoopAction::Loop,
            ..ListOptions::default()
        },
    );
    act(&mut e, ListAction::Goto(q(3.0)), 3.0);
    assert_eq!(next(&e), Some(1.0));
    act(&mut e, ListAction::Go, 4.0);
    assert_eq!((current(&e), next(&e)), (Some(1.0), Some(2.0)));
}

// ---- CUE-24: live state is not saved ---------------------------------------------------------

#[test]
fn cue_24_a_running_list_loads_off_with_its_first_cue_next() {
    let mut e = three();
    e.set_fader(1, 1.0, 0.0).unwrap();
    tick(&mut e, 0.0);
    act(&mut e, ListAction::Go, 1.0);
    act(&mut e, ListAction::Stop, 1.5);
    let text = e.to_show(meta()).to_json();
    for key in ["\"current\"", "\"paused\"", "\"progress\"", "\"connected\""] {
        assert!(!text.contains(key), "{key} saved");
    }
    let (show, _) = Show::from_json(&text).unwrap();
    let (loaded, _) = Engine::from_show(&show);
    let l = loaded.list_live(1).unwrap();
    assert_eq!((l.current, l.next, l.paused), (None, Some(q(1.0)), false));
    assert!(!loaded.playback_state(1).is_some_and(|s| s.is_active()));
}

// ---- CUE-25, 55: handle buttons --------------------------------------------------------------

#[test]
fn cue_25_best_practice_memory_buttons_are_swop_and_flash() {
    let mut e = three();
    add(&mut e, 2, 1, 2, vec![cue(1.0, &[(204, "Dimmer", 1.0)])]);
    e.press_button(2, HandleButton::Upper, true, 0.0).unwrap();
    assert!(e.playback_state(2).unwrap().swop);
    e.press_button(2, HandleButton::Upper, false, 0.1).unwrap();
    e.press_button(2, HandleButton::Lower, true, 0.2).unwrap();
    let s = e.playback_state(2).unwrap();
    assert!(s.flash && !s.swop);
}

#[test]
fn cue_25_a_cue_list_upper_button_is_go_on_the_press() {
    let mut e = three();
    e.press_button(1, HandleButton::Upper, true, 0.0).unwrap();
    tick(&mut e, 0.0);
    assert_eq!(current(&e), Some(1.0));
    e.press_button(1, HandleButton::Upper, false, 0.5).unwrap();
    tick(&mut e, 0.5);
    assert_eq!(current(&e), Some(1.0), "the release does nothing");
    let s = e.playback_state(1).unwrap();
    assert!(!s.swop && !s.flash);
    e.press_button(1, HandleButton::Lower, true, 1.0).unwrap();
    assert!(e.playback_state(1).unwrap().flash);
}

#[test]
fn cue_25_a_memory_that_becomes_a_list_takes_the_list_buttons() {
    let mut e = common::church();
    add(&mut e, 1, 1, 1, vec![cue(1.0, &[(201, "Dimmer", 1.0)])]);
    program(&mut e, 202, "Dimmer", 1.0);
    e.record_playback(&PlaybackRecord {
        add: Some(AddCue::Append),
        ..PlaybackRecord::at(1, 1)
    })
    .unwrap();
    e.press_button(1, HandleButton::Upper, true, 0.0).unwrap();
    tick(&mut e, 0.0);
    assert!(!e.playback_state(1).unwrap().swop);
    assert_eq!(current(&e), Some(1.0));
}

#[test]
fn cue_25_titan_puts_swop_and_flash_on_a_cue_list() {
    let mut e = three();
    settings(
        &mut e,
        PlaybackSettingsChange {
            handle_buttons: Some(HandleButtons::Titan),
            ..PlaybackSettingsChange::default()
        },
    );
    e.press_button(1, HandleButton::Upper, true, 0.0).unwrap();
    assert!(e.playback_state(1).unwrap().swop);
}

#[test]
fn cue_25_custom_buttons_per_kind() {
    let mut e = three();
    let pairs = BTreeMap::from([
        (ButtonKind::CueList, [Button::Back, Button::Stop]),
        (ButtonKind::Memory, [Button::Release, Button::None]),
    ]);
    settings(
        &mut e,
        PlaybackSettingsChange {
            handle_buttons: Some(HandleButtons::Custom(pairs)),
            ..PlaybackSettingsChange::default()
        },
    );
    act(&mut e, ListAction::Go, 0.0);
    act(&mut e, ListAction::Go, 1.0);
    e.press_button(1, HandleButton::Upper, true, 2.0).unwrap();
    tick(&mut e, 2.0);
    assert_eq!(current(&e), Some(1.0), "upper is Back");

    add(&mut e, 2, 1, 2, vec![cue(1.0, &[(204, "Dimmer", 1.0)])]);
    e.set_fader(2, 1.0, 3.0).unwrap();
    tick(&mut e, 3.0);
    e.press_button(2, HandleButton::Lower, true, 3.5).unwrap();
    tick(&mut e, 3.5);
    assert!(
        e.playback_state(2).unwrap().is_active(),
        "None does nothing"
    );
    e.press_button(2, HandleButton::Upper, true, 4.0).unwrap();
    tick(&mut e, 4.0);
    assert!(
        !e.playback_state(2).unwrap().is_active(),
        "upper is Release"
    );
}

#[test]
fn cue_55_flash_on_a_master_is_refused() {
    let mut e = three();
    let master = e.record_master(1, 3, MasterKind::Rate, 1).unwrap();
    assert!(e.set_flash(master, true, 0.0).is_err());
    e.press_button(master, HandleButton::Upper, true, 0.0)
        .unwrap();
    e.press_button(master, HandleButton::Lower, true, 0.0)
        .unwrap();
    tick(&mut e, 0.0);
    assert!(!e.playback_state(master).is_some_and(|s| s.is_active()));
}

// ---- CUE-26, 27: the connected list ----------------------------------------------------------

fn two_lists() -> Engine {
    let mut e = three();
    add(
        &mut e,
        2,
        1,
        2,
        vec![
            cue(1.0, &[(221, "Dimmer", 1.0)]),
            cue(2.0, &[(222, "Dimmer", 1.0)]),
        ],
    );
    e
}

#[test]
fn cue_26_without_a_connection_the_first_list_in_handle_order_is_connected() {
    let mut e = common::church();
    add(&mut e, 1, 1, 1, vec![cue(1.0, &[(201, "Dimmer", 1.0)])]);
    let two = vec![
        cue(1.0, &[(202, "Dimmer", 1.0)]),
        cue(2.0, &[(203, "Dimmer", 1.0)]),
    ];
    add(&mut e, 2, 2, 1, two.clone());
    add(&mut e, 3, 1, 4, two.clone());
    add(&mut e, 4, 1, 6, two);
    assert_eq!(e.connected(), Some(3), "page 1 slot 4 before page 2 slot 1");
    e.delete_playback(3).unwrap();
    assert_eq!(e.connected(), Some(4));
    e.delete_cue(4, q(2.0)).unwrap();
    assert_eq!(e.connected(), Some(2), "a memory is no list");
    e.delete_playback(2).unwrap();
    assert_eq!(e.connected(), None);
}

#[test]
fn cue_26_a_chase_is_a_list_too() {
    let mut e = two_lists();
    e.move_playback(1, 2, 1).unwrap();
    add(
        &mut e,
        3,
        1,
        1,
        vec![
            cue(1.0, &[(223, "Dimmer", 1.0)]),
            cue(2.0, &[(224, "Dimmer", 1.0)]),
        ],
    );
    e.change_playback(
        3,
        &PlaybackChange {
            kind: Some(Kind::Chase),
            ..PlaybackChange::default()
        },
    )
    .unwrap();
    assert_eq!(e.connected(), Some(3));
    e.connect_list(2).unwrap();
    assert_eq!(e.connected(), Some(2));
    e.connect_list(3).unwrap();
    assert_eq!(e.connected(), Some(3));
}

#[test]
fn cue_26_auto_connect_follows_the_list_that_fires_or_gets_go() {
    let mut e = two_lists();
    assert_eq!(e.connected(), Some(1));
    e.set_fader(2, 1.0, 0.0).unwrap();
    tick(&mut e, 0.0);
    assert_eq!(e.connected(), Some(2), "fired by its fader");
    act(&mut e, ListAction::Go, 1.0);
    assert_eq!(e.connected(), Some(1), "Go on list 1");
    e.cuelist(Some(2), ListAction::Next(q(2.0)), 2.0).unwrap();
    tick(&mut e, 2.0);
    assert_eq!(e.connected(), Some(2), "Set next on list 2");
    e.press_button(1, HandleButton::Upper, true, 3.0).unwrap();
    tick(&mut e, 3.0);
    assert_eq!(e.connected(), Some(1), "Go from its own handle");
}

#[test]
fn cue_26_without_auto_connect_only_connect_changes_it() {
    let mut e = two_lists();
    settings(
        &mut e,
        PlaybackSettingsChange {
            auto_connect: Some(false),
            ..PlaybackSettingsChange::default()
        },
    );
    e.set_fader(2, 1.0, 0.0).unwrap();
    e.cuelist(Some(2), ListAction::Go, 0.5).unwrap();
    tick(&mut e, 0.5);
    assert_eq!(e.connected(), Some(1));
    e.connect_list(2).unwrap();
    assert_eq!(e.connected(), Some(2));
}

#[test]
fn cue_26_connect_never_changes_the_stage_and_refuses_what_is_no_list() {
    let mut e = two_lists();
    add(&mut e, 3, 1, 3, vec![cue(1.0, &[(204, "Dimmer", 1.0)])]);
    let master = e.record_master(1, 4, MasterKind::Bpm, 1).unwrap();
    e.set_fader(1, 1.0, 0.0).unwrap();
    run(&mut e, 0.0, 0.5);
    let out = frame(&e);
    e.connect_list(2).unwrap();
    run(&mut e, 0.5, 1.0);
    assert_eq!(frame(&e), out);
    assert!(e.connect_list(3).is_err(), "a memory");
    assert!(e.connect_list(master).is_err(), "a master");
    assert!(e.connect_list(99).is_err());
    assert_eq!(e.connected(), Some(2));
}

#[test]
fn cue_27_commands_without_an_id_act_on_the_connected_list() {
    let mut e = two_lists();
    e.connect_list(2).unwrap();
    e.cuelist(None, ListAction::Go, 0.0).unwrap();
    tick(&mut e, 0.0);
    assert_eq!(e.list_live(2).unwrap().current, Some(q(1.0)));
    assert_eq!(live(&e).current, None);
    e.cuelist(None, ListAction::Release, 1.0).unwrap();
    tick(&mut e, 1.0);
    assert!(!e.playback_state(2).unwrap().is_active());
}

#[test]
fn cue_27_without_a_connected_list_they_are_refused() {
    let mut e = common::church();
    add(&mut e, 1, 1, 1, vec![cue(1.0, &[(201, "Dimmer", 1.0)])]);
    for action in [
        ListAction::Go,
        ListAction::Back,
        ListAction::Stop,
        ListAction::Goto(q(1.0)),
        ListAction::Next(q(1.0)),
        ListAction::Release,
    ] {
        let err = e.cuelist(None, action, 0.0).unwrap_err();
        assert_eq!(err.to_string(), "No cue list connected");
    }
}

// ---- CUE-49: refusals and determinism --------------------------------------------------------

/// A list with fades, a link and tracking, driven through Go, Stop, Back and the fader.
fn busy(e: &mut Engine) -> Vec<Vec<u8>> {
    let mut frames = Vec::new();
    for i in 0..=320u32 {
        let t = f64::from(i) * TICK;
        match i {
            0 => e.set_fader(1, 1.0, t).unwrap(),
            40 | 120 | 200 => e.cuelist(Some(1), ListAction::Go, t).unwrap(),
            80 => e.cuelist(Some(1), ListAction::Stop, t).unwrap(),
            100 => e.cuelist(Some(1), ListAction::Go, t).unwrap(),
            160 => e.cuelist(Some(1), ListAction::Back, t).unwrap(),
            240 => e.set_fader(1, 0.4, t).unwrap(),
            _ => {}
        }
        e.tick(t);
        frames.push(frame(e));
    }
    frames
}

fn busy_list() -> Engine {
    let mut e = three();
    for (n, fade) in [(1.0, 1.0), (2.0, 2.5), (3.0, 0.5)] {
        change(
            &mut e,
            n,
            CueChange {
                fade_in: Some(fade),
                fade_out: Some(fade),
                ..CueChange::default()
            },
        );
    }
    change(
        &mut e,
        3.0,
        CueChange {
            link: Some(Link::After { offset: 0.5 }),
            ..CueChange::default()
        },
    );
    e
}

#[test]
fn cue_49_same_actions_give_the_same_frames() {
    let (mut a, mut b) = (busy_list(), busy_list());
    assert_eq!(busy(&mut a), busy(&mut b));
}

#[test]
fn cue_49_refused_actions_change_nothing() {
    let mut e = three();
    act(&mut e, ListAction::Go, 0.0);
    let (before, out) = (live(&e), frame(&e));
    for action in [
        ListAction::Back,
        ListAction::Goto(q(9.0)),
        ListAction::Next(q(0.5)),
    ] {
        assert!(e.cuelist(Some(1), action, 1.0).is_err());
    }
    assert!(e.cuelist(Some(99), ListAction::Go, 1.0).is_err());
    tick(&mut e, 1.0);
    assert_eq!((live(&e), frame(&e)), (before, out));
}

// ---- CUE-03, 05..08 with a running list ------------------------------------------------------

#[test]
fn cue_03_append_on_the_last_cue_makes_the_new_cue_next() {
    let mut e = three();
    e.set_fader(1, 1.0, 0.0).unwrap();
    tick(&mut e, 0.0);
    act(&mut e, ListAction::Goto(q(3.0)), 1.0);
    assert_eq!(next(&e), None);
    let out = frame(&e);
    program(&mut e, 204, "Dimmer", 1.0);
    e.record_playback(&PlaybackRecord {
        add: Some(AddCue::Append),
        ..PlaybackRecord::at(1, 1)
    })
    .unwrap();
    e.clear_selection();
    e.clear();
    tick(&mut e, 2.0);
    assert_eq!(next(&e), Some(4.0));
    assert_eq!(frame(&e), out, "the output does not change");
}

#[test]
fn cue_03_append_elsewhere_keeps_the_next_cue() {
    let mut e = three();
    act(&mut e, ListAction::Go, 0.0);
    program(&mut e, 204, "Dimmer", 1.0);
    e.record_playback(&PlaybackRecord {
        add: Some(AddCue::Append),
        ..PlaybackRecord::at(1, 1)
    })
    .unwrap();
    assert_eq!(next(&e), Some(2.0));
}

#[test]
fn cue_05_insert_goes_after_the_current_cue_and_becomes_next() {
    let mut e = three();
    act(&mut e, ListAction::Goto(q(2.0)), 0.0);
    program(&mut e, 204, "Dimmer", 1.0);
    let insert = PlaybackRecord {
        add: Some(AddCue::Insert),
        ..PlaybackRecord::at(1, 1)
    };
    e.record_playback(&insert).unwrap();
    let numbers: Vec<f64> = (e.playback(1).unwrap().cues.iter())
        .map(|c| c.number.as_f64())
        .collect();
    assert_eq!(numbers, [1.0, 2.0, 2.5, 3.0]);
    assert_eq!((current(&e), next(&e)), (Some(2.0), Some(2.5)));
    act(&mut e, ListAction::Goto(q(3.0)), 1.0);
    e.record_playback(&insert).unwrap();
    assert_eq!(next(&e), Some(4.0), "⌊current⌋ + 1 after the last cue");
}

#[test]
fn cue_06_without_a_number_modes_act_on_the_current_cue() {
    let mut e = three();
    act(&mut e, ListAction::Goto(q(2.0)), 0.0);
    program(&mut e, 205, "Dimmer", 0.5);
    record(&mut e, None, RecordMode::Merge, None).unwrap();
    assert_eq!(hard(&e, 2.0, 205, "Dimmer"), Some(0.5));
    assert_eq!(hard(&e, 2.0, 202, "Dimmer"), Some(1.0));
    record(&mut e, None, RecordMode::Replace, None).unwrap();
    assert_eq!(hard(&e, 2.0, 202, "Dimmer"), None);
    assert_eq!(hard(&e, 2.0, 205, "Dimmer"), Some(0.5));
    assert_eq!(hard(&e, 1.0, 205, "Dimmer"), None);
}

#[test]
fn cue_07_deleting_the_current_cue_keeps_the_output_and_go_runs_the_one_after() {
    let mut e = three();
    e.set_fader(1, 1.0, 0.0).unwrap();
    tick(&mut e, 0.0);
    act(&mut e, ListAction::Go, 1.0);
    let out = frame(&e);
    e.delete_cue(1, q(2.0)).unwrap();
    run(&mut e, 1.5, 2.0);
    assert_eq!(frame(&e), out);
    act(&mut e, ListAction::Go, 3.0);
    assert_eq!(current(&e), Some(3.0));
}

#[test]
fn cue_08_current_and_next_follow_their_cue() {
    let mut e = three();
    act(&mut e, ListAction::Goto(q(2.0)), 0.0);
    e.renumber_cue(1, q(2.0), q(5.0)).unwrap();
    assert_eq!((current(&e), next(&e)), (Some(5.0), Some(3.0)));
    e.renumber_cues(1).unwrap();
    assert_eq!((current(&e), next(&e)), (Some(3.0), Some(2.0)));
}

// ---- CUE-52: Update modes --------------------------------------------------------------------

/// Q1 par 201 at 50 % and wash 101 Pan 0.2; Q2 par 202; Q3 par 203; Q4 par 201 at 80 %.
fn update_list() -> Engine {
    let mut e = common::church();
    add(
        &mut e,
        1,
        1,
        1,
        vec![
            cue(1.0, &[(201, "Dimmer", 0.5), (101, "Pan", 0.2)]),
            cue(2.0, &[(202, "Dimmer", 1.0)]),
            cue(3.0, &[(203, "Dimmer", 1.0)]),
            cue(4.0, &[(201, "Dimmer", 0.8)]),
        ],
    );
    e
}

#[test]
fn cue_52_forwards_stores_in_the_cue_and_later_cues_track_it() {
    let mut e = update_list();
    program(&mut e, 201, "Dimmer", 0.3);
    record(
        &mut e,
        Some(2.0),
        RecordMode::Merge,
        Some(UpdateMode::Forwards),
    )
    .unwrap();
    assert_eq!(hard(&e, 2.0, 201, "Dimmer"), Some(0.3));
    assert_eq!(hard(&e, 3.0, 201, "Dimmer"), None);
    assert_eq!(hard(&e, 4.0, 201, "Dimmer"), Some(0.8));
    assert_eq!(hard(&e, 1.0, 201, "Dimmer"), Some(0.5));
}

#[test]
fn cue_52_cue_only_protects_the_cue_after_it() {
    let mut e = update_list();
    program(&mut e, 201, "Dimmer", 0.3);
    record(
        &mut e,
        Some(2.0),
        RecordMode::Merge,
        Some(UpdateMode::CueOnly),
    )
    .unwrap();
    assert_eq!(hard(&e, 2.0, 201, "Dimmer"), Some(0.3));
    assert_eq!(
        hard(&e, 3.0, 201, "Dimmer"),
        Some(0.5),
        "the old state value"
    );
    assert_eq!(hard(&e, 4.0, 201, "Dimmer"), Some(0.8));
}

#[test]
fn cue_52_cue_only_on_an_attribute_the_state_did_not_hold() {
    let mut e = update_list();
    program(&mut e, 204, "Dimmer", 0.6);
    record(
        &mut e,
        Some(2.0),
        RecordMode::Merge,
        Some(UpdateMode::CueOnly),
    )
    .unwrap();
    assert_eq!(hard(&e, 3.0, 204, "Dimmer"), Some(0.0), "HTP gets 0");
    program(&mut e, 102, "Pan", 0.7);
    record(
        &mut e,
        Some(2.0),
        RecordMode::Merge,
        Some(UpdateMode::CueOnly),
    )
    .unwrap();
    assert_eq!(hard(&e, 2.0, 102, "Pan"), Some(0.7));
    assert_eq!(hard(&e, 3.0, 102, "Pan"), None, "LTP tracks on");
}

#[test]
fn cue_52_cue_only_leaves_a_cue_that_holds_it_or_blocks() {
    let mut e = update_list();
    program(&mut e, 201, "Dimmer", 0.3);
    record(
        &mut e,
        Some(3.0),
        RecordMode::Merge,
        Some(UpdateMode::CueOnly),
    )
    .unwrap();
    assert_eq!(hard(&e, 4.0, 201, "Dimmer"), Some(0.8), "Q4 holds it");
    change(
        &mut e,
        3.0,
        CueChange {
            track: Some(Track::Block),
            ..CueChange::default()
        },
    );
    record(
        &mut e,
        Some(2.0),
        RecordMode::Merge,
        Some(UpdateMode::CueOnly),
    )
    .unwrap();
    assert_eq!(hard(&e, 3.0, 201, "Dimmer"), Some(0.3), "Q3 kept its own");
}

#[test]
fn cue_52_backwards_stores_where_the_value_tracks_from_and_protects() {
    let mut e = update_list();
    program(&mut e, 201, "Dimmer", 0.3);
    record(
        &mut e,
        Some(2.0),
        RecordMode::QuickMerge,
        Some(UpdateMode::Backwards),
    )
    .unwrap();
    assert_eq!(hard(&e, 1.0, 201, "Dimmer"), Some(0.3), "the source cue");
    assert_eq!(hard(&e, 2.0, 201, "Dimmer"), None);
    assert_eq!(hard(&e, 3.0, 201, "Dimmer"), Some(0.5), "protected");
}

#[test]
fn cue_52_both_stores_where_the_value_tracks_from_without_protection() {
    let mut e = update_list();
    program(&mut e, 201, "Dimmer", 0.3);
    record(
        &mut e,
        Some(2.0),
        RecordMode::QuickMerge,
        Some(UpdateMode::Both),
    )
    .unwrap();
    assert_eq!(hard(&e, 1.0, 201, "Dimmer"), Some(0.3));
    assert_eq!(hard(&e, 2.0, 201, "Dimmer"), None);
    assert_eq!(hard(&e, 3.0, 201, "Dimmer"), None);
}

#[test]
fn cue_52_update_takes_the_cue_state_tracked_values_included() {
    // Owner, 2026-10-21.
    let mut e = update_list();
    program(&mut e, 201, "Dimmer", 0.3);
    also(&mut e, 205, "Dimmer", 0.9);
    record(
        &mut e,
        Some(2.0),
        RecordMode::QuickMerge,
        Some(UpdateMode::Forwards),
    )
    .unwrap();
    assert_eq!(hard(&e, 2.0, 201, "Dimmer"), Some(0.3));
    assert_eq!(hard(&e, 2.0, 205, "Dimmer"), None, "not in the state");
}

#[test]
fn cue_52_without_tracking_every_mode_stores_in_the_cue() {
    let mut e = update_list();
    options(
        &mut e,
        1,
        ListOptions {
            tracking: false,
            ..ListOptions::default()
        },
    );
    program(&mut e, 201, "Dimmer", 0.3);
    record(
        &mut e,
        Some(2.0),
        RecordMode::Merge,
        Some(UpdateMode::Backwards),
    )
    .unwrap();
    assert_eq!(hard(&e, 2.0, 201, "Dimmer"), Some(0.3));
    assert_eq!(hard(&e, 1.0, 201, "Dimmer"), Some(0.5));
    assert_eq!(hard(&e, 3.0, 201, "Dimmer"), None);
    program(&mut e, 201, "Dimmer", 0.1);
    assert!(
        record(&mut e, Some(3.0), RecordMode::QuickMerge, None).is_err(),
        "Update without tracking takes only what Q3 holds"
    );
}

#[test]
fn cue_52_the_setting_is_the_default_mode() {
    let mut e = update_list();
    settings(
        &mut e,
        PlaybackSettingsChange {
            update_mode: Some(UpdateMode::CueOnly),
            ..PlaybackSettingsChange::default()
        },
    );
    program(&mut e, 201, "Dimmer", 0.3);
    record(&mut e, Some(2.0), RecordMode::Merge, None).unwrap();
    assert_eq!(hard(&e, 3.0, 201, "Dimmer"), Some(0.5));
}

#[test]
fn cue_52_replace_stores_the_new_contents_only() {
    let mut e = update_list();
    program(&mut e, 201, "Dimmer", 0.3);
    record(
        &mut e,
        Some(2.0),
        RecordMode::Replace,
        Some(UpdateMode::CueOnly),
    )
    .unwrap();
    let q2 = &e.playback(1).unwrap().cues[1];
    assert_eq!(q2.values, levels(&[(201, "Dimmer", 0.3)]));
    assert_eq!(
        hard(&e, 3.0, 201, "Dimmer"),
        None,
        "later cues track from it"
    );
}

// ---- CUE-53: tracking follows the stored order -----------------------------------------------

#[test]
fn cue_53_after_a_delete_later_cues_track_from_the_cue_before() {
    let mut e = common::church();
    add(
        &mut e,
        1,
        1,
        1,
        vec![
            cue(1.0, &[(201, "Dimmer", 0.5)]),
            cue(2.0, &[(201, "Dimmer", 1.0)]),
            cue(3.0, &[(202, "Dimmer", 1.0)]),
        ],
    );
    e.delete_cue(1, q(2.0)).unwrap();
    e.set_fader(1, 1.0, 0.0).unwrap();
    e.cuelist(Some(1), ListAction::Goto(q(3.0)), 0.0).unwrap();
    run(&mut e, 0.0, 0.5);
    assert_eq!(dmx(&e, 21), 128);
}

/// List on 1.1: Q1 par 201 at 50 %, Q2 par 202 with a 5 s fade, running on Q2 at full.
fn on_q2() -> Engine {
    let mut e = common::church();
    let mut q2 = cue(2.0, &[(202, "Dimmer", 1.0)]);
    q2.fade_in = 5.0;
    add(&mut e, 1, 1, 1, vec![cue(1.0, &[(201, "Dimmer", 0.5)]), q2]);
    e.set_fader(1, 1.0, 0.0).unwrap();
    tick(&mut e, 0.0);
    act(&mut e, ListAction::Go, 1.0);
    run(&mut e, 1.0, 7.0);
    assert_eq!(dmx(&e, 21), 128);
    e
}

#[test]
fn cue_53_an_edit_to_the_current_state_is_on_stage_at_once() {
    let mut e = on_q2();
    program(&mut e, 201, "Dimmer", 0.2);
    record_clean(&mut e, Some(1.0), RecordMode::Merge, None);
    tick(&mut e, 7.025);
    assert_eq!(dmx(&e, 21), 51, "no fade");
}

#[test]
fn cue_53_a_cue_recorded_before_the_current_one_tracks_into_it() {
    let mut e = on_q2();
    program(&mut e, 201, "Dimmer", 0.2);
    e.record_playback(&PlaybackRecord {
        cue: Some(q(1.5)),
        ..PlaybackRecord::at(1, 1)
    })
    .unwrap();
    e.clear_selection();
    e.clear();
    tick(&mut e, 7.025);
    assert_eq!(dmx(&e, 21), 51);
}

#[test]
fn cue_53_a_track_change_to_the_current_cue_is_on_stage_at_once() {
    let mut e = on_q2();
    change(
        &mut e,
        2.0,
        CueChange {
            track: Some(Track::Block),
            ..CueChange::default()
        },
    );
    tick(&mut e, 7.025);
    assert_eq!(dmx(&e, 21), 0, "Q1's par no longer tracks into Q2");
    assert_eq!(dmx(&e, 22), 255);
}
