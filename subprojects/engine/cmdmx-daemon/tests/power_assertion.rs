//! The daemon keeps the Mac awake (OUT-38): `specs/modules/output-protocols.md`.
//!
//! macOS only, so this runs locally (Rust CI is on Linux). Runs the real binary on a temp `HOME`
//! and `--show-dir` with every output kind off and reads `pmset -g assertions`.

#![cfg(target_os = "macos")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Whether `pmset` lists the assertion under this pid.
fn listed(pid: u32) -> bool {
    let out = Command::new("pmset")
        .args(["-g", "assertions"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).lines().any(|l| {
        l.contains(&format!("pid {pid}("))
            && l.contains("PreventUserIdleSystemSleep")
            && l.contains("named: \"CMDMX DMX output\"")
    })
}

fn within_5s(mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

/// With every output kind off the assertion is there; after `signal` it is gone.
fn held_until_exit(signal: &str) {
    let name = format!("cmdmx-out38-{}{signal}", std::process::id());
    let dir = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let mut daemon = Command::new(env!("CARGO_BIN_EXE_cmdmx-daemon"))
        .env("HOME", &dir)
        .args(["--no-artnet", "--no-sacn", "--no-serial"])
        .args(["--ipc-port", &port.to_string()])
        .arg("--show-dir")
        .arg(dir.join("Shows"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let pid = daemon.id();

    // The port opens before the first tick and after the assertion: it must be there at once.
    assert!(
        within_5s(|| TcpStream::connect(("127.0.0.1", port)).is_ok()),
        "the daemon never listened"
    );
    let held = listed(pid);

    let killed = Command::new("kill")
        .args([signal, &pid.to_string()])
        .status()
        .unwrap();
    assert!(killed.success());
    daemon.wait().unwrap();
    let released = within_5s(|| !listed(pid));

    let _ = std::fs::remove_dir_all(&dir);
    assert!(held, "pmset does not list the assertion under pid {pid}");
    assert!(released, "the assertion outlived pid {pid} ({signal})");
}

#[test]
fn out_38_held_while_running_and_gone_after_sigterm() {
    held_until_exit("-TERM");
}

#[test]
fn out_38_kill_9_leaves_nothing_behind() {
    held_until_exit("-KILL");
}
