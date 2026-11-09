//! Auto Groups on Add (PTC-06): `specs/modules/patch.md` PAT-43.
//!
//! API under test: `Engine::auto_group(&mut self, profile_key, &[FixtureId])`, run right after a
//! `patch_add` with the ids it returned. It cannot fail, so an accepted Add never half-applies.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::palette::MAX_LEGEND;
use cmdmx_core::patch::{FixtureId, PatchAdd};
use cmdmx_core::profile::Profile;

fn profile(model: &str) -> Profile {
    Profile::from_json(&format!(
        r#"{{ "manufacturer": "T", "model": "{model}", "source": "user", "modes": [
          {{ "name": "Std", "footprint": 1, "attributes": [
            {{ "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 }} ] }} ] }}"#
    ))
    .unwrap()
}

fn engine() -> Engine {
    let mut e = Engine::new(Settings::default());
    e.add_profile("wash", profile("Wash")).unwrap();
    e.add_profile("spot", profile("Spot")).unwrap();
    e
}

/// Patch `count` of `key` on the next free numbers and addresses.
fn patch(e: &mut Engine, key: &str, count: u32) -> Vec<FixtureId> {
    e.patch_add(&PatchAdd {
        profile: key.into(),
        mode: Some("Std".into()),
        count,
        universe: 1,
        address: None,
        number: None,
    })
    .unwrap()
}

fn add(e: &mut Engine, key: &str, count: u32) -> Vec<FixtureId> {
    let ids = patch(e, key, count);
    e.auto_group(key, &ids);
    ids
}

/// (number, legend, members) of every group.
fn groups(e: &Engine) -> Vec<(u32, String, Vec<FixtureId>)> {
    e.groups()
        .keys()
        .map(|&id| {
            (
                id,
                e.group_legend(id).unwrap_or_default().to_string(),
                e.group(id).unwrap(),
            )
        })
        .collect()
}

#[test]
fn pat_43_a_count_of_two_or_more_makes_the_type_group_then_the_count_group() {
    let mut e = engine();
    let ids = add(&mut e, "wash", 4);
    assert_eq!(
        groups(&e),
        [
            (1, "All Wash".to_string(), ids.clone()),
            (2, "4 Wash".to_string(), ids)
        ]
    );
}

#[test]
fn pat_43_a_new_all_group_gets_every_fixture_of_the_profile_in_number_order() {
    let mut e = engine();
    let earlier = patch(&mut e, "wash", 1);
    let _other_type = patch(&mut e, "spot", 1);
    let ids = add(&mut e, "wash", 2);
    let all: Vec<FixtureId> = earlier.into_iter().chain(ids.iter().copied()).collect();
    assert_eq!(
        groups(&e),
        [
            (1, "All Wash".to_string(), all),
            (2, "2 Wash".to_string(), ids)
        ]
    );
}

#[test]
fn pat_43_an_existing_all_group_is_appended_to_and_a_new_count_group_made() {
    let mut e = engine();
    let first = add(&mut e, "wash", 2);
    let second = add(&mut e, "wash", 3);
    let all: Vec<FixtureId> = first.iter().chain(&second).copied().collect();
    assert_eq!(
        groups(&e),
        [
            (1, "All Wash".to_string(), all),
            (2, "2 Wash".to_string(), first),
            (3, "3 Wash".to_string(), second)
        ]
    );
}

#[test]
fn pat_43_a_count_of_one_appends_to_all_and_makes_no_group() {
    let mut e = engine();
    let first = add(&mut e, "wash", 2);
    let one = add(&mut e, "wash", 1);
    let all: Vec<FixtureId> = first.iter().chain(&one).copied().collect();
    assert_eq!(
        groups(&e),
        [
            (1, "All Wash".to_string(), all),
            (2, "2 Wash".to_string(), first)
        ]
    );
}

#[test]
fn pat_43_a_count_of_one_without_an_all_group_makes_nothing() {
    let mut e = engine();
    add(&mut e, "wash", 1);
    assert!(e.groups().is_empty());
}

#[test]
fn pat_43_numbers_are_the_first_free_ones() {
    let mut e = engine();
    let held = patch(&mut e, "spot", 1);
    e.select_fixture(held[0]).unwrap();
    e.record_group(1).unwrap();
    e.record_group(3).unwrap();
    e.clear_selection();
    let ids = add(&mut e, "wash", 2);
    assert_eq!(e.group(2), Some(ids.clone()));
    assert_eq!(e.group_legend(2), Some("All Wash"));
    assert_eq!(e.group(4), Some(ids));
    assert_eq!(e.group_legend(4), Some("2 Wash"));
}

#[test]
fn pat_43_only_exactly_that_legend_counts_as_the_all_group() {
    let mut e = engine();
    let held = patch(&mut e, "spot", 1);
    e.select_fixture(held[0]).unwrap();
    e.record_group(1).unwrap();
    e.set_group_legend(1, "all wash").unwrap();
    let ids = add(&mut e, "wash", 2);
    assert_eq!(e.group(1), Some(held));
    assert_eq!(e.group_legend(2), Some("All Wash"));
    assert_eq!(e.group(2), Some(ids));
}

#[test]
fn pat_43_each_type_has_its_own_all_group() {
    let mut e = engine();
    let wash = add(&mut e, "wash", 2);
    let spot = add(&mut e, "spot", 2);
    let g = groups(&e);
    assert_eq!(g[0], (1, "All Wash".to_string(), wash.clone()));
    assert_eq!(g[1], (2, "2 Wash".to_string(), wash));
    assert_eq!(g[2], (3, "All Spot".to_string(), spot.clone()));
    assert_eq!(g[3], (4, "2 Spot".to_string(), spot));
}

/// A legend has a maximum length; a long model name must not fail an Add already made.
#[test]
fn pat_43_a_long_model_name_is_cut_to_the_legend_limit_and_still_found_again() {
    let mut e = engine();
    let long = "A very long model name that goes on and on and on";
    e.add_profile("long", profile(long)).unwrap();
    let first = add(&mut e, "long", 2);
    let second = add(&mut e, "long", 2);
    let g = groups(&e);
    assert_eq!(g.len(), 3, "{g:?}");
    assert!(g
        .iter()
        .all(|(_, legend, _)| legend.chars().count() <= MAX_LEGEND));
    assert!(g[0].1.starts_with("All A very long"));
    let all: Vec<FixtureId> = first.iter().chain(&second).copied().collect();
    assert_eq!(g[0].2, all);
}
