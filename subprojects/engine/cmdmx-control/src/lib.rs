//! Control-surface protocols as pure encode/decode. The daemon owns MIDI
//! ports and feeds raw bytes in; it sends what these functions return out.
//!
//! Phase 1: Mackie Control Universal ([`mcu`]) for the BCF2000 (MC mode) and
//! the X-Touch family (ADR-003). A generic note/CC mapping follows with
//! `specs/modules/control-surfaces.md`.
//!
//! Generic control layer (all pure, no ports): [`action`] catalogue (CTL-05),
//! [`midi`] decode/encode (CTL-09, 17), [`mapping`] files (CTL-10..12, 22, 27),
//! [`convert`] value conversion and pickup (CTL-07, 12, 13), [`feedback`]
//! diffing (CTL-17..21), [`learn`] guess (CTL-26) and [`osc`] codec and
//! `/cmdmx` schema (CTL-34, 36), [`deck`] Stream Deck codec and mapping (CTL-45..49) and
//! [`deck_image`] key looks and renderer (CTL-50, 51).

pub mod action;
pub mod convert;
pub mod deck;
pub mod deck_image;
pub mod error;
pub mod feedback;
pub mod learn;
pub mod mapping;
pub mod mcu;
pub mod midi;
pub mod osc;

pub use error::ControlError;

/// Something a control surface did, normalised for the engine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SurfaceEvent {
    /// Fader `index` (0-based within the surface) moved to 0.0..=1.0.
    Fader { index: u8, level: f32 },
    /// Master fader (MCU channel 9) moved.
    MasterFader { level: f32 },
    /// A button went down or up.
    Button { button: Button, pressed: bool },
    /// Encoder `index` turned by `delta` detents (negative = counter-clockwise).
    Encoder { index: u8, delta: i8 },
    /// Fader touched / released (motor faders report this).
    FaderTouch { index: u8, touched: bool },
}

/// Buttons we care about, independent of which protocol produced them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    /// Upper button per channel (MCU "REC/ARM").
    ChannelUpper(u8),
    /// Second row per channel (MCU "SOLO").
    ChannelSolo(u8),
    /// Third row per channel (MCU "MUTE").
    ChannelMute(u8),
    /// Fourth row per channel (MCU "SELECT").
    ChannelSelect(u8),
    /// Encoder push per channel (MCU V-Pot switch).
    EncoderPush(u8),
    BankLeft,
    BankRight,
    ChannelLeft,
    ChannelRight,
    /// Any other MCU note we do not model yet, by note number.
    Other(u8),
}
