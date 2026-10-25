//! Output nodes in the core (VS6-01): `specs/modules/output-protocols.md` OUT-13, OUT-25..28.
//!
//! API these tests fix (`cmdmx_core::output` and `Engine`):
//! - `output::NodeId = u32`; `output::Node { id, line: UniverseId, enabled, kind: NodeKind }`,
//!   saved flat (`kind` is a tag next to `id`, `line`, `enabled`, OUT-13).
//! - `output::NodeKind::{Artnet { port_address: u16, target: String }, Sacn { universe: u16,
//!   priority: u8, unicast: Option<String> }, Serial { port: String }}`; addresses stay strings
//!   so the core checks them (OUT-28). `NodeKind::default_artnet(line)` and
//!   `NodeKind::default_sacn(line)` are OUT-25's defaults.
//! - `Engine::nodes()`: the nodes sorted by id. `Engine::new` holds line 1's defaults plus serial
//!   `auto` (ids 1, 2, 3; OUT-25), so `show.new` and the Demo get them.
//! - `Engine::output_add(line, enabled, kind) -> Result<NodeId, _>`, `output_edit(Node)` (whole
//!   node by id, `kind` cannot change), `output_remove(&[NodeId])` (all ids checked first).
//!   Refusals are `CoreError`s and change nothing (OUT-28). Nodes are in `EditState` (undo).
//! - `Show::output { next_node_id, nodes }`, the last top-level key; schema 6.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

mod common;

use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::output::{Node, NodeKind};
use cmdmx_core::patch::PatchAdd;
use cmdmx_core::show::{Meta, Show, SCHEMA_VERSION};
use cmdmx_core::undo::History;
use cmdmx_core::CoreError;
use serde_json::{json, Value as Json};

const V5_SHOW: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/fixtures/shows/church-demo-v5.cmdmx.json"
);

fn meta() -> Meta {
    Meta {
        name: "Output".into(),
        created: "2026-10-22T09:00:00Z".into(),
        modified: "2026-10-22T09:00:00Z".into(),
        app_version: "0.1.0".into(),
    }
}

fn artnet(port_address: u16) -> NodeKind {
    NodeKind::Artnet {
        port_address,
        target: "255.255.255.255".into(),
    }
}

fn sacn(universe: u16) -> NodeKind {
    NodeKind::Sacn {
        universe,
        priority: 100,
        unicast: None,
    }
}

fn serial(port: &str) -> NodeKind {
    NodeKind::Serial { port: port.into() }
}

fn node(id: u32, line: u16, kind: NodeKind) -> Node {
    Node {
        id,
        line,
        enabled: true,
        kind,
    }
}

fn nodes(e: &Engine) -> Vec<Node> {
    e.nodes().cloned().collect()
}

fn dimmer_on(universe: u16) -> PatchAdd {
    PatchAdd {
        profile: "dimmer".into(),
        mode: None,
        count: 1,
        universe,
        address: Some(1),
        number: Some(900),
    }
}

fn warnings(e: &mut Engine) -> Vec<String> {
    e.drain_warnings().into_iter().map(|w| w.message).collect()
}

fn refusal(r: Result<impl std::fmt::Debug, CoreError>) -> String {
    r.unwrap_err().to_string()
}

// ---- OUT-25 ------------------------------------------------------------------

#[test]
fn out_25_defaults_of_a_line() {
    assert_eq!(NodeKind::default_artnet(1), artnet(0));
    assert_eq!(NodeKind::default_artnet(3), artnet(2));
    assert_eq!(NodeKind::default_sacn(3), sacn(3));
}

#[test]
fn out_25_a_new_engine_sends_line_1_on_art_net_sacn_and_serial_auto() {
    let e = Engine::new(Settings::default());
    assert_eq!(
        nodes(&e),
        [
            node(1, 1, artnet(0)),
            node(2, 1, sacn(1)),
            node(3, 1, serial("auto")),
        ]
    );
}

// ---- OUT-13 ------------------------------------------------------------------

#[test]
fn out_13_output_is_the_last_key_with_nodes_sorted_by_id() {
    let mut e = Engine::new(Settings::default());
    e.output_add(2, false, sacn(2)).unwrap();
    let text = e.to_show(meta()).to_json();
    let v: Json = serde_json::from_str(&text).unwrap();
    assert_eq!(SCHEMA_VERSION, 7);
    assert_eq!(v["schema_version"], json!(7));
    let last_key = text
        .lines()
        .rfind(|l| l.starts_with("  \"") && !l.starts_with("   "))
        .unwrap();
    assert!(last_key.starts_with("  \"output\""), "{last_key}");
    assert_eq!(
        v["output"],
        json!({
            "next_node_id": 5,
            "nodes": [
                {"id": 1, "line": 1, "enabled": true, "kind": "artnet",
                 "port_address": 0, "target": "255.255.255.255"},
                {"id": 2, "line": 1, "enabled": true, "kind": "sacn",
                 "universe": 1, "priority": 100, "unicast": null},
                {"id": 3, "line": 1, "enabled": true, "kind": "serial", "port": "auto"},
                {"id": 4, "line": 2, "enabled": false, "kind": "sacn",
                 "universe": 2, "priority": 100, "unicast": null}
            ]
        })
    );
}

#[test]
fn out_13_nodes_round_trip_and_ids_are_never_reused() {
    let mut e = Engine::new(Settings::default());
    let id = e
        .output_add(1, true, serial("/dev/cu.usbserial-A10KXYZ"))
        .unwrap();
    assert_eq!(id, 4);
    e.output_remove(&[id]).unwrap();
    let show = e.to_show(meta());
    let (show, migrations) = Show::from_json(&show.to_json()).unwrap();
    assert!(migrations.is_empty(), "{migrations:?}");
    let (mut loaded, warnings) = Engine::from_show(&show);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(nodes(&loaded), nodes(&e));
    assert_eq!(loaded.output_add(2, true, sacn(2)).unwrap(), 5);
}

#[test]
fn out_13_a_file_without_nodes_loads_without_nodes() {
    let mut v: Json =
        serde_json::from_str(&Engine::new(Settings::default()).to_show(meta()).to_json()).unwrap();
    v["output"]["nodes"] = json!([]);
    let (show, _) = Show::from_json(&v.to_string()).unwrap();
    let (e, warnings) = Engine::from_show(&show);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(nodes(&e).is_empty(), "the operator removed them all");
}

// ---- OUT-26 ------------------------------------------------------------------

#[test]
fn out_26_the_first_fixture_on_a_universe_adds_its_default_nodes() {
    let mut e = common::church();
    warnings(&mut e);
    e.patch_add(&dimmer_on(2)).unwrap();
    let added: Vec<Node> = nodes(&e).into_iter().filter(|n| n.line == 2).collect();
    assert_eq!(added, [node(4, 2, artnet(1)), node(5, 2, sacn(2))]);
    assert_eq!(
        warnings(&mut e),
        ["Universe 2 now goes out on Art-Net 0:0:1 and sACN 2"]
    );
}

#[test]
fn out_26_a_universe_with_fixtures_or_nodes_gets_none_added() {
    let mut e = common::church();
    let before = nodes(&e);
    e.patch_add(&dimmer_on(1)).unwrap_err(); // 1.1 is taken: refused, nothing added
    e.patch_add(&PatchAdd {
        address: None,
        ..dimmer_on(1)
    })
    .unwrap();
    assert_eq!(nodes(&e), before, "universe 1 already had fixtures");
    e.output_add(3, false, sacn(3)).unwrap();
    let before = nodes(&e);
    e.patch_add(&PatchAdd {
        number: Some(901),
        ..dimmer_on(3)
    })
    .unwrap();
    assert_eq!(
        nodes(&e),
        before,
        "universe 3 already had a node, even a disabled one"
    );
}

#[test]
fn out_26_removed_nodes_stay_removed_while_the_universe_has_fixtures() {
    let mut e = common::church();
    let ids = e.patch_add(&dimmer_on(2)).unwrap();
    let line_2: Vec<u32> = e.nodes().filter(|n| n.line == 2).map(|n| n.id).collect();
    e.output_remove(&line_2).unwrap();
    e.patch_add(&PatchAdd {
        address: Some(10),
        number: Some(901),
        ..dimmer_on(2)
    })
    .unwrap();
    assert!(e.nodes().all(|n| n.line != 2));
    e.patch_move(ids[0], 2, 20).unwrap();
    assert!(e.nodes().all(|n| n.line != 2));
}

#[test]
fn out_26_a_move_to_an_empty_universe_adds_its_nodes() {
    let mut e = common::church();
    let id = e.fixtures().next().unwrap().id;
    warnings(&mut e);
    e.patch_move(id, 4, 1).unwrap();
    let added: Vec<Node> = nodes(&e).into_iter().filter(|n| n.line == 4).collect();
    assert_eq!(added, [node(4, 4, artnet(3)), node(5, 4, sacn(4))]);
    assert_eq!(
        warnings(&mut e),
        ["Universe 4 now goes out on Art-Net 0:0:3 and sACN 4"]
    );
}

#[test]
fn out_26_a_default_an_enabled_node_already_sends_is_left_out() {
    let mut e = common::church();
    e.output_edit(node(1, 1, artnet(1))).unwrap(); // line 1's Art-Net now uses 0:0:1
    warnings(&mut e);
    e.patch_add(&dimmer_on(2)).unwrap();
    let added: Vec<Node> = nodes(&e).into_iter().filter(|n| n.line == 2).collect();
    assert_eq!(added, [node(4, 2, sacn(2))]);
    assert_eq!(warnings(&mut e), ["Universe 2 now goes out on sACN 2"]);
}

#[test]
fn out_26_undo_takes_the_patch_add_and_its_nodes_back_in_one_step() {
    let mut e = common::church();
    let mut history = History::default();
    let before = nodes(&e);
    history
        .edit(&mut e, "Patch", None, |e| e.patch_add(&dimmer_on(2)))
        .unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(nodes(&e).len(), before.len() + 2);
    history.undo(&mut e).unwrap();
    assert_eq!(nodes(&e), before);
    assert!(e.fixtures().all(|f| f.universe != 2));
    history.redo(&mut e).unwrap();
    assert_eq!(nodes(&e).len(), before.len() + 2);
}

// ---- OUT-27 ------------------------------------------------------------------

#[test]
fn out_27_a_schema_5_show_gets_default_nodes_for_every_patched_universe() {
    let file = std::fs::read_to_string(V5_SHOW).unwrap();
    let (show, migrations) = Show::from_json(&file).unwrap();
    assert_eq!(migrations.len(), 2, "{migrations:?}");
    assert!(migrations[0].starts_with("5 -> 6"), "{migrations:?}");
    assert!(migrations[1].starts_with("6 -> 7"), "{migrations:?}"); // PRG-52
    assert_eq!(show.schema_version, 7);
    let (e, warnings) = Engine::from_show(&show);
    assert!(warnings.is_empty(), "{warnings:?}");
    // The church rig patches universes 1 and 2.
    assert_eq!(
        nodes(&e),
        [
            node(1, 1, artnet(0)),
            node(2, 1, sacn(1)),
            node(3, 1, serial("auto")),
            node(4, 2, artnet(1)),
            node(5, 2, sacn(2)),
        ]
    );
    let v: Json = serde_json::from_str(&e.to_show(meta()).to_json()).unwrap();
    assert_eq!(v["output"]["next_node_id"], json!(6));
}

#[test]
fn out_27_line_1_is_always_included() {
    let mut v: Json = serde_json::from_str(&std::fs::read_to_string(V5_SHOW).unwrap()).unwrap();
    for f in v["patch"]["fixtures"].as_array_mut().unwrap() {
        f["universe"] = json!(3);
    }
    // Two fixtures cannot share 3.1; keep only one per address by dropping the patch to one.
    let first = v["patch"]["fixtures"][0].clone();
    v["patch"]["fixtures"] = json!([first]);
    let (show, _) = Show::from_json(&v.to_string()).unwrap();
    let (e, _) = Engine::from_show(&show);
    let lines: Vec<u16> = e.nodes().map(|n| n.line).collect();
    assert_eq!(lines, [1, 1, 1, 3, 3]);
}

// ---- OUT-28 ------------------------------------------------------------------

#[test]
fn out_28_bad_fields_are_refused_and_change_nothing() {
    let mut e = Engine::new(Settings::default());
    let before = nodes(&e);
    let bad = [
        (0, artnet(5)),
        (2, artnet(32768)),
        (2, sacn(0)),
        (2, sacn(64000)),
        (
            2,
            NodeKind::Sacn {
                universe: 2,
                priority: 201,
                unicast: None,
            },
        ),
        (
            2,
            NodeKind::Sacn {
                universe: 2,
                priority: 100,
                unicast: Some("10.0.0".into()),
            },
        ),
        (
            2,
            NodeKind::Artnet {
                port_address: 1,
                target: "broadcast".into(),
            },
        ),
    ];
    for (line, kind) in bad {
        assert!(
            e.output_add(line, true, kind.clone()).is_err(),
            "{line} {kind:?}"
        );
    }
    assert_eq!(nodes(&e), before);
    assert!(e
        .output_edit(Node {
            line: 0,
            ..node(2, 1, sacn(1))
        })
        .is_err());
    assert_eq!(
        refusal(e.output_edit(node(2, 1, sacn(64000)))),
        "sACN universe 64000 is out of range (1 to 63999)"
    );
    assert_eq!(nodes(&e), before);
}

#[test]
fn out_28_a_second_enabled_node_with_the_same_destination_is_refused() {
    let mut e = Engine::new(Settings::default());
    e.output_add(2, true, artnet(1)).unwrap();
    e.output_add(2, true, sacn(2)).unwrap();
    assert_eq!(
        refusal(e.output_add(5, true, artnet(1))),
        "Art-Net 0:0:1 at 255.255.255.255 already sends universe 2"
    );
    assert!(
        e.output_add(5, true, sacn(2)).is_err(),
        "multicast counts as one"
    );
    assert!(
        e.output_add(1, true, serial("auto")).is_err(),
        "auto at most once"
    );
    // Another target, a unicast address or another port is another destination.
    e.output_add(
        5,
        true,
        NodeKind::Artnet {
            port_address: 1,
            target: "10.0.0.5".into(),
        },
    )
    .unwrap();
    e.output_add(
        5,
        true,
        NodeKind::Sacn {
            universe: 2,
            priority: 100,
            unicast: Some("10.0.0.5".into()),
        },
    )
    .unwrap();
    e.output_add(1, true, serial("/dev/cu.usbserial-A"))
        .unwrap();
    assert!(e
        .output_add(2, true, serial("/dev/cu.usbserial-A"))
        .is_err());
    // A disabled node may share a destination, but enabling it is refused.
    let id = e.output_add(5, false, artnet(1)).unwrap();
    let mut n = e.nodes().find(|n| n.id == id).unwrap().clone();
    n.enabled = true;
    assert!(e.output_edit(n).is_err());
    // A node never clashes with itself.
    e.output_edit(node(1, 1, artnet(0))).unwrap();
}

#[test]
fn out_28_edit_and_remove_of_an_unknown_node_or_a_kind_change_are_refused() {
    let mut e = Engine::new(Settings::default());
    let before = nodes(&e);
    assert!(e.output_edit(node(9, 1, sacn(1))).is_err());
    assert!(
        e.output_edit(node(1, 1, sacn(1))).is_err(),
        "kind cannot change"
    );
    assert!(
        e.output_remove(&[1, 9]).is_err(),
        "all ids are checked first"
    );
    assert_eq!(nodes(&e), before);
}

#[test]
fn out_28_node_edits_are_undoable() {
    let mut e = Engine::new(Settings::default());
    let mut history = History::default();
    let before = nodes(&e);
    history
        .edit(&mut e, "Add output", None, |e| {
            e.output_add(2, true, sacn(2))
        })
        .unwrap();
    history
        .edit(&mut e, "Edit output", None, |e| {
            e.output_edit(Node {
                enabled: false,
                ..node(1, 1, artnet(0))
            })
        })
        .unwrap();
    history
        .edit(&mut e, "Remove output", None, |e| e.output_remove(&[3]))
        .unwrap();
    assert_eq!(history.len(), 3);
    for _ in 0..3 {
        history.undo(&mut e).unwrap();
    }
    assert_eq!(nodes(&e), before);
    // A refused edit adds no step.
    assert!(history
        .edit(&mut e, "Add output", None, |e| e.output_add(
            0,
            true,
            sacn(2)
        ))
        .is_err());
    assert_eq!(history.len(), 0);
}

#[test]
fn out_28_bad_nodes_are_left_out_on_load_with_one_warning_each() {
    let mut v: Json =
        serde_json::from_str(&Engine::new(Settings::default()).to_show(meta()).to_json()).unwrap();
    let nodes_json = v["output"]["nodes"].as_array_mut().unwrap();
    nodes_json.push(json!({"id": 4, "line": 2, "enabled": true, "kind": "sacn",
                           "universe": 70000, "priority": 100, "unicast": null}));
    nodes_json.push(
        json!({"id": 5, "line": 2, "enabled": true, "kind": "artnet",
                           "port_address": 0, "target": "255.255.255.255"}),
    );
    nodes_json
        .push(json!({"id": 6, "line": 2, "enabled": true, "kind": "dmx-over-carrier-pigeon"}));
    nodes_json
        .push(json!({"id": 7, "line": 0, "enabled": true, "kind": "serial", "port": "/dev/cu.x"}));
    nodes_json
        .push(json!({"id": 3, "line": 2, "enabled": false, "kind": "serial", "port": "auto"}));
    let (show, _) = Show::from_json(&v.to_string()).unwrap();
    let (e, warnings) = Engine::from_show(&show);
    assert_eq!(warnings.len(), 5, "{warnings:?}");
    assert!(
        warnings[1].contains("already sends universe 1"),
        "id 5 clashes with node 1: {warnings:?}"
    );
    assert_eq!(e.nodes().map(|n| n.id).collect::<Vec<_>>(), [1, 2, 3]);
}
