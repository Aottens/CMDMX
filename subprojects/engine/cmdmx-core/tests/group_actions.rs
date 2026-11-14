//! Group handle actions (GRP-01, P1-24): `specs/modules/programmer.md` PRG-85..92. Pars 201–208 at
//! 21–28 and spots 301–306 at 35–40 (`common::church`). IPC and labels in the daemon are GRP-02.
//!
//! API under test (new in GRP-01):
//! - `Engine::delete_group(id)`, `copy_group(id, to)`, `move_group(id, to)`,
//!   `set_group_legend(id, &str)`: `Result<(), CoreError>`, a refusal changes nothing.
//! - `Engine::group_legend(id) -> Option<&str>`: `None` when the legend is empty or there is no
//!   group.
//! - `ShowGroup` key `legend`, absent when empty; `SCHEMA_VERSION` 10 with the step "9 -> 10".
//! - GRP-04, PRG-96: `Engine::drain_notices() -> Vec<String>`, as `drain_warnings`.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

mod common;

use cmdmx_core::engine::Engine;
use cmdmx_core::patch::FixtureId;
use cmdmx_core::playback::{GroupMaster, Master, PlaybackRecord};
use cmdmx_core::programmer::{GridShortcut, Item};
use cmdmx_core::show::{Meta, Show, SCHEMA_VERSION};
use cmdmx_core::undo::History;
use cmdmx_core::CoreError;
use common::{church, dmx, select};

const PARS: [FixtureId; 8] = [201, 202, 203, 204, 205, 206, 207, 208];
const SPOTS: [FixtureId; 6] = [301, 302, 303, 304, 305, 306];
const SPOT_1: u16 = 35;

fn items(ids: &[FixtureId]) -> Vec<Item> {
    ids.iter().map(|&id| Item::fixture(id)).collect()
}

/// Group 5 "Pars outside in" (201–208, a grid), nothing selected.
fn group_5(e: &mut Engine) {
    select(e, &PARS);
    e.grid_shortcut(GridShortcut::OutsideIn).unwrap();
    e.record_group(5).unwrap();
    e.set_group_legend(5, "Pars outside in").unwrap();
    e.clear_selection();
}

/// Group 6 = the spots on master 1.8; a playback holds them at 80 %, the master's fader is at
/// half, so the spots sit at 40 % (102).
fn mastered_group_6(e: &mut Engine) -> u32 {
    select(e, &SPOTS);
    e.record_group(6).unwrap();
    for id in SPOTS {
        e.set_programmer_value(id, "Dimmer", 0.8).unwrap();
    }
    let pb = e.record_playback(&PlaybackRecord::at(1, 1)).unwrap();
    e.clear_selection();
    e.clear();
    e.clear();
    e.set_fader(pb, 1.0, 0.0).unwrap();
    let master = e.record_group_master(1, 8, 6).unwrap();
    e.set_fader(master, 0.5, 0.0).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(e, SPOT_1), 102);
    master
}

fn master_group(e: &Engine, id: u32) -> u32 {
    match &e.playback(id).unwrap().master {
        Some(Master::Group(GroupMaster { group, .. })) => *group,
        other => panic!("not a group master: {other:?}"),
    }
}

fn meta() -> Meta {
    Meta {
        name: "Groups".into(),
        created: "2026-11-01T09:00:00Z".into(),
        modified: "2026-11-01T09:00:00Z".into(),
        app_version: "0.1.0".into(),
    }
}

// ---- PRG-85 ---------------------------------------------------------------------------------

#[test]
fn prg_85_delete_empties_the_cell_and_a_later_record_starts_from_nothing() {
    let mut e = church();
    group_5(&mut e);
    e.delete_group(5).unwrap();
    assert_eq!(e.group_items(5), None);
    assert_eq!(e.group_grid(5), None);
    assert_eq!(e.group_legend(5), None);

    select(&mut e, &[201]);
    e.record_group(5).unwrap();
    assert_eq!(e.group_items(5), Some(items(&[201])));
    assert_eq!(e.group_grid(5), None);
    assert_eq!(e.group_legend(5), None);
}

#[test]
fn prg_85_delete_leaves_the_selection_and_the_cues_alone() {
    let mut e = church();
    group_5(&mut e);
    e.select_group(5).unwrap();
    for id in PARS {
        e.set_programmer_value(id, "Dimmer", 1.0).unwrap();
    }
    let pb = e.record_playback(&PlaybackRecord::at(1, 1)).unwrap();
    let cue = e.playback(pb).unwrap().cues[0].clone();
    e.clear_selection();
    select(&mut e, &[201, 202]);

    e.delete_group(5).unwrap();
    assert_eq!(e.programmer().items(), items(&[201, 202]).as_slice());
    assert_eq!(e.playback(pb).unwrap().cues[0], cue);
    assert!(!cue.order.is_empty(), "the cue keeps its order");
    assert!(e.execute_command(&["Group", "5", "Enter"]).is_err());
}

#[test]
fn prg_85_deleting_an_empty_cell_is_refused() {
    let mut e = church();
    let before = e.edit_state();
    let err = e.delete_group(7).unwrap_err();
    assert_eq!(err.to_string(), "Group 7 is empty");
    assert_eq!(e.edit_state(), before);
}

// ---- PRG-86 ---------------------------------------------------------------------------------

#[test]
fn prg_86_a_mastered_group_is_not_deleted_and_the_warning_names_the_handle() {
    let mut e = church();
    let master = mastered_group_6(&mut e);
    let before = e.edit_state();
    let err = e.delete_group(6).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Group 6 is mastered by handle 1.8: delete the master first"
    );
    assert_eq!(e.edit_state(), before);
    e.tick(0.025);
    assert_eq!(dmx(&e, SPOT_1), 102, "nothing on stage changes");

    e.delete_playback(master).unwrap();
    e.delete_group(6).unwrap();
    assert_eq!(e.group_items(6), None);
}

// ---- PRG-87 ---------------------------------------------------------------------------------

#[test]
fn prg_87_copy_makes_a_new_group_with_items_grid_and_legend() {
    let mut e = church();
    group_5(&mut e);
    let grid = e.group_grid(5);
    assert!(grid.is_some());
    e.copy_group(5, 9).unwrap();
    for id in [5, 9] {
        assert_eq!(e.group_items(id), Some(items(&PARS)), "group {id}");
        assert_eq!(e.group_grid(id), grid, "group {id}");
        assert_eq!(e.group_legend(id), Some("Pars outside in"), "group {id}");
    }
}

#[test]
fn prg_87_group_masters_keep_pointing_at_the_original() {
    let mut e = church();
    let master = mastered_group_6(&mut e);
    e.copy_group(6, 9).unwrap();
    assert_eq!(master_group(&e, master), 6);
}

// ---- PRG-88 ---------------------------------------------------------------------------------

#[test]
fn prg_88_move_carries_items_grid_and_legend_and_empties_the_cell() {
    let mut e = church();
    group_5(&mut e);
    let grid = e.group_grid(5);
    e.move_group(5, 12).unwrap();
    assert_eq!(e.group_items(12), Some(items(&PARS)));
    assert_eq!(e.group_grid(12), grid);
    assert_eq!(e.group_legend(12), Some("Pars outside in"));
    assert_eq!(e.group_items(5), None);
    assert_eq!(e.group_grid(5), None);
    assert_eq!(e.group_legend(5), None);
}

#[test]
fn prg_88_every_master_follows_and_the_stage_does_not_change() {
    let mut e = church();
    let first = mastered_group_6(&mut e);
    let second = e.record_group_master(1, 7, 6).unwrap();
    select(&mut e, &PARS);
    e.record_group(5).unwrap();
    e.clear_selection();
    let other = e.record_group_master(1, 6, 5).unwrap();
    e.tick(0.025);
    assert_eq!(dmx(&e, SPOT_1), 102);

    e.move_group(6, 12).unwrap();
    assert_eq!(master_group(&e, first), 12);
    assert_eq!(master_group(&e, second), 12);
    assert_eq!(master_group(&e, other), 5);
    e.tick(0.05);
    assert_eq!(dmx(&e, SPOT_1), 102, "the master still masters the spots");
    assert_eq!(e.playback_state(first).map(|s| s.level), Some(0.5));
}

// ---- PRG-89 ---------------------------------------------------------------------------------

#[test]
fn prg_89_copy_and_move_refuse_a_used_target_itself_and_an_empty_source() {
    type Action = fn(&mut Engine, u32, u32) -> Result<(), cmdmx_core::CoreError>;
    let actions: [Action; 2] = [Engine::copy_group, Engine::move_group];
    for action in actions {
        let mut e = church();
        group_5(&mut e);
        select(&mut e, &SPOTS);
        e.record_group(6).unwrap();
        let before = e.edit_state();
        let refused = |e: &mut Engine, from, to| action(e, from, to).unwrap_err().to_string();
        assert_eq!(refused(&mut e, 5, 6), "Group 6 is used");
        assert_eq!(refused(&mut e, 5, 5), "Group 5 cannot go onto itself");
        assert_eq!(refused(&mut e, 7, 8), "Group 7 is empty");
        assert_eq!(refused(&mut e, 7, 7), "Group 7 is empty");
        assert_eq!(e.edit_state(), before);
    }
}

// ---- PRG-90 ---------------------------------------------------------------------------------

#[test]
fn prg_90_a_legend_is_0_to_32_characters_and_empty_clears_it() {
    let mut e = church();
    group_5(&mut e);
    let longest = "é".repeat(32);
    e.set_group_legend(5, &longest).unwrap();
    assert_eq!(e.group_legend(5), Some(longest.as_str()));

    let before = e.edit_state();
    let err = e.set_group_legend(5, &"é".repeat(33)).unwrap_err();
    assert_eq!(err.to_string(), "a legend is at most 32 characters");
    assert_eq!(e.edit_state(), before);

    e.set_group_legend(5, "").unwrap();
    assert_eq!(e.group_legend(5), None);
}

#[test]
fn prg_90_an_empty_cell_takes_no_legend() {
    let mut e = church();
    let before = e.edit_state();
    let err = e.set_group_legend(7, "Nothing").unwrap_err();
    assert_eq!(err.to_string(), "Group 7 is empty");
    assert_eq!(e.edit_state(), before);
    assert_eq!(e.group_legend(7), None);
}

#[test]
fn prg_90_re_recording_keeps_the_legend_and_delete_clears_it() {
    let mut e = church();
    group_5(&mut e);
    select(&mut e, &[201]);
    e.record_group(5).unwrap();
    assert_eq!(e.group_items(5), Some(items(&[201])));
    assert_eq!(e.group_legend(5), Some("Pars outside in"));
    e.delete_group(5).unwrap();
    assert_eq!(e.group_legend(5), None);
}

// ---- PRG-91 ---------------------------------------------------------------------------------

#[test]
fn prg_91_undo_of_delete_brings_back_items_grid_and_legend() {
    let mut e = church();
    group_5(&mut e);
    let grid = e.group_grid(5);
    let mut h = History::default();
    h.edit(&mut e, "Delete group 5", None, |e| e.delete_group(5))
        .unwrap();
    assert_eq!(h.undo_label(), Some("Delete group 5"));
    assert_eq!(h.undo(&mut e).unwrap(), "Delete group 5");
    assert_eq!(e.group_items(5), Some(items(&PARS)));
    assert_eq!(e.group_grid(5), grid);
    assert_eq!(e.group_legend(5), Some("Pars outside in"));
}

#[test]
fn prg_91_undo_of_move_brings_the_group_and_its_masters_back() {
    let mut e = church();
    let master = mastered_group_6(&mut e);
    e.set_group_legend(6, "Front").unwrap();
    let mut h = History::default();
    h.edit(&mut e, "Move group 6", None, |e| e.move_group(6, 12))
        .unwrap();
    assert_eq!(h.undo(&mut e).unwrap(), "Move group 6");
    assert_eq!(e.group_items(6), Some(items(&SPOTS)));
    assert_eq!(e.group_legend(6), Some("Front"));
    assert_eq!(e.group_items(12), None);
    assert_eq!(e.group_legend(12), None);
    assert_eq!(master_group(&e, master), 6);
    e.tick(0.025);
    assert_eq!(dmx(&e, SPOT_1), 102, "the stage does not change");
}

#[test]
fn prg_91_each_action_is_one_step_and_a_refused_one_adds_none() {
    let mut e = church();
    group_5(&mut e);
    let mut h = History::default();
    h.edit(&mut e, "Copy group 5", None, |e| e.copy_group(5, 9))
        .unwrap();
    h.edit(&mut e, "Legend group 5", None, |e| {
        e.set_group_legend(5, "Pars")
    })
    .unwrap();
    assert_eq!(h.len(), 2);
    assert!(h
        .edit(&mut e, "Copy group 5", None, |e| e.copy_group(5, 9))
        .is_err());
    assert!(h
        .edit(&mut e, "Delete group 7", None, |e| e.delete_group(7))
        .is_err());
    assert_eq!(h.len(), 2);

    assert_eq!(h.undo(&mut e).unwrap(), "Legend group 5");
    assert_eq!(e.group_legend(5), Some("Pars outside in"));
    assert_eq!(h.undo(&mut e).unwrap(), "Copy group 5");
    assert_eq!(e.group_items(9), None);
}

// ---- PRG-92 ---------------------------------------------------------------------------------

#[test]
fn prg_92_schema_10_round_trips_a_legend() {
    assert_eq!(SCHEMA_VERSION, 15);
    let mut e = church();
    group_5(&mut e);
    let json = e.to_show(meta()).to_json();
    assert!(json.contains("\"schema_version\": 15"));
    assert!(json.contains("\"legend\": \"Pars outside in\""), "{json}");
    let (show, migrations) = Show::from_json(&json).unwrap();
    assert!(migrations.is_empty(), "{migrations:?}");
    let (loaded, warnings) = Engine::from_show(&show);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(loaded.group_legend(5), Some("Pars outside in"));
    assert_eq!(loaded.to_show(meta()).to_json(), json);
}

#[test]
fn prg_92_a_show_without_legends_saves_as_schema_9_did() {
    let mut e = church();
    select(&mut e, &SPOTS);
    e.record_group(6).unwrap();
    e.set_group_legend(6, "Front").unwrap();
    e.set_group_legend(6, "").unwrap();
    let json = e.to_show(meta()).to_json();
    let groups = &json[json.find("\"groups\"").unwrap()..json.find("\"palettes\"").unwrap()];
    assert!(!groups.contains("legend"), "{groups}");

    let nine = json.replace("\"schema_version\": 15", "\"schema_version\": 9");
    let (show, migrations) = Show::from_json(&nine).unwrap();
    assert_eq!(migrations.len(), 6, "{migrations:?}");
    assert!(migrations[0].starts_with("9 -> 10"), "{migrations:?}");
    let (loaded, warnings) = Engine::from_show(&show);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(loaded.to_show(show.meta.clone()).to_json(), json);
}

#[test]
fn prg_92_the_schema_9_church_show_loads_unchanged() {
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/fixtures/shows/"
    );
    let v9 = std::fs::read_to_string(format!("{dir}church-demo-v9.cmdmx.json")).unwrap();
    let v10 = std::fs::read_to_string(format!("{dir}church-demo-v10.cmdmx.json")).unwrap();
    let now = std::fs::read_to_string(format!("{dir}church-demo.cmdmx.json")).unwrap();
    assert_eq!(
        v9.replace("\"schema_version\": 9", "\"schema_version\": 10"),
        v10
    );
    let (show, _) = Show::from_json(&v9).unwrap();
    let (loaded, warnings) = Engine::from_show(&show);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(loaded.to_show(show.meta.clone()).to_json(), now);
}

#[test]
fn prg_92_a_legend_too_long_in_the_file_is_left_out_with_a_warning() {
    let mut e = church();
    group_5(&mut e);
    let json = e
        .to_show(meta())
        .to_json()
        .replace("Pars outside in", &"x".repeat(33));
    let (show, _) = Show::from_json(&json).unwrap();
    let (loaded, warnings) = Engine::from_show(&show);
    assert_eq!(loaded.group_legend(5), None);
    assert_eq!(loaded.group_items(5), Some(items(&PARS)));
    assert!(
        warnings.iter().any(|w| w.starts_with("group 5: legend")),
        "{warnings:?}"
    );
}

fn line(e: &mut Engine, text: &str) -> Result<(), CoreError> {
    e.execute_command(&text.split_whitespace().collect::<Vec<_>>())
}

fn selected(e: &Engine) -> Vec<FixtureId> {
    e.programmer().selection().to_vec()
}

#[test]
fn prg_96_select_group_leaves_a_notice_with_the_legend() {
    let mut e = church();
    group_5(&mut e);
    select(&mut e, &SPOTS);
    e.record_group(8).unwrap();
    e.clear_selection();
    e.drain_notices();

    e.select_group(5).unwrap();
    assert_eq!(e.drain_notices(), ["Group 5 Pars outside in"]);
    assert_eq!(e.drain_notices(), Vec::<String>::new(), "drained");
    e.select_group(8).unwrap();
    assert_eq!(e.drain_notices(), ["Group 8"], "no legend");
    e.select_group(5).unwrap(); // PRG-02: toggles the group off
    assert_eq!(e.drain_notices(), ["Group 5 Pars outside in"]);
}

#[test]
fn prg_96_a_line_naming_one_group_leaves_a_notice() {
    let mut e = church();
    group_5(&mut e);
    for text in ["Group 5 Enter", "Group 5 @ 50 Enter", "Group 5 Locate"] {
        line(&mut e, text).unwrap();
        assert_eq!(e.drain_notices(), ["Group 5 Pars outside in"], "{text}");
    }
}

#[test]
fn prg_96_other_lines_leave_no_notice() {
    let mut e = church();
    group_5(&mut e);
    select(&mut e, &SPOTS);
    e.record_group(6).unwrap();
    e.clear_selection();
    for text in [
        "Group 5 And 6 Enter",
        "Group 5 Thro 6 Enter",
        "201 Enter",
        "201 @ 50 Enter",
    ] {
        line(&mut e, text).unwrap();
        assert_eq!(e.drain_notices(), Vec::<String>::new(), "{text}");
    }
}

#[test]
fn prg_96_a_refused_line_leaves_no_notice() {
    let mut e = church();
    group_5(&mut e);
    assert!(line(&mut e, "Group 5 Palette 99 Enter").is_err());
    assert_eq!(e.drain_notices(), Vec::<String>::new());
}

#[test]
fn prg_96_an_empty_group_is_refused_by_name_with_fixtures_selected() {
    let mut e = church();
    select(&mut e, &[201, 202]);
    for text in ["Group 7 Enter", "Group 7 @ 50 Enter", "Group 7 Locate"] {
        assert_eq!(line(&mut e, text), Err(CoreError::GroupEmpty(7)), "{text}");
    }
    assert_eq!(e.select_group(7), Err(CoreError::GroupEmpty(7)));
    assert_eq!(CoreError::GroupEmpty(7).to_string(), "Group 7 is empty");
    assert_eq!(selected(&e), [201, 202], "nothing changes");
    assert_eq!(e.drain_notices(), Vec::<String>::new());
}

#[test]
fn prg_96_a_deleted_group_is_refused_as_empty() {
    let mut e = church();
    group_5(&mut e);
    e.delete_group(5).unwrap();
    assert_eq!(line(&mut e, "Group 5 Enter"), Err(CoreError::GroupEmpty(5)));
}

#[test]
fn prg_96_a_group_without_patched_members_is_empty() {
    let mut e = church();
    select(&mut e, &[201]);
    e.record_group(7).unwrap();
    e.clear_selection();
    e.patch_remove(&[201]).unwrap();
    select(&mut e, &[202]);
    assert_eq!(line(&mut e, "Group 7 Enter"), Err(CoreError::GroupEmpty(7)));
    assert_eq!(e.select_group(7), Err(CoreError::GroupEmpty(7)));
    assert_eq!(selected(&e), [202]);
}

#[test]
fn prg_96_other_empty_lines_keep_nothing_is_selected() {
    let mut e = church();
    select(&mut e, &[201]);
    for text in ["999 Enter", "Group 7 And 8 Enter"] {
        assert_eq!(line(&mut e, text), Err(CoreError::EmptySelection), "{text}");
    }
}
