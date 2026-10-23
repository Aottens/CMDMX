//! The fixture library in the daemon (VS2-11): the bundled QLC+ set plus user profiles,
//! `specs/modules/fixture-library.md` LIB-12..14. The only part of the library with I/O.
//!
//! Loading reads only the headers of the `.qxf` files; a file is converted when it is picked.
//! Nothing here is on the tick thread: the daemon loads the library on its own thread and
//! answers library requests in the client threads (`ipc`), so output never waits for it.

use std::fs;
use std::path::{Path, PathBuf};

use cmdmx_core::library::channel_list::{self, ChannelList};
use cmdmx_core::library::qlcplus::{self, ImportReport};
use cmdmx_core::library::{Entry, EntryMode, Index, Manufacturer};
use cmdmx_core::profile::Profile;

/// Most results a search returns (LIB-14).
const SEARCH_LIMIT: usize = 50;
/// LIB-21: models listed without a manufacturer.
const MODELS_LIMIT: usize = 500;

pub struct Library {
    index: Index,
    /// Bundled QLC+ folder; the paths below it name the source of a converted profile.
    qlcplus_dir: Option<PathBuf>,
    user_dir: PathBuf,
}

impl Library {
    /// Index the bundled `*.qxf` files (recursively) and the user's `*.json` profiles. Problems
    /// are warnings, one per file or folder; a missing user folder is fine and is not created.
    pub fn load(qlcplus_dir: Option<&Path>, user_dir: &Path) -> (Library, Vec<String>) {
        let mut lib = Library {
            index: Index::default(),
            qlcplus_dir: qlcplus_dir.map(Path::to_path_buf),
            user_dir: user_dir.to_path_buf(),
        };
        let mut warnings = Vec::new();
        if let Some(dir) = qlcplus_dir {
            lib.index_qlcplus(dir, &mut warnings);
        }
        match fs::read_dir(user_dir) {
            Ok(files) => {
                for path in files.flatten().map(|f| f.path()) {
                    if path.extension().is_some_and(|x| x == "json") {
                        lib.index_user(&path, &mut warnings);
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => warnings.push(format!("{}: {e}", user_dir.display())),
        }
        (lib, warnings)
    }

    fn index_qlcplus(&mut self, dir: &Path, warnings: &mut Vec<String>) {
        let files = match fs::read_dir(dir) {
            Ok(f) => f,
            Err(e) => return warnings.push(format!("{}: {e}", dir.display())),
        };
        for file in files.flatten() {
            let path = file.path();
            // `file_type` does not follow symlinks, so a link loop cannot trap the walk.
            if file.file_type().is_ok_and(|t| t.is_dir()) {
                self.index_qlcplus(&path, warnings);
            } else if path.extension().is_some_and(|x| x == "qxf") {
                let header = fs::read_to_string(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|xml| qlcplus::header(&xml).map_err(|e| e.to_string()));
                match header {
                    Ok(h) => self.add(h.manufacturer, h.model, h.modes, "qlcplus", &path, warnings),
                    Err(e) => warnings.push(format!("{}: {e}", path.display())),
                }
            }
        }
    }

    fn index_user(&mut self, path: &Path, warnings: &mut Vec<String>) {
        let profile = fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|json| Profile::from_json(&json).map_err(|e| e.to_string()));
        match profile {
            Ok(p) => {
                let modes = p.modes.iter().map(entry_mode).collect();
                self.add(p.manufacturer, p.model, modes, "user", path, warnings);
            }
            Err(e) => warnings.push(format!("{}: {e}", path.display())),
        }
    }

    fn add(
        &mut self,
        manufacturer: String,
        model: String,
        modes: Vec<EntryMode>,
        source: &str,
        path: &Path,
        warnings: &mut Vec<String>,
    ) {
        if manufacturer.trim().is_empty() || model.trim().is_empty() {
            return warnings.push(format!("{}: no manufacturer or model", path.display()));
        }
        self.index.insert(Entry {
            manufacturer,
            model,
            modes,
            source: source.into(),
            path: path.to_string_lossy().into_owned(),
        });
    }

    /// Number of fixture types listed.
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// LIB-14: at most 50, sorted.
    pub fn search(&self, query: &str) -> Vec<&Entry> {
        self.index.search(query, SEARCH_LIMIT)
    }

    pub fn manufacturers(&self, query: &str) -> Vec<Manufacturer> {
        self.index.manufacturers(query)
    }

    pub fn models(&self, manufacturer: Option<&str>, query: &str) -> (Vec<&Entry>, bool) {
        self.index.models(manufacturer, query, MODELS_LIMIT)
    }

    fn entry(&self, manufacturer: &str, model: &str) -> Result<&Entry, String> {
        self.index
            .get(manufacturer, model)
            .ok_or_else(|| format!("no fixture '{manufacturer} {model}' in the library"))
    }

    /// Convert a bundled file: its profile and what happened to every channel.
    fn convert(&self, entry: &Entry) -> Result<qlcplus::Import, String> {
        let path = Path::new(&entry.path);
        let xml = fs::read_to_string(path).map_err(|e| format!("{}: {e}", entry.path))?;
        let rel = self
            .qlcplus_dir
            .as_deref()
            .and_then(|d| path.strip_prefix(d).ok())
            .unwrap_or(path);
        qlcplus::import(&xml, &format!("qlcplus:{}", rel.display())).map_err(|e| e.to_string())
    }

    /// LIB-10: the report of a bundled fixture; a user profile has none.
    pub fn import_report(&self, manufacturer: &str, model: &str) -> Result<ImportReport, String> {
        let entry = self.entry(manufacturer, model)?;
        if entry.source != "qlcplus" {
            return Err(format!(
                "'{manufacturer} {model}' is a user profile; it has no import report"
            ));
        }
        self.convert(entry).map(|i| i.report)
    }

    /// The profile to patch or edit: a bundled file is converted now, a user profile is read.
    pub fn profile(&self, manufacturer: &str, model: &str) -> Result<Profile, String> {
        let entry = self.entry(manufacturer, model)?;
        if entry.source == "qlcplus" {
            return self.convert(entry).map(|i| i.profile);
        }
        let json = fs::read_to_string(&entry.path).map_err(|e| format!("{}: {e}", entry.path))?;
        Profile::from_json(&json).map_err(|e| format!("{}: {e}", entry.path))
    }

    /// LIB-12: validate, write `<manufacturer> - <model>.json` atomically, list it as a user profile.
    pub fn save_profile(&mut self, profile: &Profile) -> Result<(), String> {
        profile.validate().map_err(|e| e.to_string())?;
        if profile.manufacturer.trim().is_empty() || profile.model.trim().is_empty() {
            return Err("a profile needs a manufacturer and a model".into());
        }
        let json = serde_json::to_string_pretty(profile).map_err(|e| e.to_string())?;
        fs::create_dir_all(&self.user_dir)
            .map_err(|e| format!("{}: {e}", self.user_dir.display()))?;
        let path = self
            .user_dir
            .join(file_name(&profile.manufacturer, &profile.model));
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json)
            .and_then(|()| fs::rename(&tmp, &path))
            .map_err(|e| {
                let _ = fs::remove_file(&tmp);
                format!("{}: {e}", path.display())
            })?;
        self.index.insert(Entry {
            manufacturer: profile.manufacturer.clone(),
            model: profile.model.clone(),
            modes: profile.modes.iter().map(entry_mode).collect(),
            source: "user".into(),
            path: path.to_string_lossy().into_owned(),
        });
        Ok(())
    }

    /// LIB-11: build from a channel list and save it as a user profile.
    pub fn save_channel_list(&mut self, list: &ChannelList) -> Result<Profile, String> {
        let profile = channel_list::build(list).map_err(|e| e.to_string())?;
        self.save_profile(&profile)?;
        Ok(profile)
    }
}

/// LIB-19: a user profile's mode takes its footprint.
fn entry_mode(m: &cmdmx_core::profile::Mode) -> EntryMode {
    EntryMode {
        name: m.name.clone(),
        channels: m.footprint,
    }
}

/// `<manufacturer> - <model>.json` that always lands directly in the user folder: path
/// separators, `:` and control characters become `_`, and so does a leading dot.
// ponytail: two names that differ only in those characters share a file; add a suffix if it ever matters.
fn file_name(manufacturer: &str, model: &str) -> String {
    let name: String = format!("{manufacturer} - {model}")
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':') || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let name = name
        .strip_prefix('.')
        .map_or(name.clone(), |rest| format!("_{rest}"));
    format!("{name}.json")
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use cmdmx_core::attribute::AttributeGroup;
    use cmdmx_core::library::channel_list::{ChannelList, ChannelType};
    use std::path::{Path, PathBuf};

    /// The three church files of LIB-09 stand in for the bundled QLC+ set.
    fn qlcplus_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tests/fixtures/qlcplus")
    }

    /// A fresh, empty user profiles folder per test.
    fn user_dir(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cmdmx-lib-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn load(user: &Path) -> Library {
        Library::load(Some(&qlcplus_dir()), user).0
    }

    fn names(lib: &Library, query: &str) -> Vec<(String, String)> {
        lib.search(query)
            .into_iter()
            .map(|e| (e.manufacturer.clone(), e.source.clone()))
            .collect()
    }

    #[test]
    fn lib_13_indexes_bundled_files_without_a_user_folder() {
        let user = user_dir("index");
        let (lib, warnings) = Library::load(Some(&qlcplus_dir()), &user);
        assert!(warnings.is_empty(), "{warnings:?}");
        let all = lib.search("");
        assert_eq!(all.len(), 3, "only *.qxf files count");
        let shehds = lib.search("shehds 19x15");
        assert_eq!(shehds.len(), 1);
        let modes: Vec<(&str, u16)> = shehds[0]
            .modes
            .iter()
            .map(|m| (m.name.as_str(), m.channels))
            .collect();
        assert_eq!(modes, [("16 Channel", 16), ("24 Channel", 24)]);
        assert_eq!(shehds[0].source, "qlcplus");
        assert!(!user.exists(), "loading never creates the folder");
    }

    #[test]
    fn lib_19_a_user_profile_takes_each_modes_footprint() {
        let user = user_dir("footprint");
        let mut lib = load(&user);
        let list = ChannelList {
            manufacturer: "Showtec".into(),
            model: "LED Par 56 (church)".into(),
            mode: "3 Channel".into(),
            channels: vec![ChannelType::Red, ChannelType::Green, ChannelType::Blue],
        };
        lib.save_channel_list(&list).unwrap();
        let mode = |lib: &Library| {
            let (found, _) = lib.models(Some("Showtec"), "church");
            found
                .iter()
                .flat_map(|e| e.modes.iter().map(|m| (m.name.clone(), m.channels)))
                .collect::<Vec<_>>()
        };
        assert_eq!(mode(&lib), [("3 Channel".to_string(), 3)]);
        // Read back from the folder at the next start.
        assert_eq!(mode(&load(&user)), [("3 Channel".to_string(), 3)]);
        let (found, _) = lib.models(Some("Showtec"), "56");
        let qlc: Vec<(&str, u16)> = found
            .iter()
            .find(|e| e.model == "LED Par 56")
            .unwrap()
            .modes
            .iter()
            .map(|m| (m.name.as_str(), m.channels))
            .collect();
        assert_eq!(qlc, [("6 Channel", 6)]);
        let _ = std::fs::remove_dir_all(&user);
    }

    #[test]
    fn lib_20_21_browse_the_bundled_set() {
        let user = user_dir("browse");
        let lib = load(&user);
        let names: Vec<(String, usize)> = lib
            .manufacturers("")
            .into_iter()
            .map(|m| (m.name, m.models))
            .collect();
        assert_eq!(
            names,
            [
                ("Pro-Lights".into(), 1),
                ("Shehds".into(), 1),
                ("Showtec".into(), 1)
            ]
        );
        let (found, truncated) = lib.models(None, "par");
        assert!(!truncated);
        let models: Vec<&str> = found.iter().map(|e| e.model.as_str()).collect();
        assert_eq!(models, ["LumiPar 7QPro", "LED Par 56"]);
    }

    #[test]
    fn lib_13_missing_bundled_folder_is_a_warning_not_a_failure() {
        let user = user_dir("missing");
        let (lib, warnings) = Library::load(Some(Path::new("/nonexistent/qlcplus")), &user);
        assert!(lib.search("").is_empty());
        assert_eq!(warnings.len(), 1);
        let (lib, warnings) = Library::load(None, &user);
        assert!(lib.search("").is_empty() && warnings.is_empty());
    }

    #[test]
    fn lib_14_import_report_and_profile_convert_on_pick() {
        let lib = load(&user_dir("pick"));
        let report = lib.import_report("Showtec", "LED Par 56").unwrap();
        assert_eq!(report.channels.len(), 6);
        let p = lib.profile("Pro-Lights", "LumiPar 7QPro").unwrap();
        assert_eq!(p.modes.len(), 3);
        assert!(lib.profile("Showtec", "Nope").is_err());
        assert!(lib.import_report("Nope", "Nope").is_err());
    }

    #[test]
    fn lib_11_channel_list_becomes_a_saved_user_profile() {
        let user = user_dir("channel-list");
        let mut lib = load(&user);
        let list = ChannelList {
            manufacturer: "U'King".into(),
            model: "RGBL Par".into(),
            mode: "4 Channel".into(),
            channels: vec![
                ChannelType::Red,
                ChannelType::Green,
                ChannelType::Blue,
                ChannelType::Lime,
            ],
        };
        let p = lib.save_channel_list(&list).unwrap();
        assert!(p.modes[0].virtual_dimmer);
        let file = user.join("U'King - RGBL Par.json");
        assert!(file.is_file(), "written as <manufacturer> - <model>.json");
        assert_eq!(
            names(&lib, "uking rgbl").len(),
            0,
            "words must match exactly"
        );
        assert_eq!(
            names(&lib, "u'king"),
            [("U'King".to_string(), "user".to_string())]
        );
        assert_eq!(lib.profile("U'King", "RGBL Par").unwrap(), p);
        assert!(
            lib.import_report("U'King", "RGBL Par").is_err(),
            "a user profile has no import report"
        );
        // It survives a restart.
        let again = load(&user);
        assert_eq!(again.profile("U'King", "RGBL Par").unwrap(), p);
        // A bad list writes nothing.
        let bad = ChannelList {
            channels: vec![ChannelType::Other {
                name: "M".into(),
                group: AttributeGroup::S,
                default: 2.0,
            }],
            model: "Bad".into(),
            ..list
        };
        assert!(lib.save_channel_list(&bad).is_err());
        assert!(!user.join("U'King - Bad.json").exists());
    }

    #[test]
    fn lib_12_edited_profile_saved_as_user_profile_wins() {
        let user = user_dir("edit");
        let mut lib = load(&user);
        let mut p = lib.profile("Showtec", "LED Par 56").unwrap();
        p.modes[0].attributes[2].name = "Program".into();
        lib.save_profile(&p).unwrap();
        assert_eq!(
            names(&lib, "showtec"),
            [("Showtec".to_string(), "user".to_string())]
        );
        assert_eq!(lib.profile("Showtec", "LED Par 56").unwrap(), p);
        let again = load(&user);
        assert_eq!(
            again.profile("Showtec", "LED Par 56").unwrap().modes[0].attributes[2].name,
            "Program"
        );
        // Invalid edits are refused (LIB-02) and nothing changes on disk.
        let mut bad = p.clone();
        bad.modes[0].attributes[0].channel = 99;
        assert!(lib.save_profile(&bad).is_err());
        assert_eq!(load(&user).profile("Showtec", "LED Par 56").unwrap(), p);
    }

    #[test]
    fn lib_12_unreadable_user_file_is_skipped_with_a_warning() {
        let user = user_dir("broken");
        std::fs::create_dir_all(&user).unwrap();
        std::fs::write(user.join("Broken - Thing.json"), "{ not json").unwrap();
        let (lib, warnings) = Library::load(Some(&qlcplus_dir()), &user);
        assert_eq!(lib.search("").len(), 3);
        assert!(
            warnings.iter().any(|w| w.contains("Broken - Thing.json")),
            "{warnings:?}"
        );
    }

    #[test]
    fn lib_12_file_names_cannot_escape_the_folder() {
        let user = user_dir("escape");
        let mut lib = load(&user);
        let list = ChannelList {
            manufacturer: "../../evil".into(),
            model: "a/b".into(),
            mode: "1".into(),
            channels: vec![ChannelType::Dimmer],
        };
        lib.save_channel_list(&list).unwrap();
        let files: Vec<_> = std::fs::read_dir(&user).unwrap().flatten().collect();
        assert_eq!(files.len(), 1, "written inside the user folder");
        assert!(lib.profile("../../evil", "a/b").is_ok());
    }
}
