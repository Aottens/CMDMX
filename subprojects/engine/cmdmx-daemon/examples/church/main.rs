//! The church patch, once (`specs/modules/patch.md` PAT-11, PAT-15..18; VS2-13).
//!
//! Reads the owner's Lightkey patch export (tab-separated, outside git), picks a profile per
//! fixture type, numbers the fixtures in blocks of 100 per type, patches them one by one with
//! `patch_add` and writes the show `Kerk` plus the user profiles it made. Never overwrites.
//!
//! Usage: `cargo run -p cmdmx-daemon --example church -- <export.tsv> [--show-dir PATH]
//!        [--profiles-dir PATH] [--qlcplus-dir PATH]`.

use std::path::{Path, PathBuf};

use cmdmx_core::profile::Profile;

#[cfg(test)]
mod tests;

/// One fixture row of the export (PAT-15).
#[derive(Debug, Clone, PartialEq)]
struct Row {
    /// 1-based line in the file (the header is line 1).
    line: usize,
    universe: u16,
    address: u16,
    manufacturer: String,
    model: String,
    channels: u16,
    short_name: String,
}

/// The profile a Lightkey type maps to (PAT-16).
#[derive(Debug, Clone)]
struct Picked {
    /// Engine profile key, `"<manufacturer>/<model>"` (PAT-09).
    key: String,
    mode: String,
    profile: Profile,
    /// Saved as a user profile by the run (channel lists and the Shehds).
    user: bool,
}

struct Dirs {
    show_dir: PathBuf,
    profiles_dir: PathBuf,
    qlcplus_dir: PathBuf,
}

/// PAT-15.
fn parse_export(_tsv: &str) -> Result<Vec<Row>, String> {
    Err("not built yet".into())
}

/// PAT-16: QLC+ files are read from `qlcplus_dir`.
fn profile_for(_manufacturer: &str, _model: &str, _qlcplus_dir: &Path) -> Result<Picked, String> {
    Err("not built yet".into())
}

/// PAT-17: `(number, name)` for each row, in the order of `rows`.
fn numbers_and_names(_rows: &[Row]) -> Result<Vec<(u32, String)>, String> {
    Err("not built yet".into())
}

/// PAT-16..18: the whole run. Returns the report to print (Markdown table first).
fn run(_tsv: &str, _dirs: &Dirs) -> Result<String, String> {
    Err("not built yet".into())
}

fn main() {}
