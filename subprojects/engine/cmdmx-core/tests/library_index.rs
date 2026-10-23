//! Library search index (VS2-11): `specs/modules/fixture-library.md` LIB-12..14.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use cmdmx_core::library::{Entry, EntryMode, Index, Manufacturer};

fn entry(manufacturer: &str, model: &str, source: &str) -> Entry {
    Entry {
        manufacturer: manufacturer.into(),
        model: model.into(),
        modes: vec![EntryMode {
            name: "M".into(),
            channels: 1,
        }],
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
fn lib_19_entries_serialise_modes_with_channels_without_their_path() {
    let json = serde_json::to_value(entry("Showtec", "LED Par 56", "qlcplus")).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"manufacturer":"Showtec","model":"LED Par 56","modes":[{"name":"M","channels":1}],"source":"qlcplus"})
    );
}

fn manufacturers(index: &Index, query: &str) -> Vec<(String, usize)> {
    index
        .manufacturers(query)
        .into_iter()
        .map(|Manufacturer { name, models }| (name, models))
        .collect()
}

fn models(index: &Index, manufacturer: Option<&str>, query: &str) -> (Vec<String>, bool) {
    let (found, truncated) = index.models(manufacturer, query, 500);
    let names = found
        .into_iter()
        .map(|e| format!("{}/{}", e.manufacturer, e.model))
        .collect();
    (names, truncated)
}

#[test]
fn lib_20_manufacturers_once_sorted_case_insensitively_with_counts() {
    let mut i = index();
    i.insert(entry("beamZ", "Wash 7", "qlcplus"));
    i.insert(entry("ADJ", "Mega Par", "qlcplus"));
    assert_eq!(
        manufacturers(&i, ""),
        [
            ("ADJ".into(), 1),
            ("beamZ".into(), 1),
            ("Pro-Lights".into(), 1),
            ("Shehds".into(), 2),
            ("Showtec".into(), 2),
        ]
    );
    // A user profile that hides a QLC+ one counts once (LIB-12).
    i.insert(entry("Showtec", "LED Par 56", "user"));
    assert!(manufacturers(&i, "").contains(&("Showtec".into(), 2)));
    assert_eq!(
        serde_json::to_value(&i.manufacturers("")[0]).unwrap(),
        serde_json::json!({"name":"ADJ","models":1})
    );
}

#[test]
fn lib_20_a_query_keeps_matching_manufacturers_and_counts_matches() {
    let i = index();
    // Word rule of LIB-14 over "manufacturer model".
    assert_eq!(
        manufacturers(&i, "par"),
        [
            ("Pro-Lights".into(), 1),
            ("Shehds".into(), 1),
            ("Showtec".into(), 1),
        ]
    );
    assert_eq!(manufacturers(&i, "SHOWTEC"), [("Showtec".into(), 2)]);
    assert!(manufacturers(&i, "martin").is_empty());
}

#[test]
fn lib_21_models_of_one_manufacturer_sorted_case_insensitively() {
    let mut i = index();
    i.insert(entry("Showtec", "compact Par 7", "qlcplus"));
    assert_eq!(
        models(&i, Some("Showtec"), ""),
        (
            vec![
                "Showtec/compact Par 7".into(),
                "Showtec/LED Par 56".into(),
                "Showtec/Spectral M800".into(),
            ],
            false
        )
    );
    assert_eq!(
        models(&i, Some("Showtec"), "par"),
        (
            vec!["Showtec/compact Par 7".into(), "Showtec/LED Par 56".into()],
            false
        )
    );
    assert_eq!(models(&i, Some("Nope"), ""), (vec![], false));
    // Entries keep their modes and source.
    let (found, _) = i.models(Some("Showtec"), "56", 500);
    assert_eq!(found[0].modes[0].channels, 1);
    assert_eq!(found[0].source, "qlcplus");
}

#[test]
fn lib_21_a_query_without_manufacturer_spans_all_sorted_by_manufacturer_then_model() {
    let mut i = index();
    i.insert(entry("beamZ", "LED Par 64", "qlcplus"));
    assert_eq!(
        models(&i, None, "par"),
        (
            vec![
                "beamZ/LED Par 64".into(),
                "Pro-Lights/LumiPar 7QPro".into(),
                "Shehds/LED Flat Par 18x18W".into(),
                "Showtec/LED Par 56".into(),
            ],
            false
        )
    );
}

#[test]
fn lib_21_without_manufacturer_at_most_the_limit_with_truncated() {
    let mut i = Index::default();
    for n in 0..600 {
        i.insert(entry("Gen", &format!("Par {n:03}"), "qlcplus"));
    }
    let (found, truncated) = i.models(None, "par", 500);
    assert_eq!((found.len(), truncated), (500, true));
    assert_eq!(found[0].model, "Par 000");
    let (found, truncated) = i.models(None, "par 599", 500);
    assert_eq!((found.len(), truncated), (1, false));
    // One manufacturer's models are never capped.
    let (found, truncated) = i.models(Some("Gen"), "", 500);
    assert_eq!((found.len(), truncated), (600, false));
}
