//! PBK-01, PBK-02: pages of playbacks (`specs/modules/playbacks.md`, VS1-06 subset). Since
//! P1-04 the playbacks are stored on handles (PBK-06); the rig stores them where PBK-01 put them.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

mod common;

use std::collections::BTreeMap;

use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::playback::{Playback, PAGES};
use cmdmx_core::programmer::Item;

/// The church rig with one playback per id in `ids`, each putting one wash's Dimmer at full.
fn rig(settings: Option<Settings>, ids: &[u32]) -> Engine {
    let mut e = common::church();
    if let Some(s) = settings {
        e = Engine::from_show(&{
            let mut show = e.to_show(cmdmx_core::show::Meta {
                name: "t".into(),
                created: String::new(),
                modified: String::new(),
                app_version: String::new(),
            });
            show.settings.faders_per_page = s.faders_per_page;
            show
        })
        .0;
    }
    // Each playback stored where PBK-01 put it (the schema 3 migration keeps that, PBK-45).
    let per_page = u32::from(e.settings().faders_per_page);
    for &id in ids {
        let page = u16::try_from((id - 1) / per_page + 1).unwrap();
        let slot = u16::try_from((id - 1) % per_page + 1).unwrap();
        e.add_playback(Playback::new(
            id,
            page,
            slot,
            BTreeMap::from([(
                Item::fixture(101),
                BTreeMap::from([("Dimmer".to_string(), 1.0.into())]),
            )]),
        ));
    }
    e
}

#[test]
fn pbk_01_ten_pages() {
    assert_eq!(PAGES, 10);
}

#[test]
fn pbk_01_slot_s_of_page_p_holds_playback_p_minus_1_times_per_page_plus_s() {
    let mut e = rig(None, &[1, 2, 8, 9, 10]);
    assert_eq!(e.page(), 1, "a new engine starts on page 1");
    let mut expected = vec![None; 8];
    expected[0] = Some(1);
    expected[1] = Some(2);
    expected[7] = Some(8);
    assert_eq!(e.page_slots(), expected);
    e.set_page(2).unwrap();
    let mut expected = vec![None; 8];
    expected[0] = Some(9);
    expected[1] = Some(10);
    assert_eq!(e.page_slots(), expected);
    e.set_page(10).unwrap();
    assert_eq!(e.page_slots(), vec![None; 8]);
}

#[test]
fn pbk_01_follows_faders_per_page() {
    let settings = Settings {
        faders_per_page: 10,
        ..Settings::default()
    };
    let mut e = rig(Some(settings), &[10, 11]);
    assert_eq!(e.page_slots().len(), 10);
    assert_eq!(e.page_slots()[9], Some(10));
    e.set_page(2).unwrap();
    assert_eq!(e.page_slots()[0], Some(11));
}

#[test]
fn pbk_02_page_0_and_11_are_refused() {
    let mut e = rig(None, &[1]);
    e.set_page(3).unwrap();
    assert!(e.set_page(0).is_err());
    assert!(e.set_page(11).is_err());
    assert_eq!(e.page(), 3, "nothing changes");
}

#[test]
fn pbk_02_a_page_change_keeps_playbacks_running() {
    let mut e = rig(None, &[1]);
    e.set_fader(1, 1.0, 0.0).unwrap();
    e.tick(0.0);
    let before = e.frame(1).map(|f| f.slots().to_vec());
    e.set_page(2).unwrap();
    e.tick(0.025);
    assert_eq!(e.frame(1).map(|f| f.slots().to_vec()), before);
    assert_eq!(e.playback_state(1).map(|s| s.level), Some(1.0));
    assert!(e.playback_state(1).is_some_and(|s| s.is_active()));
    e.set_page(1).unwrap();
    assert_eq!(e.playback_state(1).map(|s| s.level), Some(1.0));
}

#[test]
fn pbk_02_the_page_is_live_state_not_show_data() {
    let mut e = rig(None, &[1]);
    let meta = cmdmx_core::show::Meta {
        name: "t".into(),
        created: String::new(),
        modified: String::new(),
        app_version: String::new(),
    };
    let show = e.to_show(meta.clone());
    e.set_page(4).unwrap();
    assert_eq!(e.to_show(meta), show);
    assert_eq!(Engine::from_show(&show).0.page(), 1);
}
