//! A new show that keeps the patch (SHW-19), on the real binary: the switch, the files and the
//! history live in the tick. What the new show holds is `cmdmx-core/tests/showfile.rs`.

#![cfg(unix)]
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

fn send(ws: &mut WebSocket<TcpStream>, mut cmd: Value) {
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

fn len(v: &Value) -> usize {
    v.as_array().unwrap().len()
}

#[test]
fn shw_19_new_show_keeps_the_patch_saves_the_old_show_and_empties_the_history() {
    let dir = std::env::temp_dir().join(format!("cmdmx-shw19-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let shows = dir.join("Shows");
    let port = free_port();
    let _daemon = Daemon(
        Command::new(env!("CARGO_BIN_EXE_cmdmx-daemon"))
            .env("HOME", &dir)
            .args(["--no-artnet", "--no-sacn", "--no-serial"])
            .args(["--ipc-port", &port.to_string()])
            .arg("--show-dir")
            .arg(&shows)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut app = connect(port);
    let demo = recv(&mut app);
    assert_eq!(demo["type"], "snapshot");
    assert_eq!(demo["show"]["name"], "Demo");
    assert!(len(&demo["fixtures"]) > 0);
    assert!(len(&demo["playbacks"]) > 0);

    // A change the autosave has not written yet, and an undo step.
    let fixture = demo["fixtures"][0]["id"].clone();
    send(
        &mut app,
        json!({"cmd": "programmer.select", "fixture": fixture}),
    );
    send(&mut app, json!({"cmd": "group.record", "group": 77}));
    until(&mut app, "the undo label", |m| {
        m["type"] == "undo" && !m["undo"].is_null()
    });

    send(
        &mut app,
        json!({"cmd": "show.new", "name": "Kerk kerst", "keep": "patch"}),
    );
    let new = until(&mut app, "the new show", |m| {
        m["type"] == "snapshot" && m["show"]["name"] == "Kerk kerst"
    });
    assert_eq!(new["fixtures"], demo["fixtures"]);
    assert_eq!(len(&new["groups"]), 0);
    assert_eq!(len(&new["palettes"]), 0);
    assert_eq!(len(&new["playbacks"]), 0);
    assert_eq!(new["page"]["number"], 1);
    assert!(new["undo"]["undo"].is_null(), "UNDO-13: {}", new["undo"]);

    // The old file holds the old show with the last change, the new file exists at once.
    let read = |name: &str| -> Value {
        let path = shows.join(format!("{name}.cmdmx.json"));
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match std::fs::read_to_string(&path) {
                Ok(text) => return serde_json::from_str(&text).unwrap(),
                Err(_) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => panic!("{}: {e}", path.display()),
            }
        }
    };
    let old = read("Demo");
    assert!(old["groups"]
        .as_array()
        .unwrap()
        .iter()
        .any(|g| g["id"] == 77));
    assert!(len(&old["playbacks"]["playbacks"]) > 0);
    assert_eq!(
        len(&read("Kerk kerst")["patch"]["fixtures"]),
        len(&demo["fixtures"])
    );

    // SHW-15: an existing name is refused and the show keeps running.
    send(
        &mut app,
        json!({"cmd": "show.new", "name": "Demo", "keep": "patch"}),
    );
    let w = until(&mut app, "the refusal", |m| m["type"] == "warning");
    assert!(
        w["message"].as_str().unwrap().contains("already exists"),
        "{w}"
    );

    // Without `keep` it stays SHW-15: an empty patch.
    send(&mut app, json!({"cmd": "show.new", "name": "Leeg"}));
    let empty = until(&mut app, "the empty show", |m| {
        m["type"] == "snapshot" && m["show"]["name"] == "Leeg"
    });
    assert_eq!(len(&empty["fixtures"]), 0);
}
