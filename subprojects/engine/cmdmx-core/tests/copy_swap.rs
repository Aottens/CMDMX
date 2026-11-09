//! Copy a patched fixture and Swap Addresses (PTC-04): `specs/modules/patch.md` PAT-40, PAT-41.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;

use cmdmx_core::effect::{EffectApply, EffectSet, Shape, Wave};
use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::palette::{Palette, Window};
use cmdmx_core::patch::{FixtureId, PatchAdd};
use cmdmx_core::playback::{Playback, PlaybackRecord};
use cmdmx_core::profile::Profile;
use cmdmx_core::programmer::Item;
use cmdmx_core::undo::History;
use cmdmx_core::value::{Stored, Value};
use cmdmx_core::CoreError;

const WASH: &str = r#"{ "manufacturer": "T", "model": "Wash", "source": "channel-list", "modes": [
  { "name": "Std", "footprint": 5, "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 2 },
    { "name": "Tilt", "group": "P", "kind": "Ltp", "channel": 3 },
    { "name": "Zoom", "group": "B", "kind": "Ltp", "channel": 4 },
    { "name": "Iris", "group": "B", "kind": "Ltp", "channel": 5 } ] } ] }"#;

const SMALL: &str = r#"{ "manufacturer": "T", "model": "Small", "source": "channel-list", "modes": [
  { "name": "Two", "footprint": 2, "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "Zoom", "group": "B", "kind": "Ltp", "channel": 2 } ] } ] }"#;

fn engine() -> Engine {
    let mut e = Engine::new(Settings::default());
    e.add_profile("wash", Profile::from_json(WASH).unwrap())
        .unwrap();
    e.add_profile("small", Profile::from_json(SMALL).unwrap())
        .unwrap();
    e
}

/// `count` of `profile` from `number` on `universe.address`; returns the ids.
fn add(
    e: &mut Engine,
    profile: &str,
    count: u32,
    number: u32,
    universe: u16,
    address: u16,
) -> Vec<FixtureId> {
    e.patch_add(&PatchAdd {
        profile: profile.into(),
        mode: None,
        count,
        universe,
        address: Some(address),
        number: Some(number),
    })
    .unwrap()
}

/// Washes 101 at 1.1 and 102 at 1.6.
fn rig() -> (Engine, FixtureId, FixtureId) {
    let mut e = engine();
    let ids = add(&mut e, "wash", 2, 101, 1, 1);
    (e, ids[0], ids[1])
}

fn place(e: &Engine, id: FixtureId) -> (u16, u16) {
    let f = e.fixture(id).unwrap();
    (f.universe, f.address)
}

fn dmx(e: &Engine, universe: u16, address: u16) -> u8 {
    e.frame(universe).and_then(|f| f.channel(address)).unwrap()
}

fn level(v: f32) -> Stored {
    Stored::Value(Value::Level(v))
}

fn row(attribute: &str, v: f32) -> BTreeMap<String, Stored> {
    BTreeMap::from([(attribute.to_string(), level(v))])
}

fn cell(fixture: FixtureId, cell: u16) -> Item {
    Item {
        fixture,
        cell: Some(cell),
    }
}

fn select(e: &mut Engine, ids: &[FixtureId]) {
    for &id in ids {
        e.select_item(Item::fixture(id)).unwrap();
    }
}

// ---- PAT-40 -------------------------------------------------------------------------

#[test]
fn pat_40_copies_keep_type_and_name_and_number_on_after_the_highest() {
    let (mut e, a, b) = rig();
    e.patch_name_range(&[a, b], "Front 1").unwrap();
    add(&mut e, "small", 1, 300, 1, 100);
    // Given against number order: the copies still follow it.
    let new = e.patch_copy(&[b, a], None).unwrap();
    assert_eq!(new.len(), 2);
    let (c1, c2) = (e.fixture(new[0]).unwrap(), e.fixture(new[1]).unwrap());
    assert_eq!((c1.number, c2.number), (301, 302));
    assert_eq!((c1.name.as_str(), c2.name.as_str()), ("Front 1", "Front 2"));
    assert_eq!(c1.profile, "wash");
    assert_eq!(c1.mode, e.fixture(a).unwrap().mode);
}

#[test]
fn pat_40_a_start_number_counts_upwards_and_each_copy_gets_the_next_free_handle() {
    let (mut e, a, b) = rig();
    let taken = [
        e.fixture(a).unwrap().handle.unwrap(),
        e.fixture(b).unwrap().handle.unwrap(),
    ];
    let new = e.patch_copy(&[a, b], Some(500)).unwrap();
    let (c1, c2) = (e.fixture(new[0]).unwrap(), e.fixture(new[1]).unwrap());
    assert_eq!((c1.number, c2.number), (500, 501));
    let (h1, h2) = (c1.handle.unwrap(), c2.handle.unwrap());
    assert_ne!(h1, h2);
    assert!(!taken.contains(&h1) && !taken.contains(&h2));
}

#[test]
fn pat_40_a_used_number_or_a_bad_range_refuses_and_nothing_changes() {
    let (mut e, a, b) = rig();
    add(&mut e, "small", 1, 501, 1, 100);
    let before = e.edit_state();
    assert!(matches!(
        e.patch_copy(&[a, b], Some(500)),
        Err(CoreError::NumberInUse(501))
    ));
    assert!(matches!(
        e.patch_copy(&[a], Some(0)),
        Err(CoreError::InvalidNumber)
    ));
    assert!(matches!(
        e.patch_copy(&[], None),
        Err(CoreError::EmptySelection)
    ));
    assert!(matches!(
        e.patch_copy(&[9999], None),
        Err(CoreError::UnknownFixture(9999))
    ));
    assert!(matches!(
        e.patch_copy(&[a, a], None),
        Err(CoreError::RepeatedFixture(_))
    ));
    assert!(e.edit_state() == before, "all or nothing");
}

#[test]
fn pat_40_a_copy_starts_parked_at_the_originals_place_and_outputs_nothing() {
    let (mut e, a, b) = rig();
    e.patch_park(&[b]).unwrap();
    e.set_programmer_value(a, "Dimmer", 1.0).unwrap();
    e.tick(0.0);
    let new = e.patch_copy(&[a, b], None).unwrap();
    e.tick(0.025);
    assert!(e.fixture(new[0]).unwrap().parked);
    assert!(e.fixture(new[1]).unwrap().parked);
    assert_eq!(place(&e, new[0]), place(&e, a));
    assert_eq!(place(&e, new[1]), place(&e, b));
    assert!(!e.fixture(a).unwrap().parked, "the original keeps hanging");
    assert_eq!(dmx(&e, 1, 1), 255, "the original still plays");
}

#[test]
fn pat_40_a_copy_comes_with_cue_values_dormant_values_order_and_cells() {
    let (mut e, a, b) = rig();
    let mut pb = Playback::new(
        1,
        1,
        1,
        BTreeMap::from([
            (Item::fixture(a), row("Dimmer", 0.5)),
            (cell(a, 2), row("Zoom", 0.25)),
            (Item::fixture(b), row("Dimmer", 0.75)),
        ]),
    );
    pb.cues[0].dormant = BTreeMap::from([(Item::fixture(a), row("Pan", 0.1))]);
    pb.cues[0].order = BTreeMap::from([(Item::fixture(a), 3), (Item::fixture(b), 1)]);
    e.add_playback(pb);
    let new = e.patch_copy(&[a], None).unwrap();
    let c = new[0];
    let cue = &e.playback(1).unwrap().cues[0];
    assert_eq!(cue.values[&Item::fixture(c)], row("Dimmer", 0.5));
    assert_eq!(cue.values[&cell(c, 2)], row("Zoom", 0.25));
    assert_eq!(cue.dormant[&Item::fixture(c)], row("Pan", 0.1));
    assert_eq!(cue.order[&Item::fixture(c)], 3);
    assert_eq!(cue.order[&Item::fixture(b)], 1, "the others stay");
    assert_eq!(cue.values.len(), 5, "only the copied fixture grew");
    assert_eq!(cue.values[&Item::fixture(a)], row("Dimmer", 0.5));
}

#[test]
fn pat_40_a_copy_comes_with_the_palette_values_for_the_fixture() {
    let (mut e, a, b) = rig();
    let values = |v| BTreeMap::from([("Pan".to_string(), Value::Level(v))]);
    e.add_palette(Palette {
        id: 7,
        number: 7,
        window: Window::Positions,
        handle: 7,
        legend: "Look".into(),
        icon: None,
        global: BTreeMap::new(),
        shared: BTreeMap::new(),
        shared_cells: BTreeMap::new(),
        fixtures: BTreeMap::from([
            (Item::fixture(a), values(0.2)),
            (cell(a, 1), values(0.3)),
            (Item::fixture(b), values(0.4)),
        ]),
        effects: Vec::new(),
    })
    .unwrap();
    let c = e.patch_copy(&[a], None).unwrap()[0];
    let p = e.palette(7).unwrap();
    assert_eq!(p.fixtures[&Item::fixture(c)], values(0.2));
    assert_eq!(p.fixtures[&cell(c, 1)], values(0.3));
    assert_eq!(p.fixtures.len(), 5);
}

#[test]
fn pat_40_a_copy_joins_groups_and_effects_right_after_the_original() {
    let (mut e, a, b) = rig();
    select(&mut e, &[a, b]);
    e.record_group(5).unwrap();
    e.effect_apply(&EffectApply::Shape {
        shape: Shape::Wave(Wave::RampUp),
        attribute: None,
    })
    .unwrap();
    e.effect_set(EffectSet::Size(1.0)).unwrap();
    e.record_playback(&PlaybackRecord::at(1, 1)).unwrap();
    let pb = e.playbacks().next().unwrap().id;
    e.clear();
    e.clear();
    let new = e.patch_copy(&[a], None).unwrap();
    let c = new[0];
    assert_eq!(e.group(5).unwrap(), [a, c, b], "owner question 5");
    let entry = e.playback(pb).unwrap().cues[0].effects[0].clone();
    assert_eq!(
        entry.members,
        [Item::fixture(a), Item::fixture(c), Item::fixture(b)]
    );
}

#[test]
fn pat_40_effect_positions_of_what_hangs_do_not_change_while_the_copy_is_parked() {
    let (mut e, a, b) = rig();
    select(&mut e, &[a, b]);
    e.effect_apply(&EffectApply::Shape {
        shape: Shape::Wave(Wave::RampUp),
        attribute: None,
    })
    .unwrap();
    e.effect_set(EffectSet::Size(1.0)).unwrap();
    e.effect_set(EffectSet::Bpm(60.0)).unwrap();
    e.record_playback(&PlaybackRecord::at(1, 1)).unwrap();
    let pb = e.playbacks().next().unwrap().id;
    e.clear();
    e.clear();
    e.set_fader(pb, 1.0, 0.0).unwrap();
    e.tick(0.0);
    e.tick(0.3);
    let frame = (dmx(&e, 1, 1), dmx(&e, 1, 6));
    let c = e.patch_copy(&[a], None).unwrap()[0];
    e.tick(0.3);
    assert_eq!((dmx(&e, 1, 1), dmx(&e, 1, 6)), frame);
    let entry = &e.playback(pb).unwrap().cues[0].effects[0];
    assert_eq!(
        entry.count, 2,
        "the parked copy takes no place in the spread"
    );
    assert_eq!(entry.members.len(), 3);
    assert!(e.fixture(c).unwrap().parked);
}

#[test]
fn pat_40_the_programmer_and_the_selection_do_not_change() {
    let (mut e, a, b) = rig();
    select(&mut e, &[a, b]);
    e.record_group(5).unwrap();
    e.set_programmer_value(a, "Dimmer", 0.5).unwrap();
    let before = e.programmer_view();
    e.patch_copy(&[a, b], None).unwrap();
    assert!(e.programmer_view() == before);
}

#[test]
fn pat_40_a_move_gives_the_copy_its_address_and_it_plays_the_running_cue() {
    let (mut e, a, _) = rig();
    e.add_playback(Playback::new(
        1,
        1,
        1,
        BTreeMap::from([(Item::fixture(a), row("Dimmer", 1.0))]),
    ));
    e.set_fader(1, 1.0, 0.0).unwrap();
    e.tick(0.0);
    let c = e.patch_copy(&[a], None).unwrap()[0];
    e.tick(0.025);
    assert_eq!(dmx(&e, 1, 50), 0);
    e.patch_move_range(&[c], 1, 50).unwrap();
    e.tick(0.05);
    assert!(!e.fixture(c).unwrap().parked, "PAT-31");
    assert_eq!(dmx(&e, 1, 50), 255);
    assert_eq!(dmx(&e, 1, 1), 255);
}

#[test]
fn pat_40_the_message_says_they_are_parked() {
    let (mut e, a, b) = rig();
    e.drain_notices();
    e.patch_copy(&[a, b], None).unwrap();
    assert_eq!(
        e.drain_notices(),
        ["Copied 2 fixtures, parked: give them an address"]
    );
    e.patch_copy(&[a], None).unwrap();
    assert_eq!(
        e.drain_notices(),
        ["Copied 1 fixture, parked: give it an address"]
    );
}

#[test]
fn pat_40_one_undo_step_brings_it_all_back() {
    let (mut e, a, b) = rig();
    select(&mut e, &[a, b]);
    e.record_group(5).unwrap();
    let before = e.edit_state();
    let mut h = History::default();
    h.edit(&mut e, "Copy 2 fixtures", None, |e| {
        e.patch_copy(&[a, b], None).map(drop)
    })
    .unwrap();
    assert_eq!(e.fixtures().count(), 4);
    assert_eq!(h.undo(&mut e).unwrap(), "Copy 2 fixtures");
    assert!(e.edit_state() == before);
    assert_eq!(e.group(5).unwrap(), [a, b]);
}

// ---- PAT-41 -------------------------------------------------------------------------

#[test]
fn pat_41_pairs_in_number_order_trade_universe_and_address() {
    let mut e = engine();
    let left = add(&mut e, "wash", 2, 101, 1, 1);
    let right = add(&mut e, "wash", 2, 201, 2, 11);
    // Against number order on purpose: 101 pairs 201, 102 pairs 202.
    e.patch_swap_addresses(&[left[1], left[0]], &[right[0], right[1]])
        .unwrap();
    assert_eq!(place(&e, left[0]), (2, 11));
    assert_eq!(place(&e, left[1]), (2, 16));
    assert_eq!(place(&e, right[0]), (1, 1));
    assert_eq!(place(&e, right[1]), (1, 6));
}

#[test]
fn pat_41_different_sizes_refuse_and_nothing_changes() {
    let mut e = engine();
    let left = add(&mut e, "wash", 2, 101, 1, 1);
    let right = add(&mut e, "wash", 1, 201, 1, 11);
    let before = e.edit_state();
    assert!(matches!(
        e.patch_swap_addresses(&left, &right),
        Err(CoreError::SwapSizes { ids: 2, with: 1 })
    ));
    assert!(e.edit_state() == before);
}

#[test]
fn pat_41_a_fixture_in_both_ranges_or_a_bad_range_refuses() {
    let (mut e, a, b) = rig();
    let before = e.edit_state();
    assert!(matches!(
        e.patch_swap_addresses(&[a, b], &[b, a]),
        Err(CoreError::RepeatedFixture(_))
    ));
    assert!(matches!(
        e.patch_swap_addresses(&[], &[]),
        Err(CoreError::EmptySelection)
    ));
    assert!(matches!(
        e.patch_swap_addresses(&[a], &[9999]),
        Err(CoreError::UnknownFixture(9999))
    ));
    assert!(e.edit_state() == before);
}

#[test]
fn pat_41_a_parked_fixture_in_either_range_refuses_naming_it() {
    let (mut e, a, b) = rig();
    e.patch_park(&[b]).unwrap();
    let before = e.edit_state();
    for err in [
        e.patch_swap_addresses(&[a], &[b]).unwrap_err(),
        e.patch_swap_addresses(&[b], &[a]).unwrap_err(),
    ] {
        assert!(matches!(err, CoreError::Parked(102)));
        assert_eq!(err.to_string(), "102 is parked");
    }
    assert!(e.edit_state() == before);
}

#[test]
fn pat_41_the_new_footprints_are_checked_against_the_rest() {
    let mut e = engine();
    let wash = add(&mut e, "wash", 1, 101, 1, 1);
    let small = add(&mut e, "small", 1, 201, 1, 6);
    add(&mut e, "small", 1, 301, 1, 8); // outside both ranges, under the wash's new channels
    let before = e.edit_state();
    assert!(matches!(
        e.patch_swap_addresses(&wash, &small),
        Err(CoreError::PatchOverlap {
            number: 101,
            other: 301,
            ..
        })
    ));
    assert!(e.edit_state() == before);
}

#[test]
fn pat_41_the_new_footprints_are_checked_against_each_other() {
    let mut e = engine();
    let first = add(&mut e, "wash", 1, 101, 1, 1);
    let second = add(&mut e, "small", 1, 102, 1, 6);
    let third = add(&mut e, "small", 1, 201, 1, 8);
    let fourth = add(&mut e, "wash", 1, 202, 1, 10);
    // 101 goes to 8–12, 102 to 10–11: inside each other, though neither clashes with a fixture outside.
    let before = e.edit_state();
    assert!(matches!(
        e.patch_swap_addresses(&[first[0], second[0]], &[third[0], fourth[0]]),
        Err(CoreError::PatchOverlap { .. })
    ));
    assert!(e.edit_state() == before);
}

#[test]
fn pat_41_a_footprint_past_the_end_of_the_universe_refuses() {
    let mut e = engine();
    let wash = add(&mut e, "wash", 1, 101, 1, 1);
    let small = add(&mut e, "small", 1, 201, 1, 511);
    let before = e.edit_state();
    assert!(e.patch_swap_addresses(&wash, &small).is_err());
    assert!(e.edit_state() == before);
}

#[test]
fn pat_41_recorded_data_follows_the_id_and_old_channels_drop() {
    let mut e = engine();
    let wash = add(&mut e, "wash", 1, 101, 1, 1)[0];
    let small = add(&mut e, "small", 1, 201, 1, 20)[0];
    e.set_programmer_value(wash, "Zoom", 1.0).unwrap();
    e.set_programmer_value(small, "Zoom", 0.5).unwrap();
    e.add_playback(Playback::new(
        1,
        1,
        1,
        BTreeMap::from([(Item::fixture(wash), row("Dimmer", 1.0))]),
    ));
    e.set_fader(1, 1.0, 0.0).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, 1, 1), 255);
    assert_eq!(dmx(&e, 1, 4), 255);
    e.patch_swap_addresses(&[wash], &[small]).unwrap();
    e.tick(0.025);
    assert_eq!(dmx(&e, 1, 20), 255, "the wash's Dimmer follows it");
    assert_eq!(dmx(&e, 1, 23), 255, "and its programmer Zoom");
    assert_eq!(dmx(&e, 1, 1), 0, "the small one's Dimmer: dark");
    assert_eq!(dmx(&e, 1, 2), 128, "the small one's Zoom");
    assert_eq!(dmx(&e, 1, 4), 0, "the rest of the wash's old footprint");
    assert!(e.playback(1).unwrap().cues[0]
        .values
        .contains_key(&Item::fixture(wash)));
}

#[test]
fn pat_41_one_undo_step_brings_it_all_back() {
    let mut e = engine();
    let left = add(&mut e, "wash", 2, 101, 1, 1);
    let right = add(&mut e, "wash", 2, 201, 2, 1);
    let before = e.edit_state();
    let mut h = History::default();
    h.edit(&mut e, "Swap addresses", None, |e| {
        e.patch_swap_addresses(&left, &right)
    })
    .unwrap();
    assert_eq!(h.undo(&mut e).unwrap(), "Swap addresses");
    assert!(e.edit_state() == before);
}
