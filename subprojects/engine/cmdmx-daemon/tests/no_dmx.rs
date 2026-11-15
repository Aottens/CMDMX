//! Tests never send DMX (OUT-43): `specs/modules/output-protocols.md`.
//!
//! Reads every file in this folder and checks that each start of the daemon binary passes the
//! three kill switches (OUT-32) before it spawns, so no test puts the Demo show's values on the
//! owner's network, where real lamps may listen. A start marked `// OUT-43: loopback` may leave one
//! kind on when the same file also has a start with all three: the first run points the node at a
//! destination no lamp can hear, the second sends there (the loopback exception).

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

/// Split so this file does not count as a start itself.
const START: &str = concat!("CARGO_BIN_EXE_", "cmdmx-daemon");
const FLAGS: [&str; 3] = ["\"--no-artnet\"", "\"--no-sacn\"", "\"--no-serial\""];
const LOOPBACK: &str = "// OUT-43: loopback";

#[test]
fn out_43_tests_send_no_dmx() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut starts = 0;
    let mut missing = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let mut all_off = false;
        let mut marked = Vec::new();
        for (at, _) in text.match_indices(START) {
            starts += 1;
            // The builder runs from the binary to the call that starts it.
            let rest = &text[at..];
            let end = [".spawn()", ".output()", ".status()"]
                .iter()
                .filter_map(|call| rest.find(call))
                .min()
                .unwrap_or(rest.len());
            let builder = &rest[..end];
            let line = text[..at].lines().count();
            let lacks: Vec<_> = FLAGS.iter().filter(|f| !builder.contains(*f)).collect();
            if lacks.is_empty() {
                all_off = true;
            } else if builder.contains(LOOPBACK) && lacks.len() == 1 {
                marked.push(line);
            } else {
                for flag in lacks {
                    missing.push(format!("{name}:{line} lacks {flag}"));
                }
            }
        }
        if !all_off {
            for line in marked {
                missing.push(format!(
                    "{name}:{line} is marked loopback but no start has all three"
                ));
            }
        }
    }
    assert!(starts > 0, "no daemon start found in {}", dir.display());
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}
