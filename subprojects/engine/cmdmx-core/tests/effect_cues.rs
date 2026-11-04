//! Effects in cues and palettes (FXB-03): `specs/modules/effects.md` FX-21..29 (and the
//! `{palette}` form of FX-14 and FX-16 that FXB-02 left out). Frame-level runs are
//! `tests/scenarios/FX-24-*.json` and `FX-26-*.json`.
//!
//! API under test, on top of `tests/effect_programmer.rs`:
//! - `Cue.effects: Vec<Entry>` and `Palette.effects: Vec<Entry>` (serde `default`, left out of
//!   the JSON when empty, so a show without effects saves as before, FX-29). A palette's entries
//!   have no members (FX-22).
//! - `Entry.palette: Option<PaletteId>`: an entry that points at an effect palette reads the
//!   palette's parameters every tick (FX-23); left out of the JSON when `None`.
//! - `Entry` is `Serialize`/`Deserialize`: `id`, `look` (left out when `None`), `shape`,
//!   `attributes`, `params`, `members`, `seed`, `colours` (left out when empty), `palette`.
//!   `positions`, `count` and the group are not saved: they are recomputed at load. On load
//!   `id`, `seed`, `look`, `colours`, `palette` default, and `params` fields missing take
//!   `Params::DEFAULT` (so a scenario can write `{"size": 1.0, "bpm": 60.0, "mode": "Absolute"}`).
//! - `EffectApply::Palette(u32)` (`{"palette": 3}`, a palette number): entries from the
//!   palette's stored entries on the targets (FX-14, 22), each pointing at the palette.
//!   `Engine::apply_palette` does the same next to the values (PAL-07).
//! - `AutoIcon::Effect { look: Option<String>, shape: Shape }`: the icon of a palette with
//!   entries and no values (FX-22), from its first entry.
//! - Recording (`record_playback`, `record_palette`) accepts a programmer that holds only
//!   effects; with neither values nor entries in the mask it is refused as before.
//! - Each cue entry runs on its own clock, started when its cue fires (FX-09, 24); there is no
//!   new accessor for it: the tests read the clock from a `RampUp` on a dimmer (absolute, size
//!   1, 60 BPM: the output is frac(θ) · 255).
//! - `Show` schema 8 (FX-29); `Engine::from_show` drops a malformed entry with a warning that
//!   names the effect (FX-28) and sets the entry id counter above every loaded id.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

mod common;

use cmdmx_core::attribute::AttributeGroup;
use cmdmx_core::effect::{ColourShape, EffectApply, EffectSet, Entry, EntryId, Path, Shape, Wave};
use cmdmx_core::engine::{Engine, FixtureUsage, PaletteUsage};
use cmdmx_core::palette::{
    AutoIcon, Cell, GlobalValue, Palette, Record, RecordMode, Timing, Window,
};
use cmdmx_core::patch::{Fixture, FixtureId};
use cmdmx_core::playback::{CueNumber, ListAction, PlaybackRecord, Priority, Track};
use cmdmx_core::profile::Profile;
use cmdmx_core::programmer::Item;
use cmdmx_core::show::{Meta, Show, ShowPalette, SCHEMA_VERSION};
use cmdmx_core::undo::History;
use common::{church, dmx, select};

const TICK: f64 = 0.025;

const RGB_PAR: &str = r#"{ "manufacturer": "T", "model": "RGB Par", "source": "channel-list", "modes": [ {
  "name": "4ch", "footprint": 4,
  "colour": { "system": "RGB", "components": [ { "channel": 2 }, { "channel": 3 }, { "channel": 4 } ] },
  "attributes": [ { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 } ] } ] }"#;

const SPOT: &str = r#"{ "manufacturer": "T", "model": "Spot", "source": "channel-list", "modes": [ {
  "name": "2ch", "footprint": 2, "pan_degrees": 540, "tilt_degrees": 270,
  "attributes": [
    { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 1 },
    { "name": "Tilt", "group": "P", "kind": "Ltp", "channel": 2 } ] } ] }"#;

fn patch(e: &mut Engine, id: FixtureId, profile: &str, address: u16) {
    e.patch(Fixture {
        id,
        number: 0,
        name: String::new(),
        handle: None,
        profile: profile.into(),
        mode: None,
        universe: 1,
        address,
    })
    .unwrap();
}

/// The church rig (washes 101–104 at 1, 6, 11, 16; one-channel pars 201–208 at 21–28) plus RGB
/// pars 251 @41, 252 @45 and a spot 311 @49 (Pan 49, Tilt 50; 540°/270°).
fn rig() -> Engine {
    let mut e = church();
    e.add_profile("rgb", Profile::from_json(RGB_PAR).unwrap())
        .unwrap();
    e.add_profile("spot", Profile::from_json(SPOT).unwrap())
        .unwrap();
    patch(&mut e, 251, "rgb", 41);
    patch(&mut e, 252, "rgb", 45);
    patch(&mut e, 311, "spot", 49);
    e
}

fn items(ids: &[FixtureId]) -> Vec<Item> {
    ids.iter().map(|&id| Item::fixture(id)).collect()
}

fn q(n: f64) -> CueNumber {
    CueNumber::new(n).unwrap()
}

fn look(e: &mut Engine, ids: &[FixtureId], name: &str) -> EntryId {
    select(e, ids);
    e.effect_apply(&EffectApply::Look(name.into())).unwrap()
}

fn shape(e: &mut Engine, ids: &[FixtureId], shape: Shape) -> EntryId {
    select(e, ids);
    let apply = EffectApply::Shape {
        shape,
        attribute: None,
    };
    e.effect_apply(&apply).unwrap()
}

/// A `RampUp` on the dimmers of `ids`: absolute (FX-05 default on Htp), size 1, 60 BPM, so the
/// output is frac(θ) and θ grows by 1 a second.
fn ramp(e: &mut Engine, ids: &[FixtureId]) -> EntryId {
    let id = shape(e, ids, Shape::Wave(Wave::RampUp));
    e.effect_set(EffectSet::Size(1.0)).unwrap();
    e.effect_set(EffectSet::Bpm(60.0)).unwrap();
    id
}

/// A `LinePan` on `ids`, relative, size 0.2, 60 BPM: Pan = base + 0.1 · sin 2πθ.
fn line_pan(e: &mut Engine, ids: &[FixtureId]) -> EntryId {
    let id = shape(e, ids, Shape::Path(Path::LinePan));
    e.effect_set(EffectSet::Size(0.2)).unwrap();
    e.effect_set(EffectSet::Bpm(60.0)).unwrap();
    id
}

/// Empty the programmer: values, selection and effects.
fn clear(e: &mut Engine) {
    e.clear();
    e.clear();
    assert!(e.effects().is_empty(), "the programmer still holds effects");
}

fn record(e: &mut Engine, page: u16, slot: u16) -> u32 {
    e.record_playback(&PlaybackRecord::at(page, slot)).unwrap()
}

/// Record cue `n` onto the playback on `page.slot` (a new cue number appends it).
fn record_cue(e: &mut Engine, page: u16, slot: u16, n: f64) -> u32 {
    let record = PlaybackRecord {
        cue: Some(q(n)),
        ..PlaybackRecord::at(page, slot)
    };
    e.record_playback(&record).unwrap()
}

/// Change the stored playback `id` through `f` (the same playback, put back).
fn edit_playback(e: &mut Engine, id: u32, f: impl FnOnce(&mut cmdmx_core::playback::Playback)) {
    let mut p = e.playback(id).unwrap().clone();
    f(&mut p);
    e.add_playback(p);
}

fn cue_effects(e: &Engine, id: u32, at: usize) -> Vec<Entry> {
    e.playback(id).unwrap().cues[at].effects.clone()
}

fn palette_number(e: &Engine, id: u32) -> u32 {
    e.palettes().find(|p| p.id == id).unwrap().number
}

/// Tick from `from` (exclusive) to `to` (inclusive) in TICK steps, ending exactly on `to`.
fn tick_to(e: &mut Engine, from: f64, to: f64) {
    let mut t = from;
    while t + TICK < to - 1e-9 {
        t += TICK;
        e.tick(t);
    }
    e.tick(to);
}

/// DMX at `address` within ±1 of `expected` (float rounding at x.5).
fn assert_dmx(e: &Engine, address: u16, expected: u8) {
    let got = dmx(e, address);
    assert!(
        got.abs_diff(expected) <= 1,
        "address {address}: expected {expected}, got {got}"
    );
}

fn meta() -> Meta {
    Meta {
        name: "FX".into(),
        created: "2026-10-29T09:00:00Z".into(),
        modified: "2026-10-29T09:00:00Z".into(),
        app_version: "0.1.0".into(),
    }
}

// ---------- FX-21: recording a cue ----------

#[test]
fn fx_21_record_stores_the_programmer_entries_with_their_members() {
    let mut e = rig();
    let id = look(&mut e, &[201, 202, 203, 204], "Dimmer chase");
    let programmer = e.effects().iter().find(|x| x.id == id).unwrap().clone();
    // A programmer that holds only an effect records.
    let pb = record(&mut e, 1, 1);
    let stored = cue_effects(&e, pb, 0);
    assert_eq!(stored.len(), 1);
    let s = &stored[0];
    assert_eq!(s.members, items(&[201, 202, 203, 204]));
    assert_eq!(s.shape, Shape::Wave(Wave::Square));
    assert_eq!(s.attributes, vec!["Dimmer".to_string()]);
    assert_eq!(s.params, programmer.params);
    assert_eq!(s.look.as_deref(), Some("Dimmer chase"));
    assert_eq!(s.seed, programmer.seed);
    assert_eq!(s.palette, None);
}

#[test]
fn fx_21_the_mask_filters_the_entries() {
    let mut e = rig();
    look(&mut e, &[201, 202, 203, 204], "Dimmer chase");
    look(&mut e, &[101, 102, 103, 104], "Circle");
    let record = PlaybackRecord {
        mask: Some(vec![AttributeGroup::P]),
        ..PlaybackRecord::at(1, 1)
    };
    let pb = e.record_playback(&record).unwrap();
    let stored = cue_effects(&e, pb, 0);
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].shape, Shape::Path(Path::Circle));
    assert_eq!(stored[0].members, items(&[101, 102, 103, 104]));
}

#[test]
fn fx_21_nothing_in_the_mask_is_refused() {
    let mut e = rig();
    look(&mut e, &[201, 202, 203, 204], "Dimmer chase");
    let record = PlaybackRecord {
        mask: Some(vec![AttributeGroup::C]),
        ..PlaybackRecord::at(1, 1)
    };
    assert!(e.record_playback(&record).is_err());
    assert_eq!(e.playbacks().count(), 0);
}

#[test]
fn fx_21_a_cues_groups_count_its_effects() {
    // FXB-09: a cue holding only a Circle lists P (CUE-40, PBK-35), not nothing.
    let mut e = rig();
    look(&mut e, &[101, 102, 103, 104], "Circle");
    let pb = record(&mut e, 1, 1);
    let number = e.playback(pb).unwrap().cues[0].number;
    assert_eq!(e.cue_groups(pb, number), vec![AttributeGroup::P]);
    assert_eq!(e.playback_groups(pb), vec![AttributeGroup::P]);
}

#[test]
fn fx_21_merge_replaces_the_effect_per_member_and_attribute() {
    let mut e = rig();
    look(&mut e, &[201, 202, 203, 204], "Dimmer chase");
    let pb = record(&mut e, 1, 1);
    clear(&mut e);
    look(&mut e, &[203, 204], "Dimmer wave");
    let merge = PlaybackRecord {
        mode: Some(RecordMode::Merge),
        ..PlaybackRecord::at(1, 1)
    };
    e.record_playback(&merge).unwrap();
    let stored = cue_effects(&e, pb, 0);
    assert_eq!(stored.len(), 2, "{stored:?}");
    let chase = stored
        .iter()
        .find(|x| x.look.as_deref() == Some("Dimmer chase"));
    let wave = stored
        .iter()
        .find(|x| x.look.as_deref() == Some("Dimmer wave"));
    assert_eq!(chase.unwrap().members, items(&[201, 202]));
    assert_eq!(wave.unwrap().members, items(&[203, 204]));

    // Replace takes the programmer's entries only.
    let replace = PlaybackRecord {
        mode: Some(RecordMode::Replace),
        ..PlaybackRecord::at(1, 1)
    };
    e.record_playback(&replace).unwrap();
    let stored = cue_effects(&e, pb, 0);
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].look.as_deref(), Some("Dimmer wave"));
}

// ---------- FX-22: effect palettes ----------

/// Positions palette 1 holding a circle (no values), recorded from washes 101–104; returns its
/// number and the programmer entry it was recorded from.
fn circle_palette(e: &mut Engine) -> (u32, Entry) {
    let id = look(e, &[101, 102, 103, 104], "Circle");
    e.effect_set(EffectSet::Bpm(30.0)).unwrap();
    // An intensity effect is not in a Positions palette's groups.
    look(e, &[201], "Breathe");
    let entry = e.effects().iter().find(|x| x.id == id).unwrap().clone();
    let pid = e
        .record_palette(&Record::at(Cell::new(Window::Positions, 1)))
        .unwrap();
    clear(e);
    (palette_number(e, pid), entry)
}

#[test]
fn fx_22_a_palette_stores_the_entries_without_members() {
    let mut e = rig();
    let (number, entry) = circle_palette(&mut e);
    let p = e.palette(number).unwrap();
    assert_eq!(p.effects.len(), 1, "{:?}", p.effects);
    let s = &p.effects[0];
    assert!(s.members.is_empty());
    assert_eq!(s.shape, Shape::Path(Path::Circle));
    assert_eq!(s.attributes, entry.attributes);
    assert_eq!(s.params, entry.params);
    assert_eq!(s.look.as_deref(), Some("Circle"));
}

#[test]
fn fx_22_applying_the_palette_makes_an_entry_on_the_targets() {
    let mut e = rig();
    let (number, entry) = circle_palette(&mut e);
    let pid = e.palette(number).unwrap().id;
    // 201 has no Pan/Tilt and is skipped; selection order is member order.
    select(&mut e, &[311, 201, 101]);
    e.apply_palette(number, Timing::default()).unwrap();
    let fx = e.effects();
    assert_eq!(fx.len(), 1);
    assert_eq!(fx[0].members, items(&[311, 101]));
    assert_eq!(fx[0].shape, Shape::Path(Path::Circle));
    assert_eq!(fx[0].palette, Some(pid));
    assert_eq!(fx[0].params, entry.params);
    assert_eq!(e.current_effect(), Some(fx[0].id));
}

#[test]
fn fx_22_effect_apply_palette() {
    let mut e = rig();
    let (number, _) = circle_palette(&mut e);
    let pid = e.palette(number).unwrap().id;
    // No target with Pan/Tilt: refused, nothing changes.
    select(&mut e, &[201, 202]);
    assert!(e.effect_apply(&EffectApply::Palette(number)).is_err());
    assert!(e.effects().is_empty());
    clear(&mut e);
    select(&mut e, &[102, 103]);
    let id = e.effect_apply(&EffectApply::Palette(number)).unwrap();
    assert_eq!(e.current_effect(), Some(id));
    assert_eq!(e.effects()[0].members, items(&[102, 103]));
    assert_eq!(e.effects()[0].palette, Some(pid));

    // A palette without entries is refused.
    clear(&mut e);
    select(&mut e, &[201]);
    e.set_programmer_value(201, "Dimmer", 0.5).unwrap();
    let plain = e
        .record_palette(&Record::at(Cell::new(Window::Beams, 1)))
        .unwrap();
    let plain = palette_number(&e, plain);
    clear(&mut e);
    select(&mut e, &[201]);
    assert!(e.effect_apply(&EffectApply::Palette(plain)).is_err());
    assert!(e.effects().is_empty());
}

#[test]
fn fx_22_the_command_line_applies_an_effect_palette() {
    let mut e = rig();
    let (number, _) = circle_palette(&mut e);
    let n = number.to_string();
    e.execute_command(&["311", "Palette", &n, "Enter"]).unwrap();
    assert_eq!(e.effects().len(), 1);
    assert_eq!(e.effects()[0].members, items(&[311]));
    // The palette counts as relevant for the selection.
    let pid = e.palette(number).unwrap().id;
    assert!(e.programmer_view().palettes.relevant.contains(&pid));
}

#[test]
fn fx_22_effect_apply_palette_serde() {
    let apply: EffectApply = serde_json::from_str(r#"{"palette": 3}"#).unwrap();
    assert_eq!(apply, EffectApply::Palette(3));
}

#[test]
fn fx_22_recording_a_pointer_into_a_palette_stores_its_parameters() {
    let mut e = rig();
    let (number, entry) = circle_palette(&mut e);
    select(&mut e, &[101]);
    e.apply_palette(number, Timing::default()).unwrap();
    let pid = e
        .record_palette(&Record::at(Cell::new(Window::Positions, 2)))
        .unwrap();
    let p = e.palettes().find(|p| p.id == pid).unwrap();
    assert_eq!(p.effects.len(), 1);
    assert_eq!(p.effects[0].palette, None);
    assert_eq!(p.effects[0].params, entry.params);
}

#[test]
fn fx_22_an_effect_only_palette_shows_the_look_icon() {
    let mut e = rig();
    let (number, _) = circle_palette(&mut e);
    assert_eq!(
        e.palette_auto_icon(number),
        Some(AutoIcon::Effect {
            look: Some("Circle".into()),
            shape: Shape::Path(Path::Circle),
        })
    );
}

// ---------- FX-23: palette pointers ----------

/// Beams palette 1 holding a ramp on the dimmer (60 BPM); a memory on 1.1 that points at it on
/// par 201. Returns (palette number, playback id).
fn ramp_palette_cue(e: &mut Engine) -> (u32, u32) {
    ramp(e, &[202]);
    let pid = e
        .record_palette(&Record::at(Cell::new(Window::Beams, 1)))
        .unwrap();
    let number = palette_number(e, pid);
    clear(e);
    select(e, &[201]);
    e.apply_palette(number, Timing::default()).unwrap();
    let pb = record(e, 1, 1);
    clear(e);
    (number, pb)
}

#[test]
fn fx_21_23_a_cue_keeps_the_pointer_and_follows_a_palette_edit() {
    let mut e = rig();
    let (number, pb) = ramp_palette_cue(&mut e);
    let pid = e.palette(number).unwrap().id;
    let stored = cue_effects(&e, pb, 0);
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].palette, Some(pid));
    assert_eq!(stored[0].members, items(&[201]));

    e.set_fader(pb, 1.0, 0.0).unwrap();
    e.tick(0.0);
    tick_to(&mut e, 0.0, 0.5);
    assert_dmx(&e, 21, 128);

    // Edit the palette: 120 BPM. The cue reads it from the next tick on.
    ramp(&mut e, &[202]);
    e.effect_set(EffectSet::Bpm(120.0)).unwrap();
    let replace = Record {
        mode: Some(RecordMode::Replace),
        ..Record::at(Cell::new(Window::Beams, 1))
    };
    e.record_palette(&replace).unwrap();
    clear(&mut e);
    tick_to(&mut e, 0.5, 0.625);
    // θ = 0.5 + 0.125 · 2.
    assert_dmx(&e, 21, 191);

    // A deleted palette leaves its last parameters in the entry.
    e.delete_palette(number).unwrap();
    let stored = cue_effects(&e, pb, 0);
    assert_eq!(stored[0].params.bpm, 120.0);
    tick_to(&mut e, 0.625, 0.7);
    // θ = 0.75 + 0.075 · 2.
    assert_dmx(&e, 21, 230);
}

#[test]
fn fx_16_setting_a_parameter_of_a_pointer_copies_the_palette_parameters() {
    let mut e = rig();
    let (number, entry) = circle_palette(&mut e);
    select(&mut e, &[101]);
    e.apply_palette(number, Timing::default()).unwrap();
    e.effect_set(EffectSet::Size(0.3)).unwrap();
    let fx = &e.effects()[0];
    assert_eq!(fx.palette, None);
    assert_eq!(fx.params.size, 0.3);
    assert_eq!(fx.params.bpm, entry.params.bpm);
}

// ---------- FX-24: playback ----------

/// A memory on 1.1 with a ramp on par 201 (DMX 21).
fn ramp_cue(e: &mut Engine) -> u32 {
    ramp(e, &[201]);
    let pb = record(e, 1, 1);
    clear(e);
    pb
}

#[test]
fn fx_24_a_cue_entry_starts_when_its_cue_fires() {
    let mut e = rig();
    let pb = ramp_cue(&mut e);
    e.tick(0.0);
    tick_to(&mut e, 0.0, 1.0);
    assert_dmx(&e, 21, 0);
    e.set_fader(pb, 1.0, 1.0).unwrap();
    e.tick(1.0);
    assert_dmx(&e, 21, 0);
    tick_to(&mut e, 1.0, 1.25);
    assert_dmx(&e, 21, 64);
    tick_to(&mut e, 1.25, 1.6);
    assert_dmx(&e, 21, 153);
}

#[test]
fn fx_24_the_cue_delay_holds_the_start() {
    let mut e = rig();
    let pb = ramp_cue(&mut e);
    edit_playback(&mut e, pb, |p| {
        p.cues[0].delay = 0.5;
        p.fader_mode = cmdmx_core::playback::FaderMode::Mode1;
    });
    e.tick(0.0);
    e.set_fader(pb, 1.0, 0.0).unwrap();
    e.tick(0.0);
    tick_to(&mut e, 0.0, 0.75);
    // Started at 0.5.
    assert_dmx(&e, 21, 64);
}

#[test]
fn fx_24_a_released_playback_stops_its_effect() {
    let mut e = rig();
    let pb = ramp_cue(&mut e);
    e.set_fader(pb, 1.0, 0.0).unwrap();
    e.tick(0.0);
    tick_to(&mut e, 0.0, 0.5);
    assert_dmx(&e, 21, 128);
    e.release_playback(pb, 0.5).unwrap();
    tick_to(&mut e, 0.5, 0.6);
    assert_dmx(&e, 21, 0);
}

#[test]
fn fx_24_the_programmer_effect_beats_the_cue_effect_and_the_cue_clock_runs_on() {
    let mut e = rig();
    let pb = ramp_cue(&mut e);
    e.set_fader(pb, 1.0, 0.0).unwrap();
    e.tick(0.0);
    tick_to(&mut e, 0.0, 1.0);
    // A programmer RampDown on 201: 1 − θ.
    shape(&mut e, &[201], Shape::Wave(Wave::RampDown));
    e.effect_set(EffectSet::Size(1.0)).unwrap();
    e.effect_set(EffectSet::Bpm(60.0)).unwrap();
    tick_to(&mut e, 1.0, 1.2);
    // θ_programmer = 0.175 (its first tick does not advance): 1 − 0.175.
    assert_dmx(&e, 21, 210);
    clear(&mut e);
    tick_to(&mut e, 1.2, 1.4);
    // The cue's clock ran on: θ = 1.4.
    assert_dmx(&e, 21, 102);
}

/// Playback 1.1 ramps 201 up (Normal), 1.2 ramps it down with `down_priority`; 1.2 fires at 0,
/// 1.1 at 0.5. Returns DMX 21 at 0.8.
fn two_playbacks(down_priority: Priority) -> u8 {
    let mut e = rig();
    let up = ramp_cue(&mut e);
    shape(&mut e, &[201], Shape::Wave(Wave::RampDown));
    e.effect_set(EffectSet::Size(1.0)).unwrap();
    e.effect_set(EffectSet::Bpm(60.0)).unwrap();
    let down = record(&mut e, 1, 2);
    clear(&mut e);
    edit_playback(&mut e, down, |p| p.priority = down_priority);
    e.set_fader(down, 1.0, 0.0).unwrap();
    e.tick(0.0);
    tick_to(&mut e, 0.0, 0.5);
    e.set_fader(up, 1.0, 0.5).unwrap();
    e.tick(0.5);
    tick_to(&mut e, 0.5, 0.8);
    dmx(&e, 21)
}

#[test]
fn fx_24_the_highest_priority_playback_effect_wins_then_the_latest() {
    // High beats the later Normal: 1 − 0.8.
    assert!(two_playbacks(Priority::High).abs_diff(51) <= 1);
    // At equal priority the latest activated wins: the ramp up, started at 0.5.
    assert!(two_playbacks(Priority::Normal).abs_diff(77) <= 1);
}

// ---------- FX-25: the playback's m ----------

#[test]
fn fx_25_an_absolute_htp_effect_scales_with_the_fader() {
    let mut e = rig();
    let pb = ramp_cue(&mut e);
    e.set_fader(pb, 0.5, 0.0).unwrap();
    e.tick(0.0);
    tick_to(&mut e, 0.0, 0.6);
    // 0.6 · 0.5.
    assert_dmx(&e, 21, 77);
}

#[test]
fn fx_25_the_fader_scales_a_relative_effect_size() {
    let mut e = rig();
    select(&mut e, &[311]);
    e.set_programmer_value(311, "Pan", 0.5).unwrap();
    line_pan(&mut e, &[311]);
    let pb = record(&mut e, 1, 1);
    clear(&mut e);
    e.set_fader(pb, 0.5, 0.0).unwrap();
    e.tick(0.0);
    tick_to(&mut e, 0.0, 0.25);
    // 0.5 + 0.5 · 0.2 · ½ sin(π/2).
    assert_dmx(&e, 49, 140);
    e.set_fader(pb, 1.0, 0.25).unwrap();
    tick_to(&mut e, 0.25, 1.25);
    assert_dmx(&e, 49, 153);
}

// ---------- FX-26: cue lists ----------

/// List 1.1 with Q1 and Q2 recorded from the programmer by `q1` and `q2`; Q2 fades in and out
/// over 1 s. Returns the playback id.
fn list(e: &mut Engine, q1: impl FnOnce(&mut Engine), q2: impl FnOnce(&mut Engine)) -> u32 {
    q1(e);
    let pb = record_cue(e, 1, 1, 1.0);
    q2(e);
    record_cue(e, 1, 1, 2.0);
    clear(e);
    edit_playback(e, pb, |p| {
        p.cues[1].fade_in = 1.0;
        p.cues[1].fade_out = Some(1.0);
    });
    assert!(e.playback(pb).unwrap().is_cue_list());
    pb
}

/// Fader up, Go on Q1 at 0, run to 2.25 and Go on Q2 (CUE-15: the list's m is its fader).
fn go_go(e: &mut Engine, pb: u32) {
    e.set_fader(pb, 1.0, 0.0).unwrap();
    e.cuelist(Some(pb), ListAction::Go, 0.0).unwrap();
    e.tick(0.0);
    tick_to(e, 0.0, 2.25);
    e.cuelist(Some(pb), ListAction::Go, 2.25).unwrap();
    e.tick(2.25);
}

#[test]
fn fx_26_the_same_shape_keeps_its_clock_and_fades_its_size() {
    let mut e = rig();
    let pb = list(
        &mut e,
        |e| {
            ramp(e, &[201]);
        },
        |e| e.effect_set(EffectSet::Size(0.5)).unwrap(),
    );
    assert_eq!(cue_effects(&e, pb, 1)[0].params.size, 0.5);
    go_go(&mut e, pb);
    tick_to(&mut e, 2.25, 2.75);
    // θ = 2.75 (the clock went on), size halfway 1 → 0.5: 1 − 0.75 · 0.25.
    assert_dmx(&e, 21, 207);
    tick_to(&mut e, 2.75, 3.5);
    // 1 − 0.5 · 0.5.
    assert_dmx(&e, 21, 191);
}

#[test]
fn fx_26_another_shape_crossfades_the_output() {
    let mut e = rig();
    let pb = list(
        &mut e,
        |e| {
            ramp(e, &[201]);
        },
        |e| {
            shape(e, &[201], Shape::Wave(Wave::RampDown));
            e.effect_set(EffectSet::Size(1.0)).unwrap();
            e.effect_set(EffectSet::Bpm(60.0)).unwrap();
        },
    );
    edit_playback(&mut e, pb, |p| p.list.tracking = false);
    go_go(&mut e, pb);
    tick_to(&mut e, 2.25, 2.75);
    // A: θ 2.75 → 0.75; B started at 2.25: 1 − 0.5; halfway: 0.625.
    assert_dmx(&e, 21, 159);
    tick_to(&mut e, 2.75, 3.5);
    // B alone: 1 − 0.25.
    assert_dmx(&e, 21, 191);
}

#[test]
fn fx_26_an_effect_the_next_cue_lacks_fades_to_the_base() {
    let mut e = rig();
    let pb = list(
        &mut e,
        |e| {
            select(e, &[311]);
            e.set_programmer_value(311, "Pan", 0.5).unwrap();
            line_pan(e, &[311]);
        },
        |e| {
            e.effect_stop(None).unwrap();
        },
    );
    assert!(cue_effects(&e, pb, 1).is_empty());
    edit_playback(&mut e, pb, |p| p.list.tracking = false);
    go_go(&mut e, pb);
    assert_dmx(&e, 49, 153);
    tick_to(&mut e, 2.25, 2.75);
    // θ 2.75: offset −0.1, half of it left.
    assert_dmx(&e, 49, 115);
    tick_to(&mut e, 2.75, 3.5);
    assert_dmx(&e, 49, 128);
}

#[test]
fn cue_56_an_effect_the_cue_lacks_waits_delay_out_and_falls_over_fade_out() {
    let mut e = rig();
    let pb = list(
        &mut e,
        |e| {
            select(e, &[311]);
            e.set_programmer_value(311, "Pan", 0.5).unwrap();
            line_pan(e, &[311]);
        },
        |e| {
            e.effect_stop(None).unwrap();
        },
    );
    edit_playback(&mut e, pb, |p| {
        p.list.tracking = false;
        p.cues[1].delay_out = Some(0.5);
    });
    go_go(&mut e, pb);
    tick_to(&mut e, 2.25, 2.75);
    // θ 2.75: offset −0.1, nothing of it gone yet: the delay out has just ended.
    assert_dmx(&e, 49, 102);
    tick_to(&mut e, 2.75, 3.25);
    // θ 3.25: offset +0.1, half of it left.
    assert_dmx(&e, 49, 140);
    tick_to(&mut e, 3.25, 4.0);
    assert_dmx(&e, 49, 128);
}

#[test]
fn cue_56_an_effect_fades_out_as_in_when_nothing_is_typed() {
    let mut e = rig();
    let pb = list(
        &mut e,
        |e| {
            select(e, &[311]);
            e.set_programmer_value(311, "Pan", 0.5).unwrap();
            line_pan(e, &[311]);
        },
        |e| {
            e.effect_stop(None).unwrap();
        },
    );
    edit_playback(&mut e, pb, |p| {
        p.list.tracking = false;
        p.cues[1].fade_out = None; // follows its fade in of 1 s
    });
    go_go(&mut e, pb);
    tick_to(&mut e, 2.25, 2.75);
    // θ 2.75: offset −0.1, half of it left, as with a typed 1 s.
    assert_dmx(&e, 49, 115);
}

// ---------- FX-27: tracking ----------

#[test]
fn fx_27_entries_track_and_a_block_cue_stops_them_only_where_it_holds_something() {
    let mut e = rig();
    ramp(&mut e, &[201]);
    let pb = record_cue(&mut e, 1, 1, 1.0);
    clear(&mut e);
    select(&mut e, &[202]);
    e.set_programmer_value(202, "Dimmer", 1.0).unwrap();
    record_cue(&mut e, 1, 1, 2.0);
    clear(&mut e);
    select(&mut e, &[203]);
    e.set_programmer_value(203, "Dimmer", 1.0).unwrap();
    record_cue(&mut e, 1, 1, 3.0);
    clear(&mut e);
    select(&mut e, &[201]);
    e.set_programmer_value(201, "Dimmer", 0.4).unwrap();
    record_cue(&mut e, 1, 1, 4.0);
    clear(&mut e);
    edit_playback(&mut e, pb, |p| {
        p.cues[2].track = Track::Block;
        p.cues[3].track = Track::Block;
    });

    e.set_fader(pb, 1.0, 0.0).unwrap();
    e.cuelist(Some(pb), ListAction::Go, 0.0).unwrap();
    e.tick(0.0);
    tick_to(&mut e, 0.0, 1.0);
    e.cuelist(Some(pb), ListAction::Go, 1.0).unwrap();
    tick_to(&mut e, 1.0, 1.25);
    // Q2 tracks Q1's ramp, clock and all.
    assert_dmx(&e, 21, 64);
    assert_dmx(&e, 22, 255);
    e.cuelist(Some(pb), ListAction::Go, 1.25).unwrap();
    tick_to(&mut e, 1.25, 1.5);
    // Block Q3 holds nothing on 201: the ramp runs on.
    assert_dmx(&e, 21, 128);
    assert_dmx(&e, 23, 255);
    e.cuelist(Some(pb), ListAction::Go, 1.5).unwrap();
    tick_to(&mut e, 1.5, 1.6);
    // Block Q4 holds a value on 201: the effect stops there.
    assert_dmx(&e, 21, 102);
}

// ---------- FX-28, FX-29: the show ----------

/// A rig with memory 1.1 holding a dimmer chase on 201–204, a circle on 101–104, a rainbow on
/// 251–252 and Dimmer 0.7 on 205; Positions palette 1 a circle (no values).
fn show_rig() -> (Engine, u32, u32) {
    let mut e = rig();
    let (number, _) = circle_palette(&mut e);
    look(&mut e, &[201, 202, 203, 204], "Dimmer chase");
    look(&mut e, &[101, 102, 103, 104], "Circle");
    look(&mut e, &[251, 252], "Rainbow");
    select(&mut e, &[205]);
    e.set_programmer_value(205, "Dimmer", 0.7).unwrap();
    let pb = record(&mut e, 1, 1);
    clear(&mut e);
    (e, pb, number)
}

#[test]
fn fx_28_a_malformed_entry_is_refused_at_load_and_the_rest_loads() {
    let (e, pb, _) = show_rig();
    let json = e.to_show(meta()).to_json();
    let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let cue = &mut value["playbacks"]["playbacks"][0]["cues"][0];
    let effects = cue["effects"].as_array_mut().unwrap();
    assert_eq!(effects.len(), 3);
    effects[0]["shape"] = serde_json::json!("Wobble");
    effects[1]["params"]["bpm"] = serde_json::json!(9999.0);
    let (show, migrations) = Show::from_json(&value.to_string()).unwrap();
    let (loaded, warnings) = Engine::from_show(&show);
    let all: Vec<&String> = migrations.iter().chain(warnings.iter()).collect();
    assert_eq!(
        all.iter()
            .filter(|w| w.to_lowercase().contains("effect"))
            .count(),
        2,
        "{all:?}"
    );
    let cue = &loaded.playback(pb).unwrap().cues[0];
    assert_eq!(cue.effects.len(), 1);
    assert_eq!(cue.effects[0].shape, Shape::Colour(ColourShape::Rainbow));
    assert!(cue.values.contains_key(&Item::fixture(205)));
}

#[test]
fn fx_29_schema_8_and_a_show_without_effects_has_no_effects_key() {
    assert_eq!(SCHEMA_VERSION, 11);
    let mut e = rig();
    select(&mut e, &[201]);
    e.set_programmer_value(201, "Dimmer", 0.7).unwrap();
    record(&mut e, 1, 1);
    e.record_palette(&Record::at(Cell::new(Window::Beams, 1)))
        .unwrap();
    let json = e.to_show(meta()).to_json();
    assert!(json.contains("\"schema_version\": 11"));
    assert!(!json.contains("\"effects\""), "{json}");
    assert!(!json.contains("\"palette\": null"));

    // The same show as schema 7 loads unchanged and saves byte-identical apart from the version.
    let seven = json.replace("\"schema_version\": 11", "\"schema_version\": 7");
    let (show, migrations) = Show::from_json(&seven).unwrap();
    assert!(
        migrations.iter().any(|m| m.starts_with("7 -> 8")),
        "{migrations:?}"
    );
    let (loaded, _) = Engine::from_show(&show);
    assert_eq!(loaded.to_show(show.meta.clone()).to_json(), json);
}

#[test]
fn fx_29_effects_round_trip_through_the_show() {
    let (mut e, pb, number) = show_rig();
    // A cue pointing at the palette, on 1.2.
    select(&mut e, &[102]);
    e.apply_palette(number, Timing::default()).unwrap();
    let pointer = record(&mut e, 1, 2);
    clear(&mut e);
    let json = e.to_show(meta()).to_json();
    let (show, _) = Show::from_json(&json).unwrap();
    let (mut loaded, warnings) = Engine::from_show(&show);
    assert!(warnings.is_empty(), "{warnings:?}");
    for id in [pb, pointer] {
        assert_eq!(
            loaded.playback(id).unwrap().cues,
            e.playback(id).unwrap().cues
        );
    }
    assert_eq!(loaded.palette(number), e.palette(number));
    assert_eq!(loaded.to_show(meta()).to_json(), json);

    // The output matches tick for tick.
    for engine in [&mut e, &mut loaded] {
        engine.set_fader(pb, 1.0, 0.0).unwrap();
        engine.set_fader(pointer, 1.0, 0.0).unwrap();
    }
    let mut t = 0.0;
    while t < 1.0 {
        e.tick(t);
        loaded.tick(t);
        assert_eq!(e.frame(1), loaded.frame(1), "at {t}");
        t += TICK;
    }

    // A new programmer entry gets an id above every loaded one.
    let all: Vec<EntryId> = (loaded.playbacks())
        .flat_map(|p| p.cues.iter().flat_map(|c| c.effects.iter().map(|x| x.id)))
        .chain(
            loaded
                .palettes()
                .flat_map(|p| p.effects.iter().map(|x| x.id)),
        )
        .collect();
    let new = look(&mut loaded, &[206], "Breathe");
    assert!(all.iter().all(|&id| id < new), "{new} vs {all:?}");
}

// ---------- PAT-26: removal reaches the effects ----------

/// A `RampUp` on the dimmers of `ids` spread over 360°, recorded on 1.1; the programmer empty.
fn spread_cue(e: &mut Engine, ids: &[FixtureId]) -> u32 {
    ramp(e, ids);
    e.effect_set(EffectSet::Spread(360.0)).unwrap();
    let pb = record(e, 1, 1);
    clear(e);
    pb
}

fn start(e: &mut Engine, pb: u32) {
    e.tick(0.0);
    e.set_fader(pb, 1.0, 0.0).unwrap();
    e.tick(0.0);
}

#[test]
fn pat_26_removal_takes_the_member_out_of_every_cue_and_works_out_positions() {
    let mut e = rig();
    let pb = spread_cue(&mut e, &[201, 202, 203]);
    e.patch_remove(&[202]).unwrap();
    let entries = cue_effects(&e, pb, 0);
    assert_eq!(entries[0].members, items(&[201, 203]));
    assert_eq!(entries[0].positions, [0, 1], "FX-07: worked out again");
    assert_eq!(entries[0].count, 2);
}

#[test]
fn pat_26_a_cell_member_goes_with_its_fixture() {
    let mut e = rig();
    let pb = spread_cue(&mut e, &[251, 252]);
    edit_playback(&mut e, pb, |p| {
        p.cues[0].effects[0].members.push(Item::cell(251, 1));
    });
    e.patch_remove(&[251]).unwrap();
    assert_eq!(cue_effects(&e, pb, 0)[0].members, items(&[252]));
}

#[test]
fn pat_26_an_entry_left_without_members_goes() {
    let mut e = rig();
    ramp(&mut e, &[201]);
    line_pan(&mut e, &[311]);
    let pb = record(&mut e, 1, 1);
    clear(&mut e);
    assert_eq!(cue_effects(&e, pb, 0).len(), 2);
    e.patch_remove(&[311]).unwrap();
    let entries = cue_effects(&e, pb, 0);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].members, items(&[201]));
}

#[test]
fn pat_26_a_running_cue_gives_the_frames_of_the_show_reopened() {
    let mut e = rig();
    let pb = spread_cue(&mut e, &[201, 202, 203]);
    start(&mut e, pb);
    tick_to(&mut e, 0.0, 1.1);
    e.patch_remove(&[202]).unwrap();
    let (mut reopened, warnings) = Engine::from_show(&e.to_show(meta()));
    assert!(warnings.is_empty(), "{warnings:?}");
    start(&mut reopened, pb);
    tick_to(&mut reopened, 0.0, 1.1);
    let mut from = 1.1;
    for to in [1.2, 1.35, 1.6, 1.95] {
        tick_to(&mut e, from, to);
        tick_to(&mut reopened, from, to);
        for address in [21, 22, 23] {
            assert_eq!(
                dmx(&e, address),
                dmx(&reopened, address),
                "{address} at {to}"
            );
        }
        from = to;
    }
    assert_eq!(dmx(&e, 22), 0, "the removed fixture's channel is dark");
}

/// A show file whose first palette's entry is copied over: one on 101 and 102, one on a fixture
/// the file never patched, one without members as FX-22 records them.
fn palette_with_members(e: &mut Engine) -> (Engine, u32) {
    let (number, _) = circle_palette(e);
    let mut show = e.to_show(meta());
    let ShowPalette::Palette(p) = &mut show.palettes.palettes[0] else {
        panic!("a readable palette");
    };
    let stored = p.effects[0].clone();
    let with = |members: Vec<Item>| {
        let mut entry = stored.clone();
        entry.members = members;
        entry
    };
    p.effects = vec![
        with(items(&[101, 102])),
        with(items(&[999])),
        stored.clone(),
    ];
    let (loaded, _) = Engine::from_show(&show);
    (loaded, number)
}

#[test]
fn pat_26_the_load_drops_palette_effect_members() {
    let mut e = rig();
    let (loaded, number) = palette_with_members(&mut e);
    let effects = &loaded.palette(number).unwrap().effects;
    assert_eq!(effects.len(), 2, "the entry on 999 goes: {effects:?}");
    assert_eq!(effects[0].members, items(&[101, 102]));
    assert!(effects[1].members.is_empty(), "FX-22 entry stays");
}

#[test]
fn pat_26_removal_reaches_palette_effects_and_spares_an_entry_that_never_had_members() {
    let mut e = rig();
    let (mut loaded, number) = palette_with_members(&mut e);
    loaded.patch_remove(&[101]).unwrap();
    let effects = &loaded.palette(number).unwrap().effects;
    assert_eq!(effects[0].members, items(&[102]));
    assert_eq!(effects[0].positions, [0]);
    assert_eq!(effects[0].count, 1);
    loaded.patch_remove(&[102]).unwrap();
    let effects = &loaded.palette(number).unwrap().effects;
    assert_eq!(effects.len(), 1, "the emptied entry goes");
    assert!(effects[0].members.is_empty(), "FX-22 entry stays");
}

#[test]
fn pat_26_removal_leaves_an_ordinary_effect_palette_alone() {
    let mut e = rig();
    let (number, _) = circle_palette(&mut e);
    let before = e.palette(number).unwrap().effects.clone();
    e.patch_remove(&[101]).unwrap();
    assert_eq!(e.palette(number).unwrap().effects, before);
}

#[test]
fn pat_26_a_gone_first_member_is_not_what_fx_42_reads() {
    let mut e = rig();
    e.set_programmer_value(251, "Dimmer", 1.0).unwrap();
    e.set_programmer_value(252, "Dimmer", 0.0).unwrap();
    shape(&mut e, &[251, 252], Shape::Colour(ColourShape::Rainbow));
    assert!(!e.programmer_view().effects[0].dark);
    e.patch_remove(&[251]).unwrap();
    let view = e.programmer_view();
    assert_eq!(view.effects[0].members, items(&[252]));
    assert!(view.effects[0].dark, "252 is the first member now");
}

/// A control that was never touched and `e`, which takes `act` at 1.1: the same frames afterwards.
fn assert_same_frames_as_untouched(ids: &[FixtureId], act: impl FnOnce(&mut Engine, &mut History)) {
    let mut e = rig();
    let mut control = rig();
    let pb = spread_cue(&mut e, ids);
    spread_cue(&mut control, ids);
    start(&mut e, pb);
    start(&mut control, pb);
    tick_to(&mut e, 0.0, 1.1);
    tick_to(&mut control, 0.0, 1.1);
    let mut h = History::default();
    act(&mut e, &mut h);
    let mut from = 1.1;
    for to in [1.2, 1.35, 1.6] {
        tick_to(&mut e, from, to);
        tick_to(&mut control, from, to);
        for address in [21, 22, 23] {
            assert_eq!(
                dmx(&e, address),
                dmx(&control, address),
                "{address} at {to}"
            );
        }
        from = to;
    }
    assert_eq!(cue_effects(&e, pb, 0), cue_effects(&control, pb, 0));
}

#[test]
fn pat_26_undo_brings_the_member_back_in_a_running_cue() {
    assert_same_frames_as_untouched(&[201, 202, 203], |e, h| {
        h.edit(e, "Remove", None, |e| e.patch_remove(&[202]))
            .unwrap();
        h.undo(e).unwrap();
    });
}

#[test]
fn pat_26_undo_after_ticks_brings_a_whole_entry_back_on_its_clock() {
    assert_same_frames_as_untouched(&[201], |e, h| {
        h.edit(e, "Remove", None, |e| e.patch_remove(&[201]))
            .unwrap();
        e.tick(1.1);
        assert_eq!(dmx(e, 21), 0);
        h.undo(e).unwrap();
    });
}

// ---------- PAT-27, PAT-28: a mode change reaches the effects, and warns about palettes ----------

/// "Big" (Dimmer, Pan, Tilt, Zoom), "Panonly" (Dimmer, Pan), "Small" (Dimmer, Zoom).
const MOVER: &str = r#"{ "manufacturer": "T", "model": "Mover", "source": "channel-list", "modes": [
  { "name": "Big", "footprint": 4, "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 2 },
    { "name": "Tilt", "group": "P", "kind": "Ltp", "channel": 3 },
    { "name": "Zoom", "group": "B", "kind": "Ltp", "channel": 4 } ] },
  { "name": "Panonly", "footprint": 2, "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "Pan", "group": "P", "kind": "Ltp", "channel": 2 } ] },
  { "name": "Small", "footprint": 2, "attributes": [
    { "name": "Dimmer", "group": "I", "kind": "Htp", "channel": 1 },
    { "name": "Zoom", "group": "B", "kind": "Ltp", "channel": 2 } ] } ] }"#;

/// The rig plus movers 401 @61, 402 @65, 403 @69 in `mode` (Pan at 62, 66, 70).
fn mover_rig(mode: &str) -> Engine {
    let mut e = rig();
    e.add_profile("mover", Profile::from_json(MOVER).unwrap())
        .unwrap();
    for (id, address) in [(401, 61), (402, 65), (403, 69)] {
        e.patch(Fixture {
            id,
            number: 0,
            name: String::new(),
            handle: None,
            profile: "mover".into(),
            mode: Some(mode.into()),
            universe: 1,
            address,
        })
        .unwrap();
    }
    e
}

/// A `LinePan` on all three movers spread over 360°.
fn pan_entry(e: &mut Engine) {
    line_pan(e, &[401, 402, 403]);
    e.effect_set(EffectSet::Spread(360.0)).unwrap();
}

fn messages(e: &mut Engine) -> Vec<String> {
    e.drain_warnings().into_iter().map(|w| w.message).collect()
}

#[test]
fn pat_27_a_mode_without_the_attribute_takes_the_fixture_out_of_cue_and_programmer() {
    let mut e = mover_rig("Big");
    pan_entry(&mut e);
    let pb = record(&mut e, 1, 1);
    e.drain_warnings();
    e.patch_change_mode(402, "Small").unwrap();
    let cue = cue_effects(&e, pb, 0);
    assert_eq!(cue[0].members, items(&[401, 403]));
    assert_eq!(cue[0].positions, [0, 1], "FX-07: worked out again");
    assert_eq!(cue[0].count, 2);
    assert_eq!(e.effects()[0].members, items(&[401, 403]));
    assert_eq!(e.effects()[0].positions, [0, 1]);
    let warnings = messages(&mut e);
    assert_eq!(
        warnings.len(),
        4,
        "a path drives Pan and Tilt: {warnings:?}"
    );
    for a in ["Pan", "Tilt"] {
        let says = |source: String| {
            format!(
                "fixture 402: {source} effect '{a}' dropped, mode 'Small' has no such attribute"
            )
        };
        assert!(
            warnings.contains(&says("programmer".into())),
            "{warnings:?}"
        );
        assert!(
            warnings.contains(&says(format!("playback {pb}"))),
            "{warnings:?}"
        );
    }
}

#[test]
fn pat_27_an_entry_left_without_members_goes() {
    let mut e = mover_rig("Big");
    let id = line_pan(&mut e, &[402]);
    let pb = record(&mut e, 1, 1);
    assert!(e.effects().iter().any(|x| x.id == id));
    e.patch_change_mode(402, "Small").unwrap();
    assert!(cue_effects(&e, pb, 0).is_empty());
    assert!(e.effects().is_empty());
    assert_eq!(e.programmer_view().effects.len(), 0);
}

#[test]
fn pat_27_a_member_stays_while_the_mode_has_one_of_the_attributes() {
    let mut e = mover_rig("Big");
    look(&mut e, &[401, 402], "Circle");
    let pb = record(&mut e, 1, 1);
    e.drain_warnings();
    e.patch_change_mode(402, "Panonly").unwrap();
    assert_eq!(cue_effects(&e, pb, 0)[0].members, items(&[401, 402]));
    assert_eq!(e.effects()[0].members, items(&[401, 402]));
    assert_eq!(messages(&mut e), Vec::<String>::new());
}

#[test]
fn pat_27_a_cell_member_stays_in_every_mode() {
    let mut e = mover_rig("Big");
    pan_entry(&mut e);
    let pb = record(&mut e, 1, 1);
    edit_playback(&mut e, pb, |p| {
        p.cues[0].effects[0].members.push(Item::cell(402, 1));
    });
    e.patch_change_mode(402, "Small").unwrap();
    let mut expected = items(&[401, 403]);
    expected.push(Item::cell(402, 1));
    assert_eq!(cue_effects(&e, pb, 0)[0].members, expected);
}

#[test]
fn pat_27_a_range_counts_effects_in_the_one_line() {
    let mut e = mover_rig("Big");
    pan_entry(&mut e);
    record(&mut e, 1, 1);
    e.drain_warnings();
    e.patch_change_mode_range(&[401, 402], "Small").unwrap();
    assert_eq!(messages(&mut e), ["Dropped Pan, Tilt on 2 fixtures"]);
    assert_eq!(e.effects()[0].members, items(&[403]));
}

#[test]
fn pat_27_a_running_cue_gives_the_frames_of_the_show_reopened() {
    let mut e = mover_rig("Big");
    pan_entry(&mut e);
    let pb = record(&mut e, 1, 1);
    clear(&mut e);
    start(&mut e, pb);
    tick_to(&mut e, 0.0, 1.1);
    e.patch_change_mode(402, "Small").unwrap();
    let (mut reopened, warnings) = Engine::from_show(&e.to_show(meta()));
    assert!(warnings.is_empty(), "{warnings:?}");
    start(&mut reopened, pb);
    tick_to(&mut reopened, 0.0, 1.1);
    let mut from = 1.1;
    for to in [1.2, 1.35, 1.6, 1.95] {
        tick_to(&mut e, from, to);
        tick_to(&mut reopened, from, to);
        for address in [62, 70] {
            assert_eq!(
                dmx(&e, address),
                dmx(&reopened, address),
                "{address} at {to}"
            );
        }
        from = to;
    }
}

#[test]
fn pat_27_undo_brings_it_back_also_in_a_running_cue() {
    let setup = || {
        let mut e = mover_rig("Big");
        pan_entry(&mut e);
        let pb = record(&mut e, 1, 1);
        start(&mut e, pb);
        tick_to(&mut e, 0.0, 1.1);
        (e, pb)
    };
    let (mut e, pb) = setup();
    let (mut control, _) = setup();
    let mut h = History::default();
    h.edit(&mut e, "Mode", None, |e| e.patch_change_mode(402, "Small"))
        .unwrap();
    e.tick(1.1);
    h.undo(&mut e).unwrap();
    assert_eq!(e.fixture(402).unwrap().mode.as_deref(), Some("Big"));
    assert_eq!(e.effects(), control.effects());
    assert_eq!(cue_effects(&e, pb, 0), cue_effects(&control, pb, 0));
    let mut from = 1.1;
    for to in [1.2, 1.35, 1.6] {
        tick_to(&mut e, from, to);
        tick_to(&mut control, from, to);
        for address in [62, 66, 70] {
            assert_eq!(
                dmx(&e, address),
                dmx(&control, address),
                "{address} at {to}"
            );
        }
        from = to;
    }
}

/// Palette `number` with `legend`, filled by `fill`.
fn add_palette(e: &mut Engine, number: u32, legend: &str, fill: impl FnOnce(&mut Palette)) {
    let mut p = Palette {
        id: 100 + number,
        number,
        window: Window::Positions,
        handle: u16::try_from(number).unwrap(),
        legend: legend.into(),
        icon: None,
        global: Default::default(),
        shared: Default::default(),
        shared_cells: Default::default(),
        fixtures: Default::default(),
        effects: Vec::new(),
    };
    fill(&mut p);
    e.add_palette(p).unwrap();
}

fn holds(p: &mut Palette, item: Item, attribute: &str) {
    p.fixtures
        .entry(item)
        .or_default()
        .insert(attribute.into(), 0.5.into());
}

#[test]
fn pat_28_a_per_fixture_value_warns_once_per_palette_and_stays() {
    let mut e = mover_rig("Big");
    add_palette(&mut e, 3, "Fast strobe", |p| {
        holds(p, Item::fixture(401), "Pan");
        holds(p, Item::fixture(402), "Pan");
    });
    let before = e.palette(3).unwrap().clone();
    e.patch_change_mode_range(&[401, 402, 403], "Small")
        .unwrap();
    assert_eq!(
        messages(&mut e),
        ["Palette 3 Fast strobe: no Pan on 2 fixtures in this mode, the value stays"]
    );
    assert_eq!(
        e.palette(3).unwrap(),
        &before,
        "PAL-19: the old mode can come back"
    );
}

#[test]
fn pat_28_one_fixture_and_an_empty_legend() {
    let mut e = mover_rig("Big");
    add_palette(&mut e, 4, "", |p| holds(p, Item::fixture(401), "Pan"));
    e.patch_change_mode(401, "Small").unwrap();
    assert_eq!(
        messages(&mut e),
        ["Palette 4: no Pan on 1 fixture in this mode, the value stays"]
    );
}

#[test]
fn pat_28_a_value_by_type_counts_the_fixtures_of_that_type() {
    let mut e = mover_rig("Big");
    add_palette(&mut e, 3, "Sweep", |p| {
        p.shared.insert(
            "mover".into(),
            [
                ("Pan".to_string(), 0.5.into()),
                ("Tilt".to_string(), 0.5.into()),
            ]
            .into(),
        );
        p.shared
            .insert("spot".into(), [("Pan".to_string(), 0.5.into())].into());
    });
    e.patch_change_mode_range(&[401, 402, 403], "Small")
        .unwrap();
    assert_eq!(
        messages(&mut e),
        ["Palette 3 Sweep: no Pan, Tilt on 3 fixtures in this mode, the value stays"]
    );
}

#[test]
fn pat_28_an_effect_for_the_fixture_warns_and_one_without_members_does_not() {
    let mut e = mover_rig("Big");
    let id = line_pan(&mut e, &[401]);
    let entry = e.effects().iter().find(|x| x.id == id).unwrap().clone();
    let mut memberless = entry.clone();
    memberless.members.clear();
    clear(&mut e);
    add_palette(&mut e, 3, "With", |p| p.effects = vec![entry]);
    add_palette(&mut e, 4, "Without", |p| p.effects = vec![memberless]);
    e.patch_change_mode(401, "Small").unwrap();
    assert_eq!(
        messages(&mut e),
        ["Palette 3 With: no Pan, Tilt on 1 fixture in this mode, the value stays"]
    );
}

#[test]
fn pat_28_a_global_value_says_nothing() {
    let mut e = mover_rig("Big");
    add_palette(&mut e, 3, "Centre", |p| {
        p.global
            .insert("Pan".into(), GlobalValue::Value(0.5.into()));
    });
    e.patch_change_mode(401, "Small").unwrap();
    assert_eq!(messages(&mut e), Vec::<String>::new());
}

#[test]
fn pat_28_a_change_that_loses_nothing_says_nothing() {
    let mut e = mover_rig("Panonly");
    add_palette(&mut e, 3, "Still there", |p| {
        holds(p, Item::fixture(401), "Dimmer");
    });
    add_palette(&mut e, 4, "Never there", |p| {
        holds(p, Item::fixture(401), "Tilt");
    });
    add_palette(&mut e, 5, "Its cell", |p| {
        holds(p, Item::cell(401, 1), "Pan");
    });
    e.patch_change_mode(401, "Small").unwrap();
    assert_eq!(messages(&mut e), Vec::<String>::new());
}

#[test]
fn pat_28_each_palette_has_its_line_next_to_the_dropped_values() {
    let mut e = mover_rig("Big");
    add_palette(&mut e, 4, "Second", |p| {
        holds(p, Item::fixture(402), "Tilt")
    });
    add_palette(&mut e, 3, "First", |p| holds(p, Item::fixture(401), "Pan"));
    e.set_programmer_value(401, "Pan", 0.5).unwrap();
    e.patch_change_mode_range(&[401, 402], "Small").unwrap();
    assert_eq!(
        messages(&mut e),
        [
            "Palette 3 First: no Pan on 1 fixture in this mode, the value stays",
            "Palette 4 Second: no Tilt on 1 fixture in this mode, the value stays",
            "Dropped Pan on 1 fixtures",
        ]
    );
}

// ---- IPC-54 what a delete touches ---------------------------------------------------------

#[test]
fn ipc_54_an_effect_entry_counts_for_its_palette_and_its_members() {
    let mut e = rig();
    let (number, _) = ramp_palette_cue(&mut e);
    // The cue holds only the entry, on 201: pointing at the palette, and with 201 as a member.
    assert_eq!(
        e.palette_usage(number).unwrap(),
        PaletteUsage {
            cues: 1,
            playbacks: 1
        }
    );
    assert_eq!(
        e.fixture_usage(&[201]),
        FixtureUsage {
            cues: 1,
            playbacks: 1,
            empty_cues: 1,
            ..FixtureUsage::default()
        }
    );
    assert_eq!(e.fixture_usage(&[202]), FixtureUsage::default());
}

// ---- CUE-59, 60: effects in the Cue view, their Remove, a whole fixture's Remove ------------------

/// Q1 a ramp on 201 and 202, Q2 a ramp on 202 of its own, Q3 a value on 203 and nothing else.
/// API: `Engine::cue_effects(id, cue) -> Result<Vec<CueEffect>, CoreError>` with `CueEffect { entry:
/// Entry, from: CueNumber }` (in `playback`): the cue's state of FX-27, `entry.members` only the
/// members it still runs on there. `Engine::remove_cue_effect(id, cue, entry)` takes a hard entry out.
fn effect_list() -> (Engine, u32, EntryId, EntryId) {
    let mut e = rig();
    let first = ramp(&mut e, &[201, 202]);
    let pb = record_cue(&mut e, 1, 1, 1.0);
    clear(&mut e);
    let second = ramp(&mut e, &[202]);
    record_cue(&mut e, 1, 1, 2.0);
    clear(&mut e);
    select(&mut e, &[203]);
    e.set_programmer_value(203, "Dimmer", 1.0).unwrap();
    record_cue(&mut e, 1, 1, 3.0);
    clear(&mut e);
    (e, pb, first, second)
}

/// The view of cue `n` as (entry id, members, from).
fn viewed_effects(e: &Engine, pb: u32, n: f64) -> Vec<(EntryId, Vec<Item>, f64)> {
    let view = e.cue_effects(pb, q(n)).unwrap();
    view.into_iter()
        .map(|v| (v.entry.id, v.entry.members, v.from.as_f64()))
        .collect()
}

#[test]
fn cue_59_the_view_lists_the_entries_with_the_cue_they_are_hard_in() {
    let (e, pb, first, second) = effect_list();
    assert_eq!(
        viewed_effects(&e, pb, 1.0),
        vec![(first, items(&[201, 202]), 1.0)]
    );
    // Tracked for only 201 from Q2 on: listed with that member.
    let q2 = vec![(first, items(&[201]), 1.0), (second, items(&[202]), 2.0)];
    assert_eq!(viewed_effects(&e, pb, 2.0), q2);
    let q3 = vec![(first, items(&[201]), 1.0), (second, items(&[202]), 2.0)];
    assert_eq!(viewed_effects(&e, pb, 3.0), q3);
    assert!(e.cue_effects(pb, q(9.0)).is_err());
    assert!(e.cue_effects(99, q(1.0)).is_err());
}

#[test]
fn cue_59_remove_takes_a_hard_entry_out_and_a_tracked_one_is_refused() {
    let (mut e, pb, first, second) = effect_list();
    let before = e.playback(pb).unwrap().clone();
    let refused = e.remove_cue_effect(pb, q(2.0), first).unwrap_err();
    assert!(
        refused
            .to_string()
            .contains(&format!("Q2 holds no effect {first} of its own")),
        "{refused}"
    );
    assert!(e.remove_cue_effect(pb, q(9.0), second).is_err());
    assert!(e.remove_cue_effect(99, q(2.0), second).is_err());
    assert_eq!(e.playback(pb).unwrap(), &before);

    e.remove_cue_effect(pb, q(2.0), second).unwrap();
    assert!(cue_effects(&e, pb, 1).is_empty());
    assert_eq!(cue_effects(&e, pb, 0).len(), 1, "Q1 keeps its own");
    assert_eq!(
        viewed_effects(&e, pb, 2.0),
        vec![(first, items(&[201, 202]), 1.0)],
        "Q1's entry tracks on 202 again"
    );
}

#[test]
fn cue_59_remove_on_the_running_cue_is_on_stage_on_the_next_tick() {
    let (mut e, pb, first, _) = effect_list();
    e.set_fader(pb, 1.0, 0.0).unwrap();
    e.cuelist(Some(pb), ListAction::Go, 0.0).unwrap();
    e.tick(0.0);
    tick_to(&mut e, 0.0, 0.5);
    assert_dmx(&e, 21, 128);
    e.remove_cue_effect(pb, q(1.0), first).unwrap();
    e.tick(0.525);
    assert_eq!(dmx(&e, 21), 0);
    assert_eq!(dmx(&e, 22), 0);
}

#[test]
fn cue_60_remove_of_an_item_takes_it_out_of_the_cues_own_entries() {
    let (mut e, pb, first, _) = effect_list();
    e.remove_cue_value(pb, q(1.0), Item::fixture(201), None)
        .unwrap();
    assert_eq!(cue_effects(&e, pb, 0)[0].members, items(&[202]));
    assert_eq!(cue_effects(&e, pb, 0)[0].id, first);
    // The last member takes the entry with it, and nothing is left to refuse on.
    e.remove_cue_value(pb, q(1.0), Item::fixture(202), None)
        .unwrap();
    assert!(cue_effects(&e, pb, 0).is_empty());
    assert!(e
        .remove_cue_value(pb, q(1.0), Item::fixture(202), None)
        .is_err());
}

#[test]
fn cue_60_values_and_members_go_together_and_tracked_ones_stay() {
    let (mut e, pb, first, _) = effect_list();
    // 202 in Q2: an entry of its own. 201 in Q2: only tracked, so nothing to take out.
    let before = e.playback(pb).unwrap().clone();
    assert!(e
        .remove_cue_value(pb, q(2.0), Item::fixture(201), None)
        .is_err());
    // With an attribute it is a value's Remove (CUE-54): the entry is not touched.
    assert!(e
        .remove_cue_value(pb, q(2.0), Item::fixture(202), Some("Dimmer"))
        .is_err());
    assert_eq!(e.playback(pb).unwrap(), &before);

    edit_playback(&mut e, pb, |p| {
        p.cues[1].values.insert(
            Item::fixture(202),
            std::collections::BTreeMap::from([(
                "Dimmer".to_string(),
                cmdmx_core::value::Stored::Value(cmdmx_core::value::Value::Level(0.5)),
            )]),
        );
    });
    e.remove_cue_value(pb, q(2.0), Item::fixture(202), None)
        .unwrap();
    let q2 = &e.playback(pb).unwrap().cues[1];
    assert!(q2.values.is_empty());
    assert!(q2.effects.is_empty());
    assert_eq!(
        viewed_effects(&e, pb, 2.0),
        vec![(first, items(&[201, 202]), 1.0)],
        "what tracks from Q1 stays visible"
    );
}

#[test]
fn cue_60_remove_on_the_running_cue_is_on_stage_on_the_next_tick() {
    let (mut e, pb, _, _) = effect_list();
    e.set_fader(pb, 1.0, 0.0).unwrap();
    e.cuelist(Some(pb), ListAction::Go, 0.0).unwrap();
    e.tick(0.0);
    tick_to(&mut e, 0.0, 0.5);
    assert_dmx(&e, 21, 128);
    assert_dmx(&e, 22, 128);
    e.remove_cue_value(pb, q(1.0), Item::fixture(201), None)
        .unwrap();
    e.tick(0.525);
    assert_eq!(dmx(&e, 21), 0);
    assert_dmx(&e, 22, 134);
}
