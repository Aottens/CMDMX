//! Byte-exact DMX transport framing. No sockets, no serial ports — the daemon
//! owns I/O. Every builder here is covered by a golden-bytes test derived from
//! the respective specification (see `specs/modules/output-protocols.md`).

pub mod artnet;
pub mod enttec;
pub mod sacn;

/// DMX512 null start code.
pub const DMX_START_CODE: u8 = 0x00;

/// Largest payload any of these transports carries.
pub const MAX_CHANNELS: usize = 512;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FrameError {
    #[error("DMX payload of {0} channels exceeds 512")]
    TooManyChannels(usize),
    #[error("universe {0} out of range for this transport")]
    UniverseOutOfRange(u16),
}

/// A sequence counter that skips 0 after wrapping, as Art-Net expects
/// (0 means "sequence disabled"). sACN uses the full 0..=255 range; it
/// wraps naturally with `wrapping_add`.
#[derive(Debug, Default, Clone, Copy)]
pub struct ArtSequence(u8);

impl ArtSequence {
    pub fn advance(&mut self) -> u8 {
        self.0 = match self.0 {
            255 => 1,
            n => n + 1,
        };
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn out_02_art_sequence_wraps_to_one_not_zero() {
        let mut s = ArtSequence::default();
        assert_eq!(s.advance(), 1);
        for _ in 0..253 {
            s.advance();
        }
        assert_eq!(s.advance(), 255);
        assert_eq!(s.advance(), 1);
    }
}
