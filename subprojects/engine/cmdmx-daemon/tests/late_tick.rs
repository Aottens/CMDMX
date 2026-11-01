//! A late tick does not catch up (OUT-40): `specs/modules/output-protocols.md`.
//!
//! Runs the real binary on a temp `HOME` and `--show-dir`, with only the demo show's sACN node on,
//! unicast to `127.0.0.1`, stops it for 3 s with `SIGSTOP` and counts the packets after `SIGCONT`.
//! The test binds `127.0.0.1:5568` with `SO_REUSEADDR`: a Mac running `olad` holds `*:5568`, and
//! a socket on the exact address may share the port with it and gets the unicast packets.

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::net::{TcpListener, TcpStream, UdpSocket};
use std::os::fd::FromRawFd;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tungstenite::{Message, WebSocket};

/// Kills the daemon when the test ends, passed or not (`SIGKILL` also ends a stopped process).
struct Daemon(Child);

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// `127.0.0.1:5568`, shared with a wildcard socket on the same port if there is one.
fn sacn_on_loopback() -> UdpSocket {
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
        addr.sin_port = 5568u16.to_be();
        addr.sin_addr.s_addr = u32::from(std::net::Ipv4Addr::LOCALHOST).to_be();
        let rc = libc::bind(
            fd,
            std::ptr::addr_of!(addr).cast(),
            std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        );
        assert_eq!(rc, 0, "bind 127.0.0.1:5568");
        socket
    }
}

fn connect(port: u16) -> WebSocket<TcpStream> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(stream) = TcpStream::connect(("127.0.0.1", port)) {
            return tungstenite::client(format!("ws://127.0.0.1:{port}/"), stream)
                .unwrap()
                .0;
        }
        assert!(Instant::now() < deadline, "daemon never listened on {port}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn snapshot(ws: &mut WebSocket<TcpStream>) -> Value {
    loop {
        if let Message::Text(t) = ws.read().unwrap() {
            let m: Value = serde_json::from_str(&t).unwrap();
            assert_eq!(m["type"], "snapshot");
            return m;
        }
    }
}

/// One sACN packet within the socket's read timeout.
fn sacn_packet(socket: &UdpSocket) -> bool {
    let mut buf = [0u8; 700];
    match socket.recv(&mut buf) {
        Ok(n) => n > 13 && &buf[4..13] == b"ASC-E1.17",
        Err(_) => false,
    }
}

fn signal(daemon: &Daemon, sig: libc::c_int) {
    // SAFETY: `kill` on the pid of our own child, which `Daemon` has not reaped yet.
    let rc = unsafe { libc::kill(daemon.0.id() as libc::pid_t, sig) };
    assert_eq!(rc, 0);
}

#[test]
fn out_40_after_sigstop_the_daemon_sends_at_most_20_packets_per_250_ms() {
    let dir = std::env::temp_dir().join(format!("cmdmx-out40-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let socket = sacn_on_loopback();

    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let daemon = Daemon(
        Command::new(env!("CARGO_BIN_EXE_cmdmx-daemon"))
            .env("HOME", &dir)
            .args([
                "--no-artnet",
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
    );

    let mut app = connect(port);
    let outputs = snapshot(&mut app)["outputs"].clone();
    let node = outputs
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["kind"] == "sacn")
        .unwrap_or_else(|| panic!("no sACN node in {outputs}"))["id"]
        .clone();
    let edit = json!({"v": 1, "cmd": "output.edit", "id": node, "unicast": "127.0.0.1"});
    app.send(Message::Text(edit.to_string().into())).unwrap();

    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    assert!(sacn_packet(&socket), "no sACN packet on loopback");

    signal(&daemon, libc::SIGSTOP);
    std::thread::sleep(Duration::from_secs(3));
    // What was sent before the stop does not count.
    socket.set_nonblocking(true).unwrap();
    let mut buf = [0u8; 700];
    while socket.recv(&mut buf).is_ok() {}
    socket.set_nonblocking(false).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_millis(20)))
        .unwrap();

    signal(&daemon, libc::SIGCONT);
    let resumed = Instant::now();
    let mut windows = [0u32; 4];
    while resumed.elapsed() < Duration::from_secs(1) {
        if sacn_packet(&socket) {
            let at = resumed.elapsed().as_millis() as usize / 250;
            windows[at.min(3)] += 1;
        }
    }
    assert!(
        windows.iter().all(|&n| n <= 20),
        "packets per 250 ms after SIGCONT: {windows:?}"
    );
    assert!(
        windows.iter().sum::<u32>() >= 10,
        "output did not come back: {windows:?}"
    );
}
