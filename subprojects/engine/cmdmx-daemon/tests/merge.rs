//! Merge from another show on the real binary (MRG-02): `specs/modules/showfile.md` SHW-27 and
//! SHW-31, wire details in `specs/modules/ipc.md` (*Show file*). Reading the file, the answer to
//! the asking client, the notice to every client, the undo step, the autosave and the lock. What a
//! merge places is `cmdmx-core/tests/merge.rs`.

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

/// The messages that arrive within `wait`, for checking what did not come.
fn drain(ws: &mut WebSocket<TcpStream>, wait: Duration) -> Vec<Value> {
    ws.get_mut()
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let deadline = Instant::now() + wait;
    let mut seen = Vec::new();
    while Instant::now() < deadline {
        match ws.read() {
            Ok(Message::Text(t)) => seen.push(serde_json::from_str(&t).unwrap()),
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(e) => panic!("{e}"),
        }
    }
    ws.get_mut()
        .set_read_timeout(Some(Duration::from_secs(15)))
        .unwrap();
    seen
}

fn len(v: &Value) -> usize {
    v.as_array().unwrap().len()
}

fn read_soon(path: &Path, ok: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(v) = std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .filter(&ok)
        {
            return v;
        }
        assert!(Instant::now() < deadline, "{} never came", path.display());
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A daemon on the Demo show in a fresh temp folder, an app connected (snapshot read), and
/// "Kerk 9 nov", a copy of the Demo, written beside it.
fn start(test: &str) -> (Daemon, u16, PathBuf, WebSocket<TcpStream>, Value) {
    let dir = std::env::temp_dir().join(format!("cmdmx-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let shows = dir.join("Shows");
    let port = free_port();
    let daemon = Daemon(
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
    assert_eq!(demo["show"]["name"], "Demo");
    send(
        &mut app,
        json!({"cmd": "show.save_copy", "name": "Kerk 9 nov"}),
    );
    read_soon(&shows.join("Kerk 9 nov.cmdmx.json"), |v| {
        v["schema_version"].is_number()
    });
    (daemon, port, shows, app, demo)
}

fn warning_says(m: &Value, what: &str) -> bool {
    m["type"] == "warning" && m["message"].as_str().unwrap_or("").contains(what)
}

#[test]
fn shw_27_merge_read_answers_the_asker_with_the_contents_and_writes_nothing() {
    let (_daemon, port, shows, mut app, demo) = start("shw27");
    let mut other = connect(port);
    recv(&mut other); // its snapshot

    send(
        &mut app,
        json!({"cmd": "show.merge.read", "name": "Kerk 9 nov"}),
    );
    let contents = until(&mut app, "the contents", |m| {
        m["type"] == "show.merge.contents"
    });
    assert_eq!(contents["name"], "Kerk 9 nov");
    // A copy: every fixture is proposed onto itself.
    assert_eq!(len(&contents["fixtures"]), len(&demo["fixtures"]));
    for f in contents["fixtures"].as_array().unwrap() {
        assert_eq!(f["to"], f["number"], "{f}");
    }
    assert_eq!(len(&contents["playbacks"]), len(&demo["playbacks"]));
    assert!(len(&contents["groups"]) > 0);
    // To the asking client only.
    let seen = drain(&mut other, Duration::from_millis(500));
    assert!(
        seen.iter().all(|m| m["type"] != "show.merge.contents"),
        "{seen:?}"
    );

    // An older schema is migrated in memory; its file is never written.
    let copy = std::fs::read_to_string(shows.join("Kerk 9 nov.cmdmx.json")).unwrap();
    let mut old: Value = serde_json::from_str(&copy).unwrap();
    old["schema_version"] = json!(14);
    let old_text = serde_json::to_string_pretty(&old).unwrap();
    std::fs::write(shows.join("Oud.cmdmx.json"), &old_text).unwrap();
    send(&mut app, json!({"cmd": "show.merge.read", "name": "Oud"}));
    let read = until(&mut app, "the old show's contents", |m| {
        m["type"] == "show.merge.contents" && m["name"] == "Oud"
    });
    assert_eq!(len(&read["fixtures"]), len(&demo["fixtures"]));
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        std::fs::read_to_string(shows.join("Oud.cmdmx.json")).unwrap(),
        old_text
    );

    // Refusals: a newer schema, a file that is not a show, an unknown name, the open show.
    let mut newer = old.clone();
    newer["schema_version"] = json!(99);
    std::fs::write(shows.join("Nieuw.cmdmx.json"), newer.to_string()).unwrap();
    std::fs::write(shows.join("Kapot.cmdmx.json"), "not a show").unwrap();
    for (name, says) in [
        ("Nieuw", "newer"),
        ("Kapot", "Kapot"),
        ("Bestaat niet", "Bestaat niet"),
        ("Demo", "Demo"),
    ] {
        send(&mut app, json!({"cmd": "show.merge.read", "name": name}));
        let w = until(&mut app, &format!("the refusal of {name}"), |m| {
            m["type"] == "warning" || m["type"] == "show.merge.contents"
        });
        assert!(warning_says(&w, says), "{name}: {w}");
    }
}

#[test]
fn shw_31_merge_answers_the_plan_tells_every_client_and_is_one_undo_step() {
    let (_daemon, port, shows, mut app, demo) = start("shw31");
    let mut other = connect(port);
    recv(&mut other);
    send(
        &mut app,
        json!({"cmd": "show.merge.read", "name": "Kerk 9 nov"}),
    );
    let contents = until(&mut app, "the contents", |m| {
        m["type"] == "show.merge.contents"
    });
    let first = &contents["playbacks"][0];
    let request = json!({
        "cmd": "show.merge",
        "name": "Kerk 9 nov",
        "playbacks": [{"page": first["page"], "slot": first["slot"]}],
        "dry_run": true,
    });

    // A dry run: the plan to the asker, nothing changes, no notice.
    send(&mut app, request.clone());
    let dry = until(&mut app, "the dry run's plan", |m| {
        m["type"] == "show.merge.plan"
    });
    assert_eq!(dry["dry_run"], true);
    assert_eq!(len(&dry["placed"]), 1);
    assert_eq!(dry["placed"][0]["kind"], "playback");
    // Its own handle is taken in the open show (a copy), so it lands elsewhere.
    assert_ne!(
        dry["placed"][0]["to"],
        format!("{}.{}", first["page"], first["slot"])
    );
    let seen = drain(&mut other, Duration::from_millis(500));
    assert!(
        seen.iter()
            .all(|m| m["type"] != "notice" && m["type"] != "show.merge.plan"),
        "{seen:?}"
    );

    // The real merge: the same plan, the notice to every client, one undo step.
    let mut real = request.clone();
    real["dry_run"] = json!(false);
    send(&mut app, real.clone());
    let plan = until(&mut app, "the plan", |m| m["type"] == "show.merge.plan");
    assert_eq!(plan["dry_run"], false);
    assert_eq!(plan["placed"], dry["placed"]);
    let notice = until(&mut other, "the notice", |m| m["type"] == "notice");
    assert_eq!(notice["message"], "Merged from Kerk 9 nov: 1 playback");
    until(&mut other, "the undo label", |m| {
        m["type"] == "undo" && m["undo"] == "Merge from Kerk 9 nov"
    });

    // The autosave follows.
    let before = len(&demo["playbacks"]);
    read_soon(&shows.join("Demo.cmdmx.json"), |v| {
        v["playbacks"]["playbacks"]
            .as_array()
            .is_some_and(|p| p.len() == before + 1)
    });

    // IPC-50: the lock refuses show.merge, a dry run too, and not show.merge.read.
    send(
        &mut app,
        json!({"cmd": "lock.set", "on": true, "code": "1234"}),
    );
    until(&mut app, "the lock", |m| {
        m["type"] == "locked" || m["locked"] == true
    });
    send(&mut app, request);
    let w = until(&mut app, "the refusal", |m| {
        m["type"] == "warning" || m["type"] == "show.merge.plan"
    });
    assert!(warning_says(&w, "locked"), "{w}");
    send(&mut app, real);
    let w = until(&mut app, "the refusal", |m| {
        m["type"] == "warning" || m["type"] == "show.merge.plan"
    });
    assert!(warning_says(&w, "locked"), "{w}");
    send(
        &mut app,
        json!({"cmd": "show.merge.read", "name": "Kerk 9 nov"}),
    );
    until(&mut app, "the contents while locked", |m| {
        m["type"] == "show.merge.contents"
    });
}

#[test]
fn shw_31_a_refused_merge_is_a_warning_and_changes_nothing() {
    let (_daemon, _port, _shows, mut app, _demo) = start("shw31refused");
    send(
        &mut app,
        json!({"cmd": "show.merge", "name": "Kerk 9 nov", "groups": [4242]}),
    );
    let w = until(&mut app, "the refusal", |m| {
        m["type"] == "warning" || m["type"] == "show.merge.plan"
    });
    assert_eq!(w["type"], "warning", "{w}");
    // Nothing became an undo step.
    let seen = drain(&mut app, Duration::from_millis(500));
    assert!(seen.iter().all(|m| m["type"] != "undo"), "{seen:?}");
}
