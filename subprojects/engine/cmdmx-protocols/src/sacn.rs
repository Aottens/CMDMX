//! sACN — ANSI E1.31-2018 data packet.
//!
//! Three layers, each prefixed by a "flags & length" field whose low 12 bits
//! count the bytes from that field to the end of the packet and whose top
//! nibble is 0x7:
//! ```text
//! Root layer    (offset  0): preamble 0x0010, postamble 0x0000, "ASC-E1.17\0\0\0",
//!                            flags+len, vector 0x00000004, CID[16]
//! Framing layer (offset 38): flags+len, vector 0x00000002, source name[64],
//!                            priority, sync address, sequence, options, universe
//! DMP layer     (offset 115): flags+len, vector 0x02, addr/data type 0xA1,
//!                            first address 0, increment 1, value count,
//!                            start code + slots
//! ```

use crate::{FrameError, DMX_START_CODE, MAX_CHANNELS};

pub const PORT: u16 = 5568;
pub const ACN_PACKET_IDENTIFIER: [u8; 12] = *b"ASC-E1.17\0\0\0";
pub const VECTOR_ROOT_E131_DATA: u32 = 0x0000_0004;
pub const VECTOR_E131_DATA_PACKET: u32 = 0x0000_0002;
pub const VECTOR_DMP_SET_PROPERTY: u8 = 0x02;
pub const DEFAULT_PRIORITY: u8 = 100;

const ROOT_OFFSET: usize = 0;
const FRAMING_OFFSET: usize = 38;
const DMP_OFFSET: usize = 115;
const DATA_OFFSET: usize = 126;

/// Multicast group for a universe: 239.255.hi.lo.
pub fn multicast_address(universe: u16) -> [u8; 4] {
    [239, 255, (universe >> 8) as u8, (universe & 0xFF) as u8]
}

fn flags_and_length(len: usize) -> [u8; 2] {
    let v = 0x7000u16 | ((len as u16) & 0x0FFF);
    v.to_be_bytes()
}

/// Build an E1.31 data packet for `universe` (1..=63999).
#[allow(clippy::too_many_arguments)]
pub fn data_packet(
    cid: &[u8; 16],
    source_name: &str,
    priority: u8,
    sequence: u8,
    universe: u16,
    data: &[u8],
) -> Result<Vec<u8>, FrameError> {
    if data.len() > MAX_CHANNELS {
        return Err(FrameError::TooManyChannels(data.len()));
    }
    if !(1..=63999).contains(&universe) {
        return Err(FrameError::UniverseOutOfRange(universe));
    }
    let total = DATA_OFFSET + data.len();
    let mut p = Vec::with_capacity(total);

    // Root layer
    p.extend_from_slice(&0x0010u16.to_be_bytes());
    p.extend_from_slice(&0x0000u16.to_be_bytes());
    p.extend_from_slice(&ACN_PACKET_IDENTIFIER);
    p.extend_from_slice(&flags_and_length(total - 16));
    p.extend_from_slice(&VECTOR_ROOT_E131_DATA.to_be_bytes());
    p.extend_from_slice(cid);
    debug_assert_eq!(p.len(), FRAMING_OFFSET);

    // Framing layer
    p.extend_from_slice(&flags_and_length(total - FRAMING_OFFSET));
    p.extend_from_slice(&VECTOR_E131_DATA_PACKET.to_be_bytes());
    let mut name = [0u8; 64];
    for (dst, src) in name.iter_mut().zip(source_name.bytes().take(63)) {
        *dst = src;
    }
    p.extend_from_slice(&name);
    p.push(priority.min(200));
    p.extend_from_slice(&0u16.to_be_bytes()); // synchronization address (0 = none)
    p.push(sequence);
    p.push(0); // options: no preview, not terminated, no force-sync
    p.extend_from_slice(&universe.to_be_bytes());
    debug_assert_eq!(p.len(), DMP_OFFSET);

    // DMP layer
    p.extend_from_slice(&flags_and_length(total - DMP_OFFSET));
    p.push(VECTOR_DMP_SET_PROPERTY);
    p.push(0xA1);
    p.extend_from_slice(&0u16.to_be_bytes()); // first property address
    p.extend_from_slice(&1u16.to_be_bytes()); // address increment
    p.extend_from_slice(&((data.len() + 1) as u16).to_be_bytes());
    p.push(DMX_START_CODE);
    p.extend_from_slice(data);
    debug_assert_eq!(p.len(), total);
    let _ = ROOT_OFFSET;
    Ok(p)
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used)]
mod tests {
    use super::*;

    const CID: [u8; 16] = [
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E,
        0x1F,
    ];

    #[test]
    fn out_05_full_frame_layout_is_byte_exact() {
        let data = [0xABu8; 512];
        let p = data_packet(&CID, "ColourmotionDMX", 100, 9, 1, &data).unwrap();
        assert_eq!(p.len(), 638);
        // Root
        assert_eq!(&p[0..4], &[0x00, 0x10, 0x00, 0x00]);
        assert_eq!(&p[4..16], b"ASC-E1.17\0\0\0");
        assert_eq!(&p[16..18], &[0x72, 0x6E]); // 0x7000 | 622
        assert_eq!(&p[18..22], &[0, 0, 0, 4]);
        assert_eq!(&p[22..38], &CID);
        // Framing
        assert_eq!(&p[38..40], &[0x72, 0x58]); // 0x7000 | 600
        assert_eq!(&p[40..44], &[0, 0, 0, 2]);
        assert_eq!(&p[44..59], b"ColourmotionDMX");
        assert!(p[59..108].iter().all(|&b| b == 0));
        assert_eq!(p[108], 100); // priority
        assert_eq!(&p[109..111], &[0, 0]); // sync address
        assert_eq!(p[111], 9); // sequence
        assert_eq!(p[112], 0); // options
        assert_eq!(&p[113..115], &[0, 1]); // universe 1

        // DMP
        assert_eq!(&p[115..117], &[0x72, 0x0B]); // 0x7000 | 523
        assert_eq!(p[117], 0x02);
        assert_eq!(p[118], 0xA1);
        assert_eq!(&p[119..121], &[0, 0]);
        assert_eq!(&p[121..123], &[0, 1]);
        assert_eq!(&p[123..125], &[0x02, 0x01]); // 513 property values
        assert_eq!(p[125], 0x00); // start code
        assert!(p[126..].iter().all(|&b| b == 0xAB));
    }

    #[test]
    fn out_06_multicast_address_encodes_universe() {
        assert_eq!(multicast_address(1), [239, 255, 0, 1]);
        assert_eq!(multicast_address(256), [239, 255, 1, 0]);
        assert_eq!(multicast_address(63999), [239, 255, 0xF9, 0xFF]);
    }

    #[test]
    fn out_07_priority_is_clamped_to_200_and_universe_range_enforced() {
        let p = data_packet(&CID, "x", 255, 0, 7, &[0; 2]).unwrap();
        assert_eq!(p[108], 200);
        assert_eq!(
            data_packet(&CID, "x", 100, 0, 0, &[0; 2]),
            Err(FrameError::UniverseOutOfRange(0))
        );
        assert_eq!(
            data_packet(&CID, "x", 100, 0, 64000, &[0; 2]),
            Err(FrameError::UniverseOutOfRange(64000))
        );
    }

    #[test]
    fn out_05_long_source_name_is_truncated_to_63_bytes_plus_nul() {
        let long = "x".repeat(100);
        let p = data_packet(&CID, &long, 100, 0, 1, &[0; 2]).unwrap();
        assert!(p[44..107].iter().all(|&b| b == b'x'));
        assert_eq!(p[107], 0);
    }
}
