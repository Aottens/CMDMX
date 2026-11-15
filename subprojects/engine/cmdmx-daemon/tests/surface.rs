//! Surface keys reach the app (IPC-64, CTL-72) on the real tick: two clients, one following, and an
//! OSC sender as the surface. Runs the real binary on a temp `HOME` and `--show-dir`; the control
//! layer's side is in `src/control/tests.rs`.

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::net::{TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{Duration, Instant};

use cmdmx_control::osc::{encode, OscMessage};
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

/// Below the ephemeral ranges and never bound here. A port the kernel hands out can become another
/// test's client source port before the daemon binds it, and a socket bound to probe one can be
/// inherited by a daemon another thread forks. Either way this daemon exits and the handshake is
/// reset. A connect finds a port another program on this machine listens on.
fn free_port() -> u16 {
    static NEXT: AtomicU16 = AtomicU16::new(0);
    let base = 20_000 + (std::process::id() % 10_000) as u16;
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
            // Longer than `until`'s deadline: an autosave can take the whole 5 s of SHW-11 and more.
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

/// The daemon answered a later request and no `surface` message came before it.
fn no_surface(ws: &mut WebSocket<TcpStream>) {
    send(ws, json!({"cmd": "output.ports"}));
    until(ws, "the ports, or a surface message before them", |m| {
        assert_ne!(m["type"], "surface", "{m}");
        m["type"] == "ports"
    });
}

/// A button press on `/cmdmx/<path>`, with no argument (a tap, CTL-36).
fn tap(surface: &UdpSocket, osc_port: u16, path: &str) {
    let bytes = encode(&OscMessage {
        address: format!("/cmdmx/{path}"),
        args: vec![],
    });
    surface.send_to(&bytes, ("127.0.0.1", osc_port)).unwrap();
}

#[test]
fn ipc_64_only_the_following_client_gets_surface_keys_and_a_gone_app_refuses_them() {
    let dir = folder("ipc64-follow");
    let port = free_port();
    let osc_port = free_port();
    let _daemon = daemon(&dir, port);
    let mut app = connect(port);
    recv(&mut app);
    let mut other = connect(port);
    recv(&mut other);
    send(
        &mut app,
        json!({"cmd": "control.osc", "enabled": true, "port": osc_port}),
    );
    send(&mut app, json!({"cmd": "surface.follow", "on": true}));
    // Give the OSC thread time to bind before the first packet.
    std::thread::sleep(Duration::from_millis(500));
    let surface = UdpSocket::bind("127.0.0.1:0").unwrap();

    tap(&surface, osc_port, "keypad/4");
    let m = until(&mut app, "the keypad token", |m| m["type"] == "surface");
    assert_eq!(m["device"], "OSC");
    assert_eq!(m["keypad"], "4");
    tap(&surface, osc_port, "key/record");
    let m = until(&mut app, "the Record key", |m| m["type"] == "surface");
    assert_eq!(m["key"], "record");
    no_surface(&mut other);

    // Armed in the app: a group press is the target, not a selection.
    send(
        &mut app,
        json!({"cmd": "surface.armed", "function": "record"}),
    );
    std::thread::sleep(Duration::from_millis(200));
    tap(&surface, osc_port, "group/5");
    let m = until(&mut app, "the group tap", |m| m["type"] == "surface");
    assert_eq!(m["group"], 5);
    no_surface(&mut other);

    // Surface events are not in the message history.
    let mut later = connect(port);
    let snap = recv(&mut later);
    assert!(!snap["messages"].to_string().contains("surface"), "{snap}");

    // The app goes: it stops following and stops being armed, so Record is refused again.
    drop(app);
    std::thread::sleep(Duration::from_millis(300));
    tap(&surface, osc_port, "key/record");
    until(&mut other, "the refusal", |m| {
        m["type"] == "warning" && m["message"] == "Record on OSC needs the CMDMX app"
    });
}
