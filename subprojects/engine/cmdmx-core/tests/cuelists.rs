//! Cue lists, chases and speed masters, core storage (VS3-02): `specs/modules/cuelists.md`
//! CUE-01..09, 15 (the setting), 23, 29 (options), 35..39, 51 (the field). Behaviour on the
//! frame, the connected list and tracking are VS3-03; chases, tap and masters running are
//! VS3-04; IPC and the app VS3-05.
//!
//! API these tests fix (`cmdmx_core::playback` and `Engine`), on top of `tests/playbacks.rs`:
//! - `Playback` gains, after `group` and before `cues` (show file key order): `kind: Kind`
//!   (`Cues` default, `Chase`, `BpmMaster`, `RateMaster`), `list: ListOptions`,
//!   `chase: ChaseOptions` and `master: Option<Master>` (left out of the JSON when `None`);
//!   `Playback::is_cue_list()` (kind `Cues` with two or more cues).
//! - `ListOptions { tracking: true, fader: ListFader::KillWithOff (| KillAtZero),
//!   fire_first_cue: true, looping: LoopAction::StopOnFinal (| Loop) }` (`looping` is `"loop"`
//!   in JSON) and `ChaseOptions { bpm: 120.0, crossfade: 0.0, direction: Direction::Forward
//!   (| Backward | Bounce | Random), looping: LoopAction::Loop, speed_source: SpeedSource::Free
//!   (| Bpm(u8) | Rate(u8)) }`, both `Default` with these values.
//! - `Master::Bpm(BpmMaster { number: u8, bpm: f32, multiplier: f32, fader: BpmFader, fader_max:
//!   f32 })` | `Master::Rate(RateMaster { number: u8, rate_scale: u16 })`, untagged in JSON;
//!   `BpmFader::{Unused, Bpm, Multiplier}` (`Unused` is `"None"` in JSON); `MasterKind::{Bpm,
//!   Rate}`.
//! - `CueNumber`: thousandths. `CueNumber::new(2.5) -> Option<CueNumber>` (`None` outside
//!   0.001–9999.999 or with more than three decimals), `as_f64()`, `Display` ("2.5", "1"), a
//!   plain JSON number; `CueNumber::between(a, Option<b>) -> Option<CueNumber>` (CUE-05). Any
//!   JSON number parses in `Show::from_json`; one that breaks the rules is left out by
//!   `Engine::from_show` (CUE-39).
//! - `Cue` gains, after `overlap` and before `values`: `number: CueNumber`, `legend: String`,
//!   `link: Link` (`Wait` | `After { offset }` | `With { offset }`, JSON `{"type":"After",
//!   "offset":1.0}`), `track: Track` (`Track` | `Block` | `CueOnly` | `Solo`) and
//!   `group_times: BTreeMap<AttributeGroup, GroupTime { delay, fade }>`. `Cue::new(values)` is
//!   number 1 with these defaults.
//! - `PlaybackRecord` gains `cue: Option<CueNumber>` (CUE-04), `add: Option<AddCue>` (`Append`
//!   | `Insert`, CUE-03/05; together with `mode` it is refused) and `update_mode:
//!   Option<UpdateMode>` (its effect is VS3-03).
//! - `Engine::delete_cue(id, cue)`, `renumber_cue(id, cue, to)`, `renumber_cues(id)`,
//!   `copy_cue(id, cue, to)` and `change_cue(id, cue, &CueChange)` with `CueChange { legend,
//!   delay, fade_in, fade_out, overlap, link, track, group_times }` (each `Option`, all or
//!   nothing).
//! - `PlaybackChange` gains `kind`, `list`, `chase` and `master` (each `Option` of the whole
//!   struct, checked, all or nothing).
//! - `Engine::record_master(page, slot, MasterKind, number) -> Result<PlaybackId, _>`.
//! - `Settings` gains `auto_connect` (true), `cue_list_fader` (`KillWithOff`), `update_mode`
//!   (`UpdateMode::Forwards` | `CueOnly` | `Backwards` | `Both`) and `handle_buttons`
//!   (`HandleButtons::BestPractice` | `Titan` | `Custom(BTreeMap<ButtonKind, [Button; 2]>)`,
//!   `ButtonKind::{Memory, CueList, Chase, BpmMaster, RateMaster}`, `Button::{Flash, Swop, Go,
//!   Back, Stop, Tap, Release, None}`); `PlaybackSettingsChange` gains the same four as
//!   `Option`s.
//! - `SCHEMA_VERSION` 4.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

mod common;

use std::collections::BTreeMap;

use cmdmx_core::attribute::AttributeGroup::{I, P};
use cmdmx_core::engine::{Engine, PlaybackSettingsChange};
use cmdmx_core::palette::RecordMode;
use cmdmx_core::playback::{
    AddCue, BpmFader, BpmMaster, Button, ButtonKind, ChaseOptions, Cue, CueChange, CueNumber,
    Direction, GroupTime, HandleButtons, Kind, Link, ListFader, ListOptions, LoopAction, Master,
    MasterKind, Playback, PlaybackChange, PlaybackRecord, RateMaster, SpeedSource, Track,
    UpdateMode, Values,
};
use cmdmx_core::show::{Meta, Show, SCHEMA_VERSION};
use cmdmx_core::value::{Stored, Value};
use serde_json::{json, Value as Json};

const V3_SHOW: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/fixtures/shows/church-demo-v3.cmdmx.json"
);

// ---- helpers -------------------------------------------------------------------------------

fn meta() -> Meta {
    Meta {
        name: "t".into(),
        created: String::new(),
        modified: String::new(),
        app_version: String::new(),
    }
}

fn q(n: f64) -> CueNumber {
    CueNumber::new(n).unwrap()
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

fn cue(n: f64, values: Values) -> Cue {
    Cue {
        number: q(n),
        ..Cue::new(values)
    }
}

/// Playback 1 on 1.1 with the given cues.
fn list(e: &mut Engine, cues: Vec<Cue>) {
    e.add_playback(Playback {
        cues,
        ..Playback::new(1, 1, 1, Values::new())
    });
}

/// Service list on 1.1: Q1 pars 201–208 at 45 %, Q2 spots 301–306 at 80 %, Q3 washes at full.
fn service() -> Engine {
    let mut e = common::church();
    list(
        &mut e,
        vec![
            cue(
                1.0,
                dimmers(&[201, 202, 203, 204, 205, 206, 207, 208], 0.45),
            ),
            cue(2.0, dimmers(&[301, 302, 303, 304, 305, 306], 0.8)),
            cue(3.0, dimmers(&[101, 102, 103, 104], 1.0)),
        ],
    );
    e
}

fn numbers(e: &Engine, id: u32) -> Vec<f64> {
    e.playback(id)
        .unwrap()
        .cues
        .iter()
        .map(|c| c.number.as_f64())
        .collect()
}

fn cue_at(e: &Engine, id: u32, n: f64) -> &Cue {
    e.playback(id)
        .unwrap()
        .cues
        .iter()
        .find(|c| c.number == q(n))
        .unwrap()
}

/// Selection and programmer hold exactly `fixture` `attribute` at `level`.
fn program(e: &mut Engine, fixture: u32, attribute: &str, level: f32) {
    e.clear_selection();
    e.clear();
    common::select(e, &[fixture]);
    e.set_programmer_value(fixture, attribute, level).unwrap();
}

fn append() -> PlaybackRecord {
    PlaybackRecord {
        add: Some(AddCue::Append),
        ..PlaybackRecord::at(1, 1)
    }
}

fn to_cue(n: f64, mode: Option<RecordMode>) -> PlaybackRecord {
    PlaybackRecord {
        cue: Some(q(n)),
        mode,
        ..PlaybackRecord::at(1, 1)
    }
}

fn show_json(e: &Engine) -> Json {
    serde_json::from_str(&e.to_show(meta()).to_json()).unwrap()
}

fn load(v: &Json) -> (Engine, Vec<String>) {
    let (show, _) = Show::from_json(&v.to_string()).unwrap();
    Engine::from_show(&show)
}

fn keys_in_order(text: &str, from: &str, keys: &[&str]) {
    let mut at = text.find(from).unwrap();
    for key in keys {
        let pattern = format!("\"{key}\":");
        let i = text[at..]
            .find(&pattern)
            .unwrap_or_else(|| panic!("{key} out of order"));
        at += i + pattern.len();
    }
}

// ---- CUE-01: kinds ---------------------------------------------------------------------------

#[test]
fn cue_01_a_new_playback_is_a_memory_with_cue_1() {
    let pb = Playback::new(1, 1, 1, dimmers(&[201], 1.0));
    assert_eq!(pb.kind, Kind::Cues);
    assert!(!pb.is_cue_list());
    assert_eq!(pb.cues[0].number, q(1.0));
    assert_eq!(
        pb.list,
        ListOptions {
            tracking: true,
            fader: ListFader::KillWithOff,
            fire_first_cue: true,
            looping: LoopAction::StopOnFinal,
        }
    );
    assert_eq!(
        pb.chase,
        ChaseOptions {
            bpm: 120.0,
            crossfade: 0.0,
            direction: Direction::Forward,
            looping: LoopAction::Loop,
            speed_source: SpeedSource::Free,
        }
    );
    assert_eq!(pb.list, ListOptions::default());
    assert_eq!(pb.chase, ChaseOptions::default());
    assert_eq!(pb.master, None);
}

#[test]
fn cue_01_appending_makes_a_cue_list_deleting_makes_a_memory() {
    let mut e = common::church();
    program(&mut e, 201, "Dimmer", 0.45);
    let id = e.record_playback(&PlaybackRecord::at(1, 1)).unwrap();
    assert!(!e.playback(id).unwrap().is_cue_list());
    program(&mut e, 301, "Dimmer", 0.8);
    assert_eq!(e.record_playback(&append()).unwrap(), id);
    assert!(e.playback(id).unwrap().is_cue_list());
    assert_eq!(numbers(&e, id), [1.0, 2.0]);
    e.delete_cue(id, q(2.0)).unwrap();
    assert!(!e.playback(id).unwrap().is_cue_list());
}

#[test]
fn cue_01_kind_switches_between_cues_and_chase_keeping_the_cues() {
    let mut e = service();
    let cues = e.playback(1).unwrap().cues.clone();
    let to = |kind| PlaybackChange {
        kind: Some(kind),
        ..PlaybackChange::default()
    };
    e.change_playback(1, &to(Kind::Chase)).unwrap();
    assert_eq!(e.playback(1).unwrap().kind, Kind::Chase);
    assert!(
        !e.playback(1).unwrap().is_cue_list(),
        "a chase is not a cue list"
    );
    assert_eq!(e.playback(1).unwrap().cues, cues);
    e.change_playback(1, &to(Kind::Cues)).unwrap();
    assert_eq!(e.playback(1).unwrap().cues, cues);
    assert!(e.playback(1).unwrap().is_cue_list());
}

#[test]
fn cue_01_a_playback_cannot_become_a_master_or_stop_being_one() {
    let mut e = service();
    for kind in [Kind::BpmMaster, Kind::RateMaster] {
        let change = PlaybackChange {
            kind: Some(kind),
            ..PlaybackChange::default()
        };
        assert!(e.change_playback(1, &change).is_err(), "{kind:?}");
    }
    assert_eq!(e.playback(1).unwrap().kind, Kind::Cues);
    let m = e.record_master(1, 3, MasterKind::Bpm, 1).unwrap();
    let change = PlaybackChange {
        kind: Some(Kind::Cues),
        ..PlaybackChange::default()
    };
    assert!(e.change_playback(m, &change).is_err());
    assert_eq!(e.playback(m).unwrap().kind, Kind::BpmMaster);
}

// ---- CUE-02: cue fields and numbers ----------------------------------------------------------

#[test]
fn cue_02_a_new_cue_has_the_defaults() {
    let c = Cue::new(dimmers(&[201], 1.0));
    assert_eq!(c.number, q(1.0));
    assert_eq!(c.legend, "");
    assert_eq!(c.link, Link::Wait);
    assert_eq!(c.track, Track::Track);
    assert!(c.group_times.is_empty());
}

#[test]
fn cue_02_cue_numbers_have_at_most_three_decimals() {
    assert_eq!(q(2.5).as_f64(), 2.5);
    assert_eq!(q(2.5).to_string(), "2.5");
    assert_eq!(q(1.0).to_string(), "1");
    assert_eq!(q(2.125).to_string(), "2.125");
    assert!(CueNumber::new(0.001).is_some());
    assert!(CueNumber::new(9999.999).is_some());
    for bad in [0.0, -1.0, 10000.0, 2.0005, f64::NAN] {
        assert!(CueNumber::new(bad).is_none(), "{bad}");
    }
    assert!(q(2.0) < q(2.5) && q(2.5) < q(10.0), "ordered by value");
}

// ---- CUE-03: Append --------------------------------------------------------------------------

#[test]
fn cue_03_append_adds_a_cue_after_the_last_whole_number() {
    let mut e = common::church();
    list(
        &mut e,
        vec![
            cue(1.0, dimmers(&[201], 0.5)),
            cue(2.0, dimmers(&[202], 0.5)),
            cue(2.5, dimmers(&[203], 0.5)),
        ],
    );
    e.set_playback_settings(&PlaybackSettingsChange {
        fade_in: Some(2.0),
        ..PlaybackSettingsChange::default()
    })
    .unwrap();
    program(&mut e, 204, "Dimmer", 0.7);
    e.record_playback(&append()).unwrap();
    assert_eq!(numbers(&e, 1), [1.0, 2.0, 2.5, 3.0]);
    let new = cue_at(&e, 1, 3.0);
    assert_eq!(new.values, dimmers(&[204], 0.7));
    assert_eq!(new.fade_in, 2.0, "times from the PBK-12 settings");
    assert_eq!(
        (new.legend.as_str(), new.link, new.track),
        ("", Link::Wait, Track::Track)
    );
    program(&mut e, 205, "Dimmer", 0.7);
    e.record_playback(&append()).unwrap();
    assert_eq!(numbers(&e, 1), [1.0, 2.0, 2.5, 3.0, 4.0]);
}

#[test]
fn cue_03_append_records_the_programmer_in_the_mask() {
    let mut e = service();
    e.clear_selection();
    e.clear();
    common::select(&mut e, &[101]);
    e.set_programmer_value(101, "Dimmer", 0.6).unwrap();
    e.set_programmer_value(101, "Pan", 0.3).unwrap();
    e.record_playback(&PlaybackRecord {
        mask: Some(vec![P]),
        ..append()
    })
    .unwrap();
    let mut pan: Values = BTreeMap::new();
    pan.entry(101).or_default().insert("Pan".into(), 0.3.into());
    assert_eq!(cue_at(&e, 1, 4.0).values, pan);
}

#[test]
fn cue_03_append_with_nothing_in_the_mask_is_refused() {
    let mut e = service();
    let before = e.playback(1).unwrap().clone();
    let err = e.record_playback(&append()).unwrap_err();
    assert!(
        err.to_string().contains("Nothing in the programmer"),
        "{err}"
    );
    assert_eq!(*e.playback(1).unwrap(), before);
}

#[test]
fn cue_03_append_together_with_a_mode_is_refused() {
    let mut e = service();
    let before = e.playback(1).unwrap().clone();
    program(&mut e, 204, "Dimmer", 0.7);
    let record = PlaybackRecord {
        mode: Some(RecordMode::Merge),
        ..append()
    };
    assert!(e.record_playback(&record).is_err());
    assert_eq!(*e.playback(1).unwrap(), before);
}

#[test]
fn cue_03_append_on_a_chase_adds_a_step() {
    let mut e = service();
    e.change_playback(
        1,
        &PlaybackChange {
            kind: Some(Kind::Chase),
            ..PlaybackChange::default()
        },
    )
    .unwrap();
    program(&mut e, 204, "Dimmer", 1.0);
    e.record_playback(&append()).unwrap();
    assert_eq!(numbers(&e, 1), [1.0, 2.0, 3.0, 4.0]);
    assert_eq!(e.playback(1).unwrap().kind, Kind::Chase);
}

// ---- CUE-04: record to a cue number ----------------------------------------------------------

#[test]
fn cue_04_record_to_a_new_number_inserts_it_in_order() {
    let mut e = service();
    program(&mut e, 221, "Dimmer", 0.6);
    e.record_playback(&to_cue(2.5, None)).unwrap();
    assert_eq!(numbers(&e, 1), [1.0, 2.0, 2.5, 3.0]);
    assert_eq!(cue_at(&e, 1, 2.5).values, dimmers(&[221], 0.6));
    assert_eq!(cue_at(&e, 1, 2.5).link, Link::Wait);
}

#[test]
fn cue_04_record_to_a_number_makes_a_memory_a_list() {
    let mut e = common::church();
    program(&mut e, 201, "Dimmer", 0.5);
    let id = e.record_playback(&PlaybackRecord::at(1, 1)).unwrap();
    program(&mut e, 202, "Dimmer", 0.5);
    e.record_playback(&to_cue(5.0, None)).unwrap();
    assert_eq!(numbers(&e, id), [1.0, 5.0]);
    assert!(e.playback(id).unwrap().is_cue_list());
}

#[test]
fn cue_04_record_to_a_held_number_needs_a_mode() {
    let mut e = service();
    let before = e.playback(1).unwrap().clone();
    program(&mut e, 221, "Dimmer", 0.6);
    let err = e.record_playback(&to_cue(2.0, None)).unwrap_err();
    assert_eq!(err.to_string(), "Q2 exists: Replace, Merge or Update");
    assert_eq!(*e.playback(1).unwrap(), before);
}

#[test]
fn cue_04_replace_merge_and_update_act_on_that_cue_and_keep_its_fields() {
    let mut e = service();
    e.change_cue(
        1,
        q(2.0),
        &CueChange {
            legend: Some("Welcome".into()),
            fade_in: Some(3.0),
            link: Some(Link::After { offset: 1.0 }),
            ..CueChange::default()
        },
    )
    .unwrap();
    let q1 = cue_at(&e, 1, 1.0).clone();

    // Merge: 301 changes, 221 is added.
    e.clear_selection();
    e.clear();
    common::select(&mut e, &[301, 221]);
    e.set_programmer_value(301, "Dimmer", 0.5).unwrap();
    e.set_programmer_value(221, "Dimmer", 0.6).unwrap();
    e.record_playback(&to_cue(2.0, Some(RecordMode::Merge)))
        .unwrap();
    let q2 = cue_at(&e, 1, 2.0);
    assert_eq!(q2.values[&301]["Dimmer"], Stored::Value(Value::Level(0.5)));
    assert_eq!(q2.values[&302]["Dimmer"], Stored::Value(Value::Level(0.8)));
    assert_eq!(q2.values[&221]["Dimmer"], Stored::Value(Value::Level(0.6)));
    assert_eq!(
        (q2.legend.as_str(), q2.fade_in, q2.link),
        ("Welcome", 3.0, Link::After { offset: 1.0 })
    );
    assert_eq!(*cue_at(&e, 1, 1.0), q1, "other cues untouched");

    // Update: only what Q3's state holds (CUE-52 amended, owner 2026-10-21): the washes' Dimmer
    // and 221 tracked from Q2; 222 is in no cue, so it stays out.
    e.clear_selection();
    e.clear();
    common::select(&mut e, &[101, 221, 222]);
    e.set_programmer_value(101, "Dimmer", 0.2).unwrap();
    e.set_programmer_value(221, "Dimmer", 0.9).unwrap();
    e.set_programmer_value(222, "Dimmer", 0.4).unwrap();
    e.record_playback(&to_cue(3.0, Some(RecordMode::QuickMerge)))
        .unwrap();
    let q3 = cue_at(&e, 1, 3.0);
    assert_eq!(q3.values[&101]["Dimmer"], Stored::Value(Value::Level(0.2)));
    assert_eq!(q3.values[&221]["Dimmer"], Stored::Value(Value::Level(0.9)));
    assert!(!q3.values.contains_key(&222));

    // Replace: only the programmer's contents.
    e.record_playback(&to_cue(3.0, Some(RecordMode::Replace)))
        .unwrap();
    let mut both = dimmers(&[101], 0.2);
    both.extend(dimmers(&[221], 0.9));
    both.extend(dimmers(&[222], 0.4));
    assert_eq!(cue_at(&e, 1, 3.0).values, both);
    assert_eq!(numbers(&e, 1), [1.0, 2.0, 3.0]);
}

// ---- CUE-05: Insert --------------------------------------------------------------------------

#[test]
fn cue_05_numbers_halfway_cut_to_three_decimals() {
    let b = |a: f64, n: Option<f64>| CueNumber::between(q(a), n.map(q)).map(|c| c.as_f64());
    assert_eq!(b(2.0, Some(3.0)), Some(2.5));
    assert_eq!(b(2.0, Some(2.5)), Some(2.25));
    assert_eq!(b(2.0, Some(2.25)), Some(2.125));
    assert_eq!(b(2.0, Some(2.125)), Some(2.062));
    assert_eq!(b(2.001, Some(2.002)), None, "no room");
    assert_eq!(b(2.5, None), Some(3.0), "⌊current⌋ + 1 without a next cue");
    assert_eq!(b(7.0, None), Some(8.0));
    assert_eq!(b(9999.5, None), None, "past the last number");
}

#[test]
fn cue_05_insert_on_a_list_that_is_off_goes_after_the_last() {
    let mut e = service();
    program(&mut e, 221, "Dimmer", 0.6);
    e.record_playback(&PlaybackRecord {
        add: Some(AddCue::Insert),
        ..PlaybackRecord::at(1, 1)
    })
    .unwrap();
    assert_eq!(numbers(&e, 1), [1.0, 2.0, 3.0, 4.0]);
}

// ---- CUE-06: Replace / Merge / Update without a number ----------------------------------------

#[test]
fn cue_06_without_a_number_a_list_that_is_off_is_refused() {
    let mut e = service();
    let before = e.playback(1).unwrap().clone();
    program(&mut e, 221, "Dimmer", 0.6);
    for mode in [
        RecordMode::Replace,
        RecordMode::Merge,
        RecordMode::QuickMerge,
    ] {
        let err = e
            .record_playback(&PlaybackRecord {
                mode: Some(mode),
                ..PlaybackRecord::at(1, 1)
            })
            .unwrap_err();
        assert!(err.to_string().starts_with("Name a cue:"), "{err}");
    }
    assert_eq!(*e.playback(1).unwrap(), before);
}

// ---- CUE-07..09: editing cues -----------------------------------------------------------------

#[test]
fn cue_07_delete_removes_a_cue() {
    let mut e = service();
    e.delete_cue(1, q(2.0)).unwrap();
    assert_eq!(numbers(&e, 1), [1.0, 3.0]);
    assert!(e.delete_cue(1, q(9.0)).is_err(), "unknown cue");
    assert!(e.delete_cue(7, q(1.0)).is_err(), "unknown playback");
}

#[test]
fn cue_07_the_last_cue_cannot_be_deleted() {
    let mut e = common::church();
    e.add_playback(Playback::new(1, 1, 1, dimmers(&[201], 1.0)));
    let err = e.delete_cue(1, q(1.0)).unwrap_err();
    assert!(
        err.to_string().contains("Delete the playback instead"),
        "{err}"
    );
    assert_eq!(e.playback(1).unwrap().cues.len(), 1);
}

#[test]
fn cue_08_renumber_moves_a_cue_to_its_place() {
    let mut e = service();
    e.renumber_cue(1, q(1.0), q(3.5)).unwrap();
    assert_eq!(numbers(&e, 1), [2.0, 3.0, 3.5]);
    assert_eq!(
        cue_at(&e, 1, 3.5).values[&201]["Dimmer"],
        Stored::Value(Value::Level(0.45))
    );
    let before = e.playback(1).unwrap().clone();
    assert!(e.renumber_cue(1, q(2.0), q(3.0)).is_err(), "number in use");
    assert!(e.renumber_cue(1, q(9.0), q(4.0)).is_err(), "unknown cue");
    assert_eq!(*e.playback(1).unwrap(), before);
}

#[test]
fn cue_08_renumber_all_counts_one_two_three() {
    let mut e = common::church();
    list(
        &mut e,
        vec![
            cue(0.5, dimmers(&[201], 0.1)),
            cue(2.5, dimmers(&[202], 0.2)),
            cue(7.0, dimmers(&[203], 0.3)),
        ],
    );
    e.renumber_cues(1).unwrap();
    assert_eq!(numbers(&e, 1), [1.0, 2.0, 3.0]);
    assert_eq!(
        cue_at(&e, 1, 3.0).values,
        dimmers(&[203], 0.3),
        "order kept"
    );
}

#[test]
fn cue_08_copy_copies_a_cue_to_a_free_number() {
    let mut e = service();
    e.change_cue(
        1,
        q(1.0),
        &CueChange {
            legend: Some("Walk-in".into()),
            fade_in: Some(3.0),
            track: Some(Track::Block),
            ..CueChange::default()
        },
    )
    .unwrap();
    e.copy_cue(1, q(1.0), q(10.0)).unwrap();
    assert_eq!(numbers(&e, 1), [1.0, 2.0, 3.0, 10.0]);
    let (a, b) = (cue_at(&e, 1, 1.0).clone(), cue_at(&e, 1, 10.0).clone());
    assert_eq!(
        Cue {
            number: a.number,
            ..b
        },
        a,
        "everything but the number"
    );
    assert!(e.copy_cue(1, q(2.0), q(3.0)).is_err(), "number in use");
}

#[test]
fn cue_09_change_cue_sets_times_link_track_group_times_and_legend() {
    let mut e = service();
    let mut times = BTreeMap::new();
    times.insert(
        P,
        GroupTime {
            delay: 1.0,
            fade: 5.0,
        },
    );
    e.change_cue(
        1,
        q(3.0),
        &CueChange {
            legend: Some("Worship".into()),
            delay: Some(0.5),
            fade_in: Some(2.0),
            fade_out: Some(4.0),
            overlap: Some(50.0),
            link: Some(Link::With { offset: 2.0 }),
            track: Some(Track::CueOnly),
            group_times: Some(times.clone()),
        },
    )
    .unwrap();
    let c = cue_at(&e, 1, 3.0);
    assert_eq!(c.legend, "Worship");
    assert_eq!(
        (c.delay, c.fade_in, c.fade_out, c.overlap),
        (0.5, 2.0, 4.0, 50.0)
    );
    assert_eq!(c.link, Link::With { offset: 2.0 });
    assert_eq!(c.track, Track::CueOnly);
    assert_eq!(c.group_times, times);
}

#[test]
fn cue_09_out_of_range_is_refused_as_a_whole() {
    let mut e = service();
    let before = e.playback(1).unwrap().clone();
    let group = |delay: f32, fade: f32| {
        let mut t = BTreeMap::new();
        t.insert(I, GroupTime { delay, fade });
        Some(t)
    };
    let bad = [
        CueChange {
            delay: Some(-1.0),
            ..CueChange::default()
        },
        CueChange {
            fade_in: Some(601.0),
            ..CueChange::default()
        },
        CueChange {
            fade_out: Some(601.0),
            ..CueChange::default()
        },
        CueChange {
            overlap: Some(101.0),
            ..CueChange::default()
        },
        CueChange {
            link: Some(Link::After { offset: -1.0 }),
            ..CueChange::default()
        },
        CueChange {
            link: Some(Link::With { offset: 601.0 }),
            ..CueChange::default()
        },
        CueChange {
            group_times: group(0.0, 700.0),
            ..CueChange::default()
        },
        CueChange {
            group_times: group(-1.0, 1.0),
            ..CueChange::default()
        },
        CueChange {
            legend: Some("x".repeat(33)),
            ..CueChange::default()
        },
    ];
    for change in bad {
        let all = CueChange {
            fade_in: change.fade_in.or(Some(3.0)),
            ..change
        };
        assert!(e.change_cue(1, q(2.0), &all).is_err(), "{all:?}");
    }
    assert!(
        e.change_cue(1, q(9.0), &CueChange::default()).is_err(),
        "unknown cue"
    );
    assert_eq!(*e.playback(1).unwrap(), before);
}

// ---- CUE-15: the cue-list fader for new lists -------------------------------------------------

#[test]
fn cue_15_a_new_cue_list_takes_the_cue_list_fader_setting() {
    let mut e = common::church();
    e.set_playback_settings(&PlaybackSettingsChange {
        cue_list_fader: Some(ListFader::KillAtZero),
        ..PlaybackSettingsChange::default()
    })
    .unwrap();
    program(&mut e, 201, "Dimmer", 0.5);
    let id = e.record_playback(&PlaybackRecord::at(1, 1)).unwrap();
    program(&mut e, 202, "Dimmer", 0.5);
    e.record_playback(&append()).unwrap();
    assert_eq!(e.playback(id).unwrap().list.fader, ListFader::KillAtZero);
}

// ---- CUE-23: Copy carries the whole list ------------------------------------------------------

#[test]
fn cue_23_copy_carries_every_cue_and_the_list_options() {
    let mut e = service();
    let options = ListOptions {
        fader: ListFader::KillAtZero,
        looping: LoopAction::Loop,
        ..ListOptions::default()
    };
    e.change_playback(
        1,
        &PlaybackChange {
            list: Some(options.clone()),
            ..PlaybackChange::default()
        },
    )
    .unwrap();
    let copy = e.copy_playback(1, 2, 1).unwrap();
    let (a, b) = (e.playback(1).unwrap(), e.playback(copy).unwrap());
    assert_eq!(b.cues, a.cues);
    assert_eq!(b.list, options);
}

// ---- CUE-29: chase options are checked ---------------------------------------------------------

#[test]
fn cue_29_chase_options_out_of_range_are_refused() {
    let mut e = service();
    let before = e.playback(1).unwrap().clone();
    let bad = [
        ChaseOptions {
            bpm: 0.0,
            ..ChaseOptions::default()
        },
        ChaseOptions {
            bpm: 601.0,
            ..ChaseOptions::default()
        },
        ChaseOptions {
            crossfade: 101.0,
            ..ChaseOptions::default()
        },
        ChaseOptions {
            crossfade: -1.0,
            ..ChaseOptions::default()
        },
        ChaseOptions {
            speed_source: SpeedSource::Bpm(0),
            ..ChaseOptions::default()
        },
        ChaseOptions {
            speed_source: SpeedSource::Bpm(9),
            ..ChaseOptions::default()
        },
        ChaseOptions {
            speed_source: SpeedSource::Rate(5),
            ..ChaseOptions::default()
        },
    ];
    for chase in bad {
        let change = PlaybackChange {
            chase: Some(chase.clone()),
            ..PlaybackChange::default()
        };
        assert!(e.change_playback(1, &change).is_err(), "{chase:?}");
    }
    assert_eq!(*e.playback(1).unwrap(), before);
    let good = ChaseOptions {
        bpm: 128.0,
        crossfade: 50.0,
        direction: Direction::Bounce,
        speed_source: SpeedSource::Rate(2),
        ..ChaseOptions::default()
    };
    e.change_playback(
        1,
        &PlaybackChange {
            chase: Some(good.clone()),
            ..PlaybackChange::default()
        },
    )
    .unwrap();
    assert_eq!(e.playback(1).unwrap().chase, good);
}

// ---- CUE-35..37: speed masters ------------------------------------------------------------------

#[test]
fn cue_35_record_a_bpm_master_on_an_empty_handle() {
    let mut e = service();
    let id = e.record_master(1, 3, MasterKind::Bpm, 1).unwrap();
    let pb = e.playback(id).unwrap();
    assert_eq!((pb.page, pb.slot, pb.kind), (1, 3, Kind::BpmMaster));
    assert!(pb.cues.is_empty());
    assert_eq!(
        pb.master,
        Some(Master::Bpm(BpmMaster {
            number: 1,
            bpm: 120.0,
            multiplier: 1.0,
            fader: BpmFader::Unused,
            fader_max: 240.0,
        }))
    );
}

#[test]
fn cue_35_bpm_master_numbers_are_1_to_8_and_unique() {
    let mut e = service();
    assert!(e.record_master(1, 3, MasterKind::Bpm, 0).is_err());
    assert!(e.record_master(1, 3, MasterKind::Bpm, 9).is_err());
    assert!(
        e.record_master(1, 1, MasterKind::Bpm, 1).is_err(),
        "used handle"
    );
    e.record_master(1, 3, MasterKind::Bpm, 8).unwrap();
    assert!(
        e.record_master(1, 4, MasterKind::Bpm, 8).is_err(),
        "number in use"
    );
    e.record_master(1, 4, MasterKind::Rate, 1).unwrap();
    e.record_master(1, 5, MasterKind::Bpm, 1).unwrap();
    assert_eq!(e.playbacks().count(), 4);
}

#[test]
fn cue_35_master_options_are_checked() {
    let mut e = service();
    let id = e.record_master(1, 3, MasterKind::Bpm, 1).unwrap();
    let bpm = |change: fn(&mut BpmMaster)| {
        let mut m = BpmMaster {
            number: 1,
            bpm: 120.0,
            multiplier: 1.0,
            fader: BpmFader::Unused,
            fader_max: 240.0,
        };
        change(&mut m);
        PlaybackChange {
            master: Some(Master::Bpm(m)),
            ..PlaybackChange::default()
        }
    };
    for bad in [
        bpm(|m| m.bpm = 0.0),
        bpm(|m| m.bpm = 601.0),
        bpm(|m| m.multiplier = 3.0),
        bpm(|m| m.fader_max = 0.0),
        bpm(|m| m.number = 2), // the number is not an option
    ] {
        assert!(e.change_playback(id, &bad).is_err(), "{bad:?}");
    }
    let rate = PlaybackChange {
        master: Some(Master::Rate(RateMaster {
            number: 1,
            rate_scale: 200,
        })),
        ..PlaybackChange::default()
    };
    assert!(
        e.change_playback(id, &rate).is_err(),
        "a BPM master stays one"
    );
    e.change_playback(
        id,
        &bpm(|m| {
            m.fader = BpmFader::Multiplier;
            m.multiplier = 0.25;
        }),
    )
    .unwrap();
    let Some(Master::Bpm(m)) = &e.playback(id).unwrap().master else {
        panic!("not a BPM master");
    };
    assert_eq!((m.fader, m.multiplier), (BpmFader::Multiplier, 0.25));
}

#[test]
fn cue_36_rate_masters_are_1_to_4_at_200_percent() {
    let mut e = service();
    let id = e.record_master(1, 3, MasterKind::Rate, 4).unwrap();
    assert_eq!(e.playback(id).unwrap().kind, Kind::RateMaster);
    assert_eq!(
        e.playback(id).unwrap().master,
        Some(Master::Rate(RateMaster {
            number: 4,
            rate_scale: 200,
        }))
    );
    assert!(e.record_master(1, 4, MasterKind::Rate, 5).is_err());
    let scale = |rate_scale| PlaybackChange {
        master: Some(Master::Rate(RateMaster {
            number: 4,
            rate_scale,
        })),
        ..PlaybackChange::default()
    };
    assert!(e.change_playback(id, &scale(150)).is_err());
    e.change_playback(id, &scale(100)).unwrap();
}

#[test]
fn cue_37_recording_onto_a_master_is_refused() {
    let mut e = service();
    let id = e.record_master(1, 3, MasterKind::Bpm, 1).unwrap();
    program(&mut e, 201, "Dimmer", 1.0);
    for record in [
        PlaybackRecord {
            mode: Some(RecordMode::Merge),
            ..PlaybackRecord::at(1, 3)
        },
        PlaybackRecord {
            add: Some(AddCue::Append),
            ..PlaybackRecord::at(1, 3)
        },
        PlaybackRecord {
            cue: Some(q(1.0)),
            ..PlaybackRecord::at(1, 3)
        },
    ] {
        let err = e.record_playback(&record).unwrap_err();
        assert_eq!(err.to_string(), "Handle 1.3 holds BPM 1");
    }
    assert!(e.playback(id).unwrap().cues.is_empty());
}

#[test]
fn cue_37_a_copied_master_takes_the_next_free_number() {
    let mut e = common::church();
    let one = e.record_master(1, 1, MasterKind::Bpm, 1).unwrap();
    e.record_master(1, 2, MasterKind::Bpm, 2).unwrap();
    let copy = e.copy_playback(one, 2, 1).unwrap();
    let Some(Master::Bpm(m)) = &e.playback(copy).unwrap().master else {
        panic!("not a BPM master");
    };
    assert_eq!(m.number, 3);
    for (slot, n) in (2..=6).zip(4..=8) {
        e.record_master(2, slot, MasterKind::Bpm, n).unwrap();
    }
    assert!(e.copy_playback(one, 3, 1).is_err(), "no free number");
    e.move_playback(one, 3, 1).unwrap();
    e.delete_playback(one).unwrap();
}

// ---- CUE-38: settings and the show file ---------------------------------------------------------

#[test]
fn cue_38_new_settings_and_their_defaults() {
    let mut e = common::church();
    let s = e.settings();
    assert!(s.auto_connect);
    assert_eq!(s.cue_list_fader, ListFader::KillWithOff);
    assert_eq!(s.update_mode, UpdateMode::Forwards);
    assert_eq!(s.handle_buttons, HandleButtons::BestPractice);
    let mut custom = BTreeMap::new();
    custom.insert(ButtonKind::CueList, [Button::Go, Button::Back]);
    e.set_playback_settings(&PlaybackSettingsChange {
        auto_connect: Some(false),
        update_mode: Some(UpdateMode::CueOnly),
        handle_buttons: Some(HandleButtons::Custom(custom.clone())),
        ..PlaybackSettingsChange::default()
    })
    .unwrap();
    let s = e.settings();
    assert!(!s.auto_connect);
    assert_eq!(s.update_mode, UpdateMode::CueOnly);
    assert_eq!(s.handle_buttons, HandleButtons::Custom(custom));
}

#[test]
fn cue_38_a_custom_button_a_kind_cannot_have_is_refused() {
    let mut e = common::church();
    for (kind, pair) in [
        (ButtonKind::Memory, [Button::Go, Button::Flash]),
        (ButtonKind::Memory, [Button::Tap, Button::Flash]),
        (ButtonKind::CueList, [Button::Tap, Button::Flash]),
        (ButtonKind::BpmMaster, [Button::Flash, Button::Tap]),
        (ButtonKind::RateMaster, [Button::Tap, Button::None]),
    ] {
        let mut custom = BTreeMap::new();
        custom.insert(kind, pair);
        let change = PlaybackSettingsChange {
            handle_buttons: Some(HandleButtons::Custom(custom)),
            ..PlaybackSettingsChange::default()
        };
        assert!(
            e.set_playback_settings(&change).is_err(),
            "{kind:?} {pair:?}"
        );
    }
    assert_eq!(e.settings().handle_buttons, HandleButtons::BestPractice);
}

/// Service list with link, track and group times on Q2, a chase on 1.2 following BPM 2, BPM
/// master 2 on 1.3 and Rate master 1 on 1.4.
fn styled() -> Engine {
    let mut e = service();
    let mut times = BTreeMap::new();
    times.insert(
        P,
        GroupTime {
            delay: 1.0,
            fade: 5.0,
        },
    );
    e.change_cue(
        1,
        q(2.0),
        &CueChange {
            legend: Some("Welcome".into()),
            link: Some(Link::After { offset: 1.0 }),
            track: Some(Track::Block),
            group_times: Some(times),
            ..CueChange::default()
        },
    )
    .unwrap();
    e.add_playback(Playback {
        kind: Kind::Chase,
        chase: ChaseOptions {
            speed_source: SpeedSource::Bpm(2),
            ..ChaseOptions::default()
        },
        cues: vec![
            cue(1.0, dimmers(&[221], 1.0)),
            cue(2.0, dimmers(&[222], 1.0)),
        ],
        ..Playback::new(2, 1, 2, Values::new())
    });
    e.record_master(1, 3, MasterKind::Bpm, 2).unwrap();
    e.record_master(1, 4, MasterKind::Rate, 1).unwrap();
    e
}

#[test]
fn cue_38_schema_4_shape() {
    assert_eq!(SCHEMA_VERSION, 6); // GRM-13 adds only the GroupMaster kind, OUT-13 the output key
    let e = styled();
    let text = e.to_show(meta()).to_json();
    let v: Json = serde_json::from_str(&text).unwrap();
    assert_eq!(v["schema_version"], json!(6));
    keys_in_order(
        &text,
        "\"playbacks\": [",
        &[
            "group",
            "kind",
            "list",
            "chase",
            "cues",
            "delay",
            "fade_in",
            "fade_out",
            "overlap",
            "number",
            "legend",
            "link",
            "track",
            "group_times",
            "values",
        ],
    );
    let pbs = &v["playbacks"]["playbacks"];
    let one = &pbs[0];
    assert_eq!(one["kind"], json!("Cues"));
    assert_eq!(
        one["list"],
        json!({"tracking": true, "fader": "KillWithOff", "fire_first_cue": true, "loop": "StopOnFinal"})
    );
    assert_eq!(
        one["chase"],
        json!({"bpm": 120.0, "crossfade": 0.0, "direction": "Forward", "loop": "Loop", "speed_source": "Free"})
    );
    assert!(one.get("master").is_none(), "master only on masters");
    let q1 = &one["cues"][0];
    assert_eq!(q1["number"].as_f64(), Some(1.0));
    assert_eq!(q1["link"], json!({"type": "Wait"}));
    assert_eq!(q1["track"], json!("Track"));
    assert_eq!(q1["group_times"], json!({}));
    let q2 = &one["cues"][1];
    assert_eq!(q2["legend"], json!("Welcome"));
    assert_eq!(q2["link"], json!({"type": "After", "offset": 1.0}));
    assert_eq!(q2["track"], json!("Block"));
    assert_eq!(q2["group_times"], json!({"P": {"delay": 1.0, "fade": 5.0}}));
    assert_eq!(pbs[1]["kind"], json!("Chase"));
    assert_eq!(pbs[1]["chase"]["speed_source"], json!({"Bpm": 2}));
    assert_eq!(pbs[2]["kind"], json!("BpmMaster"));
    assert_eq!(
        pbs[2]["master"],
        json!({"number": 2, "bpm": 120.0, "multiplier": 1.0, "fader": "None", "fader_max": 240.0})
    );
    assert_eq!(pbs[2]["cues"], json!([]));
    assert_eq!(pbs[3]["kind"], json!("RateMaster"));
    assert_eq!(pbs[3]["master"], json!({"number": 1, "rate_scale": 200}));
    let s = &v["settings"];
    assert_eq!(s["auto_connect"], json!(true));
    assert_eq!(s["cue_list_fader"], json!("KillWithOff"));
    assert_eq!(s["update_mode"], json!("Forwards"));
    assert_eq!(s["handle_buttons"], json!("BestPractice"));
}

#[test]
fn cue_38_custom_handle_buttons_in_the_show() {
    let mut e = common::church();
    let mut custom = BTreeMap::new();
    custom.insert(ButtonKind::CueList, [Button::Go, Button::Back]);
    e.set_playback_settings(&PlaybackSettingsChange {
        handle_buttons: Some(HandleButtons::Custom(custom)),
        ..PlaybackSettingsChange::default()
    })
    .unwrap();
    assert_eq!(
        show_json(&e)["settings"]["handle_buttons"],
        json!({"Custom": {"CueList": ["Go", "Back"]}})
    );
}

#[test]
fn cue_38_round_trip_keeps_lists_chases_masters_and_settings() {
    let mut e = styled();
    e.set_playback_settings(&PlaybackSettingsChange {
        auto_connect: Some(false),
        cue_list_fader: Some(ListFader::KillAtZero),
        update_mode: Some(UpdateMode::Both),
        handle_buttons: Some(HandleButtons::Titan),
        ..PlaybackSettingsChange::default()
    })
    .unwrap();
    let show = e.to_show(meta());
    let (loaded, warnings) = Engine::from_show(&show);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(loaded.to_show(meta()), show);
    let (parsed, migrations) = Show::from_json(&show.to_json()).unwrap();
    assert!(migrations.is_empty(), "{migrations:?}");
    assert_eq!(parsed, show);
    assert_eq!(
        loaded.playback(1).unwrap().cues,
        e.playback(1).unwrap().cues
    );
}

#[test]
fn cue_38_a_schema_3_show_is_migrated() {
    let file = std::fs::read_to_string(V3_SHOW).unwrap();
    let mut v: Json = serde_json::from_str(&file).unwrap();
    assert_eq!(v["schema_version"], json!(3));
    // A schema 3 playback with two cues (possible, though nothing recorded one).
    let second = json!({"delay": 0.0, "fade_in": 2.0, "fade_out": 0.0, "overlap": 100.0,
        "values": {"101": {"Dimmer": 0.5}}});
    v["playbacks"]["playbacks"][0]["cues"]
        .as_array_mut()
        .unwrap()
        .push(second);
    let (show, migrations) = Show::from_json(&v.to_string()).unwrap();
    assert_eq!(migrations.len(), 3, "{migrations:?}");
    assert!(migrations[0].starts_with("3 -> 4"), "{migrations:?}");
    assert!(migrations[1].starts_with("4 -> 5"), "{migrations:?}"); // GRM-13
    assert!(migrations[2].starts_with("5 -> 6"), "{migrations:?}"); // OUT-27
    let (e, warnings) = Engine::from_show(&show);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(numbers(&e, 1), [1.0, 2.0]);
    for pb in e.playbacks() {
        assert_eq!(pb.kind, Kind::Cues);
        assert_eq!((pb.list.clone(), pb.chase.clone()), Default::default());
        for c in &pb.cues {
            assert_eq!(
                (c.legend.as_str(), c.link, c.track),
                ("", Link::Wait, Track::Track)
            );
            assert!(c.group_times.is_empty());
        }
    }
    assert_eq!(cue_at(&e, 1, 2.0).fade_in, 2.0);
    let s = e.settings();
    assert!(s.auto_connect);
    assert_eq!(s.handle_buttons, HandleButtons::BestPractice);
}

// ---- CUE-39: lenient load ---------------------------------------------------------------------

#[test]
fn cue_39_a_bad_or_repeated_cue_number_is_left_out() {
    let mut v = show_json(&service());
    let cues = v["playbacks"]["playbacks"][0]["cues"]
        .as_array_mut()
        .unwrap();
    let template = cues[0].clone();
    for n in [json!(1.0), json!(2.0005), json!(0.0), json!(10000.0)] {
        let mut c = template.clone();
        c["number"] = n;
        cues.push(c);
    }
    let (e, warnings) = load(&v);
    assert_eq!(warnings.len(), 4, "{warnings:?}");
    assert_eq!(numbers(&e, 1), [1.0, 2.0, 3.0]);
}

#[test]
fn cue_39_options_out_of_range_get_their_default() {
    let mut v = show_json(&styled());
    let one = &mut v["playbacks"]["playbacks"][0];
    one["cues"][1]["link"] = json!({"type": "After", "offset": 700.0});
    one["cues"][1]["group_times"] = json!({"P": {"delay": 1.0, "fade": 700.0}});
    one["chase"]["bpm"] = json!(0.0);
    let (e, warnings) = load(&v);
    assert_eq!(warnings.len(), 3, "{warnings:?}");
    let q2 = cue_at(&e, 1, 2.0);
    assert_eq!(q2.link, Link::Wait);
    assert!(q2.group_times.is_empty());
    assert_eq!(e.playback(1).unwrap().chase.bpm, 120.0);
}

#[test]
fn cue_39_a_bad_or_repeated_master_number_is_left_out() {
    let mut v = show_json(&styled());
    // 1.4 becomes a second BPM 2; a third master with BPM 9 on 1.5.
    let mut bpm = v["playbacks"]["playbacks"][2].clone();
    v["playbacks"]["playbacks"][3] = {
        let mut twin = bpm.clone();
        twin["id"] = json!(4);
        twin["slot"] = json!(4);
        twin
    };
    bpm["id"] = json!(5);
    bpm["slot"] = json!(5);
    bpm["master"]["number"] = json!(9);
    v["playbacks"]["playbacks"]
        .as_array_mut()
        .unwrap()
        .push(bpm);
    v["playbacks"]["next_id"] = json!(6);
    let (e, warnings) = load(&v);
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(e.playback(3).is_some());
    assert!(e.playback(4).is_none() && e.playback(5).is_none());
}

#[test]
fn cue_39_a_master_with_cues_loads_without_them() {
    let mut v = show_json(&styled());
    v["playbacks"]["playbacks"][2]["cues"] = v["playbacks"]["playbacks"][0]["cues"].clone();
    let (e, warnings) = load(&v);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(e.playback(3).unwrap().cues.is_empty());
}

#[test]
fn cue_39_a_custom_button_a_kind_cannot_have_falls_back_to_best_practice() {
    let mut v = show_json(&common::church());
    v["settings"]["handle_buttons"] = json!({"Custom": {"Memory": ["Tap", "Flash"]}});
    let (e, warnings) = load(&v);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    let mut fixed = BTreeMap::new();
    fixed.insert(ButtonKind::Memory, [Button::Swop, Button::Flash]);
    assert_eq!(e.settings().handle_buttons, HandleButtons::Custom(fixed));
}
