//! Programmer contents in the merge and the wheels: `specs/modules/programmer.md`
//! PRG-15, 17..21, 36, 37 (VS2-04). PRG-12 and PRG-13 are scenario files.
//! Colour wheels (the C part of PRG-17) are VS2-06.

#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

mod common;

use std::collections::BTreeMap;

use cmdmx_core::attribute::AttributeGroup;
use cmdmx_core::engine::Engine;
use cmdmx_core::playback::{Playback, Priority, Values};
use cmdmx_core::programmer::Item;
use cmdmx_core::programmer::Resolution;
use cmdmx_core::value::Value;
use cmdmx_core::CoreError;
use common::{church, dmx, select};

use AttributeGroup::*;

fn values(entries: &[(u32, &str, f32)]) -> Values {
    let mut v: Values = BTreeMap::new();
    for &(id, attr, val) in entries {
        v.entry(id.into())
            .or_default()
            .insert(attr.to_string(), val.into());
    }
    v
}

fn playback(e: &mut Engine, id: u32, priority: Priority, entries: &[(u32, &str, f32)]) {
    e.add_playback(Playback {
        priority,
        ..Playback::new(id, 1, 1, values(entries))
    });
    e.set_fader(id, 1.0, 0.0).unwrap();
}

fn s(list: &[&str]) -> Vec<String> {
    list.iter().map(|x| x.to_string()).collect()
}

// ---- PRG-15 -----------------------------------------------------------------

#[test]
fn prg_15_contents_returns_only_touched_values_in_the_mask() {
    let mut e = church();
    playback(
        &mut e,
        1,
        Priority::Normal,
        &[(101, "Tilt", 0.3), (102, "Dimmer", 1.0)],
    );
    e.set_programmer_value(101, "Dimmer", 0.8).unwrap();
    e.set_programmer_value(101, "Pan", 0.5).unwrap();
    e.set_programmer_value(101, "Zoom", 0.2).unwrap();
    e.set_programmer_value(301, "Dimmer", 1.0).unwrap();
    e.tick(0.1);

    assert_eq!(e.contents(&[P]), values(&[(101, "Pan", 0.5)]));
    assert_eq!(
        e.contents(&[I, B]),
        values(&[
            (101, "Dimmer", 0.8),
            (101, "Zoom", 0.2),
            (301, "Dimmer", 1.0)
        ])
    );
    assert_eq!(e.contents(&[C, G]), Values::new());
    // Tilt (playback) and 102 (playback only) never appear.
    let all = e.contents(&[I, P, C, G, B, E, S, Fx]);
    assert_eq!(all.len(), 2);
    assert!(all.get(&Item::fixture(101)).unwrap().get("Tilt").is_none());
}

// ---- PRG-17 -----------------------------------------------------------------

#[test]
fn prg_17_wheel_layout_lists_the_active_groups_attributes_of_the_targets() {
    let mut e = church();
    assert!(e.wheel_layout().is_empty(), "no targets, no wheels");
    select(&mut e, &[101]);
    e.set_active_group(P);
    assert_eq!(e.wheel_layout(), s(&["Pan", "Tilt"]));
    e.set_active_group(B);
    assert_eq!(e.wheel_layout(), s(&["Zoom"]));
}

#[test]
fn prg_17_wheel_layout_merges_attributes_of_mixed_targets() {
    let mut e = church();
    select(&mut e, &[301, 101]);
    e.set_active_group(I);
    assert_eq!(e.wheel_layout(), s(&["Dimmer"]));
    e.set_active_group(P);
    assert_eq!(e.wheel_layout(), s(&["Pan", "Tilt"]));
}

// ---- PRG-18 -----------------------------------------------------------------

#[test]
fn prg_18_active_group_switches_when_targets_lack_it() {
    let mut e = church();
    select(&mut e, &[101]);
    e.set_active_group(P);
    e.clear_selection();
    select(&mut e, &[301]); // a spot: intensity only
    assert_eq!(e.active_group(), I);
    // It switched; adding a wash does not switch back.
    select(&mut e, &[101]);
    assert_eq!(e.active_group(), I);
}

#[test]
fn prg_18_active_group_follows_the_sub_selection() {
    let mut e = church();
    select(&mut e, &[101, 301]);
    e.set_active_group(B);
    e.next_fixture(); // 101
    assert_eq!(e.active_group(), B);
    e.next_fixture(); // 301
    assert_eq!(e.active_group(), I);
}

#[test]
fn prg_18_no_targets_keeps_the_active_group() {
    let mut e = church();
    select(&mut e, &[101]);
    e.set_active_group(B);
    e.clear_selection();
    assert_eq!(e.active_group(), B);
}

// ---- PRG-19 -----------------------------------------------------------------

#[test]
fn prg_19_wheel_is_relative_and_clamps_per_fixture() {
    let mut e = church();
    e.set_programmer_value(201, "Dimmer", 0.2).unwrap();
    e.set_programmer_value(202, "Dimmer", 0.7).unwrap();
    e.clear_selection();
    select(&mut e, &[201, 202]);
    e.turn_wheel("Dimmer", 10, Resolution::Coarse).unwrap();
    let v201 = e.programmer().value(201, "Dimmer").unwrap().level();
    assert!((v201 - 0.6).abs() < 1e-5, "{v201}");
    assert_eq!(e.programmer().value(202, "Dimmer"), Some(Value::Level(1.0)));
    e.turn_wheel("Dimmer", -100, Resolution::Coarse).unwrap();
    assert_eq!(e.programmer().value(201, "Dimmer"), Some(Value::Level(0.0)));
    assert_eq!(e.programmer().value(202, "Dimmer"), Some(Value::Level(0.0)));
}

#[test]
fn prg_19_wheel_only_touches_targets_that_have_the_attribute() {
    let mut e = church();
    select(&mut e, &[101, 301]);
    e.turn_wheel("Zoom", 5, Resolution::Coarse).unwrap();
    assert!(e.programmer().value(101, "Zoom").is_some());
    assert!(!e.programmer().in_programmer(301));
}

#[test]
fn prg_19_wheel_turn_counts_as_a_change_for_the_next_select() {
    let mut e = church();
    select(&mut e, &[201]);
    e.turn_wheel("Dimmer", 1, Resolution::Normal).unwrap();
    select(&mut e, &[202]);
    assert_eq!(e.programmer().selection(), &[202]);
}

// ---- PRG-20 -----------------------------------------------------------------

#[test]
fn prg_20_first_touch_starts_from_the_current_output_value() {
    let mut e = church();
    playback(&mut e, 1, Priority::Normal, &[(201, "Dimmer", 0.6)]);
    e.tick(0.1);
    select(&mut e, &[201]);
    e.turn_wheel("Dimmer", 1, Resolution::Normal).unwrap();
    let v = e.programmer().value(201, "Dimmer").unwrap().level();
    assert!((v - (0.6 + 1.0 / 255.0)).abs() < 1e-5, "{v}");
}

#[test]
fn prg_20_untouched_attribute_without_a_source_starts_from_zero() {
    let mut e = church();
    select(&mut e, &[101]);
    e.turn_wheel("Zoom", 1, Resolution::Coarse).unwrap();
    let v = e.programmer().value(101, "Zoom").unwrap().level();
    assert!((v - 0.04).abs() < 1e-5, "{v}");
}

// ---- PRG-21 -----------------------------------------------------------------

#[test]
fn prg_21_step_sizes_per_resolution() {
    let mut e = church();
    select(&mut e, &[201]);
    e.turn_wheel("Dimmer", 1, Resolution::Normal).unwrap();
    e.tick(0.1);
    assert_eq!(dmx(&e, 21), 1, "one Normal step is one 8-bit DMX step");
    e.turn_wheel("Dimmer", 1, Resolution::Coarse).unwrap();
    let v = e.programmer().value(201, "Dimmer").unwrap().level();
    assert!((v - (1.0 / 255.0 + 0.04)).abs() < 1e-5, "{v}");
}

#[test]
fn prg_21_fine_step_moves_only_the_fine_channel_of_a_16_bit_attribute() {
    let mut e = church();
    e.set_programmer_value(101, "Pan", 32768.0 / 65535.0)
        .unwrap();
    e.tick(0.1);
    assert_eq!((dmx(&e, 2), dmx(&e, 3)), (0x80, 0x00));
    e.clear_selection();
    select(&mut e, &[101]);
    e.turn_wheel("Pan", 1, Resolution::Fine).unwrap();
    e.tick(0.2);
    assert_eq!((dmx(&e, 2), dmx(&e, 3)), (0x80, 0x01));
}

// ---- PRG-13 (unit view; the scenario file is PRG-13-*.json) ------------------

#[test]
fn prg_13_programmer_value_reaches_the_frame_over_a_normal_playback() {
    let mut e = church();
    playback(&mut e, 1, Priority::Normal, &[(201, "Dimmer", 0.8)]);
    e.set_programmer_value(201, "Dimmer", 0.0).unwrap();
    e.tick(0.1);
    assert_eq!(dmx(&e, 21), 0);
}

// ---- PRG-36 -----------------------------------------------------------------

#[test]
fn prg_36_refused_wheel_turns_change_nothing() {
    let mut e = church();
    assert_eq!(
        e.turn_wheel("Dimmer", 1, Resolution::Normal),
        Err(CoreError::EmptySelection)
    );
    select(&mut e, &[301]);
    assert_eq!(
        e.turn_wheel("Pan", 1, Resolution::Normal),
        Err(CoreError::AttributeNotInTargets("Pan".into()))
    );
    assert!(e.programmer().is_empty());
    // No change was made, so the next select still adds.
    select(&mut e, &[302]);
    assert_eq!(e.programmer().selection(), &[301, 302]);
}

// ---- PRG-37 -----------------------------------------------------------------

#[test]
fn prg_37_same_programmer_actions_give_identical_frames() {
    let run = || {
        let mut e = church();
        playback(
            &mut e,
            1,
            Priority::Normal,
            &[(101, "Dimmer", 0.5), (101, "Tilt", 0.3)],
        );
        e.execute_command(&["101", "Thro", "104", "@", "70", "Enter"])
            .unwrap();
        e.set_active_group(P);
        e.turn_wheel("Pan", 7, Resolution::Coarse).unwrap();
        e.next_fixture();
        e.turn_wheel("Tilt", -3, Resolution::Fine).unwrap();
        e.tick(0.5);
        e.frame(1).unwrap().clone()
    };
    assert_eq!(run(), run());
}
