//! Fixture Exchange (PTC-01): `specs/modules/patch.md` PAT-37..39.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;

use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::palette::{GlobalValue, Palette, Window};
use cmdmx_core::patch::{FixtureId, PatchAdd};
use cmdmx_core::playback::Playback;
use cmdmx_core::profile::Profile;
use cmdmx_core::programmer::Item;
use cmdmx_core::show::Meta;
use cmdmx_core::undo::History;
use cmdmx_core::value::{Reference, Stored, Value};
use cmdmx_core::CoreError;

/// 5 channels; the wheel has Open, Star and Moon.
const WASH: &str = r#"{ "manufacturer": "T", "model": "Wash", "source": "channel-list", "modes": [
  { "name": "Std", "footprint": 5, "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 2 },
    { "name": "Tilt", "group": "P", "kind": "Ltp", "channel": 3 },
    { "name": "Gobo", "group": "G", "kind": "Ltp", "channel": 4, "slots": [
      { "name": "Open", "from": 0, "to": 9 },
      { "name": "Star", "from": 20, "to": 30 },
      { "name": "Moon", "from": 40, "to": 50 } ] },
    { "name": "Strobe", "group": "S", "kind": "Ltp", "channel": 5 } ] } ] }"#;

/// "Long" has another wheel (no Moon) and Zoom; "Short" is 3 channels.
const SPOT: &str = r#"{ "manufacturer": "T", "model": "Spot", "source": "channel-list", "modes": [
  { "name": "Long", "footprint": 6, "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 2 },
    { "name": "Tilt", "group": "P", "kind": "Ltp", "channel": 3 },
    { "name": "Zoom", "group": "B", "kind": "Ltp", "channel": 4 },
    { "name": "Gobo", "group": "G", "kind": "Ltp", "channel": 5, "slots": [
      { "name": "star", "from": 60, "to": 70 },
      { "name": "Open", "from": 100, "to": 110 } ] },
    { "name": "Iris", "group": "B", "kind": "Ltp", "channel": 6 } ] },
  { "name": "Short", "footprint": 3, "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 2 },
    { "name": "Tilt", "group": "P", "kind": "Ltp", "channel": 3 } ] } ] }"#;

/// A Gobo without slots.
const PLAIN: &str = r#"{ "manufacturer": "T", "model": "Plain", "source": "channel-list", "modes": [
  { "name": "One", "footprint": 2, "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "Gobo", "group": "G", "kind": "Ltp", "channel": 2 } ] } ] }"#;

/// RGB with a virtual dimmer.
const PAR: &str = r#"{ "manufacturer": "T", "model": "Par", "source": "channel-list", "modes": [
  { "name": "Rgb", "footprint": 3, "virtual_dimmer": true,
    "colour": { "system": "RGB", "components": [ { "channel": 1 }, { "channel": 2 }, { "channel": 3 } ] },
    "attributes": [] } ] }"#;

/// Star, Moon and between-slots as levels on the wash's wheel, and the middle of the spot's star.
const STAR: f32 = 25.0 / 255.0;
const MOON: f32 = 45.0 / 255.0;
const BETWEEN: f32 = 15.0 / 255.0;
const SPOT_STAR: f32 = 65.0 / 255.0;

/// Two washes, 101 at 1.1 and 102 at 1.6 (ids 1 and 2).
fn rig() -> Engine {
    let mut e = Engine::new(Settings::default());
    for (key, json) in [
        ("wash", WASH),
        ("spot", SPOT),
        ("plain", PLAIN),
        ("par", PAR),
    ] {
        e.add_profile(key, Profile::from_json(json).unwrap())
            .unwrap();
    }
    e.patch_add(&PatchAdd {
        profile: "wash".into(),
        mode: Some("Std".into()),
        count: 2,
        universe: 1,
        address: Some(1),
        number: Some(101),
    })
    .unwrap();
    e
}

fn ids(e: &Engine) -> (FixtureId, FixtureId) {
    (
        e.fixture_by_number(101).unwrap().id,
        e.fixture_by_number(102).unwrap().id,
    )
}

fn both(e: &Engine) -> Vec<FixtureId> {
    let (a, b) = ids(e);
    vec![a, b]
}

fn dmx(e: &Engine, universe: u16, address: u16) -> u8 {
    e.frame(universe).and_then(|f| f.channel(address)).unwrap()
}

fn messages(e: &mut Engine) -> Vec<String> {
    e.drain_warnings().into_iter().map(|w| w.message).collect()
}

/// Only the lines about what dropped (a place on an empty universe also gives OUT-26's notice).
fn dropped(e: &mut Engine) -> Vec<String> {
    let mut lines = messages(e);
    lines.retain(|m| m.starts_with("Dropped"));
    lines
}

fn close(a: Option<Value>, b: f32) -> bool {
    a.is_some_and(|v| (v.level() - b).abs() < 1e-4)
}

/// Playback 1 on 1.1: one cue with `values` for `item`.
fn cue_of(e: &mut Engine, item: Item, values: Vec<(&str, Stored)>) {
    e.add_playback(Playback::new(
        1,
        1,
        1,
        BTreeMap::from([(
            item,
            values
                .into_iter()
                .map(|(a, s)| (a.to_string(), s))
                .collect(),
        )]),
    ));
}

fn cue_value(e: &Engine, item: Item, attribute: &str) -> Option<Stored> {
    e.playback(1).unwrap().cues[0]
        .values
        .get(&item)?
        .get(attribute)
        .copied()
}

fn palette(number: u32, fill: impl FnOnce(&mut Palette)) -> Palette {
    let mut p = Palette {
        id: 100 + number,
        number,
        window: Window::Positions,
        handle: u16::try_from(number).unwrap(),
        legend: "Look".into(),
        icon: None,
        global: BTreeMap::new(),
        shared: BTreeMap::new(),
        shared_cells: BTreeMap::new(),
        fixtures: BTreeMap::new(),
        effects: Vec::new(),
    };
    fill(&mut p);
    p
}

fn attrs(values: &[(&str, f32)]) -> BTreeMap<String, Value> {
    values
        .iter()
        .map(|(a, v)| (a.to_string(), Value::Level(*v)))
        .collect()
}

// ---- PAT-37 -------------------------------------------------------------------------

#[test]
fn pat_37_each_fixture_keeps_id_number_name_handle_groups_and_selection() {
    let mut e = rig();
    let (a, b) = ids(&e);
    e.patch_name_range(&[a, b], "Front 1").unwrap();
    e.select_item(Item::fixture(a)).unwrap();
    e.select_item(Item::fixture(b)).unwrap();
    e.record_group(5).unwrap();
    let handle = e.fixture(b).unwrap().handle;
    e.patch_exchange(&[a, b], "spot", "Short", None).unwrap();
    let f = e.fixture(b).unwrap();
    assert_eq!(f.profile, "spot");
    assert_eq!(f.mode.as_deref(), Some("Short"));
    assert_eq!(f.number, 102);
    assert_eq!(f.name, "Front 2");
    assert_eq!(f.handle, handle);
    assert_eq!((f.universe, f.address), (1, 6), "no place: it stays");
    assert_eq!(e.group(5).unwrap(), [a, b]);
    assert_eq!(e.fixture(a).unwrap().profile, "spot");
}

#[test]
fn pat_37_an_unknown_profile_or_mode_refuses_naming_it_and_nothing_changes() {
    let mut e = rig();
    let range = both(&e);
    let before = e.edit_state();
    assert_eq!(
        e.patch_exchange(&range, "nope", "Short", None),
        Err(CoreError::UnknownProfile("nope".into()))
    );
    assert_eq!(
        e.patch_exchange(&range, "spot", "Huge", None),
        Err(CoreError::UnknownMode {
            profile: "spot".into(),
            mode: "Huge".into()
        })
    );
    assert!(e.edit_state() == before);
}

#[test]
fn pat_37_empty_unknown_or_repeated_ids_refuse() {
    let mut e = rig();
    let (a, _) = ids(&e);
    assert_eq!(
        e.patch_exchange(&[], "spot", "Short", None),
        Err(CoreError::EmptySelection)
    );
    assert_eq!(
        e.patch_exchange(&[a, a], "spot", "Short", None),
        Err(CoreError::RepeatedFixture(a))
    );
    assert_eq!(
        e.patch_exchange(&[999], "spot", "Short", None),
        Err(CoreError::UnknownFixture(999))
    );
}

#[test]
fn pat_37_without_a_place_the_new_footprints_are_checked_as_pat_22() {
    let mut e = rig();
    let range = both(&e);
    let before = e.edit_state();
    assert_eq!(
        e.patch_exchange(&range, "spot", "Long", None),
        Err(CoreError::PatchOverlap {
            number: 101,
            other: 102,
            universe: 1,
            address: 6
        }),
        "the range's own new footprints"
    );
    assert!(e.edit_state() == before, "all or nothing");
    e.patch_add(&PatchAdd {
        profile: "plain".into(),
        mode: Some("One".into()),
        count: 1,
        universe: 1,
        address: Some(11),
        number: Some(201),
    })
    .unwrap();
    let (_, b) = ids(&e);
    assert_eq!(
        e.patch_exchange(&[b], "spot", "Long", None),
        Err(CoreError::PatchOverlap {
            number: 102,
            other: 201,
            universe: 1,
            address: 11
        }),
        "a fixture outside the range"
    );
}

#[test]
fn pat_37_a_place_sets_the_range_back_to_back_in_the_same_command() {
    let mut e = rig();
    let range = both(&e);
    e.patch_exchange(&range, "spot", "Long", Some((1, 1)))
        .unwrap();
    let (a, b) = ids(&e);
    assert_eq!(e.fixture(a).unwrap().address, 1);
    assert_eq!(e.fixture(b).unwrap().address, 7, "a block that grows");
    e.patch_exchange(&range, "spot", "Short", Some((2, 101)))
        .unwrap();
    let f = e.fixture(b).unwrap();
    assert_eq!((f.universe, f.address), (2, 104));
    assert_eq!(
        e.patch_exchange(&range, "spot", "Long", Some((2, 510))),
        Err(CoreError::AddressOutOfRange {
            fixture: 101,
            universe: 2,
            address: 510
        })
    );
}

#[test]
fn pat_37_a_parked_fixture_is_not_checked_stays_parked_and_a_place_unparks_it() {
    let mut e = rig();
    let (a, b) = ids(&e);
    e.patch_park(&[a]).unwrap();
    e.patch_exchange(&[a], "spot", "Long", None).unwrap();
    assert!(e.fixture(a).unwrap().parked, "no channels to check");
    e.patch_exchange(&[a], "spot", "Long", Some((2, 1)))
        .unwrap();
    let f = e.fixture(a).unwrap();
    assert!(!f.parked);
    assert_eq!((f.universe, f.address), (2, 1));
    assert!(e.fixture(b).is_some());
}

#[test]
fn pat_37_old_channels_go_to_zero_and_the_new_type_plays_from_the_next_tick() {
    let mut e = rig();
    let (a, _) = ids(&e);
    e.set_programmer_value(a, "Dimmer", 1.0).unwrap();
    e.set_programmer_value(a, "Gobo", STAR).unwrap();
    e.set_programmer_value(a, "Strobe", 0.5).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, 1, 4), 25);
    e.patch_exchange(&[a], "spot", "Short", None).unwrap();
    e.tick(0.025);
    assert_eq!(dmx(&e, 1, 1), 255, "Dimmer on the new type");
    assert_eq!(dmx(&e, 1, 4), 0, "beyond the new footprint");
    assert_eq!(dmx(&e, 1, 5), 0, "beyond the new footprint");
}

#[test]
fn pat_37_one_undo_step_brings_it_all_back() {
    let mut e = rig();
    let (a, b) = ids(&e);
    e.set_programmer_value(a, "Strobe", 0.5).unwrap();
    e.set_programmer_value(b, "Gobo", MOON).unwrap();
    let before = e.edit_state();
    let mut h = History::default();
    h.edit(&mut e, "Exchange 2 fixtures", None, |e| {
        e.patch_exchange(&[a, b], "spot", "Long", Some((1, 1)))
    })
    .unwrap();
    assert_eq!(h.undo(&mut e).unwrap(), "Exchange 2 fixtures");
    assert!(e.edit_state() == before);
    assert_eq!(e.fixture(a).unwrap().profile, "wash");
}

// ---- PAT-38 -------------------------------------------------------------------------

#[test]
fn pat_38_a_matching_attribute_stays_a_missing_one_drops_with_one_line() {
    let mut e = rig();
    let (a, b) = ids(&e);
    e.set_programmer_value(a, "Dimmer", 0.8).unwrap();
    e.set_programmer_value(a, "Strobe", 0.5).unwrap();
    e.set_programmer_value(b, "Strobe", 0.5).unwrap();
    cue_of(
        &mut e,
        Item::fixture(b),
        vec![("Pan", 0.3.into()), ("Gobo", STAR.into())],
    );
    e.drain_warnings();
    e.patch_exchange(&[a, b], "spot", "Short", None).unwrap();
    assert!(close(e.programmer().value(a, "Dimmer"), 0.8));
    assert_eq!(e.programmer().value(a, "Strobe"), None);
    assert_eq!(cue_value(&e, Item::fixture(b), "Pan"), Some(0.3.into()));
    assert_eq!(cue_value(&e, Item::fixture(b), "Gobo"), None);
    assert_eq!(messages(&mut e), ["Dropped Gobo, Strobe on 2 fixtures"]);
}

#[test]
fn pat_38_one_fixture_says_fixture_and_nothing_dropped_says_nothing() {
    let mut e = rig();
    let (a, b) = ids(&e);
    e.set_programmer_value(a, "Strobe", 0.5).unwrap();
    e.drain_warnings();
    e.patch_exchange(&[a], "spot", "Short", None).unwrap();
    assert_eq!(messages(&mut e), ["Dropped Strobe on 1 fixture"]);
    e.patch_exchange(&[b], "spot", "Short", None).unwrap();
    assert!(messages(&mut e).is_empty());
}

#[test]
fn pat_38_a_wheel_level_moves_to_the_middle_of_the_slot_of_that_name() {
    let mut e = rig();
    let (a, _) = ids(&e);
    e.set_programmer_value(a, "Gobo", STAR).unwrap();
    cue_of(&mut e, Item::fixture(a), vec![("Gobo", STAR.into())]);
    let mut pb = e.playback(1).unwrap().clone();
    pb.cues[0].dormant = BTreeMap::from([(
        Item::fixture(a),
        BTreeMap::from([("Gobo".to_string(), STAR.into())]),
    )]);
    e.add_playback(pb);
    e.patch_exchange(&[a], "spot", "Long", Some((2, 1)))
        .unwrap();
    assert!(close(e.programmer().value(a, "Gobo"), SPOT_STAR), "star");
    assert!(close(
        cue_value(&e, Item::fixture(a), "Gobo").map(|s| s.value()),
        SPOT_STAR
    ));
    assert!(close(
        e.playback(1).unwrap().cues[0]
            .dormant
            .get(&Item::fixture(a))
            .and_then(|v| v.get("Gobo"))
            .map(|s| s.value()),
        SPOT_STAR
    ));
    e.tick(0.0);
    assert_eq!(dmx(&e, 2, 5), 65);
}

#[test]
fn pat_38_no_slot_of_that_name_drops_the_value_and_names_it() {
    let mut e = rig();
    let (a, _) = ids(&e);
    e.set_programmer_value(a, "Gobo", MOON).unwrap();
    e.tick(0.0);
    e.drain_warnings();
    e.patch_exchange(&[a], "spot", "Long", Some((2, 1)))
        .unwrap();
    assert_eq!(e.programmer().value(a, "Gobo"), None);
    assert_eq!(dropped(&mut e), ["Dropped Gobo on 1 fixture"]);
    e.tick(0.025);
    assert_eq!(dmx(&e, 2, 5), 0, "the default, not another gobo");
}

#[test]
fn pat_38_a_level_in_no_slot_or_going_to_a_wheel_without_slots_stays() {
    let mut e = rig();
    let (a, b) = ids(&e);
    e.set_programmer_value(a, "Gobo", BETWEEN).unwrap();
    e.set_programmer_value(b, "Gobo", STAR).unwrap();
    e.drain_warnings();
    e.patch_exchange(&[a], "spot", "Long", Some((2, 1)))
        .unwrap();
    assert!(close(e.programmer().value(a, "Gobo"), BETWEEN));
    e.patch_exchange(&[b], "plain", "One", None).unwrap();
    assert!(close(e.programmer().value(b, "Gobo"), STAR));
    assert!(dropped(&mut e).is_empty());
}

#[test]
fn pat_38_position_is_not_converted_and_a_virtual_dimmer_is_a_dimmer() {
    let mut e = rig();
    let (a, b) = ids(&e);
    e.set_programmer_value(a, "Pan", 0.3).unwrap();
    e.set_programmer_value(b, "Dimmer", 0.6).unwrap();
    e.patch_exchange(&[a], "spot", "Short", None).unwrap();
    assert!(close(e.programmer().value(a, "Pan"), 0.3));
    e.patch_exchange(&[b], "par", "Rgb", None).unwrap();
    assert!(close(e.programmer().value(b, "Dimmer"), 0.6));
}

#[test]
fn pat_38_a_palette_reference_stays_its_fallback_follows_the_slot_rule() {
    let mut e = rig();
    let (a, _) = ids(&e);
    let reference = |value: f32| {
        Stored::Reference(Reference {
            palette: 103,
            value: Value::Level(value),
        })
    };
    e.add_palette(palette(3, |p| {
        p.fixtures
            .insert(Item::fixture(a), attrs(&[("Gobo", STAR)]));
    }))
    .unwrap();
    cue_of(
        &mut e,
        Item::fixture(a),
        vec![("Gobo", reference(STAR)), ("Pan", reference(0.3))],
    );
    e.patch_exchange(&[a], "spot", "Long", Some((2, 1)))
        .unwrap();
    assert_eq!(cue_value(&e, Item::fixture(a), "Pan"), Some(reference(0.3)));
    match cue_value(&e, Item::fixture(a), "Gobo") {
        Some(Stored::Reference(r)) => {
            assert_eq!(r.palette, 103);
            assert!(close(Some(r.value), SPOT_STAR));
        }
        other => panic!("still a reference, got {other:?}"),
    }
}

#[test]
fn pat_38_a_reference_with_a_fallback_in_no_slot_of_that_name_falls_back_to_the_default() {
    let mut e = rig();
    let (a, _) = ids(&e);
    cue_of(
        &mut e,
        Item::fixture(a),
        vec![(
            "Gobo",
            Stored::Reference(Reference {
                palette: 999,
                value: Value::Level(MOON),
            }),
        )],
    );
    e.patch_exchange(&[a], "spot", "Long", Some((2, 1)))
        .unwrap();
    match cue_value(&e, Item::fixture(a), "Gobo") {
        Some(Stored::Reference(r)) => {
            assert_eq!(r.palette, 999);
            assert_eq!(r.value, Value::Level(0.0), "never another gobo");
        }
        other => panic!("still a reference, got {other:?}"),
    }
}

#[test]
fn pat_38_a_cell_items_values_stay() {
    let mut e = rig();
    let (a, _) = ids(&e);
    let cell = Item {
        fixture: a,
        cell: Some(1),
    };
    cue_of(&mut e, cell, vec![("Strobe", 0.5.into())]);
    e.patch_exchange(&[a], "spot", "Short", None).unwrap();
    assert_eq!(cue_value(&e, cell, "Strobe"), Some(0.5.into()));
}

#[test]
fn pat_38_a_running_cue_gives_the_frames_of_the_show_reopened() {
    let meta = || Meta {
        name: "X".into(),
        created: "2026-11-08T09:00:00Z".into(),
        modified: "2026-11-08T09:00:00Z".into(),
        app_version: "0.1.0".into(),
    };
    let mut e = rig();
    let (a, b) = ids(&e);
    cue_of(
        &mut e,
        Item::fixture(a),
        vec![
            ("Dimmer", 1.0.into()),
            ("Gobo", STAR.into()),
            ("Strobe", 0.5.into()),
        ],
    );
    e.set_fader(1, 1.0, 0.0).unwrap();
    e.tick(0.0);
    e.tick(0.025);
    e.patch_exchange(&[a, b], "spot", "Long", Some((1, 1)))
        .unwrap();
    let (mut reopened, warnings) = Engine::from_show(&e.to_show(meta()));
    assert!(warnings.is_empty(), "{warnings:?}");
    reopened.tick(0.0);
    reopened.set_fader(1, 1.0, 0.0).unwrap();
    reopened.tick(0.0);
    for t in [0.05, 0.075, 0.1] {
        e.tick(t);
        reopened.tick(t);
        for address in 1..=14 {
            assert_eq!(
                dmx(&e, 1, address),
                dmx(&reopened, 1, address),
                "{address} at {t}"
            );
        }
    }
    assert_eq!(dmx(&e, 1, 5), 65, "the star on the new wheel");
}

// ---- PAT-39 -------------------------------------------------------------------------

#[test]
fn pat_39_the_fixtures_own_values_stay_with_the_slot_rule_and_a_lacking_one_is_named() {
    let mut e = rig();
    let (a, b) = ids(&e);
    e.add_palette(palette(3, |p| {
        p.fixtures.insert(
            Item::fixture(a),
            attrs(&[("Gobo", STAR), ("Strobe", 0.5), ("Pan", 0.3)]),
        );
        p.fixtures
            .insert(Item::fixture(b), attrs(&[("Gobo", MOON), ("Strobe", 0.5)]));
    }))
    .unwrap();
    e.drain_warnings();
    e.patch_exchange(&[a, b], "spot", "Long", Some((1, 1)))
        .unwrap();
    let p = e.palette(3).unwrap();
    let own = |i: FixtureId| p.fixtures.get(&Item::fixture(i)).unwrap();
    assert!(close(own(a).get("Gobo").copied(), SPOT_STAR));
    assert!(close(own(a).get("Pan").copied(), 0.3));
    assert!(close(own(a).get("Strobe").copied(), 0.5), "it stays");
    assert_eq!(own(b).get("Gobo"), None, "no slot of that name");
    let lines = messages(&mut e);
    assert!(
        lines.contains(
            &"Palette 3 Look: no Strobe on 2 fixtures in this type, the value stays".to_string()
        ),
        "{lines:?}"
    );
    assert!(
        lines.contains(&"Dropped Gobo on 1 fixture".to_string()),
        "{lines:?}"
    );
}

#[test]
fn pat_39_shared_values_get_a_copy_under_the_new_type_and_the_old_ones_stay() {
    let mut e = rig();
    let range = both(&e);
    e.add_palette(palette(4, |p| {
        p.shared.insert(
            "wash".into(),
            attrs(&[("Gobo", STAR), ("Pan", 0.3), ("Strobe", 0.5)]),
        );
        p.shared_cells
            .insert("wash".into(), BTreeMap::from([(1, attrs(&[("Pan", 0.2)]))]));
    }))
    .unwrap();
    e.patch_exchange(&range, "spot", "Long", Some((1, 1)))
        .unwrap();
    let p = e.palette(4).unwrap();
    assert_eq!(p.shared["wash"].len(), 3, "for fixtures not exchanged");
    let copy = &p.shared["spot"];
    assert!(close(copy.get("Gobo").copied(), SPOT_STAR));
    assert!(close(copy.get("Pan").copied(), 0.3));
    assert_eq!(copy.get("Strobe"), None, "by PAT-38's rule");
    assert_eq!(p.shared_cells["spot"], p.shared_cells["wash"]);
}

#[test]
fn pat_39_a_shared_value_in_no_slot_of_that_name_is_named_for_the_fixtures_of_that_type() {
    let mut e = rig();
    let range = both(&e);
    e.add_palette(palette(4, |p| {
        p.shared.insert("wash".into(), attrs(&[("Gobo", MOON)]));
    }))
    .unwrap();
    e.drain_warnings();
    e.patch_exchange(&range, "spot", "Long", Some((1, 1)))
        .unwrap();
    assert_eq!(
        e.palette(4).unwrap().shared.get("spot"),
        None,
        "nothing to copy, no empty entry"
    );
    assert_eq!(messages(&mut e), ["Dropped Gobo on 2 fixtures"]);
}

#[test]
fn pat_39_a_range_of_two_old_types_copies_the_first_in_number_order() {
    let mut e = rig();
    let (a, _) = ids(&e);
    e.patch_add(&PatchAdd {
        profile: "plain".into(),
        mode: Some("One".into()),
        count: 1,
        universe: 1,
        address: Some(20),
        number: Some(201),
    })
    .unwrap();
    let p = e.fixture_by_number(201).unwrap().id;
    e.add_palette(palette(7, |p| {
        p.shared.insert("wash".into(), attrs(&[("Dimmer", 0.2)]));
        p.shared.insert("plain".into(), attrs(&[("Dimmer", 0.7)]));
    }))
    .unwrap();
    e.patch_exchange(&[a, p], "spot", "Short", None).unwrap();
    assert_eq!(
        e.palette(7).unwrap().shared["spot"],
        attrs(&[("Dimmer", 0.2)])
    );
}

#[test]
fn pat_39_a_palette_with_its_own_values_for_the_new_type_keeps_them() {
    let mut e = rig();
    let range = both(&e);
    e.add_palette(palette(5, |p| {
        p.shared.insert("wash".into(), attrs(&[("Pan", 0.3)]));
        p.shared.insert("spot".into(), attrs(&[("Pan", 0.9)]));
    }))
    .unwrap();
    e.patch_exchange(&range, "spot", "Long", Some((1, 1)))
        .unwrap();
    assert_eq!(e.palette(5).unwrap().shared["spot"], attrs(&[("Pan", 0.9)]));
}

#[test]
fn pat_39_global_values_do_not_change_and_nothing_is_named_for_them() {
    let mut e = rig();
    let range = both(&e);
    e.add_palette(palette(6, |p| {
        p.global
            .insert("Strobe".into(), GlobalValue::Value(Value::Level(0.5)));
    }))
    .unwrap();
    let before = e.palette(6).unwrap().clone();
    e.drain_warnings();
    e.patch_exchange(&range, "spot", "Short", None).unwrap();
    assert_eq!(e.palette(6).unwrap(), &before);
    assert!(messages(&mut e).is_empty());
}
