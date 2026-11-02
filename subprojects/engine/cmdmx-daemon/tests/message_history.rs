//! The message history reaches a later client (IPC-51) and a node's state change is in it
//! (IPC-52): `specs/modules/ipc.md`. Runs the real binary on a temp `HOME` and `--show-dir`.

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

#[test]
fn ipc_51_ipc_52_a_later_client_gets_the_refusal_and_the_node_change_in_its_snapshot() {
    let dir = std::env::temp_dir().join(format!("cmdmx-ipc51-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let port = free_port();
    let _daemon = Daemon(
        Command::new(env!("CARGO_BIN_EXE_cmdmx-daemon"))
            .env("HOME", &dir)
            .args(["--no-sacn", "--ipc-port", &port.to_string()])
            .arg("--show-dir")
            .arg(dir.join("Shows"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );

    let mut first = connect(port);
    assert_eq!(recv(&mut first)["type"], "snapshot");

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
