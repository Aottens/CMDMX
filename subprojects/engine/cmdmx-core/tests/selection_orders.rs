//! Selection orders and the grid: `specs/modules/programmer.md` PRG-71..75, 84 and the PRG-05/06
//! amendments (SEL-01, P2-02). RGBW pars 201–208, RGB pars 221–226, spots 301–306; every fixture
//! is a one-channel dimmer in the church rig.
//!
//! API under test:
//! - `programmer::Order::{Reverse, Shuffle, Sort, Revert}`, `Engine::order(Order)`; fewer than two
//!   items → `CoreError::OrderTooShort`.
//! - `programmer::Grid { wings, groups, blocks, flip }`, `Grid::default()` = off `{1, 0, 1, false}`;
//!   `Engine::set_grid(Grid)`; out of range → `CoreError::InvalidGrid(_)`.
//! - `programmer::GridShortcut`, `Engine::grid_shortcut(GridShortcut)`; an empty selection →
//!   `CoreError::SelectFirst`.
//! - `Programmer::grid() -> Option<Grid>` (`None` when off), `stage() -> Option<usize>`,
//!   `positions() -> &[u32]` (per selection item, in selection order), `position_count() -> u32`,
//!   `select_order() -> &[Item]`.
//! - `effect::positions(n, wings, groups, blocks, flip)`.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

mod common;

use cmdmx_core::effect::positions;
use cmdmx_core::engine::Engine;
use cmdmx_core::patch::FixtureId;
use cmdmx_core::programmer::{Grid, GridShortcut, Order};
use cmdmx_core::undo::History;
use cmdmx_core::CoreError;
use common::{church, select};

const PARS: [FixtureId; 8] = [201, 202, 203, 204, 205, 206, 207, 208];

fn selection(e: &Engine) -> Vec<FixtureId> {
    e.programmer().selection()
}

fn targets(e: &Engine) -> Vec<FixtureId> {
    e.programmer().targets()
}

fn select_order(e: &Engine) -> Vec<FixtureId> {
    e.programmer()
        .select_order()
        .iter()
        .map(|i| i.fixture)
        .collect()
}

fn grid(wings: u8, groups: u8, blocks: u8, flip: bool) -> Grid {
    Grid {
        wings,
        groups,
        blocks,
        flip,
    }
}

fn pars() -> Engine {
    let mut e = church();
    select(&mut e, &PARS);
    e
}

fn run(e: &mut Engine, line: &str) -> Result<(), CoreError> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    e.execute_command(&tokens)
}

// ---- PRG-71 -----------------------------------------------------------------

#[test]
fn prg_71_reverse_sort_and_revert() {
    let mut e = church();
    select(&mut e, &[203, 201, 202]);
    e.order(Order::Reverse).unwrap();
    assert_eq!(selection(&e), vec![202, 201, 203]);
    e.order(Order::Sort).unwrap();
    assert_eq!(selection(&e), vec![201, 202, 203]);
    e.order(Order::Revert).unwrap();
    assert_eq!(selection(&e), vec![203, 201, 202]);
    assert_eq!(select_order(&e), vec![203, 201, 202]);
}

#[test]
fn prg_71_sort_is_by_user_number() {
    let mut e = church();
    e.patch_renumber(203, 1).unwrap();
    select(&mut e, &[201, 203, 202]);
    e.order(Order::Sort).unwrap();
    assert_eq!(selection(&e), vec![203, 201, 202]);
}

#[test]
fn prg_71_an_item_selected_after_a_reorder_goes_to_the_end_of_both() {
    let mut e = church();
    select(&mut e, &[203, 201, 202]);
    e.order(Order::Reverse).unwrap();
    select(&mut e, &[204]);
    assert_eq!(selection(&e), vec![202, 201, 203, 204]);
    assert_eq!(select_order(&e), vec![203, 201, 202, 204]);
    select(&mut e, &[201]); // tapped out of both
    assert_eq!(select_order(&e), vec![203, 202, 204]);
    e.order(Order::Revert).unwrap();
    assert_eq!(selection(&e), vec![203, 202, 204]);
}

#[test]
fn prg_71_order_tools_are_no_programmer_change() {
    let mut e = church();
    select(&mut e, &[201, 202, 203]);
    e.order(Order::Reverse).unwrap();
    select(&mut e, &[204]); // still adds (PRG-03)
    assert_eq!(selection(&e), vec![203, 202, 201, 204]);
    assert!(e.programmer().is_empty());
}

#[test]
fn prg_71_order_tools_keep_the_grid_and_the_stage_number() {
    let mut e = church();
    select(&mut e, &[201, 202, 203, 204, 205, 206]);
    e.grid_shortcut(GridShortcut::OddEven).unwrap();
    assert_eq!(targets(&e), vec![201, 203, 205]);
    e.order(Order::Reverse).unwrap();
    assert_eq!(e.programmer().grid(), Some(grid(1, 2, 1, false)));
    assert_eq!(e.programmer().stage(), Some(0));
    assert_eq!(targets(&e), vec![206, 204, 202]);
}

#[test]
fn prg_71_a_step_keeps_its_number_on_the_new_order() {
    let mut e = church();
    select(&mut e, &[201, 202, 203]);
    e.next_fixture();
    e.next_fixture();
    assert_eq!(targets(&e), vec![202]);
    e.order(Order::Reverse).unwrap();
    assert_eq!(targets(&e), vec![202]);
    e.next_fixture();
    assert_eq!(targets(&e), vec![201]);
}

#[test]
fn prg_71_fewer_than_two_items_is_refused() {
    let mut e = church();
    for tool in [Order::Reverse, Order::Shuffle, Order::Sort, Order::Revert] {
        assert_eq!(e.order(tool), Err(CoreError::OrderTooShort));
    }
    select(&mut e, &[201]);
    let before = e.programmer().clone();
    for tool in [Order::Reverse, Order::Shuffle, Order::Sort, Order::Revert] {
        assert_eq!(e.order(tool), Err(CoreError::OrderTooShort));
    }
    assert_eq!(e.programmer(), &before);
    assert_eq!(
        CoreError::OrderTooShort.to_string(),
        "Order: select at least two fixtures"
    );
}

#[test]
fn prg_71_order_and_grid_are_undone_with_the_selection() {
    let mut e = church();
    let mut h = History::default();
    h.edit(&mut e, "Select", Some("select"), |e| {
        select(e, &[203, 201, 202]);
        Ok(())
    })
    .unwrap();
    h.edit(&mut e, "Select", None, |e| e.order(Order::Reverse))
        .unwrap();
    h.edit(&mut e, "Select", None, |e| e.set_grid(grid(2, 0, 1, false)))
        .unwrap();
    assert_eq!(e.programmer().stage(), Some(0));
    h.undo(&mut e).unwrap();
    assert_eq!(e.programmer().grid(), None);
    assert_eq!(e.programmer().stage(), None);
    assert_eq!(selection(&e), vec![202, 201, 203]);
    h.undo(&mut e).unwrap();
    assert_eq!(selection(&e), vec![203, 201, 202]);
    assert_eq!(select_order(&e), vec![203, 201, 202]);
    h.redo(&mut e).unwrap();
    h.redo(&mut e).unwrap();
    assert_eq!(e.programmer().grid(), Some(grid(2, 0, 1, false)));
    assert_eq!(e.programmer().positions(), &[0, 1, 1]);
    assert_eq!(targets(&e), vec![202]);
}

// ---- PRG-72 -----------------------------------------------------------------

#[test]
fn prg_72_shuffle_is_seeded_by_the_counter_and_revert_undoes_it() {
    let mut e = pars();
    e.order(Order::Shuffle).unwrap();
    assert_eq!(selection(&e), vec![205, 204, 203, 208, 206, 207, 201, 202]);
    e.order(Order::Shuffle).unwrap();
    assert_eq!(selection(&e), vec![207, 203, 202, 206, 204, 208, 205, 201]);
    e.order(Order::Revert).unwrap();
    assert_eq!(selection(&e), PARS.to_vec());
}

#[test]
fn prg_72_same_actions_same_order() {
    let mut a = pars();
    let mut b = pars();
    for _ in 0..5 {
        a.order(Order::Shuffle).unwrap();
        b.order(Order::Shuffle).unwrap();
    }
    assert_eq!(selection(&a), selection(&b));
}

#[test]
fn prg_72_a_seed_that_gives_back_the_current_order_is_skipped() {
    let mut e = church();
    select(&mut e, &[201, 202]);
    // Seed 1 leaves two items in place, so seed 2 swaps them; seed 3 is skipped, seed 4 swaps.
    e.order(Order::Shuffle).unwrap();
    assert_eq!(selection(&e), vec![202, 201]);
    e.order(Order::Shuffle).unwrap();
    assert_eq!(selection(&e), vec![201, 202]);
    for _ in 0..20 {
        let before = selection(&e);
        e.order(Order::Shuffle).unwrap();
        assert_ne!(selection(&e), before);
    }
}

#[test]
fn prg_72_the_counter_is_live_state_and_not_undone() {
    let mut e = pars();
    let mut h = History::default();
    h.edit(&mut e, "Select", Some("select"), |e| {
        e.order(Order::Shuffle)
    })
    .unwrap();
    assert_eq!(selection(&e), vec![205, 204, 203, 208, 206, 207, 201, 202]);
    h.undo(&mut e).unwrap();
    assert_eq!(selection(&e), PARS.to_vec());
    h.edit(&mut e, "Select", Some("select"), |e| {
        e.order(Order::Shuffle)
    })
    .unwrap();
    // Seed 2 on the original order.
    assert_eq!(selection(&e), vec![206, 203, 208, 205, 202, 204, 201, 207]);
}

// ---- PRG-73 -----------------------------------------------------------------

#[test]
fn prg_73_the_grid_table() {
    let table: [(Grid, [u32; 8], u32); 7] = [
        (grid(1, 2, 1, false), [0, 1, 0, 1, 0, 1, 0, 1], 2),
        (grid(1, 3, 1, false), [0, 1, 2, 0, 1, 2, 0, 1], 3),
        (grid(1, 0, 2, false), [0, 0, 1, 1, 2, 2, 3, 3], 4),
        (grid(1, 0, 4, false), [0, 0, 0, 0, 1, 1, 1, 1], 2),
        (grid(2, 0, 1, false), [0, 1, 2, 3, 3, 2, 1, 0], 4),
        (grid(2, 0, 1, true), [3, 2, 1, 0, 0, 1, 2, 3], 4),
        (grid(1, 4, 2, false), [0, 0, 1, 1, 0, 0, 1, 1], 2),
    ];
    let mut e = pars();
    for (g, ks, n) in table {
        e.set_grid(g).unwrap();
        assert_eq!(e.programmer().grid(), Some(g));
        assert_eq!(e.programmer().positions(), &ks, "{g:?}");
        assert_eq!(e.programmer().position_count(), n, "{g:?}");
    }
}

#[test]
fn prg_73_grid_off_gives_each_item_its_own_position() {
    let mut e = pars();
    assert_eq!(e.programmer().grid(), None);
    assert_eq!(e.programmer().positions(), &[0, 1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(e.programmer().position_count(), 8);
    e.set_grid(grid(2, 0, 1, false)).unwrap();
    e.set_grid(Grid::default()).unwrap();
    assert_eq!(e.programmer().grid(), None);
    assert_eq!(e.programmer().stage(), None);
    assert_eq!(e.programmer().positions(), &[0, 1, 2, 3, 4, 5, 6, 7]);
}

#[test]
fn prg_73_flip_reverses_the_fx_07_positions() {
    assert_eq!(
        positions(8, 2, 0, 1, true),
        (vec![3, 2, 1, 0, 0, 1, 2, 3], 4)
    );
    assert_eq!(positions(4, 1, 0, 1, true), (vec![3, 2, 1, 0], 4));
    assert_eq!(positions(1, 1, 0, 1, true), (vec![0], 1));
    assert_eq!(positions(0, 1, 0, 1, true), (vec![], 0));
    assert_eq!(
        positions(8, 2, 0, 1, false),
        (vec![0, 1, 2, 3, 3, 2, 1, 0], 4)
    );
}

#[test]
fn prg_73_out_of_range_is_refused_and_changes_nothing() {
    let mut e = pars();
    e.set_grid(grid(1, 3, 1, false)).unwrap();
    e.next_fixture();
    let before = e.programmer().clone();
    for bad in [
        grid(0, 0, 1, false),
        grid(9, 0, 1, false),
        grid(1, 1, 1, false),
        grid(1, 33, 1, false),
        grid(1, 0, 0, false),
        grid(1, 0, 33, false),
    ] {
        assert!(
            matches!(e.set_grid(bad), Err(CoreError::InvalidGrid(_))),
            "{bad:?}"
        );
        assert_eq!(e.programmer(), &before);
    }
    for ok in [grid(8, 32, 32, true), grid(1, 2, 1, false)] {
        assert_eq!(e.set_grid(ok), Ok(()));
    }
}

#[test]
fn prg_73_the_grid_stays_while_items_are_tapped_in_or_out() {
    let mut e = pars();
    e.grid_shortcut(GridShortcut::OddEven).unwrap();
    select(&mut e, &[208]);
    assert_eq!(e.programmer().grid(), Some(grid(1, 2, 1, false)));
    assert_eq!(e.programmer().positions(), &[0, 1, 0, 1, 0, 1, 0]);
    select(&mut e, &[208, 301]);
    assert_eq!(e.programmer().positions(), &[0, 1, 0, 1, 0, 1, 0, 1, 0]);
    assert_eq!(e.programmer().position_count(), 2);
}

#[test]
fn prg_73_the_grid_stays_through_order_tools_steps_and_all() {
    let mut e = pars();
    e.grid_shortcut(GridShortcut::OutsideIn).unwrap();
    e.order(Order::Shuffle).unwrap();
    e.next_fixture();
    e.prev_fixture();
    e.select_all();
    assert_eq!(e.programmer().grid(), Some(grid(2, 0, 1, false)));
}

#[test]
fn prg_73_a_new_selection_ends_the_grid() {
    // After a programmer change (PRG-03).
    let mut e = pars();
    e.grid_shortcut(GridShortcut::OddEven).unwrap();
    e.set_programmer_value(201, "Dimmer", 1.0).unwrap();
    select(&mut e, &[202]);
    assert_eq!(selection(&e), vec![202]);
    assert_eq!(e.programmer().grid(), None);

    // A typed selection (PRG-08).
    let mut e = pars();
    e.grid_shortcut(GridShortcut::OddEven).unwrap();
    run(&mut e, "201 Thro 204 Enter").unwrap();
    assert_eq!(e.programmer().grid(), None);

    // All selecting the programmer (PRG-06).
    let mut e = pars();
    e.set_programmer_value(201, "Dimmer", 1.0).unwrap();
    select(&mut e, &[202, 203]);
    e.grid_shortcut(GridShortcut::OddEven).unwrap();
    e.select_all(); // ends the stage
    assert!(e.programmer().grid().is_some());
    e.select_all(); // selects the programmer
    assert_eq!(selection(&e), vec![201]);
    assert_eq!(e.programmer().grid(), None);

    // Clear of the selection.
    let mut e = pars();
    e.grid_shortcut(GridShortcut::OddEven).unwrap();
    e.clear_selection();
    assert_eq!(e.programmer().grid(), None);
    assert_eq!(e.programmer().stage(), None);

    // Tapping into an empty selection starts a new one.
    let mut e = pars();
    e.grid_shortcut(GridShortcut::OddEven).unwrap();
    select(&mut e, &PARS);
    assert!(selection(&e).is_empty());
    select(&mut e, &[201]);
    assert_eq!(e.programmer().grid(), None);
}

// ---- PRG-74 -----------------------------------------------------------------

#[test]
fn prg_74_stepping_by_position() {
    let mut e = pars();
    e.grid_shortcut(GridShortcut::OutsideIn).unwrap();
    assert_eq!(e.programmer().stage(), Some(0));
    assert_eq!(targets(&e), vec![201, 208]);
    e.next_fixture();
    assert_eq!(targets(&e), vec![202, 207]);
    e.grid_shortcut(GridShortcut::InsideOut).unwrap();
    assert_eq!(targets(&e), vec![204, 205]);
    e.grid_shortcut(GridShortcut::NInM(2, 4)).unwrap();
    assert_eq!(targets(&e), vec![201, 202, 205, 206]);
    e.next_fixture();
    assert_eq!(targets(&e), vec![203, 204, 207, 208]);
    e.next_fixture();
    assert_eq!(targets(&e), vec![201, 202, 205, 206]);
}

#[test]
fn prg_74_previous_wraps_to_the_last_position() {
    let mut e = pars();
    e.grid_shortcut(GridShortcut::OutsideIn).unwrap();
    e.prev_fixture();
    assert_eq!(e.programmer().stage(), Some(3));
    assert_eq!(targets(&e), vec![204, 205]);
}

#[test]
fn prg_74_all_ends_the_stage_and_keeps_the_grid() {
    let mut e = pars();
    e.grid_shortcut(GridShortcut::OutsideIn).unwrap();
    e.next_fixture();
    e.select_all();
    assert_eq!(e.programmer().stage(), None);
    assert_eq!(targets(&e), PARS.to_vec());
    assert_eq!(e.programmer().grid(), Some(grid(2, 0, 1, false)));
    e.next_fixture();
    assert_eq!(targets(&e), vec![201, 208]);
    e.select_all();
    e.prev_fixture();
    assert_eq!(targets(&e), vec![204, 205]);
}

#[test]
fn prg_74_grid_off_steps_one_item_as_prg_04() {
    let mut e = church();
    select(&mut e, &[101, 102, 103]);
    e.next_fixture();
    assert_eq!(e.programmer().stage(), Some(0));
    assert_eq!(targets(&e), vec![101]);
    e.prev_fixture();
    assert_eq!(targets(&e), vec![103]);
}

// ---- PRG-75 -----------------------------------------------------------------

#[test]
fn prg_75_shortcuts_set_the_whole_grid() {
    let cases = [
        (GridShortcut::OddEven, grid(1, 2, 1, false)),
        (GridShortcut::OneIn(3), grid(1, 3, 1, false)),
        (GridShortcut::NInM(2, 6), grid(1, 6, 2, false)),
        (GridShortcut::Blocks(2), grid(1, 0, 2, false)),
        (GridShortcut::Divide(2), grid(1, 0, 4, false)),
        (GridShortcut::Divide(3), grid(1, 0, 3, false)),
        (GridShortcut::OutsideIn, grid(2, 0, 1, false)),
        (GridShortcut::InsideOut, grid(2, 0, 1, true)),
    ];
    let mut e = pars();
    for (shortcut, g) in cases {
        e.grid_shortcut(GridShortcut::InsideOut).unwrap();
        e.next_fixture();
        e.grid_shortcut(shortcut).unwrap();
        assert_eq!(e.programmer().grid(), Some(g), "{shortcut:?}");
        assert_eq!(e.programmer().stage(), Some(0), "{shortcut:?}");
    }
    e.grid_shortcut(GridShortcut::Off).unwrap();
    assert_eq!(e.programmer().grid(), None);
    assert_eq!(e.programmer().stage(), None);
    assert_eq!(targets(&e), PARS.to_vec());
}

#[test]
fn prg_75_out_of_range_shortcuts_are_refused() {
    let mut e = pars();
    e.grid_shortcut(GridShortcut::OddEven).unwrap();
    let before = e.programmer().clone();
    for bad in [
        GridShortcut::OneIn(1),
        GridShortcut::OneIn(33),
        GridShortcut::NInM(0, 4),
        GridShortcut::NInM(4, 4),
        GridShortcut::NInM(5, 4),
        GridShortcut::NInM(2, 33),
        GridShortcut::Blocks(0),
        GridShortcut::Blocks(33),
        GridShortcut::Divide(1),
        GridShortcut::Divide(33),
    ] {
        assert!(
            matches!(e.grid_shortcut(bad), Err(CoreError::InvalidGrid(_))),
            "{bad:?}"
        );
        assert_eq!(e.programmer(), &before);
    }
}

#[test]
fn prg_75_an_empty_selection_is_refused() {
    let mut e = church();
    for shortcut in [
        GridShortcut::OddEven,
        GridShortcut::OneIn(3),
        GridShortcut::NInM(1, 2),
        GridShortcut::Blocks(2),
        GridShortcut::Divide(2),
        GridShortcut::OutsideIn,
        GridShortcut::InsideOut,
        GridShortcut::Off,
    ] {
        assert_eq!(e.grid_shortcut(shortcut), Err(CoreError::SelectFirst));
    }
    assert_eq!(e.programmer().grid(), None);
    assert_eq!(CoreError::SelectFirst.to_string(), "Select fixtures first");
}

#[test]
fn prg_75_a_grid_on_one_item_is_allowed() {
    let mut e = church();
    select(&mut e, &[201]);
    e.grid_shortcut(GridShortcut::OutsideIn).unwrap();
    assert_eq!(e.programmer().positions(), &[0]);
    assert_eq!(e.programmer().position_count(), 1);
    assert_eq!(targets(&e), vec![201]);
    e.next_fixture();
    assert_eq!(targets(&e), vec![201]);
}

#[test]
fn prg_75_twelve_pars_two_in_six() {
    let mut e = church();
    select(&mut e, &PARS);
    select(&mut e, &[221, 222, 223, 224]);
    e.grid_shortcut(GridShortcut::NInM(2, 6)).unwrap();
    assert_eq!(
        e.programmer().positions(),
        &[0, 0, 1, 1, 2, 2, 0, 0, 1, 1, 2, 2]
    );
    assert_eq!(e.programmer().position_count(), 3);
}

#[test]
fn prg_75_divide_uses_the_length_at_that_moment() {
    let mut e = pars();
    e.grid_shortcut(GridShortcut::Divide(2)).unwrap();
    assert_eq!(e.programmer().grid(), Some(grid(1, 0, 4, false)));
    select(&mut e, &[221, 222, 223, 224]); // the blocks stay 4
    assert_eq!(e.programmer().grid(), Some(grid(1, 0, 4, false)));
    assert_eq!(
        e.programmer().positions(),
        &[0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2]
    );
    let mut e = church();
    select(&mut e, &[201]);
    e.grid_shortcut(GridShortcut::Divide(4)).unwrap(); // ⌈1/4⌉ = 1
    assert_eq!(e.programmer().grid(), None);
}

// ---- PRG-05 / PRG-06 amended ------------------------------------------------

#[test]
fn prg_05_a_pattern_is_the_grid_groups_n() {
    let mut e = pars();
    e.select_pattern(3).unwrap();
    assert_eq!(e.programmer().grid(), Some(grid(1, 3, 1, false)));
    assert_eq!(e.programmer().stage(), Some(0));
    assert_eq!(targets(&e), vec![201, 204, 207]);
    let before = e.programmer().clone();
    assert_eq!(e.select_pattern(33), Err(CoreError::InvalidPattern(33)));
    assert_eq!(e.programmer(), &before);
}

#[test]
fn prg_06_all_ends_the_pattern_stage_but_keeps_the_grid() {
    let mut e = pars();
    e.select_pattern(2).unwrap();
    e.select_all();
    assert_eq!(targets(&e), PARS.to_vec());
    assert_eq!(e.programmer().grid(), Some(grid(1, 2, 1, false)));
}

// ---- PRG-84 -----------------------------------------------------------------

#[test]
fn prg_84_positions_never_panic_and_stay_in_range() {
    for n in 0..=40usize {
        for wings in 0..=9u8 {
            for groups in (0..=33u8).step_by(3) {
                for blocks in 0..=33u8 {
                    for flip in [false, true] {
                        let (ks, count) = positions(n, wings, groups, blocks, flip);
                        assert_eq!(ks.len(), n);
                        assert_eq!(count == 0, n == 0);
                        assert!(ks.iter().all(|&k| k < count));
                    }
                }
            }
        }
    }
}

#[test]
fn prg_84_stages_over_small_and_odd_selections_never_panic() {
    let mut e = church();
    for g in [
        grid(8, 32, 32, true),
        grid(3, 2, 5, false),
        grid(2, 0, 1, true),
    ] {
        for n in 0..=PARS.len() {
            e.clear_selection();
            select(&mut e, &PARS[..n]);
            let _ = e.set_grid(g);
            for _ in 0..3 {
                e.next_fixture();
                let _ = e.order(Order::Shuffle);
                select(&mut e, &PARS[..n.min(1)]); // tap one out and back
                select(&mut e, &PARS[..n.min(1)]);
                e.prev_fixture();
                assert_eq!(e.programmer().positions().len(), selection(&e).len());
                let count = e.programmer().position_count();
                assert!(e.programmer().positions().iter().all(|&k| k < count));
                assert!(targets(&e).iter().all(|t| selection(&e).contains(t)));
            }
            e.select_all();
            let _ = e.order(Order::Revert);
            let _ = e.grid_shortcut(GridShortcut::Divide(32));
        }
    }
}
