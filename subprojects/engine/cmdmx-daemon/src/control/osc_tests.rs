//! VS7-03: the daemon side of OSC (CTL-14, 15, 20, 21, 35..40). Packets are simulated with
//! `OscIn`s; the loopback tests at the end run the real socket thread on 127.0.0.1.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use std::fs;
use std::net::{SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

use cmdmx_control::osc::{self, OscArg, OscMessage};
use cmdmx_core::engine::Engine;
use cmdmx_core::playback::{Cue, CueNumber, HandleButton, ListAction, Playback, Values};
use cmdmx_core::undo::History;
use serde_json::{json, Value};

use super::Control;
use crate::ipc::{self, Command};
use crate::osc_io::{OscCmd, OscIn};

fn tmp(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cmdmx-osc-{test}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("control")).unwrap();
    dir
}

fn load(dir: &std::path::Path) -> Control {
    let (c, warnings) = Control::load(dir.join("control"), dir.join("state.json"));
    assert!(warnings.is_empty(), "{warnings:?}");
    c
}

fn req(c: &mut Control, j: Value) -> Result<(), String> {
    match ipc::parse_command(&j.to_string())? {
        Command::Control(r) => c.request(r),
        other => panic!("not a control command: {other:?}"),
    }
}

/// A control layer in a fresh folder with OSC on.
fn on(test: &str) -> (Control, PathBuf) {
    let dir = tmp(test);
    let mut c = load(&dir);
    req(&mut c, json!({"v":0,"cmd":"control.osc","enabled":true})).unwrap();
    (c, dir)
}

fn addr(s: &str) -> SocketAddr {
    s.parse().unwrap()
}

fn msg(address: &str, args: Vec<OscArg>) -> OscMessage {
    OscMessage {
        address: address.into(),
        args,
    }
}

fn send(c: &mut Control, from: &str, address: &str, args: Vec<OscArg>, now: f64) {
    let bytes = osc::encode(&msg(address, args));
    c.osc_event(
        OscIn::Packet {
            from: addr(from),
            bytes,
        },
        now,
    );
}

fn engine() -> Engine {
    crate::demo_engine().unwrap()
}

fn slot(e: &Engine, page: u16, slot: u16) -> u32 {
    e.playbacks()
        .find(|p| (p.page, p.slot) == (page, slot))
        .unwrap()
        .id
}

fn level(e: &Engine, id: u32) -> f32 {
    e.playback_state(id).map_or(0.0, |s| s.level)
}

fn run_all(e: &mut Engine, cmds: Vec<(String, Command)>) -> Vec<String> {
    let mut h = History::default();
    cmds.into_iter()
        .filter_map(|(dev, cmd)| crate::run_control(e, &mut h, &dev, cmd, 0.0).err())
        .collect()
}

fn has(out: &[(SocketAddr, OscMessage)], to: &str, address: &str, arg: OscArg) -> bool {
    out.iter()
        .any(|(t, m)| *t == addr(to) && m.address == address && m.args == [arg.clone()])
}

fn to<'a>(out: &'a [(SocketAddr, OscMessage)], to: &str) -> Vec<&'a OscMessage> {
    out.iter()
        .filter(|(t, _)| *t == addr(to))
        .map(|(_, m)| m)
        .collect()
}

fn osc_section(c: &Control) -> Value {
    c.snapshot()["osc"].clone()
}

// --- CTL-35 settings ---------------------------------------------------------

#[test]
fn ctl_35_osc_is_off_by_default_and_ignores_packets() {
    let dir = tmp("off");
    let mut c = load(&dir);
    let s = osc_section(&c);
    assert_eq!(s["enabled"], false);
    assert_eq!(s["port"], 8000);
    assert_eq!(s["feedback_port"], 9000);
    assert_eq!(s["allow"], json!([]));
    assert_eq!(c.osc_listen(), None);
    let e = engine();
    send(&mut c, "192.168.1.20:5000", "/cmdmx/go", vec![], 0.0);
    assert!(c.commands(&e, 0.0).is_empty());
    assert!(c.osc_feedback(&e, 0.0).is_empty());
}

#[test]
fn ctl_35_settings_are_kept_in_state_json_beside_the_devices() {
    let dir = tmp("persist");
    fs::write(
        dir.join("state.json"),
        r#"{"last_show":"Sunday","control":{"devices":{"nano":{"enabled":true,"mapping":null}}}}"#,
    )
    .unwrap();
    let mut c = load(&dir);
    req(
        &mut c,
        json!({"v":0,"cmd":"control.osc","enabled":true,"port":8001,"feedback_port":9001,
               "allow":["192.168.1.0/24","10.0.0.5"],"clients":["10.0.0.9:9100"]}),
    )
    .unwrap();
    assert_eq!(c.osc_listen(), Some(8001));
    let state: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("state.json")).unwrap()).unwrap();
    assert_eq!(state["last_show"], "Sunday");
    assert_eq!(state["control"]["devices"]["nano"]["enabled"], true);
    assert_eq!(state["control"]["osc"]["port"], 8001);
    // A restart reads them back.
    let c = load(&dir);
    let s = osc_section(&c);
    assert_eq!(s["enabled"], true);
    assert_eq!(s["port"], 8001);
    assert_eq!(s["feedback_port"], 9001);
    assert_eq!(s["allow"], json!(["192.168.1.0/24", "10.0.0.5"]));
    assert_eq!(c.osc_listen(), Some(8001));
    let clients = s["clients"].as_array().unwrap();
    assert!(clients
        .iter()
        .any(|k| k["address"] == "10.0.0.9:9100" && k["configured"] == true));
    // A partial command changes only what it names.
    let mut c = c;
    req(&mut c, json!({"v":0,"cmd":"control.osc","enabled":false})).unwrap();
    assert_eq!(c.osc_listen(), None);
    assert_eq!(osc_section(&c)["port"], 8001);
}

#[test]
fn ctl_35_bad_settings_are_refused_and_change_nothing() {
    let (mut c, _) = on("bad");
    for bad in [
        json!({"v":0,"cmd":"control.osc","port":0}),
        json!({"v":0,"cmd":"control.osc","port":70000}),
        json!({"v":0,"cmd":"control.osc","feedback_port":0}),
        json!({"v":0,"cmd":"control.osc","allow":["300.1.1.1"]}),
        json!({"v":0,"cmd":"control.osc","allow":["10.0.0.0/33"]}),
        json!({"v":0,"cmd":"control.osc","clients":["ipad"]}),
        json!({"v":0,"cmd":"control.osc","enabled":"yes"}),
    ] {
        assert!(req(&mut c, bad.clone()).is_err(), "{bad}");
    }
    let s = osc_section(&c);
    assert_eq!(
        (s["port"].clone(), s["feedback_port"].clone()),
        (json!(8000), json!(9000))
    );
    assert_eq!(s["allow"], json!([]));
}

#[test]
fn ctl_35_senders_outside_allow_are_ignored_and_counted() {
    let (mut c, _) = on("allow");
    req(
        &mut c,
        json!({"v":0,"cmd":"control.osc","allow":["192.168.1.0/24"]}),
    )
    .unwrap();
    let e = engine();
    send(&mut c, "10.0.0.1:5000", "/cmdmx/go", vec![], 0.0);
    assert!(c.commands(&e, 0.0).is_empty());
    assert_eq!(osc_section(&c)["errors"], 1);
    assert!(c.osc_feedback(&e, 0.0).is_empty(), "not a client either");
    send(&mut c, "192.168.1.7:5000", "/cmdmx/go", vec![], 0.0);
    assert_eq!(c.commands(&e, 0.0).len(), 1);
}

// --- CTL-36 the /cmdmx schema --------------------------------------------------

#[test]
fn ctl_36_a_fader_address_sets_the_slot_level_without_pickup() {
    let (mut c, _) = on("fader");
    let mut e = engine();
    send(
        &mut c,
        "192.168.1.20:5000",
        "/cmdmx/fader/1",
        vec![OscArg::Float(0.7)],
        0.0,
    );
    let cmds = c.commands(&e, 0.0);
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].0, "OSC", "refusals name OSC (CTL-06)");
    assert!(run_all(&mut e, cmds).is_empty());
    e.tick(0.025);
    assert!((level(&e, slot(&e, 1, 1)) - 0.7).abs() < 0.001);
}

#[test]
fn ctl_36_buttons_press_release_and_tap() {
    let (mut c, _) = on("buttons");
    let e = engine();
    let id = slot(&e, 1, 1);
    let from = "192.168.1.20:5000";
    send(
        &mut c,
        from,
        "/cmdmx/fader/1/upper",
        vec![OscArg::Float(1.0)],
        0.0,
    );
    send(
        &mut c,
        from,
        "/cmdmx/fader/1/upper",
        vec![OscArg::Float(0.0)],
        0.0,
    );
    send(&mut c, from, "/cmdmx/go", vec![], 0.0);
    let cmds: Vec<Command> = c.commands(&e, 0.0).into_iter().map(|(_, c)| c).collect();
    assert_eq!(
        cmds,
        [
            Command::PlaybackButton {
                id,
                button: HandleButton::Upper,
                on: true,
                at: 0.0
            },
            Command::PlaybackButton {
                id,
                button: HandleButton::Upper,
                on: false,
                at: 0.0
            },
            Command::CuelistAction {
                id: None,
                action: ListAction::Go
            },
        ]
    );
}

#[test]
fn ctl_15_36_malformed_packets_and_unknown_cmdmx_addresses_are_counted() {
    let (mut c, _) = on("malformed");
    let e = engine();
    c.osc_event(
        OscIn::Packet {
            from: addr("192.168.1.20:5000"),
            bytes: vec![0x2f, 0x61, 0x62],
        },
        0.0,
    );
    send(&mut c, "192.168.1.20:5000", "/cmdmx/nope", vec![], 0.0);
    assert_eq!(osc_section(&c)["errors"], 2);
    // An address outside /cmdmx that nothing binds is ignored, not an error.
    send(
        &mut c,
        "192.168.1.20:5000",
        "/1/fader1",
        vec![OscArg::Float(0.5)],
        0.0,
    );
    assert_eq!(osc_section(&c)["errors"], 2);
    assert!(c.commands(&e, 0.0).is_empty());
}

#[test]
fn ctl_14_osc_levels_coalesce_per_tick_buttons_never() {
    let (mut c, _) = on("coalesce");
    let e = engine();
    let from = "192.168.1.20:5000";
    for v in [0.1, 0.2, 0.3] {
        send(&mut c, from, "/cmdmx/fader/1", vec![OscArg::Float(v)], 0.0);
    }
    for v in [1.0, 0.0, 1.0] {
        send(
            &mut c,
            from,
            "/cmdmx/fader/2/upper",
            vec![OscArg::Float(v)],
            0.0,
        );
    }
    let cmds: Vec<Command> = c.commands(&e, 0.0).into_iter().map(|(_, c)| c).collect();
    let faders: Vec<_> = cmds
        .iter()
        .filter_map(|c| match c {
            Command::FaderSet { level, .. } => Some(*level),
            _ => None,
        })
        .collect();
    assert_eq!(faders, [0.3]);
    let buttons = cmds
        .iter()
        .filter(|c| matches!(c, Command::PlaybackButton { .. }))
        .count();
    assert_eq!(buttons, 3);
}

// --- CTL-37 feedback -------------------------------------------------------------

const A: &str = "192.168.1.20:5000";
const A_FB: &str = "192.168.1.20:9000";
const B: &str = "192.168.1.21:6000";
const B_FB: &str = "192.168.1.21:9000";

#[test]
fn ctl_37_a_sender_gets_the_full_state_on_its_feedback_port_then_only_changes() {
    let (mut c, _) = on("full");
    let mut e = engine();
    e.set_fader(slot(&e, 1, 1), 0.5, 0.0).unwrap();
    e.tick(0.0);
    send(&mut c, A, "/cmdmx/page", vec![OscArg::Int(1)], 0.0);
    let _ = c.commands(&e, 0.0);
    let out = c.osc_feedback(&e, 0.0);
    assert!(to(&out, A).is_empty(), "feedback goes to feedback_port");
    for (address, arg) in [
        ("/cmdmx/fader/1", OscArg::Float(0.5)),
        ("/cmdmx/fader/2", OscArg::Float(0.0)),
        ("/cmdmx/fader/1/legend", OscArg::Str(String::new())),
        ("/cmdmx/fader/1/upper", OscArg::Float(0.0)),
        ("/cmdmx/fader/1/lower", OscArg::Float(0.0)),
        ("/cmdmx/page", OscArg::Int(1)),
        ("/cmdmx/page/legend", OscArg::Str(String::new())),
        ("/cmdmx/gm", OscArg::Float(1.0)),
        ("/cmdmx/dbo", OscArg::Float(0.0)),
        ("/cmdmx/cue/current", OscArg::Str(String::new())),
        ("/cmdmx/cue/next", OscArg::Str(String::new())),
        ("/cmdmx/message", OscArg::Str(String::new())),
    ] {
        assert!(
            has(&out, A_FB, address, arg.clone()),
            "{address} {arg:?}: {out:?}"
        );
    }
    assert!(c.osc_feedback(&e, 0.025).is_empty(), "nothing changed");
    e.set_fader(slot(&e, 1, 2), 0.25, 0.05).unwrap();
    e.tick(0.05);
    let out = c.osc_feedback(&e, 0.05);
    assert_eq!(
        out,
        [(addr(A_FB), msg("/cmdmx/fader/2", vec![OscArg::Float(0.25)]))]
    );
}

#[test]
fn ctl_37_configured_clients_get_feedback_without_being_heard() {
    let (mut c, _) = on("configured");
    req(
        &mut c,
        json!({"v":0,"cmd":"control.osc","clients":["10.0.0.9:9100"]}),
    )
    .unwrap();
    let out = c.osc_feedback(&engine(), 0.0);
    assert!(
        has(&out, "10.0.0.9:9100", "/cmdmx/page", OscArg::Int(1)),
        "{out:?}"
    );
}

#[test]
fn ctl_37_a_new_client_gets_the_full_state_the_others_only_changes() {
    let (mut c, _) = on("new-client");
    let e = engine();
    send(&mut c, A, "/cmdmx/page", vec![OscArg::Int(1)], 0.0);
    let _ = c.commands(&e, 0.0);
    assert!(!c.osc_feedback(&e, 0.0).is_empty());
    send(&mut c, B, "/cmdmx/page", vec![OscArg::Int(1)], 1.0);
    let _ = c.commands(&e, 1.0);
    let out = c.osc_feedback(&e, 1.0);
    assert!(to(&out, A_FB).is_empty(), "{out:?}");
    assert!(has(&out, B_FB, "/cmdmx/page", OscArg::Int(1)), "{out:?}");
    let clients = osc_section(&c)["clients"].as_array().unwrap().len();
    assert_eq!(clients, 2);
}

#[test]
fn ctl_37_a_sender_not_heard_for_10_minutes_gets_nothing_until_it_is_heard_again() {
    let (mut c, _) = on("expire");
    let mut e = engine();
    send(&mut c, A, "/cmdmx/page", vec![OscArg::Int(1)], 0.0);
    let _ = c.commands(&e, 0.0);
    let _ = c.osc_feedback(&e, 0.0);
    e.set_fader(slot(&e, 1, 1), 0.5, 0.0).unwrap();
    e.tick(599.0);
    assert!(!c.osc_feedback(&e, 599.0).is_empty(), "still heard");
    e.set_fader(slot(&e, 1, 1), 0.6, 0.0).unwrap();
    e.tick(601.0);
    assert!(
        c.osc_feedback(&e, 601.0).is_empty(),
        "gone after 10 minutes"
    );
    assert_eq!(osc_section(&c)["clients"], json!([]));
    send(&mut c, A, "/cmdmx/page", vec![OscArg::Int(1)], 700.0);
    let _ = c.commands(&e, 700.0);
    let out = c.osc_feedback(&e, 700.0);
    assert!(
        has(&out, A_FB, "/cmdmx/page", OscArg::Int(1)),
        "full state again: {out:?}"
    );
}

#[test]
fn ctl_20_no_echo_to_the_sender_but_the_other_clients_follow() {
    let (mut c, _) = on("echo");
    let mut e = engine();
    send(&mut c, A, "/cmdmx/page", vec![OscArg::Int(1)], 0.0);
    send(&mut c, B, "/cmdmx/page", vec![OscArg::Int(1)], 0.0);
    let _ = c.commands(&e, 0.0);
    let _ = c.osc_feedback(&e, 0.0);
    send(
        &mut c,
        A,
        "/cmdmx/fader/1",
        vec![OscArg::Float(0.75)],
        0.025,
    );
    let cmds = c.commands(&e, 0.025);
    assert!(run_all(&mut e, cmds).is_empty());
    e.tick(0.025);
    let out = c.osc_feedback(&e, 0.025);
    assert!(
        has(&out, B_FB, "/cmdmx/fader/1", OscArg::Float(0.75)),
        "{out:?}"
    );
    assert!(
        !to(&out, A_FB).iter().any(|m| m.address == "/cmdmx/fader/1"),
        "{out:?}"
    );
}

#[test]
fn ctl_21_a_page_change_sends_the_new_page_and_its_levels() {
    let (mut c, _) = on("page");
    let mut e = engine();
    e.set_fader(slot(&e, 1, 1), 1.0, 0.0).unwrap();
    e.tick(0.0);
    send(&mut c, A, "/cmdmx/page", vec![OscArg::Int(1)], 0.0);
    let _ = c.commands(&e, 0.0);
    let _ = c.osc_feedback(&e, 0.0);
    e.set_page(2).unwrap();
    e.tick(0.025);
    let out = c.osc_feedback(&e, 0.025);
    assert!(has(&out, A_FB, "/cmdmx/page", OscArg::Int(2)), "{out:?}");
    assert!(
        has(&out, A_FB, "/cmdmx/fader/1", OscArg::Float(0.0)),
        "{out:?}"
    );
}

#[test]
fn ctl_37_cue_texts_and_the_last_warning() {
    let (mut c, _) = on("cues");
    let mut e = engine();
    let cue = |n: f64, legend: &str| Cue {
        number: CueNumber::new(n).unwrap(),
        legend: legend.into(),
        ..Cue::new(Values::new())
    };
    e.add_playback(Playback {
        cues: vec![cue(1.0, "Welcome"), cue(2.0, "")],
        ..Playback::new(50, 3, 1, Values::new())
    });
    e.cuelist(Some(50), ListAction::Go, 0.0).unwrap();
    e.tick(0.0);
    send(&mut c, A, "/cmdmx/page", vec![OscArg::Int(1)], 0.0);
    let _ = c.commands(&e, 0.0);
    c.note_warning("page 11 does not exist");
    let out = c.osc_feedback(&e, 0.0);
    assert!(
        has(
            &out,
            A_FB,
            "/cmdmx/cue/current",
            OscArg::Str("1 Welcome".into())
        ),
        "{out:?}"
    );
    assert!(
        has(&out, A_FB, "/cmdmx/cue/next", OscArg::Str("2".into())),
        "{out:?}"
    );
    assert!(
        has(
            &out,
            A_FB,
            "/cmdmx/message",
            OscArg::Str("page 11 does not exist".into())
        ),
        "{out:?}"
    );
}

#[test]
fn ctl_35_a_port_that_cannot_be_opened_is_shown_and_said_once() {
    let (mut c, _) = on("in-use");
    c.osc_event(OscIn::Listening(Err("address in use".into())), 0.0);
    let s = osc_section(&c);
    assert!(
        s["error"].as_str().unwrap().contains("address in use"),
        "{s}"
    );
    assert_eq!(c.take_messages().len(), 1);
    c.osc_event(OscIn::Listening(Ok(8000)), 1.0);
    assert_eq!(osc_section(&c)["error"], Value::Null);
}

// --- CTL-38 custom addresses -------------------------------------------------------

#[test]
fn ctl_38_a_custom_address_acts_and_gets_feedback_at_that_address() {
    let (mut c, dir) = on("custom");
    req(
        &mut c,
        json!({"v":0,"cmd":"control.mapping_new","name":"OSC","device":"OSC"}),
    )
    .unwrap();
    for (input, action) in [("/1/fader1", "fader 1"), ("/2/push3", "go")] {
        req(
            &mut c,
            json!({"v":0,"cmd":"control.binding","mapping":"OSC","input":input,"action":action}),
        )
        .unwrap();
    }
    assert!(dir.join("control/OSC.json").exists());
    let mut e = engine();
    send(&mut c, A, "/1/fader1", vec![OscArg::Float(0.4)], 0.0);
    send(&mut c, A, "/2/push3", vec![OscArg::Float(1.0)], 0.0);
    // The built-in schema keeps working next to it.
    send(&mut c, A, "/cmdmx/fader/2", vec![OscArg::Float(0.2)], 0.0);
    let cmds = c.commands(&e, 0.0);
    assert!(cmds.iter().any(|(_, c)| matches!(
        c,
        Command::CuelistAction {
            action: ListAction::Go,
            ..
        }
    )));
    // The demo show has no cue list, so `go` is refused, by name (CTL-06); the faders act.
    assert_eq!(run_all(&mut e, cmds), ["OSC: No cue list connected"]);
    e.tick(0.025);
    assert!((level(&e, slot(&e, 1, 1)) - 0.4).abs() < 0.001);
    assert!((level(&e, slot(&e, 1, 2)) - 0.2).abs() < 0.001);
    // B (not the sender) sees the custom address follow the engine.
    send(&mut c, B, "/cmdmx/page", vec![OscArg::Int(1)], 0.025);
    let _ = c.commands(&e, 0.025);
    let out = c.osc_feedback(&e, 0.025);
    assert!(has(&out, B_FB, "/1/fader1", OscArg::Float(0.4)), "{out:?}");
    assert!(has(&out, B_FB, "/2/push3", OscArg::Float(0.0)), "{out:?}");
}

#[test]
fn ctl_38_learn_binds_the_next_address_into_the_osc_mapping() {
    let (mut c, dir) = on("learn");
    let mut e = engine();
    req(
        &mut c,
        json!({"v":0,"cmd":"control.learn","action":"fader 2"}),
    )
    .unwrap();
    let _ = c.take_messages();
    // A /cmdmx address still acts and does not learn.
    send(&mut c, A, "/cmdmx/gm", vec![OscArg::Float(0.5)], 0.0);
    assert_eq!(c.commands(&e, 0.0).len(), 1);
    assert_eq!(c.snapshot()["learn"], "fader 2");
    // The next other address learns and does not act.
    send(&mut c, A, "/1/fader2", vec![OscArg::Float(0.3)], 0.0);
    assert!(c.commands(&e, 0.0).is_empty());
    assert_eq!(c.snapshot()["learn"], Value::Null);
    assert_eq!(c.take_messages(), ["Learned: OSC /1/fader2 → fader 2"]);
    let file = fs::read_to_string(dir.join("control/OSC.json")).unwrap();
    assert!(file.contains("/1/fader2"), "{file}");
    assert_eq!(osc_section(&c)["mapping"], "OSC");
    // From now on it acts.
    send(&mut c, A, "/1/fader2", vec![OscArg::Float(0.6)], 0.025);
    let cmds = c.commands(&e, 0.025);
    assert!(run_all(&mut e, cmds).is_empty());
    e.tick(0.05);
    assert!((level(&e, slot(&e, 1, 2)) - 0.6).abs() < 0.001);
}

// --- CTL-39, 40 IPC ---------------------------------------------------------------

#[test]
fn ctl_39_the_snapshot_has_the_osc_section() {
    let (mut c, _) = on("snapshot");
    send(&mut c, A, "/cmdmx/page", vec![OscArg::Int(1)], 3.0);
    let s = osc_section(&c);
    for key in [
        "enabled",
        "port",
        "feedback_port",
        "allow",
        "clients",
        "mapping",
        "errors",
        "error",
    ] {
        assert!(s.get(key).is_some(), "{key} in {s}");
    }
    let clients = s["clients"].as_array().unwrap();
    assert_eq!(clients.len(), 1, "{s}");
    assert_eq!(clients[0]["address"], A_FB);
    assert_eq!(clients[0]["configured"], false);
    assert!(clients[0]["last_heard"].is_number());
    // A new client is a change: a control delta follows.
    let _ = c.delta(3.0);
    send(&mut c, B, "/cmdmx/page", vec![OscArg::Int(1)], 4.0);
    assert!(c.delta(4.0).is_some());
}

#[test]
fn ctl_40_control_osc_parses_and_is_class_neutral() {
    let cmd = ipc::parse_command(
        r#"{"v":0,"cmd":"control.osc","enabled":true,"port":8000,"clients":[]}"#,
    )
    .unwrap();
    assert_eq!(crate::class(&cmd), crate::Class::Neutral);
}

// --- loopback socket (CTL-14, 36, 37) ------------------------------------------------

/// The next event from the socket thread, at most `secs` seconds.
fn next(rx: &Receiver<OscIn>, secs: f64) -> Option<OscIn> {
    rx.recv_timeout(Duration::from_secs_f64(secs)).ok()
}

#[test]
fn ctl_36_37_loopback_socket_in_and_feedback_out() {
    let (tx, rx) = channel();
    let cmds = crate::osc_io::spawn(tx).unwrap();
    cmds.send(OscCmd::Listen(Some(0))).unwrap();
    let port = match next(&rx, 2.0) {
        Some(OscIn::Listening(Ok(p))) => p,
        other => panic!("not listening: {other:?}"),
    };
    assert_ne!(port, 0, "the port actually bound");
    let client = UdpSocket::bind("127.0.0.1:0").unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let client_port = client.local_addr().unwrap().port();

    let dir = tmp("loopback");
    let mut c = load(&dir);
    req(
        &mut c,
        json!({"v":0,"cmd":"control.osc","enabled":true,"feedback_port":client_port}),
    )
    .unwrap();
    let packet = osc::encode(&msg("/cmdmx/fader/1", vec![OscArg::Float(0.5)]));
    client.send_to(&packet, ("127.0.0.1", port)).unwrap();
    let start = Instant::now();
    let mut e = engine();
    let mut acted = Vec::new();
    while acted.is_empty() && start.elapsed() < Duration::from_secs(2) {
        if let Some(ev) = next(&rx, 0.1) {
            c.osc_event(ev, 0.0);
        }
        acted = c.commands(&e, 0.0);
    }
    assert_eq!(acted.len(), 1, "the packet arrived and acted");
    assert!(run_all(&mut e, acted).is_empty());
    e.set_fader(slot(&e, 1, 2), 0.25, 0.0).unwrap();
    e.tick(0.025);
    for (to, m) in c.osc_feedback(&e, 0.025) {
        cmds.send(OscCmd::Send {
            to,
            bytes: osc::encode(&m),
        })
        .unwrap();
    }
    let mut got = Vec::new();
    let mut buf = [0u8; 1024];
    while let Ok((n, _)) = client.recv_from(&mut buf) {
        got.extend(osc::decode(&buf[..n]).unwrap());
        if got.iter().any(|m| m.address == "/cmdmx/fader/2") {
            break;
        }
    }
    assert!(
        got.contains(&msg("/cmdmx/fader/2", vec![OscArg::Float(0.25)])),
        "{got:?}"
    );
    assert!(
        !got.iter().any(|m| m.address == "/cmdmx/fader/1"),
        "no echo (CTL-20): {got:?}"
    );
}

#[test]
fn ctl_35_loopback_a_taken_port_is_an_error_not_a_crash() {
    let taken = UdpSocket::bind("0.0.0.0:0").unwrap();
    let port = taken.local_addr().unwrap().port();
    let (tx, rx) = channel();
    let cmds = crate::osc_io::spawn(tx).unwrap();
    cmds.send(OscCmd::Listen(Some(port))).unwrap();
    assert!(matches!(next(&rx, 2.0), Some(OscIn::Listening(Err(_)))));
    // It still listens on another port afterwards.
    cmds.send(OscCmd::Listen(Some(0))).unwrap();
    assert!(matches!(next(&rx, 2.0), Some(OscIn::Listening(Ok(_)))));
}

#[test]
fn ctl_21_a_mapping_change_sends_every_osc_client_the_full_state() {
    let (mut c, _) = on("refresh");
    let e = engine();
    send(&mut c, A, "/cmdmx/page", vec![OscArg::Int(1)], 0.0);
    let _ = c.commands(&e, 0.0);
    let _ = c.osc_feedback(&e, 0.0);
    req(
        &mut c,
        json!({"v":0,"cmd":"control.mapping_new","name":"OSC","device":"OSC"}),
    )
    .unwrap();
    let _ = c.osc_feedback(&e, 0.0125);
    req(
        &mut c,
        json!({"v":0,"cmd":"control.binding","mapping":"OSC","input":"/1/push1","action":"go"}),
    )
    .unwrap();
    let out = c.osc_feedback(&e, 0.025);
    assert!(has(&out, A_FB, "/cmdmx/page", OscArg::Int(1)), "{out:?}");
}
