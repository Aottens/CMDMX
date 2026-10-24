//! Patch core (VS2-10, VS2-27): `specs/modules/patch.md` PAT-01..07, 12..14, 19..24.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;

use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::patch::{FixtureId, Handle, PatchAdd};
use cmdmx_core::playback::Playback;
use cmdmx_core::profile::Profile;
use cmdmx_core::value::Value;
use cmdmx_core::CoreError;

/// Showtec LED Par 56 "6 Channel": RGB, Mode, Speed, Strobe, virtual dimmer.
const SHOWTEC: &str = r#"{ "manufacturer": "Showtec", "model": "LED Par 56", "source": "channel-list", "modes": [ {
  "name": "6 Channel", "footprint": 6, "virtual_dimmer": true,
  "colour": { "system": "RGB", "components": [ { "channel": 1 }, { "channel": 2 }, { "channel": 3 } ] },
  "attributes": [
    { "name": "Mode", "group": "S", "kind": "Ltp", "channel": 4 },
    { "name": "Speed", "group": "S", "kind": "Ltp", "channel": 5 },
    { "name": "Strobe", "group": "I", "kind": "Ltp", "channel": 6 } ] } ] }"#;

const DIMMER: &str =
    r#"{ "attributes": [ { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 } ] }"#;

/// Two modes: "Small" (Dimmer, Zoom; 2 ch) and "Big" (Dimmer, Pan, Tilt, Zoom; 4 ch).
const MOVER: &str = r#"{ "manufacturer": "T", "model": "Mover", "source": "channel-list", "modes": [
  { "name": "Small", "footprint": 2, "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "Zoom", "group": "B", "kind": "Ltp", "channel": 2 } ] },
  { "name": "Big", "footprint": 4, "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 2 },
    { "name": "Tilt", "group": "P", "kind": "Ltp", "channel": 3 },
    { "name": "Zoom", "group": "B", "kind": "Ltp", "channel": 4 } ] } ] }"#;

fn engine_with(settings: Settings) -> Engine {
    let mut e = Engine::new(settings);
    for (key, json) in [("showtec", SHOWTEC), ("dimmer", DIMMER), ("mover", MOVER)] {
        e.add_profile(key, Profile::from_json(json).unwrap())
            .unwrap();
    }
    e
}

fn engine() -> Engine {
    engine_with(Settings::default())
}

fn add(
    profile: &str,
    mode: Option<&str>,
    count: u32,
    universe: u16,
    address: Option<u16>,
    number: Option<u32>,
) -> PatchAdd {
    PatchAdd {
        profile: profile.into(),
        mode: mode.map(str::to_string),
        count,
        universe,
        address,
        number,
    }
}

fn id_of(e: &Engine, number: u32) -> FixtureId {
    e.fixture_by_number(number)
        .unwrap_or_else(|| panic!("no fixture numbered {number}"))
        .id
}

/// (number, universe, address) of every patched fixture, by number.
fn layout(e: &Engine) -> Vec<(u32, u16, u16)> {
    let mut v: Vec<_> = e
        .fixtures()
        .map(|f| (f.number, f.universe, f.address))
        .collect();
    v.sort();
    v
}

fn run(e: &mut Engine, line: &str) -> Result<(), CoreError> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    e.execute_command(&tokens)
}

fn dmx(e: &Engine, universe: u16, address: u16) -> u8 {
    e.frame(universe).and_then(|f| f.channel(address)).unwrap()
}

// ---- PAT-01 -------------------------------------------------------------------------

#[test]
fn pat_01_add_places_count_fixtures_back_to_back() {
    let mut e = engine();
    let ids = e
        .patch_add(&add(
            "showtec",
            Some("6 Channel"),
            6,
            1,
            Some(37),
            Some(221),
        ))
        .unwrap();
    assert_eq!(ids.len(), 6);
    assert_eq!(
        layout(&e),
        [
            (221, 1, 37),
            (222, 1, 43),
            (223, 1, 49),
            (224, 1, 55),
            (225, 1, 61),
            (226, 1, 67)
        ]
    );
    for (id, number) in ids.iter().zip(221..) {
        let f = e.fixture(*id).unwrap();
        assert_eq!((f.number, f.profile.as_str()), (number, "showtec"));
        assert_eq!(f.mode.as_deref(), Some("6 Channel"));
    }
}

// ---- PAT-02 -------------------------------------------------------------------------

#[test]
fn pat_02_overlap_is_refused_as_a_whole_with_the_first_clash() {
    let mut e = engine();
    e.patch_add(&add("dimmer", None, 1, 1, Some(67), Some(301)))
        .unwrap();
    let before = layout(&e);
    let err = e
        .patch_add(&add("showtec", None, 6, 1, Some(37), Some(221)))
        .unwrap_err();
    assert_eq!(
        err,
        CoreError::PatchOverlap {
            number: 226,
            other: 301,
            universe: 1,
            address: 67
        }
    );
    assert_eq!(err.to_string(), "226 would overlap 301 at 1.67");
    assert_eq!(layout(&e), before, "no partial patch");
}

#[test]
fn pat_02_running_past_512_is_refused_as_a_whole() {
    let mut e = engine();
    // 507..512 fits one par, the second would need 513..518.
    assert!(matches!(
        e.patch_add(&add("showtec", None, 2, 1, Some(507), Some(1))),
        Err(CoreError::AddressOutOfRange { .. })
    ));
    assert!(e.fixtures().next().is_none());
}

#[test]
fn pat_02_reusing_a_number_is_refused() {
    let mut e = engine();
    e.patch_add(&add("dimmer", None, 1, 1, Some(1), Some(301)))
        .unwrap();
    assert_eq!(
        e.patch_add(&add("dimmer", None, 3, 1, Some(10), Some(299))),
        Err(CoreError::NumberInUse(301))
    );
    assert_eq!(layout(&e), [(301, 1, 1)]);
}

#[test]
fn pat_02_number_zero_is_refused() {
    // 0 means "same as the id" on the low-level patch, so it is never a user number.
    let mut e = engine();
    assert_eq!(
        e.patch_add(&add("dimmer", None, 1, 1, Some(1), Some(0))),
        Err(CoreError::InvalidNumber)
    );
    e.patch_add(&add("dimmer", None, 1, 1, Some(1), Some(1)))
        .unwrap();
    assert_eq!(
        e.patch_renumber(id_of(&e, 1), 0),
        Err(CoreError::InvalidNumber)
    );
}

#[test]
fn pat_02_count_zero_is_refused() {
    let mut e = engine();
    assert!(e
        .patch_add(&add("dimmer", None, 0, 1, Some(1), Some(1)))
        .is_err());
}

// ---- PAT-03 -------------------------------------------------------------------------

#[test]
fn pat_03_without_address_takes_the_first_free_run_for_all() {
    let mut e = engine();
    e.patch_add(&add("dimmer", None, 1, 1, Some(1), Some(1)))
        .unwrap();
    e.patch_add(&add("dimmer", None, 1, 1, Some(20), Some(2)))
        .unwrap();
    // 2..=19 holds exactly three 6-channel pars.
    e.patch_add(&add("showtec", None, 3, 1, None, Some(10)))
        .unwrap();
    // The next one no longer fits before 20.
    e.patch_add(&add("showtec", None, 1, 1, None, Some(20)))
        .unwrap();
    assert_eq!(
        layout(&e),
        [
            (1, 1, 1),
            (2, 1, 20),
            (10, 1, 2),
            (11, 1, 8),
            (12, 1, 14),
            (20, 1, 21)
        ]
    );
}

#[test]
fn pat_03_without_number_continues_after_the_highest() {
    let mut e = engine();
    e.patch_add(&add("dimmer", None, 1, 1, Some(1), Some(226)))
        .unwrap();
    e.patch_add(&add("dimmer", None, 1, 1, Some(2), Some(5)))
        .unwrap();
    e.patch_add(&add("dimmer", None, 2, 1, Some(3), None))
        .unwrap();
    assert_eq!(
        layout(&e),
        [(5, 1, 2), (226, 1, 1), (227, 1, 3), (228, 1, 4)]
    );
    // An empty patch starts at 1.
    let mut fresh = engine();
    fresh
        .patch_add(&add("dimmer", None, 1, 2, None, None))
        .unwrap();
    assert_eq!(layout(&fresh), [(1, 2, 1)]);
}

#[test]
fn pat_03_no_free_run_is_refused() {
    let mut e = engine();
    // 86 pars = 516 channels: no run of that size in one universe.
    assert_eq!(
        e.patch_add(&add("showtec", None, 86, 1, None, Some(1))),
        Err(CoreError::NoFreeAddresses { universe: 1 })
    );
    e.patch_add(&add("dimmer", None, 1, 1, Some(1), Some(u32::MAX)))
        .unwrap();
    assert_eq!(
        e.patch_add(&add("dimmer", None, 1, 1, None, None)),
        Err(CoreError::NoFreeNumbers)
    );
}

// ---- PAT-04 -------------------------------------------------------------------------

#[test]
fn pat_04_move_checks_clashes_but_not_its_own_channels() {
    let mut e = engine();
    e.patch_add(&add("showtec", None, 2, 1, Some(1), Some(221)))
        .unwrap();
    let a = id_of(&e, 221);
    e.set_programmer_value(a, "Dimmer", 1.0).unwrap();
    // Onto 222 (7..12): refused.
    assert_eq!(
        e.patch_move(a, 1, 5),
        Err(CoreError::PatchOverlap {
            number: 221,
            other: 222,
            universe: 1,
            address: 7
        })
    );
    // Shifting over its own old channels is fine.
    e.patch_move(a, 1, 13).unwrap();
    e.patch_move(a, 1, 14).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, 1, 14), 255);
    // Another universe; the programmer value follows the id.
    e.patch_move(a, 2, 1).unwrap();
    assert_eq!(layout(&e), [(221, 2, 1), (222, 1, 7)]);
    assert_eq!(e.programmer().value(a, "Dimmer"), Some(Value::Level(1.0)));
    e.tick(0.025);
    assert_eq!(
        dmx(&e, 2, 1),
        255,
        "white × virtual dimmer at the new place"
    );
    assert_eq!(dmx(&e, 1, 14), 0, "old channels are free and dark");
    assert!(matches!(
        e.patch_move(a, 2, 510),
        Err(CoreError::AddressOutOfRange { .. })
    ));
    assert_eq!(
        e.patch_move(999, 1, 100),
        Err(CoreError::UnknownFixture(999))
    );
}

// ---- PAT-05 -------------------------------------------------------------------------

#[test]
fn pat_05_remove_takes_the_fixture_out_of_everything() {
    let mut e = engine();
    e.patch_add(&add("dimmer", None, 3, 1, Some(1), Some(1)))
        .unwrap();
    let (one, two, three) = (id_of(&e, 1), id_of(&e, 2), id_of(&e, 3));
    run(&mut e, "1 Thro 3 Enter").unwrap();
    e.record_group(1).unwrap();
    run(&mut e, "1 Thro 3 @ 100 Enter").unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, 1, 2), 255);

    e.patch_remove(&[two]).unwrap();
    assert!(e.fixture(two).is_none());
    assert_eq!(e.programmer().selection(), [one, three]);
    assert_eq!(e.group(1), Some(&[one, three][..]));
    assert!(!e.programmer().in_programmer(two));
    e.tick(0.025);
    assert_eq!(dmx(&e, 1, 2), 0, "its channel drops to 0");
    assert_eq!(dmx(&e, 1, 1), 255);

    // Its number is free again; its id is never reused.
    let again = e
        .patch_add(&add("dimmer", None, 1, 1, Some(2), Some(2)))
        .unwrap();
    assert_ne!(again[0], two);
    assert!(again[0] > three);
    assert_eq!(e.patch_remove(&[two]), Err(CoreError::UnknownFixture(two)));
}

// ---- PAT-06 -------------------------------------------------------------------------

#[test]
fn pat_06_renumber_changes_only_the_number() {
    let mut e = engine();
    e.patch_add(&add("dimmer", None, 8, 1, Some(1), Some(201)))
        .unwrap();
    let ids: Vec<FixtureId> = (201..=208).map(|n| id_of(&e, n)).collect();
    run(&mut e, "201 Thro 208 Enter").unwrap();
    e.record_group(3).unwrap();
    run(&mut e, "201 @ 50 Enter").unwrap();
    for (n, id) in (1201..).zip(&ids) {
        e.patch_renumber(*id, n).unwrap();
    }
    assert_eq!(e.group(3), Some(ids.as_slice()));
    assert_eq!(
        e.programmer().value(ids[0], "Dimmer"),
        Some(Value::Level(0.5))
    );
    assert!(e.fixture_by_number(201).is_none());
    run(&mut e, "1201 Enter").unwrap();
    assert_eq!(e.programmer().selection(), [ids[0]]);
    assert_eq!(
        e.patch_renumber(ids[0], 1202),
        Err(CoreError::NumberInUse(1202))
    );
    assert_eq!(e.fixture(ids[0]).unwrap().number, 1201);
}

// ---- PAT-07 -------------------------------------------------------------------------

#[test]
fn pat_07_virtual_dimmer_fixture_gets_a_dimmer() {
    let mut e = engine();
    e.patch_add(&add("showtec", None, 6, 1, Some(37), Some(221)))
        .unwrap();
    run(&mut e, "221 Thro 226 @ 40 Enter").unwrap();
    assert_eq!(
        e.programmer().value(id_of(&e, 226), "Dimmer"),
        Some(Value::Level(0.4))
    );
}

// ---- PAT-12 -------------------------------------------------------------------------

#[test]
fn pat_12_command_line_takes_user_numbers() {
    let mut e = engine();
    e.patch_add(&add("dimmer", None, 3, 1, Some(1), Some(500)))
        .unwrap();
    e.patch_add(&add("dimmer", None, 3, 1, Some(10), Some(100)))
        .unwrap();
    // Ids follow patch order, numbers do not: the line works on numbers.
    assert!(id_of(&e, 500) < id_of(&e, 100));
    run(&mut e, "102 Thro 100 And 501 Enter").unwrap();
    assert_eq!(
        e.programmer().selection(),
        [
            id_of(&e, 102),
            id_of(&e, 101),
            id_of(&e, 100),
            id_of(&e, 501)
        ]
    );
    assert_eq!(
        run(&mut e, "1 Thro 99 Enter"),
        Err(CoreError::EmptySelection)
    );
}

#[test]
fn pat_12_low_level_patch_numbers_fixtures_by_id() {
    // Scenario files and test rigs patch with an explicit id and no number.
    let mut e = engine();
    e.patch(cmdmx_core::patch::Fixture {
        id: 201,
        number: 0,
        name: String::new(),
        profile: "dimmer".into(),
        mode: None,
        universe: 1,
        address: 1,
        handle: None,
    })
    .unwrap();
    assert_eq!(e.fixture(201).unwrap().number, 201);
    assert_eq!(id_of(&e, 201), 201);
    // A later add never takes an id that is already in use.
    let ids = e
        .patch_add(&add("dimmer", None, 1, 1, Some(2), Some(1)))
        .unwrap();
    assert!(ids[0] > 201);
}

// ---- PAT-13 -------------------------------------------------------------------------

#[test]
fn pat_13_change_mode_keeps_matching_attributes_and_drops_the_rest() {
    let mut e = engine();
    e.patch_add(&add("mover", Some("Big"), 1, 1, Some(1), Some(101)))
        .unwrap();
    let m = id_of(&e, 101);
    e.set_programmer_value(m, "Dimmer", 1.0).unwrap();
    e.set_programmer_value(m, "Pan", 0.5).unwrap();
    e.add_playback(Playback::new(
        1,
        1,
        1,
        BTreeMap::from([(
            m,
            BTreeMap::from([
                ("Tilt".to_string(), 0.3.into()),
                ("Zoom".to_string(), 0.7.into()),
            ]),
        )]),
    ));
    e.set_fader(1, 1.0, 0.0).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, 1, 3), 77, "Tilt 0.3 on offset 3");
    e.drain_warnings();
    e.patch_change_mode(m, "Small").unwrap();
    assert_eq!(e.fixture(m).unwrap().mode.as_deref(), Some("Small"));
    assert_eq!(e.programmer().value(m, "Dimmer"), Some(Value::Level(1.0)));
    assert_eq!(e.programmer().value(m, "Pan"), None);
    let warnings: Vec<String> = e.drain_warnings().into_iter().map(|w| w.message).collect();
    assert!(
        warnings.iter().any(|w| w.contains("Pan")) && warnings.iter().any(|w| w.contains("Tilt")),
        "dropped attributes are reported: {warnings:?}"
    );
    e.tick(0.025);
    assert_eq!(dmx(&e, 1, 2), 179, "Zoom 0.7 now on offset 2");
    assert_eq!(
        dmx(&e, 1, 3),
        0,
        "channels beyond the smaller footprint are free"
    );
    assert_eq!(
        e.patch_change_mode(m, "Huge"),
        Err(CoreError::UnknownMode {
            profile: "mover".into(),
            mode: "Huge".into()
        })
    );
}

#[test]
fn pat_13_bigger_footprint_is_checked_like_a_patch() {
    let mut e = engine();
    e.patch_add(&add("mover", Some("Small"), 2, 1, Some(1), Some(101)))
        .unwrap();
    let first = id_of(&e, 101);
    assert_eq!(
        e.patch_change_mode(first, "Big"),
        Err(CoreError::PatchOverlap {
            number: 101,
            other: 102,
            universe: 1,
            address: 3
        })
    );
    assert_eq!(e.fixture(first).unwrap().mode.as_deref(), Some("Small"));
    // The last one has room.
    e.patch_change_mode(id_of(&e, 102), "Big").unwrap();
}

// ---- PAT-14 -------------------------------------------------------------------------

#[test]
fn pat_14_handles_follow_patch_order_and_can_move_or_swap() {
    let mut e = engine_with(Settings {
        handles_per_page: 3,
        ..Settings::default()
    });
    assert_eq!(Settings::default().handles_per_page, 40);
    e.patch_add(&add("dimmer", None, 4, 1, Some(1), Some(1)))
        .unwrap();
    let handle = |e: &Engine, n: u32| e.fixture_by_number(n).unwrap().handle;
    let h = |page, slot| Some(Handle { page, slot });
    assert_eq!(
        (1..=4).map(|n| handle(&e, n)).collect::<Vec<_>>(),
        [h(1, 1), h(1, 2), h(1, 3), h(2, 1)]
    );
    // To a free slot.
    e.patch_handle(id_of(&e, 4), Handle { page: 3, slot: 2 })
        .unwrap();
    assert_eq!(handle(&e, 4), h(3, 2));
    // Onto an occupied slot: swap.
    e.patch_handle(id_of(&e, 1), Handle { page: 1, slot: 3 })
        .unwrap();
    assert_eq!((handle(&e, 1), handle(&e, 3)), (h(1, 3), h(1, 1)));
    // The next patch takes the first free slot again.
    e.patch_add(&add("dimmer", None, 1, 1, Some(10), Some(5)))
        .unwrap();
    assert_eq!(handle(&e, 5), h(2, 1));
    for bad in [
        Handle { page: 0, slot: 1 },
        Handle { page: 1, slot: 0 },
        Handle { page: 1, slot: 4 },
    ] {
        assert_eq!(
            e.patch_handle(id_of(&e, 2), bad),
            Err(CoreError::InvalidHandle {
                page: bad.page,
                slot: bad.slot
            })
        );
    }
}

/// PAT-09: `patch.name` sets the legend; nothing else changes.
#[test]
fn pat_09_name_sets_the_legend_only() {
    let mut e = engine();
    let ids = e
        .patch_add(&add("showtec", None, 3, 1, Some(37), Some(221)))
        .unwrap();
    let before = e.fixture(ids[1]).cloned().unwrap();
    e.patch_name(ids[1], "Front left").unwrap();
    let after = e.fixture(ids[1]).cloned().unwrap();
    assert_eq!(after.name, "Front left");
    assert_eq!(
        cmdmx_core::patch::Fixture {
            name: String::new(),
            ..after
        },
        before
    );
    assert_eq!(e.patch_name(999, "x"), Err(CoreError::UnknownFixture(999)));
}

/// PAT-09: the daemon asks whether the show already has a profile under a key.
#[test]
fn pat_09_has_profile() {
    let e = engine();
    assert!(e.has_profile("showtec"));
    assert!(!e.has_profile("Showtec/LED Par 56"));
}

// ---- PAT-19..24: range edits (VS2-27) ------------------------------------------------

/// Ids of the fixtures with these numbers, in the order given.
fn ids_of(e: &Engine, numbers: &[u32]) -> Vec<FixtureId> {
    numbers.iter().map(|&n| id_of(e, n)).collect()
}

/// Six 6-channel pars 221–226 at 1.37–1.67 and a dimmer 301 at 1.100.
fn six_pars() -> Engine {
    let mut e = engine();
    e.patch_add(&add("showtec", None, 6, 1, Some(37), Some(221)))
        .unwrap();
    e.patch_add(&add("dimmer", None, 1, 1, Some(100), Some(301)))
        .unwrap();
    e
}

fn snapshot(e: &Engine) -> Vec<cmdmx_core::patch::Fixture> {
    e.fixtures().cloned().collect()
}

#[test]
fn pat_19_empty_unknown_or_repeated_ids_refuse_every_range_command() {
    let mut e = six_pars();
    let before = snapshot(&e);
    let a = id_of(&e, 221);
    let b = id_of(&e, 222);
    type Case<'a> = (&'a [FixtureId], fn(FixtureId) -> CoreError);
    let cases: [Case; 3] = [
        (&[], |_| CoreError::EmptySelection),
        (&[a, 999], |_| CoreError::UnknownFixture(999)),
        (&[a, b, a], CoreError::RepeatedFixture),
    ];
    for (ids, err) in cases {
        assert_eq!(e.patch_move_range(ids, 2, 1), Err(err(a)), "{ids:?}");
        assert_eq!(e.patch_renumber_range(ids, 900), Err(err(a)));
        assert_eq!(e.patch_name_range(ids, "Par 1"), Err(err(a)));
        assert_eq!(e.patch_change_mode_range(ids, "6 Channel"), Err(err(a)));
    }
    assert_eq!(snapshot(&e), before, "nothing changes");
}

#[test]
fn pat_20_move_places_the_range_back_to_back_in_number_order() {
    let mut e = six_pars();
    // Listed out of order: the range is worked in number order.
    let ids = ids_of(&e, &[226, 221, 224, 222, 225, 223]);
    e.patch_move_range(&ids, 2, 101).unwrap();
    assert_eq!(
        layout(&e),
        [
            (221, 2, 101),
            (222, 2, 107),
            (223, 2, 113),
            (224, 2, 119),
            (225, 2, 125),
            (226, 2, 131),
            (301, 1, 100)
        ]
    );
}

#[test]
fn pat_20_move_closes_gaps_and_ignores_the_ranges_own_channels() {
    let mut e = six_pars();
    // 221, 223, 225 (at 37, 49, 61) back to back from 1.200, then shifted over their own
    // new channels by two.
    let ids = ids_of(&e, &[221, 223, 225]);
    e.patch_move_range(&ids, 1, 200).unwrap();
    e.patch_move_range(&ids, 1, 202).unwrap();
    let placed: Vec<_> = layout(&e)
        .into_iter()
        .filter(|(n, _, _)| [221, 223, 225].contains(n))
        .collect();
    assert_eq!(placed, [(221, 1, 202), (223, 1, 208), (225, 1, 214)]);
}

#[test]
fn pat_20_move_refuses_a_clash_outside_the_range_or_past_512() {
    let mut e = six_pars();
    let before = snapshot(&e);
    let ids = ids_of(&e, &[221, 222, 223, 224, 225, 226]);
    // From 1.70 the range runs 70..105: 226 (100..105) hits 301 at 1.100.
    assert_eq!(
        e.patch_move_range(&ids, 1, 70),
        Err(CoreError::PatchOverlap {
            number: 226,
            other: 301,
            universe: 1,
            address: 100
        })
    );
    // From 2.480 the sixth (226) would start at 510 and run past 512.
    assert_eq!(
        e.patch_move_range(&ids, 2, 480),
        Err(CoreError::AddressOutOfRange {
            fixture: 226,
            universe: 2,
            address: 510
        })
    );
    assert_eq!(snapshot(&e), before, "nothing changes");
}

#[test]
fn pat_21_renumber_keeps_the_order_and_closes_gaps() {
    let mut e = engine();
    e.patch_add(&add("showtec", None, 6, 1, Some(1), Some(1221)))
        .unwrap();
    let all = ids_of(&e, &[1221, 1222, 1223, 1224, 1225, 1226]);
    let mut shuffled = all.clone();
    shuffled.reverse();
    e.patch_renumber_range(&shuffled, 1301).unwrap();
    let numbers: Vec<u32> = all
        .iter()
        .map(|&id| e.fixture(id).unwrap().number)
        .collect();
    assert_eq!(numbers, [1301, 1302, 1303, 1304, 1305, 1306]);
    // 1301, 1303, 1305 from 1401 → 1401, 1402, 1403.
    let odd = [all[0], all[2], all[4]];
    e.patch_renumber_range(&odd, 1401).unwrap();
    let numbers: Vec<u32> = odd
        .iter()
        .map(|&id| e.fixture(id).unwrap().number)
        .collect();
    assert_eq!(numbers, [1401, 1402, 1403]);
    assert_eq!(e.fixture_by_number(1402).map(|f| f.id), Some(all[2]));
}

#[test]
fn pat_21_own_old_numbers_are_free_but_outside_numbers_refuse() {
    let mut e = engine();
    e.patch_add(&add("showtec", None, 6, 1, Some(1), Some(201)))
        .unwrap();
    let ids = ids_of(&e, &[201, 202, 203, 204, 205, 206]);
    e.patch_renumber_range(&ids, 203).unwrap();
    let numbers: Vec<u32> = ids
        .iter()
        .map(|&id| e.fixture(id).unwrap().number)
        .collect();
    assert_eq!(numbers, [203, 204, 205, 206, 207, 208]);

    let mut e = engine();
    e.patch_add(&add("showtec", None, 6, 1, Some(1), Some(201)))
        .unwrap();
    e.patch_add(&add("dimmer", None, 1, 1, Some(100), Some(207)))
        .unwrap();
    let before = snapshot(&e);
    let ids = ids_of(&e, &[201, 202, 203, 204, 205, 206]);
    assert_eq!(
        e.patch_renumber_range(&ids, 203),
        Err(CoreError::NumberInUse(207))
    );
    assert_eq!(
        e.patch_renumber_range(&ids, 0),
        Err(CoreError::InvalidNumber)
    );
    assert_eq!(snapshot(&e), before, "nothing changes");
}

#[test]
fn pat_22_change_mode_refuses_a_range_that_no_longer_fits_in_place() {
    let mut e = engine();
    // Six 2-channel movers back to back at 1.1–1.11: Big (4 channels) cannot fit in place.
    e.patch_add(&add("mover", Some("Small"), 6, 1, Some(1), Some(221)))
        .unwrap();
    let before = snapshot(&e);
    let ids = ids_of(&e, &[226, 225, 224, 223, 222, 221]);
    assert_eq!(
        e.patch_change_mode_range(&ids, "Big"),
        Err(CoreError::PatchOverlap {
            number: 221,
            other: 222,
            universe: 1,
            address: 3
        })
    );
    assert_eq!(snapshot(&e), before, "nothing changes");
    // Moved apart first, the whole range changes.
    e.patch_move(ids[0], 1, 200).unwrap();
    e.patch_move(ids[1], 1, 210).unwrap();
    e.patch_move(ids[2], 1, 220).unwrap();
    e.patch_move(ids[3], 1, 230).unwrap();
    e.patch_move(ids[4], 1, 240).unwrap();
    e.patch_move(ids[5], 1, 250).unwrap();
    e.patch_change_mode_range(&ids, "Big").unwrap();
    assert!(ids
        .iter()
        .all(|&id| e.fixture(id).unwrap().mode.as_deref() == Some("Big")));
}

#[test]
fn pat_22_change_mode_refuses_naming_the_first_fixture_without_the_mode() {
    let mut e = engine();
    e.patch_add(&add("mover", Some("Small"), 1, 1, Some(1), Some(101)))
        .unwrap();
    e.patch_add(&add("showtec", None, 2, 1, Some(101), Some(102)))
        .unwrap();
    let before = snapshot(&e);
    let ids = ids_of(&e, &[103, 101, 102]);
    assert_eq!(
        e.patch_change_mode_range(&ids, "Big"),
        Err(CoreError::FixtureLacksMode {
            fixture: 102,
            mode: "Big".into()
        })
    );
    assert_eq!(snapshot(&e), before, "nothing changes");
}

#[test]
fn pat_22_dropped_values_give_one_warning_for_the_range() {
    let mut e = engine();
    for (address, number) in [(1, 101), (11, 102), (21, 103)] {
        e.patch_add(&add(
            "mover",
            Some("Big"),
            1,
            1,
            Some(address),
            Some(number),
        ))
        .unwrap();
    }
    let ids = ids_of(&e, &[101, 102, 103]);
    for &id in &ids {
        e.set_programmer_value(id, "Pan", 0.5).unwrap();
    }
    e.add_playback(Playback::new(
        1,
        1,
        1,
        BTreeMap::from([(ids[1], BTreeMap::from([("Tilt".to_string(), 0.3.into())]))]),
    ));
    e.drain_warnings();
    e.patch_change_mode_range(&ids, "Small").unwrap();
    let warnings: Vec<String> = e.drain_warnings().into_iter().map(|w| w.message).collect();
    assert_eq!(warnings, ["Dropped Pan, Tilt on 3 fixtures"]);
    assert!(ids
        .iter()
        .all(|&id| e.programmer().value(id, "Pan").is_none()));
}

#[test]
fn pat_23_numbered_names_count_up_in_number_order() {
    let mut e = engine();
    e.patch_add(&add("showtec", None, 10, 1, Some(1), Some(221)))
        .unwrap();
    let mut ids = ids_of(&e, &[221, 222, 223, 224, 225, 226, 227, 228, 229, 230]);
    let names = |e: &Engine, ids: &[FixtureId]| -> Vec<String> {
        let mut by_number: Vec<_> = ids
            .iter()
            .map(|&id| e.fixture(id).unwrap())
            .map(|f| (f.number, f.name.clone()))
            .collect();
        by_number.sort();
        by_number.into_iter().map(|(_, n)| n).collect()
    };
    ids.reverse();
    let six = &ids[4..]; // 221–226, listed backwards
    e.patch_name_range(six, "Par 1").unwrap();
    assert_eq!(
        names(&e, six),
        ["Par 1", "Par 2", "Par 3", "Par 4", "Par 5", "Par 6"]
    );
    e.patch_name_range(six, "P40").unwrap();
    assert_eq!(names(&e, six), ["P40", "P41", "P42", "P43", "P44", "P45"]);
    e.patch_name_range(&ids, "Par 01").unwrap();
    assert_eq!(
        names(&e, &ids),
        [
            "Par 01", "Par 02", "Par 03", "Par 04", "Par 05", "Par 06", "Par 07", "Par 08",
            "Par 09", "Par 10"
        ]
    );
    e.patch_name_range(six, "Front").unwrap();
    assert!(names(&e, six).iter().all(|n| n == "Front"));
    e.patch_name_range(six, "").unwrap();
    assert!(names(&e, six).iter().all(String::is_empty));
}

#[test]
fn pat_24_remove_refuses_repeated_unknown_or_empty_ids() {
    let mut e = six_pars();
    let before = snapshot(&e);
    let a = id_of(&e, 221);
    assert_eq!(e.patch_remove(&[a, a]), Err(CoreError::RepeatedFixture(a)));
    assert_eq!(
        e.patch_remove(&[a, 999]),
        Err(CoreError::UnknownFixture(999))
    );
    assert_eq!(e.patch_remove(&[]), Err(CoreError::EmptySelection));
    assert_eq!(snapshot(&e), before, "nothing changes");
    e.patch_remove(&ids_of(&e, &[221, 222])).unwrap();
    assert!(e.fixture(a).is_none());
}
