//! The show file on disk (`specs/modules/showfile.md` SHW-09..14). Implementation: SHW-b.
//!
//! The shows folder, the last-show state file, opening with recovery and backup, and autosave.
//! The tick never touches the disk for a show except at a switch: `Autosave` only decides, the
//! saver thread writes (SHW-11).

use std::fs;
use std::io::{self, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{SystemTime, UNIX_EPOCH};

use cmdmx_core::engine::Engine;
use cmdmx_core::show::{Meta, Show};
use serde::Serialize;

const EXT: &str = ".cmdmx.json";
const DEMO: &str = "Demo";
/// SHW-11: the autosave looks for a change this often.
const AUTOSAVE_EVERY: f64 = 5.0;

/// SHW-09: 1–64 characters, no `/` or `:`, not starting with `.`.
pub fn valid_name(name: &str) -> Result<(), String> {
    let len = name.chars().count();
    if !(1..=64).contains(&len) || name.contains(['/', ':']) || name.starts_with('.') {
        return Err(format!(
            "'{name}' is not a show name (1-64 characters, no / or :, not starting with .)"
        ));
    }
    Ok(())
}

/// `<path>` with `suffix` appended to the whole file name.
fn suffixed(path: &Path, suffix: &str) -> PathBuf {
    let mut p = path.as_os_str().to_owned();
    p.push(suffix);
    PathBuf::from(p)
}

/// SHW-11: write `<path>.tmp`, `fsync` it and rename it over `path`, so `path` is always either
/// the previous or the new complete file. The folder is created when missing.
pub fn write_atomic(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = suffixed(path, ".tmp");
    let mut file = fs::File::create(&tmp)?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    fs::rename(&tmp, path)
}

/// `YYYY-MM-DDTHH:MM:SSZ` (the core never reads the clock, SHW-01).
pub fn iso_utc(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z % 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Meta of a show that starts now.
pub fn new_meta(name: &str, wall: &str) -> Meta {
    Meta {
        name: name.into(),
        created: wall.into(),
        modified: wall.into(),
        app_version: env!("CARGO_PKG_VERSION").into(),
    }
}

/// SHW-15: one row of `show.list`.
#[derive(Debug, Serialize)]
pub struct ShowEntry {
    pub name: String,
    pub modified: String,
}

/// SHW-12: a show as opened. `show` is what the file held (`None` for a Demo that was built).
pub struct Opened {
    pub name: String,
    pub engine: Engine,
    pub meta: Meta,
    pub show: Option<Show>,
    pub warnings: Vec<String>,
}

// The engine has no `Debug`; `Result::unwrap_err` wants one.
impl std::fmt::Debug for Opened {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Opened")
            .field("name", &self.name)
            .field("warnings", &self.warnings)
            .finish_non_exhaustive()
    }
}

/// SHW-09: the shows folder and the file that remembers the last open show.
pub struct Shows {
    dir: PathBuf,
    state_file: PathBuf,
}

impl Shows {
    pub fn new(dir: PathBuf, state_file: PathBuf) -> Shows {
        Shows { dir, state_file }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}{EXT}"))
    }

    pub fn state_file(&self) -> &Path {
        &self.state_file
    }

    /// Only `*.cmdmx.json` files, sorted by name ignoring case.
    pub fn list(&self) -> Vec<ShowEntry> {
        let mut list: Vec<ShowEntry> = fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let file = entry.file_name().into_string().ok()?;
                let name = file.strip_suffix(EXT)?.to_string();
                let modified = entry.metadata().and_then(|m| m.modified()).ok()?;
                Some(ShowEntry {
                    name,
                    modified: iso_utc(modified),
                })
            })
            .collect();
        list.sort_by_key(|e| e.name.to_lowercase());
        list
    }

    /// Names are unique ignoring case (APFS).
    pub fn exists(&self, name: &str) -> bool {
        let name = name.to_lowercase();
        self.list().iter().any(|e| e.name.to_lowercase() == name)
    }

    /// SHW-14 and SHW-04/07/12: settle a leftover `.tmp`, read the file, build the engine and
    /// keep the original as `.bak` when the load left something out or migrated.
    pub fn open(&self, name: &str) -> Result<Opened, String> {
        // Keep the casing the file has.
        let lower = name.to_lowercase();
        let name = self
            .list()
            .into_iter()
            .find(|e| e.name.to_lowercase() == lower)
            .map_or_else(|| name.to_string(), |e| e.name);
        let name = name.as_str();
        let path = self.path(name);
        let mut warnings = Vec::new();
        let tmp = suffixed(&path, ".tmp");
        if let Ok(tmp_time) = fs::metadata(&tmp).and_then(|m| m.modified()) {
            let newer = fs::metadata(&path)
                .and_then(|m| m.modified())
                .map_or(true, |show_time| tmp_time > show_time);
            let usable =
                newer && fs::read_to_string(&tmp).is_ok_and(|json| Show::from_json(&json).is_ok());
            if usable && fs::rename(&tmp, &path).is_ok() {
                warnings.push(format!(
                    "{name}: recovered the autosave of {}",
                    iso_utc(tmp_time)
                ));
            } else {
                let _ = fs::remove_file(&tmp);
                warnings.push(format!("{name}: discarded an unfinished autosave"));
            }
        }
        let json = fs::read_to_string(&path).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => format!("there is no show named {name}"),
            _ => format!("cannot read {name}: {e}"),
        })?;
        let (show, migrations) = Show::from_json(&json).map_err(|e| match e {
            // The core names meta.name from inside the file; a copy has another name.
            cmdmx_core::CoreError::ShowTooNew {
                found, supported, ..
            } => format!(
                "{name} was made by a newer ColourmotionDMX (schema {found}; this one reads up to {supported})"
            ),
            e => format!("{name}: {e}"),
        })?;
        let (engine, lost) = Engine::from_show(&show);
        if !lost.is_empty() || !migrations.is_empty() {
            let bak = suffixed(&path, ".bak");
            warnings.push(match fs::copy(&path, &bak) {
                Ok(_) => format!("{name}: the original file is kept as {name}{EXT}.bak"),
                Err(e) => format!("{name}: could not keep the original as {name}{EXT}.bak: {e}"),
            });
        }
        warnings.extend(lost.into_iter().map(|w| format!("{name}: {w}")));
        warnings.extend(
            migrations
                .into_iter()
                .map(|m| format!("{name}: migrated {m}")),
        );
        let mut meta = show.meta.clone();
        meta.name = name.into();
        Ok(Opened {
            name: name.into(),
            engine,
            meta,
            show: Some(show),
            warnings,
        })
    }

    /// The name in `state.json`.
    pub fn last(&self) -> Option<String> {
        let json = fs::read_to_string(self.state_file()).ok()?;
        let v: serde_json::Value = serde_json::from_str(&json).ok()?;
        Some(v.get("last_show")?.as_str()?.to_string())
    }

    pub fn remember(&self, name: &str) -> io::Result<()> {
        let json = serde_json::json!({ "last_show": name }).to_string();
        write_atomic(self.state_file(), &json)
    }

    /// SHW-10: the last show; else (also when it is refused) Demo from its file, else the
    /// built-in Demo. A refused file is only reported, never written (`Opened::show` is `None`
    /// for the built Demo, and a refused show is never the one that autosaves).
    pub fn start(&self, demo: impl FnOnce() -> Engine, wall: &str) -> Opened {
        let mut warnings = Vec::new();
        let mut opened = None;
        for name in self.last().into_iter().chain([DEMO.to_string()]) {
            match self.open(&name) {
                Ok(o) => {
                    opened = Some(o);
                    break;
                }
                // A show that is merely gone is not worth a warning; a refused one is.
                Err(e) if self.exists(&name) => warnings.push(e),
                Err(_) => {}
            }
        }
        let mut opened = opened.unwrap_or_else(|| {
            // A Demo file that was refused is never overwritten: take the first free name.
            let name = (2..)
                .map(|n| format!("{DEMO} {n}"))
                .find(|n| !self.exists(n))
                .filter(|_| self.exists(DEMO))
                .unwrap_or_else(|| DEMO.into());
            Opened {
                meta: new_meta(&name, wall),
                name,
                engine: demo(),
                show: None,
                warnings: Vec::new(),
            }
        });
        warnings.append(&mut opened.warnings);
        opened.warnings = warnings;
        opened
    }
}

/// SHW-15: what `show.*` asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum ShowRequest {
    List,
    New { name: String },
    Open { name: String },
    SaveAs { name: String },
}

/// SHW-15: the `show` part of the snapshot and the `show` delta.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ShowState {
    pub name: String,
    pub saved_at: Option<String>,
    pub autosave_ok: bool,
}

/// SHW-11: a copy of the show for the saver thread.
pub struct Job {
    pub path: PathBuf,
    pub json: String,
    pub show: Show,
}

/// SHW-11/13: the saver's answer to one [`Job`]; `at` is when the write ended.
pub struct Done {
    pub show: Show,
    pub result: Result<(), String>,
    pub at: String,
}

/// SHW-11/13: decides when the open show needs writing; the tick owns it, the saver writes.
pub struct Autosave {
    path: PathBuf,
    meta: Meta,
    /// The last show known to be on disk (as read, or as written successfully).
    saved: Option<Show>,
    checked: f64,
    saved_at: Option<String>,
    ok: bool,
}

impl Autosave {
    pub fn new(path: PathBuf, meta: Meta, saved: Option<Show>, now: f64) -> Autosave {
        Autosave {
            path,
            meta,
            // The file is there: it was saved when it says so.
            saved_at: saved.as_ref().map(|s| s.meta.modified.clone()),
            saved,
            checked: now,
            ok: true,
        }
    }

    /// Show data only: live state is not in `to_show`, and `meta` is left out of the comparison.
    fn changed(&self, engine: &Engine, wall: &str) -> Option<Job> {
        let known = self.saved.as_ref().map(|s| s.meta.clone());
        let mut show = engine.to_show(known.unwrap_or_else(|| self.meta.clone()));
        if self.saved.as_ref() == Some(&show) {
            return None;
        }
        show.meta = Meta {
            modified: wall.into(),
            ..self.meta.clone()
        };
        Some(Job {
            path: self.path.clone(),
            json: show.to_json(),
            show,
        })
    }

    /// At most every 5 s; a failed save is retried by the same rule (SHW-13).
    pub fn poll(&mut self, engine: &Engine, now: f64, wall: &str) -> Option<Job> {
        if now - self.checked < AUTOSAVE_EVERY {
            return None;
        }
        self.checked = now;
        self.changed(engine, wall)
    }

    /// Before a show switch: no wait (SHW-11).
    pub fn flush(&mut self, engine: &Engine, wall: &str) -> Option<Job> {
        self.changed(engine, wall)
    }

    /// The saver's answer. Returns a warning on the first failure of a streak and a
    /// note on the first success after one (SHW-13).
    pub fn finished(&mut self, done: Done) -> Option<String> {
        let name = done.show.meta.name.clone();
        if name != self.meta.name {
            // The flush of a show we have left: report a failure, change nothing here.
            return done
                .result
                .err()
                .map(|e| format!("saving {name} failed: {e}"));
        }
        match done.result {
            Ok(()) => {
                let was_failing = !std::mem::replace(&mut self.ok, true);
                self.saved_at = Some(done.at);
                self.saved = Some(done.show);
                was_failing.then(|| format!("autosave of {name} works again"))
            }
            Err(e) => {
                let first = std::mem::replace(&mut self.ok, false);
                first.then(|| format!("autosave of {name} failed: {e}; retrying every 5 s"))
            }
        }
    }

    pub fn state(&self) -> ShowState {
        ShowState {
            name: self.meta.name.clone(),
            saved_at: self.saved_at.clone(),
            autosave_ok: self.ok,
        }
    }
}

/// SHW-11: the thread that does the disk writes, so the tick never waits. A panic in a write
/// is caught and comes back as a failed save.
pub fn spawn_saver() -> io::Result<(Sender<Job>, Receiver<Done>)> {
    let (jobs, job_rx) = channel::<Job>();
    let (done_tx, done) = channel();
    std::thread::Builder::new()
        .name("show-saver".into())
        .spawn(move || {
            for job in job_rx {
                let result = catch_unwind(AssertUnwindSafe(|| write_atomic(&job.path, &job.json)))
                    .unwrap_or_else(|_| Err(io::Error::other("the writer panicked")))
                    .map_err(|e| e.to_string());
                let at = iso_utc(SystemTime::now());
                if done_tx
                    .send(Done {
                        show: job.show,
                        result,
                        at,
                    })
                    .is_err()
                {
                    return;
                }
            }
        })?;
    Ok((jobs, done))
}

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]
mod tests {
    use super::*;
    use cmdmx_core::engine::Engine;
    use cmdmx_core::show::{Meta, Show};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    const WALL: &str = "2026-10-18T09:00:00Z";

    /// A fresh, empty folder per test.
    fn tmp(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cmdmx-shw-{test}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn shows(test: &str) -> Shows {
        let dir = tmp(test);
        Shows::new(dir.join("Shows"), dir.join("state.json"))
    }

    fn demo() -> Engine {
        crate::demo_engine().unwrap()
    }

    fn meta(name: &str) -> Meta {
        Meta {
            name: name.into(),
            created: WALL.into(),
            modified: WALL.into(),
            app_version: "0.1.0".into(),
        }
    }

    /// Write `engine` as show `name` the way autosave would.
    fn save(shows: &Shows, name: &str, engine: &Engine) -> String {
        let json = engine.to_show(meta(name)).to_json();
        write_atomic(&shows.path(name), &json).unwrap();
        json
    }

    fn set_mtime(path: &Path, secs_ago: u64) {
        let t = SystemTime::now() - Duration::from_secs(secs_ago);
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }

    fn tmp_path(shows: &Shows, name: &str) -> PathBuf {
        let mut p = shows.path(name).into_os_string();
        p.push(".tmp");
        PathBuf::from(p)
    }

    fn bak_path(shows: &Shows, name: &str) -> PathBuf {
        let mut p = shows.path(name).into_os_string();
        p.push(".bak");
        PathBuf::from(p)
    }

    // ---- SHW-09 --------------------------------------------------------------

    #[test]
    fn shw_09_name_rules() {
        for ok in ["Kerk", "Kerst 2026", "a", &"x".repeat(64)] {
            assert!(valid_name(ok).is_ok(), "{ok:?}");
        }
        for bad in ["", &"x".repeat(65), "a/b", "a:b", ".hidden"] {
            assert!(valid_name(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn shw_09_show_file_name_and_folder() {
        let s = shows("folder");
        assert!(s.path("Kerk").ends_with("Shows/Kerk.cmdmx.json"));
        save(&s, "Kerk", &demo());
        assert!(
            s.path("Kerk").is_file(),
            "the folder is created when missing"
        );
    }

    #[test]
    fn shw_09_list_holds_shows_sorted_ignoring_case() {
        let s = shows("list");
        for name in ["Zondag", "avond", "Kerk"] {
            save(&s, name, &demo());
        }
        fs::write(s.path("Kerk").with_file_name("notes.txt"), "x").unwrap();
        fs::write(tmp_path(&s, "Kerk"), "x").unwrap();
        let list = s.list();
        let names: Vec<&str> = list.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["avond", "Kerk", "Zondag"]);
        assert!(list[0].modified.ends_with('Z') && list[0].modified.len() == 20);
    }

    #[test]
    fn shw_09_names_are_unique_ignoring_case() {
        let s = shows("unique");
        save(&s, "Kerk", &demo());
        assert!(s.exists("Kerk"));
        assert!(s.exists("kerk"));
        assert!(!s.exists("Kerst"));
    }

    #[test]
    fn shw_09_last_show_is_remembered() {
        let s = shows("remember");
        assert_eq!(s.last(), None);
        s.remember("Kerk").unwrap();
        assert_eq!(s.last().as_deref(), Some("Kerk"));
        let again = Shows::new(s.path("x").parent().unwrap().into(), s.state_file().into());
        assert_eq!(again.last().as_deref(), Some("Kerk"));
    }

    // ---- SHW-10 --------------------------------------------------------------

    #[test]
    fn shw_10_start_opens_the_last_show() {
        let s = shows("start-last");
        let mut e = demo();
        e.patch_name(101, "Left wash").unwrap();
        save(&s, "Kerk", &e);
        s.remember("Kerk").unwrap();
        let opened = s.start(demo, WALL);
        assert_eq!(opened.name, "Kerk");
        assert_eq!(opened.engine.fixture(101).unwrap().name, "Left wash");
        assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
    }

    #[test]
    fn shw_10_without_a_last_show_the_demo_is_built() {
        let s = shows("start-none");
        let opened = s.start(demo, WALL);
        assert_eq!(opened.name, "Demo");
        assert!(
            opened.show.is_none(),
            "not on disk yet: the first autosave writes it"
        );
        assert_eq!(opened.engine.fixtures().count(), demo().fixtures().count());
        assert_eq!(opened.meta.created, WALL);
    }

    #[test]
    fn shw_10_an_existing_demo_file_is_opened_not_rebuilt() {
        let s = shows("start-demo-file");
        let mut e = demo();
        e.patch_name(101, "From the file").unwrap();
        save(&s, "Demo", &e);
        let opened = s.start(demo, WALL);
        assert_eq!(opened.name, "Demo");
        assert_eq!(opened.engine.fixture(101).unwrap().name, "From the file");
    }

    #[test]
    fn shw_10_a_missing_last_show_falls_back_to_demo() {
        let s = shows("start-missing");
        s.remember("Gone").unwrap();
        assert_eq!(s.start(demo, WALL).name, "Demo");
    }

    #[test]
    fn shw_10_a_refused_last_show_falls_back_to_demo_and_is_never_written() {
        let s = shows("start-refused");
        let json =
            save(&s, "Kerk", &demo()).replace("\"schema_version\": 2", "\"schema_version\": 9");
        fs::write(s.path("Kerk"), &json).unwrap();
        s.remember("Kerk").unwrap();
        let opened = s.start(demo, WALL);
        assert_eq!(opened.name, "Demo");
        assert!(
            opened
                .warnings
                .iter()
                .any(|w| w.contains("Kerk") && w.contains("newer")),
            "{:?}",
            opened.warnings
        );
        assert_eq!(fs::read_to_string(s.path("Kerk")).unwrap(), json);
        assert!(!bak_path(&s, "Kerk").exists());
    }

    // ---- SHW-11 --------------------------------------------------------------

    #[test]
    fn shw_11_write_atomic_replaces_and_leaves_no_tmp() {
        let s = shows("atomic");
        write_atomic(&s.path("Kerk"), "one").unwrap();
        write_atomic(&s.path("Kerk"), "two").unwrap();
        assert_eq!(fs::read_to_string(s.path("Kerk")).unwrap(), "two");
        assert!(!tmp_path(&s, "Kerk").exists());
    }

    /// An autosave for show "Kerk" whose file holds `engine` as it is now.
    fn autosave(s: &Shows, engine: &Engine) -> Autosave {
        let show = engine.to_show(meta("Kerk"));
        Autosave::new(s.path("Kerk"), meta("Kerk"), Some(show), 0.0)
    }

    #[test]
    fn shw_11_autosave_writes_show_changes_within_five_seconds() {
        let s = shows("autosave-change");
        let mut e = demo();
        let mut a = autosave(&s, &e);
        assert!(a.poll(&e, 1.0, WALL).is_none(), "nothing changed");
        e.patch_name(101, "Left wash").unwrap();
        assert!(a.poll(&e, 2.0, WALL).is_none(), "checked every 5 s");
        let job = a.poll(&e, 5.0, "2026-10-18T09:00:05Z").unwrap();
        assert_eq!(job.path, s.path("Kerk"));
        assert_eq!(job.show.meta.modified, "2026-10-18T09:00:05Z");
        assert_eq!(job.show.meta.created, WALL);
        assert_eq!(job.json, job.show.to_json());
        assert!(job.json.contains("Left wash"));
    }

    #[test]
    fn shw_15_an_opened_show_was_saved_when_its_file_says() {
        let s = shows("saved-at");
        let a = autosave(&s, &demo());
        assert_eq!(a.state().saved_at.as_deref(), Some(WALL));
    }

    #[test]
    fn shw_11_live_state_is_not_a_show_change() {
        let s = shows("autosave-live");
        let mut e = demo();
        let mut a = autosave(&s, &e);
        e.set_fader(1, 0.7, 0.0).unwrap();
        e.set_grand_master(0.5).unwrap();
        e.select_group(1).unwrap();
        e.tick(1.0);
        assert!(a.poll(&e, 5.0, WALL).is_none());
    }

    #[test]
    fn shw_11_a_written_show_is_not_written_again() {
        let s = shows("autosave-done");
        let mut e = demo();
        let mut a = autosave(&s, &e);
        e.patch_name(101, "Left wash").unwrap();
        let job = a.poll(&e, 5.0, WALL).unwrap();
        assert!(a
            .finished(Done {
                show: job.show,
                result: Ok(()),
                at: WALL.into()
            })
            .is_none());
        assert!(a.poll(&e, 10.0, WALL).is_none());
        assert_eq!(a.state().saved_at.as_deref(), Some(WALL));
    }

    #[test]
    fn shw_11_flush_writes_at_once_but_only_a_change() {
        let s = shows("autosave-flush");
        let mut e = demo();
        let mut a = autosave(&s, &e);
        assert!(a.flush(&e, WALL).is_none());
        e.patch_name(101, "Left wash").unwrap();
        assert!(a.flush(&e, WALL).is_some(), "no 5 s wait before a switch");
    }

    #[test]
    fn shw_11_a_new_show_without_a_file_is_written_on_the_first_poll() {
        let s = shows("autosave-new");
        let e = demo();
        let mut a = Autosave::new(s.path("Demo"), meta("Demo"), None, 0.0);
        assert_eq!(a.state().saved_at, None);
        assert!(a.poll(&e, 5.0, WALL).is_some());
    }

    #[test]
    fn shw_11_saver_thread_writes_the_job() {
        let s = shows("saver");
        let e = demo();
        let mut a = Autosave::new(s.path("Demo"), meta("Demo"), None, 0.0);
        let job = a.poll(&e, 5.0, WALL).unwrap();
        let json = job.json.clone();
        let (jobs, done) = spawn_saver().unwrap();
        jobs.send(job).unwrap();
        let done = done.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(done.result.is_ok(), "{:?}", done.result);
        assert!(done.at.ends_with('Z'));
        assert_eq!(fs::read_to_string(s.path("Demo")).unwrap(), json);
    }

    // ---- SHW-12 --------------------------------------------------------------

    #[test]
    fn shw_12_a_lossy_load_keeps_the_original_as_bak() {
        let s = shows("bak");
        let json = save(&s, "Kerk", &demo());
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        v["groups"][0]["fixtures"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!(999));
        let broken = Show::from_json(&v.to_string()).unwrap().0.to_json();
        fs::write(s.path("Kerk"), &broken).unwrap();
        let opened = s.open("Kerk").unwrap();
        assert!(opened.warnings.iter().any(|w| w.contains("999")));
        assert!(
            opened.warnings.iter().any(|w| w.contains(".bak")),
            "{:?}",
            opened.warnings
        );
        assert_eq!(fs::read_to_string(bak_path(&s, "Kerk")).unwrap(), broken);
    }

    #[test]
    fn shw_12_a_clean_load_makes_no_bak() {
        let s = shows("no-bak");
        save(&s, "Kerk", &demo());
        let opened = s.open("Kerk").unwrap();
        assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
        assert!(opened.show.is_some());
        assert!(!bak_path(&s, "Kerk").exists());
    }

    // ---- SHW-13 --------------------------------------------------------------

    #[test]
    fn shw_13_a_failing_autosave_warns_once_and_retries() {
        let s = shows("autosave-fail");
        let mut e = demo();
        let mut a = autosave(&s, &e);
        e.patch_name(101, "Left wash").unwrap();
        let fail = |a: &mut Autosave, job: Job| {
            a.finished(Done {
                show: job.show,
                result: Err("disk full".into()),
                at: WALL.into(),
            })
        };
        let job = a.poll(&e, 5.0, WALL).unwrap();
        let warning = fail(&mut a, job).unwrap();
        assert!(warning.contains("disk full"), "{warning}");
        assert!(!a.state().autosave_ok);
        let job = a.poll(&e, 10.0, WALL).expect("retried after 5 s");
        assert!(fail(&mut a, job).is_none(), "warned once");
        let job = a.poll(&e, 15.0, WALL).unwrap();
        let back = a.finished(Done {
            show: job.show,
            result: Ok(()),
            at: WALL.into(),
        });
        assert!(
            back.unwrap().contains("works again"),
            "said once when it works again"
        );
        assert!(a.state().autosave_ok);
        e.patch_name(101, "Right wash").unwrap();
        let job = a.poll(&e, 20.0, WALL).unwrap();
        assert!(
            fail(&mut a, job).is_some(),
            "fails anew after it worked: warn again"
        );
    }

    // ---- SHW-14 --------------------------------------------------------------

    #[test]
    fn shw_14_a_newer_valid_tmp_is_recovered() {
        let s = shows("recover");
        save(&s, "Kerk", &demo());
        set_mtime(&s.path("Kerk"), 60);
        let mut e = demo();
        e.patch_name(101, "Unsaved").unwrap();
        fs::write(tmp_path(&s, "Kerk"), e.to_show(meta("Kerk")).to_json()).unwrap();
        let opened = s.open("Kerk").unwrap();
        assert_eq!(opened.engine.fixture(101).unwrap().name, "Unsaved");
        assert!(
            opened.warnings.iter().any(|w| w.contains("recovered")),
            "{:?}",
            opened.warnings
        );
        assert!(!tmp_path(&s, "Kerk").exists());
        assert!(fs::read_to_string(s.path("Kerk"))
            .unwrap()
            .contains("Unsaved"));
    }

    #[test]
    fn shw_14_an_invalid_or_older_tmp_is_deleted() {
        let s = shows("tmp-delete");
        save(&s, "Kerk", &demo());
        fs::write(tmp_path(&s, "Kerk"), "{\"truncat").unwrap();
        let opened = s.open("Kerk").unwrap();
        assert_eq!(opened.warnings.len(), 1, "{:?}", opened.warnings);
        assert!(!tmp_path(&s, "Kerk").exists());

        let mut e = demo();
        e.patch_name(101, "Old").unwrap();
        fs::write(tmp_path(&s, "Kerk"), e.to_show(meta("Kerk")).to_json()).unwrap();
        set_mtime(&tmp_path(&s, "Kerk"), 60);
        let opened = s.open("Kerk").unwrap();
        assert_ne!(opened.engine.fixture(101).unwrap().name, "Old");
        assert!(!tmp_path(&s, "Kerk").exists());
    }

    #[test]
    fn shw_10_a_refused_demo_file_is_never_written_either() {
        let s = shows("start-refused-demo");
        let json =
            save(&s, "Demo", &demo()).replace("\"schema_version\": 2", "\"schema_version\": 9");
        fs::write(s.path("Demo"), &json).unwrap();
        let opened = s.start(demo, WALL);
        assert_eq!(opened.name, "Demo 2", "the built Demo gets a free name");
        assert!(opened.show.is_none());
        assert!(
            opened.warnings.iter().any(|w| w.contains("newer")),
            "{:?}",
            opened.warnings
        );
        assert_eq!(fs::read_to_string(s.path("Demo")).unwrap(), json);
    }

    #[test]
    fn shw_09_open_keeps_the_name_as_the_file_has_it() {
        let s = shows("case");
        save(&s, "Kerk", &demo());
        let opened = s.open("kerk").unwrap();
        assert_eq!(opened.name, "Kerk");
        assert_eq!(opened.meta.name, "Kerk");
    }

    #[test]
    fn shw_04_a_refusal_names_the_file_not_the_name_inside_it() {
        // A copy of Kerk saved by a newer version under another file name (SHW-c run).
        let s = shows("refused-copy");
        let json =
            save(&s, "Kerk", &demo()).replace("\"schema_version\": 2", "\"schema_version\": 9");
        fs::write(s.path("Toekomst"), json).unwrap();
        let err = s.open("Toekomst").unwrap_err();
        assert!(err.contains("Toekomst") && err.contains("newer"), "{err}");
        assert!(!err.contains("Kerk"), "{err}");
    }

    #[test]
    fn shw_14_open_of_an_unknown_show_is_refused() {
        let s = shows("unknown");
        assert!(s.open("Nope").unwrap_err().contains("Nope"));
    }

    // ---- times ---------------------------------------------------------------

    #[test]
    fn iso_utc_formats_seconds_since_the_epoch() {
        assert_eq!(iso_utc(UNIX_EPOCH), "1970-01-01T00:00:00Z");
        assert_eq!(
            iso_utc(UNIX_EPOCH + Duration::from_secs(1_760_000_000)),
            "2025-10-09T08:53:20Z"
        );
        assert_eq!(
            iso_utc(UNIX_EPOCH + Duration::from_secs(951_782_400)),
            "2000-02-29T00:00:00Z"
        );
    }
}
