//! Profile save into the show (VS2-24): `specs/modules/fixture-library.md` LIB-25 and the
//! engine side of `specs/modules/ipc.md` IPC-39.
//!
//! API under test:
//! - `Profile::validate_for_save() -> Result<(), CoreError>`: LIB-02 plus slot order and overlap.
//! - `Engine::save_profile(key, Profile) -> Result<usize, CoreError>`: checked all or nothing,
//!   installs the show's copy and returns the number of patched fixtures using it. A refusal
//!   because of stored content is `CoreError::ProfileInUse(message)`.
//! - `Engine::set_programmer_level(attribute, value) -> Result<(), CoreError>` (IPC-39).

#![allow(
    clippy::indexing_slicing,
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used
)]

use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::palette::{Cell, Record, Window};
use cmdmx_core::patch::Fixture;
use cmdmx_core::playback::PlaybackRecord;
use cmdmx_core::profile::Profile;
use cmdmx_core::value::Value;
use cmdmx_core::CoreError;

const KEY: &str = "Showtec/Phantom 25 LED Spot";

/// The church Phantom 25 as a user profile: Pan and Tilt 16-bit, default 0.
const PHANTOM: &str = r#"{
  "manufacturer": "Showtec",
  "model": "Phantom 25 LED Spot",
  "source": "user",
  "modes": [
    {
      "name": "11 Channel",
      "footprint": 11,
      "attributes": [
        { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 1, "fine_channel": 2 },
        { "name": "Tilt", "group": "P", "kind": "Ltp", "channel": 3, "fine_channel": 4 },
        { "name": "Speed", "group": "S", "kind": "Ltp", "channel": 5 },
        { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 6 },
        { "name": "Shutter", "group": "I", "kind": "Ltp", "channel": 7 },
        { "name": "Colour Wheel", "group": "C", "kind": "Ltp", "channel": 8, "slots": [
          { "name": "Open", "from": 0, "to": 9 },
          { "name": "Red", "from": 10, "to": 19 }
        ] },
        { "name": "Gobo", "group": "G", "kind": "Ltp", "channel": 9, "slots": [
          { "name": "Open", "from": 0, "to": 7 },
          { "name": "Dots", "from": 8, "to": 15 }
        ] },
        { "name": "Gobo Rotation", "group": "G", "kind": "Ltp", "channel": 10 },
        { "name": "Prism", "group": "B", "kind": "Ltp", "channel": 11 }
      ]
    },
    {
      "name": "6 Channel",
      "footprint": 6,
      "attributes": [
        { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 1 },
        { "name": "Tilt", "group": "P", "kind": "Ltp", "channel": 2 },
        { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 3 },
        { "name": "Colour Wheel", "group": "C", "kind": "Ltp", "channel": 4 },
        { "name": "Gobo", "group": "G", "kind": "Ltp", "channel": 5 },
        { "name": "Prism", "group": "B", "kind": "Ltp", "channel": 6 }
      ]
    }
  ]
}"#;

fn phantom() -> Profile {
    Profile::from_json(PHANTOM).unwrap()
}

/// Fixtures 201–204 in "11 Channel" at addresses 1, 12, 23, 34 of universe 1.
fn church() -> Engine {
    let mut e = Engine::new(Settings::default());
    e.add_profile(KEY, phantom()).unwrap();
    for (i, id) in (201..=204).enumerate() {
        e.patch(Fixture {
            id,
            number: 0,
            name: String::new(),
            handle: None,
            profile: KEY.into(),
            mode: Some("11 Channel".into()),
            universe: 1,
            address: 1 + 11 * i as u16,
        })
        .unwrap();
    }
    e
}

fn select(e: &mut Engine, ids: &[u32]) {
    for &id in ids {
        e.select_fixture(id).unwrap();
    }
}

fn dmx(e: &Engine, address: u16) -> u8 {
    e.frame(1).and_then(|f| f.channel(address)).unwrap()
}

/// `PHANTOM` with one edit applied to its JSON.
fn edited(edit: impl FnOnce(&mut serde_json::Value)) -> Profile {
    let mut v: serde_json::Value = serde_json::from_str(PHANTOM).unwrap();
    edit(&mut v);
    serde_json::from_value(v).unwrap()
}

fn in_use(result: Result<usize, CoreError>) -> String {
    match result {
        Err(CoreError::ProfileInUse(message)) => message,
        other => panic!("expected ProfileInUse, got {other:?}"),
    }
}

// ---- LIB-25: validation on the save path ---------------------------------------------------

#[test]
fn lib_25_slot_from_above_to_is_refused() {
    let p = edited(|v| {
        v["modes"][0]["attributes"][6]["slots"][1] =
            serde_json::json!({"name":"Dots","from":15,"to":8})
    });
    let err = p.validate_for_save().unwrap_err().to_string();
    assert!(err.contains("Gobo") && err.contains("Dots"), "{err}");
    // Loading existing profiles is unchanged (LIB-25: only on this path).
    assert!(p.validate().is_ok());
}

#[test]
fn lib_25_overlapping_slots_of_one_channel_are_refused() {
    let p = edited(|v| v["modes"][0]["attributes"][5]["slots"][1]["from"] = 9.into());
    let err = p.validate_for_save().unwrap_err().to_string();
    assert!(
        err.contains("Colour Wheel") && err.contains("overlap"),
        "{err}"
    );
    // Touching ranges and gaps are fine; slots given out of order are sorted, not refused.
    let ok = edited(|v| {
        v["modes"][0]["attributes"][5]["slots"] = serde_json::json!([
            {"name":"Red","from":20,"to":29},
            {"name":"Open","from":0,"to":9}
        ])
    });
    assert!(ok.validate_for_save().is_ok());
}

#[test]
fn lib_25_lib_02_problems_are_refused_and_change_nothing() {
    let mut e = church();
    let bad = edited(|v| v["modes"][0]["attributes"][0]["default"] = 1.5.into());
    assert!(matches!(
        e.save_profile(KEY, bad),
        Err(CoreError::InvalidProfile { .. })
    ));
    let overlap = edited(|v| v["modes"][0]["attributes"][6]["slots"][1]["from"] = 7.into());
    assert!(e.save_profile(KEY, overlap).is_err());
    assert_eq!(e.profile(KEY), Some(&phantom()));
}

// ---- LIB-25: what the show uses must stay ----------------------------------------------

#[test]
fn lib_25_a_used_mode_keeps_its_name_and_footprint() {
    let mut e = church();
    let renamed = edited(|v| v["modes"][0]["name"] = "Eleven".into());
    let msg = in_use(e.save_profile(KEY, renamed));
    assert!(msg.contains("11 Channel") && msg.contains("201"), "{msg}");

    let grown = edited(|v| {
        v["modes"][0]["footprint"] = 12.into();
    });
    let msg = in_use(e.save_profile(KEY, grown));
    assert!(msg.contains("11 Channel") && msg.contains("11"), "{msg}");

    // A mode no fixture uses may change.
    let unused = edited(|v| v["modes"][1]["footprint"] = 7.into());
    assert_eq!(e.save_profile(KEY, unused), Ok(4));
    assert_eq!(e.profile(KEY).unwrap().modes[1].footprint, 7);
}

fn without_gobo() -> Profile {
    edited(|v| {
        let attrs = v["modes"][0]["attributes"].as_array_mut().unwrap();
        attrs.retain(|a| a["name"] != "Gobo");
    })
}

#[test]
fn lib_25_an_attribute_the_programmer_holds_may_not_disappear() {
    let mut e = church();
    select(&mut e, &[201]);
    e.set_programmer_value(201, "Gobo", Value::Level(0.1))
        .unwrap();
    let msg = in_use(e.save_profile(KEY, without_gobo()));
    assert_eq!(msg, "Gobo is used by the programmer");
    assert_eq!(e.profile(KEY), Some(&phantom()));
}

#[test]
fn lib_25_an_attribute_a_cue_holds_may_not_disappear() {
    let mut e = church();
    select(&mut e, &[202]);
    e.set_programmer_value(202, "Gobo", Value::Level(0.1))
        .unwrap();
    let id = e.record_playback(&PlaybackRecord::at(1, 2)).unwrap();
    e.clear();
    e.clear();
    let msg = in_use(e.save_profile(KEY, without_gobo()));
    assert_eq!(msg, format!("Gobo is used by cue 1 of list {id}"));
}

#[test]
fn lib_25_an_attribute_a_palette_holds_may_not_disappear() {
    let mut e = church();
    select(&mut e, &[203]);
    e.set_programmer_value(203, "Gobo", Value::Level(0.1))
        .unwrap();
    e.record_palette(&Record::at(Cell::new(Window::Beams, 1)))
        .unwrap();
    e.clear();
    e.clear();
    let msg = in_use(e.save_profile(KEY, without_gobo()));
    assert_eq!(msg, "Gobo is used by palette 1");
}

#[test]
fn lib_25_an_attribute_nothing_holds_may_go() {
    let mut e = church();
    assert_eq!(e.save_profile(KEY, without_gobo()), Ok(4));
    assert!(e.profile(KEY).unwrap().modes[0].attribute("Gobo").is_none());
}

#[test]
fn lib_25_a_profile_the_show_does_not_carry_is_installed_unused() {
    let mut e = church();
    let mut other = phantom();
    other.model = "Phantom 50 LED Spot".into();
    assert_eq!(
        e.save_profile("Showtec/Phantom 50 LED Spot", other.clone()),
        Ok(0)
    );
    assert_eq!(e.profile("Showtec/Phantom 50 LED Spot"), Some(&other));
}

// ---- LIB-25 scenario: new defaults reach the output on the next tick ------------------------

fn pan_tilt_half() -> Profile {
    edited(|v| {
        v["modes"][0]["attributes"][0]["default"] = 0.5.into();
        v["modes"][0]["attributes"][1]["default"] = 0.5.into();
    })
}

#[test]
fn lib_25_phantom_pan_tilt_default_half_outputs_half_on_the_next_tick() {
    let mut e = church();
    e.tick(0.0);
    assert_eq!((dmx(&e, 1), dmx(&e, 3)), (0, 0));
    assert_eq!(e.save_profile(KEY, pan_tilt_half()), Ok(4));
    e.tick(0.025);
    for base in [1u16, 12, 23, 34] {
        // 0.5 as 16 bit = 32768: coarse 128, fine 0.
        assert_eq!(
            [
                dmx(&e, base),
                dmx(&e, base + 1),
                dmx(&e, base + 2),
                dmx(&e, base + 3)
            ],
            [128, 0, 128, 0],
            "fixture at {base}"
        );
    }
}

#[test]
fn lib_25_a_value_a_source_proposed_is_held_not_reset_to_the_new_default() {
    let mut e = church();
    select(&mut e, &[201]);
    e.set_programmer_value(201, "Pan", Value::Level(0.25))
        .unwrap();
    e.tick(0.0);
    e.save_profile(KEY, pan_tilt_half()).unwrap();
    e.tick(0.025);
    // 0.25 × 65535 = 16383.75 → 16384: coarse 64, fine 0.
    assert_eq!((dmx(&e, 1), dmx(&e, 2)), (64, 0));
    assert_eq!(dmx(&e, 12), 128, "202 was never proposed: new default");
}

// ---- IPC-39: programmer.set in the engine --------------------------------------------------

#[test]
fn ipc_39_set_puts_an_absolute_level_on_every_target_that_has_it() {
    let mut e = church();
    select(&mut e, &[201, 202]);
    e.set_programmer_level("Pan", 0.5).unwrap();
    assert_eq!(e.programmer().value(201, "Pan"), Some(Value::Level(0.5)));
    assert_eq!(e.programmer().value(202, "Pan"), Some(Value::Level(0.5)));
    assert_eq!(e.programmer().value(203, "Pan"), None);
}

#[test]
fn ipc_39_set_refuses_bad_levels_unknown_attributes_and_colour() {
    let mut e = church();
    select(&mut e, &[201]);
    assert!(e.set_programmer_level("Pan", 1.5).is_err());
    assert!(e.set_programmer_level("Pan", -0.1).is_err());
    assert!(e.set_programmer_level("Zoom", 0.5).is_err());
    assert!(e.set_programmer_level("Colour", 0.5).is_err());
    assert_eq!(e.programmer().value(201, "Pan"), None);
    e.clear();
    assert!(
        e.set_programmer_level("Pan", 0.5).is_err(),
        "nothing selected"
    );
}
