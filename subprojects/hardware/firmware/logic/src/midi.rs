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
