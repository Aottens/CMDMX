//! The show file on disk (`specs/modules/showfile.md` SHW-09..14). Implementation: SHW-b.

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
        assert!(s.path("Kerk").is_file(), "the folder is created when missing");
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
        assert!(opened.show.is_none(), "not on disk yet: the first autosave writes it");
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
        let json = save(&s, "Kerk", &demo()).replace("\"schema_version\": 1", "\"schema_version\": 9");
        fs::write(s.path("Kerk"), &json).unwrap();
        s.remember("Kerk").unwrap();
        let opened = s.start(demo, WALL);
        assert_eq!(opened.name, "Demo");
        assert!(
            opened.warnings.iter().any(|w| w.contains("Kerk") && w.contains("newer")),
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
        assert!(a.finished(Done { show: job.show, result: Ok(()), at: WALL.into() }).is_none());
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
        assert!(opened.warnings.iter().any(|w| w.contains(".bak")), "{:?}", opened.warnings);
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
            a.finished(Done { show: job.show, result: Err("disk full".into()), at: WALL.into() })
        };
        let job = a.poll(&e, 5.0, WALL).unwrap();
        let warning = fail(&mut a, job).unwrap();
        assert!(warning.contains("disk full"), "{warning}");
        assert!(!a.state().autosave_ok);
        let job = a.poll(&e, 10.0, WALL).expect("retried after 5 s");
        assert!(fail(&mut a, job).is_none(), "warned once");
        let job = a.poll(&e, 15.0, WALL).unwrap();
        assert!(a.finished(Done { show: job.show, result: Ok(()), at: WALL.into() }).is_none());
        assert!(a.state().autosave_ok);
        e.patch_name(101, "Right wash").unwrap();
        let job = a.poll(&e, 20.0, WALL).unwrap();
        assert!(fail(&mut a, job).is_some(), "fails anew after it worked: warn again");
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
        assert!(opened.warnings.iter().any(|w| w.contains("recovered")), "{:?}", opened.warnings);
        assert!(!tmp_path(&s, "Kerk").exists());
        assert!(fs::read_to_string(s.path("Kerk")).unwrap().contains("Unsaved"));
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
