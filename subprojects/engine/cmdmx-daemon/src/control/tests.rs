//! VS7-02: the daemon side of the generic MIDI control layer (CTL-06, 08, 14..16, 21, 23..25, 28,
//! 29, 39, 40). Ports are simulated with `MidiEvent`s; the macOS tests at the end use a virtual
//! CoreMIDI port through the real MIDI thread.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use cmdmx_core::engine::Engine;
use cmdmx_core::undo::History;
use serde_json::{json, Value};

use super::{Control, MidiEvent, MidiOut, PortInfo};
use crate::ipc::{self, Command};

const NANO: &str = r#"{"name":"nano","device":"nano*","bindings":[
  {"input":{"kind":"cc","channel":1,"number":7},"action":"fader 1","pickup":false},
  {"input":{"kind":"note","channel":1,"number":60},"action":"upper 1"},
  {"input":{"kind":"note","channel":1,"number":61},"action":"page 2"},
  {"input":{"kind":"note","channel":1,"number":62},"action":"group 99"},
  {"input":{"kind":"note","channel":1,"number":70},"action":"keypad Group"},
  {"input":{"kind":"note","channel":1,"number":71},"action":"keypad 1"},
  {"input":{"kind":"note","channel":1,"number":72},"action":"keypad Enter"}
]}"#;

fn tmp(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cmdmx-control-{test}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("control")).unwrap();
    dir
}

/// A control layer in a fresh folder with the `nano` mapping file.
fn setup(test: &str) -> (Control, PathBuf) {
    let dir = tmp(test);
    fs::write(dir.join("control/nano.json"), NANO).unwrap();
    let (c, warnings) = Control::load(dir.join("control"), dir.join("state.json"));
    assert!(warnings.is_empty(), "{warnings:?}");
    (c, dir)
}

fn port(name: &str, output: bool) -> PortInfo {
    PortInfo {
        name: name.into(),
        input: true,
        output,
    }
}

fn ports(c: &mut Control, list: &[PortInfo]) {
    c.event(MidiEvent::Ports(list.to_vec()), 0.0);
}

fn packet(c: &mut Control, device: &str, bytes: &[u8]) {
    c.event(
        MidiEvent::Packet {
            device: device.into(),
            bytes: bytes.to_vec(),
        },
        0.0,
    );
}

fn req(c: &mut Control, json: &str) -> Result<(), String> {
    match ipc::parse_command(json)? {
        Command::Control(r) => c.request(r),
        other => panic!("not a control command: {other:?}"),
    }
}

/// `nano` connected with an output and enabled; it gets the `nano` mapping (CTL-24).
fn enabled(test: &str) -> (Control, PathBuf) {
    let (mut c, dir) = setup(test);
    ports(&mut c, &[port("nano", true)]);
    req(
        &mut c,
        r#"{"v":1,"cmd":"control.enable","device":"nano","on":true}"#,
    )
    .unwrap();
    (c, dir)
}

fn device<'a>(snapshot: &'a Value, name: &str) -> &'a Value {
    snapshot["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["name"] == name)
        .unwrap_or_else(|| panic!("no device {name} in {snapshot}"))
}

fn engine() -> Engine {
    crate::demo_engine().unwrap()
}

fn slot(engine: &Engine, page: u16, slot: u16) -> u32 {
    engine
        .playbacks()
        .find(|p| (p.page, p.slot) == (page, slot))
        .unwrap()
        .id
}

fn level(engine: &Engine, id: u32) -> f32 {
    engine.playback_state(id).map_or(0.0, |s| s.level)
}

/// Run the commands as the tick does: through the history, refusals named by device (CTL-06).
fn run_all(
    engine: &mut Engine,
    history: &mut History,
    cmds: Vec<(String, Command)>,
) -> Vec<String> {
    cmds.into_iter()
        .filter_map(|(dev, cmd)| crate::run_control(engine, history, &dev, cmd, 0.0).err())
        .collect()
}

fn cc7(v: u8) -> [u8; 3] {
    [0xB0, 7, v]
}

// --- CTL-08 devices ------------------------------------------------------

#[test]
fn ctl_08_same_names_are_numbered() {
    let names: Vec<String> = ["BCF2000", "nano", "BCF2000", "BCF2000"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(
        crate::midi_io::number_names(&names),
        ["BCF2000", "nano", "BCF2000 #2", "BCF2000 #3"]
    );
}

#[test]
fn ctl_08_states_ok_missing_disabled() {
    let (mut c, _) = setup("states");
    ports(&mut c, &[port("nano", true)]);
    let s = c.snapshot();
    let d = device(&s, "nano");
    assert_eq!(d["state"], "Disabled");
    assert_eq!(d["enabled"], false);
    assert_eq!(d["input"], true);
    assert_eq!(d["output"], true);
    assert_eq!(d["errors"], 0);
    req(
        &mut c,
        r#"{"v":1,"cmd":"control.enable","device":"nano","on":true}"#,
    )
    .unwrap();
    assert_eq!(device(&c.snapshot(), "nano")["state"], "Ok");
    ports(&mut c, &[]);
    assert_eq!(device(&c.snapshot(), "nano")["state"], "Missing");
    req(
        &mut c,
        r#"{"v":1,"cmd":"control.enable","device":"nano","on":false}"#,
    )
    .unwrap();
    assert_eq!(device(&c.snapshot(), "nano")["state"], "Disabled");
}

#[test]
fn ctl_08_two_devices_run_at_once_each_with_its_own_mapping() {
    let (mut c, dir) = setup("two");
    fs::write(
        dir.join("control/other.json"),
        r#"{"name":"other","device":"other","bindings":[{"input":{"kind":"cc","channel":1,"number":7},"action":"gm","pickup":false}]}"#,
    )
    .unwrap();
    let (mut c2, _) = Control::load(dir.join("control"), dir.join("state.json"));
    std::mem::swap(&mut c, &mut c2);
    ports(&mut c, &[port("nano", false), port("other", false)]);
    for d in ["nano", "other"] {
        let j = format!(r#"{{"v":1,"cmd":"control.enable","device":"{d}","on":true}}"#);
        req(&mut c, &j).unwrap();
    }
    assert_eq!(device(&c.snapshot(), "other")["mapping"], "other");
    packet(&mut c, "nano", &cc7(127));
    packet(&mut c, "other", &cc7(0));
    let e = engine();
    let cmds: Vec<Command> = c.commands(&e, 0.0).into_iter().map(|(_, c)| c).collect();
    assert!(cmds.contains(&Command::FaderSet {
        playback: slot(&e, 1, 1),
        level: 1.0
    }));
    assert!(cmds.contains(&Command::GrandMasterSet { level: 0.0 }));
}

// --- CTL-24 first sight --------------------------------------------------

#[test]
fn ctl_24_a_new_device_is_disabled_and_does_nothing() {
    let (mut c, _) = setup("new-disabled");
    ports(&mut c, &[port("nano", true)]);
    packet(&mut c, "nano", &cc7(127));
    assert!(c.commands(&engine(), 0.0).is_empty());
    assert!(c.feedback(&engine()).is_empty());
}

#[test]
fn ctl_24_enabled_it_gets_the_matching_mapping_and_it_is_remembered() {
    let (mut c, dir) = enabled("remembered");
    assert_eq!(device(&c.snapshot(), "nano")["mapping"], "nano");
    packet(&mut c, "nano", &cc7(127));
    assert_eq!(c.commands(&engine(), 0.0).len(), 1);
    // A new run: the device is known, enabled, with its mapping, and Missing until it is seen.
    let (mut again, _) = Control::load(dir.join("control"), dir.join("state.json"));
    let s = again.snapshot();
    let d = device(&s, "nano");
    assert_eq!(d["enabled"], true);
    assert_eq!(d["mapping"], "nano");
    assert_eq!(d["state"], "Missing");
    ports(&mut again, &[port("nano", true)]);
    assert_eq!(device(&again.snapshot(), "nano")["state"], "Ok");
}

#[test]
fn ctl_24_without_a_matching_mapping_it_gets_an_empty_one() {
    let (mut c, dir) = setup("empty-mapping");
    ports(&mut c, &[port("APC mini", true)]);
    req(
        &mut c,
        r#"{"v":1,"cmd":"control.enable","device":"APC mini","on":true}"#,
    )
    .unwrap();
    let s = c.snapshot();
    let name = device(&s, "APC mini")["mapping"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(name, "nano");
    let file = fs::read_to_string(dir.join(format!("control/{name}.json"))).unwrap();
    let (m, _) = cmdmx_control::mapping::Mapping::from_json(&file).unwrap();
    assert!(m.bindings.is_empty());
    assert!(cmdmx_control::mapping::device_matches(
        &m.device, "APC mini"
    ));
}

#[test]
fn ctl_24_a_star_mapping_matches_any_device() {
    let dir = tmp("star");
    fs::write(
        dir.join("control/any.json"),
        r#"{"name":"any","device":"*","bindings":[]}"#,
    )
    .unwrap();
    let (mut c, _) = Control::load(dir.join("control"), dir.join("state.json"));
    ports(&mut c, &[port("whatever", false)]);
    req(
        &mut c,
        r#"{"v":1,"cmd":"control.enable","device":"whatever","on":true}"#,
    )
    .unwrap();
    assert_eq!(device(&c.snapshot(), "whatever")["mapping"], "any");
}

// --- CTL-23 files ----------------------------------------------------------

#[test]
fn ctl_23_control_folder_follows_show_dir() {
    let args = |s: &str| {
        s.split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>()
            .into_iter()
    };
    let cli = crate::parse_args(args("")).unwrap();
    assert!(cli
        .control_dir
        .ends_with("Library/Application Support/CMDMX/control"));
    let cli = crate::parse_args(args("--show-dir /tmp/shows")).unwrap();
    assert_eq!(cli.control_dir, PathBuf::from("/tmp/shows/control"));
}

#[test]
fn ctl_23_state_json_keeps_the_last_show_and_the_devices() {
    let (mut c, dir) = setup("state-json");
    let state = dir.join("state.json");
    let shows = crate::shows::Shows::new(dir.join("Shows"), state.clone());
    shows.remember("Church").unwrap();
    ports(&mut c, &[port("nano", true)]);
    req(
        &mut c,
        r#"{"v":1,"cmd":"control.enable","device":"nano","on":true}"#,
    )
    .unwrap();
    let v: Value = serde_json::from_str(&fs::read_to_string(&state).unwrap()).unwrap();
    assert_eq!(v["last_show"], "Church");
    assert_eq!(v["control"]["devices"]["nano"]["enabled"], true);
    assert_eq!(v["control"]["devices"]["nano"]["mapping"], "nano");
    // Remembering another show keeps the devices.
    shows.remember("Other").unwrap();
    let v: Value = serde_json::from_str(&fs::read_to_string(&state).unwrap()).unwrap();
    assert_eq!(v["last_show"], "Other");
    assert_eq!(v["control"]["devices"]["nano"]["enabled"], true);
    assert_eq!(shows.last().as_deref(), Some("Other"));
}

#[test]
fn ctl_23_every_change_is_written_atomically_to_the_mapping_file() {
    let (mut c, dir) = enabled("written");
    req(
        &mut c,
        r#"{"v":1,"cmd":"control.binding","mapping":"nano","input":{"kind":"cc","channel":1,"number":9},"action":"gm","invert":true}"#,
    )
    .unwrap();
    let file = fs::read_to_string(dir.join("control/nano.json")).unwrap();
    let (m, w) = cmdmx_control::mapping::Mapping::from_json(&file).unwrap();
    assert!(w.is_empty());
    let b = m
        .bindings
        .iter()
        .find(|b| b.action.to_string() == "gm")
        .unwrap();
    assert!(b.invert);
    let leftovers: Vec<_> = fs::read_dir(dir.join("control"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| !n.ends_with(".json"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn ctl_23_import_refuses_an_existing_name_unless_replace() {
    let (mut c, dir) = setup("import");
    let json = json!({
        "v": 1, "cmd": "control.mapping_import",
        "json": r#"{"name":"nano","device":"x","bindings":[]}"#
    });
    let err = req(&mut c, &json.to_string()).unwrap_err();
    assert!(err.contains("nano"), "{err}");
    let mut json = json;
    json["replace"] = true.into();
    req(&mut c, &json.to_string()).unwrap();
    let file = fs::read_to_string(dir.join("control/nano.json")).unwrap();
    assert!(file.contains("\"x\""));
}

#[test]
fn ctl_23_a_bad_binding_in_a_file_is_one_warning_the_rest_loads() {
    let dir = tmp("bad-binding");
    fs::write(
        dir.join("control/m.json"),
        r#"{"name":"m","device":"*","bindings":[{"input":{"kind":"cc","channel":1,"number":1},"action":"fader x"},{"input":{"kind":"cc","channel":1,"number":2},"action":"gm"}]}"#,
    )
    .unwrap();
    fs::write(dir.join("control/broken.json"), "{not json").unwrap();
    let (c, warnings) = Control::load(dir.join("control"), dir.join("state.json"));
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(c.mapping_reply("m").contains("gm"));
}

// --- CTL-14 never blocking, coalesced ------------------------------------

#[test]
fn ctl_14_values_coalesce_per_tick_buttons_never() {
    let (mut c, _) = enabled("coalesce");
    for v in 0..=127u8 {
        packet(&mut c, "nano", &cc7(v));
    }
    for b in [
        [0x90, 60, 127],
        [0x80, 60, 0],
        [0x90, 60, 127],
        [0x80, 60, 0],
    ] {
        packet(&mut c, "nano", &b);
    }
    let e = engine();
    let cmds: Vec<Command> = c.commands(&e, 0.0).into_iter().map(|(_, c)| c).collect();
    let faders: Vec<_> = cmds
        .iter()
        .filter(|c| matches!(c, Command::FaderSet { .. }))
        .collect();
    assert_eq!(
        faders,
        [&Command::FaderSet {
            playback: slot(&e, 1, 1),
            level: 1.0
        }]
    );
    let buttons: Vec<bool> = cmds
        .iter()
        .filter_map(|c| match c {
            Command::PlaybackButton { on, .. } => Some(*on),
            _ => None,
        })
        .collect();
    assert_eq!(buttons, [true, false, true, false]);
    assert!(c.commands(&e, 0.0).is_empty(), "drained");
}

#[test]
fn ctl_14_a_flood_fits_in_a_tick() {
    let (mut c, _) = enabled("flood");
    let mut e = engine();
    let mut h = History::default();
    let start = Instant::now();
    // 2000 messages per second is 50 per 25 ms tick; take ten seconds' worth in one tick.
    for i in 0..20_000u32 {
        packet(&mut c, "nano", &cc7((i % 128) as u8));
    }
    let cmds = c.commands(&e, 0.0);
    assert!(run_all(&mut e, &mut h, cmds).is_empty());
    e.tick(0.025);
    let _ = c.feedback(&e);
    assert!(
        start.elapsed() < Duration::from_millis(250),
        "{:?}",
        start.elapsed()
    );
}

// --- CTL-15 malformed input ----------------------------------------------

#[test]
fn ctl_15_malformed_messages_are_counted_per_device_and_ignored() {
    let (mut c, _) = enabled("malformed");
    packet(&mut c, "nano", &[0x40, 0x41]); // data without a status
    packet(&mut c, "nano", &[0xB0, 7]); // truncated
    packet(&mut c, "nano", &cc7(127));
    let e = engine();
    assert_eq!(c.commands(&e, 0.0).len(), 1);
    assert_eq!(device(&c.snapshot(), "nano")["errors"], 2);
    // A packet from a device nobody listed changes nothing and does not panic.
    packet(&mut c, "ghost", &[0xFF, 0x00, 0x90]);
    assert!(c.commands(&e, 0.0).is_empty());
}

// --- CTL-16 hotplug (simulated) --------------------------------------------

#[test]
fn ctl_16_disconnect_is_one_warning_and_keeps_the_bindings() {
    let (mut c, _) = enabled("disconnect");
    let _ = c.take_messages();
    ports(&mut c, &[]);
    ports(&mut c, &[]);
    assert_eq!(c.take_messages(), ["nano disconnected"]);
    assert_eq!(device(&c.snapshot(), "nano")["state"], "Missing");
    ports(&mut c, &[port("nano", true)]);
    assert_eq!(device(&c.snapshot(), "nano")["state"], "Ok");
    packet(&mut c, "nano", &cc7(127));
    assert_eq!(c.commands(&engine(), 0.0).len(), 1);
}

// --- CTL-21 feedback -------------------------------------------------------

fn cc_out(v: u8) -> MidiOut {
    MidiOut {
        device: "nano".into(),
        bytes: vec![0xB0, 7, v],
    }
}

fn note_out(n: u8, v: u8) -> MidiOut {
    MidiOut {
        device: "nano".into(),
        bytes: vec![0x90, n, v],
    }
}

#[test]
fn ctl_21_full_state_on_attach_then_only_changes() {
    let (mut c, _) = enabled("attach");
    let mut e = engine();
    e.set_fader(slot(&e, 1, 1), 0.5, 0.0).unwrap();
    e.tick(0.0);
    let out = c.feedback(&e);
    assert!(out.contains(&cc_out(64)), "{out:?}");
    assert!(out.contains(&note_out(61, 0)), "page 2 LED off: {out:?}");
    assert!(c.feedback(&e).is_empty(), "nothing changed");
    // Reattach: the full state again.
    ports(&mut c, &[]);
    ports(&mut c, &[port("nano", true)]);
    assert!(c.feedback(&e).contains(&cc_out(64)));
}

#[test]
fn ctl_21_a_page_change_sends_the_new_targets() {
    let (mut c, _) = enabled("page");
    let mut e = engine();
    e.set_fader(slot(&e, 1, 1), 1.0, 0.0).unwrap();
    e.tick(0.0);
    let _ = c.feedback(&e);
    // Page 2's slot 1 is at 0: the fader goes down, the page 2 LED goes on.
    e.set_page(2).unwrap();
    e.tick(0.025);
    let out = c.feedback(&e);
    assert!(out.contains(&cc_out(0)), "{out:?}");
    assert!(out.contains(&note_out(61, 127)), "{out:?}");
    // An empty slot sends 0 too.
    e.set_page(1).unwrap();
    e.tick(0.05);
    assert!(c.feedback(&e).contains(&cc_out(127)));
    e.set_page(9).unwrap();
    e.tick(0.075);
    assert!(c.feedback(&e).contains(&cc_out(0)));
}

#[test]
fn ctl_21_no_echo_of_the_input_that_moved() {
    let (mut c, _) = enabled("echo");
    let mut e = engine();
    let mut h = History::default();
    let _ = c.feedback(&e);
    packet(&mut c, "nano", &cc7(100));
    let cmds = c.commands(&e, 0.0);
    assert!(run_all(&mut e, &mut h, cmds).is_empty());
    e.tick(0.025);
    assert!((level(&e, slot(&e, 1, 1)) - 100.0 / 127.0).abs() < 0.001);
    assert!(!c.feedback(&e).iter().any(|o| o.bytes.get(1) == Some(&7)));
}

#[test]
fn ctl_21_a_device_without_output_gets_no_feedback() {
    let (mut c, _) = setup("no-output");
    ports(&mut c, &[port("nano", false)]);
    req(
        &mut c,
        r#"{"v":1,"cmd":"control.enable","device":"nano","on":true}"#,
    )
    .unwrap();
    assert!(c.feedback(&engine()).is_empty());
}

// --- CTL-06 commands keep their class --------------------------------------

#[test]
fn ctl_06_a_fader_is_live_a_recording_line_is_an_edit_and_refusals_name_the_device() {
    let (mut c, _) = enabled("classes");
    let mut e = engine();
    let mut h = History::default();
    packet(&mut c, "nano", &cc7(127));
    let cmds = c.commands(&e, 0.0);
    assert!(run_all(&mut e, &mut h, cmds).is_empty());
    assert_eq!(h.undo_label(), None, "a fader move is live");
    // Group 1 Enter through the keypad selects: an undoable edit.
    for n in [70, 71, 72] {
        packet(&mut c, "nano", &[0x90, n, 127]);
        packet(&mut c, "nano", &[0x80, n, 0]);
    }
    let cmds = c.commands(&e, 0.0);
    assert!(cmds.iter().any(|(_, c)| matches!(c, Command::Line { .. })));
    assert!(run_all(&mut e, &mut h, cmds).is_empty());
    assert!(h.undo_label().is_some());
    // A refusal carries the device name.
    packet(&mut c, "nano", &[0x90, 62, 127]);
    let cmds = c.commands(&e, 0.0);
    let errors = run_all(&mut e, &mut h, cmds);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].starts_with("nano: "), "{errors:?}");
}

#[test]
fn ctl_06_page_next_at_the_last_page_does_nothing() {
    let dir = tmp("last-page");
    fs::write(
        dir.join("control/p.json"),
        r#"{"name":"p","device":"*","bindings":[{"input":{"kind":"note","channel":1,"number":1},"action":"page next"},{"input":{"kind":"note","channel":1,"number":2},"action":"page prev"}]}"#,
    )
    .unwrap();
    let (mut c, _) = Control::load(dir.join("control"), dir.join("state.json"));
    ports(&mut c, &[port("pad", false)]);
    req(
        &mut c,
        r#"{"v":1,"cmd":"control.enable","device":"pad","on":true}"#,
    )
    .unwrap();
    let mut e = engine();
    packet(&mut c, "pad", &[0x90, 2, 127]);
    assert!(c.commands(&e, 0.0).is_empty(), "page prev on page 1");
    e.set_page(cmdmx_core::playback::PAGES).unwrap();
    packet(&mut c, "pad", &[0x90, 1, 127]);
    assert!(c.commands(&e, 0.0).is_empty(), "page next on the last page");
    packet(&mut c, "pad", &[0x90, 2, 127]);
    assert_eq!(
        c.commands(&e, 0.0)
            .into_iter()
            .map(|(_, c)| c)
            .collect::<Vec<_>>(),
        [Command::PageSet {
            page: cmdmx_core::playback::PAGES - 1
        }]
    );
}

// --- CTL-25 Learn ----------------------------------------------------------

#[test]
fn ctl_25_learn_needs_three_steps_then_binds_and_ends() {
    let (mut c, dir) = enabled("learn");
    let _ = c.take_messages();
    req(
        &mut c,
        r#"{"v":1,"cmd":"control.learn","action":"fader 3"}"#,
    )
    .unwrap();
    assert_eq!(c.snapshot()["learn"], "fader 3");
    packet(&mut c, "nano", &[0xB0, 20, 30]);
    packet(&mut c, "nano", &[0xB0, 20, 32]); // jitter: 2 steps
    let _ = c.commands(&engine(), 0.0);
    assert_eq!(c.snapshot()["learn"], "fader 3", "jitter does not learn");
    packet(&mut c, "nano", &[0xB0, 20, 33]);
    let _ = c.commands(&engine(), 0.0);
    assert_eq!(c.snapshot()["learn"], Value::Null);
    assert_eq!(c.take_messages(), ["Learned: nano CC 20 ch 1 → fader 3"]);
    let file = fs::read_to_string(dir.join("control/nano.json")).unwrap();
    let (m, _) = cmdmx_control::mapping::Mapping::from_json(&file).unwrap();
    assert!(m.bindings.iter().any(|b| b.action.to_string() == "fader 3"));
}

#[test]
fn ctl_25_a_note_press_learns_at_once_and_replaces_with_a_message() {
    let (mut c, _) = enabled("learn-replace");
    let _ = c.take_messages();
    req(&mut c, r#"{"v":1,"cmd":"control.learn","action":"go"}"#).unwrap();
    // Re-arming replaces the armed action.
    req(&mut c, r#"{"v":1,"cmd":"control.learn","action":"back"}"#).unwrap();
    packet(&mut c, "nano", &[0x90, 61, 127]); // was page 2
    let cmds = c.commands(&engine(), 0.0);
    assert!(
        cmds.is_empty(),
        "a learning input does not also act: {cmds:?}"
    );
    let msgs = c.take_messages();
    assert!(
        msgs.contains(&"Learned: nano Note 61 ch 1 → back".to_string()),
        "{msgs:?}"
    );
    assert!(
        msgs.contains(&"Note 61 ch 1 was page 2".to_string()),
        "{msgs:?}"
    );
    assert!(c.mapping_reply("nano").contains("back"));
}

#[test]
fn ctl_25_cancel_and_disabled_devices_never_learn() {
    let (mut c, _) = enabled("learn-cancel");
    ports(&mut c, &[port("nano", true), port("other", true)]);
    req(&mut c, r#"{"v":1,"cmd":"control.learn","action":"go"}"#).unwrap();
    packet(&mut c, "other", &[0x90, 1, 127]);
    let _ = c.commands(&engine(), 0.0);
    assert_eq!(
        c.snapshot()["learn"],
        "go",
        "a disabled device does not learn"
    );
    req(&mut c, r#"{"v":1,"cmd":"control.learn_cancel"}"#).unwrap();
    assert_eq!(c.snapshot()["learn"], Value::Null);
}

// --- CTL-28 last input -----------------------------------------------------

#[test]
fn ctl_28_last_input_is_shown_at_most_ten_times_a_second() {
    let (mut c, _) = enabled("last-input");
    let _ = c.delta(0.0);
    c.event(
        MidiEvent::Packet {
            device: "nano".into(),
            bytes: cc7(64).to_vec(),
        },
        1.0,
    );
    let _ = c.commands(&engine(), 1.0);
    let d: Value = serde_json::from_str(&c.delta(1.0).unwrap()).unwrap();
    assert_eq!(d["type"], "control");
    assert_eq!(
        d["last_input"],
        json!({"device":"nano","input":"CC 7 ch 1","value":64})
    );
    c.event(
        MidiEvent::Packet {
            device: "nano".into(),
            bytes: cc7(65).to_vec(),
        },
        1.05,
    );
    let _ = c.commands(&engine(), 1.05);
    assert!(c.delta(1.05).is_none(), "within 100 ms");
    let d: Value = serde_json::from_str(&c.delta(1.11).unwrap()).unwrap();
    assert_eq!(d["last_input"]["value"], 65);
    assert_eq!(c.snapshot()["last_input"]["value"], 65);
}

// --- CTL-29, 39, 40 IPC -------------------------------------------------------

#[test]
fn ctl_29_control_commands_are_class_neutral() {
    for json in [
        r#"{"v":1,"cmd":"control.enable","device":"nano","on":true}"#,
        r#"{"v":1,"cmd":"control.learn","action":"go"}"#,
        r#"{"v":1,"cmd":"control.mapping_delete","name":"nano"}"#,
    ] {
        let cmd = ipc::parse_command(json).unwrap();
        assert_eq!(crate::class(&cmd), crate::Class::Neutral, "{json}");
    }
}

#[test]
fn ctl_39_snapshot_has_the_control_section() {
    let (c, _) = enabled("snapshot");
    let s: Value = serde_json::from_str(&ipc::snapshot(
        &engine(),
        &[],
        &crate::shows::ShowState {
            name: "Demo".into(),
            saved_at: None,
            autosave_ok: true,
        },
        &ipc::UndoInfo::default(),
        &c.snapshot(),
    ))
    .unwrap();
    let control = &s["control"];
    assert_eq!(
        device(control, "nano"),
        &json!({"name":"nano","input":true,"output":true,"state":"Ok","enabled":true,"mapping":"nano","errors":0,"kind":"midi"})
    );
    assert!(control["mappings"]
        .as_array()
        .unwrap()
        .contains(&json!({"name":"nano","profile":false})));
    assert_eq!(control["learn"], Value::Null);
    assert_eq!(control["last_input"], Value::Null);
}

#[test]
fn ctl_39_a_change_sends_a_control_delta() {
    let (mut c, _) = setup("delta");
    let _ = c.delta(0.0);
    assert!(c.delta(0.0).is_none());
    ports(&mut c, &[port("nano", true)]);
    let d: Value = serde_json::from_str(&c.delta(0.0).unwrap()).unwrap();
    assert_eq!(d["type"], "control");
    assert_eq!(device(&d, "nano")["state"], "Disabled");
}

#[test]
fn ctl_40_every_command_parses_and_unknown_actions_are_refused() {
    for json in [
        r#"{"v":1,"cmd":"control.enable","device":"nano","on":true}"#,
        r#"{"v":1,"cmd":"control.assign","device":"nano","mapping":"nano"}"#,
        r#"{"v":1,"cmd":"control.learn","action":"fader 2.3","mapping":"nano"}"#,
        r#"{"v":1,"cmd":"control.learn_cancel"}"#,
        r#"{"v":1,"cmd":"control.binding","mapping":"nano","input":{"kind":"cc","channel":1,"number":7},"pickup":true}"#,
        r#"{"v":1,"cmd":"control.unbind","mapping":"nano","input":{"kind":"cc","channel":1,"number":7}}"#,
        r#"{"v":1,"cmd":"control.mapping_new","name":"x","device":"x*"}"#,
        r#"{"v":1,"cmd":"control.mapping_rename","name":"x","to":"y"}"#,
        r#"{"v":1,"cmd":"control.mapping_delete","name":"y"}"#,
        r#"{"v":1,"cmd":"control.mapping_import","json":"{}","replace":true}"#,
    ] {
        assert!(
            matches!(ipc::parse_command(json), Ok(Command::Control(_))),
            "{json}"
        );
    }
    assert_eq!(
        ipc::parse_command(r#"{"v":1,"cmd":"control.learn","action":"fader x"}"#),
        Err("unknown action \"fader x\"".to_string())
    );
}

#[test]
fn ctl_40_mapping_edits_and_their_refusals() {
    let (mut c, dir) = enabled("edits");
    // An edit without an action keeps the binding's action.
    req(
        &mut c,
        r#"{"v":1,"cmd":"control.binding","mapping":"nano","input":{"kind":"cc","channel":1,"number":7},"invert":true}"#,
    )
    .unwrap();
    let reply: Value = serde_json::from_str(&c.mapping_reply("nano")).unwrap();
    assert_eq!(reply["type"], "control.mapping");
    let b = reply["mapping"]["bindings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["input"]["number"] == 7)
        .unwrap()
        .clone();
    assert_eq!(b["action"], "fader 1");
    assert_eq!(b["invert"], true);
    // A new input needs an action.
    assert!(req(
        &mut c,
        r#"{"v":1,"cmd":"control.binding","mapping":"nano","input":{"kind":"cc","channel":1,"number":99}}"#
    )
    .is_err());
    req(
        &mut c,
        r#"{"v":1,"cmd":"control.unbind","mapping":"nano","input":{"kind":"cc","channel":1,"number":7}}"#,
    )
    .unwrap();
    assert!(!c.mapping_reply("nano").contains("fader 1"));
    // New, rename (the device follows), delete.
    req(
        &mut c,
        r#"{"v":1,"cmd":"control.mapping_new","name":"spare","device":"x"}"#,
    )
    .unwrap();
    assert!(req(
        &mut c,
        r#"{"v":1,"cmd":"control.mapping_new","name":"spare","device":"x"}"#
    )
    .is_err());
    req(
        &mut c,
        r#"{"v":1,"cmd":"control.mapping_rename","name":"nano","to":"mine"}"#,
    )
    .unwrap();
    assert!(dir.join("control/mine.json").exists());
    assert!(!dir.join("control/nano.json").exists());
    assert_eq!(device(&c.snapshot(), "nano")["mapping"], "mine");
    req(
        &mut c,
        r#"{"v":1,"cmd":"control.assign","device":"nano","mapping":"spare"}"#,
    )
    .unwrap();
    assert_eq!(device(&c.snapshot(), "nano")["mapping"], "spare");
    assert!(req(
        &mut c,
        r#"{"v":1,"cmd":"control.assign","device":"nano","mapping":"nope"}"#
    )
    .is_err());
    req(
        &mut c,
        r#"{"v":1,"cmd":"control.mapping_delete","name":"mine"}"#,
    )
    .unwrap();
    assert!(!dir.join("control/mine.json").exists());
    assert!(req(
        &mut c,
        r#"{"v":1,"cmd":"control.enable","device":"ghost","on":true}"#
    )
    .is_err());
}

// --- macOS: a virtual CoreMIDI port through the real MIDI thread -------------

#[cfg(target_os = "macos")]
mod virtual_port {
    use std::sync::mpsc::{channel, Receiver};

    use midir::os::unix::VirtualOutput;
    use midir::MidiOutput;

    use super::*;

    /// Drain the MIDI thread's events into `c` until `done` holds, at most `secs` seconds.
    fn wait(
        c: &mut Control,
        rx: &Receiver<MidiEvent>,
        secs: f64,
        done: impl Fn(&Control) -> bool,
    ) -> bool {
        let start = Instant::now();
        while start.elapsed().as_secs_f64() < secs {
            while let Ok(ev) = rx.try_recv() {
                c.event(ev, 0.0);
            }
            if done(c) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    fn state(c: &Control, name: &str) -> Option<String> {
        let s = c.snapshot();
        s["devices"]
            .as_array()?
            .iter()
            .find(|d| d["name"] == name)
            .and_then(|d| d["state"].as_str().map(str::to_string))
    }

    #[test]
    fn ctl_16_a_virtual_port_goes_missing_and_is_attached_again_within_2s() {
        let name = format!("cmdmx-test-hotplug-{}", std::process::id());
        let (mut c, _) = setup("virtual-hotplug");
        let (tx, rx) = channel();
        let _out = crate::midi_io::spawn(tx).unwrap();
        let src = MidiOutput::new("cmdmx-test")
            .unwrap()
            .create_virtual(&name)
            .unwrap();
        assert!(
            wait(&mut c, &rx, 3.0, |c| state(c, &name).is_some()),
            "seen"
        );
        let j = json!({"v":1,"cmd":"control.enable","device":name,"on":true});
        req(&mut c, &j.to_string()).unwrap();
        assert_eq!(state(&c, &name).as_deref(), Some("Ok"));
        let _ = c.take_messages();
        drop(src);
        assert!(wait(&mut c, &rx, 3.0, |c| state(c, &name).as_deref()
            == Some("Missing")));
        assert_eq!(c.take_messages(), [format!("{name} disconnected")]);
        let _src = MidiOutput::new("cmdmx-test")
            .unwrap()
            .create_virtual(&name)
            .unwrap();
        assert!(
            wait(&mut c, &rx, 2.0, |c| state(c, &name).as_deref()
                == Some("Ok")),
            "attached again within 2 s"
        );
    }

    #[test]
    fn ctl_14_a_virtual_port_flood_keeps_40_hz() {
        let name = format!("cmdmx-test-flood-{}", std::process::id());
        let (mut c, dir) = setup("virtual-flood");
        let mapping = json!({"name":"flood","device":name,"bindings":[
            {"input":{"kind":"cc","channel":1,"number":7},"action":"fader 1","pickup":false}]});
        fs::write(dir.join("control/flood.json"), mapping.to_string()).unwrap();
        let (mut c2, _) = Control::load(dir.join("control"), dir.join("state.json"));
        std::mem::swap(&mut c, &mut c2);
        let (tx, rx) = channel();
        let _out = crate::midi_io::spawn(tx).unwrap();
        let mut src = MidiOutput::new("cmdmx-test")
            .unwrap()
            .create_virtual(&name)
            .unwrap();
        assert!(wait(&mut c, &rx, 3.0, |c| state(c, &name).is_some()));
        let j = json!({"v":1,"cmd":"control.enable","device":name,"on":true});
        req(&mut c, &j.to_string()).unwrap();
        // The sender: 2000 messages a second for one second, ending at 127.
        let sender = std::thread::spawn(move || {
            for i in 0..2000u32 {
                let v = if i == 1999 { 127 } else { (i % 100) as u8 };
                src.send(&[0xB0, 7, v]).unwrap();
                if i % 50 == 49 {
                    std::thread::sleep(Duration::from_millis(25));
                }
            }
            src
        });
        // The tick: 40 Hz for 1.5 s, measuring the work per tick.
        let mut e = engine();
        let mut h = History::default();
        let started = Instant::now();
        let mut worst = Duration::ZERO;
        for n in 0..60u32 {
            let t0 = Instant::now();
            let now = started.elapsed().as_secs_f64();
            while let Ok(ev) = rx.try_recv() {
                c.event(ev, now);
            }
            let cmds = c.commands(&e, now);
            assert!(run_all(&mut e, &mut h, cmds).is_empty());
            e.tick(now);
            let _ = c.feedback(&e);
            worst = worst.max(t0.elapsed());
            let next = Duration::from_millis(25) * (n + 1);
            std::thread::sleep(next.saturating_sub(started.elapsed()));
        }
        let _src = sender.join().unwrap();
        assert!(worst < Duration::from_millis(25), "worst tick {worst:?}");
        assert_eq!(level(&e, slot(&e, 1, 1)), 1.0, "the last value arrived");
    }
}

// --- VS1-07 the Mackie Control profile (CTL-20, 21, 30..33) ------------------

const MC: &str = "Mackie Control";

/// A control layer in an empty folder: only the shipped profile.
fn bare(test: &str) -> (Control, PathBuf) {
    let dir = tmp(test);
    let (c, warnings) = Control::load(dir.join("control"), dir.join("state.json"));
    assert!(warnings.is_empty(), "{warnings:?}");
    (c, dir)
}

/// `names` connected with outputs and enabled; each gets the profile (CTL-24).
fn surface(test: &str, names: &[&str]) -> (Control, PathBuf) {
    let (mut c, dir) = bare(test);
    let list: Vec<PortInfo> = names.iter().map(|n| port(n, true)).collect();
    ports(&mut c, &list);
    for n in names {
        req(
            &mut c,
            &format!(r#"{{"v":1,"cmd":"control.enable","device":"{n}","on":true}}"#),
        )
        .unwrap();
    }
    (c, dir)
}

/// The demo engine with `fpp` faders per page, a playback 900 + s on page 1 slot s for every
/// slot above 8, and playback 950 on page 2 slot 1.
fn engine_fpp(fpp: u16) -> Engine {
    use cmdmx_core::playback::Playback;
    use std::collections::BTreeMap;
    let demo = engine();
    let mut show = demo.to_show(cmdmx_core::show::Meta {
        name: "t".into(),
        created: String::new(),
        modified: String::new(),
        app_version: String::new(),
    });
    show.settings.faders_per_page = fpp;
    let (mut e, _) = Engine::from_show(&show);
    let values = || {
        BTreeMap::from([(
            cmdmx_core::programmer::Item::fixture(1),
            BTreeMap::from([("Dimmer".to_string(), 1.0.into())]),
        )])
    };
    for s in 9..=fpp {
        e.add_playback(Playback::new(900 + u32::from(s), 1, s, values()));
    }
    if e.playbacks().all(|p| (p.page, p.slot) != (2, 1)) {
        e.add_playback(Playback::new(950, 2, 1, values()));
    }
    assert_eq!(e.settings().faders_per_page, fpp);
    e
}

/// Fader `ch` (1..=9) moved to the 14-bit `v`.
fn pb(ch: u8, v: u16) -> [u8; 3] {
    [0xE0 | (ch - 1), (v & 0x7F) as u8, (v >> 7) as u8]
}

fn note(n: u8, v: u8) -> [u8; 3] {
    [0x90, n, v]
}

/// The motor of fader `ch` driven to `level` (CTL-02).
fn motor(device: &str, ch: u8, level: f32) -> MidiOut {
    MidiOut {
        device: device.into(),
        bytes: pb(ch, cmdmx_control::mcu::position(level)).to_vec(),
    }
}

fn out(device: &str, bytes: [u8; 3]) -> MidiOut {
    MidiOut {
        device: device.into(),
        bytes: bytes.to_vec(),
    }
}

fn press(c: &mut Control, device: &str, n: u8) {
    packet(c, device, &note(n, 127));
    packet(c, device, &note(n, 0));
}

/// Move fader `ch` of `device` to full, return the playback it set.
fn fader_target(c: &mut Control, e: &Engine, device: &str, ch: u8) -> Option<u32> {
    packet(c, device, &pb(ch, 0x3FFF));
    let cmds = c.commands(e, 0.0);
    assert!(cmds.len() <= 1, "{cmds:?}");
    cmds.into_iter().find_map(|(_, cmd)| match cmd {
        Command::FaderSet { playback, .. } => Some(playback),
        _ => None,
    })
}

#[test]
fn ctl_30_the_profile_ships_read_only_and_is_listed() {
    let (mut c, dir) = bare("mc-listed");
    let mappings = c.snapshot()["mappings"].clone();
    assert!(
        mappings
            .as_array()
            .unwrap()
            .contains(&json!({"name": MC, "profile": true})),
        "{mappings}"
    );
    assert!(c.mapping_reply(MC).contains("fader 1"));
    ports(&mut c, &[port("BCF2000", true)]);
    req(
        &mut c,
        r#"{"v":1,"cmd":"control.enable","device":"BCF2000","on":true}"#,
    )
    .unwrap();
    assert_eq!(device(&c.snapshot(), "BCF2000")["mapping"], MC);
    assert!(
        !dir.join("control/Mackie Control.json").exists(),
        "a profile is copied only when it is edited"
    );
    // The shipped file loads clean and carries the driver.
    let (m, warnings) = cmdmx_control::mapping::Mapping::from_json(include_str!(
        "../../../profiles/mackie-control.json"
    ))
    .unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(m.name, MC);
    assert_eq!(m.extra.get("driver"), Some(&json!("mackie_control")));
    assert_eq!(m.bindings.len(), 41);
}

#[test]
fn ctl_30_editing_the_profile_saves_a_user_copy_that_hides_it() {
    let (mut c, dir) = bare("mc-copy");
    let listed = |c: &Control| -> Vec<Value> {
        c.snapshot()["mappings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["name"] == MC)
            .cloned()
            .collect()
    };
    req(
        &mut c,
        r#"{"v":1,"cmd":"control.binding","mapping":"Mackie Control","input":{"kind":"note","channel":1,"number":33},"action":"key step_prev"}"#,
    )
    .unwrap();
    assert!(dir.join("control/Mackie Control.json").exists());
    assert_eq!(listed(&c), [json!({"name": MC, "profile": false})]);
    // After a restart the copy still hides the profile.
    let (mut again, warnings) = Control::load(dir.join("control"), dir.join("state.json"));
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(listed(&again), [json!({"name": MC, "profile": false})]);
    assert!(again.mapping_reply(MC).contains("step_prev"));
    // Deleting the copy brings the shipped profile back.
    req(
        &mut again,
        r#"{"v":1,"cmd":"control.mapping_delete","name":"Mackie Control"}"#,
    )
    .unwrap();
    assert_eq!(listed(&again), [json!({"name": MC, "profile": true})]);
    assert!(!again.mapping_reply(MC).contains("step_prev"));
    // The profile itself cannot be deleted or renamed.
    assert!(req(
        &mut again,
        r#"{"v":1,"cmd":"control.mapping_delete","name":"Mackie Control"}"#,
    )
    .is_err());
    assert!(req(
        &mut again,
        r#"{"v":1,"cmd":"control.mapping_rename","name":"Mackie Control","to":"mine"}"#,
    )
    .is_err());
    assert_eq!(listed(&again), [json!({"name": MC, "profile": true})]);
}

#[test]
fn ctl_31_fader_1_moves_playback_1_and_motor_faders_never_pick_up() {
    let (mut c, _) = surface("mc-fader", &["BCF2000"]);
    let mut e = engine();
    let mut h = History::default();
    let p1 = slot(&e, 1, 1);
    packet(&mut c, "BCF2000", &pb(1, 0x3FFF));
    let cmds = c.commands(&e, 0.0);
    assert!(run_all(&mut e, &mut h, cmds).is_empty());
    e.tick(0.025);
    assert_eq!(level(&e, p1), 1.0, "full travel is exactly full (CTL-01)");
    // Far from the engine level a motor fader still sets it: no pickup (CTL-13).
    e.set_fader(p1, 0.0, 0.05).unwrap();
    e.tick(0.05);
    packet(&mut c, "BCF2000", &pb(1, 512 << 4));
    let cmds = c.commands(&e, 0.075);
    assert!(run_all(&mut e, &mut h, cmds).is_empty());
    e.tick(0.075);
    assert!((level(&e, p1) - 512.0 / 1023.0).abs() < 1e-6);
}

#[test]
fn ctl_31_six_per_page_leaves_faders_7_and_8_unbound() {
    let (mut c, _) = surface("mc-six", &["BCF2000"]);
    let mut e = engine_fpp(6);
    e.set_fader(slot(&e, 1, 6), 1.0, 0.0).unwrap();
    e.tick(0.0);
    let fb = c.feedback(&e);
    assert!(fb.contains(&motor("BCF2000", 6, 1.0)), "{fb:?}");
    assert!(fb.contains(&motor("BCF2000", 7, 0.0)), "{fb:?}");
    assert!(fb.contains(&motor("BCF2000", 8, 0.0)), "{fb:?}");
    assert!(
        fb.contains(&out("BCF2000", note(6, 0))),
        "REC 7 LED off: {fb:?}"
    );
    assert_eq!(fader_target(&mut c, &e, "BCF2000", 6), Some(slot(&e, 1, 6)));
    assert_eq!(fader_target(&mut c, &e, "BCF2000", 7), None);
    press(&mut c, "BCF2000", 6); // REC 7: unbound, does nothing
    assert!(c.commands(&e, 0.0).is_empty());
    // Fader 6's own move is not echoed; unbound fader 7 stays at 0 (CTL-20, 31).
    assert!(c.feedback(&e).is_empty());
}

#[test]
fn ctl_31_ten_per_page_channel_shows_slots_9_and_10_on_faders_1_and_2() {
    let (mut c, _) = surface("mc-ten", &["BCF2000"]);
    let mut e = engine_fpp(10);
    e.set_fader(slot(&e, 1, 9), 1.0, 0.0).unwrap();
    e.set_fader(slot(&e, 1, 3), 0.5, 0.0).unwrap();
    e.tick(0.0);
    let _ = c.feedback(&e);
    assert_eq!(fader_target(&mut c, &e, "BCF2000", 8), Some(slot(&e, 1, 8)));
    press(&mut c, "BCF2000", 49); // Channel ▶
    assert!(
        c.commands(&e, 0.0).is_empty(),
        "the window is not an engine command"
    );
    let fb = c.feedback(&e);
    assert!(fb.contains(&motor("BCF2000", 1, 1.0)), "{fb:?}");
    assert!(fb.contains(&motor("BCF2000", 3, 0.0)), "{fb:?}");
    assert_eq!(fader_target(&mut c, &e, "BCF2000", 1), Some(slot(&e, 1, 9)));
    assert_eq!(
        fader_target(&mut c, &e, "BCF2000", 2),
        Some(slot(&e, 1, 10))
    );
    assert_eq!(fader_target(&mut c, &e, "BCF2000", 3), None);
    // At the last window Channel ▶ stops; Channel ◀ goes back.
    press(&mut c, "BCF2000", 49);
    assert_eq!(fader_target(&mut c, &e, "BCF2000", 1), Some(slot(&e, 1, 9)));
    press(&mut c, "BCF2000", 48);
    assert_eq!(fader_target(&mut c, &e, "BCF2000", 1), Some(slot(&e, 1, 1)));
}

#[test]
fn ctl_31_an_extender_adds_eight_faders() {
    let (mut c, _) = surface("mc-ext", &["BCF2000", "X-Touch-Ext"]);
    let e = engine_fpp(16);
    assert_eq!(fader_target(&mut c, &e, "BCF2000", 1), Some(slot(&e, 1, 1)));
    assert_eq!(
        fader_target(&mut c, &e, "X-Touch-Ext", 1),
        Some(slot(&e, 1, 9))
    );
    assert_eq!(
        fader_target(&mut c, &e, "X-Touch-Ext", 8),
        Some(slot(&e, 1, 16))
    );
    // 16 faders show the whole page: Channel ▶ moves nothing.
    press(&mut c, "BCF2000", 49);
    assert_eq!(fader_target(&mut c, &e, "BCF2000", 1), Some(slot(&e, 1, 1)));
}

#[test]
fn ctl_31_the_user_sets_the_unit_order() {
    let (mut c, dir) = surface("mc-order", &["BCF2000", "X-Touch-Ext"]);
    let e = engine_fpp(16);
    req(
        &mut c,
        r#"{"v":1,"cmd":"control.order","devices":["X-Touch-Ext","BCF2000"]}"#,
    )
    .unwrap();
    assert_eq!(
        fader_target(&mut c, &e, "X-Touch-Ext", 1),
        Some(slot(&e, 1, 1))
    );
    assert_eq!(fader_target(&mut c, &e, "BCF2000", 1), Some(slot(&e, 1, 9)));
    assert_eq!(c.snapshot()["order"], json!(["X-Touch-Ext", "BCF2000"]));
    // Kept in state.json; a unit not in the list follows the listed ones, by name.
    let (mut again, _) = Control::load(dir.join("control"), dir.join("state.json"));
    assert_eq!(again.snapshot()["order"], json!(["X-Touch-Ext", "BCF2000"]));
    ports(
        &mut again,
        &[
            port("BCF2000", true),
            port("X-Touch-Ext", true),
            port("A-X-Touch", true),
        ],
    );
    req(
        &mut again,
        r#"{"v":1,"cmd":"control.enable","device":"A-X-Touch","on":true}"#,
    )
    .unwrap();
    let e = engine_fpp(24);
    assert_eq!(
        fader_target(&mut again, &e, "X-Touch-Ext", 1),
        Some(slot(&e, 1, 1))
    );
    assert_eq!(
        fader_target(&mut again, &e, "BCF2000", 1),
        Some(slot(&e, 1, 9))
    );
    assert_eq!(
        fader_target(&mut again, &e, "A-X-Touch", 1),
        Some(slot(&e, 1, 17))
    );
}

#[test]
fn ctl_32_bank_pages_and_any_page_change_resets_the_window() {
    let (mut c, _) = surface("mc-bank", &["BCF2000"]);
    let mut e = engine_fpp(10);
    let mut h = History::default();
    press(&mut c, "BCF2000", 49); // Channel ▶: faders 1–2 show slots 9–10
    let _ = c.commands(&e, 0.0);
    press(&mut c, "BCF2000", 47); // Bank ▶
    let cmds = c.commands(&e, 0.0);
    assert!(
        cmds.iter()
            .any(|(_, cmd)| matches!(cmd, Command::PageSet { page: 2 })),
        "{cmds:?}"
    );
    assert!(run_all(&mut e, &mut h, cmds).is_empty());
    e.tick(0.025);
    assert_eq!(e.page(), 2);
    assert_eq!(fader_target(&mut c, &e, "BCF2000", 1), Some(slot(&e, 2, 1)));
    // A page change from the screen resets it too.
    press(&mut c, "BCF2000", 49);
    let _ = c.commands(&e, 0.05);
    e.set_page(1).unwrap();
    e.tick(0.05);
    assert_eq!(fader_target(&mut c, &e, "BCF2000", 1), Some(slot(&e, 1, 1)));
    // Bank ◀ on page 1 does nothing.
    press(&mut c, "BCF2000", 46);
    assert!(c.commands(&e, 0.075).is_empty());
}

#[test]
fn ctl_33_default_bindings() {
    use cmdmx_core::playback::{HandleButton, ListAction};
    let (mut c, _) = surface("mc-defaults", &["BCF2000"]);
    let e = engine();
    let p1 = slot(&e, 1, 1);
    for bytes in [
        pb(9, 0x3FFF),    // master fader
        note(0, 127),     // REC 1
        note(24, 127),    // SELECT 1
        note(94, 127),    // Play
        note(93, 127),    // Stop
        note(91, 127),    // Rewind
        note(32, 127),    // V-Pot push 1
        note(33, 127),    // V-Pot push 2: unbound
        [0xB0, 17, 0x41], // V-Pot 2 down 1: no wheel 2 without a selection
    ] {
        packet(&mut c, "BCF2000", &bytes);
    }
    let cmds: Vec<Command> = c.commands(&e, 0.0).into_iter().map(|(_, c)| c).collect();
    assert_eq!(cmds.len(), 7, "{cmds:?}");
    assert!(matches!(cmds[0], Command::GrandMasterSet { level } if level == 1.0));
    assert!(matches!(
        cmds[1],
        Command::PlaybackButton { id, button: HandleButton::Upper, on: true, .. } if id == p1
    ));
    assert!(matches!(
        cmds[2],
        Command::PlaybackButton { id, button: HandleButton::Lower, on: true, .. } if id == p1
    ));
    assert!(matches!(
        cmds[3],
        Command::CuelistAction {
            id: None,
            action: ListAction::Go
        }
    ));
    assert!(matches!(
        cmds[4],
        Command::CuelistAction {
            id: None,
            action: ListAction::Stop
        }
    ));
    assert!(matches!(
        cmds[5],
        Command::CuelistAction {
            id: None,
            action: ListAction::Back
        }
    ));
    assert!(matches!(cmds[6], Command::Step { forward: true }));
    // Rings show the wheel's value; with no wheel they are off.
    let fb = c.feedback(&e);
    assert!(fb.contains(&out("BCF2000", [0xB0, 48, 0])), "{fb:?}");
    assert!(
        !fb.iter().any(|o| o.bytes[0] == 0xE8),
        "no echo of the master's own move (CTL-20): {fb:?}"
    );
}

#[test]
fn ctl_20_a_touched_motor_gets_no_position_until_release_and_touch_never_learns() {
    let (mut c, _) = surface("mc-touch", &["BCF2000"]);
    let mut e = engine();
    let p1 = slot(&e, 1, 1);
    let _ = c.feedback(&e);
    packet(&mut c, "BCF2000", &note(104, 127));
    assert!(c.commands(&e, 0.0).is_empty(), "touch is not an input");
    e.set_fader(p1, 0.7, 0.0).unwrap();
    e.tick(0.025);
    let fb = c.feedback(&e);
    assert!(!fb.iter().any(|o| o.bytes[0] == 0xE0), "{fb:?}");
    packet(&mut c, "BCF2000", &note(104, 0));
    assert!(c.commands(&e, 0.05).is_empty());
    assert!(c.feedback(&e).contains(&motor("BCF2000", 1, 0.7)));
    assert!(!c.feedback(&e).iter().any(|o| o.bytes[0] == 0xE0), "once");
    // Learn ignores touch (CTL-25).
    req(&mut c, r#"{"v":1,"cmd":"control.learn","action":"go"}"#).unwrap();
    packet(&mut c, "BCF2000", &note(105, 127));
    assert_eq!(c.snapshot()["learn"], "go");
}

#[test]
fn ctl_21_motors_follow_a_page_change() {
    let (mut c, _) = surface("mc-page", &["BCF2000"]);
    let mut e = engine();
    e.set_fader(slot(&e, 1, 1), 1.0, 0.0).unwrap();
    e.tick(0.0);
    assert!(c.feedback(&e).contains(&motor("BCF2000", 1, 1.0)));
    e.set_page(2).unwrap();
    e.tick(0.025);
    let fb = c.feedback(&e);
    assert!(fb.contains(&motor("BCF2000", 1, 0.0)), "{fb:?}");
}

// --- CTL-59 Fan ---------------------------------------------------------------

const FAN: &str = r#"{"name":"nano","device":"nano*","bindings":[
  {"input":{"kind":"note","channel":1,"number":80},"action":"fan"},
  {"input":{"kind":"note","channel":1,"number":81},"action":"fan wings"},
  {"input":{"kind":"note","channel":1,"number":82},"action":"fan off"}
]}"#;

#[test]
fn ctl_59_fan_buttons_toggle_set_and_turn_off_with_led_feedback() {
    use cmdmx_core::programmer::FanCurve;
    let dir = tmp("fan");
    fs::write(dir.join("control/nano.json"), FAN).unwrap();
    let (mut c, warnings) = Control::load(dir.join("control"), dir.join("state.json"));
    assert!(warnings.is_empty(), "{warnings:?}");
    ports(&mut c, &[port("nano", true)]);
    req(
        &mut c,
        r#"{"v":1,"cmd":"control.enable","device":"nano","on":true}"#,
    )
    .unwrap();
    let mut e = engine();
    let mut h = History::default();
    e.select_fixture(101).unwrap();
    let n = h.len();
    let _ = c.feedback(&e);

    let mut push = |c: &mut Control, e: &mut Engine, key: u8| {
        press(c, "nano", key);
        let cmds = c.commands(e, 0.0);
        assert!(run_all(e, &mut h, cmds).is_empty());
        e.tick(0.025);
        c.feedback(e)
    };

    let out = push(&mut c, &mut e, 80);
    assert_eq!(e.fan(), Some(FanCurve::Line));
    assert!(out.contains(&note_out(80, 127)), "fan LED on: {out:?}");

    let out = push(&mut c, &mut e, 81);
    assert_eq!(e.fan(), Some(FanCurve::Wings));
    assert!(
        out.contains(&note_out(81, 127)),
        "fan wings LED on: {out:?}"
    );

    let out = push(&mut c, &mut e, 82);
    assert_eq!(e.fan(), None);
    assert!(out.contains(&note_out(80, 0)), "fan LED off: {out:?}");
    assert!(out.contains(&note_out(81, 0)), "fan wings LED off: {out:?}");

    push(&mut c, &mut e, 80);
    assert_eq!(
        e.fan(),
        Some(FanCurve::Wings),
        "the toggle keeps the last curve"
    );
    assert_eq!(h.len(), n, "Fan is live, never in the history");
}

// --- FX-35 effect actions -----------------------------------------------------

const EFFECT: &str = r#"{"name":"nano","device":"nano*","bindings":[
  {"input":{"kind":"note","channel":1,"number":90},"action":"effect tap"},
  {"input":{"kind":"note","channel":1,"number":91},"action":"effect restart"},
  {"input":{"kind":"note","channel":1,"number":92},"action":"effect stop"},
  {"input":{"kind":"cc","channel":1,"number":16},"action":"wheel 1","encoding":"sign_magnitude"}
]}"#;

#[test]
fn fx_35_effect_tap_restart_stop_and_the_effect_wheels_from_a_device() {
    use cmdmx_control::action::Action;
    for name in ["effect tap", "effect restart", "effect stop"] {
        let a: Action = name.parse().unwrap();
        assert_eq!(a.to_string(), name, "the catalogue line round-trips");
    }
    let dir = tmp("effect");
    fs::write(dir.join("control/nano.json"), EFFECT).unwrap();
    let (mut c, warnings) = Control::load(dir.join("control"), dir.join("state.json"));
    assert!(warnings.is_empty(), "{warnings:?}");
    ports(&mut c, &[port("nano", true)]);
    req(
        &mut c,
        r#"{"v":1,"cmd":"control.enable","device":"nano","on":true}"#,
    )
    .unwrap();
    let mut e = engine();
    let mut h = History::default();
    for f in 101..=104 {
        e.select_fixture(f).unwrap();
    }
    e.effect_apply(&cmdmx_core::effect::EffectApply::Look("Circle".into()))
        .unwrap();
    e.set_effect_wheels(true);
    let _ = c.feedback(&e);

    // Taps 0.5 s apart set the entry's tempo to 120 BPM.
    for t in [1.0, 1.5, 2.0] {
        press(&mut c, "nano", 90);
        let cmds = c.commands(&e, t);
        assert!(run_all(&mut e, &mut h, cmds).is_empty());
    }
    assert!((e.effects()[0].params.bpm - 120.0).abs() < 0.05);

    e.tick(0.0);
    e.tick(3.0);
    press(&mut c, "nano", 91);
    let cmds = c.commands(&e, 3.0);
    assert!(run_all(&mut e, &mut h, cmds).is_empty());
    assert_eq!(e.effect_theta(e.effects()[0].id), Some(0.0));

    // Wheel 1 of the effect layout is Size: through FX-17, nothing new on the device side.
    let before = e.effects()[0].params.size;
    packet(&mut c, "nano", &[0xB0, 16, 5]);
    let cmds = c.commands(&e, 3.0);
    assert!(run_all(&mut e, &mut h, cmds).is_empty());
    assert!(e.effects()[0].params.size > before);

    press(&mut c, "nano", 92);
    let cmds = c.commands(&e, 3.0);
    assert!(run_all(&mut e, &mut h, cmds).is_empty());
    assert!(e.effects().is_empty());
}
