//! BRD-08: the one-time move of the data folders from the old product name to `CMDMX`.

use std::fs;
use std::path::Path;

/// (old, new) relative to home. Logs and the pid file are not migrated.
#[rustfmt::skip]
const ITEMS: [(&str, &str, Kind); 4] = [
    ("Documents/ColourmotionDMX/Shows", "Documents/CMDMX/Shows", Kind::Shows), // brand: old-name
    ("Library/Application Support/ColourmotionDMX/profiles", "Library/Application Support/CMDMX/profiles", Kind::Profiles), // brand: old-name
    ("Library/Application Support/ColourmotionDMX/control", "Library/Application Support/CMDMX/control", Kind::Shows), // brand: old-name
    ("Library/Application Support/ColourmotionDMX/state.json", "Library/Application Support/CMDMX/state.json", Kind::Shows), // brand: old-name
];

const LEFT: &str = "Old ColourmotionDMX folder left at"; // brand: old-name

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Shows,
    Profiles,
}

/// Moves the default data items under `home` from the old name to the new one, once.
/// `shows_by_flag`: `--show-dir` was given (shows, state and control are then used as given);
/// `profiles_by_flag`: `--profiles-dir` was given. Returns the warnings for the first client.
pub fn run(home: &Path, shows_by_flag: bool, profiles_by_flag: bool) -> Vec<String> {
    let mut warnings = Vec::new();
    for (old, new, kind) in ITEMS {
        if (kind == Kind::Shows && shows_by_flag) || (kind == Kind::Profiles && profiles_by_flag) {
            continue;
        }
        let (old, new) = (home.join(old), home.join(new));
        if !old.exists() {
            continue;
        }
        if new.exists() {
            warnings.push(format!("{LEFT} {}", old.display()));
            continue;
        }
        let moved = match new.parent() {
            Some(parent) => fs::create_dir_all(parent).and_then(|()| fs::rename(&old, &new)),
            None => fs::rename(&old, &new),
        };
        match moved {
            Ok(()) => eprintln!("migrated {} to {}", old.display(), new.display()),
            Err(e) => warnings.push(format!(
                "Could not move {} to {}: {e}",
                old.display(),
                new.display()
            )),
        }
    }
    warnings
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    const OLD_SHOWS: &str = "Documents/ColourmotionDMX/Shows"; // brand: old-name
    const OLD_SUPPORT: &str = "Library/Application Support/ColourmotionDMX"; // brand: old-name
    const NEW_SHOWS: &str = "Documents/CMDMX/Shows";
    const NEW_SUPPORT: &str = "Library/Application Support/CMDMX";

    /// A fresh temp home holding the old layout: a show, a profile, a mapping and state.json.
    fn old_home(test: &str) -> PathBuf {
        let home = std::env::temp_dir().join(format!("cmdmx-brd-{test}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(home.join(OLD_SHOWS)).unwrap();
        fs::write(home.join(OLD_SHOWS).join("Kerk.cmdmx.json"), "kerk").unwrap();
        fs::create_dir_all(home.join(OLD_SUPPORT).join("profiles")).unwrap();
        fs::write(home.join(OLD_SUPPORT).join("profiles/par.json"), "par").unwrap();
        fs::create_dir_all(home.join(OLD_SUPPORT).join("control")).unwrap();
        fs::write(home.join(OLD_SUPPORT).join("control/bcf.json"), "bcf").unwrap();
        fs::write(home.join(OLD_SUPPORT).join("state.json"), "{}").unwrap();
        home
    }

    #[test]
    fn brd_08_migrates_old_folders() {
        let home = old_home("migrates");
        let warnings = run(&home, false, false);
        assert!(warnings.is_empty(), "{warnings:?}");
        let read = |p: &str| fs::read_to_string(home.join(p)).unwrap();
        assert_eq!(read(&format!("{NEW_SHOWS}/Kerk.cmdmx.json")), "kerk");
        assert_eq!(read(&format!("{NEW_SUPPORT}/profiles/par.json")), "par");
        assert_eq!(read(&format!("{NEW_SUPPORT}/control/bcf.json")), "bcf");
        assert_eq!(read(&format!("{NEW_SUPPORT}/state.json")), "{}");
        assert!(!home.join(OLD_SHOWS).exists());
        assert!(!home.join(OLD_SUPPORT).join("profiles").exists());
        assert!(!home.join(OLD_SUPPORT).join("control").exists());
        assert!(!home.join(OLD_SUPPORT).join("state.json").exists());
        // Once: a second start finds nothing to move and says nothing.
        assert!(run(&home, false, false).is_empty());
        assert_eq!(read(&format!("{NEW_SHOWS}/Kerk.cmdmx.json")), "kerk");
    }

    #[test]
    fn brd_08_both_exist_warns() {
        let home = old_home("both");
        fs::create_dir_all(home.join(NEW_SHOWS)).unwrap();
        fs::write(home.join(NEW_SHOWS).join("New.cmdmx.json"), "new").unwrap();
        let warnings = run(&home, false, false);
        let old = home.join(OLD_SHOWS);
        assert_eq!(
            warnings,
            vec![format!(
                "Old ColourmotionDMX folder left at {}", // brand: old-name
                old.display()
            )]
        );
        // Nothing moved into the existing new folder; the old one stays.
        assert!(old.join("Kerk.cmdmx.json").exists());
        assert!(!home.join(NEW_SHOWS).join("Kerk.cmdmx.json").exists());
        // The other items had no new counterpart, so they moved.
        assert!(home.join(NEW_SUPPORT).join("profiles/par.json").exists());
        assert!(home.join(NEW_SUPPORT).join("state.json").exists());
    }

    #[test]
    fn brd_08_flag_paths_untouched() {
        let home = old_home("flags");
        let warnings = run(&home, true, true);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(home.join(OLD_SHOWS).join("Kerk.cmdmx.json").exists());
        assert!(home.join(OLD_SUPPORT).join("profiles/par.json").exists());
        assert!(home.join(OLD_SUPPORT).join("control/bcf.json").exists());
        assert!(home.join(OLD_SUPPORT).join("state.json").exists());
        assert!(!home.join("Documents/CMDMX").exists());
        assert!(!home.join(NEW_SUPPORT).exists());
    }

    #[test]
    fn brd_08_rename_failure_is_warning() {
        let home = old_home("failure");
        // A file where the new Documents folder should be: the shows cannot move there.
        fs::create_dir_all(home.join("Documents")).unwrap();
        fs::write(home.join("Documents/CMDMX"), "in the way").unwrap();
        let warnings = run(&home, false, false);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0].contains(&home.join(OLD_SHOWS).display().to_string()),
            "{warnings:?}"
        );
        // The shows stay where they were; the rest still moved.
        assert!(home.join(OLD_SHOWS).join("Kerk.cmdmx.json").exists());
        assert!(home.join(NEW_SUPPORT).join("profiles/par.json").exists());
    }
}
