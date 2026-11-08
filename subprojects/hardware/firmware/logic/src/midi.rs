//! USB-MIDI 1.0 event packets, cable 0, channel 1 (HW-20).

/// MSB packet then LSB packet (CC + 32), to be sent in one transfer.
pub fn cc14_packets(msb_cc: u8, value: u16) -> [[u8; 4]; 2] {
    let v = value.min(16383);
    let msb = (v >> 7) as u8;
    let lsb = (v & 0x7F) as u8;
    let cc = msb_cc & 0x7F;
    [
        cc_packet(cc, msb),
        cc_packet(cc.wrapping_add(32) & 0x7F, lsb),
    ]
}

pub fn cc_packet(cc: u8, value: u8) -> [u8; 4] {
    [0x0B, 0xB0, cc & 0x7F, value & 0x7F]
}

pub fn note_packet(note: u8, on: bool) -> [u8; 4] {
    if on {
        [0x09, 0x90, note & 0x7F, 127]
    } else {
        [0x08, 0x80, note & 0x7F, 0]
    }
}

/// Note on/off on channel 1 as (note, velocity); note off is velocity 0.
pub fn parse_feedback(p: &[u8; 4]) -> Option<(u8, u8)> {
    let [cin, status, note, vel] = *p;
    match (cin & 0x0F, status) {
        (0x09, 0x90) => Some((note & 0x7F, vel & 0x7F)),
        (0x08, 0x80) => Some((note & 0x7F, 0)),
        _ => None,
    }
}

/// Control change on channel 1 as (cc, value).
pub fn parse_cc(p: &[u8; 4]) -> Option<(u8, u8)> {
    let [cin, status, cc, value] = *p;
    (cin & 0x0F == 0x0B && status == 0xB0).then_some((cc & 0x7F, value & 0x7F))
}

/// Assembles 14-bit CC pairs from the host: MSB (CC 0..=31) then LSB (CC + 32).
pub struct Cc14Rx {
    msb: [Option<u8>; 32],
}

impl Default for Cc14Rx {
    fn default() -> Self {
        Self::new()
    }
}

impl Cc14Rx {
    pub fn new() -> Self {
        Self { msb: [None; 32] }
    }

    /// Returns (msb_cc, value) when an LSB completes a stored MSB.
    pub fn feed(&mut self, cc: u8, value: u8) -> Option<(u8, u16)> {
        match cc {
            0..=31 => {
                *self.msb.get_mut(usize::from(cc))? = Some(value & 0x7F);
                None
            }
            32..=63 => {
                let msb_cc = cc - 32;
                let msb = self.msb.get_mut(usize::from(msb_cc))?.take()?;
                Some((msb_cc, u16::from(msb) << 7 | u16::from(value & 0x7F)))
            }
            _ => None,
        }
    }
}
