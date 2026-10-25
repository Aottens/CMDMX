//! Profile model v2 (VS2-08): `specs/modules/fixture-library.md` LIB-01..03, 16, 17.
//! LIB-03 on the frame is also a scenario: `tests/scenarios/LIB-03-*.json`.

#![allow(
    clippy::indexing_slicing,
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used
)]

use cmdmx_core::attribute::{AttributeDef, AttributeGroup, AttributeKind};
use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::patch::Fixture;
use cmdmx_core::playback::Playback;
use cmdmx_core::profile::{ColourSystem, Profile, TargetColour};
use cmdmx_core::programmer::Item;
use cmdmx_core::CoreError;
use std::collections::BTreeMap;

/// Shehds 19x15-like "24 Channel" mode: three RGBW cells, a colour macro with slots,
/// physical ranges (LIB-07 shape, LIB-15 slots, LIB-16 ranges).
const SHEHDS: &str = r#"{
  "manufacturer": "Shehds",
  "model": "LED Beam+Wash 19x15W RGBW Zoom",
  "source": "qlcplus:Shehds/Shehds-LED-Beam-Wash-19x15W-RGBW-Zoom.qxf",
  "modes": [
    {
      "name": "16 Channel",
      "footprint": 16,
      "attributes": [
        { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
        { "name": "White", "group": "C", "kind": "Ltp", "channel": 5, "emitter": true }
      ],
      "colour": { "system": "RGB", "components": [ { "channel": 2 }, { "channel": 3 }, { "channel": 4 } ] }
    },
    {
      "name": "24 Channel",
      "footprint": 24,
      "attributes": [
        { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 1, "fine_channel": 2, "default": 0.5, "locate": 0.5 },
        { "name": "Tilt", "group": "P", "kind": "Ltp", "channel": 3, "fine_channel": 4, "default": 0.5, "locate": 0.5 },
        { "name": "Pan/Tilt Speed", "group": "S", "kind": "Ltp", "channel": 5 },
        { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 6, "locate": 1.0, "highlight": 1.0 },
        { "name": "Strobe", "group": "I", "kind": "Ltp", "channel": 7 },
        { "name": "Zoom", "group": "B", "kind": "Ltp", "channel": 20 },
        { "name": "Colour Macro", "group": "C", "kind": "Ltp", "channel": 21, "slots": [
          { "name": "Open", "from": 0, "to": 7 },
          { "name": "Red", "from": 8, "to": 15, "colour": { "r": 1.0, "g": 0.0, "b": 0.0 } }
        ] }
      ],
      "cells": [
        { "colour": { "system": "RGB", "components": [ { "channel": 8 }, { "channel": 9 }, { "channel": 10 } ] },
          "attributes": [ { "name": "White", "group": "C", "kind": "Ltp", "channel": 11, "emitter": true } ] },
        { "colour": { "system": "RGB", "components": [ { "channel": 12 }, { "channel": 13 }, { "channel": 14 } ] },
          "attributes": [ { "name": "White", "group": "C", "kind": "Ltp", "channel": 15, "emitter": true } ] },
        { "colour": { "system": "RGB", "components": [ { "channel": 16 }, { "channel": 17 }, { "channel": 18 } ] },
          "attributes": [ { "name": "White", "group": "C", "kind": "Ltp", "channel": 19, "emitter": true } ] }
      ],
      "pan_degrees": 540.0,
      "tilt_degrees": 180.0,
      "zoom_degrees": [8.0, 55.0]
    }
  ]
}"#;

/// Showtec LED Par 56 "6 Channel": RGB, no dimmer channel, a Mode channel.
const SHOWTEC: &str = r#"{
  "manufacturer": "Showtec",
  "model": "LED Par 56",
  "source": "qlcplus:Showtec/Showtec-LED-Par-56.qxf",
  "modes": [ {
    "name": "6 Channel",
    "footprint": 6,
    "virtual_dimmer": true,
    "colour": { "system": "RGB", "components": [ { "channel": 1 }, { "channel": 2 }, { "channel": 3 } ] },
    "attributes": [
      { "name": "Mode", "group": "S", "kind": "Ltp", "channel": 4, "default": 0.0 },
      { "name": "Speed", "group": "S", "kind": "Ltp", "channel": 5 },
      { "name": "Strobe", "group": "I", "kind": "Ltp", "channel": 6 }
    ]
  } ]
}"#;

fn shehds() -> Profile {
    Profile::from_json(SHEHDS).expect("valid Shehds profile")
}

/// A one-mode profile (mode "M", footprint 8) with the given attributes, as JSON.
fn one_mode(attributes: &str, extra: &str) -> String {
    format!(
        r#"{{ "manufacturer": "T", "model": "T", "source": "channel-list",
              "modes": [ {{ "name": "M", "footprint": 8, "attributes": [{attributes}] {extra} }} ] }}"#
    )
}

fn assert_rejected(json: &str, needle: &str) {
    match Profile::from_json(json) {
        Err(CoreError::InvalidProfile { mode, problem }) => {
            assert_eq!(mode, "M");
            assert!(
                problem.contains(needle),
                "problem '{problem}' should mention '{needle}'"
            );
        }
        other => panic!("expected InvalidProfile mentioning '{needle}', got {other:?}"),
    }
}

// ---- LIB-01: model and JSON round trip ---------------------------------------

#[test]
fn lib_01_full_profile_round_trips_unchanged() {
    for json in [SHEHDS, SHOWTEC] {
        let p = Profile::from_json(json).unwrap();
        let again = Profile::from_json(&p.to_json()).unwrap();
        assert_eq!(again, p);
        assert_eq!(again.to_json(), p.to_json(), "serialisation is stable");
    }
}

#[test]
fn lib_01_keys_come_in_a_fixed_order() {
    let json = shehds().to_json();
    let at = |key: &str| {
        json.find(&format!("\"{key}\""))
            .unwrap_or_else(|| panic!("{key} missing"))
    };
    assert!(at("manufacturer") < at("model"));
    assert!(at("model") < at("source"));
    assert!(at("source") < at("modes"));
    assert!(at("name") < at("footprint"));
}

#[test]
fn lib_01_model_carries_modes_cells_slots_and_defaults() {
    let p = shehds();
    assert_eq!(p.manufacturer, "Shehds");
    assert_eq!(p.modes.len(), 2);
    let m24 = p.mode(Some("24 Channel")).unwrap();
    assert_eq!(m24.footprint, 24);
    assert_eq!(m24.cells.len(), 3);
    assert_eq!(
        m24.cells[1].colour.as_ref().unwrap().components[0].channel,
        12
    );
    let pan = m24.attribute("Pan").unwrap();
    assert_eq!(pan.fine_channel, Some(2));
    assert_eq!(pan.default, 0.5);
    assert_eq!(pan.locate, Some(0.5));
    assert_eq!(m24.attribute("Dimmer").unwrap().highlight, Some(1.0));
    let slots = &m24.attribute("Colour Macro").unwrap().slots;
    assert_eq!(slots[1].name, "Red");
    assert_eq!((slots[1].from, slots[1].to), (8, 15));
    assert_eq!(
        slots[1].colour,
        Some(TargetColour {
            r: 1.0,
            g: 0.0,
            b: 0.0
        })
    );
    // Omitted fields take their defaults.
    let speed = m24.attribute("Pan/Tilt Speed").unwrap();
    assert_eq!(speed.default, 0.0);
    assert_eq!(
        (speed.locate, speed.highlight, speed.emitter),
        (None, None, false)
    );
    assert!(speed.slots.is_empty());

    let par = Profile::from_json(SHOWTEC).unwrap();
    let m = par.mode(None).unwrap();
    assert!(m.virtual_dimmer);
    assert_eq!(m.colour.as_ref().unwrap().system, ColourSystem::Rgb);
    assert!(!shehds().mode(Some("16 Channel")).unwrap().virtual_dimmer);
}

#[test]
fn lib_01_mode_lookup_by_name_or_first() {
    let p = shehds();
    assert_eq!(p.mode(None).unwrap().name, "16 Channel");
    assert_eq!(p.mode(Some("24 Channel")).unwrap().name, "24 Channel");
    assert!(p.mode(Some("99 Channel")).is_none());
}

#[test]
fn lib_01_minimal_scenario_profile_is_a_single_default_mode() {
    let p = Profile::from_json(
        r#"{ "attributes": [
              { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
              { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 2, "fine_channel": 3 } ] }"#,
    )
    .unwrap();
    assert_eq!(p.modes.len(), 1);
    let m = p.mode(None).unwrap();
    assert_eq!(m.name, "default");
    assert_eq!(m.footprint, 3, "footprint from the highest channel used");
    assert_eq!(m.attributes.len(), 2);
    // The same profile built in code.
    let built = Profile::from_attributes(vec![
        AttributeDef::new("Dimmer", AttributeGroup::I, AttributeKind::Htp, 1),
        AttributeDef {
            fine_channel: Some(3),
            ..AttributeDef::new("Pan", AttributeGroup::P, AttributeKind::Ltp, 2)
        },
    ]);
    assert_eq!(built, p);
    assert_eq!(Profile::from_json(&p.to_json()).unwrap(), p);
}

#[test]
fn lib_01_fixture_uses_its_patched_mode() {
    let mut e = Engine::new(Settings::default());
    e.add_profile("shehds", shehds()).unwrap();
    let fx = |id, mode: Option<&str>, address| Fixture {
        id,
        number: 0,
        name: String::new(),
        handle: None,
        profile: "shehds".into(),
        mode: mode.map(str::to_string),
        universe: 1,
        address,
    };
    // 24-channel mode: Dimmer on offset 6.
    e.patch(fx(1, Some("24 Channel"), 1)).unwrap();
    // No mode = the first mode (16 Channel): Dimmer on offset 1.
    e.patch(fx(2, None, 25)).unwrap();
    // The 24-channel footprint does not fit at 490 (490 + 23 = 513).
    assert!(matches!(
        e.patch(fx(3, Some("24 Channel"), 490)),
        Err(CoreError::AddressOutOfRange { .. })
    ));
    assert_eq!(
        e.patch(fx(4, Some("99 Channel"), 100)),
        Err(CoreError::UnknownMode {
            profile: "shehds".into(),
            mode: "99 Channel".into()
        })
    );
    e.add_playback(Playback::new(
        1,
        1,
        1,
        BTreeMap::from([
            (
                Item::fixture(1),
                BTreeMap::from([("Dimmer".to_string(), 1.0.into())]),
            ),
            (
                Item::fixture(2),
                BTreeMap::from([("Dimmer".to_string(), 1.0.into())]),
            ),
        ]),
    ));
    e.set_fader(1, 1.0, 0.0).unwrap();
    e.tick(0.0);
    let f = e.frame(1).unwrap();
    assert_eq!(f.channel(6), Some(255), "fixture 1 Dimmer at 1 + 6 - 1");
    assert_eq!(f.channel(25), Some(255), "fixture 2 Dimmer at 25 + 1 - 1");
}

// ---- LIB-02: load validation ---------------------------------------------------

#[test]
fn lib_02_attribute_outside_footprint_is_rejected() {
    assert_rejected(
        &one_mode(
            r#"{ "name": "Zoom", "group": "B", "kind": "Ltp", "channel": 9 }"#,
            "",
        ),
        "Zoom",
    );
    assert_rejected(
        &one_mode(
            r#"{ "name": "Zoom", "group": "B", "kind": "Ltp", "channel": 0 }"#,
            "",
        ),
        "Zoom",
    );
    assert_rejected(
        &one_mode(
            r#"{ "name": "Pan", "group": "P", "kind": "Ltp", "channel": 8, "fine_channel": 9 }"#,
            "",
        ),
        "Pan",
    );
}

#[test]
fn lib_02_colour_component_outside_footprint_is_rejected() {
    assert_rejected(
        &one_mode(
            "",
            r#", "colour": { "system": "RGB", "components": [ { "channel": 7 }, { "channel": 8 }, { "channel": 9 } ] }"#,
        ),
        "9",
    );
    assert_rejected(
        &one_mode(
            "",
            r#", "cells": [ { "colour": { "system": "CMY", "components": [ { "channel": 1 }, { "channel": 2 }, { "channel": 12 } ] }, "attributes": [] } ]"#,
        ),
        "12",
    );
}

#[test]
fn lib_02_two_claims_on_one_channel_are_rejected() {
    // Two attributes.
    assert_rejected(
        &one_mode(
            r#"{ "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
               { "name": "Strobe", "group": "I", "kind": "Ltp", "channel": 1 }"#,
            "",
        ),
        "1",
    );
    // A fine byte on another attribute's channel.
    assert_rejected(
        &one_mode(
            r#"{ "name": "Pan", "group": "P", "kind": "Ltp", "channel": 1, "fine_channel": 2 },
               { "name": "Tilt", "group": "P", "kind": "Ltp", "channel": 2 }"#,
            "",
        ),
        "2",
    );
    // A colour component on an attribute's channel.
    assert_rejected(
        &one_mode(
            r#"{ "name": "White", "group": "C", "kind": "Ltp", "channel": 3, "emitter": true }"#,
            r#", "colour": { "system": "RGB", "components": [ { "channel": 1 }, { "channel": 2 }, { "channel": 3 } ] }"#,
        ),
        "3",
    );
    // A cell channel on a mode attribute's channel.
    assert_rejected(
        &one_mode(
            r#"{ "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 4 }"#,
            r#", "cells": [ { "attributes": [ { "name": "White", "group": "C", "kind": "Ltp", "channel": 4, "emitter": true } ] } ]"#,
        ),
        "4",
    );
}

#[test]
fn lib_02_values_outside_unit_range_are_rejected() {
    for field in [
        r#""default": 1.5"#,
        r#""locate": -0.1"#,
        r#""highlight": 2.0"#,
    ] {
        assert_rejected(
            &one_mode(
                &format!(
                    r#"{{ "name": "Iris", "group": "B", "kind": "Ltp", "channel": 1, {field} }}"#
                ),
                "",
            ),
            "Iris",
        );
    }
}

#[test]
fn lib_02_malformed_json_is_an_error_not_a_panic() {
    assert!(matches!(
        Profile::from_json("{ not json"),
        Err(CoreError::InvalidProfile { .. })
    ));
}

#[test]
fn lib_02_rejected_profile_never_reaches_the_patch() {
    let mut bad = Profile::from_attributes(vec![
        AttributeDef::new("Dimmer", AttributeGroup::I, AttributeKind::Htp, 1),
        AttributeDef::new("Strobe", AttributeGroup::I, AttributeKind::Ltp, 1),
    ]);
    assert!(matches!(
        bad.validate(),
        Err(CoreError::InvalidProfile { .. })
    ));
    let mut e = Engine::new(Settings::default());
    assert!(matches!(
        e.add_profile("bad", bad.clone()),
        Err(CoreError::InvalidProfile { .. })
    ));
    assert_eq!(
        e.patch(Fixture {
            id: 1,
            number: 0,
            name: String::new(),
            handle: None,
            profile: "bad".into(),
            mode: None,
            universe: 1,
            address: 1,
        }),
        Err(CoreError::UnknownProfile("bad".into()))
    );
    // Fixed, it is accepted.
    bad.modes[0].attributes[1].channel = 2;
    bad.modes[0].footprint = 2;
    assert_eq!(bad.validate(), Ok(()));
}

// ---- LIB-03: an attribute nobody proposed outputs its default ------------------

#[test]
fn lib_03_defaults_are_on_the_frame_from_the_first_tick() {
    let mut e = Engine::new(Settings::default());
    e.add_profile("shehds", shehds()).unwrap();
    e.patch(Fixture {
        id: 1,
        number: 0,
        name: String::new(),
        handle: None,
        profile: "shehds".into(),
        mode: Some("24 Channel".into()),
        universe: 1,
        address: 1,
    })
    .unwrap();
    e.tick(0.0);
    let f = e.frame(1).unwrap();
    // Pan/Tilt default 0.5, 16-bit → 0x80 0x00.
    assert_eq!(
        (f.channel(1), f.channel(2), f.channel(3), f.channel(4)),
        (Some(0x80), Some(0x00), Some(0x80), Some(0x00))
    );
    assert_eq!(f.channel(6), Some(0), "HTP Dimmer stays 0 (ENG-07)");
}

#[test]
fn lib_03_released_ltp_holds_its_last_value_not_the_default() {
    let mut e = Engine::new(Settings::default());
    e.add_profile("shehds", shehds()).unwrap();
    e.patch(Fixture {
        id: 1,
        number: 0,
        name: String::new(),
        handle: None,
        profile: "shehds".into(),
        mode: Some("24 Channel".into()),
        universe: 1,
        address: 1,
    })
    .unwrap();
    e.add_playback(Playback::new(
        1,
        1,
        1,
        BTreeMap::from([(
            Item::fixture(1),
            BTreeMap::from([("Pan".to_string(), 1.0.into())]),
        )]),
    ));
    e.set_fader(1, 1.0, 0.0).unwrap();
    e.tick(0.0);
    e.set_fader(1, 0.0, 0.025).unwrap();
    e.tick(0.025);
    let f = e.frame(1).unwrap();
    assert_eq!((f.channel(1), f.channel(2)), (Some(255), Some(255)));
}

// ---- LIB-16: physical ranges -----------------------------------------------------

#[test]
fn lib_16_physical_ranges_are_stored() {
    let p = shehds();
    let m24 = p.mode(Some("24 Channel")).unwrap();
    assert_eq!(m24.pan_degrees, Some(540.0));
    assert_eq!(m24.tilt_degrees, Some(180.0));
    assert_eq!(m24.zoom_degrees, Some([8.0, 55.0]));
    let m16 = p.mode(Some("16 Channel")).unwrap();
    assert_eq!(
        (m16.pan_degrees, m16.tilt_degrees, m16.zoom_degrees),
        (None, None, None)
    );
}

// ---- LIB-17: emitters are generic ---------------------------------------------------

const RGBW_WW_CW_PLUS: &str = r#"
  { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
  { "name": "Warm White", "group": "C", "kind": "Ltp", "channel": 5, "emitter": true },
  { "name": "Cold White", "group": "C", "kind": "Ltp", "channel": 6, "emitter": true },
  { "name": "Royal Blue", "group": "C", "kind": "Ltp", "channel": 7, "emitter": true },
  { "name": "CTO", "group": "C", "kind": "Ltp", "channel": 8 }"#;

#[test]
fn lib_17_emitters_are_found_by_flag_not_by_name() {
    let p = Profile::from_json(&one_mode(
        RGBW_WW_CW_PLUS,
        r#", "colour": { "system": "RGB", "components": [ { "channel": 2 }, { "channel": 3 }, { "channel": 4 } ] }"#,
    ))
    .unwrap();
    let names: Vec<&str> = p
        .mode(None)
        .unwrap()
        .emitters()
        .map(|a| a.name.as_str())
        .collect();
    assert_eq!(names, ["Warm White", "Cold White", "Royal Blue"]);
}

#[test]
fn lib_17_emitter_outside_group_c_is_rejected() {
    assert_rejected(
        &one_mode(
            r#"{ "name": "UV", "group": "I", "kind": "Ltp", "channel": 1, "emitter": true }"#,
            "",
        ),
        "UV",
    );
}

#[test]
fn lib_17_two_emitters_with_one_name_are_rejected() {
    assert_rejected(
        &one_mode(
            r#"{ "name": "White", "group": "C", "kind": "Ltp", "channel": 1, "emitter": true },
               { "name": "White", "group": "C", "kind": "Ltp", "channel": 2, "emitter": true }"#,
            "",
        ),
        "White",
    );
    // Same name in different cells is fine (one White per cell, LIB-07).
    assert!(shehds().validate().is_ok());
}

#[test]
fn lib_02_a_typo_gets_a_precise_message() {
    let json = SHOWTEC.replace(
        r#""kind": "Ltp", "channel": 5"#,
        r#""kind": "Ltp", "channel": "five""#,
    );
    match Profile::from_json(&json) {
        Err(CoreError::InvalidProfile { problem, .. }) => {
            assert!(problem.contains("line"), "serde position in '{problem}'")
        }
        other => panic!("expected InvalidProfile, got {other:?}"),
    }
}
