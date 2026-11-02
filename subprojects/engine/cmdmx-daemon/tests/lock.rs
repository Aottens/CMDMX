//! The console lock on the real tick (IPC-50): `specs/modules/ipc.md`. Runs the real binary on a
//! temp `HOME` and `--show-dir`; the surfaces' side is in `src/control/*tests.rs`.

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::net::{TcpListener, TcpStream, UdpSocket};
use std::os::fd::FromRawFd;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use cmdmx_control::osc::{encode, OscArg, OscMessage};
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
            .args(["--no-sacn", "--no-serial", "--ipc-port", &port.to_string()])
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

fn lock_set(ws: &mut WebSocket<TcpStream>, on: bool, code: &str) {
    send(ws, json!({"cmd": "lock.set", "on": on, "code": code}));
}

fn saw_locked(ws: &mut WebSocket<TcpStream>, on: bool) {
    until(ws, "the locked delta", |m| {
        m["type"] == "locked" && m["locked"] == on
    });
}

fn saw_warning(ws: &mut WebSocket<TcpStream>, text: &str) {
    until(ws, text, |m| m["type"] == "warning" && m["message"] == text);
}

#[test]
fn ipc_50_the_lock_is_in_the_snapshot_and_a_delta_to_every_client() {
    let dir = folder("ipc50-delta");
    let port = free_port();
    let _daemon = daemon(&dir, port);
    let mut first = connect(port);
    let mut second = connect(port);
    assert_eq!(recv(&mut first)["locked"], false);
    assert_eq!(recv(&mut second)["locked"], false);

    lock_set(&mut first, true, "1234");
    saw_locked(&mut first, true);
    saw_locked(&mut second, true);
    assert_eq!(fresh_snapshot(port)["locked"], true);

    lock_set(&mut second, false, "1234");
    saw_locked(&mut first, false);
    saw_locked(&mut second, false);
    assert_eq!(fresh_snapshot(port)["locked"], false);
}

#[test]
fn ipc_50_a_changing_command_is_refused_and_information_still_answers() {
    let dir = folder("ipc50-refused");
    let port = free_port();
    let _daemon = daemon(&dir, port);
    let mut app = connect(port);
    let before = recv(&mut app);
    lock_set(&mut app, true, "1234");
    saw_locked(&mut app, true);

    for cmd in [
        json!({"cmd": "grand_master.set", "level": 0.25}),
        json!({"cmd": "fader.set", "playback": 2, "level": 0.6}),
        json!({"cmd": "programmer.select", "fixture": 4}),
        json!({"cmd": "cuelist.go", "playback": 2}),
        json!({"cmd": "output.add", "line": 3, "kind": "artnet"}),
        json!({"cmd": "undo"}),
        json!({"cmd": "control.osc", "enabled": true}),
        // A second lock does not change the code either.
        json!({"cmd": "lock.set", "on": true, "code": "9999"}),
        // The app thread's own requests: nothing is read or written first.
        json!({"cmd": "show.new", "name": "Other"}),
        json!({"cmd": "show.save_as", "name": "Other"}),
        json!({"cmd": "show.open", "name": "No such show"}),
        json!({"cmd": "library.import_gdtf", "path": "/no/such/file.gdtf"}),
    ] {
        send(&mut app, cmd);
        saw_warning(&mut app, "Console is locked");
    }

    for (cmd, answer) in [
        (json!({"cmd": "output.ports"}), "ports"),
        (json!({"cmd": "show.list"}), "show.list"),
        (
            json!({"cmd": "library.gels", "range": "Lee"}),
            "library.gels",
        ),
        (
            json!({"cmd": "control.mapping", "name": "Mackie Control"}),
            "control.mapping",
        ),
    ] {
        send(&mut app, cmd);
        until(&mut app, answer, |m| m["type"] == answer);
    }

    let now = fresh_snapshot(port);
    assert_eq!(now["grand_master"], before["grand_master"]);
    assert_eq!(now["outputs"], before["outputs"]);
    assert_eq!(now["show"]["name"], before["show"]["name"]);
    assert_eq!(now["programmer"], before["programmer"]);
    assert!(!dir.join("Shows/Other.cmdmx.json").exists());

    // The code of the first lock still holds, and after it commands run again.
    lock_set(&mut app, false, "9999");
    saw_warning(&mut app, "Wrong code");
    lock_set(&mut app, false, "1234");
    saw_locked(&mut app, false);
    send(&mut app, json!({"cmd": "grand_master.set", "level": 0.25}));
    until(&mut app, "the GM delta", |m| {
        m["type"] == "grand_master" && m["level"].as_f64() == Some(0.25)
    });
}

#[test]
fn ipc_50_a_wrong_code_stays_locked_the_same_code_and_the_master_code_unlock() {
    let dir = folder("ipc50-codes");
    let port = free_port();
    let _daemon = daemon(&dir, port);
    let mut app = connect(port);
    recv(&mut app);

    lock_set(&mut app, true, "1234");
    saw_locked(&mut app, true);
    lock_set(&mut app, false, "0000");
    saw_warning(&mut app, "Wrong code");
    lock_set(&mut app, false, "");
    saw_warning(&mut app, "Wrong code");
    assert_eq!(fresh_snapshot(port)["locked"], true);
    lock_set(&mut app, false, "68340");
    saw_locked(&mut app, false);

    // An empty code is a code.
    lock_set(&mut app, true, "");
    saw_locked(&mut app, true);
    lock_set(&mut app, false, "1");
    saw_warning(&mut app, "Wrong code");
    lock_set(&mut app, false, "");
    saw_locked(&mut app, false);

    // Unlocked, an unlock changes nothing and says nothing.
    lock_set(&mut app, false, "0000");
    send(&mut app, json!({"cmd": "grand_master.set", "level": 0.5}));
    let m = until(&mut app, "the GM delta, or a warning before it", |m| {
        (m["type"] == "grand_master" && m["level"].as_f64() == Some(0.5)) || m["type"] == "warning"
    });
    assert_ne!(m["type"], "warning", "{m}");
}

/// `127.0.0.1:6454`, shared with a wildcard socket on the same port if there is one (`olad`).
fn artnet_on_loopback() -> UdpSocket {
    // SAFETY: plain socket calls on a fresh fd; the fd is owned by the `UdpSocket` afterwards.
    unsafe {
        let fd = libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0);
        assert!(fd >= 0, "socket");
        let socket = UdpSocket::from_raw_fd(fd);
        let on: libc::c_int = 1;
        let rc = libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_REUSEADDR,
            std::ptr::addr_of!(on).cast(),
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        );
        assert_eq!(rc, 0, "SO_REUSEADDR");
        let mut addr: libc::sockaddr_in = std::mem::zeroed();
        addr.sin_family = libc::AF_INET as libc::sa_family_t;
        addr.sin_port = 6454u16.to_be();
        addr.sin_addr.s_addr = u32::from(std::net::Ipv4Addr::LOCALHOST).to_be();
        let rc = libc::bind(
            fd,
            std::ptr::addr_of!(addr).cast(),
            std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        );
        assert_eq!(rc, 0, "bind 127.0.0.1:6454 (a monitor running?)");
        socket
    }
}

/// Channel 4 of the next ArtDmx frame of universe 0:0:0.
fn channel_4(socket: &UdpSocket) -> u8 {
    let mut buf = [0u8; 600];
    loop {
        let n = socket.recv(&mut buf).expect("Art-Net frames stopped");
        if n >= 22
            && &buf[..8] == b"Art-Net\0"
            && buf[8..10] == [0x00, 0x50]
            && buf[14..16] == [0, 0]
        {
            return buf[21];
        }
    }
}

#[test]
fn ipc_50_a_fade_finishes_on_the_wire_while_locked() {
    let dir = folder("ipc50-fade");
    let artnet = artnet_on_loopback();
    artnet
        .set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();
    let port = free_port();
    let _daemon = daemon(&dir, port);
    let mut app = connect(port);
    let snapshot = recv(&mut app);
    let node = snapshot["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["kind"] == "artnet")
        .unwrap()["id"]
        .clone();
    send(
        &mut app,
        json!({"cmd": "output.edit", "id": node, "target": "127.0.0.1"}),
    );

    // A one-cue list on page 9 slot 1 (free in the demo show): dimmer 4 to full in 3 s.
    send(&mut app, json!({"cmd": "programmer.select", "fixture": 4}));
    send(
        &mut app,
        json!({"cmd": "programmer.set", "attribute": "Dimmer", "value": 1.0}),
    );
    send(
        &mut app,
        json!({"cmd": "playback.record", "page": 9, "slot": 1}),
    );
    send(&mut app, json!({"cmd": "programmer.clear"}));
    send(&mut app, json!({"cmd": "programmer.clear"}));
    let on_9_1 = |p: &Value| p["page"] == 9 && p["slot"] == 1;
    let stored = until(&mut app, "the recorded playback", |m| {
        m["playbacks"]
            .as_array()
            .is_some_and(|p| p.iter().any(on_9_1))
    });
    let list = stored["playbacks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| on_9_1(p))
        .unwrap()["id"]
        .clone();
    send(
        &mut app,
        json!({"cmd": "cue.times", "id": list, "cue": 1, "fade_in": 3.0}),
    );
    until(&mut app, "the cue's time", |m| {
        m["playbacks"]
            .as_array()
            .is_some_and(|p| p.iter().any(|p| p["cues"][0]["fade_in"] == 3.0))
    });

    // Fired, then locked straight away: the fade runs on under the lock.
    send(
        &mut app,
        json!({"cmd": "fader.set", "playback": list, "level": 1.0}),
    );
    lock_set(&mut app, true, "1234");
    saw_locked(&mut app, true);

    let deadline = Instant::now() + Duration::from_secs(6);
    let (mut between, mut full) = (false, false);
    while Instant::now() < deadline && !full {
        let level = channel_4(&artnet);
        between |= level > 0 && level < 255;
        full = level == 255;
    }
    assert!(between, "no frame between 0 and full");
    assert!(full, "the fade never finished");
    assert_eq!(fresh_snapshot(port)["locked"], true, "still locked");
}

#[test]
fn ipc_50_autosave_goes_on_the_lock_is_not_saved_and_a_restart_is_unlocked() {
    let dir = folder("ipc50-restart");
    let port = free_port();
    let first = daemon(&dir, port);
    let mut app = connect(port);
    recv(&mut app);
    // Show data changed just before the lock: the autosave has to write it under the lock.
    send(
        &mut app,
        json!({"cmd": "output.add", "line": 3, "kind": "artnet"}),
    );
    lock_set(&mut app, true, "1234");
    saw_locked(&mut app, true);
    // SHW-11: within 5 s, and the `show` delta names the write.
    until(&mut app, "an autosave under the lock", |m| {
        m["type"] == "show" && !m["saved_at"].is_null()
    });
    drop(app);
    drop(first);

    for entry in std::fs::read_dir(dir.join("Shows")).unwrap() {
        let text = std::fs::read_to_string(entry.unwrap().path()).unwrap_or_default();
        assert!(!text.contains("\"locked\""), "the lock is live state");
    }
    let _again = daemon(&dir, port);
    assert_eq!(fresh_snapshot(port)["locked"], false);
}

#[test]
fn ipc_50_osc_is_dropped_on_the_real_tick_without_a_warning() {
    let dir = folder("ipc50-osc");
    let port = free_port();
    let osc_port = free_port();
    let _daemon = daemon(&dir, port);
    let mut app = connect(port);
    recv(&mut app);
    send(
        &mut app,
        json!({"cmd": "control.osc", "enabled": true, "port": osc_port}),
    );
    // Give the OSC thread time to bind before the first packet.
    std::thread::sleep(Duration::from_millis(500));

    let gm = |level: f32| {
        encode(&OscMessage {
            address: "/cmdmx/gm".into(),
            args: vec![OscArg::Float(level)],
        })
    };
    let surface = UdpSocket::bind("127.0.0.1:0").unwrap();
    let daemon_at = ("127.0.0.1", osc_port);

    lock_set(&mut app, true, "1234");
    saw_locked(&mut app, true);
    surface.send_to(&gm(0.5), daemon_at).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let now = fresh_snapshot(port);
    assert_eq!(now["grand_master"], 1.0, "dropped");
    let said = now["messages"].to_string();
    assert!(!said.contains("Console is locked"), "no warning: {said}");

    lock_set(&mut app, false, "1234");
    saw_locked(&mut app, false);
    surface.send_to(&gm(0.5), daemon_at).unwrap();
    until(&mut app, "the GM delta from OSC", |m| {
        m["type"] == "grand_master" && m["level"].as_f64() == Some(0.5)
    });
}
