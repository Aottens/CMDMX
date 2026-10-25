//! Chases, Tap and the speed masters (VS3-04): `specs/modules/cuelists.md` CUE-28..37 with the
//! owner's VS3-04 answers (owner decision 7). What reaches the frame is mostly in the scenarios
//! `tests/scenarios/CUE-28..36-*.json`; these tests cover tap timing, the random order, the
//! effective tempo, the live state the IPC will show and the refusals.
//!
//! API these tests fix (on top of `tests/cuelist_run.rs`):
//! - `Engine::cuelist(id, ListAction, now)` works on a chase (CUE-32): Go fires an inactive chase
//!   (at fader 0 too, HTP out, CUE-31), Go on a running chase starts the next step now, Back the
//!   previous step (one step against the direction; Random: the step shown before; wraps with
//!   `Loop`, refused at the start with `StopOnFinal`), Stop pauses (Stop on a paused chase =
//!   Back), Go to cue starts that step, Set next sets the step Go or the clock starts next. Back
//!   and Go to cue on a paused chase step and keep it paused; only Go resumes.
//! - `Engine::list_live(id)` also answers for a chase: `current` = the step on stage, `next` = the
//!   step Go or the clock starts next, `paused`, `progress` = how far the step clock is into the
//!   current step (0.0–1.0).
//! - `Engine::tap(id, at: Timestamp) -> Result<(), CoreError>` (CUE-33): `at` is the receive
//!   time. Refused on a memory, a cue list and a Rate master. `press_button` passes its `now` as
//!   the tap time.
//! - `Engine::set_bpm(id, bpm) -> Result<(), CoreError>` (CUE-34): 1–600, on a chase or a BPM
//!   master; a chase following an existing `Bpm n` master sets the master.
//! - `Engine::effective_bpm(id) -> Option<f32>` (CUE-29, 35): a chase's tempo after its speed
//!   source; a BPM master's tempo (fader or bpm, times the multiplier); `None` otherwise.
//! - Masters never fire (CUE-37): `set_fader` on a master only sets its level.
//! - Scenario actions `tap {playback, at}` and `bpm {playback, bpm}`.
//! - Time model: a step is due when the step clock reaches it (1e-6 of a step allowance); the
//!   clock advances by Δt · BPM / 60 per tick and starts at most one step per tick. A tap starts
//!   the next step in the next tick with the clock counting from the tap's time.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

mod common;

use cmdmx_core::engine::Engine;
use cmdmx_core::playback::{
    BpmFader, BpmMaster, ChaseOptions, Cue, CueNumber, Direction, HandleButton, Kind, ListAction,
    LoopAction, Master, MasterKind, Playback, PlaybackChange, RateMaster, SpeedSource, Values,
};
use cmdmx_core::show::Meta;

const TICK: f64 = 0.025;

// ---- helpers -------------------------------------------------------------------------------

fn q(n: f64) -> CueNumber {
    CueNumber::new(n).unwrap()
}

/// `n` steps, step k = par 200 + k at full (DMX 20 + k).
fn steps(n: u32) -> Vec<Cue> {
    (1..=n)
        .map(|k| {
            let mut values = Values::new();
            values
                .entry((200 + k).into())
                .or_default()
                .insert("Dimmer".to_string(), 1.0.into());
            Cue {
                number: q(f64::from(k)),
                ..Cue::new(values)
            }
        })
        .collect()
}

fn opts(bpm: f32, direction: Direction, looping: LoopAction) -> ChaseOptions {
    ChaseOptions {
        bpm,
        direction,
        looping,
        ..ChaseOptions::default()
    }
}

/// Chase `id` on handle 1.`slot` with `n` steps.
fn add_chase(e: &mut Engine, id: u32, slot: u16, n: u32, chase: ChaseOptions) {
    e.add_playback(Playback {
        kind: Kind::Chase,
        chase,
        cues: steps(n),
        ..Playback::new(id, 1, slot, Values::new())
    });
}

fn chase(n: u32, chase: ChaseOptions) -> Engine {
    let mut e = common::church();
    add_chase(&mut e, 1, 1, n, chase);
    e
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

fn step(e: &Engine) -> Option<f64> {
    e.list_live(1).unwrap().current.map(CueNumber::as_f64)
}

/// The steps a chase shows, one entry per change, from `from` to `to`.
fn shown(e: &mut Engine, from: f64, to: f64) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    let mut i = (from / TICK).round() as u64;
    loop {
        let t = i as f64 * TICK;
        if t > to + 1e-9 {
            break;
        }
        e.tick(t);
        if let Some(s) = step(e) {
            if out.last() != Some(&s) {
                out.push(s);
            }
        }
        i += 1;
    }
    out
}

fn bpm_master(e: &mut Engine, slot: u16, bpm: f32, multiplier: f32, fader: BpmFader) -> u32 {
    let id = e.record_master(1, slot, MasterKind::Bpm, 1).unwrap();
    let master = Master::Bpm(BpmMaster {
        number: 1,
        bpm,
        multiplier,
        fader,
        fader_max: 240.0,
    });
    e.change_playback(
        id,
        &PlaybackChange {
            master: Some(master),
            ..PlaybackChange::default()
        },
    )
    .unwrap();
    id
}

fn rate_master(e: &mut Engine, slot: u16, number: u8, rate_scale: u16) -> u32 {
    let id = e.record_master(1, slot, MasterKind::Rate, number).unwrap();
    e.change_playback(
        id,
        &PlaybackChange {
            master: Some(Master::Rate(RateMaster { number, rate_scale })),
            ..PlaybackChange::default()
        },
    )
    .unwrap();
    id
}

fn source(e: &mut Engine, id: u32, speed_source: SpeedSource) {
    let mut chase = e.playback(id).unwrap().chase.clone();
    chase.speed_source = speed_source;
    e.change_playback(
        id,
        &PlaybackChange {
            chase: Some(chase),
            ..PlaybackChange::default()
        },
    )
    .unwrap();
}

fn bpm_of(e: &Engine, id: u32) -> f32 {
    e.playback(id).unwrap().chase.bpm
}

fn master_bpm(e: &Engine, id: u32) -> f32 {
    match e.playback(id).unwrap().master.as_ref().unwrap() {
        Master::Bpm(m) => m.bpm,
        other => panic!("not a BPM master: {other:?}"),
    }
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

fn meta() -> Meta {
    Meta {
        name: "t".into(),
        created: String::new(),
        modified: String::new(),
        app_version: String::new(),
    }
}

// ---- CUE-28: the chase clock -----------------------------------------------------------------

#[test]
fn cue_28_a_stalled_tick_starts_one_step() {
    let mut e = chase(4, opts(600.0, Direction::Forward, LoopAction::Loop));
    e.set_fader(1, 1.0, 0.0).unwrap();
    e.tick(0.0);
    assert_eq!(step(&e), Some(1.0));
    e.tick(5.0); // fifty steps due
    assert_eq!(step(&e), Some(2.0), "at most one step per tick");
    assert!(e.list_live(1).unwrap().progress < 1.0);
}

#[test]
fn cue_28_the_live_state_of_a_chase() {
    let mut e = chase(3, opts(120.0, Direction::Forward, LoopAction::Loop));
    assert_eq!(e.list_live(1).unwrap().current, None);
    assert_eq!(e.list_live(1).unwrap().next, Some(q(1.0)));
    e.set_fader(1, 1.0, 0.0).unwrap();
    run(&mut e, 0.0, 0.25);
    let live = e.list_live(1).unwrap();
    assert_eq!(live.current, Some(q(1.0)));
    assert_eq!(live.next, Some(q(2.0)));
    assert!(!live.paused);
    assert!(close(live.progress, 0.5), "{}", live.progress);
    run(&mut e, 0.275, 1.1);
    assert_eq!(
        e.list_live(1).unwrap().next,
        Some(q(1.0)),
        "Loop: 1 after 3"
    );
    assert_eq!(e.effective_bpm(1), Some(120.0));
}

#[test]
fn cue_28_a_kill_resets_to_the_first_step() {
    let mut e = chase(3, opts(120.0, Direction::Forward, LoopAction::Loop));
    e.set_fader(1, 1.0, 0.0).unwrap();
    run(&mut e, 0.0, 1.1);
    assert_eq!(step(&e), Some(3.0));
    e.release_playback(1, 1.2).unwrap();
    e.tick(1.2);
    let live = e.list_live(1).unwrap();
    assert_eq!((live.current, live.next), (None, Some(q(1.0))));
}

#[test]
fn cue_28_set_next_on_an_inactive_chase_is_where_it_starts() {
    let mut e = chase(3, opts(120.0, Direction::Forward, LoopAction::Loop));
    e.cuelist(Some(1), ListAction::Next(q(3.0)), 0.0).unwrap();
    e.set_fader(1, 1.0, 0.0).unwrap();
    e.tick(0.0);
    assert_eq!(step(&e), Some(3.0));
    assert_eq!(common::dmx(&e, 23), 255);
}

// ---- CUE-29, 35..37: the effective tempo ----------------------------------------------------

#[test]
fn cue_29_speed_sources_and_the_missing_master_fallback() {
    let mut e = chase(3, opts(100.0, Direction::Forward, LoopAction::Loop));
    assert_eq!(e.effective_bpm(1), Some(100.0), "Free");
    source(&mut e, 1, SpeedSource::Bpm(1));
    assert_eq!(e.effective_bpm(1), Some(100.0), "no BPM 1 yet: runs free");
    let master = bpm_master(&mut e, 2, 90.0, 2.0, BpmFader::Unused);
    assert_eq!(e.effective_bpm(1), Some(180.0), "BPM 1 × 2");
    assert_eq!(e.effective_bpm(master), Some(180.0));
    e.delete_playback(master).unwrap();
    assert_eq!(e.effective_bpm(1), Some(100.0), "deleted: runs free again");
    assert_eq!(
        e.playback(1).unwrap().chase.speed_source,
        SpeedSource::Bpm(1),
        "the source is kept"
    );
    let rate = rate_master(&mut e, 3, 2, 200);
    source(&mut e, 1, SpeedSource::Rate(2));
    e.set_fader(rate, 0.75, 0.0).unwrap();
    assert_eq!(
        e.effective_bpm(1),
        Some(150.0),
        "fader 75 % of 200 % = ×1.5"
    );
    assert_eq!(e.effective_bpm(rate), None);
}

#[test]
fn cue_35_bpm_on_fader_until_a_tap_or_a_typed_tempo() {
    let mut e = common::church();
    let m = bpm_master(&mut e, 2, 120.0, 1.0, BpmFader::Bpm);
    e.set_fader(m, 0.5, 0.0).unwrap();
    assert_eq!(e.effective_bpm(m), Some(120.0), "50 % of 240");
    e.set_fader(m, 0.25, 0.0).unwrap();
    assert_eq!(e.effective_bpm(m), Some(60.0));
    e.tap(m, 1.0).unwrap();
    e.tap(m, 1.5).unwrap();
    assert_eq!(e.effective_bpm(m), Some(120.0), "the tap wins");
    e.set_fader(m, 0.75, 2.0).unwrap();
    assert_eq!(e.effective_bpm(m), Some(180.0), "until the fader moves");
    e.set_bpm(m, 100.0).unwrap();
    assert_eq!(e.effective_bpm(m), Some(100.0), "a typed tempo wins too");
    e.set_fader(m, 0.0, 3.0).unwrap();
    assert_eq!(e.effective_bpm(m), Some(0.0), "fader 0 stops it");
    assert_eq!(
        master_bpm(&e, m),
        100.0,
        "the fader's tempo is live, not saved"
    );
}

#[test]
fn cue_35_multiplier_on_fader_picks_five_zones() {
    let mut e = common::church();
    let m = bpm_master(&mut e, 2, 120.0, 1.0, BpmFader::Multiplier);
    for (level, bpm) in [
        (0.0, 30.0),
        (0.1, 30.0),
        (0.3, 60.0),
        (0.5, 120.0),
        (0.7, 240.0),
        (1.0, 480.0),
    ] {
        e.set_fader(m, level, 0.0).unwrap();
        assert_eq!(e.effective_bpm(m), Some(bpm), "fader {level}");
    }
}

#[test]
fn cue_44_a_bpm_master_reports_its_effective_multiplier() {
    let mut e = common::church();
    let m = bpm_master(&mut e, 2, 120.0, 1.0, BpmFader::Multiplier);
    e.set_fader(m, 0.7, 0.0).unwrap();
    assert_eq!(e.effective_multiplier(m), Some(2.0), "the fader's zone");
    assert_eq!(e.effective_multiplier(999), None, "only BPM masters");
    let mut e = common::church();
    let fixed = bpm_master(&mut e, 3, 120.0, 0.5, BpmFader::Unused);
    e.set_fader(fixed, 0.7, 0.0).unwrap();
    assert_eq!(e.effective_multiplier(fixed), Some(0.5), "the stored one");
}

#[test]
fn cue_35_without_a_fader_mode_the_fader_does_nothing() {
    let mut e = common::church();
    let m = bpm_master(&mut e, 2, 120.0, 0.5, BpmFader::Unused);
    e.set_fader(m, 0.2, 0.0).unwrap();
    assert_eq!(
        e.effective_bpm(m),
        Some(60.0),
        "bpm × the stored multiplier"
    );
}

#[test]
fn cue_36_rate_scale_and_fader() {
    let mut e = chase(3, opts(120.0, Direction::Forward, LoopAction::Loop));
    let rate = rate_master(&mut e, 2, 1, 100);
    source(&mut e, 1, SpeedSource::Rate(1));
    assert_eq!(
        e.effective_bpm(1),
        Some(0.0),
        "a rate master at 0 stops its chases"
    );
    e.set_fader(rate, 0.5, 0.0).unwrap();
    assert_eq!(e.effective_bpm(1), Some(60.0), "50 % of 100 %");
    e.set_fader(rate, 1.0, 0.0).unwrap();
    assert_eq!(e.effective_bpm(1), Some(120.0));
}

#[test]
fn cue_37_masters_never_fire_or_enter_the_merge() {
    let mut e = common::church();
    let m = bpm_master(&mut e, 2, 120.0, 1.0, BpmFader::Bpm);
    let r = rate_master(&mut e, 3, 1, 200);
    for id in [m, r] {
        e.set_fader(id, 1.0, 0.0).unwrap();
    }
    e.tick(0.0);
    for id in [m, r] {
        assert!(
            !e.playback_state(id).is_some_and(|s| s.is_active()),
            "master {id} fired"
        );
    }
}

#[test]
fn cue_31_a_zero_sent_again_does_not_kill_a_chase_started_at_zero() {
    let mut e = chase(3, opts(120.0, Direction::Forward, LoopAction::Loop));
    act(&mut e, ListAction::Go, 0.0);
    e.set_fader(1, 0.0, 0.1).unwrap(); // a touched fader at 0
    e.tick(0.1);
    assert!(e.playback_state(1).unwrap().is_active(), "no fall, no kill");
    e.set_fader(1, 0.5, 0.2).unwrap();
    e.set_fader(1, 0.0, 0.3).unwrap();
    e.tick(0.3);
    assert!(!e.playback_state(1).unwrap().is_active(), "the fall kills");
}

#[test]
fn cue_33_a_tap_time_that_is_not_a_number_is_refused() {
    let mut e = chase(3, opts(120.0, Direction::Forward, LoopAction::Loop));
    assert!(e.tap(1, f64::NAN).is_err());
    taps(&mut e, 1, &[0.0, 0.5]);
    assert_eq!(bpm_of(&e, 1), 120.0);
}

// ---- CUE-30: direction -----------------------------------------------------------------------

#[test]
fn cue_30_random_never_repeats_a_step_and_visits_every_step() {
    let mut e = chase(5, opts(600.0, Direction::Random, LoopAction::Loop));
    e.set_fader(1, 1.0, 0.0).unwrap();
    let seq = shown(&mut e, 0.0, 6.0);
    assert_eq!(seq.first(), Some(&1.0), "it starts with the first step");
    assert!(seq.len() >= 55, "{seq:?}");
    for k in 1..=5 {
        assert!(seq.contains(&f64::from(k)), "step {k} never shown: {seq:?}");
    }
    let plain: Vec<f64> = (0..seq.len()).map(|i| (i % 5 + 1) as f64).collect();
    assert_ne!(seq, plain, "random is not forward");
}

#[test]
fn cue_30_random_is_deterministic_and_restarts_after_a_kill() {
    let sequence = |e: &mut Engine, from: f64| {
        e.set_fader(1, 1.0, from).unwrap();
        shown(e, from, from + 3.0)
    };
    let mut a = chase(4, opts(600.0, Direction::Random, LoopAction::Loop));
    let mut b = chase(4, opts(600.0, Direction::Random, LoopAction::Loop));
    let first = sequence(&mut a, 0.0);
    assert_eq!(first, sequence(&mut b, 0.0), "same show, same order");
    a.set_fader(1, 0.0, 3.5).unwrap();
    a.tick(3.5);
    assert_eq!(sequence(&mut a, 4.0), first, "a kill restarts the sequence");
}

#[test]
fn cue_30_random_stop_on_final_shows_as_many_steps_as_the_chase_has() {
    let mut e = chase(4, opts(600.0, Direction::Random, LoopAction::StopOnFinal));
    e.set_fader(1, 1.0, 0.0).unwrap();
    let seq = shown(&mut e, 0.0, 5.0);
    assert_eq!(seq.len(), 4, "{seq:?}");
    assert!(
        e.playback_state(1).unwrap().is_active(),
        "it stays on stage"
    );
}

// ---- CUE-32: Go, Back, Stop, Go to cue on a chase --------------------------------------------

#[test]
fn cue_32_back_wraps_with_loop_and_is_refused_at_the_start_without() {
    let mut e = chase(4, opts(60.0, Direction::Forward, LoopAction::Loop));
    act(&mut e, ListAction::Go, 0.0);
    e.set_fader(1, 1.0, 0.0).unwrap(); // Go at fader 0 runs with HTP out (CUE-31)
    act(&mut e, ListAction::Back, 0.5);
    assert_eq!(step(&e), Some(4.0));
    assert_eq!(common::dmx(&e, 24), 255);

    let mut e = chase(4, opts(60.0, Direction::Forward, LoopAction::StopOnFinal));
    act(&mut e, ListAction::Go, 0.0);
    assert!(e.cuelist(Some(1), ListAction::Back, 0.5).is_err());

    let mut e = chase(4, opts(60.0, Direction::Backward, LoopAction::Loop));
    act(&mut e, ListAction::Go, 0.0);
    assert_eq!(step(&e), Some(4.0), "Backward starts on the last step");
    act(&mut e, ListAction::Back, 0.5);
    assert_eq!(step(&e), Some(1.0), "against Backward is n + 1, wrapping");
}

#[test]
fn cue_32_back_on_bounce_and_random() {
    let mut e = chase(3, opts(120.0, Direction::Bounce, LoopAction::Loop));
    e.set_fader(1, 1.0, 0.0).unwrap();
    run(&mut e, 0.0, 1.6); // 1 2 3 2: on its way down
    assert_eq!(step(&e), Some(2.0));
    act(&mut e, ListAction::Back, 1.7);
    assert_eq!(step(&e), Some(3.0), "against the way down");

    let mut e = chase(5, opts(600.0, Direction::Random, LoopAction::Loop));
    e.set_fader(1, 1.0, 0.0).unwrap();
    let seq = shown(&mut e, 0.0, 1.0);
    let before = seq[seq.len() - 2];
    act(&mut e, ListAction::Back, 1.025);
    assert_eq!(step(&e), Some(before), "the step shown before: {seq:?}");
}

#[test]
fn cue_32_a_paused_chase_steps_by_hand_and_stays_paused() {
    let mut e = chase(4, opts(60.0, Direction::Forward, LoopAction::Loop));
    act(&mut e, ListAction::Go, 0.0);
    act(&mut e, ListAction::Stop, 0.5);
    assert!(e.list_live(1).unwrap().paused);
    act(&mut e, ListAction::Goto(q(3.0)), 1.0);
    run(&mut e, 1.025, 4.0);
    assert_eq!(step(&e), Some(3.0));
    assert!(e.list_live(1).unwrap().paused, "Go to cue keeps it paused");
    act(&mut e, ListAction::Stop, 4.0);
    assert_eq!(step(&e), Some(2.0), "Stop on a paused chase goes back");
    assert!(e.list_live(1).unwrap().paused);
    act(&mut e, ListAction::Go, 5.0);
    assert!(!e.list_live(1).unwrap().paused, "Go resumes");
    assert_eq!(step(&e), Some(2.0), "and does not step");
    run(&mut e, 5.025, 6.1);
    assert_eq!(step(&e), Some(3.0), "the step counts from the resume");
}

#[test]
fn cue_32_go_on_a_chase_connects_it() {
    let mut e = chase(3, opts(120.0, Direction::Forward, LoopAction::Loop));
    add_chase(
        &mut e,
        2,
        2,
        3,
        opts(120.0, Direction::Forward, LoopAction::Loop),
    );
    e.cuelist(Some(2), ListAction::Go, 0.0).unwrap();
    assert_eq!(e.connected(), Some(2));
    e.tick(0.0);
    assert!(e.playback_state(2).unwrap().is_active());
}

// ---- CUE-33: Tap ------------------------------------------------------------------------------

fn taps(e: &mut Engine, id: u32, at: &[f64]) {
    for &t in at {
        e.tap(id, t).unwrap();
    }
}

#[test]
fn cue_33_the_first_tap_changes_nothing() {
    let mut e = chase(3, opts(120.0, Direction::Forward, LoopAction::Loop));
    taps(&mut e, 1, &[1.0]);
    assert_eq!(bpm_of(&e, 1), 120.0);
}

#[test]
fn cue_33_taps_set_the_bpm_from_the_mean_of_the_last_three_intervals() {
    let mut e = chase(3, opts(120.0, Direction::Forward, LoopAction::Loop));
    taps(&mut e, 1, &[10.0, 10.4, 10.8, 11.2]);
    assert!(close(bpm_of(&e, 1), 150.0), "{}", bpm_of(&e, 1));
    taps(&mut e, 1, &[11.8]); // intervals 0.4 0.4 0.6 → 0.4667 s
    assert!(close(bpm_of(&e, 1), 128.6), "{}", bpm_of(&e, 1));
}

#[test]
fn cue_33_a_pause_over_two_seconds_starts_a_new_series() {
    let mut e = chase(3, opts(120.0, Direction::Forward, LoopAction::Loop));
    taps(&mut e, 1, &[0.0, 0.5, 1.0, 1.5]);
    assert!(close(bpm_of(&e, 1), 120.0));
    taps(&mut e, 1, &[3.6]);
    assert!(close(bpm_of(&e, 1), 120.0), "2.1 s: a new series");
    taps(&mut e, 1, &[4.6]);
    assert!(close(bpm_of(&e, 1), 60.0), "only the new interval counts");
}

#[test]
fn cue_33_rounded_to_a_tenth_and_clamped() {
    let mut e = chase(3, opts(120.0, Direction::Forward, LoopAction::Loop));
    taps(&mut e, 1, &[0.0, 1.99]);
    assert!(close(bpm_of(&e, 1), 30.2), "{}", bpm_of(&e, 1));
    taps(&mut e, 1, &[5.0, 5.05]);
    assert_eq!(bpm_of(&e, 1), 600.0, "1200 BPM is clamped");
}

#[test]
fn cue_33_tap_is_saved_with_the_show() {
    let mut e = chase(3, opts(120.0, Direction::Forward, LoopAction::Loop));
    taps(&mut e, 1, &[0.0, 0.4]);
    let show = e.to_show(meta());
    let saved = show.playbacks.playbacks.iter().find(|p| p.id == 1).unwrap();
    assert!(close(saved.chase.bpm, 150.0));
}

#[test]
fn cue_33_tap_is_refused_on_a_memory_a_cue_list_and_a_rate_master() {
    let mut e = common::church();
    let memory = steps(1);
    e.add_playback(Playback {
        cues: memory,
        ..Playback::new(1, 1, 1, Values::new())
    });
    e.add_playback(Playback {
        cues: steps(2),
        ..Playback::new(2, 1, 2, Values::new())
    });
    let rate = rate_master(&mut e, 3, 1, 200);
    for id in [1, 2, rate] {
        assert!(e.tap(id, 0.0).is_err(), "tap on {id}");
    }
}

#[test]
fn cue_33_handle_buttons_tap_a_chase_and_a_bpm_master() {
    let mut e = chase(3, opts(120.0, Direction::Forward, LoopAction::Loop));
    e.press_button(1, HandleButton::Upper, true, 0.0).unwrap();
    e.press_button(1, HandleButton::Upper, false, 0.1).unwrap();
    e.press_button(1, HandleButton::Upper, true, 0.4).unwrap();
    assert!(close(bpm_of(&e, 1), 150.0), "{}", bpm_of(&e, 1));
    let m = bpm_master(&mut e, 2, 120.0, 1.0, BpmFader::Unused);
    e.press_button(m, HandleButton::Lower, true, 1.0).unwrap();
    e.press_button(m, HandleButton::Lower, true, 2.0).unwrap();
    assert!(close(master_bpm(&e, m), 60.0));
}

#[test]
fn cue_33_a_tap_on_a_chase_following_a_master_goes_to_the_master() {
    let mut e = chase(3, opts(120.0, Direction::Forward, LoopAction::Loop));
    let m = bpm_master(&mut e, 2, 120.0, 1.0, BpmFader::Unused);
    source(&mut e, 1, SpeedSource::Bpm(1));
    taps(&mut e, 1, &[0.0, 0.5]);
    assert!(close(master_bpm(&e, m), 120.0));
    taps(&mut e, 1, &[1.5]);
    assert!(close(master_bpm(&e, m), 80.0), "{}", master_bpm(&e, m));
    assert_eq!(bpm_of(&e, 1), 120.0, "the chase's own bpm is untouched");

    source(&mut e, 1, SpeedSource::Bpm(4));
    taps(&mut e, 1, &[6.0, 7.0]);
    assert!(close(bpm_of(&e, 1), 60.0), "no BPM 4: the chase's own bpm");
}

#[test]
fn cue_33_a_master_tap_beat_syncs_every_chase_on_it() {
    let mut e = chase(4, opts(120.0, Direction::Forward, LoopAction::Loop));
    add_chase(
        &mut e,
        2,
        2,
        4,
        opts(120.0, Direction::Forward, LoopAction::Loop),
    );
    let m = bpm_master(&mut e, 3, 120.0, 1.0, BpmFader::Unused);
    source(&mut e, 1, SpeedSource::Bpm(1));
    source(&mut e, 2, SpeedSource::Bpm(1));
    e.set_fader(1, 1.0, 0.0).unwrap();
    run(&mut e, 0.0, 0.2);
    e.set_fader(2, 1.0, 0.2).unwrap();
    run(&mut e, 0.225, 1.0); // chase 2 runs 0.2 s behind
    e.tap(m, 1.01).unwrap();
    run(&mut e, 1.025, 1.6);
    let (one, two) = (step(&e), e.list_live(2).unwrap().current);
    e.tap(m, 1.61).unwrap(); // 100 BPM, beat at 1.61; neither clock is due at 1.625
    e.tick(1.625);
    assert_ne!(step(&e), one, "chase 1 stepped at the tap");
    assert_ne!(e.list_live(2).unwrap().current, two, "chase 2 stepped too");
    let p1 = e.list_live(1).unwrap().progress;
    let p2 = e.list_live(2).unwrap().progress;
    assert!(close(p1, p2), "in step: {p1} vs {p2}");
    assert!(close(p1, 0.025), "counted from the tap: {p1}");
}

#[test]
fn cue_33_a_tap_on_a_paused_chase_only_sets_the_tempo() {
    let mut e = chase(4, opts(120.0, Direction::Forward, LoopAction::Loop));
    act(&mut e, ListAction::Go, 0.0);
    act(&mut e, ListAction::Stop, 0.1);
    taps(&mut e, 1, &[0.5, 1.5]);
    run(&mut e, 0.125, 3.0);
    assert_eq!(step(&e), Some(1.0));
    assert!(e.list_live(1).unwrap().paused);
    assert!(close(bpm_of(&e, 1), 60.0));
}

#[test]
fn cue_49_same_taps_same_frames() {
    let frames = || {
        let mut e = chase(4, opts(120.0, Direction::Bounce, LoopAction::Loop));
        e.set_fader(1, 1.0, 0.0).unwrap();
        let mut out = Vec::new();
        for i in 0..200u32 {
            let t = f64::from(i) * TICK;
            if i == 50 || i == 70 || i == 91 {
                e.tap(1, t - 0.011).unwrap();
            }
            e.tick(t);
            out.push(e.frame(1).unwrap().slots().to_vec());
        }
        out
    };
    assert_eq!(frames(), frames());
}

// ---- CUE-34: a typed tempo -------------------------------------------------------------------

#[test]
fn cue_34_typed_bpm_on_a_chase_and_a_master() {
    let mut e = chase(3, opts(120.0, Direction::Forward, LoopAction::Loop));
    e.set_bpm(1, 90.0).unwrap();
    assert_eq!(bpm_of(&e, 1), 90.0);
    for bad in [0.5, 600.5, f32::NAN] {
        assert!(e.set_bpm(1, bad).is_err(), "{bad}");
    }
    assert_eq!(bpm_of(&e, 1), 90.0);
    let m = bpm_master(&mut e, 2, 120.0, 1.0, BpmFader::Unused);
    e.set_bpm(m, 100.0).unwrap();
    assert_eq!(master_bpm(&e, m), 100.0);
    source(&mut e, 1, SpeedSource::Bpm(1));
    e.set_bpm(1, 140.0).unwrap();
    assert_eq!(
        master_bpm(&e, m),
        140.0,
        "a chase on BPM 1 types into the master"
    );
    assert_eq!(bpm_of(&e, 1), 90.0);
}

#[test]
fn cue_34_typed_bpm_is_refused_on_a_memory_and_a_rate_master() {
    let mut e = common::church();
    e.add_playback(Playback {
        cues: steps(1),
        ..Playback::new(1, 1, 1, Values::new())
    });
    let rate = rate_master(&mut e, 2, 1, 200);
    assert!(e.set_bpm(1, 120.0).is_err());
    assert!(e.set_bpm(rate, 120.0).is_err());
}
