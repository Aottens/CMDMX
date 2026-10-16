//! Art-Net output node and its thread (OUT-03/04/11/12).

use std::io;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};

use cmdmx_core::UNIVERSE_SIZE;
use cmdmx_protocols::{artnet, ArtSequence};

pub type FrameData = [u8; UNIVERSE_SIZE];

/// OUT-04: limited broadcast on 6454 unless `--artnet-target` says otherwise.
pub fn default_target() -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::BROADCAST, artnet::PORT)
}

/// OUT-12: node state; a message is returned only when the state flips.
#[derive(Debug, Default)]
pub struct Health {
    degraded: bool,
}

impl Health {
    pub fn record(&mut self, result: &io::Result<()>) -> Option<String> {
        match (self.degraded, result) {
            (false, Err(e)) => {
                self.degraded = true;
                Some(format!("Degraded: {e}"))
            }
            (true, Ok(())) => {
                self.degraded = false;
                Some("Ok again".into())
            }
            _ => None,
        }
    }
}

pub struct ArtNetNode {
    target: SocketAddrV4,
    port_address: u16,
    sequence: ArtSequence,
    socket: Option<UdpSocket>,
    health: Health,
}

impl ArtNetNode {
    pub fn new(target: SocketAddrV4, port_address: u16) -> Self {
        ArtNetNode {
            target,
            port_address,
            sequence: ArtSequence::default(),
            socket: None,
            health: Health::default(),
        }
    }

    /// Send one frame; returns a log line on a state change (OUT-12).
    /// A missing or failed socket is reopened on the next call.
    pub fn send(&mut self, data: &[u8]) -> Option<String> {
        let result = self.try_send(data);
        if result.is_err() {
            self.socket = None;
        }
        self.health
            .record(&result)
            .map(|m| format!("art-net {}: {m}", self.target))
    }

    fn try_send(&mut self, data: &[u8]) -> io::Result<()> {
        let packet = artnet::art_dmx(self.port_address, self.sequence.advance(), 0, data)
            .map_err(io::Error::other)?;
        let socket = match self.socket.take() {
            Some(s) => s,
            None => open_socket()?,
        };
        let sent = socket.send_to(&packet, self.target);
        self.socket = Some(socket);
        sent.map(|_| ())
    }
}

/// Ephemeral source port so a local monitor can own 6454 (OUT-04).
fn open_socket() -> io::Result<UdpSocket> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    socket.set_broadcast(true)?;
    Ok(socket)
}

/// OUT-11: run the node on its own thread; panics are caught per frame.
pub fn spawn(mut node: ArtNetNode) -> SyncSender<FrameData> {
    let (tx, rx) = sync_channel(1);
    std::thread::spawn(move || run(&mut node, &rx));
    tx
}

fn run(node: &mut ArtNetNode, rx: &Receiver<FrameData>) {
    while let Ok(frame) = rx.recv() {
        match catch_unwind(AssertUnwindSafe(|| node.send(&frame))) {
            Ok(Some(msg)) => eprintln!("{msg}"),
            Ok(None) => {}
            Err(_) => eprintln!("art-net output: panic caught, continuing"),
        }
    }
}

/// OUT-11: hand a frame to the output thread without blocking. A full
/// channel drops this frame; `false` means the thread is gone.
pub fn publish(tx: &SyncSender<FrameData>, frame: &FrameData) -> bool {
    !matches!(tx.try_send(*frame), Err(TrySendError::Disconnected(_)))
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io;
    use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
    use std::sync::mpsc::sync_channel;
    use std::time::{Duration, Instant};

    fn receiver() -> (UdpSocket, SocketAddrV4) {
        let rx = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        rx.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let port = rx.local_addr().unwrap().port();
        (rx, SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
    }

    fn recv(rx: &UdpSocket) -> Vec<u8> {
        let mut buf = [0u8; 1024];
        let n = rx.recv(&mut buf).unwrap();
        buf[..n].to_vec()
    }

    #[test]
    fn out_03_each_frame_becomes_one_artdmx_with_rising_sequence() {
        let (rx, target) = receiver();
        let mut node = ArtNetNode::new(target, 0);
        let mut frame = [0u8; 512];
        for seq in 1..=3u8 {
            frame[0] = seq * 10;
            assert_eq!(node.send(&frame), None);
            let p = recv(&rx);
            assert_eq!(&p[0..8], b"Art-Net\0");
            assert_eq!(p[12], seq, "sequence per node (OUT-02)");
            assert_eq!(&p[14..16], &[0, 0], "port-address 0");
            assert_eq!(p.len(), 18 + 512);
            assert_eq!(p[18], seq * 10);
        }
    }

    #[test]
    fn out_03_thread_sends_every_published_frame() {
        let (rx, target) = receiver();
        let tx = spawn(ArtNetNode::new(target, 0));
        for v in [1u8, 2, 3] {
            assert!(publish(&tx, &[v; 512]));
            assert_eq!(recv(&rx)[18], v);
        }
    }

    #[test]
    fn out_04_socket_allows_broadcast_and_uses_ephemeral_source_port() {
        let (rx, target) = receiver();
        let mut node = ArtNetNode::new(target, 0);
        assert_eq!(node.send(&[0; 512]), None);
        let sock = node.socket.as_ref().unwrap();
        assert!(sock.broadcast().unwrap());
        let mut buf = [0u8; 1024];
        let (_, from) = rx.recv_from(&mut buf).unwrap();
        assert_ne!(from.port(), cmdmx_protocols::artnet::PORT);
    }

    #[test]
    fn out_04_default_target_is_limited_broadcast_on_6454() {
        assert_eq!(
            default_target(),
            SocketAddrV4::new(Ipv4Addr::BROADCAST, 6454)
        );
    }

    #[test]
    fn out_11_publish_never_blocks_when_output_is_not_consuming() {
        let (tx, _rx) = sync_channel(1);
        let start = Instant::now();
        for _ in 0..1000 {
            assert!(publish(&tx, &[0; 512]));
        }
        assert!(start.elapsed() < Duration::from_millis(100));
    }

    #[test]
    fn out_11_publish_reports_a_dead_output_thread() {
        let (tx, rx) = sync_channel(1);
        drop(rx);
        assert!(!publish(&tx, &[0; 512]));
    }

    #[test]
    fn out_12_logs_once_per_state_change() {
        let err = || Err(io::Error::other("no route"));
        let mut h = Health::default();
        assert_eq!(h.record(&Ok(())), None);
        assert!(h.record(&err()).unwrap().contains("Degraded"));
        assert_eq!(h.record(&err()), None, "still degraded: silent");
        assert!(h.record(&Ok(())).unwrap().contains("Ok"));
        assert_eq!(h.record(&Ok(())), None);
    }
}
