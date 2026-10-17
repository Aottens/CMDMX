//! What the engine exposes for rendering, and the default Clear:
//! `specs/modules/programmer.md` PRG-35 and PRG-29 (default `SelectionThenProgrammer` only),
//! shaped by `specs/modules/ipc.md` IPC-11 (VS2-07).

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

mod common;

use cmdmx_core::attribute::AttributeGroup::{self, *};
use cmdmx_core::programmer::{AttributeGroupState, ProgrammerView, WheelState};
use common::{church, select};

fn group_state(view: &ProgrammerView, g: AttributeGroup) -> &AttributeGroupState {
    view.attribute_groups.iter().find(|s| s.group == g).unwrap()
}

// ---- PRG-29 (default behaviour) ----------------------------------------------

#[test]
fn prg_29_first_clear_deselects_second_empties_the_programmer() {
    let mut e = church();
    e.execute_command(&["201", "Thro", "203", "@", "50", "Enter"])
        .unwrap();
    e.clear();
    assert!(e.programmer().selection().is_empty());
    assert_eq!(e.programmer().value(201, "Dimmer"), Some(0.5));
    e.clear();
    assert!(e.programmer().is_empty());
    e.clear(); // already empty: harmless
    assert!(e.programmer().is_empty());
}

#[test]
fn prg_29_clear_with_only_a_selection_just_deselects() {
    let mut e = church();
    select(&mut e, &[101]);
    e.clear();
    assert!(e.programmer().selection().is_empty());
    assert!(e.programmer().is_empty());
}

// ---- PRG-35 ------------------------------------------------------------------

#[test]
fn prg_35_view_of_an_empty_programmer() {
    let e = church();
    let v = e.programmer_view();
    assert!(v.selection.is_empty());
    assert!(v.targets.is_empty());
    assert_eq!(v.active_group, I);
    assert!(v.wheels.is_empty());
    assert!(v.in_programmer.is_empty());
    let order: Vec<_> = v.attribute_groups.iter().map(|s| s.group).collect();
    assert_eq!(order, vec![I, P, C, G, B, E, S, Fx]);
    assert!(v
        .attribute_groups
        .iter()
        .all(|s| !s.available && !s.has_values));
}

#[test]
fn prg_35_view_shows_selection_targets_and_group_states() {
    let mut e = church();
    select(&mut e, &[301, 101]);
    e.next_fixture(); // target 301
    e.next_fixture(); // target 101
    e.set_active_group(P);
    e.turn_wheel("Pan", 10, cmdmx_core::programmer::Resolution::Coarse)
        .unwrap();
    let v = e.programmer_view();
    assert_eq!(v.selection, vec![301, 101]);
    assert_eq!(v.targets, vec![101]);
    assert_eq!(v.active_group, P);
    assert!(group_state(&v, I).available);
    assert!(group_state(&v, P).available && group_state(&v, P).has_values);
    assert!(group_state(&v, B).available && !group_state(&v, B).has_values);
    assert!(!group_state(&v, C).available);
    assert_eq!(v.in_programmer, vec![101]);
}

#[test]
fn prg_35_wheels_show_the_first_targets_value_and_whether_it_is_touched() {
    let mut e = church();
    select(&mut e, &[101, 102]);
    e.set_active_group(P);
    e.turn_wheel("Pan", 10, cmdmx_core::programmer::Resolution::Coarse)
        .unwrap();
    e.tick(0.1);
    let v = e.programmer_view();
    assert_eq!(v.wheels.len(), 2);
    let pan: &WheelState = &v.wheels[0];
    assert_eq!(pan.attribute, "Pan");
    assert!((pan.value - 0.4).abs() < 1e-5, "{}", pan.value);
    assert!(pan.touched);
    let tilt = &v.wheels[1];
    assert_eq!(tilt.attribute, "Tilt");
    assert_eq!(tilt.value, 0.0);
    assert!(!tilt.touched);
}

#[test]
fn prg_35_in_programmer_is_sorted_and_has_values_follows_the_targets() {
    let mut e = church();
    e.execute_command(&["302", "And", "201", "@", "100", "Enter"])
        .unwrap();
    let v = e.programmer_view();
    assert_eq!(v.in_programmer, vec![201, 302]);
    assert!(group_state(&v, I).has_values);
    e.clear(); // deselect: no targets, so no group "has values" for them
    let v = e.programmer_view();
    assert!(!group_state(&v, I).has_values);
    assert_eq!(v.in_programmer, vec![201, 302]);
}
