//! The OSC UDP socket on its own thread (VS7-03, CTL-14, 34, 35). It only moves datagrams; the
//! tick thread decodes them (`control::osc`). Both channels are unbounded, so nothing here can
//! block the tick.

use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::time::Duration;

/// From the socket thread to the tick.
#[derive(Debug)]
pub enum OscIn {
    Packet {
        from: SocketAddr,
        bytes: Vec<u8>,
    },
    /// The answer to every `Listen(Some(_))`: the port actually bound, or why not.
    Listening(Result<u16, String>),
}

/// From the tick to the socket thread.
#[derive(Debug)]
pub enum OscCmd {
    /// (Re)bind on all interfaces (port 0 = any free port); `None` closes the socket.
    Listen(Option<u16>),
    /// Feedback, sent from the bound socket; ignored while closed.
    Send { to: SocketAddr, bytes: Vec<u8> },
}

/// How long a read waits before the thread looks at its commands again.
const POLL: Duration = Duration::from_millis(5);
/// The largest UDP datagram.
const MAX_PACKET: usize = 65_536;

/// Start the OSC thread; it ends when the returned sender is dropped.
pub fn spawn(events: Sender<OscIn>) -> io::Result<Sender<OscCmd>> {
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("osc".into())
        .spawn(move || {
            // A panic here ends OSC, never the daemon.
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&events, &rx))).is_err()
            {
                eprintln!("osc: thread panicked; OSC is off");
            }
        })?;
    Ok(tx)
}

fn bind(port: u16) -> Result<(UdpSocket, u16), String> {
    let socket = UdpSocket::bind(("0.0.0.0", port)).map_err(|e| e.to_string())?;
    socket
        .set_read_timeout(Some(POLL))
        .map_err(|e| e.to_string())?;
    let port = socket.local_addr().map_err(|e| e.to_string())?.port();
    Ok((socket, port))
}

fn run(events: &Sender<OscIn>, rx: &Receiver<OscCmd>) {
    let mut socket: Option<UdpSocket> = None;
    let mut buf = vec![0u8; MAX_PACKET];
    loop {
        // Commands first; while closed there is nothing else to wait for.
        loop {
            let next = match socket {
                Some(_) => rx.try_recv(),
                None => rx.recv().map_err(|_| TryRecvError::Disconnected),
            };
            match next {
                Ok(OscCmd::Listen(port)) => {
                    socket = None;
                    if let Some(port) = port {
                        let answer = bind(port).map(|(s, p)| {
                            socket = Some(s);
                            p
                        });
                        if events.send(OscIn::Listening(answer)).is_err() {
                            return;
                        }
                    }
                }
                Ok(OscCmd::Send { to, bytes }) => {
                    if let Some(s) = &socket {
                        let _ = s.send_to(&bytes, to);
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        let Some(s) = &socket else { continue };
        match s.recv_from(&mut buf) {
            Ok((n, from)) => {
                let bytes = buf.get(..n).unwrap_or_default().to_vec();
                if events.send(OscIn::Packet { from, bytes }).is_err() {
                    return;
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            // Do not spin on a socket that keeps failing.
            Err(_) => std::thread::sleep(POLL),
        }
    }
}
