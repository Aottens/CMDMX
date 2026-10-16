//! Art-Net 4 — ArtDmx packet (OpCode 0x5000).
//!
//! Layout (Art-Net 4 specification, "ArtDmx"):
//! ```text
//!  0..8   ID        "Art-Net\0"
//!  8..10  OpCode    0x5000 little-endian
//! 10..12  ProtVer   14, big-endian
//! 12      Sequence  1..=255, 0 = disabled
//! 13      Physical  input port, informational
//! 14      SubUni    low 8 bits of the 15-bit port-address (sub-net + universe)
//! 15      Net       high 7 bits of the port-address
//! 16..18  Length    even, 2..=512, big-endian
//! 18..    Data
//! ```
//! Port-address is 15 bits: Net (7) | Sub-Net (4) | Universe (4). We take it
//! as one `u16` so callers can map "universe 1" however their node config
//! says (Titan defaults line n → port-address n-1).

use crate::{FrameError, MAX_CHANNELS};

pub const PORT: u16 = 0x1936; // 6454
pub const ID: &[u8; 8] = b"Art-Net\0";
pub const OP_DMX: u16 = 0x5000;
pub const PROTOCOL_VERSION: u16 = 14;
pub const HEADER_LEN: usize = 18;

/// Build an ArtDmx packet. `data` is the 512-slot frame without start code;
/// shorter frames are padded to an even length of at least 2.
pub fn art_dmx(
    port_address: u16,
    sequence: u8,
    physical: u8,
    data: &[u8],
) -> Result<Vec<u8>, FrameError> {
    if data.len() > MAX_CHANNELS {
        return Err(FrameError::TooManyChannels(data.len()));
    }
    if port_address > 0x7FFF {
        return Err(FrameError::UniverseOutOfRange(port_address));
    }
    let mut length = data.len().max(2);
    if length % 2 == 1 {
        length += 1;
    }

    let mut packet = Vec::with_capacity(HEADER_LEN + length);
    packet.extend_from_slice(ID);
    packet.extend_from_slice(&OP_DMX.to_le_bytes());
    packet.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    packet.push(sequence);
    packet.push(physical);
    packet.push((port_address & 0x00FF) as u8);
    packet.push(((port_address >> 8) & 0x7F) as u8);
    packet.extend_from_slice(&(length as u16).to_be_bytes());
    packet.extend_from_slice(data);
    packet.resize(HEADER_LEN + length, 0);
    Ok(packet)
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn out_01_header_is_byte_exact_for_full_frame_on_port_address_zero() {
        let data = [0x7Fu8; 512];
        let p = art_dmx(0, 1, 0, &data).unwrap();
        assert_eq!(&p[0..8], b"Art-Net\0");
        assert_eq!(&p[8..10], &[0x00, 0x50]); // 0x5000 LE
        assert_eq!(&p[10..12], &[0x00, 0x0E]); // ProtVer 14 BE
        assert_eq!(p[12], 1); // sequence
        assert_eq!(p[13], 0); // physical
        assert_eq!(p[14], 0); // SubUni
        assert_eq!(p[15], 0); // Net
        assert_eq!(&p[16..18], &[0x02, 0x00]); // length 512 BE
        assert_eq!(p.len(), 18 + 512);
        assert!(p[18..].iter().all(|&b| b == 0x7F));
    }

    #[test]
    fn out_01_port_address_splits_into_subuni_and_net() {
        // Net 1, Sub-Net 2, Universe 3 → 0b0000001_0010_0011 = 0x0123
        let p = art_dmx(0x0123, 7, 0, &[0, 0]).unwrap();
        assert_eq!(p[14], 0x23);
        assert_eq!(p[15], 0x01);
    }

    #[test]
    fn out_01_short_payload_is_padded_to_even_length_min_two() {
        let p = art_dmx(0, 1, 0, &[255]).unwrap();
        assert_eq!(&p[16..18], &[0x00, 0x02]);
        assert_eq!(&p[18..], &[255, 0]);
        let p = art_dmx(0, 1, 0, &[1, 2, 3]).unwrap();
        assert_eq!(&p[16..18], &[0x00, 0x04]);
        assert_eq!(&p[18..], &[1, 2, 3, 0]);
    }

    #[test]
    fn out_01_rejects_oversized_payload_and_port_address() {
        assert_eq!(
            art_dmx(0, 1, 0, &[0u8; 513]),
            Err(FrameError::TooManyChannels(513))
        );
        assert_eq!(
            art_dmx(0x8000, 1, 0, &[0u8; 2]),
            Err(FrameError::UniverseOutOfRange(0x8000))
        );
    }
}
