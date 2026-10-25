//! Feedback velocity to key LED colour (HW-20, HW-32).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

const fn c(r: u8, g: u8, b: u8) -> Rgb {
    Rgb { r, g, b }
}

/// One colour per block of 8 velocities (wire-map.md).
const TABLE: [Rgb; 16] = [
    c(0, 0, 0),
    c(255, 0, 0),
    c(255, 96, 0),
    c(255, 160, 0),
    c(255, 255, 0),
    c(128, 255, 0),
    c(0, 255, 0),
    c(0, 255, 128),
    c(0, 255, 255),
    c(0, 128, 255),
    c(0, 0, 255),
    c(128, 0, 255),
    c(255, 0, 255),
    c(255, 64, 128),
    c(255, 180, 100),
    c(255, 255, 255),
];

/// Dim red while the host is gone or silent.
// A static, not a const: the host test asserts on its fields and clippy rejects asserts on consts.
pub static DISCONNECTED: Rgb = c(32, 0, 0);

pub fn colour(velocity: u8) -> Rgb {
    TABLE
        .get(usize::from(velocity.min(127) / 8))
        .copied()
        .unwrap_or(TABLE[0])
}
