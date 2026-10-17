//! ColourmotionDMX engine core.
//!
//! Pure, deterministic, no I/O. Time is passed in; the caller owns the clock.
//! Behavioural reference: Avolites Titan (see `specs/modules/*.md`).
//!
//! Module map (phase 1):
//! - [`attribute`]  attribute groups (I P C G B E S FX) and HTP/LTP kind
//! - [`profile`]    fixture profiles (minimal JSON profile now, GDTF later — ADR-004)
//! - [`patch`]      fixtures on universes/addresses
//! - [`playback`]   playbacks: priority, fader mode, values, runtime level
//! - [`programmer`] selection, sub-selection and manual values (`specs/modules/programmer.md`)
//! - [`engine`]     the merge (`specs/modules/engine-merge.md`)
//! - [`scenario`]   JSON scenario format used by `tests/scenarios/*.json`

pub mod attribute;
pub mod engine;
pub mod patch;
pub mod playback;
pub mod profile;
pub mod programmer;
pub mod scenario;

/// Seconds on the caller's clock. Never read from the system clock inside core.
pub type Timestamp = f64;

/// Index of a DMX universe (1-based, as users see it).
pub type UniverseId = u16;

/// One DMX universe frame: 512 slots, start code handled by the transport layer.
pub const UNIVERSE_SIZE: usize = 512;

/// Errors the core reports instead of panicking. Every one of these is a
/// rejected action; the engine keeps ticking and keeps its previous frame.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum CoreError {
    #[error("unknown fixture id {0}")]
    UnknownFixture(u32),
    #[error("unknown playback id {0}")]
    UnknownPlayback(u32),
    #[error("unknown profile '{0}'")]
    UnknownProfile(String),
    #[error("fixture {fixture} at universe {universe} address {address} exceeds the universe")]
    AddressOutOfRange {
        fixture: u32,
        universe: UniverseId,
        address: u16,
    },
    #[error("invalid value {0} (expected 0.0..=1.0)")]
    InvalidLevel(f32),
    #[error("unknown group id {0}")]
    UnknownGroup(u32),
    #[error("fixture {fixture} has no attribute '{attribute}'")]
    UnknownAttribute { fixture: u32, attribute: String },
    #[error("nothing is selected")]
    EmptySelection,
    #[error("invalid pattern of {0} stages (expected at least 2)")]
    InvalidPattern(usize),
}

/// A non-fatal condition surfaced to the UI. The engine never stops for a warning.
#[derive(Debug, Clone, PartialEq)]
pub struct Warning {
    pub at: Timestamp,
    pub message: String,
}
