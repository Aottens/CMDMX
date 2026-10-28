//! Automatic emitter mixing (VS2-44, IDEA-32, ADR-010): LIB-36..38 (profile data) and
//! ENG-20..24 (the mix in the frame writer). ENG-22 is also a scenario in `tests/scenarios/`.
//!
//! API these tests fix:
//! - Profile JSON: `"automix": true` on a mode (written only when true), `"output": 2.0` on an
//!   emitter attribute (0.25..=4.0, written only when not 1.0).
//! - `cmdmx_core::colour::rgb_equivalent(cie: [f32; 2]) -> Option<[f32; 3]>` (ENG-20; `None` =
//!   not mixable).
//! - `cmdmx_core::colour::default_cie(name: &str) -> Option<[f32; 2]>` (LIB-37).

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use cmdmx_core::colour::{default_cie, rgb_equivalent, TargetColour};
use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::patch::Fixture;
use cmdmx_core::profile::Profile;
use cmdmx_core::CoreError;

const D65: [f32; 2] = [0.3127, 0.3290];
const LIME: [f32; 2] = [0.41, 0.55];
const AMBER: [f32; 2] = [0.5752, 0.4242];
const UV: [f32; 2] = [0.174, 0.005];

/// An RGBW par (Dimmer 1, RGB 2–4, White 5) with auto-mix on and White at D65, its output
/// factor given as `output` (`""` = default).
fn rgbw(automix: bool, output: &str) -> String {
    format!(
        r#"{{ "manufacturer": "T", "model": "RGBW", "source": "channel-list", "modes": [ {{
  "name": "5ch", "footprint": 5, "automix": {automix},
  "colour": {{ "system": "RGB", "components": [ {{ "channel": 2 }}, {{ "channel": 3 }}, {{ "channel": 4 }} ] }},
  "attributes": [
    {{ "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 }},
    {{ "name": "White", "group": "C", "kind": "Ltp", "channel": 5, "emitter": true,
       "cie": [0.3127, 0.3290] {output} }} ] }} ] }}"#
    )
}

/// U'King RGBL: no dimmer channel (RGB 1–3, Lime 4), auto-mix on.
const RGBL: &str = r#"{ "manufacturer": "T", "model": "RGBL", "source": "channel-list", "modes": [ {
  "name": "4ch", "footprint": 4, "virtual_dimmer": true, "automix": true,
  "colour": { "system": "RGB", "components": [ { "channel": 1 }, { "channel": 2 }, { "channel": 3 } ] },
  "attributes": [ { "name": "Lime", "group": "C", "kind": "Ltp", "channel": 4, "emitter": true,
                    "cie": [0.41, 0.55] } ] } ] }"#;

/// RGB + UV (with a violet `cie`) + Amber without `cie`: neither is mixable.
const RGBUV: &str = r#"{ "manufacturer": "T", "model": "RGBUV", "source": "channel-list", "modes": [ {
  "name": "6ch", "footprint": 6, "automix": true,
  "colour": { "system": "RGB", "components": [ { "channel": 2 }, { "channel": 3 }, { "channel": 4 } ] },
  "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "UV", "group": "C", "kind": "Ltp", "channel": 5, "emitter": true, "cie": [0.174, 0.005] },
    { "name": "Amber", "group": "C", "kind": "Ltp", "channel": 6, "emitter": true } ] } ] }"#;

/// Three-cell RGBW wash with a master dimmer on 1, auto-mix on (cells RGBW 2–5, 6–9, 10–13).
const CELLS: &str = r#"{ "manufacturer": "T", "model": "Cells", "source": "channel-list", "modes": [ {
  "name": "13ch", "footprint": 13, "automix": true,
  "attributes": [ { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 } ],
  "cells": [
    { "colour": { "system": "RGB", "components": [ { "channel": 2 }, { "channel": 3 }, { "channel": 4 } ] },
      "attributes": [ { "name": "White", "group": "C", "kind": "Ltp", "channel": 5, "emitter": true, "cie": [0.3127, 0.3290] } ] },
    { "colour": { "system": "RGB", "components": [ { "channel": 6 }, { "channel": 7 }, { "channel": 8 } ] },
      "attributes": [ { "name": "White", "group": "C", "kind": "Ltp", "channel": 9, "emitter": true, "cie": [0.3127, 0.3290] } ] },
    { "colour": { "system": "RGB", "components": [ { "channel": 10 }, { "channel": 11 }, { "channel": 12 } ] },
      "attributes": [ { "name": "White", "group": "C", "kind": "Ltp", "channel": 13, "emitter": true, "cie": [0.3127, 0.3290] } ] } ] } ] }"#;

/// RGB + Warm White, Cold White, Amber, Lime (5–8), auto-mix on; outputs to stress ENG-24.
fn many(output: f32) -> String {
    let e = |name: &str, ch: u16, cie: [f32; 2]| {
        format!(
            r#"{{ "name": "{name}", "group": "C", "kind": "Ltp", "channel": {ch}, "emitter": true,
                 "cie": [{}, {}], "output": {output} }}"#,
            cie[0], cie[1]
        )
    };
    format!(
        r#"{{ "manufacturer": "T", "model": "Many", "source": "channel-list", "modes": [ {{
  "name": "8ch", "footprint": 8, "automix": true,
  "colour": {{ "system": "RGB", "components": [ {{ "channel": 1 }}, {{ "channel": 2 }}, {{ "channel": 3 }} ] }},
  "attributes": [ {{ "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 4 }}, {}, {}, {}, {} ] }} ] }}"#,
        e("Warm White", 5, [0.4366, 0.4042]),
        e("Cold White", 6, [0.3135, 0.3237]),
        e("Amber", 7, AMBER),
        e("Lime", 8, LIME),
    )
}

fn engine(profiles: &[(&str, &str)], patch: &[(u32, &str, u16)]) -> Engine {
    let mut e = Engine::new(Settings::default());
    for &(key, json) in profiles {
        e.add_profile(key, Profile::from_json(json).unwrap())
            .unwrap();
    }
    for &(id, profile, address) in patch {
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
    e
}

/// 201 (auto RGBW @1), 202 (plain RGBW @6), 211 (RGBL @11).
fn rig() -> Engine {
    let auto = rgbw(true, "");
    let plain = rgbw(false, "");
    engine(
        &[("auto", &auto), ("plain", &plain), ("rgbl", RGBL)],
        &[(201, "auto", 1), (202, "plain", 6), (211, "rgbl", 11)],
    )
}

fn rgb(r: f32, g: f32, b: f32) -> TargetColour {
    TargetColour { r, g, b }
}

fn dmx_range(e: &Engine, from: u16, to: u16) -> Vec<u8> {
    let f = e.frame(1).unwrap();
    (from..=to).map(|a| f.channel(a).unwrap()).collect()
}

/// Set Colour (and Dimmer full) on one fixture and render.
fn show(e: &mut Engine, id: u32, colour: TargetColour) {
    e.set_programmer_value(id, "Colour", colour).unwrap();
    e.set_programmer_value(id, "Dimmer", 1.0).unwrap();
    e.tick(0.0);
}

fn close3(actual: [f32; 3], expected: [f32; 3]) {
    for (a, b) in actual.iter().zip(expected) {
        assert!((a - b).abs() < 0.01, "expected {expected:?}, got {actual:?}");
    }
}

fn assert_rejected(json: &str, needle: &str) {
    match Profile::from_json(json) {
        Err(CoreError::InvalidProfile { problem, .. }) => assert!(
            problem.contains(needle),
            "problem '{problem}' should mention '{needle}'"
        ),
        other => panic!("expected InvalidProfile mentioning '{needle}', got {other:?}"),
    }
}

// ---- LIB-36: automix per mode -----------------------------------------------------------

#[test]
fn lib_36_automix_round_trips_and_is_written_only_when_true() {
    let on = Profile::from_json(&rgbw(true, "")).unwrap();
    let json = on.to_json();
    assert!(json.contains("\"automix\":true"), "{json}");
    assert_eq!(Profile::from_json(&json).unwrap(), on, "LIB-01");

    let off = Profile::from_json(&rgbw(false, "")).unwrap();
    assert!(!off.to_json().contains("automix"), "false is not written");
    assert_ne!(on, off);
}

#[test]
fn lib_36_automix_needs_an_rgb_system() {
    let cmy = r#"{ "manufacturer": "T", "model": "T", "source": "channel-list", "modes": [ {
      "name": "M", "footprint": 4, "automix": true,
      "colour": { "system": "CMY", "components": [ { "channel": 2 }, { "channel": 3 }, { "channel": 4 } ] },
      "attributes": [ { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 } ] } ] }"#;
    assert_rejected(cmy, "automix");
    let none = r#"{ "manufacturer": "T", "model": "T", "source": "channel-list", "modes": [ {
      "name": "M", "footprint": 1, "automix": true,
      "attributes": [ { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 } ] } ] }"#;
    assert_rejected(none, "automix");
    assert!(Profile::from_json(CELLS).is_ok(), "an RGB system per cell counts");
}

// ---- LIB-37: name defaults ----------------------------------------------------------------

#[test]
fn lib_37_default_chromaticity_by_name() {
    assert_eq!(default_cie("White"), Some([0.3127, 0.3290]));
    assert_eq!(default_cie("cold white"), Some([0.3135, 0.3237]));
    assert_eq!(default_cie("WARM WHITE"), Some([0.4366, 0.4042]));
    assert_eq!(default_cie("Amber"), Some([0.5752, 0.4242]));
    assert_eq!(default_cie("Lime"), Some([0.4100, 0.5500]));
    for none in ["UV", "Indigo", "Cyan", "Whites", ""] {
        assert_eq!(default_cie(none), None, "{none}");
    }
}

// ---- LIB-38: output factor ----------------------------------------------------------------

#[test]
fn lib_38_output_round_trips_and_is_written_only_when_not_one() {
    let two = Profile::from_json(&rgbw(true, r#", "output": 2.0"#)).unwrap();
    let json = two.to_json();
    assert!(json.contains("\"output\":2.0"), "{json}");
    assert_eq!(Profile::from_json(&json).unwrap(), two, "LIB-01");
    let one = Profile::from_json(&rgbw(true, r#", "output": 1.0"#)).unwrap();
    assert!(!one.to_json().contains("output"), "1.0 is not written");
    assert_eq!(one, Profile::from_json(&rgbw(true, "")).unwrap());
}

#[test]
fn lib_38_output_out_of_range_or_off_an_emitter_is_rejected() {
    assert_rejected(&rgbw(true, r#", "output": 0.2"#), "output");
    assert_rejected(&rgbw(true, r#", "output": 4.5"#), "output");
    let on_zoom = r#"{ "manufacturer": "T", "model": "T", "source": "channel-list", "modes": [ {
      "name": "M", "footprint": 1,
      "attributes": [ { "name": "Zoom", "group": "B", "kind": "Ltp", "channel": 1, "output": 2.0 } ] } ] }"#;
    assert_rejected(on_zoom, "output");
}

// ---- ENG-20: RGB equivalent ---------------------------------------------------------------

#[test]
fn eng_20_rgb_equivalent_of_the_church_emitters() {
    close3(rgb_equivalent(D65).unwrap(), [1.0, 1.0, 1.0]);
    close3(rgb_equivalent(LIME).unwrap(), [0.808, 1.0, 0.0]);
    close3(rgb_equivalent(AMBER).unwrap(), [1.0, 0.361, 0.0]);
    close3(rgb_equivalent([0.4366, 0.4042]).unwrap(), [1.0, 0.645, 0.248]);
}

#[test]
fn eng_20_small_components_become_zero_and_outside_the_gamut_is_not_mixable() {
    let lime = rgb_equivalent(LIME).unwrap();
    assert_eq!(lime[2], 0.0, "0.035 blue is dropped");
    let amber = rgb_equivalent(AMBER).unwrap();
    assert_eq!(amber[2], 0.0, "-0.009 blue is dropped");
    assert_eq!(rgb_equivalent(UV), None, "UV");
    assert_eq!(rgb_equivalent([0.1566, 0.0177]), None, "deep indigo");
}

// ---- ENG-21: extraction -------------------------------------------------------------------

#[test]
fn eng_21_rgbw_takes_the_white_out_of_rgb() {
    let mut e = rig();
    show(&mut e, 201, rgb(1.0, 0.5, 0.5));
    assert_eq!(dmx_range(&e, 1, 5), [255, 128, 0, 0, 128]);
}

#[test]
fn eng_21_rgb_lime_uses_the_lime_for_yellow() {
    let mut e = rig();
    show(&mut e, 211, rgb(1.0, 0.8, 0.0));
    // Lime 0.8; R = 1 − 0.8 × 0.808 = 0.354.
    assert_eq!(dmx_range(&e, 11, 14), [90, 0, 0, 204]);
}

#[test]
fn eng_21_output_factor_scales_the_emitter_level() {
    let bright = rgbw(true, r#", "output": 2.0"#);
    let dim = rgbw(true, r#", "output": 0.5"#);
    let mut e = engine(
        &[("bright", &bright), ("dim", &dim)],
        &[(201, "bright", 1), (202, "dim", 6)],
    );
    for id in [201, 202] {
        e.set_programmer_value(id, "Colour", rgb(1.0, 1.0, 1.0))
            .unwrap();
        e.set_programmer_value(id, "Dimmer", 1.0).unwrap();
    }
    e.tick(0.0);
    assert_eq!(dmx_range(&e, 1, 5), [255, 0, 0, 0, 128], "output 2.0");
    assert_eq!(dmx_range(&e, 6, 10), [255, 128, 128, 128, 255], "output 0.5");
}

#[test]
fn eng_21_saturated_blue_leaves_the_white_off() {
    let mut e = rig();
    show(&mut e, 201, rgb(0.0, 0.0, 1.0));
    assert_eq!(dmx_range(&e, 1, 5), [255, 0, 0, 255, 0]);
}

#[test]
fn eng_21_every_cell_mixes_its_own_white() {
    let mut e = engine(&[("cells", CELLS)], &[(501, "cells", 1)]);
    show(&mut e, 501, rgb(1.0, 0.5, 0.5));
    assert_eq!(dmx_range(&e, 1, 1), [255]);
    for cell in [2, 6, 10] {
        assert_eq!(
            dmx_range(&e, cell, cell + 3),
            [128, 0, 0, 128],
            "cell at {cell}"
        );
    }
}

// ---- ENG-22: direct on top ----------------------------------------------------------------

#[test]
fn eng_22_direct_white_adds_to_the_auto_level() {
    let mut e = rig();
    e.set_programmer_value(201, "White", 0.3).unwrap();
    show(&mut e, 201, rgb(1.0, 0.5, 0.5));
    assert_eq!(dmx_range(&e, 1, 5), [255, 128, 0, 0, 204]);
    e.set_programmer_value(201, "White", 0.9).unwrap();
    e.tick(0.025);
    assert_eq!(dmx_range(&e, 5, 5), [255], "clamped at full");
}

#[test]
fn eng_22_highlight_open_white_is_all_white_emitter() {
    let mut e = rig();
    e.select_fixture(201).unwrap();
    e.set_highlight(true).unwrap();
    e.tick(0.0);
    assert_eq!(dmx_range(&e, 1, 5), [255, 0, 0, 0, 255]);
}

// ---- ENG-23: where it does not act --------------------------------------------------------

#[test]
fn eng_23_without_automix_the_output_is_nominal() {
    let mut e = rig();
    show(&mut e, 202, rgb(1.0, 0.5, 0.5));
    assert_eq!(dmx_range(&e, 6, 10), [255, 255, 128, 128, 0]);
}

#[test]
fn eng_23_unmixable_emitters_stay_direct() {
    let mut e = engine(&[("uv", RGBUV)], &[(601, "uv", 1)]);
    e.set_programmer_value(601, "UV", 0.4).unwrap();
    show(&mut e, 601, rgb(1.0, 0.6, 0.0));
    assert_eq!(dmx_range(&e, 1, 6), [255, 255, 153, 0, 102, 0]);
}

#[test]
fn eng_23_virtual_dimmer_scales_after_the_mix() {
    let mut e = rig();
    e.set_programmer_value(211, "Colour", rgb(1.0, 0.8, 0.0))
        .unwrap();
    e.set_programmer_value(211, "Dimmer", 0.5).unwrap();
    e.tick(0.0);
    // Lime 0.8 × 0.5, R 0.354 × 0.5.
    assert_eq!(dmx_range(&e, 11, 14), [45, 0, 0, 102]);
}

// ---- ENG-24: safety -----------------------------------------------------------------------

/// For every colour on a 0.1 grid and outputs at both ends of LIB-38's range, the light of the
/// written channels (RGB plus each emitter's level × output × RGB equivalent) is the target,
/// within the DMX rounding of five channels at output 4.0.
#[test]
fn eng_24_mix_stays_in_range_and_never_overshoots() {
    let profiles: Vec<(String, String)> = [0.25f32, 1.0, 4.0]
        .iter()
        .map(|o| (format!("many{o}"), many(*o)))
        .collect();
    let refs: Vec<(&str, &str)> = profiles
        .iter()
        .map(|(k, j)| (k.as_str(), j.as_str()))
        .collect();
    let mut e = engine(
        &refs,
        &[(701, "many0.25", 1), (702, "many1", 9), (703, "many4", 17)],
    );
    let eq: Vec<[f32; 3]> = [[0.4366, 0.4042], [0.3135, 0.3237], AMBER, LIME]
        .iter()
        .map(|c| rgb_equivalent(*c).unwrap())
        .collect();
    let mut t = 0.0;
    for r in 0..=10 {
        for g in 0..=10 {
            for b in 0..=10 {
                let target = [r as f32 / 10.0, g as f32 / 10.0, b as f32 / 10.0];
                for id in [701, 702, 703] {
                    e.set_programmer_value(id, "Colour", rgb(target[0], target[1], target[2]))
                        .unwrap();
                    e.set_programmer_value(id, "Dimmer", 1.0).unwrap();
                }
                t += 0.025;
                e.tick(t);
                for (base, output) in [(1u16, 0.25f32), (9, 1.0), (17, 4.0)] {
                    let ch = dmx_range(&e, base, base + 7);
                    let level = |i: usize| f32::from(ch[i]) / 255.0;
                    let mut light = [level(0), level(1), level(2)];
                    for (k, e_rgb) in eq.iter().enumerate() {
                        for c in 0..3 {
                            light[c] += level(4 + k) * output * e_rgb[c];
                        }
                    }
                    for c in 0..3 {
                        assert!(
                            light[c] <= target[c] + 0.05,
                            "target {target:?}, output {output}: light {light:?}"
                        );
                        assert!(
                            light[c] >= target[c] - 0.05,
                            "target {target:?}, output {output}: light {light:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn eng_24_two_engines_write_the_same_bytes() {
    let run = || {
        let mut e = rig();
        show(&mut e, 201, rgb(0.7, 0.9, 0.3));
        show(&mut e, 211, rgb(0.9, 0.7, 0.1));
        dmx_range(&e, 1, 14)
    };
    assert_eq!(run(), run());
}
