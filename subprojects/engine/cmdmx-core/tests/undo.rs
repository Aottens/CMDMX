//! Undo history and edit-state snapshots (VS5-01): `specs/modules/undo.md` UNDO-01..03, 05..12,
//! 16, 17; design in ADR-008.
//!
//! API these tests fix (`cmdmx_core::undo` and `Engine`):
//! - `Engine::edit_state(&self) -> EditState` (`Clone + PartialEq + Debug`): the stored show
//!   (patch, profiles, groups, palettes, playbacks with cues and options, page legends, settings)
//!   plus the programmer's values, palette references and selection. Not in it: id counters,
//!   fader levels, playback states, GM, DBO, page, held values, palette fades, the wheels' group
//!   and colour model.
//! - `undo::History` (`Default`), `undo::DEPTH` = 100.
//! - `History::edit(&mut self, &mut Engine, label, key: Option<&str>, f)`: runs one edit command
//!   `f(&mut Engine) -> Result<T, CoreError>` and returns its result. It adds a step labelled
//!   `label` holding the edit state from before `f`, unless `f` failed or left the edit state
//!   equal (UNDO-01). A `key` equal to the previous edit's key merges into that step (UNDO-05).
//! - `History::live(&mut self)`: a live command happened; it ends a coalescing run.
//! - `History::undo(&mut self, &mut Engine) -> Result<String, CoreError>` and `redo`: the label of
//!   the step acted on; `CoreError::NothingToUndo` / `NothingToRedo` ("Nothing to undo" / "Nothing
//!   to redo") when empty.
//! - `History::undo_label() / redo_label() -> Option<&str>` (IPC-31), `len()`, `redo_len()`,
//!   `clear()` (UNDO-13, used by the daemon).

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

mod common;

use cmdmx_core::engine::Engine;
use cmdmx_core::palette::{Cell, Record, Window};
use cmdmx_core::patch::PatchAdd;
use cmdmx_core::playback::{Master, Playback, PlaybackRecord, Values};
use cmdmx_core::programmer::Resolution;
use cmdmx_core::undo::{History, DEPTH};
use cmdmx_core::value::{Stored, Value};
use cmdmx_core::CoreError;
use common::{church, dmx};

fn line(e: &mut Engine, text: &str) -> Result<(), CoreError> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    e.execute_command(&tokens)
}

/// One command-line line as one step (UNDO-02).
fn cmd(h: &mut History, e: &mut Engine, text: &str) {
    h.edit(e, text, None, |e| line(e, text)).unwrap();
}

fn dimmer(fixture: u32, level: f32) -> Values {
    [(
        fixture,
        [("Dimmer".to_string(), Stored::Value(Value::Level(level)))].into(),
    )]
    .into()
}

fn add_dimmer(at: u16) -> PatchAdd {
    PatchAdd {
        profile: "dimmer".into(),
        mode: None,
        count: 1,
        universe: 1,
        address: Some(at),
        number: None,
    }
}

// ---- UNDO-01 ----------------------------------------------------------------

#[test]
fn undo_01_an_accepted_edit_adds_one_labelled_step() {
    let mut e = church();
    let mut h = History::default();
    let before = e.edit_state();
    cmd(&mut h, &mut e, "201 @ 50 Enter");
    assert_eq!(h.len(), 1);
    assert_eq!(h.undo_label(), Some("201 @ 50 Enter"));
    assert_eq!(h.redo_label(), None);
    assert_ne!(e.edit_state(), before);
}

#[test]
fn undo_01_a_refused_or_unchanged_command_adds_no_step_and_keeps_redo() {
    let mut e = church();
    let mut h = History::default();
    cmd(&mut h, &mut e, "201 @ 50 Enter");
    h.undo(&mut e).unwrap();
    assert_eq!(h.redo_len(), 1);
    // Clear on an empty programmer changes nothing.
    h.edit(&mut e, "Clear", None, |e| {
        e.clear();
        Ok(())
    })
    .unwrap();
    // A refused command.
    let err = h
        .edit(&mut e, "Delete playback 9", None, |e| e.delete_playback(9))
        .unwrap_err();
    assert_eq!(err, CoreError::UnknownPlayback(9));
    assert_eq!(h.len(), 0);
    assert_eq!(h.redo_len(), 1, "the redo list is left alone");
    assert_eq!(h.redo_label(), Some("201 @ 50 Enter"));
}

// ---- UNDO-02 ----------------------------------------------------------------

#[test]
fn undo_02_a_command_line_line_is_one_step() {
    let mut e = church();
    let mut h = History::default();
    let before = e.edit_state();
    cmd(&mut h, &mut e, "201 Thro 205 @ 80 Enter");
    assert_eq!(h.len(), 1);
    assert_eq!(e.programmer().selection().len(), 5);
    assert_eq!(h.undo(&mut e).unwrap(), "201 Thro 205 @ 80 Enter");
    assert_eq!(e.edit_state(), before);
    assert!(e.programmer().selection().is_empty());
    assert!(e.programmer().values().is_empty());
}

// ---- UNDO-03 ----------------------------------------------------------------

#[test]
fn undo_03_one_patch_remove_brings_back_groups_palettes_and_cues_at_once() {
    let mut e = church();
    line(&mut e, "101 Thro 102 Enter").unwrap();
    e.record_group(1).unwrap();
    e.set_programmer_value(101, "Pan", 0.25).unwrap();
    e.set_programmer_value(102, "Pan", 0.75).unwrap();
    e.record_palette(&Record::at(Cell::new(Window::Positions, 1)))
        .unwrap();
    e.record_playback(&PlaybackRecord::at(1, 1)).unwrap();
    e.clear();
    e.clear();
    let before = e.edit_state();

    let mut h = History::default();
    h.edit(&mut e, "Patch remove 101", None, |e| e.patch_remove(&[101]))
        .unwrap();
    assert!(e.fixture(101).is_none());
    assert_eq!(e.group(1), Some(&[102][..]));

    h.undo(&mut e).unwrap();
    assert_eq!(e.edit_state(), before);
    assert!(e.fixture(101).is_some());
    assert_eq!(e.group(1), Some(&[101, 102][..]));
    assert_eq!(e.palette_value(1, 101, "Pan"), Some(Value::Level(0.25)));
    let pb = e.playbacks().next().unwrap();
    assert!(pb.cues[0].values.contains_key(&101));
}

// ---- UNDO-05 ----------------------------------------------------------------

#[test]
fn undo_05_wheel_turns_on_one_attribute_are_one_step_without_time() {
    let mut e = church();
    let mut h = History::default();
    cmd(&mut h, &mut e, "101 Enter");
    let before = e.edit_state();
    for _ in 0..3 {
        h.edit(&mut e, "Wheel Pan", Some("wheel:Pan"), |e| {
            e.turn_wheel("Pan", 10, Resolution::Normal)
        })
        .unwrap();
        e.tick(60.0); // a minute between turns changes nothing
    }
    assert_eq!(h.len(), 2);
    assert_eq!(h.undo(&mut e).unwrap(), "Wheel Pan");
    assert_eq!(e.edit_state(), before);
}

#[test]
fn undo_05_another_key_or_a_live_command_ends_the_run() {
    let mut e = church();
    let mut h = History::default();
    cmd(&mut h, &mut e, "101 Enter");
    let pan = |h: &mut History, e: &mut Engine| {
        h.edit(e, "Wheel Pan", Some("wheel:Pan"), |e| {
            e.turn_wheel("Pan", 10, Resolution::Normal)
        })
        .unwrap()
    };
    pan(&mut h, &mut e);
    h.edit(&mut e, "Wheel Tilt", Some("wheel:Tilt"), |e| {
        e.turn_wheel("Tilt", 10, Resolution::Normal)
    })
    .unwrap();
    pan(&mut h, &mut e);
    assert_eq!(h.len(), 4);
    h.live();
    pan(&mut h, &mut e);
    assert_eq!(h.len(), 5);
    // A command without a key ends the run as well.
    cmd(&mut h, &mut e, "102 Enter");
    pan(&mut h, &mut e);
    assert_eq!(h.len(), 7);
}

#[test]
fn undo_05_selection_taps_in_a_row_are_one_step() {
    let mut e = church();
    let mut h = History::default();
    for id in [201, 202, 203] {
        h.edit(&mut e, "Select", Some("select"), |e| e.select_fixture(id))
            .unwrap();
    }
    assert_eq!(h.len(), 1);
    h.undo(&mut e).unwrap();
    assert!(e.programmer().selection().is_empty());
}

// ---- UNDO-06 ----------------------------------------------------------------

#[test]
fn undo_06_undo_and_redo_swap_the_saved_state() {
    let mut e = church();
    let mut h = History::default();
    let empty = e.edit_state();
    cmd(&mut h, &mut e, "201 @ 50 Enter");
    let after = e.edit_state();
    assert_eq!(h.undo(&mut e).unwrap(), "201 @ 50 Enter");
    assert_eq!(e.edit_state(), empty);
    assert_eq!(
        (h.undo_label(), h.redo_label()),
        (None, Some("201 @ 50 Enter"))
    );
    assert_eq!(h.redo(&mut e).unwrap(), "201 @ 50 Enter");
    assert_eq!(e.edit_state(), after);
    assert_eq!(
        (h.undo_label(), h.redo_label()),
        (Some("201 @ 50 Enter"), None)
    );
}

#[test]
fn undo_06_empty_lists_are_refused_and_change_nothing() {
    let mut e = church();
    let mut h = History::default();
    let state = e.edit_state();
    let undo = h.undo(&mut e).unwrap_err();
    assert_eq!(undo, CoreError::NothingToUndo);
    assert_eq!(undo.to_string(), "Nothing to undo");
    let redo = h.redo(&mut e).unwrap_err();
    assert_eq!(redo, CoreError::NothingToRedo);
    assert_eq!(redo.to_string(), "Nothing to redo");
    assert_eq!(e.edit_state(), state);
    assert_eq!((h.len(), h.redo_len()), (0, 0));
}

// ---- UNDO-07 ----------------------------------------------------------------

#[test]
fn undo_07_a_new_step_empties_the_redo_list() {
    let mut e = church();
    let mut h = History::default();
    cmd(&mut h, &mut e, "201 @ 50 Enter");
    cmd(&mut h, &mut e, "202 @ 50 Enter");
    h.undo(&mut e).unwrap();
    h.undo(&mut e).unwrap();
    assert_eq!(h.redo_len(), 2);
    cmd(&mut h, &mut e, "203 @ 50 Enter");
    assert_eq!(h.redo_len(), 0);
    assert_eq!(h.redo(&mut e).unwrap_err(), CoreError::NothingToRedo);
}

// ---- UNDO-08 ----------------------------------------------------------------

#[test]
fn undo_08_the_history_keeps_the_newest_100_steps() {
    assert_eq!(DEPTH, 100);
    let mut e = church();
    let mut h = History::default();
    let mut after_first = None;
    for i in 0..=DEPTH {
        let level = (i + 1) as f32 / 200.0;
        h.edit(&mut e, &format!("Step {i}"), None, |e| {
            e.select_fixture(201).ok();
            e.set_programmer_value(201, "Dimmer", level)
        })
        .unwrap();
        if i == 0 {
            after_first = Some(e.edit_state());
        }
    }
    assert_eq!(h.len(), DEPTH);
    for _ in 0..DEPTH {
        h.undo(&mut e).unwrap();
    }
    assert_eq!(Some(e.edit_state()), after_first, "step 0 was dropped");
    assert_eq!(h.undo(&mut e).unwrap_err(), CoreError::NothingToUndo);
    assert_eq!(h.redo_len(), DEPTH);
}

// ---- UNDO-09 ----------------------------------------------------------------

#[test]
fn undo_09_undo_and_redo_leave_the_live_state_alone() {
    let mut e = church();
    e.add_playback(Playback::new(1, 1, 1, dimmer(201, 1.0)));
    e.set_fader(1, 0.6, 0.0).unwrap();
    e.set_grand_master(0.5).unwrap();
    e.set_page(3).unwrap();
    e.tick(0.0);
    let out = dmx(&e, 21);
    let mut h = History::default();
    cmd(&mut h, &mut e, "301 @ 100 Enter");
    h.undo(&mut e).unwrap();
    h.redo(&mut e).unwrap();
    h.undo(&mut e).unwrap();
    e.tick(0.025);
    let state = e.playback_state(1).unwrap();
    assert_eq!(state.level, 0.6);
    assert!(state.is_active());
    assert_eq!(e.grand_master(), 0.5);
    assert_eq!(e.page(), 3);
    assert_eq!(dmx(&e, 21), out, "201 at 0.6 · GM 0.5");
    assert_eq!(dmx(&e, 35), 0, "301 is undone");
}

// ---- UNDO-10 ----------------------------------------------------------------

#[test]
fn undo_10_removed_playback_is_killed() {
    let mut e = church();
    let mut h = History::default();
    h.edit(&mut e, "Record playback 1.1", None, |e| {
        e.add_playback(Playback::new(1, 1, 1, dimmer(201, 1.0)));
        Ok(())
    })
    .unwrap();
    e.set_fader(1, 1.0, 0.0).unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, 21), 255);
    h.undo(&mut e).unwrap();
    assert!(e.playback(1).is_none());
    assert!(e.playback_state(1).is_none(), "killed as Delete does");
    e.tick(0.025);
    assert_eq!(dmx(&e, 21), 0);
}

#[test]
fn undo_10_returned_playback_comes_back_not_running() {
    let mut e = church();
    e.add_playback(Playback::new(1, 1, 1, dimmer(201, 1.0)));
    e.set_fader(1, 1.0, 0.0).unwrap();
    e.tick(0.0);
    let mut h = History::default();
    h.edit(&mut e, "Delete playback 1.1", None, |e| {
        e.delete_playback(1)
    })
    .unwrap();
    e.tick(0.025);
    assert_eq!(dmx(&e, 21), 0);
    h.undo(&mut e).unwrap();
    assert!(e.playback(1).is_some());
    let state = e.playback_state(1).unwrap();
    assert!(!state.is_active());
    assert_eq!(state.level, 0.0);
    e.tick(0.05);
    assert_eq!(dmx(&e, 21), 0);
    e.set_fader(1, 1.0, 0.075).unwrap();
    e.tick(0.075);
    assert_eq!(dmx(&e, 21), 255, "it plays again as a recorded playback");
}

#[test]
fn undo_10_a_changed_playback_keeps_running_with_its_old_contents() {
    let mut e = church();
    e.add_playback(Playback::new(1, 1, 1, dimmer(201, 1.0)));
    e.set_fader(1, 1.0, 0.0).unwrap();
    e.tick(0.0);
    let mut h = History::default();
    h.edit(&mut e, "Record playback 1.1", None, |e| {
        e.add_playback(Playback::new(1, 1, 1, dimmer(201, 0.5)));
        Ok(())
    })
    .unwrap();
    e.tick(0.025);
    assert_eq!(dmx(&e, 21), 128);
    h.undo(&mut e).unwrap();
    assert!(e.playback_state(1).unwrap().is_active());
    e.tick(0.05);
    assert_eq!(dmx(&e, 21), 255);
}

#[test]
fn undo_10_a_group_master_keeps_its_current_level() {
    let mut e = church();
    line(&mut e, "201 Enter").unwrap();
    e.record_group(6).unwrap();
    e.clear();
    let id = e.record_group_master(1, 8, 6).unwrap();
    e.set_fader(id, 0.4, 0.0).unwrap();
    let mut h = History::default();
    h.edit(&mut e, "Page legend 2", None, |e| {
        e.set_page_legend(2, "Songs")
    })
    .unwrap();
    e.set_fader(id, 0.7, 0.1).unwrap();
    h.undo(&mut e).unwrap();
    assert_eq!(e.page_legend(2), Some(""));
    assert_eq!(e.playback_state(id).unwrap().level, 0.7);
    let Some(Master::Group(m)) = &e.playback(id).unwrap().master else {
        panic!("a group master");
    };
    assert_eq!(m.level, 0.7, "GRM-13: the saved level is live state");
}

// ---- UNDO-11 ----------------------------------------------------------------

#[test]
fn undo_11_undo_never_lowers_the_id_counters() {
    let mut e = church();
    let mut h = History::default();
    let first = h
        .edit(&mut e, "Patch add", None, |e| e.patch_add(&add_dimmer(100)))
        .unwrap();
    h.undo(&mut e).unwrap();
    let again = h
        .edit(&mut e, "Patch add", None, |e| e.patch_add(&add_dimmer(100)))
        .unwrap();
    assert_eq!(
        again[0],
        first[0] + 1,
        "PAT-12: the undone id is not reused"
    );

    line(&mut e, "201 @ 100 Enter").unwrap();
    let pb = h
        .edit(&mut e, "Record playback 1.1", None, |e| {
            e.record_playback(&PlaybackRecord::at(1, 1))
        })
        .unwrap();
    h.undo(&mut e).unwrap();
    let pb2 = h
        .edit(&mut e, "Record playback 1.1", None, |e| {
            e.record_playback(&PlaybackRecord::at(1, 1))
        })
        .unwrap();
    assert_eq!(pb2, pb + 1);

    line(&mut e, "101 Enter").unwrap();
    e.set_programmer_value(101, "Pan", 0.3).unwrap();
    let record = Record::at(Cell::new(Window::Positions, 1));
    let pal = h
        .edit(&mut e, "Record palette", None, |e| {
            e.record_palette(&record)
        })
        .unwrap();
    h.undo(&mut e).unwrap();
    let pal2 = h
        .edit(&mut e, "Record palette", None, |e| {
            e.record_palette(&record)
        })
        .unwrap();
    assert_eq!(pal2, pal + 1);
}

// ---- UNDO-12 ----------------------------------------------------------------

#[test]
fn undo_12_removed_fixtures_stop_and_returned_ones_output_again() {
    let mut e = church();
    line(&mut e, "201 @ 100 Enter").unwrap();
    e.tick(0.0);
    assert_eq!(dmx(&e, 21), 255);
    let mut h = History::default();
    h.edit(&mut e, "Patch remove 201", None, |e| e.patch_remove(&[201]))
        .unwrap();
    e.tick(0.025);
    assert_eq!(dmx(&e, 21), 0);
    h.undo(&mut e).unwrap();
    e.tick(0.05);
    assert_eq!(dmx(&e, 21), 255, "back with its programmer value");
    h.redo(&mut e).unwrap();
    e.tick(0.075);
    assert_eq!(dmx(&e, 21), 0);
}

#[test]
fn undo_12_an_undone_patch_move_outputs_at_the_old_address() {
    let mut e = church();
    line(&mut e, "201 @ 100 Enter").unwrap();
    let mut h = History::default();
    h.edit(&mut e, "Patch move 201", None, |e| {
        e.patch_move(201, 1, 100)
    })
    .unwrap();
    e.tick(0.0);
    assert_eq!((dmx(&e, 21), dmx(&e, 100)), (0, 255));
    h.undo(&mut e).unwrap();
    e.tick(0.025);
    assert_eq!((dmx(&e, 21), dmx(&e, 100)), (255, 0));
}

#[test]
fn undo_12_a_returned_fixture_starts_from_its_ltp_defaults() {
    let mut e = church();
    line(&mut e, "101 Enter").unwrap();
    e.set_programmer_value(101, "Zoom", 1.0).unwrap();
    e.tick(0.0);
    e.clear();
    e.clear();
    e.tick(0.025);
    assert_eq!(dmx(&e, 5), 255, "Zoom is held");
    let mut h = History::default();
    h.edit(&mut e, "Patch remove 101", None, |e| e.patch_remove(&[101]))
        .unwrap();
    h.undo(&mut e).unwrap();
    e.tick(0.05);
    assert_eq!(
        dmx(&e, 5),
        0,
        "a patch edit starts held values at the default"
    );
}

// ---- UNDO-16 ----------------------------------------------------------------

#[test]
fn undo_16_the_same_commands_give_the_same_history_and_state() {
    let session = || {
        let mut e = church();
        let mut h = History::default();
        cmd(&mut h, &mut e, "201 Thro 203 @ 70 Enter");
        h.edit(&mut e, "Record playback 1.1", None, |e| {
            e.record_playback(&PlaybackRecord::at(1, 1))
        })
        .unwrap();
        cmd(&mut h, &mut e, "101 Enter");
        h.edit(&mut e, "Wheel Pan", Some("wheel:Pan"), |e| {
            e.turn_wheel("Pan", 5, Resolution::Coarse)
        })
        .unwrap();
        h.undo(&mut e).unwrap();
        h.undo(&mut e).unwrap();
        h.redo(&mut e).unwrap();
        (
            e.edit_state(),
            h.undo_label().map(str::to_string),
            h.redo_label().map(str::to_string),
            h.len(),
        )
    };
    let a = session();
    assert_eq!(a, session());
    assert_eq!(a.1.as_deref(), Some("101 Enter"));
    assert_eq!(a.2.as_deref(), Some("Wheel Pan"));
}

// ---- UNDO-17 ----------------------------------------------------------------

#[test]
fn undo_17_ticks_never_touch_the_history() {
    let mut e = church();
    let mut h = History::default();
    cmd(&mut h, &mut e, "201 @ 50 Enter");
    for i in 0..40 {
        e.tick(f64::from(i) * 0.025);
    }
    assert_eq!((h.len(), h.redo_len()), (1, 0));
    assert_eq!(h.undo_label(), Some("201 @ 50 Enter"));
}
