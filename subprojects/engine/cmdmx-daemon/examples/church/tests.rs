//! PAT-15..18: the church one-off. QLC+ types are checked against the files in
//! `tests/fixtures/qlcplus/`; the two only in the bundled set are skipped when
//! `vendor/qlcplus` is absent (CI).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::fs;
use std::path::{Path, PathBuf};

use cmdmx_core::attribute::AttributeDef;
use cmdmx_core::patch::Handle;
use cmdmx_core::profile::Profile;
use cmdmx_core::show::Show;

use super::*;

const HEADER: &str = "Universe\tAddress\tCount\tManufacturer\tModel\tManufacturer & model\tMode\tChannels\tShort name\tComments";

/// An export with one row per `(address, manufacturer, model, channels, short name)`.
fn export(rows: &[(u16, &str, &str, u16, &str)]) -> String {
    let mut tsv = format!("{HEADER}\n");
    for (address, manufacturer, model, channels, short) in rows {
        tsv.push_str(&format!(
            "1\t{address:03}\t1\t{manufacturer}\t{model}\t{manufacturer} {model}\tMode 1\t{channels}\t{short}\t\n"
        ));
    }
    tsv
}

/// A fresh, empty folder per test.
fn tmp(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cmdmx-church-{test}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn dirs(test: &str) -> Dirs {
    let dir = tmp(test);
    Dirs {
        show_dir: dir.join("Shows"),
        profiles_dir: dir.join("profiles"),
        qlcplus_dir: dir.join("qlcplus"),
    }
}

fn repo_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/qlcplus")
        .join(name)
}

fn attr<'a>(profile: &'a Profile, mode: &str, name: &str) -> &'a AttributeDef {
    profile
        .mode(Some(mode))
        .unwrap()
        .attributes
        .iter()
        .find(|a| a.name == name)
        .unwrap_or_else(|| panic!("{} has no {name}", profile.model))
}

fn footprint(picked: &Picked) -> u16 {
    picked.profile.mode(Some(&picked.mode)).unwrap().footprint
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .map(|it| {
            it.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

// ---- PAT-15: reading the export ----------------------------------------------------------

#[test]
fn pat_15_reads_one_row_per_fixture() {
    let tsv = export(&[
        (185, "Generic", "PAR Spot", 1, "P27"),
        (464, "beamZ", "Fuze 75S Spot", 13, "MH6"),
    ]);
    let rows = parse_export(&tsv).unwrap();
    assert_eq!(
        rows,
        vec![
            Row {
                line: 2,
                universe: 1,
                address: 185,
                manufacturer: "Generic".into(),
                model: "PAR Spot".into(),
                channels: 1,
                short_name: "P27".into(),
            },
            Row {
                line: 3,
                universe: 1,
                address: 464,
                manufacturer: "beamZ".into(),
                model: "Fuze 75S Spot".into(),
                channels: 13,
                short_name: "MH6".into(),
            },
        ]
    );
}

#[test]
fn pat_15_wrong_header_is_refused() {
    let tsv = "Universe\tAddress\tModel\n1\t001\tPAR Spot\n";
    assert!(parse_export(tsv).is_err());
}

#[test]
fn pat_15_count_other_than_one_is_refused_with_its_line() {
    let tsv = format!(
        "{HEADER}\n1\t001\t1\tGeneric\tPAR Spot\tGeneric PAR Spot\tMode 1\t1\tP1\t\n1\t002\t4\tGeneric\tPAR Spot\tGeneric PAR Spot\tMode 1\t1\tP2\t\n"
    );
    let err = parse_export(&tsv).unwrap_err();
    assert!(err.contains("line 3"), "{err}");
}

#[test]
fn pat_15_non_number_is_refused_with_its_line() {
    let tsv =
        format!("{HEADER}\n1\tabc\t1\tGeneric\tPAR Spot\tGeneric PAR Spot\tMode 1\t1\tP1\t\n");
    let err = parse_export(&tsv).unwrap_err();
    assert!(err.contains("line 2"), "{err}");
}

// ---- PAT-16: a profile per type -----------------------------------------------------------

#[test]
fn pat_16_channel_list_types_follow_the_table() {
    let none = Path::new("/nonexistent");
    for (manufacturer, model, key, mode, channels) in [
        (
            "Showtec",
            "Phantom 25 LED Spot",
            "Showtec/Phantom 25 LED Spot",
            "11 Channel",
            11,
        ),
        (
            "Prolights",
            "Lumipar6UQ",
            "Pro-Lights/Lumipar6UQ",
            "4 Channel",
            4,
        ),
        ("Generic", "PAR Spot", "Generic/PAR Spot", "1 Channel", 1),
        (
            "Generic",
            "PAR Wash (RGBW)",
            "Generic/PAR Wash (RGBW)",
            "4 Channel",
            4,
        ),
        (
            "beamZ",
            "BTF50Z Mini Fresnel",
            "beamZ/BTF50Z Mini Fresnel",
            "3 Channel",
            3,
        ),
        (
            "Generic",
            "PAR Spot (RGBL)",
            "Generic/PAR Spot (RGBL)",
            "4 Channel",
            4,
        ),
        (
            "beamZ",
            "Fuze 75S Spot",
            "beamZ/Fuze 75S Spot",
            "16 Bit",
            13,
        ),
        (
            "Generic",
            "PAR Spot (RGB)",
            "Generic/PAR Spot (RGB)",
            "3 Channel",
            3,
        ),
    ] {
        let picked = profile_for(manufacturer, model, none).unwrap();
        assert_eq!(picked.key, key);
        assert_eq!(picked.mode, mode, "{key}");
        assert_eq!(footprint(&picked), channels, "{key}");
        assert!(picked.user, "{key} is saved as a user profile");
        let (m, rest) = key.split_once('/').unwrap();
        assert_eq!(
            (
                picked.profile.manufacturer.as_str(),
                picked.profile.model.as_str()
            ),
            (m, rest)
        );
    }
}

#[test]
fn pat_16_movers_follow_the_lightkey_layout_with_the_shutter_open() {
    let none = Path::new("/nonexistent");
    let phantom = profile_for("Showtec", "Phantom 25 LED Spot", none)
        .unwrap()
        .profile;
    let m = "11 Channel";
    assert_eq!(
        (
            attr(&phantom, m, "Pan").channel,
            attr(&phantom, m, "Pan").fine_channel
        ),
        (1, Some(3))
    );
    assert_eq!(
        (
            attr(&phantom, m, "Tilt").channel,
            attr(&phantom, m, "Tilt").fine_channel
        ),
        (2, Some(4))
    );
    assert_eq!(attr(&phantom, m, "Colour Wheel").channel, 6);
    assert_eq!(attr(&phantom, m, "Shutter").channel, 7);
    assert_eq!(attr(&phantom, m, "Shutter").default, 1.0);
    assert_eq!(attr(&phantom, m, "Dimmer").channel, 8);
    assert_eq!(attr(&phantom, m, "Gobo").channel, 9);

    let fuze = profile_for("beamZ", "Fuze 75S Spot", none).unwrap().profile;
    let m = "16 Bit";
    assert_eq!(
        (
            attr(&fuze, m, "Pan").channel,
            attr(&fuze, m, "Pan").fine_channel
        ),
        (1, Some(2))
    );
    assert_eq!(
        (
            attr(&fuze, m, "Tilt").channel,
            attr(&fuze, m, "Tilt").fine_channel
        ),
        (3, Some(4))
    );
    assert_eq!(attr(&fuze, m, "Shutter").channel, 7);
    assert_eq!(attr(&fuze, m, "Shutter").default, 1.0);
    assert_eq!(attr(&fuze, m, "Dimmer").channel, 8);
    assert_eq!(attr(&fuze, m, "Focus").channel, 9);
    assert_eq!(attr(&fuze, m, "Prism").channel, 10);
}

#[test]
fn pat_16_unknown_type_is_refused() {
    let err = profile_for("Acme", "Blinder 9000", Path::new("/nonexistent")).unwrap_err();
    assert!(err.contains("Blinder 9000"), "{err}");
}

#[test]
fn pat_16_qlcplus_types_and_the_shehds_zoom() {
    let d = dirs("pat-16-qlc");
    for (folder, file) in [
        ("Showtec", "Showtec-LED-Par-56.qxf"),
        ("Shehds", "Shehds-LED-Beam+Wash-19x15W-RGBW-Zoom.qxf"),
    ] {
        fs::create_dir_all(d.qlcplus_dir.join(folder)).unwrap();
        fs::copy(repo_fixture(file), d.qlcplus_dir.join(folder).join(file)).unwrap();
    }
    let par = profile_for("Showtec", "LED PAR 56 Short DMX", &d.qlcplus_dir).unwrap();
    assert_eq!(
        (par.key.as_str(), par.mode.as_str()),
        ("Showtec/LED Par 56", "6 Channel")
    );
    assert_eq!(footprint(&par), 6);
    assert!(
        !par.user,
        "a plain QLC+ profile is not saved as a user profile"
    );

    let shehds = profile_for(
        "Shehds",
        "LED Beam+Wash 19x15W RGBW Zoom Light",
        &d.qlcplus_dir,
    )
    .unwrap();
    assert_eq!(shehds.key, "Shehds/LED Beam+Wash 19x15W RGBW Zoom Lighting");
    assert_eq!(shehds.mode, "24 Channel");
    assert_eq!(footprint(&shehds), 24);
    assert!(shehds.user);
    assert_eq!(attr(&shehds.profile, "24 Channel", "Zoom").channel, 6);
    for mode in &shehds.profile.modes {
        assert!(
            mode.attributes.iter().all(|a| a.name != "Focus"),
            "{} still has Focus",
            mode.name
        );
        assert!(
            mode.attributes.iter().any(|a| a.name == "Zoom"),
            "{} has no Zoom",
            mode.name
        );
    }
}

#[test]
fn pat_16_bundled_qlcplus_types() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../vendor/qlcplus/fixtures");
    if !dir.is_dir() {
        eprintln!("skipped: run scripts/fetch-qlcplus.sh first");
        return;
    }
    for (manufacturer, model, key, mode, channels) in [
        (
            "Showtec",
            "LED PAR 56 Short DMX",
            "Showtec/LED Par 56",
            "6 Channel",
            6,
        ),
        (
            "Showtec",
            "Compact Par 7 Tri",
            "Showtec/Compact Par 7 Tri",
            "3 Channel",
            3,
        ),
        (
            "Stairville",
            "Hz-200 DMX",
            "Stairville/Hz-200 DMX",
            "Default",
            2,
        ),
        (
            "Shehds",
            "LED Beam+Wash 19x15W RGBW Zoom Light",
            "Shehds/LED Beam+Wash 19x15W RGBW Zoom Lighting",
            "24 Channel",
            24,
        ),
    ] {
        let picked = profile_for(manufacturer, model, &dir).unwrap();
        assert_eq!((picked.key.as_str(), picked.mode.as_str()), (key, mode));
        assert_eq!(footprint(&picked), channels, "{key}");
    }
}

#[test]
fn pat_16_footprint_mismatch_is_refused_and_nothing_is_written() {
    let d = dirs("pat-16-footprint");
    let tsv = export(&[(185, "Generic", "PAR Spot", 3, "P27")]);
    let err = run(&tsv, &d).unwrap_err();
    assert!(err.contains("PAR Spot"), "{err}");
    assert!(files_in(&d.show_dir).is_empty());
    assert!(files_in(&d.profiles_dir).is_empty());
}

// ---- PAT-17: numbers and names -------------------------------------------------------------

/// The church's duplicate short names, in address order.
fn church_sample() -> Vec<Row> {
    parse_export(&export(&[
        (1, "Showtec", "LED PAR 56 Short DMX", 6, "P1"),
        (7, "Showtec", "LED PAR 56 Short DMX", 6, "P2"),
        (109, "Showtec", "Phantom 25 LED Spot", 11, "MH1"),
        (221, "Generic", "PAR Wash (RGBW)", 4, "P44"),
        (234, "beamZ", "BTF50Z Mini Fresnel", 3, "P40"),
        (250, "Showtec", "LED PAR 56 Short DMX", 6, "P35"),
        (
            289,
            "Shehds",
            "LED Beam+Wash 19x15W RGBW Zoom Light",
            24,
            "MH6",
        ),
        (
            313,
            "Shehds",
            "LED Beam+Wash 19x15W RGBW Zoom Light",
            24,
            "MH7",
        ),
        (
            337,
            "Shehds",
            "LED Beam+Wash 19x15W RGBW Zoom Light",
            24,
            "MH8",
        ),
        (373, "Generic", "PAR Spot (RGBL)", 4, "P40"),
        (464, "beamZ", "Fuze 75S Spot", 13, "MH6"),
        (477, "beamZ", "Fuze 75S Spot", 13, "MH7"),
    ]))
    .unwrap()
}

fn expected_sample() -> Vec<(u32, String)> {
    [
        (101, "P1"),
        (102, "P2"),
        (201, "MH1"),
        (301, "P44"),
        (401, "P40"),
        (103, "P35"),
        (501, "MH6"),
        (502, "MH7"),
        (503, "MH8"),
        (601, "P45"),
        (701, "MH9"),
        (702, "MH10"),
    ]
    .into_iter()
    .map(|(n, s)| (n, s.to_string()))
    .collect()
}

#[test]
fn pat_17_blocks_of_100_per_type_and_duplicate_names_move_on() {
    assert_eq!(
        numbers_and_names(&church_sample()).unwrap(),
        expected_sample()
    );
}

#[test]
fn pat_17_order_comes_from_the_address_not_the_file() {
    let mut rows = church_sample();
    rows.reverse();
    let mut expected = expected_sample();
    expected.reverse();
    assert_eq!(numbers_and_names(&rows).unwrap(), expected);
}

#[test]
fn pat_17_more_than_99_of_one_type_is_refused() {
    let rows: Vec<(u16, &str, &str, u16, &str)> = (1..=100)
        .map(|a| (a, "Generic", "PAR Spot", 1, "D"))
        .collect();
    let rows = parse_export(&export(&rows)).unwrap();
    assert!(numbers_and_names(&rows).is_err());
}

// ---- PAT-18: the run -----------------------------------------------------------------------

fn generic_rig() -> String {
    export(&[
        (185, "Generic", "PAR Spot", 1, "P27"),
        (186, "Generic", "PAR Spot", 1, "P28"),
        (361, "Generic", "PAR Spot (RGBL)", 4, "P37"),
        (490, "Generic", "PAR Spot (RGB)", 3, "P42"),
    ])
}

#[test]
fn pat_18_run_writes_the_show_kerk_and_the_user_profiles() {
    let d = dirs("pat-18-run");
    let report = run(&generic_rig(), &d).unwrap();

    let json = fs::read_to_string(d.show_dir.join("Kerk.cmdmx.json")).unwrap();
    let (show, _) = Show::from_json(&json).unwrap();
    assert_eq!(show.meta.name, "Kerk");
    let placed: Vec<(u32, &str, &str, Option<&str>, u16, u16)> = show
        .patch
        .fixtures
        .iter()
        .map(|f| {
            (
                f.number,
                f.name.as_str(),
                f.profile.as_str(),
                f.mode.as_deref(),
                f.universe,
                f.address,
            )
        })
        .collect();
    assert_eq!(
        placed,
        vec![
            (101, "P27", "Generic/PAR Spot", Some("1 Channel"), 1, 185),
            (102, "P28", "Generic/PAR Spot", Some("1 Channel"), 1, 186),
            (
                201,
                "P37",
                "Generic/PAR Spot (RGBL)",
                Some("4 Channel"),
                1,
                361
            ),
            (
                301,
                "P42",
                "Generic/PAR Spot (RGB)",
                Some("3 Channel"),
                1,
                490
            ),
        ]
    );
    // Patched in number order, so the handles follow the numbers (PAT-14).
    let rgbl = show
        .patch
        .fixtures
        .iter()
        .find(|f| f.number == 201)
        .unwrap();
    assert_eq!(rgbl.handle, Some(Handle { page: 1, slot: 3 }));
    assert_eq!(
        show.profiles.keys().map(String::as_str).collect::<Vec<_>>(),
        vec![
            "Generic/PAR Spot",
            "Generic/PAR Spot (RGB)",
            "Generic/PAR Spot (RGBL)"
        ]
    );

    assert_eq!(
        files_in(&d.profiles_dir),
        vec![
            "Generic - PAR Spot (RGB).json",
            "Generic - PAR Spot (RGBL).json",
            "Generic - PAR Spot.json"
        ]
    );
    let saved = Profile::from_json(
        &fs::read_to_string(d.profiles_dir.join("Generic - PAR Spot.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(Some(&saved), show.profiles.get("Generic/PAR Spot"));

    assert!(
        report.contains("| Number | Name | Profile | Mode | Address | Footprint |"),
        "{report}"
    );
    assert!(
        report.contains("| 101 | P27 | Generic/PAR Spot | 1 Channel | 1.185 | 1 |"),
        "{report}"
    );
    assert!(
        report.contains("| 201 | P37 | Generic/PAR Spot (RGBL) | 4 Channel | 1.361 | 4 |"),
        "{report}"
    );
}

#[test]
fn pat_18_an_existing_show_is_never_overwritten() {
    let d = dirs("pat-18-exists");
    run(&generic_rig(), &d).unwrap();
    let path = d.show_dir.join("Kerk.cmdmx.json");
    fs::write(&path, "the owner's show").unwrap();
    let err = run(&generic_rig(), &d).unwrap_err();
    assert!(err.contains("Kerk"), "{err}");
    assert_eq!(fs::read_to_string(&path).unwrap(), "the owner's show");
}

#[test]
fn pat_18_an_existing_user_profile_is_kept_and_reported() {
    let d = dirs("pat-18-keep");
    fs::create_dir_all(&d.profiles_dir).unwrap();
    let mine = d.profiles_dir.join("Generic - PAR Spot.json");
    fs::write(&mine, "the owner's profile").unwrap();
    let report = run(&generic_rig(), &d).unwrap();
    assert_eq!(fs::read_to_string(&mine).unwrap(), "the owner's profile");
    assert!(report.contains("Generic - PAR Spot.json"), "{report}");
    assert!(d.show_dir.join("Kerk.cmdmx.json").is_file());
}

#[test]
fn pat_18_a_clash_refuses_the_run_and_writes_nothing() {
    let d = dirs("pat-18-clash");
    // The RGBL par at 184 covers 184..187, over the dimmer at 185.
    let tsv = export(&[
        (185, "Generic", "PAR Spot", 1, "P27"),
        (184, "Generic", "PAR Spot (RGBL)", 4, "P37"),
    ]);
    assert!(run(&tsv, &d).is_err());
    assert!(files_in(&d.show_dir).is_empty());
    assert!(files_in(&d.profiles_dir).is_empty());
}
