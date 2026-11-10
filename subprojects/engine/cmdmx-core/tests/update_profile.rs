//! Update a patched profile from the library (PTC-05): `specs/modules/patch.md` PAT-42.
//!
//! API under test: `Engine::update_profile(key, Profile) -> Result<usize, CoreError>` replaces
//! the show's copy under LIB-25's fit checks and returns how many patched fixtures use it.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::patch::PatchAdd;
use cmdmx_core::profile::Profile;
use cmdmx_core::undo::History;
use cmdmx_core::value::Value;
use cmdmx_core::CoreError;

const KEY: &str = "T/Spot";

const SPOT: &str = r#"{ "manufacturer": "T", "model": "Spot", "source": "user", "modes": [
  { "name": "Std", "footprint": 3, "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 2 },
    { "name": "Gobo", "group": "G", "kind": "Ltp", "channel": 3, "slots": [
      { "name": "Open", "from": 0, "to": 9 },
      { "name": "Dots", "from": 10, "to": 19 } ] } ] },
  { "name": "Small", "footprint": 1, "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 } ] } ] }"#;

fn spot() -> Profile {
    Profile::from_json(SPOT).unwrap()
}

/// `SPOT` with one edit applied to its JSON: what the library holds now.
fn library(edit: impl FnOnce(&mut serde_json::Value)) -> Profile {
    let mut v: serde_json::Value = serde_json::from_str(SPOT).unwrap();
    edit(&mut v);
    serde_json::from_value(v).unwrap()
}

/// Two Spots in "Std" at 1 and 4 of universe 1, ids in the returned order.
fn rig() -> (Engine, Vec<u32>) {
    let mut e = Engine::new(Settings::default());
    e.add_profile(KEY, spot()).unwrap();
    let ids = e
        .patch_add(&PatchAdd {
            profile: KEY.into(),
            mode: Some("Std".into()),
            count: 2,
            universe: 1,
            address: Some(1),
            number: Some(1),
        })
        .unwrap();
    (e, ids)
}

fn in_use(result: Result<usize, CoreError>) -> String {
    match result {
        Err(CoreError::ProfileInUse(message)) => message,
        other => panic!("expected ProfileInUse, got {other:?}"),
    }
}

#[test]
fn pat_42_the_shows_copy_is_replaced_and_output_follows_on_the_next_tick() {
    let (mut e, _) = rig();
    e.tick(0.0);
    assert_eq!(e.frame(1).and_then(|f| f.channel(2)), Some(0));
    let newer = library(|v| v["modes"][0]["attributes"][1]["default"] = 0.5.into());
    assert_eq!(e.update_profile(KEY, newer.clone()), Ok(2));
    assert_eq!(e.profile(KEY), Some(&newer));
    e.tick(0.025);
    assert_eq!(e.frame(1).and_then(|f| f.channel(2)), Some(128));
    assert_eq!(e.frame(1).and_then(|f| f.channel(5)), Some(128));
}

#[test]
fn pat_42_a_used_mode_keeps_its_name_and_footprint_or_nothing_changes() {
    let (mut e, _) = rig();
    let renamed = library(|v| v["modes"][0]["name"] = "Eleven".into());
    let msg = in_use(e.update_profile(KEY, renamed));
    assert!(msg.contains("Std"), "{msg}");
    let grown = library(|v| v["modes"][0]["footprint"] = 4.into());
    let msg = in_use(e.update_profile(KEY, grown));
    assert!(msg.contains("Std"), "{msg}");
    assert_eq!(e.profile(KEY), Some(&spot()));
}

#[test]
fn pat_42_a_mode_nothing_uses_may_change() {
    let (mut e, _) = rig();
    let newer = library(|v| v["modes"][1]["footprint"] = 2.into());
    assert_eq!(e.update_profile(KEY, newer), Ok(2));
    assert_eq!(e.profile(KEY).unwrap().modes[1].footprint, 2);
}

#[test]
fn pat_42_a_stored_attribute_may_not_disappear() {
    let (mut e, ids) = rig();
    e.select_fixture(ids[0]).unwrap();
    e.set_programmer_value(ids[0], "Gobo", Value::Level(0.1))
        .unwrap();
    let newer = library(|v| {
        let attrs = v["modes"][0]["attributes"].as_array_mut().unwrap();
        attrs.retain(|a| a["name"] != "Gobo");
    });
    assert_eq!(
        in_use(e.update_profile(KEY, newer)),
        "Gobo is used by the programmer"
    );
    assert_eq!(e.profile(KEY), Some(&spot()));
}

/// LIB-25's slot order and overlap checks guard the edit form; a QLC+ file may overlap and is
/// valid in the library (LIB-02), so PAT-42 names only the fit checks.
#[test]
fn pat_42_a_library_copy_with_overlapping_slots_still_updates() {
    let (mut e, _) = rig();
    let newer = library(|v| v["modes"][0]["attributes"][2]["slots"][1]["from"] = 5.into());
    assert!(newer.validate().is_ok() && newer.validate_for_save().is_err());
    assert_eq!(e.update_profile(KEY, newer.clone()), Ok(2));
    assert_eq!(e.profile(KEY), Some(&newer));
}

#[test]
fn pat_42_an_identical_copy_says_up_to_date_and_adds_no_step() {
    let (mut e, _) = rig();
    let mut h = History::default();
    h.edit(&mut e, "Update profile Spot", None, |e| {
        e.update_profile(KEY, spot())
    })
    .unwrap();
    assert_eq!(h.len(), 0);
    let notices = e.drain_notices();
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert!(notices[0].contains("already up to date"), "{notices:?}");
    assert!(notices[0].contains("Spot"), "{notices:?}");
}

#[test]
fn pat_42_undo_brings_the_old_copy_back() {
    let (mut e, _) = rig();
    let mut h = History::default();
    let newer = library(|v| v["modes"][0]["attributes"][1]["default"] = 0.5.into());
    h.edit(&mut e, "Update profile Spot", None, |e| {
        e.update_profile(KEY, newer)
    })
    .unwrap();
    assert_eq!(h.len(), 1);
    h.undo(&mut e).unwrap();
    assert_eq!(e.profile(KEY), Some(&spot()));
}

#[test]
fn pat_42_a_profile_the_show_does_not_carry_is_refused() {
    let (mut e, _) = rig();
    assert_eq!(
        e.update_profile("T/Other", spot()),
        Err(CoreError::UnknownProfile("T/Other".into()))
    );
    assert!(!e.has_profile("T/Other"));
}
