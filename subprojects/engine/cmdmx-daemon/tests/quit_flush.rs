//! SIGTERM writes pending show changes before the daemon exits (REL-14): `specs/modules/release.md`.
//!
//! Runs the real binary on a temp `HOME` and `--show-dir` with every output kind off, changes show
//! data (an Art-Net node's target, OUT-32), and signals it well inside the 5 s autosave window
//! (SHW-11). The show file must hold the change after the exit.

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tungstenite::{Message, WebSocket};

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

#[test]
fn rel_14_sigterm_flushes_pending_autosave() {
    let dir = std::env::temp_dir().join(format!("cmdmx-rel14-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let shows = dir.join("Shows");

    let port = free_port();
    let mut daemon = Command::new(env!("CARGO_BIN_EXE_cmdmx-daemon"))
        .env("HOME", &dir)
        .args(["--no-artnet", "--no-sacn", "--no-serial"])
        .args(["--ipc-port", &port.to_string()])
        .arg("--show-dir")
        .arg(&shows)
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(dir.join("stderr.log")).unwrap())
        .spawn()
        .unwrap();

    let mut app = connect(port);
    let snapshot = recv(&mut app);
    assert_eq!(snapshot["type"], "snapshot");
    let node = snapshot["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["kind"] == "artnet")
        .unwrap_or_else(|| panic!("no Art-Net node in {}", snapshot["outputs"]))["id"]
        .clone();
    let cmd = json!({"v": 1, "cmd": "output.edit", "id": node, "target": "10.9.8.7"});
    app.send(Message::Text(cmd.to_string().into())).unwrap();
    // Wait until the engine took the change, then signal at once (well inside 5 s).
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(Instant::now() < deadline, "the edit never came back");
        if recv(&mut app).to_string().contains("10.9.8.7") {
            break;
        }
    }

    // SAFETY: kill(2) on our own child's pid.
    let pid = i32::try_from(daemon.id()).unwrap();
    assert_eq!(unsafe { libc::kill(pid, libc::SIGTERM) }, 0);
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(s) = daemon.try_wait().unwrap() {
            break s;
        }
        if Instant::now() > deadline {
            let _ = daemon.kill();
            panic!("daemon did not exit within 5 s of SIGTERM");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(status.success(), "daemon exit: {status}");

    let saved: String = std::fs::read_dir(&shows)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.to_string_lossy().ends_with(".cmdmx.json"))
        .map(|p| std::fs::read_to_string(p).unwrap())
        .collect();
    assert!(
        saved.contains("10.9.8.7"),
        "the show file lacks the change made before SIGTERM"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
