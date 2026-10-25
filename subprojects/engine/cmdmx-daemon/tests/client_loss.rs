//! Losing a client changes nothing but that connection (IPC-44, ADR-002): `specs/modules/ipc.md`.
//!
//! Runs the real binary on a temp `HOME` and `--show-dir`, without sACN and serial, with the demo
//! show's Art-Net node pointed at `127.0.0.1` (OUT-04: the daemon sends from an ephemeral port, so
//! this test owns the protocol port on loopback). A client sets a live look, vanishes without a
//! close handshake, and the frames must run on unchanged; a new client must see the look.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::net::{TcpListener, TcpStream, UdpSocket};
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

fn send(ws: &mut WebSocket<TcpStream>, cmd: Value) {
    let mut cmd = cmd;
    cmd["v"] = json!(0);
    ws.send(Message::Text(cmd.to_string().into())).unwrap();
}

/// Reads messages until `pred` holds for one (an `error` or `warning` fails the test at once).
fn until(ws: &mut WebSocket<TcpStream>, what: &str, pred: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(Instant::now() < deadline, "never saw {what}");
        let m = recv(ws);
        assert!(
            m["type"] != "error" && m["type"] != "warning",
            "while waiting for {what}: {m}"
        );
        if pred(&m) {
            return m;
        }
    }
}

/// One ArtDmx packet: (sequence, the 512 channels of universe 0:0:0).
fn art_dmx(socket: &UdpSocket) -> (u8, Vec<u8>) {
    let mut buf = [0u8; 600];
    loop {
        let n = socket.recv(&mut buf).expect("Art-Net frames stopped");
        // "Art-Net\0", OpDmx 0x5000 little-endian, port-address 0 at 14..16.
        if n >= 18
            && &buf[..8] == b"Art-Net\0"
            && buf[8..10] == [0x00, 0x50]
            && buf[14..16] == [0, 0]
        {
            let len = usize::from(u16::from_be_bytes([buf[16], buf[17]]));
            return (buf[12], buf[18..18 + len.min(n - 18)].to_vec());
        }
    }
}

/// Frames for `secs`: every frame equal to `look`, the sequence without a gap (1..=255, 0 never).
fn frames_hold(socket: &UdpSocket, look: &[u8], mut last: u8, secs: f64) -> (u8, usize) {
    let start = Instant::now();
    let mut count = 0;
    while start.elapsed().as_secs_f64() < secs {
        let (seq, data) = art_dmx(socket);
        let want = if last == 255 { 1 } else { last + 1 };
        assert_eq!(seq, want, "frame skipped after sequence {last}");
        assert_eq!(&data[..], look, "frame {seq} changed");
        last = seq;
        count += 1;
    }
    (last, count)
}

#[test]
fn ipc_44_killed_client_changes_nothing_and_a_new_client_sees_the_live_state() {
    let dir = std::env::temp_dir().join(format!("cmdmx-ipc44-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let artnet = UdpSocket::bind(("127.0.0.1", 6454))
        .expect("Art-Net port 6454 on loopback is taken (a monitor running?)");
    artnet
        .set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();

    let port = free_port();
    let _daemon = Daemon(
        Command::new(env!("CARGO_BIN_EXE_cmdmx-daemon"))
            .env("HOME", &dir)
            .args(["--no-sacn", "--no-serial", "--ipc-port", &port.to_string()])
            .arg("--show-dir")
            .arg(dir.join("Shows"))
            .stdout(Stdio::null())
            .stderr(std::fs::File::create(dir.join("stderr.log")).unwrap())
            .spawn()
            .unwrap(),
    );

    // The first client: points Art-Net at loopback and sets a live look.
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
    send(
        &mut app,
        json!({"cmd": "output.edit", "id": node, "target": "127.0.0.1"}),
    );
    // A fader: playback 2 → dimmer 2 at channel 2 (IPC-09).
    send(
        &mut app,
        json!({"cmd": "fader.set", "playback": 2, "level": 0.6}),
    );
    // A cue list on page 9 slot 1 (free in the demo show): dimmer 4 at full in Q1, half in Q2.
    send(&mut app, json!({"cmd": "programmer.select", "fixture": 4}));
    send(
        &mut app,
        json!({"cmd": "programmer.set", "attribute": "Dimmer", "value": 1.0}),
    );
    send(
        &mut app,
        json!({"cmd": "playback.record", "page": 9, "slot": 1}),
    );
    send(
        &mut app,
        json!({"cmd": "programmer.set", "attribute": "Dimmer", "value": 0.5}),
    );
    send(
        &mut app,
        json!({"cmd": "playback.record", "page": 9, "slot": 1, "mode": "Append"}),
    );
    // Clear twice: the first deselects, the second empties the programmer (PRG clear precedence).
    send(&mut app, json!({"cmd": "programmer.clear"}));
    send(&mut app, json!({"cmd": "programmer.clear"}));
    let list = until(&mut app, "the recorded list with two cues", |m| {
        m["playbacks"].as_array().is_some_and(|p| {
            p.iter()
                .any(|p| p["cues"].as_array().is_some_and(|c| c.len() == 2))
        })
    })["playbacks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["cues"].as_array().is_some_and(|c| c.len() == 2))
        .unwrap()["id"]
        .clone();
    // The fader up fires the list at Q1 (CUE-16) and is its intensity master (CUE-15).
    send(
        &mut app,
        json!({"cmd": "fader.set", "playback": list, "level": 1.0}),
    );
    // The programmer: dimmer 3 at 40 %.
    send(&mut app, json!({"cmd": "programmer.select", "fixture": 3}));
    send(
        &mut app,
        json!({"cmd": "programmer.set", "attribute": "Dimmer", "value": 0.4}),
    );

    // Wait for the look on the wire (cue fades done), then take it as the reference.
    let deadline = Instant::now() + Duration::from_secs(10);
    let (mut last, look) = loop {
        assert!(Instant::now() < deadline, "the look never reached Art-Net");
        let (seq, data) = art_dmx(&artnet);
        if data.len() == 512 && data[1] == 153 && data[2] == 102 && data[3] == 255 {
            break (seq, data);
        }
    };
    (last, _) = frames_hold(&artnet, &look, last, 0.5);

    // Gone without a close frame: unread data in the socket makes the kernel reset it, as when
    // the app is killed with `kill -9`. The library thread's start-up lines and the once-a-second
    // channel-1 report can land at any moment (a full `vendor/qlcplus` takes seconds), so they
    // are not counted.
    let log_lines = || {
        std::fs::read_to_string(dir.join("stderr.log"))
            .unwrap()
            .lines()
            .filter(|l| !l.starts_with("library: ") && !l.starts_with("universe "))
            .count()
    };
    let before = log_lines();
    app.get_mut().shutdown(std::net::Shutdown::Both).unwrap();
    drop(app);

    let (last, count) = frames_hold(&artnet, &look, last, 2.0);
    assert!(
        count >= 72,
        "{count} frames in 2 s, expected 40 Hz (OUT-03)"
    );
    assert!(
        log_lines() <= before + 1,
        "more than one log line for the lost client"
    );

    // A new client sees the live state.
    let mut again = connect(port);
    let snap = recv(&mut again);
    assert_eq!(snap["type"], "snapshot");
    let playback = |id: &Value| {
        snap["playbacks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| &p["id"] == id)
            .unwrap_or_else(|| panic!("playback {id} missing from {}", snap["playbacks"]))
            .clone()
    };
    let fader = playback(&json!(2));
    assert!(
        (fader["level"].as_f64().unwrap() - 0.6).abs() < 1e-6,
        "{fader}"
    );
    assert_eq!(fader["state"], "active", "{fader}");
    let cues = playback(&list);
    assert_eq!(cues["cue"]["current"], 1.0, "{cues}");
    assert_eq!(
        snap["programmer"]["in_programmer"],
        json!([3]),
        "{}",
        snap["programmer"]
    );
    assert_eq!(
        snap["programmer"]["selection"],
        json!([3]),
        "{}",
        snap["programmer"]
    );

    // And the new client changed nothing either.
    frames_hold(&artnet, &look, last, 0.5);
    let _ = std::fs::remove_dir_all(&dir);
}
