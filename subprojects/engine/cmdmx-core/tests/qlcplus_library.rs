//! LIB-13: every file of the bundled QLC+ library imports or fails cleanly (VS2-11).
//! Needs `scripts/fetch-qlcplus.sh`; skipped (passes) when `vendor/qlcplus` is absent, e.g. in CI.

#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use cmdmx_core::library::qlcplus::import;

fn qxf_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            qxf_files(&path, out);
        } else if path.extension().is_some_and(|x| x == "qxf") {
            out.push(path);
        }
    }
}

#[test]
fn lib_13_bundled_qlcplus_library_imports() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../vendor/qlcplus/fixtures");
    if !dir.is_dir() {
        eprintln!("skipped: run scripts/fetch-qlcplus.sh first");
        return;
    }
    let mut files = Vec::new();
    qxf_files(&dir, &mut files);
    files.sort();
    let mut failures = Vec::new();
    for path in &files {
        let xml = std::fs::read_to_string(path).unwrap();
        let source = path.strip_prefix(&dir).unwrap().display().to_string();
        if let Err(e) = import(&xml, &source) {
            failures.push(e.to_string());
        }
    }
    let ok = files.len() - failures.len();
    eprintln!("{ok} of {} QLC+ files imported", files.len());
    for f in &failures {
        eprintln!("  {f}");
    }
    // Every failure is a clean error (no panic got us here); at most 1 % may fail.
    assert!(
        failures.len() * 100 <= files.len(),
        "{} failures",
        failures.len()
    );
}
