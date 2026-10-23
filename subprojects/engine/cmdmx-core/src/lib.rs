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
//! - [`palette`]    palettes: storage, kind detection, resolving, icons (`specs/modules/palettes.md`)
//! - [`output`]    output nodes: where a line is sent (`specs/modules/output-protocols.md` OUT-13, OUT-25..28)
//! - [`patch`]      fixtures on universes/addresses
//! - [`playback`]   playbacks: handles, cues, options, runtime level
//! - `cuelist`      cue lists running: transitions, links, tracking (`specs/modules/cuelists.md`)
//! - [`programmer`] selection, sub-selection and manual values (`specs/modules/programmer.md`)
//! - [`command`]    command line parser (`specs/modules/programmer.md` PRG-08..11)
//! - [`show`]       the show file: schema, (de)serialisation, migrations (`specs/modules/showfile.md`)
//! - [`engine`]     the merge (`specs/modules/engine-merge.md`)
//! - [`scenario`]   JSON scenario format used by `tests/scenarios/*.json`

pub mod attribute;
pub mod colour;
pub mod command;
mod cuelist;
pub mod engine;
pub mod library;
pub mod output;
pub mod palette;
pub mod patch;
pub mod playback;
pub mod profile;
pub mod programmer;
pub mod scenario;
pub mod show;
pub mod undo;
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
    /// LIB-25: a profile edit the show's content does not allow, with the reason as plain text.
    #[error("{0}")]
    ProfileInUse(String),
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
    #[error("page {0} does not exist (pages are 1 to {max})", max = crate::playback::PAGES)]
    InvalidPage(u16),
    #[error("fixture {fixture} attribute '{attribute}' takes a different kind of value")]
    WrongValueKind { fixture: u32, attribute: String },
    #[error("unknown group id {0}")]
    UnknownGroup(u32),
    #[error("fixture {fixture} has no attribute '{attribute}'")]
    UnknownAttribute { fixture: u32, attribute: String },
    #[error("Nothing to undo")]
    NothingToUndo,
    #[error("Nothing to redo")]
    NothingToRedo,
    #[error("nothing is selected")]
    EmptySelection,
    #[error("no selected fixture has attribute '{0}'")]
    AttributeNotInTargets(String),
    #[error("invalid pattern of {0} stages (expected at least 2)")]
    InvalidPattern(usize),
    #[error("unknown palette {0}")]
    UnknownPalette(u32),
    #[error("palette numbers are 1 to {max}", max = crate::palette::MAX_NUMBER)]
    InvalidPaletteNumber(u32),
    #[error("palette number {0} is already in use")]
    PaletteNumberInUse(u32),
    #[error("no free palette numbers")]
    NoFreePaletteNumbers,
    #[error("palette id {0} is already in use")]
    DuplicatePalette(u32),
    #[error("{window} cell {handle} is not a place for a palette (handles start at 1)")]
    InvalidPaletteCell { window: String, handle: u16 },
    #[error("{window} cell {handle} already holds a palette")]
    PaletteCellInUse { window: String, handle: u16 },
    #[error("Palette {0} is used: Replace, Merge or Quick Merge")]
    PaletteUsed(u32),
    #[error("Nothing in the programmer for {0}")]
    NothingToRecord(String),
    #[error("Palette {0} does not fit the selection")]
    PaletteDoesNotFit(String),
    #[error("Record as Shared: the values differ inside a fixture type")]
    SharedValuesDiffer,
    #[error("a legend is at most {max} characters", max = crate::palette::MAX_LEGEND)]
    InvalidLegend,
    #[error("invalid icon id '{0}' (expected category.name, lower case a-z 0-9 . -, at most 48 characters)")]
    InvalidIcon(String),
    #[error("import of '{source}' failed: {problem}")]
    ImportFailed {
        // `r#` keeps thiserror from treating this String as an error source.
        r#source: String,
        problem: String,
    },
    #[error("invalid palette time {0} (expected 0 to 600 s)")]
    InvalidPaletteTime(f32),
    #[error("invalid palette overlap {0} (expected 0 to 100 %)")]
    InvalidPaletteOverlap(f32),
    #[error("Handle {page}.{slot} is used: Replace or Merge")]
    HandleUsedRecord { page: u16, slot: u16 },
    #[error("Handle {page}.{slot} is used")]
    HandleUsed { page: u16, slot: u16 },
    #[error("Nothing in the programmer to record")]
    NothingToRecordPlayback,
    #[error("Off: nothing of the selection is in the programmer")]
    NothingToOff,
    #[error("invalid time {0} (expected 0 to 600 s)")]
    InvalidTime(f32),
    #[error("invalid overlap {0} (expected 0 to 100 %)")]
    InvalidOverlap(f32),
    #[error("invalid playback group {0} (expected 1 to 99)")]
    InvalidGroup(u8),
    #[error("slot {slot} already holds a Locked playback")]
    SlotAlreadyLocked { slot: u16 },
    #[error("invalid halo colour (components are 0.0 to 1.0)")]
    InvalidHalo,
    #[error("Q{0} exists: Replace, Merge or Update")]
    CueExists(crate::playback::CueNumber),
    #[error("Name a cue: {0} is not running")]
    NameACue(String),
    #[error("Handle {page}.{slot} holds {master}")]
    HoldsMaster {
        page: u16,
        slot: u16,
        master: String,
    },
    #[error("there is no cue Q{0}")]
    UnknownCue(crate::playback::CueNumber),
    #[error("Q{0} is already in use")]
    CueNumberInUse(crate::playback::CueNumber),
    #[error("cue numbers run from 0.001 to 9999.999 with at most three decimals")]
    InvalidCueNumber,
    #[error("No room {0}: renumber")]
    NoCueRoom(String),
    #[error("Q{0} is the last cue: Delete the playback instead")]
    LastCue(crate::playback::CueNumber),
    #[error("No cue list connected")]
    NoCueListConnected,
    #[error("playback {0} is not a cue list")]
    NotACueList(u32),
    /// A list action that does not apply now, e.g. "Q3 is the last cue" (CUE-10, CUE-11).
    #[error("{0}")]
    ListRefused(String),
    #[error("invalid {0}")]
    InvalidOption(String),
    #[error("invalid command: {0}")]
    InvalidCommand(String),
    /// OUT-28: an output node edit that is refused, with the reason as plain text.
    #[error("{0}")]
    InvalidOutput(String),
    #[error("unknown output node {0}")]
    UnknownNode(u32),
    #[error("output node {0} cannot change its kind")]
    NodeKindChanged(u32),
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
