//! The message history reaches a later client (IPC-51) and a node's state change is in it
//! (IPC-52): `specs/modules/ipc.md`. Runs the real binary on a temp `HOME` and `--show-dir`.
//! It starts twice (OUT-43): the first run has every output off and only disables the demo's
//! serial node on port "auto" (it would open the first USB-DMX interface); the second, with only
//! serial on, reopens that show and adds a node on a port that is not there, so no frame is sent.

#![cfg(unix)] // `libc` is a unix-only dependency
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::net::{TcpListener, TcpStream};
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

fn connect(port: u16) -> WebSocket<TcpStream> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(stream) = TcpStream::connect(("127.0.0.1", port)) {
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
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

fn until(ws: &mut WebSocket<TcpStream>, what: &str, pred: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(Instant::now() < deadline, "never saw {what}");
        let m = recv(ws);
        if pred(&m) {
            return m;
        }
    }
}

/// SIGTERM, then wait for the exit: REL-14 writes the show file on the way out.
fn sigterm_and_wait(daemon: &mut Child) {
    // SAFETY: kill(2) on our own child's pid.
    let pid = i32::try_from(daemon.id()).unwrap();
    assert_eq!(unsafe { libc::kill(pid, libc::SIGTERM) }, 0);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(s) = daemon.try_wait().unwrap() {
            assert!(s.success(), "daemon exit: {s}");
            return;
        }
        assert!(
            Instant::now() < deadline,
            "daemon did not exit within 5 s of SIGTERM"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Every node of the shows folder after the first run, checked before the second start: a show
/// that did not save, or a second node of the kind, can never send off the Mac (OUT-43).
fn saved_nodes(shows: &std::path::Path) -> Vec<Value> {
    std::fs::read_dir(shows)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.to_string_lossy().ends_with(".cmdmx.json"))
        .map(|p| serde_json::from_str::<Value>(&std::fs::read_to_string(p).unwrap()).unwrap())
        .filter_map(|show| show.pointer("/output/nodes")?.as_array().cloned())
        .flatten()
        .collect()
}

#[test]
fn ipc_51_ipc_52_a_later_client_gets_the_refusal_and_the_node_change_in_its_snapshot() {
    let dir = std::env::temp_dir().join(format!("cmdmx-ipc51-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // First run, every output off: disable the serial node on "auto" and save the show (OUT-43).
    let port = free_port();
    let mut first = Daemon(
        Command::new(env!("CARGO_BIN_EXE_cmdmx-daemon"))
            .env("HOME", &dir)
            .args(["--no-artnet", "--no-sacn", "--no-serial"])
            .args(["--ipc-port", &port.to_string()])
            .arg("--show-dir")
            .arg(dir.join("Shows"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut setup = connect(port);
    let snapshot = recv(&mut setup);
    assert_eq!(snapshot["type"], "snapshot");
    let auto = snapshot["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["kind"] == "serial" && n["port"] == "auto")
        .unwrap_or_else(|| panic!("no serial node on auto in {}", snapshot["outputs"]))["id"]
        .clone();
    let off = json!({"v": 1, "cmd": "output.edit", "id": auto, "enabled": false});
    setup.send(Message::Text(off.to_string().into())).unwrap();
    until(&mut setup, "the serial node disabled", |m| {
        m["outputs"]
            .as_array()
            .is_some_and(|o| o.iter().any(|n| n["id"] == auto && n["enabled"] == false))
    });
    sigterm_and_wait(&mut first.0);
    let saved = saved_nodes(&dir.join("Shows"));
    assert!(saved.iter().any(|n| n["id"] == auto), "{saved:?}");
    for n in saved.iter().filter(|n| n["kind"] == "serial") {
        assert_eq!(n["enabled"], false, "{n}");
    }

    // Second run, only serial on: the "auto" node must still be disabled.
    let port = free_port();
    let _daemon = Daemon(
        Command::new(env!("CARGO_BIN_EXE_cmdmx-daemon"))
            // OUT-43: loopback
            .env("HOME", &dir)
            .args(["--no-artnet", "--no-sacn", "--ipc-port", &port.to_string()])
            .arg("--show-dir")
            .arg(dir.join("Shows"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );

    let mut first = connect(port);
    let snapshot = recv(&mut first);
    assert_eq!(snapshot["type"], "snapshot");
    let reopened = snapshot["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == auto)
        .unwrap_or_else(|| panic!("node {auto} gone from {}", snapshot["outputs"]));
    assert_eq!(reopened["enabled"], false, "{reopened}");
    assert!(
        snapshot["outputs"]
            .as_array()
            .unwrap()
            .iter()
            .all(|n| n["kind"] != "serial" || n["enabled"] == false),
        "an enabled serial node in {}",
        snapshot["outputs"]
    );

    // The same destination twice: the second is refused on the tick's failed-command path.
    let refuse = json!({"v": 1, "cmd": "output.add", "line": 3, "kind": "artnet"});
    for _ in 0..2 {
        first
            .send(Message::Text(refuse.to_string().into()))
            .unwrap();
    }
    let refusal = until(&mut first, "the refusal", |m| m["type"] == "warning");
    assert!(refusal["at"].as_str().unwrap().ends_with('Z'), "{refusal}");

    // A serial node on a port that is not there goes Degraded (OUT-31, IPC-52).
    let missing = "/dev/cu.cmdmx-no-such-port";
    let add = json!({"v": 1, "cmd": "output.add", "line": 7, "kind": "serial", "port": missing});
    first.send(Message::Text(add.to_string().into())).unwrap();
    let change = until(&mut first, "the node change", |m| {
        m["type"] == "warning"
            && m["message"]
                .as_str()
                .is_some_and(|t| t.contains(" universe 7: Degraded: port not found"))
    });

    let mut second = connect(port);
    let snapshot = recv(&mut second);
    assert_eq!(snapshot["type"], "snapshot");
    let history = snapshot["messages"].as_array().unwrap();
    let wanted = |m: &Value| {
        history.contains(&json!({"at": m["at"], "level": "warning", "message": m["message"]}))
    };
    assert!(wanted(&refusal), "{refusal} not in {history:?}");
    assert!(wanted(&change), "{change} not in {history:?}");
}
