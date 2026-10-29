//! Programmer effects in the merge (FXB-02): `specs/modules/effects.md` FX-09..11, 14..20, 31 and
//! `engine-merge.md` ENG-14..19. The shape functions themselves are `tests/effects.rs` (FXB-01);
//! a frame-level run is `tests/scenarios/FX-14-*.json`.
//!
//! API under test (`cmdmx_core::effect` unless noted):
//! - `EntryId` (u32, never reused) and `Entry { id, look: Option<String>, shape, attributes:
//!   Vec<String>, params: Params, members: Vec<Item>, positions: Vec<u32>, count: u32, seed: u64,
//!   colours: Vec<TargetColour> }`: a programmer effect entry. `attributes` is `["Dimmer"]` (a
//!   wave's attribute), `["Pan", "Tilt"]` (a path) or `["Colour"]`; `positions`/`count` are
//!   FX-07's *k* per member and *N*; `colours` is the `ColourMix` list (empty for other shapes).
//! - `EffectApply::{Look(String), Shape { shape, attribute: Option<String> }}` (serde snake_case:
//!   `{"look": "Circle"}`, `{"shape": {"shape": {"Wave": "Sine"}, "attribute": "Dimmer"}}`; a wave
//!   without an attribute means Dimmer). A look is found by its `LOOKS` name.
//! - `EffectSet::{Size, Top, Bpm, Beats, Spread, Phase, Width, Crossfade (f32), SpeedSource,
//!   Direction, Mode, Wings, Groups, Blocks (u8), Colours(Vec<TargetColour>)}` (serde snake_case,
//!   `{"wings": 2}`), with `EffectSet::name() -> &'static str` ("wings") for labels and keys.
//! - `Engine::effect_apply(&EffectApply) -> Result<EntryId>` (FX-14, 31), `effect_set(EffectSet)`
//!   on the current entry (FX-16), `effect_select(EntryId)` (FX-18), `effect_stop(Option<
//!   AttributeGroup>)` on the targets (FX-19), `effect_restart()` (FX-11), `effect_tap(at)`
//!   (FX-10), `set_effect_wheels(bool)` / `effect_wheels() -> bool` (FX-17), `effects() ->
//!   &[Entry]`, `current_effect() -> Option<EntryId>`, `effect_theta(EntryId) -> Option<f64>`
//!   (the clock, θ_total in cycles; live state, not in the edit state, FX-20).
//! - A new entry's clock is 0 and its first tick does not advance it; each later tick advances it
//!   by Δt · rate (FX-09).

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

mod common;

use cmdmx_core::attribute::AttributeGroup;
use cmdmx_core::colour::TargetColour;
use cmdmx_core::effect::{
    Direction, EffectApply, EffectSet, EntryId, Mode, Shape, SpeedSource, Wave,
};
use cmdmx_core::engine::Engine;
use cmdmx_core::patch::{Fixture, FixtureId};
use cmdmx_core::playback::{MasterKind, Playback, Values};
use cmdmx_core::profile::Profile;
use cmdmx_core::programmer::{Item, Resolution::Coarse};
use cmdmx_core::scenario::{build_engine, Scenario};
use cmdmx_core::undo::History;
use cmdmx_core::value::Value;
use common::{church, dmx, select};

const RGB_PAR: &str = r#"{ "manufacturer": "T", "model": "RGB Par", "source": "channel-list", "modes": [ {
  "name": "4ch", "footprint": 4,
  "colour": { "system": "RGB", "components": [ { "channel": 2 }, { "channel": 3 }, { "channel": 4 } ] },
  "attributes": [ { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 } ] } ] }"#;

const SPOT: &str = r#"{ "manufacturer": "T", "model": "Spot", "source": "channel-list", "modes": [ {
  "name": "2ch", "footprint": 2, "pan_degrees": 540, "tilt_degrees": 270,
  "attributes": [
    { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 1 },
    { "name": "Tilt", "group": "P", "kind": "Ltp", "channel": 2 } ] } ] }"#;

fn patch(e: &mut Engine, id: FixtureId, profile: &str, address: u16) {
    e.patch(Fixture {
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

/// The church rig (washes 101–104 at 1, 6, 11, 16: Dimmer, Pan 16-bit, Tilt, Zoom; one-channel
/// pars 201–208 at 21–28) plus RGB pars 251 @41, 252 @45 (Dimmer, R, G, B) and a spot 311 @49
/// (Pan, Tilt; 540°/270°).
fn rig() -> Engine {
    let mut e = church();
    e.add_profile("rgb", Profile::from_json(RGB_PAR).unwrap())
        .unwrap();
    e.add_profile("spot", Profile::from_json(SPOT).unwrap())
        .unwrap();
    patch(&mut e, 251, "rgb", 41);
    patch(&mut e, 252, "rgb", 45);
    patch(&mut e, 311, "spot", 49);
    e
}

fn look(name: &str) -> EffectApply {
    EffectApply::Look(name.into())
}

fn apply(e: &mut Engine, ids: &[FixtureId], name: &str) -> EntryId {
    select(e, ids);
    e.effect_apply(&look(name)).unwrap()
}

fn items(ids: &[FixtureId]) -> Vec<Item> {
    ids.iter().map(|&id| Item::fixture(id)).collect()
}

fn entry(e: &Engine, id: EntryId) -> &cmdmx_core::effect::Entry {
    e.effects().iter().find(|x| x.id == id).unwrap()
}

fn theta(e: &Engine, id: EntryId) -> f64 {
    e.effect_theta(id).unwrap()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// DMX at `address` within ±1 of `expected` (float rounding at x.5).
fn assert_dmx(e: &Engine, address: u16, expected: u8) {
    let got = dmx(e, address);
    assert!(
        got.abs_diff(expected) <= 1,
        "address {address}: expected {expected}, got {got}"
    );
}

/// 16-bit Pan of a wash at `address` (coarse at +1, fine at +2) as 0..1.
fn pan16(e: &Engine, address: u16) -> f32 {
    let coarse = u16::from(dmx(e, address + 1));
    let fine = u16::from(dmx(e, address + 2));
    f32::from(coarse << 8 | fine) / 65535.0
}

fn rgb(r: f32, g: f32, b: f32) -> TargetColour {
    TargetColour { r, g, b }
}

fn colour_close(a: TargetColour, b: TargetColour) -> bool {
    (a.r - b.r).abs() < 1e-3 && (a.g - b.g).abs() < 1e-3 && (a.b - b.b).abs() < 1e-3
}

// ---------- FX-14, FX-31: apply ----------

#[test]
fn fx_14_a_look_runs_at_once_on_the_targets_that_have_its_attribute() {
    let mut e = rig();
    // 101 (wash) and 311 (spot) have Pan/Tilt; 201 does not and is skipped.
    select(&mut e, &[201, 101, 311]);
    let id = e.effect_apply(&look("Circle")).unwrap();
    let x = entry(&e, id);
    assert_eq!(x.look.as_deref(), Some("Circle"));
    assert_eq!(x.shape, Shape::Path(cmdmx_core::effect::Path::Circle));
    assert_eq!(x.attributes, ["Pan", "Tilt"]);
    assert_eq!(x.members, items(&[101, 311]), "target order, without 201");
    assert_eq!(x.params.mode, Mode::Relative);
    assert!((x.params.size - 0.2).abs() < 1e-6, "the look's defaults");
    assert_eq!(e.current_effect(), Some(id));

    // Dimmer chase on four pars: width 100 %/N, and it runs from the first tick.
    let mut e = rig();
    let id = apply(&mut e, &[201, 202, 203, 204], "Dimmer chase");
    let x = entry(&e, id);
    assert_eq!(x.attributes, ["Dimmer"]);
    assert_eq!((x.positions.clone(), x.count), (vec![0, 1, 2, 3], 4));
    assert!((x.params.width - 0.25).abs() < 1e-6);
    e.tick(0.0);
    assert_eq!(
        [dmx(&e, 21), dmx(&e, 22), dmx(&e, 23), dmx(&e, 24)],
        [255, 0, 0, 0]
    );
    e.tick(0.15); // 120 BPM: θ = 0.3, the second par leads
    assert_eq!(
        [dmx(&e, 21), dmx(&e, 22), dmx(&e, 23), dmx(&e, 24)],
        [0, 255, 0, 0]
    );
}

#[test]
fn fx_14_fx_31_a_look_no_target_can_run_is_refused() {
    let mut e = rig();
    select(&mut e, &[201, 202]);
    let err = e.effect_apply(&look("Circle")).unwrap_err();
    assert!(err.to_string().contains("Pan/Tilt"), "{err}");
    assert!(e.effects().is_empty());
    assert!(e.effect_apply(&look("No such look")).is_err());
    e.clear_selection();
    assert!(e.effect_apply(&look("Dimmer wave")).is_err(), "no targets");
    assert!(e.effects().is_empty());
}

#[test]
fn fx_14_one_programmer_effect_per_fixture_and_attribute() {
    let mut e = rig();
    let a = apply(&mut e, &[201, 202, 203, 204], "Dimmer wave");
    let b = apply(&mut e, &[203, 204], "Breathe");
    assert_eq!(entry(&e, a).members, items(&[201, 202]));
    assert_eq!(entry(&e, b).members, items(&[203, 204]));
    assert_eq!(
        (entry(&e, a).positions.clone(), entry(&e, a).count),
        (vec![0, 1], 2),
        "positions follow the members"
    );
    // An entry with no members left goes away.
    let c = apply(&mut e, &[201, 202], "Sparkle");
    let ids: Vec<EntryId> = e.effects().iter().map(|x| x.id).collect();
    assert_eq!(ids, [b, c]);
    // Another attribute on the same fixtures is another effect.
    let d = apply(&mut e, &[101], "Zoom pulse");
    let w = apply(&mut e, &[101], "Breathe");
    assert_eq!(entry(&e, d).attributes, ["Zoom"]);
    assert_eq!(entry(&e, w).members, items(&[101]));
    assert_eq!(e.effects().len(), 4);
}

#[test]
fn fx_14_a_shape_with_an_attribute_makes_an_entry() {
    let mut e = rig();
    select(&mut e, &[101, 102]);
    let id = e
        .effect_apply(&EffectApply::Shape {
            shape: Shape::Wave(Wave::Triangle),
            attribute: Some("Zoom".into()),
        })
        .unwrap();
    let x = entry(&e, id);
    assert_eq!(x.look, None);
    assert_eq!(x.attributes, ["Zoom"]);
    assert_eq!(x.params.mode, Mode::Relative, "an LTP wave is relative");
    // A wave without an attribute is on Dimmer, absolute there (FX-05).
    let id = e
        .effect_apply(&EffectApply::Shape {
            shape: Shape::Wave(Wave::Sine),
            attribute: None,
        })
        .unwrap();
    assert_eq!(entry(&e, id).attributes, ["Dimmer"]);
    assert_eq!(entry(&e, id).params.mode, Mode::Absolute);
    let refused = e.effect_apply(&EffectApply::Shape {
        shape: Shape::Wave(Wave::Sine),
        attribute: Some("Gobo".into()),
    });
    assert!(refused.is_err());
}

// ---------- FX-15, ENG-14..19: the merge ----------

#[test]
fn fx_15_eng_15_a_programmer_circle_runs_around_the_cue_position() {
    let mut e = rig();
    let mut values = Values::new();
    let at = values.entry(Item::fixture(101)).or_default();
    at.insert("Pan".into(), 0.5.into());
    at.insert("Tilt".into(), 0.5.into());
    e.add_playback(Playback::new(1, 1, 1, values));
    e.set_fader(1, 1.0, 0.0).unwrap();
    apply(&mut e, &[101], "Circle"); // size 0.2, 20 BPM
    e.tick(0.0); // θ = 0: (x, y) = (0, ½)
    assert!((pan16(&e, 1) - 0.5).abs() < 0.002);
    assert_dmx(&e, 4, 153); // Tilt 0.5 + 0.2 · 0.5
    e.tick(0.75); // θ = ¼: (½, 0)
    assert!((pan16(&e, 1) - 0.6).abs() < 0.002, "{}", pan16(&e, 1));
    assert_dmx(&e, 4, 128);
    assert_eq!(e.programmer().value(101, "Pan"), None, "no value of its own");
}

#[test]
fn fx_02_a_circle_is_round_on_a_spot_with_known_degrees() {
    let mut e = rig();
    e.set_programmer_value(311, "Pan", 0.5).unwrap();
    e.set_programmer_value(311, "Tilt", 0.5).unwrap();
    apply(&mut e, &[311], "Circle");
    e.tick(0.0);
    assert_dmx(&e, 49, 128);
    assert_dmx(&e, 50, 179); // 0.5 + 0.2 · ½ · 540/270 = 0.7
}

#[test]
fn eng_16_the_effect_is_clamped_before_the_grand_master() {
    let mut e = rig();
    select(&mut e, &[201]);
    e.set_programmer_value(201, "Dimmer", 0.9).unwrap();
    e.effect_apply(&EffectApply::Shape {
        shape: Shape::Wave(Wave::Sine),
        attribute: Some("Dimmer".into()),
    })
    .unwrap();
    e.effect_set(EffectSet::Mode(Mode::Relative)).unwrap();
    e.effect_set(EffectSet::Size(1.0)).unwrap();
    e.effect_set(EffectSet::Bpm(60.0)).unwrap();
    e.set_grand_master(0.5).unwrap();
    e.tick(0.0); // w = 0: 0.9 − 0.5 = 0.4, GM → 0.2
    assert_dmx(&e, 21, 51);
    e.tick(0.5); // w = 1: 1.4 clamped to 1.0, GM → 0.5 (not 0.7)
    assert_dmx(&e, 21, 128);
}

#[test]
fn eng_14_an_absolute_dimmer_effect_replaces_the_base_and_is_mastered() {
    let mut e = rig();
    e.set_programmer_value(201, "Dimmer", 0.2).unwrap();
    let id = apply(&mut e, &[201], "Breathe"); // Abs, size 0.5, top 1
    e.effect_set(EffectSet::Bpm(60.0)).unwrap();
    e.tick(0.0);
    assert_dmx(&e, 21, 128); // top − size = 0.5 whatever the base
    e.tick(0.5);
    assert_dmx(&e, 21, 255);
    e.set_grand_master(0.5).unwrap();
    e.tick(1.0);
    assert_dmx(&e, 21, 64);
    assert!(close(theta(&e, id), 1.0));
}

#[test]
fn eng_18_when_the_effect_stops_the_attribute_returns_to_its_base() {
    let mut e = rig();
    apply(&mut e, &[101], "Circle");
    e.tick(0.0);
    e.tick(0.75); // Pan 0 + 0.2 · ½ = 0.1
    assert!((pan16(&e, 1) - 0.1).abs() < 0.002, "{}", pan16(&e, 1));
    e.effect_stop(None).unwrap();
    assert!(e.effects().is_empty());
    e.tick(1.0);
    assert!(pan16(&e, 1) < 0.002, "held base 0, not 0.1: {}", pan16(&e, 1));
}

#[test]
fn eng_19_take_over_compares_the_output_with_the_effect() {
    let json = r#"{
      "spec": "ENG-19",
      "profiles": { "dimmer": { "attributes": [ { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 } ] } },
      "patch": [ { "id": 2, "profile": "dimmer", "universe": 1, "address": 2 } ],
      "groups": [ { "id": 6, "fixtures": [ 2 ] } ],
      "playbacks": [ { "id": 2, "page": 1, "slot": 8, "kind": "GroupMaster", "cues": [],
        "master": { "group": 6, "mode": "TakeOver", "scale": 100, "enabled": true, "level": 0.0 } } ],
      "actions": [], "expect": []
    }"#;
    let scenario: Scenario = serde_json::from_str(json).unwrap();
    let mut e = build_engine(&scenario).unwrap();
    select(&mut e, &[2]);
    e.effect_apply(&look("Breathe")).unwrap();
    e.effect_set(EffectSet::Bpm(0.0)).unwrap(); // stopped at 0.5
    e.tick(0.0);
    assert_dmx(&e, 2, 128);
    e.set_fader(2, 0.4, 0.1).unwrap();
    e.tick(0.1);
    assert_eq!(e.programmer().value(2, "Dimmer"), None, "0.5 is not passed");
    e.set_fader(2, 0.6, 0.2).unwrap();
    e.tick(0.2);
    assert_eq!(e.programmer().value(2, "Dimmer"), Some(Value::Level(0.6)));
}

#[test]
fn eng_17_fx_09_a_speed_change_never_jumps_the_phase() {
    let mut e = rig();
    let id = apply(&mut e, &[201], "Dimmer wave");
    e.effect_set(EffectSet::Bpm(60.0)).unwrap();
    e.tick(0.0);
    assert!(close(theta(&e, id), 0.0));
    e.tick(0.5);
    assert!(close(theta(&e, id), 0.5));
    e.effect_set(EffectSet::Bpm(120.0)).unwrap();
    e.tick(0.75);
    assert!(close(theta(&e, id), 1.0), "{}", theta(&e, id));
}

#[test]
fn eng_17_fx_12_same_actions_same_bytes() {
    let run = || {
        let mut e = rig();
        apply(&mut e, &[201, 202, 203, 204, 205, 206, 207, 208], "Sparkle");
        apply(&mut e, &[101, 102, 103, 104], "Fan sweep");
        apply(&mut e, &[251, 252], "Rainbow wave");
        let mut out = Vec::new();
        for tick in 0..400u32 {
            if tick == 200 {
                e.effect_set(EffectSet::Bpm(97.0)).unwrap();
            }
            e.tick(f64::from(tick) * 0.025);
            out.extend_from_slice(e.frame(1).unwrap().slots());
        }
        out
    };
    let a = run();
    assert_eq!(a, run());
    assert!(a.iter().any(|&v| v != 0 && v != 255), "the effects ran");
}

// ---------- FX-16: set ----------

#[test]
fn fx_16_set_changes_a_parameter_and_recomputes_positions() {
    let mut e = rig();
    let id = apply(&mut e, &[201, 202, 203, 204, 205, 206, 207, 208], "Dimmer chase");
    assert_eq!(entry(&e, id).positions, [0, 1, 2, 3, 4, 5, 6, 7]);
    e.effect_set(EffectSet::Wings(2)).unwrap();
    assert_eq!(
        (entry(&e, id).positions.clone(), entry(&e, id).count),
        (vec![0, 1, 2, 3, 3, 2, 1, 0], 4)
    );
    e.effect_set(EffectSet::Groups(2)).unwrap();
    assert_eq!(
        (entry(&e, id).positions.clone(), entry(&e, id).count),
        (vec![0, 1, 0, 1, 1, 0, 1, 0], 2)
    );
    e.effect_set(EffectSet::Direction(Direction::Backward))
        .unwrap();
    e.effect_set(EffectSet::Spread(180.0)).unwrap();
    let p = entry(&e, id).params;
    assert_eq!((p.wings, p.groups, p.direction), (2, 2, Direction::Backward));
    assert!((p.spread - 180.0).abs() < 1e-6);
}

#[test]
fn fx_16_out_of_range_is_refused_and_changes_nothing() {
    let mut e = rig();
    assert!(
        e.effect_set(EffectSet::Size(0.5)).is_err(),
        "no current entry"
    );
    let id = apply(&mut e, &[201, 202], "Dimmer wave");
    let before = entry(&e, id).clone();
    for bad in [
        EffectSet::Size(1.5),
        EffectSet::Bpm(601.0),
        EffectSet::Beats(3.0),
        EffectSet::Wings(9),
        EffectSet::Groups(1),
        EffectSet::Spread(f32::NAN),
        EffectSet::SpeedSource(SpeedSource::Bpm(9)),
    ] {
        assert!(e.effect_set(bad.clone()).is_err(), "{bad:?}");
    }
    assert_eq!(entry(&e, id), &before);
}

// ---------- FX-17: effect wheels ----------

#[test]
fn fx_17_the_effect_wheels() {
    let mut e = rig();
    select(&mut e, &[201, 202, 203, 204]);
    assert!(!e.effect_wheels());
    e.set_effect_wheels(true);
    assert!(e.turn_wheel("Size", 1, Coarse).is_err(), "no entry yet");
    let id = e.effect_apply(&look("Dimmer chase")).unwrap();
    assert_eq!(
        e.wheel_layout(),
        ["Size", "Speed", "Spread", "Phase", "Width", "Top", "Beats", "Wings"]
    );
    e.turn_wheel("Size", -10, Coarse).unwrap();
    e.turn_wheel("Speed", 5, Coarse).unwrap();
    e.turn_wheel("Spread", 2, Coarse).unwrap();
    e.turn_wheel("Phase", 1, Coarse).unwrap();
    e.turn_wheel("Width", 5, Coarse).unwrap();
    e.turn_wheel("Beats", 1, Coarse).unwrap();
    e.turn_wheel("Wings", 1, Coarse).unwrap();
    let p = entry(&e, id).params;
    assert!((p.size - 0.9).abs() < 1e-4, "size 1 % a step: {}", p.size);
    assert!((p.bpm - 125.0).abs() < 1e-4, "BPM 1 a step");
    assert!((p.spread - 390.0).abs() < 1e-4, "spread 15° a step");
    assert!((p.phase - 15.0).abs() < 1e-4, "phase 15° a step");
    assert!((p.width - 0.30).abs() < 1e-4, "width 1 % a step");
    assert!((p.beats - 2.0).abs() < 1e-6, "the next beats value");
    assert_eq!(p.wings, 2);
    assert_eq!(entry(&e, id).count, 2, "positions follow the wheel");
    e.turn_wheel("Size", 50, Coarse).unwrap();
    assert!((entry(&e, id).params.size - 1.0).abs() < 1e-6, "clamped");

    // ColourMix shows Crossfade where a wave shows Width.
    apply(&mut e, &[251, 252], "Colour chase");
    assert_eq!(
        e.wheel_layout(),
        ["Size", "Speed", "Spread", "Phase", "Crossfade", "Top", "Beats", "Wings"]
    );
    e.set_effect_wheels(false);
    assert!(!e.wheel_layout().iter().any(|w| w == "Size"));
}

// ---------- FX-18: select ----------

#[test]
fn fx_18_select_an_entry_or_its_members() {
    let mut e = rig();
    let a = apply(&mut e, &[201, 202], "Dimmer wave");
    let b = apply(&mut e, &[101], "Circle");
    assert_eq!(e.current_effect(), Some(b));
    e.effect_select(a).unwrap();
    assert_eq!(e.current_effect(), Some(a));
    assert!(e.effect_select(999).is_err());
    assert_eq!(e.current_effect(), Some(a));
    e.clear_selection();
    select(&mut e, &[101]);
    assert_eq!(e.current_effect(), Some(b), "exactly b's members");
    select(&mut e, &[201]);
    assert_eq!(e.current_effect(), Some(b), "101 + 201 are no entry's members");
    e.clear_selection();
    select(&mut e, &[201, 202]);
    assert_eq!(e.current_effect(), Some(a));
}

// ---------- FX-19: stop, Off, Clear ----------

#[test]
fn fx_19_stop_takes_the_targets_out() {
    let mut e = rig();
    let a = apply(&mut e, &[201, 202, 203, 204], "Dimmer chase");
    let b = apply(&mut e, &[101], "Circle");
    e.clear_selection();
    select(&mut e, &[201, 202]);
    e.effect_stop(None).unwrap();
    assert_eq!(entry(&e, a).members, items(&[203, 204]));
    e.tick(0.0);
    assert_eq!([dmx(&e, 21), dmx(&e, 22)], [0, 0]);

    e.clear_selection();
    select(&mut e, &[101]);
    assert!(
        e.effect_stop(Some(AttributeGroup::I)).is_err(),
        "101 has no intensity effect"
    );
    assert_eq!(e.effects().len(), 2);
    e.effect_stop(Some(AttributeGroup::P)).unwrap();
    assert!(e.effects().iter().all(|x| x.id != b), "no members left");
    assert!(e.effect_stop(None).is_err(), "nothing left to stop");
}

#[test]
fn fx_19_off_on_an_attribute_removes_its_effect() {
    let mut e = rig();
    let a = apply(&mut e, &[101, 102], "Circle");
    e.clear_selection();
    select(&mut e, &[101]);
    // A path is one effect on the Pan/Tilt pair: Off on either takes the member out.
    e.off_attribute("Pan").unwrap();
    assert_eq!(entry(&e, a).members, items(&[102]));
    assert!(e.off_attribute("Tilt").is_err(), "nothing left to Off");

    let w = apply(&mut e, &[201], "Breathe");
    e.off(&[AttributeGroup::I]).unwrap();
    assert!(e.effects().iter().all(|x| x.id != w));
}

#[test]
fn fx_19_clear_empties_the_entries() {
    let mut e = rig();
    apply(&mut e, &[201, 202, 203, 204], "Dimmer chase");
    e.tick(0.0);
    assert_eq!(dmx(&e, 21), 255);
    e.clear(); // the first Clear deselects
    assert_eq!(e.effects().len(), 1);
    e.clear();
    assert!(e.effects().is_empty());
    assert_eq!(e.current_effect(), None);
    e.tick(0.1);
    assert_eq!(dmx(&e, 21), 0);
}

#[test]
fn fx_19_a_timed_clear_fades_the_effect_out() {
    let mut e = rig();
    apply(&mut e, &[201], "Breathe");
    e.effect_set(EffectSet::Bpm(0.0)).unwrap(); // held at top − size = 0.5
    e.tick(0.0);
    assert_dmx(&e, 21, 128);
    e.clear_timed(2.0, None).unwrap();
    assert_eq!(e.effects().len(), 1, "it fades, then goes");
    e.tick(1.0); // the fade starts here
    assert_dmx(&e, 21, 128);
    e.tick(2.0);
    assert_dmx(&e, 21, 64);
    e.tick(3.0);
    assert_eq!(dmx(&e, 21), 0);
    assert!(e.effects().is_empty());
    assert_eq!(e.programmer().items(), items(&[201]), "the selection stays");
}

// ---------- FX-20: undo ----------

#[test]
fn fx_20_sets_coalesce_and_undo_does_not_rewind_the_clock() {
    let mut e = rig();
    let mut h = History::default();
    select(&mut e, &[201]);
    let id = h
        .edit(&mut e, "Effect", None, |e| e.effect_apply(&look("Breathe")))
        .unwrap();
    e.tick(0.0);
    e.tick(1.0); // 20 BPM: θ = ⅓
    let key = format!("effect:{id}:size");
    h.edit(&mut e, "Effect size", Some(&key), |e| {
        e.effect_set(EffectSet::Size(0.4))
    })
    .unwrap();
    h.edit(&mut e, "Effect size", Some(&key), |e| {
        e.effect_set(EffectSet::Size(0.3))
    })
    .unwrap();
    assert_eq!(h.len(), 2, "apply, then one coalesced set");
    h.undo(&mut e).unwrap();
    assert!((entry(&e, id).params.size - 0.5).abs() < 1e-6);
    assert!(close(theta(&e, id), 1.0 / 3.0), "the clock is live");
    h.undo(&mut e).unwrap();
    assert!(e.effects().is_empty());
    assert_eq!(e.current_effect(), None);
    h.redo(&mut e).unwrap();
    assert_eq!(e.effects().len(), 1);
    assert_eq!(e.current_effect(), Some(id));
}

// ---------- FX-11, FX-10, FX-09: clock and tempo ----------

#[test]
fn fx_11_restart_sets_every_programmer_clock_to_zero() {
    let mut e = rig();
    let a = apply(&mut e, &[201], "Breathe");
    let b = apply(&mut e, &[101], "Circle");
    e.tick(0.0);
    e.tick(1.0);
    assert!(theta(&e, a) > 0.0 && theta(&e, b) > 0.0);
    e.effect_restart();
    assert_eq!((theta(&e, a), theta(&e, b)), (0.0, 0.0));
}

#[test]
fn fx_10_tap_sets_the_bpm_and_beat_syncs() {
    let mut e = rig();
    assert!(e.effect_tap(0.0).is_err(), "no current entry");
    let id = apply(&mut e, &[201], "Breathe"); // 20 BPM
    e.tick(0.0);
    e.tick(1.0); // θ = ⅓
    e.effect_tap(1.0).unwrap();
    assert!((entry(&e, id).params.bpm - 20.0).abs() < 1e-6, "one tap");
    assert!(close(theta(&e, id), 1.0 / 3.0), "the first tap does not sync");
    e.effect_tap(1.5).unwrap();
    assert!((entry(&e, id).params.bpm - 120.0).abs() < 1e-4);
    assert!(close(theta(&e, id), 0.0), "to the nearest whole beat");
    e.tick(2.0);
    assert!(close(theta(&e, id), 2.0));
    // More than 2 s later a new series starts.
    e.effect_tap(5.0).unwrap();
    assert!((entry(&e, id).params.bpm - 120.0).abs() < 1e-4);
    e.effect_tap(5.4).unwrap();
    assert!((entry(&e, id).params.bpm - 150.0).abs() < 1e-4);
}

#[test]
fn fx_10_a_tap_on_a_bpm_master_syncs_the_entries_that_follow_it() {
    let mut e = rig();
    let master = e.record_master(2, 1, MasterKind::Bpm, 1).unwrap(); // 120 BPM
    let id = apply(&mut e, &[201], "Breathe");
    e.effect_set(EffectSet::SpeedSource(SpeedSource::Bpm(1)))
        .unwrap();
    e.tick(0.0);
    e.tick(0.9); // 120 BPM: θ = 1.8
    assert!(close(theta(&e, id), 1.8), "{}", theta(&e, id));
    e.effect_tap(1.0).unwrap();
    e.effect_tap(1.25).unwrap(); // taps the master
    assert!((e.effective_bpm(master).unwrap() - 240.0).abs() < 1e-3);
    assert!((entry(&e, id).params.bpm - 20.0).abs() < 1e-6, "own bpm kept");
    assert!(close(theta(&e, id), 2.0));
    e.tick(1.35); // 240 BPM: + 0.45 · 4
    assert!(close(theta(&e, id), 3.8), "{}", theta(&e, id));
    e.tap(master, 1.35).unwrap(); // CUE-33 on the master itself
    assert!(close(theta(&e, id), 4.0), "{}", theta(&e, id));
}

#[test]
fn fx_09_speed_sources_resolve_like_chases() {
    let mut e = rig();
    let id = apply(&mut e, &[201], "Breathe"); // 20 BPM
    e.effect_set(EffectSet::SpeedSource(SpeedSource::Bpm(3)))
        .unwrap();
    e.tick(0.0);
    e.tick(3.0); // master 3 is missing: Free at 20 BPM
    assert!(close(theta(&e, id), 1.0), "{}", theta(&e, id));
    e.record_master(2, 1, MasterKind::Bpm, 3).unwrap(); // 120 BPM
    e.tick(3.5);
    assert!(close(theta(&e, id), 2.0), "{}", theta(&e, id));
    e.effect_set(EffectSet::Beats(2.0)).unwrap(); // a cycle lasts two beats
    e.tick(4.5);
    assert!(close(theta(&e, id), 3.0), "{}", theta(&e, id));
}

// ---------- FX-03, FX-05: colour ----------

#[test]
fn fx_03_a_colour_chase_runs_the_base_and_its_complement() {
    let mut e = rig();
    let red = rgb(1.0, 0.0, 0.0);
    for id in [251, 252] {
        e.set_programmer_value(id, "Dimmer", 1.0).unwrap();
        e.set_programmer_value(id, "Colour", Value::Colour(red))
            .unwrap();
    }
    let id = apply(&mut e, &[251, 252], "Colour chase");
    let colours = &entry(&e, id).colours;
    assert_eq!(colours.len(), 2);
    assert!(colour_close(colours[0], red));
    assert!(colour_close(colours[1], rgb(0.0, 1.0, 1.0)), "{colours:?}");
    e.tick(0.0);
    assert_eq!([dmx(&e, 42), dmx(&e, 43), dmx(&e, 44)], [255, 0, 0]);
    assert_eq!([dmx(&e, 46), dmx(&e, 47), dmx(&e, 48)], [0, 255, 255]);
}

#[test]
fn fx_03_fx_05_a_rainbow_replaces_the_base_colour() {
    let mut e = rig();
    e.set_programmer_value(251, "Dimmer", 1.0).unwrap();
    e.set_programmer_value(251, "Colour", Value::Colour(rgb(0.0, 0.0, 1.0)))
        .unwrap();
    apply(&mut e, &[251], "Rainbow"); // 6 BPM
    e.tick(0.0);
    assert_eq!([dmx(&e, 42), dmx(&e, 43), dmx(&e, 44)], [255, 0, 0]);
    e.tick(10.0 / 3.0); // θ = ⅓: green
    assert_dmx(&e, 42, 0);
    assert_dmx(&e, 43, 255);
    assert_dmx(&e, 44, 0);
    // Size ½ mixes the base in (lerp, FX-05).
    e.effect_set(EffectSet::Size(0.5)).unwrap();
    e.tick(10.0 / 3.0);
    assert_dmx(&e, 42, 0);
    assert_dmx(&e, 43, 128);
    assert_dmx(&e, 44, 128);
}

#[test]
fn fx_06_a_colour_list_holds_two_to_eight_colours() {
    let mut e = rig();
    let id = apply(&mut e, &[251, 252], "Colour fade");
    let red = rgb(1.0, 0.0, 0.0);
    assert!(e.effect_set(EffectSet::Colours(vec![red])).is_err());
    assert!(e.effect_set(EffectSet::Colours(vec![red; 9])).is_err());
    let three = vec![red, rgb(0.0, 1.0, 0.0), rgb(0.0, 0.0, 1.0)];
    e.effect_set(EffectSet::Colours(three.clone())).unwrap();
    assert_eq!(entry(&e, id).colours, three);
    // Not a ColourMix: refused.
    apply(&mut e, &[201], "Breathe");
    assert!(e.effect_set(EffectSet::Colours(three)).is_err());
}
