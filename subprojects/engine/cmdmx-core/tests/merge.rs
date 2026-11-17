//! Merge from another show, core (MRG-01): `specs/modules/showfile.md` SHW-28..31. Reading the
//! file, the wire and the lock (SHW-27, 31) are `cmdmx-daemon/tests/merge.rs`.
//!
//! API these tests fix (`cmdmx_core::merge` and `Engine`):
//! - `merge::FixtureMap = BTreeMap<u32, Option<u32>>`: the other show's fixture number → the open
//!   show's number, `None` = unmapped.
//! - `merge::proposed_map(&Engine, &Show) -> FixtureMap` (SHW-28) and
//!   `merge::contents(&Engine, &Show) -> Contents` (SHW-27's lists, `Serialize`).
//! - `merge::Request { map, groups, palettes, playbacks: Vec<Slot { page, slot }> }`
//!   (`Deserialize`, every field defaulted); `map` holds only the operator's changed lines.
//! - `Engine::merge(&mut self, other: &Show, request: &Request, dry_run: bool)
//!   -> Result<Plan, CoreError>`; a refusal changes nothing, a dry run changes nothing.
//! - `Plan { placed: Vec<Placed { kind, from, to, brought }>, left_out: LeftOut { fixtures,
//!   values } }` (`Serialize`); `kind` is "group", "palette" or "playback"; `from` and `to` are a
//!   number ("7") or a handle ("1.2"). A palette or group linked to an identical one of the open
//!   show is not placed. `Plan::summary(name) -> String` is the notice of SHW-31.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

mod common;

use std::collections::BTreeMap;

use cmdmx_core::attribute::{AttributeDef, AttributeGroup, AttributeKind};
use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::merge::{self, FixtureMap, LeftOut, Placed, PlacedKind, Request, Slot};
use cmdmx_core::palette::{Palette, Window};
use cmdmx_core::patch::Fixture;
use cmdmx_core::playback::{Cue, GroupMaster, Kind, Master, MasterKind, Playback, Values};
use cmdmx_core::profile::Profile;
use cmdmx_core::programmer::Item;
use cmdmx_core::show::{Meta, Show, ShowGroup};
use cmdmx_core::undo::History;
use cmdmx_core::value::{Reference, Stored, Value};

fn meta(name: &str) -> Meta {
    Meta {
        name: name.into(),
        created: "2026-11-09T09:00:00Z".into(),
        modified: "2026-11-09T09:00:00Z".into(),
        app_version: "0.1.0".into(),
    }
}

fn dimmer_profile() -> Profile {
    Profile::from_attributes(vec![AttributeDef::new(
        "Dimmer",
        AttributeGroup::I,
        AttributeKind::Htp,
        1,
    )])
}

fn fixture(id: u32, number: u32, profile: &str, address: u16) -> Fixture {
    Fixture {
        parked: false,
        behaviour: Default::default(),
        swap_pan_tilt: false,
        id,
        number,
        name: String::new(),
        handle: None,
        profile: profile.into(),
        mode: None,
        universe: 1,
        address,
    }
}

fn item(id: u32) -> Item {
    id.into()
}

fn dim(level: f32) -> BTreeMap<String, Value> {
    BTreeMap::from([("Dimmer".to_string(), Value::Level(level))])
}

fn palette(id: u32, number: u32, window: Window, handle: u16, values: &[(u32, f32)]) -> Palette {
    Palette {
        id,
        number,
        window,
        handle,
        legend: format!("P{number}"),
        icon: None,
        global: BTreeMap::new(),
        shared: BTreeMap::new(),
        shared_cells: BTreeMap::new(),
        fixtures: values.iter().map(|&(f, l)| (item(f), dim(l))).collect(),
        effects: Vec::new(),
    }
}

fn values(entries: &[(u32, Stored)]) -> Values {
    entries
        .iter()
        .map(|&(f, s)| (item(f), BTreeMap::from([("Dimmer".to_string(), s)])))
        .collect()
}

fn level(l: f32) -> Stored {
    Stored::Value(Value::Level(l))
}

fn reference(palette: u32, l: f32) -> Stored {
    Stored::Reference(Reference {
        palette,
        value: Value::Level(l),
    })
}

/// The open show: the church rig (`common::church()`, ids = numbers: washes 101–104, dimmers
/// 201–208, 221–226, 301–306) with group 1 = 203, 204; palette 1 (Colours cell 1, 205 at 10 %);
/// palette 4 (Positions cell 1, 201 at 70 %); a memory on 1.1 putting 205 at full; the bpm
/// master 1 on 3.1.
fn open() -> Engine {
    let mut e = common::church();
    common::select(&mut e, &[203, 204]);
    e.record_group(1).unwrap();
    e.clear_selection();
    e.add_palette(palette(50, 1, Window::Colours, 1, &[(205, 0.1)]))
        .unwrap();
    e.add_palette(palette(51, 4, Window::Positions, 1, &[(201, 0.7)]))
        .unwrap();
    e.add_playback(Playback::new(60, 1, 1, values(&[(205, level(1.0))])));
    e.add_playback(Playback {
        kind: Kind::BpmMaster,
        master: Some(Master::new(MasterKind::Bpm, 1)),
        cues: Vec::new(),
        ..Playback::new(61, 3, 1, Values::new())
    });
    e
}

/// The other show, "Kerk 9 nov". Its fixture ids differ from their numbers:
/// id 1201 = number 201, 1202 = 202, 1301 = 301 (dimmers, as in the open show);
/// 1101 = number 101 is a dimmer here and a wash there (unmapped); 1999 = number 999 (unmapped).
/// - group 1 "Front" = 1201, 1202; group 5 "Spot" = 1301, 1999; group 7 = 1201.
/// - palette 1 (id 10, Colours cell 1): 1201 at 50 %.
/// - palette 3 (id 11, Beams cell 4): 1202 at 30 %, 1999 at 90 %.
/// - palette 4 (id 12, Positions cell 1): 1201 at 70 % — identical to the open show's palette 4.
/// - 1.1 memory "Welcome" (id 1): 1201 → palette 11, 1202 → palette 10, 1999 at full.
/// - 2.1 group master of group 7 at 40 % (id 2).
/// - 2.3 cue list "Walk-in" (id 3): cue 1 (fade 3 s, legend "Doors") 1202 → palette 12;
///   cue 2.5 (legend "Dim") 1301 at 20 %.
/// - 4.1 bpm master 1 (id 4).
fn other() -> Show {
    let mut e = Engine::new(Settings::default());
    e.add_profile("dimmer", dimmer_profile()).unwrap();
    for (n, (id, number)) in [
        (1201, 201),
        (1202, 202),
        (1301, 301),
        (1101, 101),
        (1999, 999),
    ]
    .into_iter()
    .enumerate()
    {
        e.patch(fixture(id, number, "dimmer", 100 + n as u16))
            .unwrap();
    }
    e.add_palette(palette(10, 1, Window::Colours, 1, &[(1201, 0.5)]))
        .unwrap();
    e.add_palette(palette(
        11,
        3,
        Window::Beams,
        4,
        &[(1202, 0.3), (1999, 0.9)],
    ))
    .unwrap();
    e.add_palette(palette(12, 4, Window::Positions, 1, &[(1201, 0.7)]))
        .unwrap();
    e.add_playback(Playback {
        legend: "Welcome".into(),
        ..Playback::new(
            1,
            1,
            1,
            values(&[
                (1201, reference(11, 0.3)),
                (1202, reference(10, 0.5)),
                (1999, level(1.0)),
            ]),
        )
    });
    e.add_playback(Playback {
        kind: Kind::GroupMaster,
        master: Some(Master::Group(GroupMaster {
            level: 0.4,
            ..GroupMaster::new(7)
        })),
        cues: Vec::new(),
        ..Playback::new(2, 2, 1, Values::new())
    });
    let mut first = Cue::new(values(&[(1202, reference(12, 0.7))]));
    first.fade_in = 3.0;
    first.legend = "Doors".into();
    let mut second = Cue::new(values(&[(1301, level(0.2))]));
    second.number = 2.5.into();
    second.legend = "Dim".into();
    e.add_playback(Playback {
        legend: "Walk-in".into(),
        cues: vec![first, second],
        ..Playback::new(3, 2, 3, Values::new())
    });
    e.add_playback(Playback {
        kind: Kind::BpmMaster,
        master: Some(Master::new(MasterKind::Bpm, 1)),
        cues: Vec::new(),
        ..Playback::new(4, 4, 1, Values::new())
    });
    let mut show = e.to_show(meta("Kerk 9 nov"));
    for (id, items, legend) in [
        (1, vec![1201, 1202], "Front"),
        (5, vec![1301, 1999], "Spot"),
        (7, vec![1201], ""),
    ] {
        show.groups.push(ShowGroup {
            id,
            items: items.into_iter().map(item).collect(),
            grid: None,
            legend: legend.into(),
        });
    }
    show
}

fn slot(page: u16, slot: u16) -> Slot {
    Slot { page, slot }
}

/// Groups 1 and 5, palette 1 and the playbacks on 1.1, 2.1 and 2.3, with the proposed map.
fn request() -> Request {
    Request {
        map: FixtureMap::new(),
        groups: vec![1, 5],
        palettes: vec![1],
        playbacks: vec![slot(1, 1), slot(2, 1), slot(2, 3)],
    }
}

fn placed(kind: PlacedKind, from: &str, to: &str, brought: bool) -> Placed {
    Placed {
        kind,
        from: from.into(),
        to: to.into(),
        brought,
    }
}

fn at(e: &Engine, page: u16, slot: u16) -> &Playback {
    e.playbacks()
        .find(|p| (p.page, p.slot) == (page, slot))
        .unwrap_or_else(|| panic!("nothing on {page}.{slot}"))
}

fn dimmer_of(values: &Values, id: u32) -> &Stored {
    &values[&item(id)]["Dimmer"]
}

// ---- SHW-28 -----------------------------------------------------------------

#[test]
fn shw_28_each_fixture_is_proposed_onto_the_same_number_with_the_same_profile() {
    let map = merge::proposed_map(&open(), &other());
    assert_eq!(
        map,
        FixtureMap::from([
            (101, None), // a dimmer there, a wash here
            (201, Some(201)),
            (202, Some(202)),
            (301, Some(301)),
            (999, None), // no 999 here
        ])
    );
    // SHW-27: the lists the app shows, with the proposed map as `to`.
    let contents = serde_json::to_value(merge::contents(&open(), &other())).unwrap();
    let fixtures = contents["fixtures"].as_array().unwrap();
    assert_eq!(fixtures.len(), 5);
    let f999 = fixtures.iter().find(|f| f["number"] == 999).unwrap();
    assert_eq!(f999["profile"], "dimmer");
    assert!(f999["to"].is_null());
    let f201 = fixtures.iter().find(|f| f["number"] == 201).unwrap();
    assert_eq!(f201["to"], 201);
    let groups: Vec<_> = contents["groups"].as_array().unwrap().iter().collect();
    assert_eq!(groups.len(), 3);
    assert_eq!(groups[0]["number"], 1);
    assert_eq!(groups[0]["legend"], "Front");
    // Items in the other show's fixture numbers, not its ids.
    assert_eq!(groups[0]["items"], serde_json::json!(["201", "202"]));
    let palettes = contents["palettes"].as_array().unwrap();
    assert_eq!(palettes.len(), 3);
    assert_eq!(palettes[1]["number"], 3);
    assert_eq!(palettes[1]["window"], "Beams");
    assert_eq!(palettes[1]["legend"], "P3");
    let playbacks = contents["playbacks"].as_array().unwrap();
    assert_eq!(playbacks.len(), 4);
    let walk_in = playbacks.iter().find(|p| p["legend"] == "Walk-in").unwrap();
    assert_eq!(
        (walk_in["page"].clone(), walk_in["slot"].clone()),
        (2.into(), 3.into())
    );
    assert_eq!(walk_in["kind"], "Cues");
    assert_eq!(walk_in["cues"], 2);
    let master = playbacks.iter().find(|p| p["page"] == 2 && p["slot"] == 1);
    assert_eq!(master.unwrap()["kind"], "GroupMaster");
}

#[test]
fn shw_28_values_items_and_members_follow_the_map_from_ids_to_ids() {
    let mut e = open();
    e.merge(&other(), &request(), false).unwrap();
    // Group 1 of the other show (1201, 1202) lands as group 2 holding the open show's 201, 202.
    assert_eq!(e.groups()[&2], vec![item(201), item(202)]);
    let welcome = at(&e, 1, 2);
    let values = &welcome.cues[0].values;
    assert!(values.contains_key(&item(201)));
    assert!(values.contains_key(&item(202)));
    assert!(!values.contains_key(&item(1201)));
    assert!(!values.contains_key(&item(999)) && !values.contains_key(&item(1999)));
}

#[test]
fn shw_28_an_edited_line_moves_a_fixture_and_null_leaves_it_out() {
    let mut e = open();
    let mut req = request();
    req.map = FixtureMap::from([(201, Some(203)), (202, None)]);
    let plan = e.merge(&other(), &req, false).unwrap();
    assert_eq!(e.groups()[&2], vec![item(203)]);
    let values = &at(&e, 1, 2).cues[0].values;
    assert!(values.contains_key(&item(203)));
    assert!(!values.contains_key(&item(201)) && !values.contains_key(&item(202)));
    assert_eq!(plan.left_out.fixtures, vec![202, 999]);
}

#[test]
fn shw_28_a_bad_map_is_refused_naming_the_first_bad_line_and_nothing_changes() {
    for (map, line) in [
        // No fixture 555 in the open show.
        (FixtureMap::from([(201, Some(555))]), "201"),
        // 101 is a wash here, 201 a dimmer there.
        (FixtureMap::from([(201, Some(101))]), "201"),
        // 202 already takes 202.
        (FixtureMap::from([(201, Some(202))]), "201"),
        // The other show has no fixture 777.
        (FixtureMap::from([(777, Some(205))]), "777"),
    ] {
        let mut e = open();
        let before = e.edit_state();
        let mut req = request();
        req.map = map.clone();
        let err = e.merge(&other(), &req, false).unwrap_err().to_string();
        assert!(err.contains(line), "{map:?}: {err}");
        assert_eq!(e.edit_state(), before, "{map:?}");
    }
}

#[test]
fn shw_28_never_patches_a_fixture_or_adds_a_profile() {
    let mut e = open();
    let fixtures: Vec<_> = e.to_show(meta("x")).patch.fixtures;
    let profiles: Vec<_> = e.to_show(meta("x")).profiles.into_keys().collect();
    e.merge(&other(), &request(), false).unwrap();
    assert_eq!(e.to_show(meta("x")).patch.fixtures, fixtures);
    assert_eq!(
        e.to_show(meta("x"))
            .profiles
            .into_keys()
            .collect::<Vec<_>>(),
        profiles
    );
}

// ---- SHW-29 -----------------------------------------------------------------

#[test]
fn shw_29_everything_lands_on_a_free_place_and_nothing_is_overwritten() {
    let mut e = open();
    let plan = e.merge(&other(), &request(), false).unwrap();
    assert_eq!(
        plan.placed,
        vec![
            placed(PlacedKind::Group, "1", "2", false), // 1 is taken: the lowest free number
            placed(PlacedKind::Group, "5", "5", false), // free: keeps it
            placed(PlacedKind::Group, "7", "7", true),  // the group master's, brought along
            placed(PlacedKind::Palette, "1", "2", false), // number 1 and Colours cell 1 taken
            placed(PlacedKind::Palette, "3", "3", true), // the memory's, brought along, both free
            placed(PlacedKind::Playback, "1.1", "1.2", false), // 1.1 taken: first free slot on page 1
            placed(PlacedKind::Playback, "2.1", "2.1", false),
            placed(PlacedKind::Playback, "2.3", "2.3", false),
        ]
    );
    // The open show's own are untouched.
    assert_eq!(e.groups()[&1], vec![item(203), item(204)]);
    assert_eq!(e.palette(1).unwrap().fixtures[&item(205)], dim(0.1));
    assert_eq!(
        e.playback(60).unwrap().cues[0].values,
        values(&[(205, level(1.0))])
    );
    // The palette took number 2 and the first free cell of its window.
    let p2 = e.palette(2).unwrap();
    assert_eq!((p2.window, p2.handle), (Window::Colours, 2));
    assert_eq!(p2.fixtures, BTreeMap::from([(item(201), dim(0.5))]));
    assert_eq!(p2.legend, "P1");
    let p3 = e.palette(3).unwrap();
    assert_eq!((p3.window, p3.handle), (Window::Beams, 4));
    // Legends of groups travel too.
    assert_eq!(e.group_legend(2), Some("Front"));
    assert_eq!(e.group_legend(5), Some("Spot"));
}

#[test]
fn shw_29_cue_lists_keep_numbers_times_options_and_legends() {
    let mut e = open();
    e.merge(&other(), &request(), false).unwrap();
    let walk_in = at(&e, 2, 3);
    assert_eq!(walk_in.legend, "Walk-in");
    assert_eq!(walk_in.cues.len(), 2);
    assert_eq!(walk_in.cues[0].legend, "Doors");
    assert_eq!(walk_in.cues[0].fade_in, 3.0);
    assert_eq!(walk_in.cues[1].number, 2.5.into());
    assert_eq!(walk_in.cues[1].legend, "Dim");
    assert_eq!(at(&e, 1, 2).legend, "Welcome");
    // A new id, above every id the open show used.
    assert!(at(&e, 2, 3).id > 61);
}

#[test]
fn shw_29_a_full_page_spills_to_the_pages_after_it_never_before() {
    let mut e = open();
    let fpp = Settings::default().faders_per_page;
    for s in 2..=fpp {
        e.add_playback(Playback::new(100 + u32::from(s), 1, s, Values::new()));
    }
    let req = Request {
        playbacks: vec![slot(1, 1)],
        ..Request::default()
    };
    let plan = e.merge(&other(), &req, false).unwrap();
    assert_eq!(plan.placed.last().unwrap().to, "2.1");
}

#[test]
fn shw_29_what_does_not_fit_or_does_not_exist_is_refused_as_a_whole() {
    let fpp = Settings::default().faders_per_page;
    let full = || {
        let mut e = open();
        for page in 1..=cmdmx_core::playback::PAGES {
            for s in 1..=fpp {
                if e.playbacks().all(|p| (p.page, p.slot) != (page, s)) {
                    e.add_playback(Playback::new(
                        1000 + u32::from(page * 100 + s),
                        page,
                        s,
                        Values::new(),
                    ));
                }
            }
        }
        e
    };
    let cases: Vec<(Engine, Request)> = vec![
        // No free slot on page 1 or after it; the groups in the request are not taken either.
        (full(), request()),
        // The other show has no group 42.
        (
            open(),
            Request {
                groups: vec![1, 42],
                ..Request::default()
            },
        ),
        // No palette 9 there.
        (
            open(),
            Request {
                palettes: vec![9],
                ..Request::default()
            },
        ),
        // Nothing on 5.5 there.
        (
            open(),
            Request {
                playbacks: vec![slot(5, 5)],
                ..Request::default()
            },
        ),
        // The bpm master 1 is in use here: it does not fit.
        (
            open(),
            Request {
                playbacks: vec![slot(4, 1)],
                ..Request::default()
            },
        ),
    ];
    for (n, (mut e, req)) in cases.into_iter().enumerate() {
        let before = e.edit_state();
        assert!(e.merge(&other(), &req, false).is_err(), "case {n}");
        assert_eq!(e.edit_state(), before, "case {n}");
    }
}

// ---- SHW-30 -----------------------------------------------------------------

#[test]
fn shw_30_palettes_and_groups_used_come_along_and_references_follow() {
    let mut e = open();
    e.merge(&other(), &request(), false).unwrap();
    let id_of = |e: &Engine, number: u32| e.palette(number).unwrap().id;
    let welcome = &at(&e, 1, 2).cues[0].values;
    // Palette 3 was not ticked but the memory uses it: it came along as number 3.
    assert_eq!(dimmer_of(welcome, 201), &reference(id_of(&e, 3), 0.3),);
    // Palette 1 of the other show is now number 2; the reference never points to the open
    // show's own palette 1, which only shares the number.
    assert_eq!(dimmer_of(welcome, 202), &reference(id_of(&e, 2), 0.5));
    // Palette 4 is identical here (same window, number and values after the map): linked.
    let walk_in = &at(&e, 2, 3).cues[0].values;
    assert_eq!(dimmer_of(walk_in, 202), &reference(51, 0.7));
    assert_eq!(e.palettes().filter(|p| p.number == 4).count(), 1);
    // The group master masters the group it brought along.
    let Some(Master::Group(m)) = &at(&e, 2, 1).master else {
        panic!("not a group master")
    };
    assert_eq!(m.group, 7);
    assert_eq!(e.groups()[&7], vec![item(201)]);
}

#[test]
fn shw_30_an_identical_group_is_linked_not_copied() {
    let mut e = open();
    common::select(&mut e, &[201]);
    e.record_group(7).unwrap();
    e.clear_selection();
    let groups = e.groups().len();
    let req = Request {
        playbacks: vec![slot(2, 1)],
        ..Request::default()
    };
    let plan = e.merge(&other(), &req, false).unwrap();
    assert_eq!(
        plan.placed,
        vec![placed(PlacedKind::Playback, "2.1", "2.1", false)]
    );
    assert_eq!(e.groups().len(), groups);
}

#[test]
fn shw_30_a_group_with_the_same_number_but_other_items_comes_along_on_a_free_number() {
    let mut e = open();
    common::select(&mut e, &[202]);
    e.record_group(7).unwrap();
    e.clear_selection();
    let req = Request {
        playbacks: vec![slot(2, 1)],
        ..Request::default()
    };
    let plan = e.merge(&other(), &req, false).unwrap();
    assert_eq!(plan.placed[0], placed(PlacedKind::Group, "7", "2", true));
    let Some(Master::Group(m)) = &at(&e, 2, 1).master else {
        panic!("not a group master")
    };
    assert_eq!(m.group, 2);
    assert_eq!(e.groups()[&7], vec![item(202)]);
}

// ---- SHW-31 -----------------------------------------------------------------

#[test]
fn shw_31_values_of_unmapped_fixtures_are_left_out_and_counted() {
    let mut e = open();
    let plan = e.merge(&other(), &request(), false).unwrap();
    // 1999 (number 999): one item of group 5, one value of palette 3, one value of 1.1.
    assert_eq!(
        plan.left_out,
        LeftOut {
            fixtures: vec![999],
            values: 3
        }
    );
    assert_eq!(e.groups()[&5], vec![item(301)]);
}

#[test]
fn shw_31_a_merge_is_one_undo_step_and_nothing_on_stage_changes() {
    let mut e = open();
    e.set_page(3).unwrap();
    common::select(&mut e, &[221]);
    let selection = e.programmer().items().to_vec();
    let before = e.edit_state();
    let mut h = History::default();
    h.edit(&mut e, "Merge from Kerk 9 nov", None, |e| {
        e.merge(&other(), &request(), false)
    })
    .unwrap();
    assert_eq!(h.len(), 1);
    assert_eq!(h.undo_label(), Some("Merge from Kerk 9 nov"));
    // Merged playbacks arrive off, at level 0; the group master at the level where it changes
    // nothing (GRM-05), so the stage does not change.
    for (page, s) in [(1, 2), (2, 3)] {
        let state = e.playback_state(at(&e, page, s).id).unwrap();
        assert_eq!(state.level, 0.0);
        assert!(state.activated.is_none());
    }
    let master = at(&e, 2, 1);
    let Some(Master::Group(m)) = &master.master else {
        panic!("not a group master")
    };
    assert_eq!(m.level, 1.0);
    assert_eq!(e.playback_state(master.id).unwrap().level, 1.0);
    // The programmer, selection and page stay.
    assert_eq!(e.page(), 3);
    assert_eq!(e.programmer().items(), selection.as_slice());
    // One undo puts everything back.
    h.undo(&mut e).unwrap();
    assert_eq!(e.edit_state(), before);
}

#[test]
fn shw_31_a_dry_run_returns_the_same_plan_and_changes_nothing() {
    let mut e = open();
    let before = e.edit_state();
    let dry = e.merge(&other(), &request(), true).unwrap();
    assert_eq!(e.edit_state(), before);
    let real = e.merge(&other(), &request(), false).unwrap();
    assert_eq!(dry, real);
    assert_ne!(e.edit_state(), before);
}

#[test]
fn shw_31_the_notice_counts_what_was_placed_and_left_out() {
    let mut e = open();
    let plan = e.merge(&other(), &request(), false).unwrap();
    assert_eq!(
        plan.summary("Kerk 9 nov"),
        "Merged from Kerk 9 nov: 3 playbacks, 2 palettes, 3 groups; 1 fixture not in this show, 3 values left out"
    );
    let mut e = open();
    let req = Request {
        groups: vec![1],
        ..Request::default()
    };
    let plan = e.merge(&other(), &req, false).unwrap();
    assert_eq!(
        plan.summary("Kerk 9 nov"),
        "Merged from Kerk 9 nov: 1 group"
    );
}

#[test]
fn shw_31_the_plan_reads_on_the_wire() {
    let mut e = open();
    let plan = e.merge(&other(), &request(), true).unwrap();
    let v = serde_json::to_value(&plan).unwrap();
    assert_eq!(
        v["placed"][0],
        serde_json::json!({"kind": "group", "from": "1", "to": "2", "brought": false})
    );
    assert_eq!(
        v["left_out"],
        serde_json::json!({"fixtures": [999], "values": 3})
    );
    // The request reads from the wire with the map's keys as strings.
    let req: Request = serde_json::from_value(serde_json::json!({
        "map": {"201": 203, "202": null},
        "groups": [1],
        "playbacks": [{"page": 2, "slot": 3}]
    }))
    .unwrap();
    assert_eq!(req.map, FixtureMap::from([(201, Some(203)), (202, None)]));
    assert_eq!(req.playbacks, vec![slot(2, 3)]);
    assert!(req.palettes.is_empty());
}

#[test]
fn shw_31_merged_effects_take_new_entry_ids_and_lose_unmapped_members() {
    let entry = |id: u32, members: &[&str]| -> cmdmx_core::effect::Entry {
        serde_json::from_value(serde_json::json!({
            "id": id, "shape": {"Wave": "Sine"}, "attributes": ["Dimmer"], "members": members
        }))
        .unwrap()
    };
    let mut e = open();
    let mut mine = Cue::new(values(&[(206, level(1.0))]));
    mine.effects = vec![entry(1, &["206"])];
    e.add_playback(Playback {
        cues: vec![mine],
        ..Playback::new(70, 5, 1, Values::new())
    });
    let mut show = other();
    let pb = show
        .playbacks
        .playbacks
        .iter_mut()
        .find(|p| p.id == 1)
        .unwrap();
    // Entry 1 clashes with the open show's; entry 2 only has an unmapped member.
    pb.cues[0].effects = vec![entry(1, &["1201", "1999"]), entry(2, &["1999"])];
    let req = Request {
        playbacks: vec![slot(1, 1)],
        ..Request::default()
    };
    let plan = e.merge(&show, &req, false).unwrap();
    let merged = &at(&e, 1, 2).cues[0].effects;
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].members, vec![item(201)]);
    assert_ne!(merged[0].id, 1, "a new entry id, not the open show's");
    // 1999's value in the cue and in palette 3 (brought along), its two effect memberships.
    assert_eq!(plan.left_out.values, 4);
}
