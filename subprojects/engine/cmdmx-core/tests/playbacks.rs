//! Playbacks, core storage (VS2-20): `specs/modules/playbacks.md` PBK-06..14, 24, 31..34, 43..45.
//! Fader modes, times, release, groups, Flash/Swop/Latch and DBO on the frame are VS2-21
//! (scenarios); IPC and the app are VS2-22.
//!
//! API these tests fix (`cmdmx_core::playback` and `Engine`):
//! - `Playback { id, page, slot, legend, icon, halo, priority, fader_mode, kill_point,
//!   release_mask, release_time, paging, group, cues }` (field order = show file key order),
//!   `Playback::new(id, page, slot, values)` (one cue, everything else at its default);
//!   `Cue { delay, fade_in, fade_out, overlap, values }`, `Cue::new(values)`;
//!   `KillPoint::{FadeOutComplete, FaderAtZero}`, `Paging::{Unlocked, Locked, TransparentLock}`.
//!   `halo` is a `TargetColour` like every other colour in the core; the daemon sends sRGB.
//! - `Engine::record_playback(&PlaybackRecord) -> Result<PlaybackId, _>`,
//!   `PlaybackRecord::at(page, slot)` with `mask`, `record_by`, `mode` (`palette::RecordMode`;
//!   `QuickMerge` is the Update of PBK-13).
//! - `Engine::copy_playback(id, page, slot) -> Result<PlaybackId, _>`,
//!   `move_playback(id, page, slot)`, `delete_playback(id)`.
//! - `Engine::change_playback(id, &PlaybackChange)`: every field optional (`Option`, and
//!   `Option<Option<_>>` for the nullable `release_mask`, `release_time`, `group`); all or
//!   nothing.
//! - `Engine::set_playback_legend(id, legend, icon, halo)`, `playback_auto_icon(id)`,
//!   `playback_halo(id)` (the effective halo, PBK-33).
//! - `Engine::set_page_legend(page, legend)`, `page_legend(page) -> Option<&str>`.
//! - `Settings` gains `cue_record_by` (default `Channel`), `cue_delay`, `cue_fade_in`,
//!   `cue_fade_out` (0), `cue_overlap` (100), `release_mask` (empty), `release_time` (2.0);
//!   `Engine::set_playback_settings(&PlaybackSettingsChange)` (fields optional, all or nothing).
//! - `Engine::add_playback(Playback)` stays the unchecked setup call for tests and scenarios (it
//!   raises the next id above the playback's); `Engine::playbacks()` iterates them by id.
//! - `Show.pages: Vec<ShowPage { number, legend }>` (pages with a legend) and
//!   `Show.playbacks: ShowPlaybacks { next_id, playbacks }`; `SCHEMA_VERSION` 3.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

mod common;

use std::collections::BTreeMap;

use cmdmx_core::attribute::AttributeGroup::{self, C, I, P};
use cmdmx_core::colour::TargetColour;
use cmdmx_core::engine::{Engine, PlaybackSettingsChange, Settings};
use cmdmx_core::palette::{AutoIcon, Cell, Record, RecordBy, RecordMode, Timing, Window};
use cmdmx_core::patch::Fixture;
use cmdmx_core::playback::{
    Cue, FaderMode, KillPoint, Paging, Playback, PlaybackChange, PlaybackRecord, Priority, Values,
};
use cmdmx_core::profile::Profile;
use cmdmx_core::show::{Meta, Show, SCHEMA_VERSION};
use cmdmx_core::value::{Reference, Stored, Value};
use serde_json::{json, Value as Json};

const V2_SHOW: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/fixtures/shows/church-demo-v2.cmdmx.json"
);

/// Plain RGB par, virtual dimmer.
const RGB: &str = r#"{ "manufacturer": "T", "model": "RGB Par", "source": "channel-list", "modes": [ {
  "name": "3ch", "footprint": 3, "virtual_dimmer": true,
  "colour": { "system": "RGB", "components": [ { "channel": 1 }, { "channel": 2 }, { "channel": 3 } ] } } ] }"#;

/// `common::church()` (washes 101–104 at 1, 6, 11, 16 with Dimmer on their first channel;
/// dimmers 201–208, 221–226, 301–306 from 21) plus RGB pars 501 and 502 on universe 2.
fn rig() -> Engine {
    let mut e = common::church();
    e.add_profile("rgb", Profile::from_json(RGB).unwrap())
        .unwrap();
    for (id, address) in [(501, 1), (502, 4)] {
        e.patch(Fixture {
            id,
            number: 0,
            name: String::new(),
            handle: None,
            profile: "rgb".into(),
            mode: None,
            universe: 2,
            address,
        })
        .unwrap();
    }
    e
}

fn meta() -> Meta {
    Meta {
        name: "t".into(),
        created: String::new(),
        modified: String::new(),
        app_version: String::new(),
    }
}

fn rgb(r: f32, g: f32, b: f32) -> TargetColour {
    TargetColour { r, g, b }
}

fn values(entries: &[(u32, &str, Stored)]) -> Values {
    let mut v: Values = BTreeMap::new();
    for (id, name, stored) in entries {
        v.entry(*id)
            .or_default()
            .insert((*name).to_string(), *stored);
    }
    v
}

fn dimmers(ids: &[u32], level: f32) -> Values {
    values(
        &ids.iter()
            .map(|&id| (id, "Dimmer", level.into()))
            .collect::<Vec<_>>(),
    )
}

fn cue_values(e: &Engine, id: u32) -> &Values {
    &e.playback(id).unwrap().cues[0].values
}

fn set(e: &mut Engine, fixture: u32, attribute: &str, value: impl Into<Value>) {
    e.set_programmer_value(fixture, attribute, value).unwrap();
}

/// Empty selection and programmer.
fn clear_all(e: &mut Engine) {
    e.clear_selection();
    e.clear();
}

fn slots_on(e: &mut Engine, page: u16) -> Vec<Option<u32>> {
    e.set_page(page).unwrap();
    e.page_slots()
}

/// Palette 1 on Colours cell 1 holding 501 Colour `c`.
fn colour_palette(e: &mut Engine, c: TargetColour) {
    common::select(e, &[501]);
    set(e, 501, "Colour", c);
    e.record_palette(&Record::at(Cell::new(Window::Colours, 1)))
        .unwrap();
    clear_all(e);
}

fn dimmer_out(e: &mut Engine, t: f64, address: u16) -> u8 {
    e.tick(t);
    e.frame(1).unwrap().channel(address).unwrap()
}

// ---- PBK-06, 07: stored handles and paging locks -------------------------------

#[test]
fn pbk_06_a_slot_shows_the_playback_stored_on_it() {
    let mut e = rig();
    e.add_playback(Playback::new(7, 2, 3, dimmers(&[101], 1.0)));
    e.add_playback(Playback::new(3, 1, 1, dimmers(&[102], 1.0)));
    let mut page1 = vec![None; 8];
    page1[0] = Some(3);
    assert_eq!(slots_on(&mut e, 1), page1);
    let mut page2 = vec![None; 8];
    page2[2] = Some(7);
    assert_eq!(
        slots_on(&mut e, 2),
        page2,
        "the id says nothing about the place"
    );
}

#[test]
fn pbk_06_a_slot_above_faders_per_page_is_hidden_but_kept() {
    let mut e = rig();
    e.add_playback(Playback::new(1, 1, 10, dimmers(&[101], 1.0)));
    assert_eq!(e.page_slots(), vec![None; 8]);
    let show = e.to_show(meta());
    let mut show10 = show.clone();
    show10.settings.faders_per_page = 10;
    let (e10, warnings) = Engine::from_show(&show10);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(
        e10.page_slots()[9],
        Some(1),
        "reachable again when the setting grows"
    );
    let (e8, _) = Engine::from_show(&show);
    assert!(
        e8.playback(1).is_some(),
        "a smaller setting never deletes it"
    );
}

#[test]
fn pbk_07_locked_shows_on_every_page_and_covers_that_slot() {
    let mut e = rig();
    e.add_playback(Playback {
        paging: Paging::Locked,
        ..Playback::new(1, 1, 8, dimmers(&[301, 302, 303, 304, 305, 306], 1.0))
    });
    e.add_playback(Playback::new(2, 2, 8, dimmers(&[101], 1.0)));
    for page in [1, 2, 5, 10] {
        assert_eq!(slots_on(&mut e, page)[7], Some(1), "page {page}");
    }
}

#[test]
fn pbk_07_transparent_lock_shows_only_where_the_slot_is_empty() {
    let mut e = rig();
    e.add_playback(Playback {
        paging: Paging::TransparentLock,
        ..Playback::new(1, 1, 8, dimmers(&[301], 1.0))
    });
    e.add_playback(Playback::new(2, 2, 8, dimmers(&[101], 1.0)));
    assert_eq!(slots_on(&mut e, 1)[7], Some(1));
    assert_eq!(
        slots_on(&mut e, 2)[7],
        Some(2),
        "page 2 has its own playback there"
    );
    assert_eq!(slots_on(&mut e, 3)[7], Some(1));
}

#[test]
fn pbk_07_unlocked_shows_only_on_its_home_page() {
    let mut e = rig();
    e.add_playback(Playback::new(1, 1, 8, dimmers(&[301], 1.0)));
    assert_eq!(slots_on(&mut e, 1)[7], Some(1));
    assert_eq!(slots_on(&mut e, 2)[7], None);
}

#[test]
fn pbk_07_a_second_locked_playback_on_a_slot_is_refused() {
    let mut e = rig();
    e.add_playback(Playback {
        paging: Paging::Locked,
        ..Playback::new(1, 1, 8, dimmers(&[301], 1.0))
    });
    e.add_playback(Playback::new(2, 2, 8, dimmers(&[101], 1.0)));
    let lock = PlaybackChange {
        paging: Some(Paging::Locked),
        ..PlaybackChange::default()
    };
    assert!(e.change_playback(2, &lock).is_err());
    assert_eq!(e.playback(2).unwrap().paging, Paging::Unlocked);
    // Another slot is fine.
    e.add_playback(Playback::new(3, 2, 7, dimmers(&[102], 1.0)));
    e.change_playback(3, &lock).unwrap();
    assert_eq!(e.playback(3).unwrap().paging, Paging::Locked);
}

#[test]
fn pbk_07_record_onto_a_locked_slot_addresses_the_locked_playback() {
    let mut e = rig();
    e.add_playback(Playback {
        paging: Paging::Locked,
        ..Playback::new(1, 1, 8, dimmers(&[301], 1.0))
    });
    common::select(&mut e, &[302]);
    set(&mut e, 302, "Dimmer", 1.0);
    let err = e.record_playback(&PlaybackRecord::at(2, 8)).unwrap_err();
    assert!(err.to_string().contains("is used"), "{err}");
    let id = e
        .record_playback(&PlaybackRecord {
            mode: Some(RecordMode::Merge),
            ..PlaybackRecord::at(2, 8)
        })
        .unwrap();
    assert_eq!(id, 1);
    assert!(cue_values(&e, 1).contains_key(&302));
    assert_eq!(e.playbacks().count(), 1);
}

#[test]
fn pbk_07_record_onto_a_transparent_slot_makes_a_playback_that_covers_it() {
    let mut e = rig();
    e.add_playback(Playback {
        paging: Paging::TransparentLock,
        ..Playback::new(1, 1, 8, dimmers(&[301], 1.0))
    });
    common::select(&mut e, &[302]);
    set(&mut e, 302, "Dimmer", 1.0);
    let id = e.record_playback(&PlaybackRecord::at(2, 8)).unwrap();
    assert_ne!(id, 1);
    let pb = e.playback(id).unwrap();
    assert_eq!((pb.page, pb.slot), (2, 8));
    assert_eq!(slots_on(&mut e, 2)[7], Some(id));
    assert_eq!(slots_on(&mut e, 3)[7], Some(1));
}

// ---- PBK-08: page legends -------------------------------------------------------

#[test]
fn pbk_08_page_legends() {
    let mut e = rig();
    assert_eq!(e.page_legend(2), Some(""));
    e.set_page_legend(2, "Service").unwrap();
    assert_eq!(e.page_legend(2), Some("Service"));
    assert!(e.set_page_legend(2, &"x".repeat(33)).is_err());
    assert!(e.set_page_legend(0, "x").is_err());
    assert!(e.set_page_legend(11, "x").is_err());
    assert_eq!(e.page_legend(2), Some("Service"), "refusals change nothing");
    assert_eq!(e.page_legend(11), None);
}

// ---- PBK-09..12: recording ---------------------------------------------------------

#[test]
fn pbk_09_record_onto_an_empty_handle_stores_the_whole_programmer() {
    let mut e = rig();
    common::select(&mut e, &[101, 102]);
    set(&mut e, 101, "Dimmer", 1.0);
    set(&mut e, 102, "Pan", 0.25);
    e.clear_selection(); // not only the selected fixtures: everything in the programmer
    let id = e.record_playback(&PlaybackRecord::at(1, 1)).unwrap();
    let pb = e.playback(id).unwrap();
    assert_eq!((pb.page, pb.slot), (1, 1));
    assert_eq!(pb.cues.len(), 1);
    assert_eq!(
        pb.cues[0].values,
        values(&[(101, "Dimmer", 1.0.into()), (102, "Pan", 0.25.into())])
    );
    assert_eq!(pb.priority, Priority::Normal);
    assert_eq!(pb.fader_mode, FaderMode::Mode0);
    assert_eq!(pb.kill_point, KillPoint::FadeOutComplete);
    assert_eq!(pb.paging, Paging::Unlocked);
    assert_eq!(
        (pb.release_mask.clone(), pb.release_time, pb.group),
        (None, None, None)
    );
    assert_eq!(
        (pb.legend.as_str(), pb.icon.clone(), pb.halo),
        ("", None, None)
    );
    assert_eq!(e.page_slots()[0], Some(id));
    assert!(
        e.programmer().stored(101, "Dimmer").is_some(),
        "the programmer is left as it is"
    );
}

#[test]
fn pbk_09_mask_limits_what_is_recorded() {
    let mut e = rig();
    common::select(&mut e, &[101]);
    set(&mut e, 101, "Dimmer", 1.0);
    set(&mut e, 101, "Pan", 0.25);
    let id = e
        .record_playback(&PlaybackRecord {
            mask: Some(vec![P]),
            ..PlaybackRecord::at(1, 1)
        })
        .unwrap();
    assert_eq!(*cue_values(&e, id), values(&[(101, "Pan", 0.25.into())]));
}

#[test]
fn pbk_09_nothing_in_the_mask_is_refused() {
    let mut e = rig();
    let err = e.record_playback(&PlaybackRecord::at(1, 1)).unwrap_err();
    assert!(
        err.to_string().contains("Nothing in the programmer"),
        "{err}"
    );
    common::select(&mut e, &[101]);
    set(&mut e, 101, "Dimmer", 1.0);
    assert!(e
        .record_playback(&PlaybackRecord {
            mask: Some(vec![C]),
            ..PlaybackRecord::at(1, 1)
        })
        .is_err());
    assert_eq!(e.playbacks().count(), 0);
}

#[test]
fn pbk_09_a_handle_outside_the_pages_is_refused() {
    let mut e = rig();
    common::select(&mut e, &[101]);
    set(&mut e, 101, "Dimmer", 1.0);
    for (page, slot) in [(0, 1), (11, 1), (1, 0), (1, 9)] {
        assert!(
            e.record_playback(&PlaybackRecord::at(page, slot)).is_err(),
            "{page}.{slot}"
        );
    }
    assert_eq!(e.playbacks().count(), 0);
}

#[test]
fn pbk_10_record_by_channel_is_the_default_mixed_takes_the_whole_group() {
    let mut e = rig();
    assert_eq!(e.settings().cue_record_by, RecordBy::Channel);
    common::select(&mut e, &[101]);
    set(&mut e, 101, "Pan", 0.25);
    let a = e.record_playback(&PlaybackRecord::at(1, 1)).unwrap();
    assert_eq!(*cue_values(&e, a), values(&[(101, "Pan", 0.25.into())]));
    let b = e
        .record_playback(&PlaybackRecord {
            record_by: Some(RecordBy::Mixed),
            ..PlaybackRecord::at(1, 2)
        })
        .unwrap();
    let attrs: Vec<&String> = cue_values(&e, b)[&101].keys().collect();
    assert_eq!(attrs, ["Pan", "Tilt"]);
}

#[test]
fn pbk_11_references_stay_references_manual_values_stay_values() {
    let mut e = rig();
    colour_palette(&mut e, rgb(1.0, 0.0, 0.0));
    let palette = e.palette(1).unwrap().id;
    common::select(&mut e, &[501]);
    e.apply_palette(1, Timing::default()).unwrap();
    e.clear_selection();
    common::select(&mut e, &[502]);
    set(&mut e, 502, "Colour", rgb(0.0, 0.0, 1.0));
    let id = e.record_playback(&PlaybackRecord::at(1, 1)).unwrap();
    let cue = cue_values(&e, id);
    assert_eq!(
        cue[&501]["Colour"],
        Stored::Reference(Reference {
            palette,
            value: Value::Colour(rgb(1.0, 0.0, 0.0)),
        }),
        "a reference to the same palette, fallback = what it resolves to when recording"
    );
    assert_eq!(
        cue[&502]["Colour"],
        Stored::Value(Value::Colour(rgb(0.0, 0.0, 1.0)))
    );
}

#[test]
fn pbk_11_a_recorded_reference_follows_the_palette() {
    let mut e = rig();
    colour_palette(&mut e, rgb(1.0, 0.0, 0.0));
    common::select(&mut e, &[501]);
    e.apply_palette(1, Timing::default()).unwrap();
    set(&mut e, 501, "Dimmer", 1.0); // virtual dimmer
    let id = e.record_playback(&PlaybackRecord::at(1, 1)).unwrap();
    clear_all(&mut e);
    e.set_fader(id, 1.0, 0.0).unwrap();
    e.tick(0.0);
    assert_eq!(e.frame(2).unwrap().channel(1), Some(255));
    // Palette 1 re-recorded blue: the playback turns blue on the next tick (PAL-11).
    common::select(&mut e, &[501]);
    set(&mut e, 501, "Colour", rgb(0.0, 0.0, 1.0));
    e.record_palette(&Record {
        mode: Some(RecordMode::Replace),
        ..Record::at(Cell::new(Window::Colours, 1))
    })
    .unwrap();
    clear_all(&mut e);
    e.tick(0.025);
    let f = e.frame(2).unwrap();
    assert_eq!((f.channel(1), f.channel(3)), (Some(0), Some(255)));
}

#[test]
fn pbk_12_new_cues_take_the_time_settings() {
    let mut e = rig();
    let defaults = e.settings().clone();
    assert_eq!(
        (
            defaults.cue_delay,
            defaults.cue_fade_in,
            defaults.cue_fade_out,
            defaults.cue_overlap
        ),
        (0.0, 0.0, 0.0, 100.0)
    );
    e.set_playback_settings(&PlaybackSettingsChange {
        fade_in: Some(3.0),
        fade_out: Some(4.0),
        delay: Some(0.5),
        overlap: Some(50.0),
        ..PlaybackSettingsChange::default()
    })
    .unwrap();
    common::select(&mut e, &[101]);
    set(&mut e, 101, "Dimmer", 1.0);
    let id = e.record_playback(&PlaybackRecord::at(1, 1)).unwrap();
    let cue = &e.playback(id).unwrap().cues[0];
    assert_eq!(
        (cue.delay, cue.fade_in, cue.fade_out, cue.overlap),
        (0.5, 3.0, 4.0, 50.0)
    );
}

#[test]
fn pbk_12_out_of_range_settings_are_refused_as_a_whole() {
    let mut e = rig();
    let before = e.settings().clone();
    for change in [
        PlaybackSettingsChange {
            fade_in: Some(601.0),
            ..PlaybackSettingsChange::default()
        },
        PlaybackSettingsChange {
            fade_out: Some(2.0),
            delay: Some(-1.0),
            ..PlaybackSettingsChange::default()
        },
        PlaybackSettingsChange {
            overlap: Some(101.0),
            ..PlaybackSettingsChange::default()
        },
        PlaybackSettingsChange {
            release_time: Some(f32::NAN),
            ..PlaybackSettingsChange::default()
        },
    ] {
        assert!(e.set_playback_settings(&change).is_err(), "{change:?}");
    }
    assert_eq!(*e.settings(), before);
}

// ---- PBK-13: recording onto a used handle -----------------------------------------------

/// Playback 1 on 1.1: 101 Dimmer 1.0, 102 Dimmer 1.0, legend "Walk-in", fader mode 1, fade 3 s.
fn used_handle() -> Engine {
    let mut e = rig();
    let mut pb = Playback {
        legend: "Walk-in".into(),
        fader_mode: FaderMode::Mode1,
        ..Playback::new(1, 1, 1, dimmers(&[101, 102], 1.0))
    };
    pb.cues[0].fade_in = 3.0;
    e.add_playback(pb);
    // Programmer: 101 Dimmer 0.4 + Pan 0.7, 103 Dimmer 0.3.
    common::select(&mut e, &[101, 103]);
    set(&mut e, 101, "Dimmer", 0.4);
    set(&mut e, 101, "Pan", 0.7);
    set(&mut e, 103, "Dimmer", 0.3);
    e
}

#[test]
fn pbk_13_a_used_handle_needs_a_mode() {
    let mut e = used_handle();
    let before = e.playback(1).unwrap().clone();
    let err = e.record_playback(&PlaybackRecord::at(1, 1)).unwrap_err();
    assert_eq!(err.to_string(), "Handle 1.1 is used: Replace or Merge");
    assert_eq!(*e.playback(1).unwrap(), before);
}

#[test]
fn pbk_13_replace_keeps_everything_but_the_values() {
    let mut e = used_handle();
    let id = e
        .record_playback(&PlaybackRecord {
            mode: Some(RecordMode::Replace),
            ..PlaybackRecord::at(1, 1)
        })
        .unwrap();
    assert_eq!(id, 1);
    let pb = e.playback(1).unwrap();
    assert_eq!(
        pb.cues[0].values,
        values(&[
            (101, "Dimmer", 0.4.into()),
            (101, "Pan", 0.7.into()),
            (103, "Dimmer", 0.3.into())
        ])
    );
    assert_eq!(
        (pb.legend.as_str(), pb.fader_mode),
        ("Walk-in", FaderMode::Mode1)
    );
    assert_eq!(pb.cues[0].fade_in, 3.0, "times stay");
}

#[test]
fn pbk_13_merge_writes_over_and_keeps_the_rest() {
    let mut e = used_handle();
    e.record_playback(&PlaybackRecord {
        mode: Some(RecordMode::Merge),
        ..PlaybackRecord::at(1, 1)
    })
    .unwrap();
    assert_eq!(
        *cue_values(&e, 1),
        values(&[
            (101, "Dimmer", 0.4.into()),
            (101, "Pan", 0.7.into()),
            (102, "Dimmer", 1.0.into()),
            (103, "Dimmer", 0.3.into())
        ])
    );
}

#[test]
fn pbk_13_update_only_touches_what_the_cue_already_holds() {
    let mut e = used_handle();
    e.record_playback(&PlaybackRecord {
        mode: Some(RecordMode::QuickMerge),
        ..PlaybackRecord::at(1, 1)
    })
    .unwrap();
    assert_eq!(
        *cue_values(&e, 1),
        values(&[(101, "Dimmer", 0.4.into()), (102, "Dimmer", 1.0.into())])
    );
}

#[test]
fn pbk_13_update_with_nothing_the_cue_holds_is_refused() {
    let mut e = used_handle();
    clear_all(&mut e);
    common::select(&mut e, &[104]);
    set(&mut e, 104, "Dimmer", 0.5);
    let before = e.playback(1).unwrap().clone();
    assert!(e
        .record_playback(&PlaybackRecord {
            mode: Some(RecordMode::QuickMerge),
            ..PlaybackRecord::at(1, 1)
        })
        .is_err());
    assert_eq!(*e.playback(1).unwrap(), before);
}

#[test]
fn pbk_13_an_active_playback_changes_its_output_on_the_next_tick() {
    let mut e = rig();
    e.add_playback(Playback::new(1, 1, 1, dimmers(&[101], 1.0)));
    e.set_fader(1, 1.0, 0.0).unwrap();
    assert_eq!(dimmer_out(&mut e, 0.0, 1), 255);
    common::select(&mut e, &[101]);
    set(&mut e, 101, "Dimmer", 0.5);
    e.record_playback(&PlaybackRecord {
        mode: Some(RecordMode::Replace),
        ..PlaybackRecord::at(1, 1)
    })
    .unwrap();
    clear_all(&mut e);
    assert_eq!(dimmer_out(&mut e, 0.025, 1), 128);
}

#[test]
fn pbk_13_ids_are_never_reused() {
    let mut e = rig();
    common::select(&mut e, &[101]);
    set(&mut e, 101, "Dimmer", 1.0);
    let a = e.record_playback(&PlaybackRecord::at(1, 1)).unwrap();
    e.delete_playback(a).unwrap();
    let b = e.record_playback(&PlaybackRecord::at(1, 1)).unwrap();
    assert!(b > a);
    let (loaded, _) = Engine::from_show(&e.to_show(meta()));
    let mut loaded = loaded;
    common::select(&mut loaded, &[101]);
    set(&mut loaded, 101, "Dimmer", 1.0);
    let c = loaded.record_playback(&PlaybackRecord::at(1, 2)).unwrap();
    assert!(c > b, "next id survives a save and load");
}

// ---- PBK-14: Copy, Move, Delete, palette and patch changes ---------------------------------

fn styled() -> Engine {
    let mut e = rig();
    let mut pb = Playback {
        legend: "Front".into(),
        icon: Some("fixture.par".into()),
        halo: Some(rgb(1.0, 0.5, 0.0)),
        priority: Priority::High,
        group: Some(3),
        ..Playback::new(1, 1, 1, dimmers(&[301, 302], 1.0))
    };
    pb.cues[0].fade_in = 2.0;
    e.add_playback(pb);
    e
}

#[test]
fn pbk_14_copy_makes_a_new_playback_with_everything() {
    let mut e = styled();
    let id = e.copy_playback(1, 2, 4).unwrap();
    assert_ne!(id, 1);
    let (a, b) = (e.playback(1).unwrap(), e.playback(id).unwrap());
    assert_eq!((b.page, b.slot), (2, 4));
    assert_eq!(
        Playback {
            id: 1,
            page: 1,
            slot: 1,
            ..b.clone()
        },
        *a
    );
}

#[test]
fn pbk_14_move_keeps_the_id_and_a_running_playback_runs_on() {
    let mut e = styled();
    e.set_fader(1, 0.5, 0.0).unwrap();
    e.move_playback(1, 3, 2).unwrap();
    let pb = e.playback(1).unwrap();
    assert_eq!((pb.page, pb.slot), (3, 2));
    let state = e.playback_state(1).unwrap();
    assert!(state.is_active());
    assert_eq!(state.level, 0.5);
    assert_eq!(slots_on(&mut e, 1)[0], None);
    assert_eq!(slots_on(&mut e, 3)[1], Some(1));
}

#[test]
fn pbk_14_copy_or_move_onto_a_used_handle_is_refused() {
    let mut e = styled();
    e.add_playback(Playback::new(2, 2, 4, dimmers(&[101], 1.0)));
    assert!(e.copy_playback(1, 2, 4).is_err());
    assert!(e.move_playback(1, 2, 4).is_err());
    assert!(e.move_playback(1, 11, 1).is_err());
    assert!(e.copy_playback(9, 3, 3).is_err(), "unknown playback");
    assert_eq!(e.playbacks().count(), 2);
    let pb = e.playback(1).unwrap();
    assert_eq!((pb.page, pb.slot), (1, 1));
}

#[test]
fn pbk_14_delete_kills_and_removes() {
    let mut e = styled();
    e.set_fader(1, 1.0, 0.0).unwrap();
    e.tick(0.0);
    assert_eq!(
        dimmer_out(&mut e, 2.0, 35),
        255,
        "after its 2 s fade-in (PBK-15)"
    );
    e.delete_playback(1).unwrap();
    assert!(e.playback(1).is_none());
    assert!(e.playback_state(1).is_none());
    assert_eq!(dimmer_out(&mut e, 2.025, 35), 0);
    assert!(e.set_fader(1, 0.5, 2.05).is_err());
    assert!(e.delete_playback(1).is_err());
    assert_eq!(e.page_slots()[0], None);
}

#[test]
fn pbk_14_deleting_a_palette_leaves_the_fallback_in_every_cue() {
    let mut e = rig();
    colour_palette(&mut e, rgb(1.0, 0.0, 0.0));
    let palette = e.palette(1).unwrap().id;
    e.add_playback(Playback::new(
        1,
        1,
        1,
        values(&[(
            501,
            "Colour",
            Stored::Reference(Reference {
                palette,
                value: Value::Colour(rgb(1.0, 0.0, 0.0)),
            }),
        )]),
    ));
    e.delete_palette(1).unwrap();
    assert_eq!(
        cue_values(&e, 1)[&501]["Colour"],
        Stored::Value(Value::Colour(rgb(1.0, 0.0, 0.0)))
    );
}

#[test]
fn pbk_14_removing_a_fixture_removes_its_values_the_playback_stays() {
    let mut e = rig();
    e.add_playback(Playback::new(1, 1, 1, dimmers(&[301], 1.0)));
    e.add_playback(Playback::new(2, 1, 2, dimmers(&[301, 302], 1.0)));
    e.patch_remove(&[301]).unwrap();
    assert!(cue_values(&e, 1).is_empty());
    assert_eq!(*cue_values(&e, 2), dimmers(&[302], 1.0));
}

// ---- PBK-24 and options --------------------------------------------------------------------

#[test]
fn pbk_24_a_changed_priority_applies_from_the_next_tick() {
    let mut e = rig();
    e.add_playback(Playback::new(1, 1, 1, dimmers(&[101], 1.0)));
    e.add_playback(Playback::new(2, 1, 2, dimmers(&[101], 0.2)));
    e.set_fader(1, 1.0, 0.0).unwrap();
    e.set_fader(2, 1.0, 0.0).unwrap();
    assert_eq!(dimmer_out(&mut e, 0.0, 1), 255, "HTP: highest wins");
    e.change_playback(
        2,
        &PlaybackChange {
            priority: Some(Priority::High),
            ..PlaybackChange::default()
        },
    )
    .unwrap();
    assert_eq!(
        dimmer_out(&mut e, 0.025, 1),
        51,
        "the High playback owns the dimmer"
    );
}

#[test]
fn pbk_24_change_sets_options_and_times() {
    let mut e = rig();
    e.add_playback(Playback::new(1, 1, 1, dimmers(&[101], 1.0)));
    e.change_playback(
        1,
        &PlaybackChange {
            fader_mode: Some(FaderMode::Mode2),
            kill_point: Some(KillPoint::FaderAtZero),
            release_mask: Some(Some(vec![I, AttributeGroup::C])),
            release_time: Some(Some(0.5)),
            group: Some(Some(1)),
            fade_in: Some(3.0),
            fade_out: Some(4.0),
            delay: Some(1.0),
            overlap: Some(50.0),
            ..PlaybackChange::default()
        },
    )
    .unwrap();
    let pb = e.playback(1).unwrap();
    assert_eq!(pb.fader_mode, FaderMode::Mode2);
    assert_eq!(pb.kill_point, KillPoint::FaderAtZero);
    assert_eq!(pb.release_mask, Some(vec![I, AttributeGroup::C]));
    assert_eq!((pb.release_time, pb.group), (Some(0.5), Some(1)));
    let cue = &pb.cues[0];
    assert_eq!(
        (cue.delay, cue.fade_in, cue.fade_out, cue.overlap),
        (1.0, 3.0, 4.0, 50.0)
    );
    // Back to the global mask and time.
    e.change_playback(
        1,
        &PlaybackChange {
            release_mask: Some(None),
            release_time: Some(None),
            group: Some(None),
            ..PlaybackChange::default()
        },
    )
    .unwrap();
    let pb = e.playback(1).unwrap();
    assert_eq!(
        (pb.release_mask.clone(), pb.release_time, pb.group),
        (None, None, None)
    );
}

#[test]
fn pbk_24_an_out_of_range_change_is_refused_as_a_whole() {
    let mut e = rig();
    e.add_playback(Playback::new(1, 1, 1, dimmers(&[101], 1.0)));
    let before = e.playback(1).unwrap().clone();
    for change in [
        PlaybackChange {
            priority: Some(Priority::High),
            fade_in: Some(601.0),
            ..PlaybackChange::default()
        },
        PlaybackChange {
            overlap: Some(-1.0),
            ..PlaybackChange::default()
        },
        PlaybackChange {
            release_time: Some(Some(700.0)),
            ..PlaybackChange::default()
        },
        PlaybackChange {
            group: Some(Some(0)),
            ..PlaybackChange::default()
        },
        PlaybackChange {
            group: Some(Some(100)),
            ..PlaybackChange::default()
        },
    ] {
        assert!(e.change_playback(1, &change).is_err(), "{change:?}");
    }
    assert!(e.change_playback(9, &PlaybackChange::default()).is_err());
    assert_eq!(*e.playback(1).unwrap(), before);
}

// ---- PBK-31..33: legends, icons, halos -----------------------------------------------------

#[test]
fn pbk_31_legend_and_icon() {
    let mut e = rig();
    e.add_playback(Playback::new(1, 1, 1, dimmers(&[101], 1.0)));
    e.set_playback_legend(1, "Walk-in", Some("service.walk-in"), None)
        .unwrap();
    let pb = e.playback(1).unwrap();
    assert_eq!(
        (pb.legend.as_str(), pb.icon.as_deref()),
        ("Walk-in", Some("service.walk-in"))
    );
    assert!(e
        .set_playback_legend(1, &"x".repeat(33), None, None)
        .is_err());
    assert!(e
        .set_playback_legend(1, "x", Some("Bad Icon"), None)
        .is_err());
    assert!(e.set_playback_legend(9, "x", None, None).is_err());
    assert_eq!(
        e.playback(1).unwrap().legend,
        "Walk-in",
        "refusals change nothing"
    );
}

#[test]
fn pbk_32_automatic_icon_from_the_first_cue() {
    let mut e = rig();
    e.add_playback(Playback::new(
        1,
        1,
        1,
        values(&[
            (501, "Colour", rgb(1.0, 0.0, 0.0).into()),
            (502, "Colour", rgb(0.0, 0.0, 1.0).into()),
        ]),
    ));
    e.add_playback(Playback::new(2, 1, 2, values(&[(101, "Zoom", 0.4.into())])));
    assert_eq!(
        e.playback_auto_icon(1),
        Some(AutoIcon::Colours(vec![
            rgb(1.0, 0.0, 0.0),
            rgb(0.0, 0.0, 1.0)
        ]))
    );
    assert_eq!(
        e.playback_auto_icon(2),
        Some(AutoIcon::Value {
            attribute: "Zoom".into(),
            value: 0.4
        })
    );
    e.set_playback_legend(1, "", Some("colour.drop"), None)
        .unwrap();
    assert!(
        matches!(e.playback_auto_icon(1), Some(AutoIcon::Colours(_))),
        "a picked icon still reports the automatic one"
    );
}

#[test]
fn pbk_32_a_reference_counts_with_what_it_resolves_to_now() {
    let mut e = rig();
    colour_palette(&mut e, rgb(0.0, 0.0, 1.0));
    let palette = e.palette(1).unwrap().id;
    e.add_playback(Playback::new(
        1,
        1,
        1,
        values(&[(
            501,
            "Colour",
            Stored::Reference(Reference {
                palette,
                value: Value::Colour(rgb(1.0, 0.0, 0.0)),
            }),
        )]),
    ));
    assert_eq!(
        e.playback_auto_icon(1),
        Some(AutoIcon::Colours(vec![rgb(0.0, 0.0, 1.0)]))
    );
}

#[test]
fn pbk_33_halo_is_automatic_unless_picked() {
    let mut e = rig();
    e.add_playback(Playback::new(
        1,
        1,
        1,
        values(&[(501, "Colour", rgb(1.0, 0.0, 0.0).into())]),
    ));
    e.add_playback(Playback::new(2, 1, 2, dimmers(&[101], 1.0)));
    assert_eq!(e.playback_halo(1), Some(rgb(1.0, 0.0, 0.0)));
    assert_eq!(e.playback_halo(2), None, "no colour, no automatic halo");
    e.set_playback_legend(2, "", None, Some(rgb(0.0, 1.0, 0.0)))
        .unwrap();
    assert_eq!(e.playback_halo(2), Some(rgb(0.0, 1.0, 0.0)));
    assert_eq!(e.playback(2).unwrap().halo, Some(rgb(0.0, 1.0, 0.0)));
    assert!(e
        .set_playback_legend(2, "", None, Some(rgb(1.5, 0.0, 0.0)))
        .is_err());
}

// ---- PBK-34: show file schema 3 -------------------------------------------------------------

fn as_json(show: &Show) -> Json {
    serde_json::from_str(&show.to_json()).unwrap()
}

/// Each key appears after the previous one in `text`, searching from `from` on (the show has
/// one playback with one cue here, so a key out of order is not found).
fn keys_in_order(text: &str, from: &str, keys: &[&str]) {
    let mut at = text.find(from).unwrap();
    for key in keys {
        let pattern = format!("\"{key}\":");
        let i = text[at..]
            .find(&pattern)
            .unwrap_or_else(|| panic!("{key} out of order"));
        at += i + pattern.len();
    }
}

#[test]
fn pbk_34_schema_3_shape() {
    assert_eq!(SCHEMA_VERSION, 3);
    let mut e = styled();
    e.set_page_legend(2, "Service").unwrap();
    let text = e.to_show(meta()).to_json();
    let v: Json = serde_json::from_str(&text).unwrap();
    // Key order is read from the text (serde_json's `Value` sorts its keys).
    keys_in_order(&text, "", &["palettes", "pages", "playbacks"]);
    assert_eq!(v["pages"], json!([{"number": 2, "legend": "Service"}]));
    assert_eq!(v["playbacks"]["next_id"], json!(2));
    keys_in_order(
        &text,
        "\"playbacks\": [",
        &[
            "id",
            "page",
            "slot",
            "legend",
            "icon",
            "halo",
            "priority",
            "fader_mode",
            "kill_point",
            "release_mask",
            "release_time",
            "paging",
            "group",
            "cues",
            "delay",
            "fade_in",
            "fade_out",
            "overlap",
            "values",
        ],
    );
    let pb = &v["playbacks"]["playbacks"][0];
    assert_eq!(pb["paging"], json!("Unlocked"));
    assert_eq!(pb["kill_point"], json!("FadeOutComplete"));
    let s = &v["settings"];
    assert_eq!(s["cue_record_by"], json!("Channel"));
    assert_eq!(
        (
            &s["cue_delay"],
            &s["cue_fade_in"],
            &s["cue_fade_out"],
            &s["cue_overlap"]
        ),
        (&json!(0.0), &json!(0.0), &json!(0.0), &json!(100.0))
    );
    assert_eq!(
        (&s["release_mask"], &s["release_time"]),
        (&json!([]), &json!(2.0))
    );
}

#[test]
fn pbk_34_round_trip_keeps_playbacks_pages_and_settings() {
    let mut e = styled();
    e.set_page_legend(3, "Video").unwrap();
    e.change_playback(
        1,
        &PlaybackChange {
            paging: Some(Paging::Locked),
            release_mask: Some(Some(vec![I])),
            ..PlaybackChange::default()
        },
    )
    .unwrap();
    e.set_playback_settings(&PlaybackSettingsChange {
        release_time: Some(1.0),
        release_mask: Some(vec![C]),
        record_by: Some(RecordBy::Mixed),
        ..PlaybackSettingsChange::default()
    })
    .unwrap();
    let show = e.to_show(meta());
    let (loaded, warnings) = Engine::from_show(&show);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(loaded.to_show(meta()), show);
    assert_eq!(loaded.page_legend(3), Some("Video"));
    assert_eq!(loaded.settings().release_time, 1.0);
}

#[test]
fn pbk_34_live_state_is_not_saved() {
    let mut e = styled();
    let show = e.to_show(meta());
    e.set_fader(1, 0.7, 0.0).unwrap();
    e.set_page(3).unwrap();
    assert_eq!(e.to_show(meta()), show);
    let (loaded, _) = Engine::from_show(&show);
    assert!(!loaded.playback_state(1).unwrap().is_active());
}

// ---- PBK-43: determinism --------------------------------------------------------------------

#[test]
fn pbk_43_same_actions_same_playbacks() {
    let run = || {
        let mut e = rig();
        common::select(&mut e, &[101, 102]);
        set(&mut e, 101, "Dimmer", 1.0);
        set(&mut e, 102, "Pan", 0.3);
        let a = e.record_playback(&PlaybackRecord::at(1, 1)).unwrap();
        let b = e.copy_playback(a, 2, 1).unwrap();
        e.move_playback(a, 1, 5).unwrap();
        e.set_playback_legend(b, "Copy", None, None).unwrap();
        e.to_show(meta()).to_json()
    };
    assert_eq!(run(), run());
}

// ---- PBK-44: lenient load ------------------------------------------------------------------

fn show_json(e: &Engine) -> Json {
    as_json(&e.to_show(meta()))
}

fn load(v: &Json) -> (Engine, Vec<String>) {
    let (show, _) = Show::from_json(&v.to_string()).unwrap();
    Engine::from_show(&show)
}

fn two_playbacks() -> Engine {
    let mut e = rig();
    e.add_playback(Playback::new(1, 1, 1, dimmers(&[101], 1.0)));
    e.add_playback(Playback::new(2, 1, 2, dimmers(&[102], 1.0)));
    e
}

#[test]
fn pbk_44_a_second_playback_on_a_handle_or_id_is_left_out() {
    let mut v = show_json(&two_playbacks());
    v["playbacks"]["playbacks"][1]["slot"] = json!(1);
    let (e, warnings) = load(&v);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(e.playback(1).is_some() && e.playback(2).is_none());

    let mut v = show_json(&two_playbacks());
    v["playbacks"]["playbacks"][1]["id"] = json!(1);
    let (e, warnings) = load(&v);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(e.playbacks().count(), 1);
}

#[test]
fn pbk_44_a_handle_outside_the_pages_is_left_out() {
    for (page, slot) in [(0, 1), (11, 1), (1, 0)] {
        let mut v = show_json(&two_playbacks());
        v["playbacks"]["playbacks"][1]["page"] = json!(page);
        v["playbacks"]["playbacks"][1]["slot"] = json!(slot);
        let (e, warnings) = load(&v);
        assert_eq!(warnings.len(), 1, "{page}.{slot}: {warnings:?}");
        assert!(e.playback(2).is_none());
    }
}

#[test]
fn pbk_44_an_option_or_time_out_of_range_gets_its_default() {
    let mut v = show_json(&two_playbacks());
    v["playbacks"]["playbacks"][1]["cues"][0]["fade_in"] = json!(900.0);
    v["playbacks"]["playbacks"][1]["group"] = json!(100);
    v["playbacks"]["playbacks"][1]["release_time"] = json!(-2.0);
    let (e, warnings) = load(&v);
    assert_eq!(warnings.len(), 3, "{warnings:?}");
    let pb = e.playback(2).unwrap();
    assert_eq!(
        (pb.cues[0].fade_in, pb.group, pb.release_time),
        (0.0, None, None)
    );
}

#[test]
fn pbk_44_a_second_locked_playback_on_a_slot_loads_unlocked() {
    let mut v = show_json(&two_playbacks());
    for i in 0..2 {
        v["playbacks"]["playbacks"][i]["paging"] = json!("Locked");
        v["playbacks"]["playbacks"][i]["slot"] = json!(8);
        v["playbacks"]["playbacks"][i]["page"] = json!(i + 1);
    }
    let (e, warnings) = load(&v);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(e.playback(1).unwrap().paging, Paging::Locked);
    assert_eq!(e.playback(2).unwrap().paging, Paging::Unlocked);
}

#[test]
fn pbk_44_values_for_unknown_fixtures_and_palettes() {
    let mut v = show_json(&two_playbacks());
    v["playbacks"]["playbacks"][0]["cues"][0]["values"]["998"] = json!({"Dimmer": 1.0});
    v["playbacks"]["playbacks"][1]["cues"][0]["values"]["101"] =
        json!({"Pan": {"palette": 77, "value": 0.3}});
    let (e, warnings) = load(&v);
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(warnings.iter().any(|w| w.contains("998")), "{warnings:?}");
    assert!(!cue_values(&e, 1).contains_key(&998));
    assert_eq!(
        cue_values(&e, 2)[&101]["Pan"],
        Stored::Value(Value::Level(0.3))
    );
}

// ---- PBK-45: migration from schema 2 -------------------------------------------------------

#[test]
fn pbk_45_a_schema_2_show_keeps_its_playbacks_where_pbk_01_put_them() {
    let file = std::fs::read_to_string(V2_SHOW).unwrap();
    let (show, migrations) = Show::from_json(&file).unwrap();
    assert_eq!(migrations.len(), 1, "{migrations:?}");
    assert!(migrations[0].starts_with("2 -> 3"), "{migrations:?}");
    let (e, warnings) = Engine::from_show(&show);
    assert!(warnings.is_empty(), "{warnings:?}");
    let one = e.playback(1).unwrap();
    assert_eq!((one.page, one.slot), (1, 1));
    assert_eq!(one.cues.len(), 1);
    assert_eq!(
        one.cues[0].values[&101]["Pan"],
        Stored::Value(Value::Level(0.5))
    );
    let cue = &one.cues[0];
    assert_eq!(
        (cue.delay, cue.fade_in, cue.fade_out, cue.overlap),
        (0.0, 0.0, 0.0, 100.0)
    );
    let two = e.playback(2).unwrap();
    assert_eq!((two.page, two.slot, two.priority), (1, 2, Priority::High));
    assert_eq!(two.fader_mode, FaderMode::Mode2);
    assert_eq!(e.settings().cue_record_by, RecordBy::Channel);
    assert_eq!(e.settings().release_time, 2.0);
    assert_eq!(show.playbacks.next_id, 3);
}

#[test]
fn pbk_45_ids_map_to_pages_with_the_shows_faders_per_page() {
    let mut v: Json = serde_json::from_str(&std::fs::read_to_string(V2_SHOW).unwrap()).unwrap();
    let template = v["playbacks"][0].clone();
    let entry = |id: u32| {
        let mut p = template.clone();
        p["id"] = json!(id);
        p
    };
    v["settings"]["faders_per_page"] = json!(8);
    v["playbacks"] = json!([entry(9), entry(16), entry(81)]);
    let (show, _) = Show::from_json(&v.to_string()).unwrap();
    let (e, warnings) = Engine::from_show(&show);
    let at = |id| e.playback(id).map(|p| (p.page, p.slot));
    assert_eq!(at(9), Some((2, 1)));
    assert_eq!(at(16), Some((2, 8)));
    assert_eq!(at(81), None, "beyond page 10");
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("81"), "{warnings:?}");
}

// ---- Cue type --------------------------------------------------------------------------------

#[test]
fn cue_new_has_zero_times_and_full_overlap() {
    let cue = Cue::new(dimmers(&[101], 1.0));
    assert_eq!(
        (cue.delay, cue.fade_in, cue.fade_out, cue.overlap),
        (0.0, 0.0, 0.0, 100.0)
    );
    let pb = Playback::new(4, 2, 3, dimmers(&[101], 1.0));
    assert_eq!(pb.cues, vec![cue]);
    let _ = Settings::default();
}
