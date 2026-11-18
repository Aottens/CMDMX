//! Blind over the wire (BLD-02, IPC-67 Blind part; PRG-105..109) on the real binary, as
//! `lock.rs` runs it: a temp `HOME` and `--show-dir`, every output off.
//!
//! Wire this fixes:
//! - `programmer.blind {"on":true}` and `{"on":false,"fade":2.0,"clear":false}`; `fade` and `clear`
//!   optional. Refused while locked. Not an undo step (PRG-105, PRG-107).
//! - The snapshot has `blind` (bool); a change is `{"type":"blind","blind":b}` to every client,
//!   also when New or Open ends Blind (PRG-109).
//! - A stored change PRG-108 guards, sent without `"confirm":true` in Blind, is answered with
//!   `{"type":"on_stage","message":<the sentence>}`, not a warning, and changes nothing.

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tungstenite::{Message, WebSocket};

/// Kills the daemon when the test ends, passed or not.
struct Daemon(Child);

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn folder(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cmdmx-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn daemon(dir: &Path, port: u16) -> Daemon {
    Daemon(
        Command::new(env!("CARGO_BIN_EXE_cmdmx-daemon"))
            .env("HOME", dir)
            .args([
                "--no-artnet",
                "--no-sacn",
                "--no-serial",
                "--ipc-port",
                &port.to_string(),
            ])
            .arg("--show-dir")
            .arg(dir.join("Shows"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

fn connect(port: u16) -> WebSocket<TcpStream> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(stream) = TcpStream::connect(("127.0.0.1", port)) {
            stream
                .set_read_timeout(Some(Duration::from_secs(15)))
                .unwrap();
            return tungstenite::client(format!("ws://127.0.0.1:{port}/"), stream)
                .unwrap()
                .0;
        }
        assert!(Instant::now() < deadline, "daemon never listened on {port}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn recv(ws: &mut WebSocket<TcpStream>) -> Value {
    loop {
        if let Message::Text(t) = ws.read().unwrap() {
            return serde_json::from_str(&t).unwrap();
        }
    }
}

fn send(ws: &mut WebSocket<TcpStream>, cmd: Value) {
    let mut cmd = cmd;
    cmd["v"] = json!(1);
    ws.send(Message::Text(cmd.to_string().into())).unwrap();
}

fn until(ws: &mut WebSocket<TcpStream>, what: &str, pred: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(Instant::now() < deadline, "never saw {what}");
        let m = recv(ws);
        if pred(&m) {
            return m;
        }
    }
}

/// A new client's snapshot: what the daemon holds now.
fn fresh_snapshot(port: u16) -> Value {
    let mut ws = connect(port);
    let s = recv(&mut ws);
    assert_eq!(s["type"], "snapshot");
    s
}

fn blind(ws: &mut WebSocket<TcpStream>, on: bool) {
    send(ws, json!({"cmd": "programmer.blind", "on": on}));
}

fn saw_blind(ws: &mut WebSocket<TcpStream>, on: bool) {
    until(ws, "the blind delta", |m| {
        m["type"] == "blind" && m["blind"] == on
    });
}

/// Fixture 4 at Dimmer 0.5 recorded on page 9 slot 1 (free in the demo show), its fader up;
/// the programmer emptied. Returns the playback id.
fn live_cue(app: &mut WebSocket<TcpStream>) -> u64 {
    send(app, json!({"cmd": "programmer.select", "fixture": 4}));
    send(
        app,
        json!({"cmd": "programmer.set", "attribute": "Dimmer", "value": 0.5}),
    );
    send(app, json!({"cmd": "playback.record", "page": 9, "slot": 1}));
    let on_9_1 = |p: &Value| p["page"] == 9 && p["slot"] == 1;
    let stored = until(app, "the recorded playback", |m| {
        m["playbacks"]
            .as_array()
            .is_some_and(|p| p.iter().any(on_9_1))
    });
    let id = stored["playbacks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| on_9_1(p))
        .unwrap()["id"]
        .as_u64()
        .unwrap();
    send(
        app,
        json!({"cmd": "fader.set", "playback": id, "level": 1.0}),
    );
    id
}

/// Fixture 4's stored Dimmer in cue 1 of `id` (IPC-24).
fn cue_dimmer(app: &mut WebSocket<TcpStream>, id: u64) -> Value {
    send(app, json!({"cmd": "cue.view", "id": id, "cue": 1}));
    let view = until(app, "the cue view", |m| m["type"] == "cue.view");
    let row = view["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["fixture"] == 4)
        .cloned()
        .unwrap_or_else(|| panic!("no fixture 4 in {view}"));
    row["values"]["Dimmer"]["value"].clone()
}

#[test]
fn ipc_67_blind_is_in_the_snapshot_and_a_delta_to_every_client() {
    let dir = folder("ipc67-delta");
    let port = free_port();
    let _daemon = daemon(&dir, port);
    let mut first = connect(port);
    let mut second = connect(port);
    assert_eq!(recv(&mut first)["blind"], false);
    assert_eq!(recv(&mut second)["blind"], false);

    blind(&mut first, true);
    saw_blind(&mut first, true);
    saw_blind(&mut second, true);
    assert_eq!(fresh_snapshot(port)["blind"], true);

    send(
        &mut second,
        json!({"cmd": "programmer.blind", "on": false, "fade": 2.0, "clear": true}),
    );
    saw_blind(&mut first, false);
    saw_blind(&mut second, false);
    assert_eq!(fresh_snapshot(port)["blind"], false);

    // A bad fade is a warning and Blind stays.
    blind(&mut first, true);
    saw_blind(&mut first, true);
    send(
        &mut first,
        json!({"cmd": "programmer.blind", "on": false, "fade": -1.0}),
    );
    until(&mut first, "a warning", |m| m["type"] == "warning");
    assert_eq!(fresh_snapshot(port)["blind"], true);
}

#[test]
fn ipc_67_a_stored_change_on_stage_gets_an_on_stage_reply_then_confirm_does_it() {
    let dir = folder("ipc67-on-stage");
    let port = free_port();
    let _daemon = daemon(&dir, port);
    let mut app = connect(port);
    recv(&mut app);
    let id = live_cue(&mut app);
    blind(&mut app, true);
    saw_blind(&mut app, true);
    send(
        &mut app,
        json!({"cmd": "programmer.set", "attribute": "Dimmer", "value": 1.0}),
    );
    let merge = json!({"cmd": "playback.record", "page": 9, "slot": 1, "mode": "Merge"});
    send(&mut app, merge.clone());
    let reply = until(&mut app, "the on_stage reply", |m| {
        m["type"] == "on_stage" || m["type"] == "warning"
    });
    assert_eq!(reply["type"], "on_stage", "{reply}");
    let sentence = reply["message"].as_str().unwrap();
    assert!(
        sentence.contains(&format!("live on playback {id}")),
        "{sentence}"
    );
    assert_eq!(cue_dimmer(&mut app, id), json!(0.5), "nothing changed");

    let mut confirmed = merge;
    confirmed["confirm"] = json!(true);
    send(&mut app, confirmed);
    let deadline = Instant::now() + Duration::from_secs(10);
    while cue_dimmer(&mut app, id) != json!(1.0) {
        assert!(
            Instant::now() < deadline,
            "the confirmed merge never landed"
        );
    }
}

/// Cue 1's fade in on playback `id`, from a fresh snapshot.
fn fade_in(port: u16, id: u64) -> Value {
    let snap = fresh_snapshot(port);
    let p = snap["playbacks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == id)
        .unwrap()
        .clone();
    p["cues"][0]["fade_in"].clone()
}

#[test]
fn prg_108_a_time_change_to_the_live_cue_asks_in_blind_too() {
    let dir = folder("prg108-times");
    let port = free_port();
    let _daemon = daemon(&dir, port);
    let mut app = connect(port);
    recv(&mut app);
    let id = live_cue(&mut app);
    blind(&mut app, true);
    saw_blind(&mut app, true);
    let times = json!({"cmd": "cue.times", "id": id, "cue": 1, "fade_in": 4.0});
    send(&mut app, times.clone());
    let reply = until(&mut app, "the on_stage reply", |m| {
        m["type"] == "on_stage" || m["type"] == "warning"
    });
    assert_eq!(reply["type"], "on_stage", "{reply}");
    assert_ne!(fade_in(port, id), json!(4.0), "nothing changed");

    let mut confirmed = times;
    confirmed["confirm"] = json!(true);
    send(&mut app, confirmed);
    let deadline = Instant::now() + Duration::from_secs(10);
    while fade_in(port, id) != json!(4.0) {
        assert!(Instant::now() < deadline, "the confirmed time never landed");
    }
}

#[test]
fn ipc_67_blind_is_refused_while_locked() {
    let dir = folder("ipc67-locked");
    let port = free_port();
    let _daemon = daemon(&dir, port);
    let mut app = connect(port);
    recv(&mut app);
    send(
        &mut app,
        json!({"cmd": "lock.set", "on": true, "code": "1234"}),
    );
    until(&mut app, "the locked delta", |m| {
        m["type"] == "locked" && m["locked"] == true
    });
    blind(&mut app, true);
    until(&mut app, "the refusal", |m| {
        m["type"] == "warning" && m["message"] == "Console is locked"
    });
    assert_eq!(fresh_snapshot(port)["blind"], false);
}

#[test]
fn ipc_67_prg_109_a_new_show_ends_blind() {
    let dir = folder("ipc67-new");
    let port = free_port();
    let _daemon = daemon(&dir, port);
    let mut app = connect(port);
    recv(&mut app);
    blind(&mut app, true);
    saw_blind(&mut app, true);
    send(&mut app, json!({"cmd": "show.new", "name": "Leeg"}));
    saw_blind(&mut app, false);
    assert_eq!(fresh_snapshot(port)["blind"], false);
}
