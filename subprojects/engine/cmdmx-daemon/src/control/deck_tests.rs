//! VS7-06: Stream Decks on the tick (CTL-44, 46..49, 52..54, 56, 57). The deck thread is
//! simulated with `DeckEvent`s; what the tick asks of it comes back from `deck_out`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use std::fs;
use std::path::PathBuf;

use cmdmx_control::deck_image::{KeyLook, KeyState};
use cmdmx_core::engine::Engine;
use cmdmx_core::playback::HandleButton;
use serde_json::{json, Value};

use super::Control;
use crate::deck_io::{DeckCmd, DeckEvent, DeckInfo, DeckState};
use crate::ipc::{self, Command};

const DECK: &str = "Stream Deck Original (A1B2)";
const ORIGINAL: u16 = 0x0060;

fn setup(test: &str) -> (Control, PathBuf) {
    let dir = std::env::temp_dir().join(format!("cmdmx-deck-{test}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("control")).unwrap();
    let (c, warnings) = Control::load(dir.join("control"), dir.join("state.json"));
    assert!(warnings.is_empty(), "{warnings:?}");
    (c, dir)
}

fn decks(c: &mut Control, list: &[(&str, u16)]) {
    let list = list
        .iter()
        .map(|(n, pid)| DeckInfo {
            name: n.to_string(),
            product_id: *pid,
        })
        .collect();
    c.deck_event(DeckEvent::Decks(list), 0.0);
}

fn state(c: &mut Control, s: DeckState) {
    c.deck_event(
        DeckEvent::State {
            name: DECK.into(),
            state: s,
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

fn enable(c: &mut Control, on: bool) {
    let j = json!({"v":0,"cmd":"control.enable","device":DECK,"on":on}).to_string();
    req(c, &j).unwrap();
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

/// An Original key press or release as the V1 report has it: rows mirrored (CTL-47).
fn key(c: &mut Control, n: u16, down: bool) {
    let p = usize::from(n - 1);
    let col = p % 5;
    let device = p - col + (4 - col);
    let mut bytes = vec![0u8; 17];
    bytes[0] = 0x01;
    bytes[1 + device] = u8::from(down);
    c.deck_event(
        DeckEvent::Report {
            name: DECK.into(),
            bytes,
        },
        0.0,
    );
}

fn press(c: &mut Control, n: u16) {
    key(c, n, true);
    key(c, n, false);
}

/// An Original that is connected, enabled and open (Ok), with its first deck_out drained.
fn ready(test: &str, e: &Engine) -> (Control, PathBuf) {
    let (mut c, dir) = setup(test);
    decks(&mut c, &[(DECK, ORIGINAL)]);
    enable(&mut c, true);
    state(&mut c, DeckState::Ok);
    c.deck_out(e);
    (c, dir)
}

fn looks(cmds: &[DeckCmd]) -> Vec<(u16, KeyLook)> {
    cmds.iter()
        .filter_map(|c| match c {
            DeckCmd::Looks { name, keys } if name == DECK => Some(keys.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn look_of(cmds: &[DeckCmd], key: u16) -> Option<KeyLook> {
    looks(cmds)
        .into_iter()
        .rev()
        .find(|(k, _)| *k == key)
        .map(|(_, l)| l)
}

fn slot(engine: &Engine, page: u16, slot: u16) -> u32 {
    engine
        .playbacks()
        .find(|p| (p.page, p.slot) == (page, slot))
        .unwrap()
        .id
}

// --- CTL-44 listed as a device ----------------------------------------------

#[test]
fn ctl_44_a_deck_is_a_device_of_kind_streamdeck_and_starts_disabled() {
    let (mut c, _) = setup("listed");
    decks(&mut c, &[(DECK, ORIGINAL)]);
    let s = c.snapshot();
    let d = device(&s, DECK);
    assert_eq!(d["kind"], "streamdeck");
    assert_eq!(d["model"], "Stream Deck Original");
    assert_eq!(d["state"], "Disabled");
    assert_eq!(d["enabled"], false);
    // Not opened: the tick asks nothing of the deck thread (CTL-46).
    assert!(c.deck_out(&engine()).is_empty());
}

// --- CTL-45 Unsupported ------------------------------------------------------

#[test]
fn ctl_45_an_unknown_elgato_product_is_unsupported_with_one_warning_and_never_opened() {
    let (mut c, _) = setup("unsupported");
    let name = "Elgato 0x0fff (0042)";
    decks(&mut c, &[(name, 0x0fff)]);
    decks(&mut c, &[(name, 0x0fff)]);
    assert_eq!(device(&c.snapshot(), name)["state"], "Unsupported");
    let msgs = c.take_messages();
    assert_eq!(msgs.len(), 1, "{msgs:?}");
    assert!(
        msgs[0].contains(name) && msgs[0].contains("not supported"),
        "{msgs:?}"
    );
    let j = json!({"v":0,"cmd":"control.enable","device":name,"on":true}).to_string();
    assert!(req(&mut c, &j).is_err());
    assert!(c.deck_out(&engine()).is_empty());
}

// --- CTL-46 open when enabled, Busy, close when disabled ---------------------

#[test]
fn ctl_46_enabling_opens_busy_warns_once_and_disabling_closes() {
    let (mut c, _) = setup("busy");
    let e = engine();
    decks(&mut c, &[(DECK, ORIGINAL)]);
    enable(&mut c, true);
    let out = c.deck_out(&e);
    assert!(
        out.iter()
            .any(|o| matches!(o, DeckCmd::Open { name, brightness: 70 } if name == DECK)),
        "{out:?}"
    );
    // Not open yet: no images go out.
    assert!(looks(&out).is_empty());
    state(&mut c, DeckState::Busy);
    assert_eq!(device(&c.snapshot(), DECK)["state"], "Busy");
    state(&mut c, DeckState::Busy);
    let msgs = c.take_messages();
    assert_eq!(
        msgs,
        [format!(
            "{DECK} is in use by another app; quit the Elgato Stream Deck app"
        )]
    );
    state(&mut c, DeckState::Ok);
    assert_eq!(device(&c.snapshot(), DECK)["state"], "Ok");
    enable(&mut c, false);
    let out = c.deck_out(&e);
    assert!(
        out.iter()
            .any(|o| matches!(o, DeckCmd::Close { name } if name == DECK)),
        "{out:?}"
    );
    assert_eq!(device(&c.snapshot(), DECK)["state"], "Disabled");
}

// --- CTL-49 default profile ----------------------------------------------------

#[test]
fn ctl_49_an_enabled_deck_gets_the_generated_profile() {
    let e = engine();
    let (c, _) = ready("profile", &e);
    let s = c.snapshot();
    assert_eq!(device(&s, DECK)["mapping"], "Stream Deck Original");
    let m = s["mappings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == "Stream Deck Original")
        .unwrap()
        .clone();
    assert_eq!(m["profile"], true);
    let reply: Value = serde_json::from_str(&c.mapping_reply("Stream Deck Original")).unwrap();
    let bindings = reply["mapping"]["bindings"].as_array().unwrap();
    let action = |n: u16| {
        bindings
            .iter()
            .find(|b| b["input"] == json!({"kind":"key","number":n}))
            .map(|b| b["action"].as_str().unwrap().to_string())
    };
    let fpp = e.settings().faders_per_page;
    assert_eq!(action(1).as_deref(), Some("upper 1"));
    assert_eq!(action(15).as_deref(), Some("go"));
    assert_eq!(action(13).as_deref(), Some("page prev"));
    let upper = (1..=12).filter(|n| action(*n).is_some()).count();
    assert_eq!(upper, usize::from(fpp.min(12)));
}

// --- CTL-47/48 inputs to commands, deck pages ----------------------------------

#[test]
fn ctl_47_a_key_press_runs_its_action_as_a_command() {
    let e = engine();
    let (mut c, _) = ready("press", &e);
    key(&mut c, 1, true);
    let cmds: Vec<Command> = c.commands(&e, 1.0).into_iter().map(|(_, c)| c).collect();
    assert_eq!(
        cmds,
        [Command::PlaybackButton {
            id: slot(&e, 1, 1),
            button: HandleButton::Upper,
            on: true,
            at: 1.0
        }]
    );
    key(&mut c, 1, false);
    let cmds: Vec<(String, Command)> = c.commands(&e, 2.0);
    assert!(matches!(
        cmds[0].1,
        Command::PlaybackButton { on: false, .. }
    ));
    assert_eq!(cmds[0].0, DECK);
}

#[test]
fn ctl_47_a_disabled_deck_does_nothing() {
    let e = engine();
    let (mut c, _) = ready("disabled", &e);
    enable(&mut c, false);
    press(&mut c, 15);
    assert!(c.commands(&e, 0.0).is_empty());
}

#[test]
fn ctl_48_deck_page_keys_switch_only_the_deck_and_send_that_page() {
    let e = engine();
    let (mut c, dir) = setup("pages");
    fs::write(
        dir.join("control/mine.json"),
        r#"{"name":"mine","device":"Stream Deck Original*","bindings":[
          {"input":{"kind":"key","number":1},"action":"deck page next"},
          {"input":{"kind":"key","number":2},"action":"go"},
          {"input":{"kind":"key","number":1},"action":"deck page prev","deck_page":2},
          {"input":{"kind":"key","number":2},"action":"page 3","deck_page":2,"label":"Three"}
        ]}"#,
    )
    .unwrap();
    let (mut c2, _) = Control::load(dir.join("control"), dir.join("state.json"));
    std::mem::swap(&mut c, &mut c2);
    decks(&mut c, &[(DECK, ORIGINAL)]);
    enable(&mut c, true);
    req(
        &mut c,
        &json!({"v":0,"cmd":"control.assign","device":DECK,"mapping":"mine"}).to_string(),
    )
    .unwrap();
    state(&mut c, DeckState::Ok);
    c.deck_out(&e);
    assert_eq!(device(&c.snapshot(), DECK)["deck_pages"], 2);
    press(&mut c, 1);
    // No engine command for a deck page.
    assert!(c.commands(&e, 0.0).is_empty());
    assert_eq!(device(&c.snapshot(), DECK)["deck_page"], 2);
    // The new page goes out in full: every key, unbound ones black.
    let out = c.deck_out(&e);
    let keys: Vec<u16> = looks(&out).iter().map(|(k, _)| *k).collect();
    for k in 1..=15 {
        assert!(
            keys.contains(&k),
            "key {k} not sent after the deck page change"
        );
    }
    assert_eq!(look_of(&out, 2).unwrap().label, "Three");
    assert_eq!(look_of(&out, 7).unwrap(), KeyLook::black());
    // Key 2 now does page 3.
    press(&mut c, 2);
    let cmds: Vec<Command> = c.commands(&e, 0.0).into_iter().map(|(_, c)| c).collect();
    assert_eq!(cmds, [Command::PageSet { page: 3 }]);
}

// --- CTL-52 image feedback timing ----------------------------------------------

#[test]
fn ctl_52_looks_go_out_once_then_only_when_the_level_bar_moves() {
    let mut e = engine();
    let (mut c, _) = setup("timing");
    decks(&mut c, &[(DECK, ORIGINAL)]);
    enable(&mut c, true);
    state(&mut c, DeckState::Ok);
    let first = c.deck_out(&e);
    let l1 = look_of(&first, 1).unwrap();
    assert_eq!(l1.label, "PB 1");
    assert_eq!(l1.state, KeyState::Inactive);
    assert_eq!(l1.level, Some(0.0));
    assert!(c.deck_out(&e).is_empty(), "nothing changed, nothing sent");
    let id = slot(&e, 1, 1);
    e.set_fader(id, 0.5, 0.0).unwrap();
    e.tick(0.0);
    let out = c.deck_out(&e);
    let l = look_of(&out, 1).unwrap();
    assert_eq!(l.state, KeyState::Active);
    assert_eq!(l.level, Some(0.5));
    assert_eq!(looks(&out).len(), 1, "only key 1 changed");
    // 72 px bar: 0.503 is the same pixel as 0.5.
    e.set_fader(id, 0.503, 0.1).unwrap();
    e.tick(0.1);
    assert!(look_of(&c.deck_out(&e), 1).is_none());
}

// --- CTL-53 brightness -----------------------------------------------------------

#[test]
fn ctl_53_brightness_is_set_kept_in_state_json_and_refused_out_of_range() {
    let e = engine();
    let (mut c, dir) = ready("brightness", &e);
    assert_eq!(device(&c.snapshot(), DECK)["brightness"], 70);
    req(
        &mut c,
        &json!({"v":0,"cmd":"control.deck","device":DECK,"brightness":40}).to_string(),
    )
    .unwrap();
    let out = c.deck_out(&e);
    assert!(
        out.iter()
            .any(|o| matches!(o, DeckCmd::Brightness { name, percent: 40 } if name == DECK)),
        "{out:?}"
    );
    assert_eq!(device(&c.snapshot(), DECK)["brightness"], 40);
    let saved: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("state.json")).unwrap()).unwrap();
    assert_eq!(saved["control"]["devices"][DECK]["brightness"], 40);
    // A new run opens it at the kept brightness.
    let (mut again, _) = Control::load(dir.join("control"), dir.join("state.json"));
    decks(&mut again, &[(DECK, ORIGINAL)]);
    let out = again.deck_out(&e);
    assert!(
        out.iter()
            .any(|o| matches!(o, DeckCmd::Open { name, brightness: 40 } if name == DECK)),
        "{out:?}"
    );
    let bad = json!({"v":0,"cmd":"control.deck","device":DECK,"brightness":101}).to_string();
    assert!(req(&mut c, &bad).is_err());
    let bad = json!({"v":0,"cmd":"control.deck","device":"nope","brightness":50}).to_string();
    assert!(req(&mut c, &bad).is_err());
}

// --- CTL-54 unplug and back ---------------------------------------------------------

#[test]
fn ctl_54_unplugged_is_missing_and_back_it_is_opened_with_its_full_images() {
    let e = engine();
    let (mut c, _) = ready("unplug", &e);
    decks(&mut c, &[]);
    let s = c.snapshot();
    assert_eq!(device(&s, DECK)["state"], "Missing");
    assert_eq!(device(&s, DECK)["mapping"], "Stream Deck Original");
    assert_eq!(c.take_messages(), [format!("{DECK} disconnected")]);
    decks(&mut c, &[(DECK, ORIGINAL)]);
    let out = c.deck_out(&e);
    assert!(
        out.iter()
            .any(|o| matches!(o, DeckCmd::Open { name, .. } if name == DECK)),
        "{out:?}"
    );
    state(&mut c, DeckState::Ok);
    let keys: Vec<u16> = looks(&c.deck_out(&e)).iter().map(|(k, _)| *k).collect();
    for k in 1..=15 {
        assert!(keys.contains(&k), "key {k} not sent again");
    }
}

#[test]
fn ctl_54_a_deck_that_failed_a_write_gets_its_full_images_when_it_is_ok_again() {
    let e = engine();
    let (mut c, _) = ready("failed", &e);
    state(&mut c, DeckState::Missing);
    assert_eq!(device(&c.snapshot(), DECK)["state"], "Missing");
    assert!(looks(&c.deck_out(&e)).is_empty(), "no images while Missing");
    state(&mut c, DeckState::Ok);
    assert_eq!(looks(&c.deck_out(&e)).len(), 15);
}

// --- CTL-56 Learn ----------------------------------------------------------------------

#[test]
fn ctl_56_a_key_press_learns_at_once_on_the_current_deck_page() {
    let e = engine();
    let (mut c, dir) = ready("learn", &e);
    req(
        &mut c,
        r#"{"v":0,"cmd":"control.learn","action":"upper 3"}"#,
    )
    .unwrap();
    press(&mut c, 14);
    assert!(
        c.commands(&e, 0.0).is_empty(),
        "a learning press does not act"
    );
    let msgs = c.take_messages();
    assert_eq!(msgs[0], format!("Learned: {DECK} key 14 → upper 3"));
    assert_eq!(msgs[1], "key 14 was page next");
    assert!(c.snapshot()["learn"].is_null());
    // Learning edits the profile: it becomes a user copy (CTL-30).
    let file = dir.join("control/Stream Deck Original.json");
    let saved: Value = serde_json::from_str(&fs::read_to_string(file).unwrap()).unwrap();
    let b = saved["bindings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["input"] == json!({"kind":"key","number":14}))
        .unwrap()
        .clone();
    assert_eq!(b["action"], "upper 3");
    assert!(b.get("deck_page").is_none(), "page 1 is the default");
    press(&mut c, 14);
    let cmds: Vec<Command> = c.commands(&e, 0.0).into_iter().map(|(_, c)| c).collect();
    assert!(matches!(
        cmds[0],
        Command::PlaybackButton {
            button: HandleButton::Upper,
            on: true,
            ..
        }
    ));
}

#[test]
fn ctl_56_learning_on_deck_page_2_binds_on_page_2() {
    let e = engine();
    let (mut c, dir) = setup("learn2");
    fs::write(
        dir.join("control/mine.json"),
        r#"{"name":"mine","device":"Stream Deck Original*","bindings":[
          {"input":{"kind":"key","number":1},"action":"deck page 2"},
          {"input":{"kind":"key","number":1},"action":"deck page 1","deck_page":2},
          {"input":{"kind":"key","number":5},"action":"go"}
        ]}"#,
    )
    .unwrap();
    let (mut c2, _) = Control::load(dir.join("control"), dir.join("state.json"));
    std::mem::swap(&mut c, &mut c2);
    decks(&mut c, &[(DECK, ORIGINAL)]);
    enable(&mut c, true);
    req(
        &mut c,
        &json!({"v":0,"cmd":"control.assign","device":DECK,"mapping":"mine"}).to_string(),
    )
    .unwrap();
    state(&mut c, DeckState::Ok);
    press(&mut c, 1);
    c.commands(&e, 0.0);
    req(&mut c, r#"{"v":0,"cmd":"control.learn","action":"dbo"}"#).unwrap();
    press(&mut c, 5);
    let msgs = c.take_messages();
    assert_eq!(
        msgs,
        [format!("Learned: {DECK} key 5 → dbo")],
        "page 1's go is not replaced"
    );
    let reply: Value = serde_json::from_str(&c.mapping_reply("mine")).unwrap();
    let five: Vec<&Value> = reply["mapping"]["bindings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|b| b["input"] == json!({"kind":"key","number":5}))
        .collect();
    assert_eq!(five.len(), 2);
    assert!(five
        .iter()
        .any(|b| b["action"] == "go" && b.get("deck_page").is_none()));
    assert!(five
        .iter()
        .any(|b| b["action"] == "dbo" && b["deck_page"] == 2));
}

// --- CTL-57 snapshot --------------------------------------------------------------------

#[test]
fn ctl_57_the_snapshot_describes_the_deck_and_its_keys() {
    let e = engine();
    let (c, _) = ready("snapshot", &e);
    let s = c.snapshot();
    let d = device(&s, DECK);
    assert_eq!(d["kind"], "streamdeck");
    assert_eq!(d["model"], "Stream Deck Original");
    assert_eq!(d["verified"], false);
    assert_eq!(d["cols"], 5);
    assert_eq!(d["rows"], 3);
    assert_eq!(
        d["extras"],
        json!({"dials": 0, "touch_keys": 0, "strip": null, "info": null})
    );
    assert_eq!(d["deck_page"], 1);
    assert_eq!(d["deck_pages"], 1);
    assert_eq!(d["brightness"], 70);
    assert_eq!(d["state"], "Ok");
    let keys = d["keys"].as_array().unwrap();
    assert_eq!(keys.len(), 15, "every key of the page");
    // `sub` is the upper button's function as CUE-25 sets it (engine.handle_buttons); `halo`
    // is PBK-33's (`#rrggbb` or null).
    let first = &keys[0];
    let mut fields: Vec<&str> = first
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    fields.sort_unstable();
    assert_eq!(fields, ["halo", "key", "label", "level", "state", "sub"]);
    assert!(!first["sub"].as_str().unwrap().is_empty());
    assert!(first["halo"].is_null() || first["halo"].as_str().unwrap().len() == 7);
    assert_eq!(first["key"], 1);
    assert_eq!(first["label"], "PB 1");
    assert_eq!(first["level"], 0.0);
    assert_eq!(first["state"], "inactive");
    let go = keys.iter().find(|k| k["key"] == 15).unwrap();
    assert_eq!(go["label"], "Go");
    // An unbound key is black.
    let off = keys.iter().find(|k| k["key"] == 12).unwrap();
    assert_eq!(off["state"], "off");
}

#[test]
fn ctl_57_a_midi_device_has_no_deck_fields() {
    let (mut c, _) = setup("midi");
    c.event(
        super::MidiEvent::Ports(vec![super::PortInfo {
            name: "nano".into(),
            input: true,
            output: true,
        }]),
        0.0,
    );
    let s = c.snapshot();
    let d = device(&s, "nano");
    assert_eq!(d["kind"], "midi");
    assert!(d.get("keys").is_none());
}

#[test]
fn ctl_57_changed_looks_reach_the_clients_at_most_ten_times_a_second() {
    let mut e = engine();
    let (mut c, _) = ready("delta", &e);
    c.delta(0.0);
    let id = slot(&e, 1, 1);
    let mut sent = 0;
    for i in 0..20 {
        let now = f64::from(i) * 0.025;
        e.set_fader(id, (i as f32 + 1.0) / 20.0, now).unwrap();
        e.tick(now);
        c.deck_out(&e);
        if c.delta(now).is_some() {
            sent += 1;
        }
    }
    // 0.5 s of changes every 25 ms: about 5 deltas, never 20.
    assert!((4..=6).contains(&sent), "{sent} deltas");
}

#[test]
fn ctl_44_a_midi_port_list_leaves_the_decks_alone() {
    let e = engine();
    let (mut c, _) = ready("midi-ports", &e);
    c.event(super::MidiEvent::Ports(Vec::new()), 0.0);
    let s = c.snapshot();
    assert_eq!(device(&s, DECK)["state"], "Ok");
    assert!(c.take_messages().is_empty());
    // A deck seen but not enabled is not forgotten either.
    let (mut c, _) = setup("midi-ports-2");
    decks(&mut c, &[(DECK, ORIGINAL)]);
    c.event(super::MidiEvent::Ports(Vec::new()), 0.0);
    assert_eq!(device(&c.snapshot(), DECK)["state"], "Disabled");
}

// --- CTL-67 IPC for the on-screen deck ------------------------------------------------------

/// An Original, enabled, Ok, on mapping `mine` with key 5 bound on deck pages 1 and 2.
fn two_pages(test: &str, e: &Engine) -> (Control, PathBuf) {
    let (mut c, dir) = setup(test);
    fs::write(
        dir.join("control/mine.json"),
        r#"{"name":"mine","device":"Stream Deck Original*","bindings":[
          {"input":{"kind":"key","number":5},"action":"go"},
          {"input":{"kind":"key","number":5},"action":"dbo","deck_page":2},
          {"input":{"kind":"key","number":1},"action":"stop","deck_page":3}
        ]}"#,
    )
    .unwrap();
    let (mut c2, _) = Control::load(dir.join("control"), dir.join("state.json"));
    std::mem::swap(&mut c, &mut c2);
    decks(&mut c, &[(DECK, ORIGINAL)]);
    enable(&mut c, true);
    req(
        &mut c,
        &json!({"v":0,"cmd":"control.assign","device":DECK,"mapping":"mine"}).to_string(),
    )
    .unwrap();
    state(&mut c, DeckState::Ok);
    c.deck_out(e);
    (c, dir)
}

fn bindings_of(c: &Control, key: u16) -> Vec<Value> {
    let reply: Value = serde_json::from_str(&c.mapping_reply("mine")).unwrap();
    reply["mapping"]["bindings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|b| b["input"] == json!({"kind":"key","number":key}))
        .cloned()
        .collect()
}

#[test]
fn ctl_67_control_deck_page_sets_the_deck_page_and_sends_it_in_full() {
    let e = engine();
    let (mut c, _) = two_pages("setpage", &e);
    assert_eq!(device(&c.snapshot(), DECK)["deck_page"], 1);
    req(
        &mut c,
        &json!({"v":0,"cmd":"control.deck","device":DECK,"page":2}).to_string(),
    )
    .unwrap();
    assert_eq!(device(&c.snapshot(), DECK)["deck_page"], 2);
    let out = c.deck_out(&e);
    let keys: Vec<u16> = looks(&out).iter().map(|(k, _)| *k).collect();
    for k in 1..=15 {
        assert!(
            keys.contains(&k),
            "key {k} not sent after control.deck page"
        );
    }
    // The brightness is left alone when only the page is given.
    assert_eq!(device(&c.snapshot(), DECK)["brightness"], 70);
    let err = req(
        &mut c,
        &json!({"v":0,"cmd":"control.deck","device":DECK,"page":4}).to_string(),
    )
    .unwrap_err();
    assert_eq!(err, "deck page 4 outside 1..=3");
    assert!(req(
        &mut c,
        &json!({"v":0,"cmd":"control.deck","device":DECK,"page":0}).to_string(),
    )
    .is_err());
    assert_eq!(device(&c.snapshot(), DECK)["deck_page"], 2);
    // Brightness alone still works; neither field is refused.
    req(
        &mut c,
        &json!({"v":0,"cmd":"control.deck","device":DECK,"brightness":30}).to_string(),
    )
    .unwrap();
    assert_eq!(device(&c.snapshot(), DECK)["brightness"], 30);
    assert_eq!(device(&c.snapshot(), DECK)["deck_page"], 2);
    assert!(req(
        &mut c,
        &json!({"v":0,"cmd":"control.deck","device":DECK}).to_string()
    )
    .is_err());
}

#[test]
fn ctl_67_binding_and_unbind_pick_the_binding_on_its_deck_page() {
    let e = engine();
    let (mut c, _) = two_pages("perpage", &e);
    let five = json!({"kind":"key","number":5});
    // Edit page 2's key 5: page 1's stays as it is.
    req(
        &mut c,
        &json!({"v":0,"cmd":"control.binding","mapping":"mine","input":five,"deck_page":2,"label":"Dark"})
            .to_string(),
    )
    .unwrap();
    let b = bindings_of(&c, 5);
    assert_eq!(b.len(), 2, "{b:?}");
    assert!(b
        .iter()
        .any(|b| b["action"] == "dbo" && b["deck_page"] == 2 && b["label"] == "Dark"));
    assert!(b
        .iter()
        .any(|b| b["action"] == "go" && b.get("deck_page").is_none() && b.get("label").is_none()));
    // A key on a page without a binding there needs an action, then it is added.
    let six = json!({"kind":"key","number":6});
    assert!(req(
        &mut c,
        &json!({"v":0,"cmd":"control.binding","mapping":"mine","input":six,"deck_page":2,"label":"x"})
            .to_string(),
    )
    .is_err());
    req(
        &mut c,
        &json!({"v":0,"cmd":"control.binding","mapping":"mine","input":six,"deck_page":2,"action":"go"})
            .to_string(),
    )
    .unwrap();
    assert_eq!(bindings_of(&c, 6).len(), 1);
    assert_eq!(bindings_of(&c, 6)[0]["deck_page"], 2);
    // Unbind with a deck page removes only that page's binding; without one, page 1's.
    req(
        &mut c,
        &json!({"v":0,"cmd":"control.unbind","mapping":"mine","input":five,"deck_page":2})
            .to_string(),
    )
    .unwrap();
    let b = bindings_of(&c, 5);
    assert_eq!(b.len(), 1);
    assert_eq!(b[0]["action"], "go");
    assert!(req(
        &mut c,
        &json!({"v":0,"cmd":"control.unbind","mapping":"mine","input":five,"deck_page":2})
            .to_string(),
    )
    .is_err());
    req(
        &mut c,
        &json!({"v":0,"cmd":"control.unbind","mapping":"mine","input":five}).to_string(),
    )
    .unwrap();
    assert!(bindings_of(&c, 5).is_empty());
}

// --- VS7-07 run amendments (CTL-46, 47, 50, 57) ------------------------------------------------

#[test]
fn ctl_46_busy_warns_again_after_disable_and_enable() {
    let (mut c, _) = setup("busy_again");
    decks(&mut c, &[(DECK, ORIGINAL)]);
    enable(&mut c, true);
    state(&mut c, DeckState::Busy);
    assert_eq!(c.take_messages().len(), 1);
    enable(&mut c, false);
    enable(&mut c, true);
    state(&mut c, DeckState::Busy);
    assert_eq!(
        c.take_messages(),
        [format!(
            "{DECK} is in use by another app; quit the Elgato Stream Deck app"
        )]
    );
}

#[test]
fn ctl_47_a_continuous_action_on_a_key_only_shows() {
    let e = engine();
    let (mut c, dir) = setup("gm_key");
    fs::write(
        dir.join("control/mine.json"),
        r#"{"name":"mine","device":"Stream Deck Original*","bindings":[
          {"input":{"kind":"key","number":1},"action":"gm"},
          {"input":{"kind":"key","number":2},"action":"fader 1"},
          {"input":{"kind":"key","number":3},"action":"go"}
        ]}"#,
    )
    .unwrap();
    let (mut c2, _) = Control::load(dir.join("control"), dir.join("state.json"));
    std::mem::swap(&mut c, &mut c2);
    decks(&mut c, &[(DECK, ORIGINAL)]);
    enable(&mut c, true);
    req(
        &mut c,
        &json!({"v":0,"cmd":"control.assign","device":DECK,"mapping":"mine"}).to_string(),
    )
    .unwrap();
    state(&mut c, DeckState::Ok);
    let out = c.deck_out(&e);
    assert_eq!(
        look_of(&out, 1).unwrap().level,
        Some(1.0),
        "the GM level shows"
    );
    press(&mut c, 1);
    press(&mut c, 2);
    assert!(
        c.commands(&e, 0.0).is_empty(),
        "no GM or fader command from a key"
    );
    press(&mut c, 3);
    assert!(
        !c.commands(&e, 0.0).is_empty(),
        "a button action still runs"
    );
}

#[test]
fn ctl_57_each_key_says_whether_it_is_fixed() {
    let e = engine();
    let (mut c, _) = ready("fixed", &e);
    req(
        &mut c,
        &json!({"v":0,"cmd":"control.binding","mapping":"Stream Deck Original","input":{"kind":"key","number":9},"action":"upper 1.4"})
            .to_string(),
    )
    .unwrap();
    let d = device(&c.snapshot(), DECK).clone();
    let key = |n: u64| {
        d["keys"]
            .as_array()
            .unwrap()
            .iter()
            .find(|k| k["key"] == n)
            .cloned()
            .unwrap()
    };
    assert_eq!(key(9)["fixed"], true);
    assert_eq!(key(1)["fixed"], false);
}
