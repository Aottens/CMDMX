//! Library search index (VS2-11): `specs/modules/fixture-library.md` LIB-12..14.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use cmdmx_core::library::{Entry, Index};

fn entry(manufacturer: &str, model: &str, source: &str) -> Entry {
    Entry {
        manufacturer: manufacturer.into(),
        model: model.into(),
        modes: vec!["M".into()],
        source: source.into(),
        path: format!("{manufacturer}/{model}"),
    }
}

fn found(index: &Index, query: &str) -> Vec<(String, String, String)> {
    index
        .search(query, 50)
        .into_iter()
        .map(|e| (e.manufacturer.clone(), e.model.clone(), e.source.clone()))
        .collect()
}

fn index() -> Index {
    let mut i = Index::default();
    for (m, model) in [
        ("Shehds", "LED Beam+Wash 19x15W RGBW Zoom Lighting"),
        ("Showtec", "LED Par 56"),
        ("Showtec", "Spectral M800"),
        ("Pro-Lights", "LumiPar 7QPro"),
        ("Shehds", "LED Flat Par 18x18W"),
    ] {
        i.insert(entry(m, model, "qlcplus"));
    }
    i
}

#[test]
fn lib_14_every_word_must_match_case_insensitively_sorted() {
    let i = index();
    assert_eq!(
        found(&i, "shehds"),
        [
            (
                "Shehds".into(),
                "LED Beam+Wash 19x15W RGBW Zoom Lighting".into(),
                "qlcplus".into()
            ),
            (
                "Shehds".into(),
                "LED Flat Par 18x18W".into(),
                "qlcplus".into()
            ),
        ]
    );
    assert_eq!(found(&i, "PAR showtec").len(), 1);
    assert_eq!(
        found(&i, "  19X15   shehds "),
        [(
            "Shehds".into(),
            "LED Beam+Wash 19x15W RGBW Zoom Lighting".into(),
            "qlcplus".into()
        )]
    );
    assert!(found(&i, "showtec martin").is_empty());
    let all = i.search("", 50);
    assert_eq!(all.len(), 5, "an empty query lists everything");
    assert_eq!(all[0].manufacturer, "Pro-Lights");
}

#[test]
fn lib_14_at_most_the_limit() {
    let mut i = Index::default();
    for n in 0..120 {
        i.insert(entry("Gen", &format!("Par {n:03}"), "qlcplus"));
    }
    assert_eq!(i.search("gen", 50).len(), 50);
    assert_eq!(i.search("gen", 50)[0].model, "Par 000");
}

#[test]
fn lib_12_user_profile_hides_the_qlcplus_one() {
    let mut i = index();
    i.insert(entry("Showtec", "LED Par 56", "user"));
    assert_eq!(
        found(&i, "par 56"),
        [("Showtec".into(), "LED Par 56".into(), "user".into())]
    );
    // A QLC+ entry loaded later never replaces a user profile.
    i.insert(entry("Showtec", "LED Par 56", "qlcplus"));
    assert_eq!(i.get("Showtec", "LED Par 56").unwrap().source, "user");
    assert!(i.get("Showtec", "Nope").is_none());
}

#[test]
fn lib_14_entries_serialise_without_their_path() {
    let json = serde_json::to_value(entry("Showtec", "LED Par 56", "qlcplus")).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"manufacturer":"Showtec","model":"LED Par 56","modes":["M"],"source":"qlcplus"})
    );
}
