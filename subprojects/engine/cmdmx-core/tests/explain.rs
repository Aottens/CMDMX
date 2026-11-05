//! Why a value is what it is (SEE-03): `specs/modules/engine-merge.md` ENG-25.
//!
//! API these tests fix (`cmdmx_core::engine`):
//! - `Engine::explain(&self, fixture: FixtureId) -> Option<Vec<Explanation>>`: `None` for an
//!   unknown fixture, else one entry per logical attribute of the fixture (cells after the
//!   fixture, `cell: Some(n)`), for the time of the last tick. It reads, changes nothing.
//! - `Explanation { name: String, cell: Option<u16>, value: Value, base: Base,
//!   others: Vec<Proposal>, modifiers: Vec<Modifier> }`; `value` is what the tick writes for the
//!   attribute (before a virtual dimmer scales it).
//! - `Base { source: Source, value: Value, tracked: bool, palette: Option<PaletteId> }`: the
//!   merge winner and its value before any modifier.
//! - `Source::{Programmer, Playback { id: PlaybackId, cue: Option<CueNumber> }, Held, Default}`:
//!   `cue` is the list's current cue (`None` for a memory); `tracked` is true when the value is
//!   not hard in that cue (CUE-50).
//! - `Proposal { source: Source, value: Value }`: an HTP source that lost.
//! - `Modifier::{Effect, Master { playback: PlaybackId, mode: GroupMode, level: f32, enabled:
//!   bool }, GrandMaster(f32), Dbo, Locate, Highlight, AutoMix}`: every group master that holds
//!   the fixture is listed (its level is what it applies, the saved one while disabled); the
//!   Grand Master only below full; DBO while on. All derive `Debug, Clone, PartialEq`.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

mod common;

use std::collections::BTreeMap;

use cmdmx_core::effect::EffectApply;
use cmdmx_core::engine::{Base, Engine, Explanation, Modifier, Proposal, Source};
use cmdmx_core::patch::FixtureId;
use cmdmx_core::playback::{
    Cue, CueNumber, GroupMaster, GroupMode, ListAction, Master, Playback, PlaybackChange, Values,
};
use cmdmx_core::programmer::Item;
use cmdmx_core::value::Value;

use common::select;

const SPOTS: [u32; 6] = [301, 302, 303, 304, 305, 306];

fn q(n: f64) -> CueNumber {
    CueNumber::new(n).unwrap()
}

fn levels(entries: &[(u32, &str, f32)]) -> Values {
    let mut v: Values = BTreeMap::new();
    for (id, name, level) in entries {
        v.entry((*id).into())
            .or_default()
            .insert((*name).to_string(), (*level).into());
    }
    v
}

fn record_group(e: &mut Engine, group: u32, ids: &[u32]) {
    e.clear_selection();
    select(e, ids);
    e.record_group(group).unwrap();
    e.clear_selection();
}

/// The church rig with playback 1 on 1.1 putting par 201 and spot 301 at 80 %, fader full.
fn rig() -> Engine {
    let mut e = common::church();
    e.add_playback(Playback::new(
        1,
        1,
        1,
        levels(&[(201, "Dimmer", 0.8), (301, "Dimmer", 0.8)]),
    ));
    e.set_fader(1, 1.0, 0.0).unwrap();
    e
}

fn attribute(e: &Engine, id: FixtureId, name: &str) -> Explanation {
    let all = e.explain(id).expect("a patched fixture");
    all.into_iter()
        .find(|x| x.name == name && x.cell.is_none())
        .unwrap_or_else(|| panic!("no {name} on {id}"))
}

/// ENG-25's test: `explain` after a tick equals what the next tick writes, for every attribute
/// of the fixture that has a channel of its own.
fn assert_explains_frame(e: &mut Engine, id: FixtureId, t: f64) {
    let explained = e.explain(id).expect("a patched fixture");
    e.tick(t);
    let fixture = e.fixture(id).unwrap().clone();
    let mode = e.fixture_mode(id).unwrap().clone();
    let frame = e.frame(fixture.universe).unwrap();
    let mut checked = 0;
    for x in explained.iter().filter(|x| x.cell.is_none()) {
        let (Some(def), Value::Level(level)) = (mode.attribute(&x.name), x.value) else {
            continue;
        };
        let (coarse, fine) = def.to_dmx(level);
        let at = |ch: u16| frame.channel(fixture.address + ch - 1).unwrap();
        assert_eq!(at(def.channel), coarse, "{id} {}: {x:?}", x.name);
        if let (Some(ch), Some(fine)) = (def.fine_channel, fine) {
            assert_eq!(at(ch), fine, "{id} {} fine: {x:?}", x.name);
        }
        checked += 1;
    }
    assert!(checked > 0, "nothing on {id} to check");
}

#[test]
fn eng_25_an_unknown_fixture_has_no_explanation() {
    let e = rig();
    assert_eq!(e.explain(999), None);
}

#[test]
fn eng_25_a_programmer_value() {
    let mut e = rig();
    select(&mut e, &[202]);
    e.set_programmer_value(202, "Dimmer", 0.6).unwrap();
    e.tick(0.0);
    let x = attribute(&e, 202, "Dimmer");
    assert_eq!(x.base.source, Source::Programmer);
    assert_eq!(x.base.value, Value::Level(0.6));
    assert_eq!(x.value, Value::Level(0.6));
    assert!(x.modifiers.is_empty());
    assert_explains_frame(&mut e, 202, 0.025);
}

#[test]
fn eng_25_a_memory() {
    let mut e = rig();
    e.tick(0.0);
    let x = attribute(&e, 201, "Dimmer");
    assert_eq!(
        x.base,
        Base {
            source: Source::Playback { id: 1, cue: None },
            value: Value::Level(0.8),
            tracked: false,
            palette: None,
        }
    );
    assert!(x.others.is_empty());
    assert_explains_frame(&mut e, 201, 0.025);
}

#[test]
fn eng_25_nobody_proposing_is_default_for_htp_and_held_for_ltp() {
    let mut e = rig();
    e.tick(0.0);
    let dimmer = attribute(&e, 202, "Dimmer");
    assert_eq!(dimmer.base.source, Source::Default);
    assert_eq!(dimmer.value, Value::Level(0.0));
    let tilt = attribute(&e, 101, "Tilt");
    assert_eq!(tilt.base.source, Source::Held);
    assert_explains_frame(&mut e, 202, 0.025);
    assert_explains_frame(&mut e, 101, 0.05);
}

#[test]
fn eng_25_a_running_list_with_a_tracked_value() {
    let mut e = common::church();
    e.add_playback(Playback {
        cues: vec![
            Cue {
                number: q(1.0),
                ..Cue::new(levels(&[(201, "Dimmer", 1.0)]))
            },
            Cue {
                number: q(2.0),
                ..Cue::new(levels(&[(202, "Dimmer", 1.0)]))
            },
        ],
        ..Playback::new(1, 1, 1, Values::new())
    });
    e.set_fader(1, 1.0, 0.0).unwrap();
    e.tick(0.0);
    e.cuelist(Some(1), ListAction::Goto(q(2.0)), 1.0).unwrap();
    e.tick(1.0);
    let tracked = attribute(&e, 201, "Dimmer");
    assert_eq!(
        tracked.base.source,
        Source::Playback {
            id: 1,
            cue: Some(q(2.0))
        }
    );
    assert!(tracked.base.tracked, "201 is hard in Q1: {tracked:?}");
    let hard = attribute(&e, 202, "Dimmer");
    assert!(!hard.base.tracked, "202 is hard in Q2: {hard:?}");
    assert_explains_frame(&mut e, 201, 1.025);
    assert_explains_frame(&mut e, 202, 1.05);
}

#[test]
fn eng_25_two_htp_playbacks_name_the_one_that_lost() {
    let mut e = rig();
    e.add_playback(Playback::new(2, 1, 2, levels(&[(201, "Dimmer", 0.4)])));
    e.set_fader(2, 1.0, 0.0).unwrap();
    e.tick(0.0);
    let x = attribute(&e, 201, "Dimmer");
    assert_eq!(x.base.source, Source::Playback { id: 1, cue: None });
    assert_eq!(
        x.others,
        vec![Proposal {
            source: Source::Playback { id: 2, cue: None },
            value: Value::Level(0.4),
        }]
    );
    assert_explains_frame(&mut e, 201, 0.025);
}

#[test]
fn eng_25_a_scale_master_at_0_holds_it_down() {
    let mut e = rig();
    record_group(&mut e, 6, &SPOTS);
    let id = e.record_group_master(1, 8, 6).unwrap();
    e.set_fader(id, 0.0, 0.0).unwrap();
    e.tick(0.0);
    let x = attribute(&e, 301, "Dimmer");
    assert_eq!(x.base.value, Value::Level(0.8));
    assert_eq!(x.value, Value::Level(0.0));
    assert_eq!(
        x.modifiers,
        vec![Modifier::Master {
            playback: id,
            mode: GroupMode::Scale,
            level: 0.0,
            enabled: true,
        }]
    );
    assert_eq!(
        attribute(&e, 201, "Dimmer").modifiers,
        vec![],
        "201 is free"
    );
    assert_explains_frame(&mut e, 301, 0.025);
}

#[test]
fn eng_25_a_disabled_master_applies_its_saved_level() {
    let mut e = rig();
    record_group(&mut e, 6, &SPOTS);
    let id = e.record_group_master(1, 8, 6).unwrap();
    e.set_fader(id, 0.5, 0.0).unwrap();
    e.tick(0.0);
    let Some(Master::Group(m)) = e.playback(id).unwrap().master.clone() else {
        panic!("a group master");
    };
    e.change_playback(
        id,
        &PlaybackChange {
            master: Some(Master::Group(GroupMaster {
                enabled: false,
                ..m
            })),
            ..PlaybackChange::default()
        },
    )
    .unwrap();
    e.set_fader(id, 1.0, 0.025).unwrap();
    e.tick(0.025);
    let x = attribute(&e, 301, "Dimmer");
    assert_eq!(
        x.modifiers,
        vec![Modifier::Master {
            playback: id,
            mode: GroupMode::Scale,
            level: 0.5,
            enabled: false,
        }]
    );
    assert_explains_frame(&mut e, 301, 0.05);
}

#[test]
fn eng_25_an_effect() {
    let mut e = common::church();
    let mut values = Values::new();
    let at = values.entry(Item::fixture(101)).or_default();
    at.insert("Pan".into(), 0.5.into());
    at.insert("Tilt".into(), 0.5.into());
    e.add_playback(Playback::new(1, 1, 1, values));
    e.set_fader(1, 1.0, 0.0).unwrap();
    select(&mut e, &[101]);
    e.effect_apply(&EffectApply::Look("Circle".into())).unwrap();
    e.tick(0.0);
    e.tick(0.75);
    let x = attribute(&e, 101, "Tilt");
    assert_eq!(x.base.source, Source::Playback { id: 1, cue: None });
    assert_eq!(x.base.value, Value::Level(0.5));
    assert!(x.modifiers.contains(&Modifier::Effect), "{x:?}");
    assert_explains_frame(&mut e, 101, 0.75);
}

#[test]
fn eng_25_the_grand_master_at_half() {
    let mut e = rig();
    e.set_grand_master(0.5).unwrap();
    e.tick(0.0);
    let x = attribute(&e, 201, "Dimmer");
    assert_eq!(x.modifiers, vec![Modifier::GrandMaster(0.5)]);
    assert_eq!(x.value, Value::Level(0.4));
    assert_explains_frame(&mut e, 201, 0.025);
}

#[test]
fn eng_25_dbo() {
    let mut e = rig();
    e.set_dbo(true);
    e.tick(0.0);
    let x = attribute(&e, 201, "Dimmer");
    assert_eq!(x.modifiers, vec![Modifier::Dbo]);
    assert_eq!(x.value, Value::Level(0.0));
    assert_explains_frame(&mut e, 201, 0.025);
}

#[test]
fn eng_25_explain_changes_nothing() {
    let mut e = rig();
    e.tick(0.0);
    let before = e.frame(1).unwrap().slots().to_vec();
    for id in [101, 201, 301] {
        e.explain(id);
    }
    e.tick(0.0);
    assert_eq!(e.frame(1).unwrap().slots().to_vec(), before);
}
