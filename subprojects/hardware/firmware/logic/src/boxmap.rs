//! The USB-MIDI box's controls on the wire map (HW-61).
use crate::wiremap::{note, Control};

pub const FADERS: [Control; 2] = [Control::PlaybackFader(1), Control::PlaybackFader(2)];
pub const KEYS: [Control; 4] = [
    Control::Upper(1),
    Control::Upper(2),
    Control::Lower(1),
    Control::Lower(2),
];
pub const WHEEL: Control = Control::Wheel(1);

/// Index into `KEYS` of the key with this note.
pub fn key_for_note(n: u8) -> Option<usize> {
    KEYS.iter().position(|&k| note(k) == Some(n))
}
