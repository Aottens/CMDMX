//! The daemon binary's command line (SHW-18): `specs/modules/showfile.md`.
//!
//! Runs the real binary with `HOME` and `--show-dir` pointing at a fresh temp dir, plus flags
//! that make a regressed binary stop after one tick without network or serial output, so a
//! failure never touches the owner's shows folder.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::process::{Command, Output};

fn temp(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cmdmx-cli-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(dir: &PathBuf, first: &[&str]) -> Output {
    let show_dir = dir.join("Shows");
    Command::new(env!("CARGO_BIN_EXE_cmdmx-daemon"))
        .env("HOME", dir)
        .args(first)
        .args(["--ticks", "1", "--no-artnet", "--no-sacn", "--no-serial"])
        .args(["--ipc-port", "0", "--show-dir"])
        .arg(&show_dir)
        .output()
        .unwrap()
}

/// Nothing was created under the temp `HOME`: no shows folder, no state file.
fn untouched(dir: &PathBuf) -> bool {
    std::fs::read_dir(dir).unwrap().next().is_none()
}

#[test]
fn shw_18_help_prints_usage_and_opens_nothing() {
    for flag in ["--help", "-h"] {
        let dir = temp(flag.trim_start_matches('-'));
        let out = run(&dir, &[flag]);
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert_eq!(out.status.code(), Some(0), "{flag}");
        assert!(stdout.contains("--show-dir"), "{flag}: {stdout}");
        assert!(untouched(&dir), "{flag} touched {}", dir.display());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

#[test]
fn shw_18_unknown_argument_exits_2_and_opens_nothing() {
    for (name, bad) in [("unknown", "--bogus"), ("ticks", "--ticks=nope")] {
        let dir = temp(name);
        let out = run(&dir, &[bad]);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "{bad}");
        assert!(
            stderr.contains(&format!("cmdmx-daemon: unknown argument '{bad}'")),
            "{bad}: {stderr}"
        );
        assert!(
            stderr.contains("--show-dir"),
            "{bad}: usage missing: {stderr}"
        );
        assert!(untouched(&dir), "{bad} touched {}", dir.display());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

#[test]
fn shw_18_bad_ticks_value_exits_2_and_opens_nothing() {
    let dir = temp("ticks-value");
    let show_dir = dir.join("Shows");
    // `--ticks nope` last, so nothing after it could stop a regressed run.
    let mut child = Command::new(env!("CARGO_BIN_EXE_cmdmx-daemon"))
        .env("HOME", &dir)
        .args(["--no-artnet", "--no-sacn", "--no-serial", "--ipc-port", "0"])
        .arg("--show-dir")
        .arg(&show_dir)
        .args(["--ticks", "nope"])
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break s;
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_dir_all(&dir);
            panic!("--ticks nope kept the daemon running");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    assert_eq!(status.code(), Some(2));
    assert!(untouched(&dir), "--ticks nope touched {}", dir.display());
    std::fs::remove_dir_all(&dir).unwrap();
}
