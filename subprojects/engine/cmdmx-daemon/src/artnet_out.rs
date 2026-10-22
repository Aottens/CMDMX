//! Art-Net output node (OUT-03/04); threading and health live in `output`.

use std::io;
use std::net::{SocketAddrV4, UdpSocket};

use cmdmx_protocols::{artnet, ArtSequence};

use crate::output::{open_udp, Health, OutputState};

pub struct ArtNetNode {
    /// `art-net #3`, the prefix of every log line (OUT-30).
    name: String,
    target: SocketAddrV4,
    port_address: u16,
    sequence: ArtSequence,
    socket: Option<UdpSocket>,
    health: Health,
}

impl ArtNetNode {
    pub fn new(name: String, target: SocketAddrV4, port_address: u16) -> Self {
        ArtNetNode {
            name,
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
            .map(|m| format!("{}: {m}", self.name))
    }

    /// OUT-33: why the node is degraded.
    pub fn detail(&self) -> Option<&str> {
        self.health.detail()
    }

    /// IPC-20.
    pub fn state(&self) -> OutputState {
        self.health.state()
    }

    fn try_send(&mut self, data: &[u8]) -> io::Result<()> {
        let packet = artnet::art_dmx(self.port_address, self.sequence.advance(), 0, data)
            .map_err(io::Error::other)?;
        let socket = match self.socket.take() {
            Some(s) => s,
            None => {
                let s = open_udp()?;
                s.set_broadcast(true)?;
                s
            }
        };
        let sent = socket.send_to(&packet, self.target);
        self.socket = Some(socket);
        sent.map(|_| ())
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
    use std::time::Duration;

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
        let mut node = ArtNetNode::new("art-net #1".into(), target, 0);
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
        let mut node = ArtNetNode::new("art-net #1".into(), target, 0);
        let (tx, _status) = crate::output::spawn("art-net", move |f| (node.send(f), node.state()));
        for v in [1u8, 2, 3] {
            assert!(crate::output::publish(&tx, &[v; 512]));
            assert_eq!(recv(&rx)[18], v);
        }
    }

    #[test]
    fn out_04_socket_allows_broadcast_and_uses_ephemeral_source_port() {
        let (rx, target) = receiver();
        let mut node = ArtNetNode::new("art-net #1".into(), target, 0);
        assert_eq!(node.send(&[0; 512]), None);
        let sock = node.socket.as_ref().unwrap();
        assert!(sock.broadcast().unwrap());
        let mut buf = [0u8; 1024];
        let (_, from) = rx.recv_from(&mut buf).unwrap();
        assert_ne!(from.port(), cmdmx_protocols::artnet::PORT);
    }
}
