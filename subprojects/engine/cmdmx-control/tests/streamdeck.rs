//! Stream Deck: model table, HID report codec, deck pages, default mapping, key looks and the
//! key image renderer (specs/modules/control-surfaces.md CTL-45, 47..51; ADR-009).
//!
//! Packet layouts follow the open reference python-elgato-streamdeck. Golden images live in
//! `tests/golden/`; `CMDMX_BLESS=1 cargo test` rewrites them (look at them before committing).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use cmdmx_control::action::Action;
use cmdmx_control::convert::Reading;
use cmdmx_control::deck::{
    brightness_report, deck_pages, default_mapping, image_reports, model, models, on_page,
    turn_page, DeckInput, DeckKind, DeckReader, Family, ImageFormat, Model, ELGATO_VENDOR,
};
use cmdmx_control::deck_image::{
    encode, key_look, legend_lines, render, render_rgb, KeyLook, KeyState, PlaybackView, Subject,
};
use cmdmx_control::mapping::{Binding, Input, Mapping};

const ORIGINAL: u16 = 0x0060;
const MK2: u16 = 0x0080;
const MINI: u16 = 0x0063;
const XL: u16 = 0x006c;
const NEO: u16 = 0x009a;
const PLUS: u16 = 0x0084;
const PEDAL: u16 = 0x0086;

fn m(pid: u16) -> Model {
    model(pid).unwrap_or_else(|| panic!("model {pid:#06x} missing"))
}

fn key(n: u16) -> DeckInput {
    DeckInput {
        kind: DeckKind::Key,
        number: n,
    }
}

fn input(kind: DeckKind, n: u16) -> DeckInput {
    DeckInput { kind, number: n }
}

fn parse(s: &str) -> Action {
    s.parse().unwrap()
}

// --- CTL-45 model table -----------------------------------------------------

#[test]
fn ctl_45_the_table_holds_the_phase_1_models() {
    let all = models().unwrap();
    let mut pids: Vec<u16> = all.iter().map(|m| m.product_id).collect();
    pids.sort_unstable();
    let mut want = vec![
        ORIGINAL, 0x006d, MK2, MINI, 0x0090, XL, 0x008f, NEO, PLUS, PEDAL,
    ];
    want.sort_unstable();
    assert_eq!(pids, want);
    assert_eq!(ELGATO_VENDOR, 0x0fd9);
    for x in &all {
        assert!(x.cols > 0 && x.rows > 0, "{}", x.name);
        assert!(x.name.starts_with("Stream Deck"), "{}", x.name);
        // Only a model that passed a gate-3 run: the Original (VS7-07 run, 2026-10-25).
        assert_eq!(
            x.verified,
            x.product_id == 0x0060,
            "{} is verified only after a gate-3 run",
            x.name
        );
    }
}

#[test]
fn ctl_45_the_owners_original_is_15_keys_of_72_px_bmp() {
    let o = m(ORIGINAL);
    assert_eq!(o.name, "Stream Deck Original");
    assert_eq!((o.cols, o.rows, o.keys()), (5, 3, 15));
    assert_eq!(o.key_px, 72);
    assert_eq!(o.format, ImageFormat::Bmp);
    assert_eq!((o.rotation, o.flip), (0, [true, true]));
    assert_eq!(o.family, Family::V1);
}

#[test]
fn ctl_45_models_differ_by_table_only() {
    let mini = m(MINI);
    assert_eq!(
        (mini.keys(), mini.key_px, mini.format),
        (6, 80, ImageFormat::Bmp)
    );
    assert_eq!(
        (mini.rotation, mini.flip, mini.family),
        (90, [false, true], Family::Mini)
    );
    let xl = m(XL);
    assert_eq!(
        (xl.keys(), xl.key_px, xl.format),
        (32, 96, ImageFormat::Jpeg)
    );
    let neo = m(NEO);
    assert_eq!((neo.keys(), neo.touch_keys), (8, 2));
    assert_eq!(neo.info.map(|s| (s.w, s.h)), Some((248, 58)));
    let plus = m(PLUS);
    assert_eq!((plus.keys(), plus.key_px, plus.dials), (8, 120, 4));
    assert_eq!(plus.strip.map(|s| (s.w, s.h)), Some((800, 100)));
    assert_eq!(plus.flip, [false, false]);
    let pedal = m(PEDAL);
    assert_eq!((pedal.keys(), pedal.format), (3, ImageFormat::None));
}

#[test]
fn ctl_45_an_unknown_product_id_is_not_in_the_table() {
    assert!(model(0x9999).is_none());
}

// --- CTL-47 inputs ----------------------------------------------------------

#[test]
fn ctl_47_deck_inputs_parse_and_print() {
    let i: Input = serde_json::from_str(r#"{"kind":"key","number":5}"#).unwrap();
    assert_eq!(i, Input::Deck(key(5)));
    let i: Input = serde_json::from_str(r#"{"kind":"dial_push","number":2}"#).unwrap();
    assert_eq!(i, Input::Deck(input(DeckKind::DialPush, 2)));
    assert_eq!(
        serde_json::to_string(&Input::Deck(input(DeckKind::Strip, 3))).unwrap(),
        r#"{"kind":"strip","number":3}"#
    );
    // A MIDI input still parses as MIDI.
    let i: Input = serde_json::from_str(r#"{"kind":"note","channel":1,"number":60}"#).unwrap();
    assert!(matches!(i, Input::Midi(_)));
    assert_eq!(key(5).to_string(), "key 5");
    assert_eq!(input(DeckKind::DialPush, 2).to_string(), "dial push 2");
    assert_eq!(input(DeckKind::Touch, 1).to_string(), "touch 1");
}

#[test]
fn ctl_47_original_keys_are_numbered_as_printed() {
    // The Original reports its keys mirrored per row: printed key 1 (top left) is byte 5.
    let mut r = DeckReader::new(&m(ORIGINAL));
    let mut report = vec![0x01u8; 1];
    report.extend([0u8; 15]);
    assert_eq!(r.read(&report).unwrap(), vec![]);
    report[5] = 1;
    assert_eq!(
        r.read(&report).unwrap(),
        vec![(key(1), Reading::Button(true))]
    );
    assert_eq!(r.read(&report).unwrap(), vec![], "no change, no event");
    report[5] = 0;
    report[1] = 1; // device key 0 = printed key 5 (top right)
    assert_eq!(
        r.read(&report).unwrap(),
        vec![
            (key(1), Reading::Button(false)),
            (key(5), Reading::Button(true))
        ]
    );
    report[1] = 0;
    report[15] = 1; // device key 14 = printed key 11 (bottom left)
    let ev = r.read(&report).unwrap();
    assert!(ev.contains(&(key(11), Reading::Button(true))), "{ev:?}");
}

#[test]
fn ctl_47_mini_keys_are_not_mirrored() {
    let mut r = DeckReader::new(&m(MINI));
    let report = [0x01, 1, 0, 0, 0, 0, 0];
    assert_eq!(
        r.read(&report).unwrap(),
        vec![(key(1), Reading::Button(true))]
    );
}

#[test]
fn ctl_47_v2_keys_follow_a_4_byte_header() {
    let mut r = DeckReader::new(&m(MK2));
    let mut report = vec![0x01, 0x00, 0x0f, 0x00];
    report.extend([0u8; 15]);
    report[4 + 6] = 1;
    assert_eq!(
        r.read(&report).unwrap(),
        vec![(key(7), Reading::Button(true))]
    );
}

#[test]
fn ctl_47_neo_touch_keys_follow_the_keys() {
    let mut r = DeckReader::new(&m(NEO));
    let mut report = vec![0x01, 0x00, 0x0a, 0x00];
    report.extend([0u8; 10]);
    report[4 + 9] = 1;
    assert_eq!(
        r.read(&report).unwrap(),
        vec![(input(DeckKind::Touch, 2), Reading::Button(true))]
    );
}

#[test]
fn ctl_47_pedal_has_three_keys() {
    let mut r = DeckReader::new(&m(PEDAL));
    let report = [0x01, 0x00, 0x03, 0x00, 0, 1, 0];
    assert_eq!(
        r.read(&report).unwrap(),
        vec![(key(2), Reading::Button(true))]
    );
}

#[test]
fn ctl_47_plus_dials_turn_in_signed_steps_and_push() {
    let mut r = DeckReader::new(&m(PLUS));
    let turn = [0x01, 0x03, 0x05, 0x00, 0x01, 0xFE, 0x00, 0x00, 0x03];
    assert_eq!(
        r.read(&turn).unwrap(),
        vec![
            (input(DeckKind::Dial, 1), Reading::Steps(-2.0)),
            (input(DeckKind::Dial, 4), Reading::Steps(3.0)),
        ]
    );
    let push = [0x01, 0x03, 0x05, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00];
    assert_eq!(
        r.read(&push).unwrap(),
        vec![(input(DeckKind::DialPush, 2), Reading::Button(true))]
    );
    let up = [0x01, 0x03, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    assert_eq!(
        r.read(&up).unwrap(),
        vec![(input(DeckKind::DialPush, 2), Reading::Button(false))]
    );
}

#[test]
fn ctl_47_plus_strip_short_tap_hits_one_of_dials_segments() {
    let mut r = DeckReader::new(&m(PLUS));
    // x = 450 of 800 with 4 dials: segment 3.
    let tap = [0x01, 0x02, 0x0e, 0x00, 0x01, 0x00, 0xC2, 0x01, 0x32, 0x00];
    assert_eq!(
        r.read(&tap).unwrap(),
        vec![(input(DeckKind::Strip, 3), Reading::Tap)]
    );
    let x_max = [0x01, 0x02, 0x0e, 0x00, 0x01, 0x00, 0x20, 0x03, 0x32, 0x00]; // x = 800
    assert_eq!(
        r.read(&x_max).unwrap(),
        vec![(input(DeckKind::Strip, 4), Reading::Tap)]
    );
    // Long press (2) and drag (3) are ignored in phase 1.
    let long = [0x01, 0x02, 0x0e, 0x00, 0x02, 0x00, 0xC2, 0x01, 0x32, 0x00];
    assert_eq!(r.read(&long).unwrap(), vec![]);
    let drag = [0x01, 0x02, 0x0e, 0x00, 0x03, 0x00, 0xC2, 0x01, 0x32, 0x00];
    assert_eq!(r.read(&drag).unwrap(), vec![]);
}

#[test]
fn ctl_47_reports_that_do_not_parse_are_errors() {
    let mut r = DeckReader::new(&m(MK2));
    assert!(r.read(&[]).is_err());
    assert!(r.read(&[0x01, 0x00, 0x0f]).is_err(), "too short");
    assert!(r.read(&[0x07; 19]).is_err(), "wrong report id");
    let mut o = DeckReader::new(&m(ORIGINAL));
    assert!(o.read(&[0x01, 0, 0]).is_err(), "too short for 15 keys");
}

// --- image packets and brightness (CTL-45 report families) -----------------

#[test]
fn ctl_45_v1_original_image_goes_in_two_halves_of_8191_byte_reports() {
    let img: Vec<u8> = (0..15606u32).map(|i| (i % 251) as u8).collect();
    let reports = image_reports(&m(ORIGINAL), 1, &img);
    assert_eq!(reports.len(), 2);
    for r in &reports {
        assert_eq!(r.len(), 8191);
    }
    // Printed key 1 is device key 4; the header carries it + 1. Pages count from 1.
    assert_eq!(
        &reports[0][..16],
        &[2, 1, 1, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(
        &reports[1][..16],
        &[2, 1, 2, 0, 1, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(&reports[0][16..16 + 7803], &img[..7803]);
    assert_eq!(&reports[1][16..16 + 7803], &img[7803..]);
    assert!(
        reports[1][16 + 7803..].iter().all(|b| *b == 0),
        "zero padding"
    );
}

#[test]
fn ctl_45_mini_image_goes_in_1024_byte_reports_from_page_0() {
    let img = vec![0xAB; 2000];
    let reports = image_reports(&m(MINI), 3, &img);
    assert_eq!(reports.len(), 2);
    assert_eq!(&reports[0][..6], &[2, 1, 0, 0, 0, 3]);
    assert_eq!(&reports[1][..6], &[2, 1, 1, 0, 1, 3]);
    assert!(reports.iter().all(|r| r.len() == 1024));
    assert_eq!(reports[0][16..].len(), 1008);
    assert!(reports[1][16..16 + 992].iter().all(|b| *b == 0xAB));
    assert!(reports[1][16 + 992..].iter().all(|b| *b == 0));
}

#[test]
fn ctl_45_v2_image_reports_carry_length_and_page() {
    let img = vec![0x55; 1500];
    let reports = image_reports(&m(MK2), 1, &img);
    assert_eq!(reports.len(), 2);
    assert_eq!(&reports[0][..8], &[2, 7, 0, 0, 0xF8, 0x03, 0, 0]);
    assert_eq!(&reports[1][..8], &[2, 7, 0, 1, 0xE4, 0x01, 1, 0]);
    assert!(reports.iter().all(|r| r.len() == 1024));
    assert!(reports[1][8..8 + 484].iter().all(|b| *b == 0x55));
    assert!(reports[1][8 + 484..].iter().all(|b| *b == 0));
    // XL key 32 is device key 31.
    assert_eq!(image_reports(&m(XL), 32, &img)[0][2], 31);
}

#[test]
fn ctl_45_a_key_outside_the_model_sends_nothing() {
    assert!(image_reports(&m(MK2), 0, &[1, 2, 3]).is_empty());
    assert!(image_reports(&m(MK2), 16, &[1, 2, 3]).is_empty());
    assert!(
        image_reports(&m(PEDAL), 1, &[1, 2, 3]).is_empty(),
        "no images"
    );
}

#[test]
fn ctl_45_brightness_feature_reports() {
    let mut v1 = vec![0x05, 0x55, 0xAA, 0xD1, 0x01, 70];
    v1.resize(17, 0);
    assert_eq!(brightness_report(&m(ORIGINAL), 70), v1);
    assert_eq!(brightness_report(&m(MINI), 70), v1);
    let mut v2 = vec![0x03, 0x08, 70];
    v2.resize(32, 0);
    assert_eq!(brightness_report(&m(MK2), 70), v2);
    assert_eq!(brightness_report(&m(MK2), 250)[2], 100, "clamped to 100");
}

// --- CTL-48 deck pages ------------------------------------------------------

#[test]
fn ctl_48_deck_page_actions_parse_and_print() {
    for s in ["deck page next", "deck page prev", "deck page 3"] {
        assert_eq!(parse(s).to_string(), s);
    }
    assert_eq!(parse("deck page 3"), Action::DeckPage(3));
    assert_eq!(parse("deck page next"), Action::DeckPageNext);
    assert_eq!(parse("deck page prev"), Action::DeckPagePrev);
    assert!("deck page 0".parse::<Action>().is_err());
}

fn deck_mapping(json: &str) -> Mapping {
    let (mapping, warnings) = Mapping::from_json(json).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    mapping
}

const PAGED: &str = r#"{"name":"Deck","device":"Stream Deck Original (*","bindings":[
    {"input":{"kind":"key","number":1},"action":"upper 1"},
    {"input":{"kind":"key","number":1},"action":"go","deck_page":2},
    {"input":{"kind":"key","number":2},"action":"deck page 3","deck_page":2},
    {"input":{"kind":"key","number":2},"action":"deck page 2","deck_page":3}
]}"#;

#[test]
fn ctl_48_bindings_carry_a_deck_page_default_1() {
    let mapping = deck_mapping(PAGED);
    let pages: Vec<u16> = mapping.bindings.iter().map(|b| b.deck_page).collect();
    assert_eq!(pages, vec![1, 2, 2, 3]);
    assert_eq!(deck_pages(&mapping), 3);
    let json = mapping.to_json();
    assert_eq!(
        json.matches("deck_page").count(),
        3,
        "page 1 is not written"
    );
    assert_eq!(Mapping::from_json(&json).unwrap().0, mapping);
    assert_eq!(
        deck_pages(&deck_mapping(r#"{"name":"E","bindings":[]}"#)),
        1
    );
}

#[test]
fn ctl_48_turning_pages_stops_at_both_ends() {
    assert_eq!(turn_page(1, 3, &Action::DeckPageNext), 2);
    assert_eq!(turn_page(3, 3, &Action::DeckPageNext), 3);
    assert_eq!(turn_page(1, 3, &Action::DeckPagePrev), 1);
    assert_eq!(turn_page(2, 3, &Action::DeckPagePrev), 1);
    assert_eq!(turn_page(1, 3, &Action::DeckPage(3)), 3);
    assert_eq!(turn_page(1, 3, &Action::DeckPage(4)), 1, "no such page");
    assert_eq!(
        turn_page(2, 3, &parse("page next")),
        2,
        "console page, not deck"
    );
}

#[test]
fn ctl_48_only_the_current_deck_page_binds_keys() {
    let mapping = deck_mapping(PAGED);
    let on = |p: u16| -> Vec<String> {
        (mapping.bindings.iter())
            .filter(|b| on_page(b, p))
            .map(|b| b.action.to_string())
            .collect()
    };
    assert_eq!(on(1), vec!["upper 1"]);
    assert_eq!(on(2), vec!["go", "deck page 3"]);
    assert_eq!(on(3), vec!["deck page 2"]);
    // A dial is not a key: it is on every deck page.
    let dial = Binding::new(Input::Deck(input(DeckKind::Dial, 1)), parse("wheel 1"));
    assert!(on_page(&dial, 1) && on_page(&dial, 2));
}

#[test]
fn ctl_48_binding_a_key_replaces_only_on_its_deck_page() {
    let mut mapping = deck_mapping(PAGED);
    let mut b = Binding::new(Input::Deck(key(1)), parse("stop"));
    b.deck_page = 2;
    let old = mapping.bind(b).unwrap();
    assert_eq!(old.action, parse("go"));
    assert_eq!(mapping.bindings.len(), 4);
    assert_eq!(mapping.bindings[0].action, parse("upper 1"), "page 1 kept");
}

// --- CTL-49 default deck mapping --------------------------------------------

fn keys_of(mapping: &Mapping) -> Vec<(u16, String)> {
    let mut v: Vec<(u16, String)> = (mapping.bindings.iter())
        .filter_map(|b| match &b.input {
            Input::Deck(DeckInput {
                kind: DeckKind::Key,
                number,
            }) => Some((*number, b.action.to_string())),
            _ => None,
        })
        .collect();
    v.sort();
    v
}

fn of(kind: DeckKind, mapping: &Mapping) -> Vec<(u16, String)> {
    let mut v: Vec<(u16, String)> = (mapping.bindings.iter())
        .filter_map(|b| match &b.input {
            Input::Deck(d) if d.kind == kind => Some((d.number, b.action.to_string())),
            _ => None,
        })
        .collect();
    v.sort();
    v
}

fn pairs(v: &[(u16, &str)]) -> Vec<(u16, String)> {
    v.iter().map(|(n, s)| (*n, s.to_string())).collect()
}

#[test]
fn ctl_49_original_with_10_per_page() {
    let mapping = default_mapping(&m(ORIGINAL), 10);
    assert_eq!(mapping.name, "Stream Deck Original");
    assert!(cmdmx_control::mapping::device_matches(
        &mapping.device,
        "Stream Deck Original (A1B2)"
    ));
    assert!(!cmdmx_control::mapping::device_matches(
        &mapping.device,
        "Stream Deck Original V2 (A1B2)"
    ));
    let mut want: Vec<(u16, String)> = (1..=10).map(|n| (n, format!("upper {n}"))).collect();
    want.extend(pairs(&[(13, "page prev"), (14, "page next"), (15, "go")]));
    assert_eq!(keys_of(&mapping), want);
}

#[test]
fn ctl_49_mini_with_8_per_page() {
    let mapping = default_mapping(&m(MINI), 8);
    assert_eq!(
        keys_of(&mapping),
        pairs(&[
            (1, "upper 1"),
            (2, "upper 2"),
            (3, "upper 3"),
            (4, "upper 4"),
            (5, "upper 5"),
            (6, "go")
        ])
    );
}

#[test]
fn ctl_49_faders_per_page_is_never_assumed() {
    let xl = default_mapping(&m(XL), 40);
    let k = keys_of(&xl);
    assert_eq!(k.len(), 32);
    assert_eq!(k[28], (29, "upper 29".to_string()));
    assert_eq!(k[31], (32, "go".to_string()));
    let small = default_mapping(&m(MK2), 4);
    assert_eq!(keys_of(&small).len(), 4 + 3);
}

#[test]
fn ctl_49_pedal_neo_and_plus_extras() {
    assert_eq!(
        keys_of(&default_mapping(&m(PEDAL), 10)),
        pairs(&[(1, "back"), (2, "go"), (3, "stop")])
    );
    let neo = default_mapping(&m(NEO), 8);
    assert_eq!(
        of(DeckKind::Touch, &neo),
        pairs(&[(1, "deck page prev"), (2, "deck page next")])
    );
    assert_eq!(
        keys_of(&neo)[5..],
        pairs(&[(6, "page prev"), (7, "page next"), (8, "go")])
    );
    let plus = default_mapping(&m(PLUS), 8);
    assert_eq!(
        of(DeckKind::Dial, &plus),
        pairs(&[
            (1, "wheel 1"),
            (2, "wheel 2"),
            (3, "wheel 3"),
            (4, "wheel 4")
        ])
    );
    assert!(
        of(DeckKind::DialPush, &plus).is_empty(),
        "dial pushes unbound"
    );
}

#[test]
fn ctl_49_the_default_mapping_round_trips() {
    let mapping = default_mapping(&m(PLUS), 8);
    let (back, warnings) = Mapping::from_json(&mapping.to_json()).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(back, mapping);
}

// --- CTL-50 key looks -------------------------------------------------------

fn pb(legend: &str, level: f32) -> PlaybackView {
    PlaybackView {
        id: 12,
        legend: legend.to_string(),
        halo: Some([255, 48, 0]),
        level,
        running: false,
        function: Some("Flash".to_string()),
        held: false,
    }
}

fn bound(action: &str) -> Binding {
    Binding::new(Input::Deck(key(1)), parse(action))
}

#[test]
fn ctl_50_a_playback_key_shows_halo_legend_function_level_and_state() {
    let look = key_look(Some(&bound("upper 3")), &Subject::Playback(pb("Wash", 0.5)));
    assert_eq!(
        look,
        KeyLook {
            label: "Wash".to_string(),
            sub: "Flash".to_string(),
            halo: Some([255, 48, 0]),
            level: Some(0.5),
            state: KeyState::Active,
            fixed: false,
        }
    );
    let off = key_look(Some(&bound("upper 3")), &Subject::Playback(pb("Wash", 0.0)));
    assert_eq!(off.state, KeyState::Inactive);
    let mut running = pb("Wash", 0.0);
    running.running = true;
    let look = key_look(Some(&bound("go 3")), &Subject::Playback(running));
    assert_eq!(look.state, KeyState::Active, "running counts as active");
    let mut held = pb("Wash", 1.0);
    held.held = true;
    let look = key_look(Some(&bound("upper 3")), &Subject::Playback(held));
    assert_eq!(look.state, KeyState::Held);
}

#[test]
fn ctl_50_an_empty_legend_is_pb_id_and_a_missing_function_is_the_action_word() {
    let mut v = pb("", 0.0);
    v.function = None;
    v.halo = None;
    let look = key_look(Some(&bound("go 3")), &Subject::Playback(v));
    assert_eq!(look.label, "PB 12");
    assert_eq!(look.sub, "Go");
    assert_eq!(look.halo, None, "no halo, no bar");
}

#[test]
fn ctl_50_an_empty_slot_an_unbound_key_and_feedback_off_are_black() {
    assert_eq!(
        key_look(Some(&bound("upper 3")), &Subject::Empty),
        KeyLook::black()
    );
    assert_eq!(
        key_look(None, &Subject::Playback(pb("Wash", 1.0))),
        KeyLook::black()
    );
    let mut b = bound("upper 3");
    b.feedback = false;
    assert_eq!(
        key_look(Some(&b), &Subject::Playback(pb("Wash", 1.0))),
        KeyLook::black()
    );
    assert_eq!(KeyLook::black().state, KeyState::Off);
}

#[test]
fn ctl_50_other_actions_show_a_label_and_their_led_state() {
    let other = |action: &str, active: bool| {
        key_look(
            Some(&bound(action)),
            &Subject::Other {
                name: None,
                active,
                level: None,
            },
        )
    };
    assert_eq!(other("go", true).label, "Go");
    assert_eq!(other("go", true).state, KeyState::Active);
    assert_eq!(other("go", false).state, KeyState::Inactive);
    assert_eq!(other("page 2", false).label, "Page 2");
    assert_eq!(other("key clear", false).label, "Clear");
    assert_eq!(other("key rem_dim", false).label, "Rem dim");
    assert_eq!(other("deck page 2", false).label, "Deck 2");
    assert_eq!(other("gm", false).label, "GM");
    assert_eq!(other("keypad 7", false).label, "7");
    assert_eq!(other("go", false).level, None);
    let named = key_look(
        Some(&bound("group 4")),
        &Subject::Other {
            name: Some("Front".to_string()),
            active: true,
            level: None,
        },
    );
    assert_eq!(named.label, "Front");
    let gm = key_look(
        Some(&bound("gm")),
        &Subject::Other {
            name: None,
            active: true,
            level: Some(0.8),
        },
    );
    assert_eq!(gm.level, Some(0.8), "continuous actions show the level bar");
}

#[test]
fn ctl_50_binding_label_and_colour_replace_legend_and_halo() {
    let mut b = bound("upper 3");
    b.label = Some("Kerk".to_string());
    b.colour = Some("#00ff00".to_string());
    let look = key_look(Some(&b), &Subject::Playback(pb("Wash", 0.5)));
    assert_eq!(look.label, "Kerk");
    assert_eq!(look.halo, Some([0, 255, 0]));
    b.colour = Some("green".to_string());
    let look = key_look(Some(&b), &Subject::Playback(pb("Wash", 0.5)));
    assert_eq!(look.halo, Some([255, 48, 0]), "a bad colour is ignored");
    let json = serde_json::to_string(&b).unwrap();
    assert!(json.contains(r#""label":"Kerk""#), "{json}");
}

#[test]
fn ctl_50_the_legend_wraps_over_at_most_3_lines_and_shrinks() {
    let (short, short_px) = legend_lines("Wash", 72);
    assert_eq!(short, vec!["Wash"]);
    let (two, _) = legend_lines("Front wash", 72);
    assert!(!two.is_empty() && two.len() <= 2, "{two:?}");
    let (long, long_px) = legend_lines("Front wash stage left and right side warm", 72);
    assert!(long.len() <= 3, "{long:?}");
    assert!(long_px < short_px, "{long_px} vs {short_px}");
    let (cut, _) = legend_lines(
        "Supercalifragilisticexpialidocious beyond every realm of fitting on one small key",
        72,
    );
    assert!(cut.len() <= 3, "{cut:?}");
    assert!(cut.last().unwrap().ends_with('…'), "{cut:?}");
    assert_eq!(legend_lines("", 72).0, Vec::<String>::new());
}

// --- CTL-51 rendering ---------------------------------------------------------

fn sample() -> KeyLook {
    KeyLook {
        label: "Front Wash".to_string(),
        sub: "Flash".to_string(),
        halo: Some([255, 48, 0]),
        level: Some(0.5),
        state: KeyState::Active,
        fixed: false,
    }
}

fn px(rgb: &[u8], size: usize, x: usize, y: usize) -> [u8; 3] {
    let i = (y * size + x) * 3;
    [rgb[i], rgb[i + 1], rgb[i + 2]]
}

#[test]
fn ctl_51_black_look_is_all_black() {
    let rgb = render_rgb(&KeyLook::black(), 72);
    assert_eq!(rgb.len(), 72 * 72 * 3);
    assert!(rgb.iter().all(|b| *b == 0));
}

#[test]
fn ctl_50_the_level_fills_the_top_bar_in_the_halo_colour_and_the_bottom_stays_black() {
    let rgb = render_rgb(&sample(), 72);
    // Level 0.5: the left half of the top bar in the halo, the rest the halo at 25 %.
    assert_eq!(px(&rgb, 72, 10, 1), [255, 48, 0], "filled part");
    assert_eq!(px(&rgb, 72, 62, 1), [64, 12, 0], "unfilled part");
    for x in [2, 36, 70] {
        assert_eq!(px(&rgb, 72, x, 71), [0, 0, 0], "nothing along the bottom");
    }
    let mut no_halo = sample();
    no_halo.halo = None;
    let rgb = render_rgb(&no_halo, 72);
    assert_eq!(px(&rgb, 72, 10, 1), [255, 255, 255], "white without a halo");
    assert_eq!(px(&rgb, 72, 62, 1), [40, 40, 40], "grey 40 without a halo");
    let mut no_level = sample();
    no_level.level = None;
    let rgb = render_rgb(&no_level, 72);
    assert_eq!(px(&rgb, 72, 62, 1), [255, 48, 0], "no level: the halo bar");
    no_level.halo = None;
    assert_eq!(px(&render_rgb(&no_level, 72), 72, 36, 1), [0, 0, 0]);
}

#[test]
fn ctl_50_a_key_that_does_not_follow_the_page_is_fixed_and_shows_a_lock() {
    let p = Subject::Playback(pb("Wash", 0.0));
    assert!(!key_look(Some(&bound("upper 3")), &p).fixed);
    assert!(key_look(Some(&bound("upper 1.3")), &p).fixed);
    assert!(key_look(Some(&bound("go playback 12")), &p).fixed);
    assert!(key_look(Some(&bound("playback 12 level")), &p).fixed);
    let gm = Subject::Other {
        name: None,
        active: true,
        level: Some(1.0),
    };
    assert!(!key_look(Some(&bound("gm")), &gm).fixed);
    // The lock: the top-right corner under the bar, black on a key that follows the page.
    let corner = |look: &KeyLook| {
        let rgb = render_rgb(look, 72);
        (58..72).any(|x| (9..23).any(|y| px(&rgb, 72, x, y) != [0, 0, 0]))
    };
    let mut look = sample();
    assert!(!corner(&look));
    look.fixed = true;
    assert!(corner(&look));
}

#[test]
fn ctl_51_inactive_is_dimmed_to_40_percent_and_held_is_inverted() {
    let mut dim = sample();
    dim.state = KeyState::Inactive;
    assert_eq!(px(&render_rgb(&dim, 72), 72, 10, 1), [102, 19, 0]);
    let mut held = sample();
    held.state = KeyState::Held;
    let rgb = render_rgb(&held, 72);
    assert_eq!(px(&rgb, 72, 10, 1), [0, 207, 255], "halo inverted");
    assert_eq!(
        px(&rgb, 72, 1, 36),
        [255, 255, 255],
        "black background inverted"
    );
}

#[test]
fn ctl_51_the_legend_is_drawn() {
    let mut text = sample();
    text.halo = None;
    text.level = None;
    text.sub = String::new();
    assert!(render_rgb(&text, 72).iter().any(|b| *b > 128));
}

#[test]
fn ctl_51_bmp_is_bottom_up_bgr_after_flip() {
    // One red pixel top left; the Original flips both ways: it lands bottom right, which a
    // bottom-up BMP stores in its first row.
    let mut rgb = vec![0u8; 72 * 72 * 3];
    rgb[0] = 255;
    let bmp = encode(&rgb, &m(ORIGINAL));
    assert_eq!(bmp.len(), 54 + 72 * 72 * 3);
    assert_eq!(&bmp[..2], b"BM");
    assert_eq!(u32::from_le_bytes(bmp[2..6].try_into().unwrap()), 15606);
    assert_eq!(u32::from_le_bytes(bmp[10..14].try_into().unwrap()), 54);
    assert_eq!(i32::from_le_bytes(bmp[18..22].try_into().unwrap()), 72);
    assert_eq!(i32::from_le_bytes(bmp[22..26].try_into().unwrap()), 72);
    assert_eq!(u16::from_le_bytes(bmp[28..30].try_into().unwrap()), 24);
    let at = 54 + 71 * 3;
    assert_eq!(&bmp[at..at + 3], &[0, 0, 255]);
    assert_eq!(bmp.iter().filter(|b| **b == 255).count(), 1);
}

#[test]
fn ctl_51_mini_rotates_90_then_flips_vertically() {
    // Top left, rotated 90° counter-clockwise, goes bottom left; the vertical flip brings it
    // back top left, which a bottom-up BMP stores in its last row.
    let mut rgb = vec![0u8; 80 * 80 * 3];
    rgb[0] = 255;
    let bmp = encode(&rgb, &m(MINI));
    assert_eq!(bmp.len(), 54 + 80 * 80 * 3);
    let at = 54 + (79 * 80) * 3;
    assert_eq!(&bmp[at..at + 3], &[0, 0, 255]);
    // A pixel top right goes top left after rotating, bottom left after the flip.
    let mut rgb = vec![0u8; 80 * 80 * 3];
    rgb[79 * 3 + 1] = 255;
    let bmp = encode(&rgb, &m(MINI));
    assert_eq!(&bmp[54..57], &[0, 255, 0]);
}

#[test]
fn ctl_51_jpeg_has_the_models_size() {
    for (pid, size) in [(MK2, 72u16), (XL, 96), (PLUS, 120)] {
        let jpg = render(&sample(), &m(pid));
        assert_eq!(&jpg[..2], &[0xFF, 0xD8], "SOI");
        assert_eq!(&jpg[jpg.len() - 2..], &[0xFF, 0xD9], "EOI");
        let sof = jpg.windows(2).position(|w| w == [0xFF, 0xC0]).unwrap();
        let h = u16::from_be_bytes([jpg[sof + 5], jpg[sof + 6]]);
        let w = u16::from_be_bytes([jpg[sof + 7], jpg[sof + 8]]);
        assert_eq!((w, h), (size, size));
    }
}

#[test]
fn ctl_51_a_model_without_images_renders_nothing() {
    assert!(render(&sample(), &m(PEDAL)).is_empty());
}

#[test]
fn ctl_51_same_look_and_model_same_bytes() {
    for pid in [ORIGINAL, MK2, MINI, XL, PLUS] {
        assert_eq!(render(&sample(), &m(pid)), render(&sample(), &m(pid)));
    }
}

fn golden(name: &str, bytes: &[u8]) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name);
    if std::env::var_os("CMDMX_BLESS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes).unwrap();
        return;
    }
    let want = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("{} missing ({e}); run with CMDMX_BLESS=1", path.display()));
    assert!(want == bytes, "{name} differs from its golden image");
}

#[test]
fn ctl_51_golden_images_per_key_size() {
    let mut dim = sample();
    dim.state = KeyState::Inactive;
    dim.label = "Supercalifragilistic organ loft".to_string();
    let mut held = sample();
    held.state = KeyState::Held;
    golden("original-72.bmp", &render(&sample(), &m(ORIGINAL)));
    golden("original-72-inactive.bmp", &render(&dim, &m(ORIGINAL)));
    golden("mk2-72.jpg", &render(&sample(), &m(MK2)));
    golden("mini-80.bmp", &render(&sample(), &m(MINI)));
    golden("xl-96.jpg", &render(&held, &m(XL)));
    golden("plus-120.jpg", &render(&sample(), &m(PLUS)));
}
