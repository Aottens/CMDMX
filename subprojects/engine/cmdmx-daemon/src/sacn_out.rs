//! sACN (E1.31) output node (OUT-06/07/17); threading and health live in `output`.

use std::io;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};

use cmdmx_protocols::sacn;

use crate::output::{open_udp, Health};

pub const SOURCE_NAME: &str = "ColourmotionDMX";

/// OUT-17: fixed CID c0105e00-d3a0-4c00-8000-000000000001 until the show
/// file stores a generated one (SHW).
pub const CID: [u8; 16] = [
    0xc0, 0x10, 0x5e, 0x00, 0xd3, 0xa0, 0x4c, 0x00, 0x80, 0x00, 0, 0, 0, 0, 0, 1,
];

/// OUT-06: 239.255.hi.lo:5568.
pub fn multicast_target(universe: u16) -> SocketAddrV4 {
    SocketAddrV4::new(
        Ipv4Addr::from(sacn::multicast_address(universe)),
        sacn::PORT,
    )
}

pub struct SacnNode {
    target: SocketAddrV4,
    universe: u16,
    priority: u8,
    sequence: u8,
    socket: Option<UdpSocket>,
    health: Health,
}

impl SacnNode {
    pub fn new(target: SocketAddrV4, universe: u16, priority: u8) -> Self {
        SacnNode {
            target,
            universe,
            priority,
            sequence: 0,
            socket: None,
            health: Health::default(),
        }
    }

    /// Send one frame; returns a log line on a state change (OUT-12).
    pub fn send(&mut self, data: &[u8]) -> Option<String> {
        let result = self.try_send(data);
        if result.is_err() {
            self.socket = None;
        }
        self.health
            .record(&result)
            .map(|m| format!("sacn {}: {m}", self.target))
    }

    fn try_send(&mut self, data: &[u8]) -> io::Result<()> {
        let packet = sacn::data_packet(
            &CID,
            SOURCE_NAME,
            self.priority,
            self.sequence,
            self.universe,
            data,
        )
        .map_err(io::Error::other)?;
        self.sequence = self.sequence.wrapping_add(1);
        let socket = match self.socket.take() {
            Some(s) => s,
            None => {
                let s = open_udp()?;
                s.set_multicast_loop_v4(true)?;
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
    fn out_06_default_target_is_multicast_group_of_the_universe() {
        assert_eq!(
            multicast_target(1),
            SocketAddrV4::new(Ipv4Addr::new(239, 255, 0, 1), 5568)
        );
        assert_eq!(
            multicast_target(0x0102),
            SocketAddrV4::new(Ipv4Addr::new(239, 255, 1, 2), 5568)
        );
    }

    #[test]
    fn out_06_each_frame_becomes_one_packet_with_sequence_from_zero() {
        let (rx, target) = receiver();
        let mut node = SacnNode::new(target, 1, 100);
        let mut frame = [0u8; 512];
        for seq in 0..3u8 {
            frame[0] = seq + 7;
            assert_eq!(node.send(&frame), None);
            let p = recv(&rx);
            assert_eq!(p.len(), 126 + 512);
            assert_eq!(p[111], seq, "sequence");
            assert_eq!(&p[113..115], &[0, 1], "universe 1");
            assert_eq!(p[126], seq + 7);
        }
    }

    #[test]
    fn out_06_socket_loops_multicast_back_and_uses_ephemeral_port() {
        let (rx, target) = receiver();
        let mut node = SacnNode::new(target, 1, 100);
        assert_eq!(node.send(&[0; 512]), None);
        assert!(node.socket.as_ref().unwrap().multicast_loop_v4().unwrap());
        let mut buf = [0u8; 1024];
        let (_, from) = rx.recv_from(&mut buf).unwrap();
        assert_ne!(from.port(), cmdmx_protocols::sacn::PORT);
    }

    #[test]
    fn out_07_priority_is_carried_per_node() {
        let (rx, target) = receiver();
        let mut node = SacnNode::new(target, 1, 150);
        node.send(&[0; 512]);
        assert_eq!(recv(&rx)[108], 150);
    }

    #[test]
    fn out_17_source_name_and_fixed_cid() {
        let (rx, target) = receiver();
        let mut node = SacnNode::new(target, 1, 100);
        node.send(&[0; 512]);
        let p = recv(&rx);
        assert_eq!(
            &p[22..38],
            &[0xc0, 0x10, 0x5e, 0x00, 0xd3, 0xa0, 0x4c, 0x00, 0x80, 0x00, 0, 0, 0, 0, 0, 1]
        );
        assert_eq!(&p[44..60], b"ColourmotionDMX\0");
    }
}
