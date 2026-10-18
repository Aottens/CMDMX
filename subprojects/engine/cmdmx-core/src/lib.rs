//! ColourmotionDMX engine core.
//!
//! Pure, deterministic, no I/O. Time is passed in; the caller owns the clock.
//! Behavioural reference: Avolites Titan (see `specs/modules/*.md`).
//!
//! Module map (phase 1):
//! - [`attribute`]  attribute groups (I P C G B E S FX) and HTP/LTP kind
//! - [`profile`]    fixture profiles with modes (own model, importers later — ADR-006, `specs/modules/fixture-library.md`)
//! - [`library`]    fixture library importers: QLC+ `.qxf` (LIB-05..10, 15..18)
//! - [`colour`]     colour as one value: target RGB, HSI/RGB/CMY wheel views (ADR-005)
//! - [`value`]      what an attribute holds: a level or a colour
//! - [`patch`]      fixtures on universes/addresses
//! - [`playback`]   playbacks: priority, fader mode, values, runtime level
//! - [`programmer`] selection, sub-selection and manual values (`specs/modules/programmer.md`)
//! - [`command`]    command line parser (`specs/modules/programmer.md` PRG-08..11)
//! - [`show`]       the show file: schema, (de)serialisation, migrations (`specs/modules/showfile.md`)
//! - [`engine`]     the merge (`specs/modules/engine-merge.md`)
//! - [`scenario`]   JSON scenario format used by `tests/scenarios/*.json`

pub mod attribute;
pub mod colour;
pub mod command;
pub mod engine;
pub mod library;
pub mod patch;
pub mod playback;
pub mod profile;
pub mod programmer;
pub mod scenario;
pub mod show;
pub mod value;

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
    #[error("profile '{profile}' has no mode '{mode}'")]
    UnknownMode { profile: String, mode: String },
    #[error("invalid profile (mode '{mode}'): {problem}")]
    InvalidProfile { mode: String, problem: String },
    #[error("fixture {fixture} at universe {universe} address {address} exceeds the universe")]
    AddressOutOfRange {
        fixture: u32,
        universe: UniverseId,
        address: u16,
    },
    #[error("{number} would overlap {other} at {universe}.{address}")]
    PatchOverlap {
        number: u32,
        other: u32,
        universe: UniverseId,
        address: u16,
    },
    #[error("number {0} is already in use")]
    NumberInUse(u32),
    #[error("user numbers start at 1")]
    InvalidNumber,
    #[error("no free run of addresses in universe {universe}")]
    NoFreeAddresses { universe: UniverseId },
    #[error("no free fixture numbers")]
    NoFreeNumbers,
    #[error("invalid handle page {page} slot {slot}")]
    InvalidHandle { page: u16, slot: u16 },
    #[error("invalid fixture count {0} (expected at least 1)")]
    InvalidCount(u32),
    #[error("fixture id {0} is already patched")]
    DuplicateFixture(u32),
    #[error("invalid value {0} (expected 0.0..=1.0)")]
    InvalidLevel(f32),
    #[error("fixture {fixture} attribute '{attribute}' takes a different kind of value")]
    WrongValueKind { fixture: u32, attribute: String },
    #[error("unknown group id {0}")]
    UnknownGroup(u32),
    #[error("fixture {fixture} has no attribute '{attribute}'")]
    UnknownAttribute { fixture: u32, attribute: String },
    #[error("nothing is selected")]
    EmptySelection,
    #[error("no selected fixture has attribute '{0}'")]
    AttributeNotInTargets(String),
    #[error("invalid pattern of {0} stages (expected at least 2)")]
    InvalidPattern(usize),
    #[error("import of '{source}' failed: {problem}")]
    ImportFailed {
        // `r#` keeps thiserror from treating this String as an error source.
        r#source: String,
        problem: String,
    },
    #[error("invalid command: {0}")]
    InvalidCommand(String),
    #[error("not a show file: {0}")]
    InvalidShow(String),
    #[error("{name} was made by a newer ColourmotionDMX (schema {found}; this one reads up to {supported})")]
    ShowTooNew {
        name: String,
        found: u64,
        supported: u32,
    },
}

/// A non-fatal condition surfaced to the UI. The engine never stops for a warning.
#[derive(Debug, Clone, PartialEq)]
pub struct Warning {
    pub at: Timestamp,
    pub message: String,
}
