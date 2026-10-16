//! Replays every `tests/scenarios/*.json` at the repo root against the engine.
//!
//! Each file names the criterion it proves (`"spec": "ENG-01"`). A failing
//! scenario prints the criterion and every mismatching channel.

#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;

use cmdmx_core::scenario::{run, Scenario};

fn scenario_dir() -> PathBuf {
    // <repo>/subprojects/engine/cmdmx-core → <repo>/tests/scenarios
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/scenarios")
        .canonicalize()
        .expect("tests/scenarios directory exists at the repo root")
}

#[test]
fn all_scenarios_pass() {
    let dir = scenario_dir();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("readable scenario directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no scenario files in {}", dir.display());

    let mut failures = Vec::new();
    for path in &files {
        let text = std::fs::read_to_string(path).expect("scenario readable");
        let scenario: Scenario = match serde_json::from_str(&text) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{}: invalid scenario JSON: {e}", path.display()));
                continue;
            }
        };
        match run(&scenario) {
            Ok(mismatches) if mismatches.is_empty() => {
                eprintln!("ok   {} ({})", scenario.spec, path.display());
            }
            Ok(mismatches) => {
                for m in mismatches {
                    failures.push(format!("{} ({}): {m}", scenario.spec, path.display()));
                }
            }
            Err(e) => failures.push(format!(
                "{} ({}): engine rejected setup: {e}",
                scenario.spec,
                path.display()
            )),
        }
    }

    assert!(
        failures.is_empty(),
        "{} scenario failure(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}
