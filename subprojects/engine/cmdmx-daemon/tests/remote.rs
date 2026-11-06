//! The remote listener on the real binary (RMT-01..08): `specs/modules/remote.md`. Runs the daemon on
//! a temp `HOME` and `--show-dir` (the Demo show), drives it from an app client on the IPC port and
//! from phone clients on the remote port over loopback (the listener is on every interface).

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tungstenite::client::IntoClientRequest;
use tungstenite::{Message, WebSocket};

/// Kills the daemon when the test ends, passed or not.
struct Daemon(Child);

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// As in `lock.rs`: below the ephemeral ranges, never bound here, and not taken by anyone else.
/// A range of its own (30 000+) so it does not meet the other test binaries' ports.
fn free_port() -> u16 {
    static NEXT: AtomicU16 = AtomicU16::new(0);
    let base = 30_000 + (std::process::id() % 10_000) as u16;
    loop {
        let port = base + NEXT.fetch_add(1, Ordering::Relaxed);
        if TcpStream::connect(("127.0.0.1", port)).is_err() {
            return port;
        }
    }
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
            .args(["--no-sacn", "--no-serial", "--ipc-port", &port.to_string()])
            .arg("--show-dir")
            .arg(dir.join("Shows"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

type Ws = WebSocket<TcpStream>;

fn connect(port: u16) -> Ws {
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

fn recv(ws: &mut Ws) -> Value {
    loop {
        if let Message::Text(t) = ws.read().unwrap() {
            return serde_json::from_str(&t).unwrap();
        }
    }
}

fn send(ws: &mut Ws, cmd: Value) {
    let mut cmd = cmd;
    cmd["v"] = json!(1);
    ws.send(Message::Text(cmd.to_string().into())).unwrap();
}

fn until(ws: &mut Ws, what: &str, pred: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(Instant::now() < deadline, "never saw {what}");
        let m = recv(ws);
        if pred(&m) {
            return m;
        }
    }
}

/// The app's view of the remote: from a fresh snapshot.
fn remote_info(ipc: u16) -> Value {
    let mut ws = connect(ipc);
    let s = recv(&mut ws);
    assert_eq!(s["type"], "snapshot");
    s["remote"].clone()
}

/// `remote.set_login` from the app, then wait until the app sees the listener on.
fn turn_on(app: &mut Ws, port: u16) {
    send(
        app,
        json!({"cmd": "remote.set_login", "username": "aernout", "password": "kerst2026", "port": port}),
    );
    until(app, "the remote on", |m| {
        m["type"] == "remote" && m["remote"]["on"] == true && m["remote"]["port"] == port
    });
}

/// Whether anything listens on the remote port.
fn listening(port: u16) -> bool {
    TcpStream::connect(("127.0.0.1", port)).is_ok()
}

/// One plain HTTP request; the whole answer as text.
fn http(port: u16, request: &str) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.write_all(request.as_bytes()).unwrap();
    let mut out = Vec::new();
    let _ = s.read_to_end(&mut out);
    String::from_utf8_lossy(&out).into_owned()
}

fn get(port: u16, path: &str) -> String {
    http(
        port,
        &format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"),
    )
}

/// A phone: a WebSocket on `/ws` with the given `Origin` (`None` = no header).
fn phone_with(port: u16, path: &str, origin: Option<&str>) -> Result<Ws, String> {
    let stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .unwrap();
    let mut req = format!("ws://127.0.0.1:{port}{path}")
        .into_client_request()
        .unwrap();
    if let Some(o) = origin {
        req.headers_mut().insert("Origin", o.parse().unwrap());
    }
    tungstenite::client(req, stream)
        .map(|(ws, _)| ws)
        .map_err(|e| e.to_string())
}

fn phone(port: u16) -> Ws {
    phone_with(port, "/ws", Some(&format!("http://127.0.0.1:{port}"))).unwrap()
}

fn login(ws: &mut Ws, username: &str, password: &str) -> Value {
    send(
        ws,
        json!({"cmd": "login", "username": username, "password": password}),
    );
    recv(ws)
}

/// A phone that logged in: its token, and the remote snapshot that follows the login.
fn logged_in(port: u16) -> (Ws, String, Value) {
    let mut ws = phone(port);
    let reply = login(&mut ws, "aernout", "kerst2026");
    assert_eq!(reply["type"], "login", "{reply}");
    let token = reply["token"].as_str().unwrap().to_string();
    assert!(!token.is_empty());
    let snap = recv(&mut ws);
    assert_eq!(snap["type"], "snapshot", "{snap}");
    (ws, token, snap)
}

/// Whether the server closed the connection (a close frame, a reset or end of stream).
fn closed(ws: &mut Ws) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        match ws.read() {
            Ok(Message::Close(_)) | Err(_) => return true,
            Ok(_) => {}
        }
    }
    false
}

fn state_json(dir: &Path) -> Value {
    let text = std::fs::read_to_string(dir.join("Shows/state.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

#[test]
fn rmt_01_off_until_a_login_then_on_off_and_back_after_a_restart() {
    let dir = folder("rmt01");
    let (ipc, port) = (free_port(), free_port());
    let first = daemon(&dir, ipc);
    let mut app = connect(ipc);
    let snap = recv(&mut app);
    assert_eq!(snap["remote"]["on"], false);
    assert_eq!(snap["remote"]["port"], 7320, "the default port");
    assert!(!listening(port));

    // Both fields 1–64 characters, the port 1024–65535: anything else is an error to the app.
    for bad in [
        json!({"cmd": "remote.set_login", "username": "", "password": "x"}),
        json!({"cmd": "remote.set_login", "username": "a", "password": ""}),
        json!({"cmd": "remote.set_login", "username": "a".repeat(65), "password": "x"}),
        json!({"cmd": "remote.set_login", "username": "a", "password": "x", "port": 80}),
    ] {
        send(&mut app, bad);
        until(&mut app, "an error", |m| m["type"] == "error");
    }
    assert!(!listening(port));

    turn_on(&mut app, port);
    assert!(listening(port));
    let mut ws = phone(port);
    assert_eq!(login(&mut ws, "aernout", "kerst2026")["type"], "login");

    // Off: the port closes and so does every remote client.
    send(&mut app, json!({"cmd": "remote.off"}));
    until(&mut app, "the remote off", |m| {
        m["type"] == "remote" && m["remote"]["on"] == false
    });
    assert!(closed(&mut ws));
    assert!(!listening(port));
    assert_eq!(state_json(&dir)["remote"]["on"], false);

    // On again, then a restart: the login, the port and on come back from `state.json`.
    turn_on(&mut app, port);
    let saved = state_json(&dir)["remote"].clone();
    assert_eq!(saved["on"], true);
    assert_eq!(saved["username"], "aernout");
    assert_eq!(saved["port"], port);
    drop(app);
    drop(first);
    let ipc2 = free_port();
    let _second = daemon(&dir, ipc2);
    let info = remote_info(ipc2);
    assert_eq!(info["on"], true);
    assert_eq!(info["port"], port);
    let (_ws, _, _) = logged_in(port);

    // Opening another show keeps them: they are not show data.
    let mut app = connect(ipc2);
    recv(&mut app);
    send(&mut app, json!({"cmd": "show.new", "name": "Other"}));
    until(&mut app, "the new show", |m| {
        m["type"] == "snapshot" && m["show"]["name"] == "Other"
    });
    assert_eq!(remote_info(ipc2)["on"], true);
    assert!(listening(port));
}

#[test]
fn rmt_01_a_port_in_use_is_a_warning_and_the_listener_stays_off() {
    let dir = folder("rmt01-busy");
    let (ipc, port) = (free_port(), free_port());
    let taken = TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)).unwrap();
    let _daemon = daemon(&dir, ipc);
    let mut app = connect(ipc);
    recv(&mut app);
    send(
        &mut app,
        json!({"cmd": "remote.set_login", "username": "aernout", "password": "kerst2026", "port": port}),
    );
    let w = until(&mut app, "the port warning", |m| m["type"] == "warning");
    assert!(
        w["message"].as_str().unwrap().contains(&port.to_string()),
        "{w}"
    );
    assert_eq!(remote_info(ipc)["on"], false);
    drop(taken);
}

#[test]
fn rmt_02_the_password_is_a_salted_hash_and_never_leaves_the_daemon() {
    let dir = folder("rmt02");
    let (ipc, port) = (free_port(), free_port());
    let _daemon = daemon(&dir, ipc);
    let mut app = connect(ipc);
    recv(&mut app);
    turn_on(&mut app, port);

    let state = std::fs::read_to_string(dir.join("Shows/state.json")).unwrap();
    assert!(!state.contains("kerst2026"), "the password itself: {state}");
    let saved = state_json(&dir)["remote"].clone();
    let (salt, hash) = (
        saved["salt"].as_str().unwrap(),
        saved["hash"].as_str().unwrap(),
    );
    assert!(salt.len() >= 32 && hash.len() >= 64, "{saved}");

    // The same password set again gets another salt, so another hash.
    turn_on(&mut app, port);
    let again = state_json(&dir)["remote"].clone();
    assert_ne!(again["salt"], saved["salt"]);
    assert_ne!(again["hash"], saved["hash"]);

    let mut fresh = connect(ipc);
    let text = match fresh.read().unwrap() {
        Message::Text(t) => t.to_string(),
        m => panic!("{m:?}"),
    };
    assert!(!text.contains("kerst2026"));
    let snap: Value = serde_json::from_str(&text).unwrap();
    let remote = &snap["remote"];
    assert_eq!(remote["on"], true);
    assert_eq!(remote["username"], "aernout");
    assert_eq!(remote["port"], port);
    assert_eq!(remote["clients"], 0);
    let keys: Vec<_> = remote.as_object().unwrap().keys().cloned().collect();
    assert_eq!(
        keys.len(),
        5,
        "only on, username, port, addresses, clients: {remote}"
    );
    for a in remote["addresses"].as_array().unwrap() {
        let ip: Ipv4Addr = a.as_str().unwrap().parse().unwrap();
        assert!(!ip.is_loopback(), "{ip}");
    }

    // A phone that logs in and goes away is counted in a `remote` delta.
    let (ws, _, _) = logged_in(port);
    until(&mut app, "one phone", |m| {
        m["type"] == "remote" && m["remote"]["clients"] == 1
    });
    drop(ws);
    until(&mut app, "no phone", |m| {
        m["type"] == "remote" && m["remote"]["clients"] == 0
    });
}

#[test]
fn rmt_03_the_page_on_get_slash_404_elsewhere_and_the_websocket_only_from_its_own_origin() {
    let dir = folder("rmt03");
    let (ipc, port) = (free_port(), free_port());
    let _daemon = daemon(&dir, ipc);
    let mut app = connect(ipc);
    recv(&mut app);
    turn_on(&mut app, port);

    let page = get(port, "/");
    assert!(page.starts_with("HTTP/1.1 200"), "{page}");
    assert!(
        page.to_lowercase().contains("content-type: text/html"),
        "{page}"
    );
    assert!(
        page.contains("<html")
            || page.contains("<!doctype html")
            || page.contains("<!DOCTYPE html")
    );
    for path in ["/index.html", "/ws", "/../state.json", "/favicon.ico"] {
        let r = get(port, path);
        assert!(r.starts_with("HTTP/1.1 404"), "{path}: {r}");
    }

    // No Origin, another site's Origin, another port's Origin, or a path other than /ws: refused.
    assert!(phone_with(port, "/ws", None).is_err());
    assert!(phone_with(port, "/ws", Some("http://evil.example")).is_err());
    assert!(phone_with(port, "/ws", Some(&format!("http://evil.example:{port}"))).is_err());
    assert!(phone_with(port, "/ws", Some("http://127.0.0.1:1")).is_err());
    assert!(phone_with(port, "/", Some(&format!("http://127.0.0.1:{port}"))).is_err());
    assert!(phone_with(port, "/ws", Some(&format!("http://127.0.0.1:{port}"))).is_ok());

    // The app's own port still refuses a browser Origin (IPC-02).
    let stream = TcpStream::connect(("127.0.0.1", ipc)).unwrap();
    let mut req = format!("ws://127.0.0.1:{ipc}/")
        .into_client_request()
        .unwrap();
    req.headers_mut().insert(
        "Origin",
        format!("http://127.0.0.1:{port}").parse().unwrap(),
    );
    assert!(tungstenite::client(req, stream).is_err());
}

#[test]
fn rmt_04_login_with_a_password_or_a_token_and_nothing_before_it() {
    let dir = folder("rmt04");
    let (ipc, port) = (free_port(), free_port());
    let _daemon = daemon(&dir, ipc);
    let mut app = connect(ipc);
    recv(&mut app);
    turn_on(&mut app, port);

    let (_first, token, _) = logged_in(port);

    // A token logs in again without the password.
    let mut ws = phone(port);
    send(&mut ws, json!({"cmd": "login", "token": token}));
    assert_eq!(recv(&mut ws)["type"], "login");
    assert_eq!(recv(&mut ws)["type"], "snapshot");

    // A wrong password or username: the error, then the connection closes.
    for (user, pass) in [("aernout", "wrong"), ("someone", "kerst2026")] {
        let mut ws = phone(port);
        let reply = login(&mut ws, user, pass);
        assert_eq!(
            reply,
            json!({"v": 1, "type": "login", "error": "Wrong username or password"})
        );
        assert!(closed(&mut ws));
    }

    // Anything else first closes it.
    let mut ws = phone(port);
    send(
        &mut ws,
        json!({"cmd": "patch.name", "id": 1, "name": "Sneaky"}),
    );
    assert!(closed(&mut ws));

    // A new login makes the old token useless.
    turn_on(&mut app, port);
    let mut ws = phone(port);
    send(&mut ws, json!({"cmd": "login", "token": token}));
    assert_eq!(recv(&mut ws)["error"], "Wrong username or password");
}

#[test]
fn rmt_05_five_wrong_logins_lock_the_address_out_and_a_silent_phone_never_slows_the_tick() {
    let dir = folder("rmt05");
    let (ipc, port) = (free_port(), free_port());
    let _daemon = daemon(&dir, ipc);
    let mut app = connect(ipc);
    recv(&mut app);
    turn_on(&mut app, port);

    // A phone that logs in and then never reads again.
    let (_silent, _, _) = logged_in(port);

    for _ in 0..5 {
        let mut ws = phone(port);
        assert_eq!(
            login(&mut ws, "aernout", "wrong")["error"],
            "Wrong username or password"
        );
    }
    // The sixth, even with the right password, is refused for 30 s.
    let mut ws = phone(port);
    assert_eq!(
        login(&mut ws, "aernout", "kerst2026")["error"],
        "Too many tries, wait 30 s"
    );

    // Many patch changes, each a remote snapshot the silent phone never takes: the app is still
    // answered at once.
    for n in 0..200 {
        send(
            &mut app,
            json!({"cmd": "patch.name", "id": 1, "name": format!("Busy {n}")}),
        );
    }
    let started = Instant::now();
    send(&mut app, json!({"cmd": "show.list"}));
    until(&mut app, "the show list", |m| m["type"] == "show.list");
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn rmt_06_the_remote_snapshot_and_its_resend() {
    let dir = folder("rmt06");
    let (ipc, port) = (free_port(), free_port());
    let _daemon = daemon(&dir, ipc);
    let mut app = connect(ipc);
    let app_snap = recv(&mut app);
    turn_on(&mut app, port);

    let (mut ws, _, snap) = logged_in(port);
    assert_eq!(snap["v"], 1);
    assert_eq!(snap["show"], app_snap["show"]["name"]);
    assert_eq!(snap["locked"], false);
    assert_eq!(snap["highlight"], Value::Null);
    let fixtures = snap["fixtures"].as_array().unwrap();
    assert_eq!(
        fixtures.len(),
        app_snap["fixtures"].as_array().unwrap().len()
    );
    assert!(!fixtures.is_empty());
    let numbers: Vec<u64> = fixtures
        .iter()
        .map(|f| f["number"].as_u64().unwrap())
        .collect();
    let mut sorted = numbers.clone();
    sorted.sort_unstable();
    assert_eq!(numbers, sorted, "number order");
    for f in fixtures {
        let mut keys: Vec<_> = f.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                "address",
                "footprint",
                "id",
                "mode",
                "name",
                "number",
                "parked",
                "profile",
                "universe"
            ]
        );
        assert_eq!(f["parked"], false);
    }
    let first = fixtures[0]["id"].as_u64().unwrap();

    // A patch change from the app: the snapshot again.
    send(
        &mut app,
        json!({"cmd": "patch.name", "id": first, "name": "Front L"}),
    );
    until(&mut ws, "the renamed fixture", |m| {
        m["type"] == "snapshot"
            && m["fixtures"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["name"] == "Front L")
    });

    // The lock.
    send(
        &mut app,
        json!({"cmd": "lock.set", "on": true, "code": "1"}),
    );
    until(&mut ws, "locked", |m| {
        m["type"] == "snapshot" && m["locked"] == true
    });
    send(
        &mut app,
        json!({"cmd": "lock.set", "on": false, "code": "1"}),
    );
    until(&mut ws, "unlocked", |m| {
        m["type"] == "snapshot" && m["locked"] == false
    });

    // Another show.
    send(&mut app, json!({"cmd": "show.new", "name": "Other"}));
    until(&mut ws, "the other show", |m| {
        m["type"] == "snapshot" && m["show"] == "Other" && m["fixtures"] == json!([])
    });
}

#[test]
fn rmt_07_only_the_allowed_commands_and_the_console_rules_for_them() {
    let dir = folder("rmt07");
    let (ipc, port) = (free_port(), free_port());
    let _daemon = daemon(&dir, ipc);
    let mut app = connect(ipc);
    recv(&mut app);
    turn_on(&mut app, port);
    let (mut ws, _, snap) = logged_in(port);
    let fixtures = snap["fixtures"].as_array().unwrap();
    let (a, b) = (&fixtures[0], &fixtures[1]);
    let id = a["id"].as_u64().unwrap();

    // A name: one undo step at the console, the new snapshot on the phone.
    send(
        &mut ws,
        json!({"cmd": "patch.name", "id": id, "name": "Front L"}),
    );
    until(&mut app, "the undo label", |m| {
        m["type"] == "undo" && m["undo"].as_str().is_some()
    });
    until(&mut ws, "the name", |m| {
        m["type"] == "snapshot" && m["fixtures"][0]["name"] == "Front L"
    });

    // A move to a free address.
    send(
        &mut ws,
        json!({"cmd": "patch.move", "id": id, "universe": 4, "address": 101}),
    );
    until(&mut ws, "the move", |m| {
        m["type"] == "snapshot"
            && m["fixtures"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["id"] == id && f["universe"] == 4 && f["address"] == 101)
    });

    // A move onto another fixture: refused, a warning to the phone (and the app).
    send(
        &mut ws,
        json!({"cmd": "patch.move", "id": id, "universe": b["universe"], "address": b["address"]}),
    );
    until(&mut ws, "the clash warning", |m| m["type"] == "warning");
    until(&mut app, "the clash warning", |m| m["type"] == "warning");

    // Anything else is an error to that phone, and changes nothing.
    for cmd in [
        json!({"cmd": "programmer.clear"}),
        json!({"cmd": "patch.remove", "ids": [id]}),
        json!({"cmd": "patch.add", "profile": "Demo/Wash", "mode": "default", "count": 1, "universe": 9}),
        json!({"cmd": "patch.renumber", "id": id, "number": 999}),
        json!({"cmd": "remote.set_login", "username": "x", "password": "y"}),
        json!({"cmd": "remote.off"}),
        json!({"cmd": "lock.set", "on": true, "code": "1"}),
        json!({"cmd": "show.new", "name": "Gone"}),
        json!({"cmd": "undo"}),
        json!({"cmd": "no.such"}),
    ] {
        send(&mut ws, cmd.clone());
        until(&mut ws, &format!("the error for {cmd}"), |m| {
            m["type"] == "error"
        });
    }
    assert_eq!(remote_info(ipc)["on"], true);

    // Locked: an edit from the phone is refused with the lock's warning.
    send(
        &mut app,
        json!({"cmd": "lock.set", "on": true, "code": "1"}),
    );
    until(&mut ws, "locked", |m| {
        m["type"] == "snapshot" && m["locked"] == true
    });
    send(
        &mut ws,
        json!({"cmd": "patch.name", "id": id, "name": "Locked out"}),
    );
    until(&mut ws, "the lock warning", |m| {
        m["type"] == "warning" && m["message"] == "Console is locked"
    });
}

#[test]
fn rmt_08_remote_highlight_uses_the_console_selection_and_highlight() {
    let dir = folder("rmt08");
    let (ipc, port) = (free_port(), free_port());
    let _daemon = daemon(&dir, ipc);
    let mut app = connect(ipc);
    recv(&mut app);
    turn_on(&mut app, port);
    let (mut ws, _, snap) = logged_in(port);
    let fixtures = snap["fixtures"].as_array().unwrap();
    let (a, b) = (
        fixtures[0]["id"].as_u64().unwrap(),
        fixtures[1]["id"].as_u64().unwrap(),
    );

    // Something selected at the console first: the highlight replaces the whole selection.
    send(&mut app, json!({"cmd": "programmer.select", "fixture": a}));
    send(&mut ws, json!({"cmd": "remote.highlight", "id": b}));
    until(&mut ws, "the highlight", |m| {
        m["type"] == "snapshot" && m["highlight"] == b
    });
    let now = connect(ipc);
    let mut now = now;
    let p = recv(&mut now)["programmer"].clone();
    assert_eq!(p["highlight"], true, "{p}");
    assert_eq!(p["selection"], json!([b]), "{p}");

    // `null` switches Highlight off and keeps the selection.
    send(&mut ws, json!({"cmd": "remote.highlight", "id": null}));
    until(&mut ws, "no highlight", |m| {
        m["type"] == "snapshot" && m["highlight"] == Value::Null
    });
    let mut now = connect(ipc);
    let p = recv(&mut now)["programmer"].clone();
    assert_eq!(p["highlight"], false);
    assert_eq!(p["selection"], json!([b]));

    // An unknown fixture is a warning; a phone that goes away leaves things as they are.
    send(&mut ws, json!({"cmd": "remote.highlight", "id": 99_999}));
    until(&mut ws, "the warning", |m| m["type"] == "warning");
    send(&mut ws, json!({"cmd": "remote.highlight", "id": a}));
    until(&mut ws, "the highlight", |m| {
        m["type"] == "snapshot" && m["highlight"] == a
    });
    drop(ws);
    std::thread::sleep(Duration::from_millis(300));
    let mut now = connect(ipc);
    let p = recv(&mut now)["programmer"].clone();
    assert_eq!(p["highlight"], true);
    assert_eq!(p["selection"], json!([a]));
}
