//! Action catalogue, mapping JSON, learn guess and feedback diffing
//! (specs/modules/control-surfaces.md CTL-05, 17..20, 22, 26, 27).
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use cmdmx_control::action::{Action, Key, KeypadToken, Target, ValueKind};
use cmdmx_control::feedback::Feedback;
use cmdmx_control::learn;
use cmdmx_control::mapping::{device_matches, Binding, Encoding, Input, Mapping};
use cmdmx_control::midi::{Kind, MidiInput, MidiMessage};

fn parse(s: &str) -> Action {
    s.parse().unwrap()
}

fn cc(number: u8, value: u16) -> MidiMessage {
    MidiMessage {
        input: MidiInput {
            kind: Kind::Cc,
            channel: 1,
            number: Some(number),
        },
        value,
    }
}

// --- CTL-05 action catalogue ----------------------------------------------

#[test]
fn ctl_05_parses_fader_forms() {
    assert_eq!(parse("fader 3"), Action::Fader(Target::Slot(3)));
    assert_eq!(
        parse("fader 2.3"),
        Action::Fader(Target::PageSlot { page: 2, slot: 3 })
    );
    assert_eq!(
        parse("playback 12 level"),
        Action::Fader(Target::Playback(12))
    );
}

#[test]
fn ctl_05_parses_handle_and_go_actions() {
    assert_eq!(parse("upper 3"), Action::Upper(Target::Slot(3)));
    assert_eq!(
        parse("lower 2.1"),
        Action::Lower(Target::PageSlot { page: 2, slot: 1 })
    );
    assert_eq!(parse("go"), Action::Go(Target::Connected));
    assert_eq!(parse("go 4"), Action::Go(Target::Slot(4)));
    assert_eq!(
        parse("back playback 12"),
        Action::Back(Target::Playback(12))
    );
    assert_eq!(parse("stop"), Action::Stop(Target::Connected));
    assert_eq!(parse("release 1"), Action::Release(Target::Slot(1)));
    assert_eq!(parse("tap"), Action::Tap(Target::Connected));
}

#[test]
fn ctl_05_parses_page_gm_dbo_keys_wheels_and_selections() {
    assert_eq!(parse("page next"), Action::PageNext);
    assert_eq!(parse("page prev"), Action::PagePrev);
    assert_eq!(parse("page 4"), Action::Page(4));
    assert_eq!(parse("gm"), Action::Gm);
    assert_eq!(parse("dbo"), Action::Dbo);
    assert_eq!(parse("key clear"), Action::Key(Key::Clear));
    assert_eq!(parse("key rem_dim"), Action::Key(Key::RemDim));
    assert_eq!(parse("key step_prev"), Action::Key(Key::StepPrev));
    assert_eq!(parse("keypad 7"), Action::Keypad(KeypadToken::Digit(7)));
    assert_eq!(parse("keypad Thro"), Action::Keypad(KeypadToken::Thro));
    assert_eq!(parse("keypad @"), Action::Keypad(KeypadToken::At));
    assert_eq!(parse("keypad ."), Action::Keypad(KeypadToken::Dot));
    assert_eq!(parse("wheel 2"), Action::Wheel(2));
    assert_eq!(parse("group 5"), Action::Group(5));
    assert_eq!(parse("palette 7"), Action::Palette(7));
    assert!(matches!(
        parse("attribute_group FX"),
        Action::AttributeGroup(_)
    ));
    assert!(matches!(
        parse("attribute_group I"),
        Action::AttributeGroup(_)
    ));
}

#[test]
fn ctl_05_display_round_trips_every_form() {
    for s in [
        "fader 3",
        "fader 2.3",
        "playback 12 level",
        "upper 3",
        "lower 2.1",
        "go",
        "go 4",
        "back playback 12",
        "stop",
        "release",
        "tap 2",
        "page next",
        "page prev",
        "page 4",
        "gm",
        "dbo",
        "key clear",
        "key locate",
        "key highlight",
        "key rem_dim",
        "key undo",
        "key redo",
        "key all",
        "key step_next",
        "key step_prev",
        "keypad 0",
        "keypad .",
        "keypad @",
        "keypad Thro",
        "keypad And",
        "keypad Not",
        "keypad Fixture",
        "keypad Group",
        "keypad Palette",
        "keypad Full",
        "keypad Enter",
        "keypad Backspace",
        "wheel 2",
        "attribute_group P",
        "attribute_group FX",
        "group 5",
        "palette 7",
    ] {
        assert_eq!(parse(s).to_string(), s);
    }
}

#[test]
fn ctl_05_refuses_strings_outside_the_catalogue() {
    for s in [
        "",
        "fader x",
        "fader 0",
        "fader 2.0",
        "fader",
        "page 0",
        "page up",
        "key nuke",
        "keypad 10",
        "wheel 0",
        "attribute_group Q",
        "go playback",
        "gm 1",
        "launch missiles",
    ] {
        let err = s.parse::<Action>().unwrap_err();
        assert_eq!(err.to_string(), format!("unknown action \"{s}\""), "{s}");
    }
}

#[test]
fn ctl_05_tolerates_extra_whitespace() {
    assert_eq!(parse("  fader   3 "), Action::Fader(Target::Slot(3)));
}

#[test]
fn ctl_05_value_kinds() {
    assert_eq!(parse("fader 1").value_kind(), ValueKind::Continuous);
    assert_eq!(
        parse("playback 3 level").value_kind(),
        ValueKind::Continuous
    );
    assert_eq!(parse("gm").value_kind(), ValueKind::Continuous);
    assert_eq!(parse("wheel 1").value_kind(), ValueKind::Relative);
    assert_eq!(parse("go").value_kind(), ValueKind::Button);
    assert_eq!(parse("dbo").value_kind(), ValueKind::Button);
    assert_eq!(parse("keypad Enter").value_kind(), ValueKind::Button);
}

// --- CTL-22 mapping JSON ----------------------------------------------------

const SPEC_EXAMPLE: &str = r#"{"name":"nanoKONTROL2","device":"nanoKONTROL2*","bindings":[{"input":{"kind":"cc","channel":1,"number":0},"action":"fader 1","pickup":true},{"input":{"kind":"note","channel":1,"number":41},"action":"go","on_value":127}]}"#;

#[test]
fn ctl_22_spec_example_loads() {
    let (m, warnings) = Mapping::from_json(SPEC_EXAMPLE).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(m.name, "nanoKONTROL2");
    assert_eq!(m.device, "nanoKONTROL2*");
    assert_eq!(m.bindings.len(), 2);
    let b = &m.bindings[0];
    assert_eq!(
        b.input,
        Input::Midi(MidiInput {
            kind: Kind::Cc,
            channel: 1,
            number: Some(0)
        })
    );
    assert_eq!(b.action, parse("fader 1"));
    assert!(b.pickup);
    // Defaults.
    assert_eq!(b.min, 0.0);
    assert_eq!(b.max, 1.0);
    assert!(!b.invert);
    assert!(b.feedback);
    assert_eq!(b.on_value, 127);
    assert_eq!(b.off_value, 0);
    assert_eq!(b.encoding, None);
}

#[test]
fn ctl_22_options_and_osc_inputs_load() {
    let json = r#"{"name":"m","device":"*","bindings":[
        {"input":{"kind":"cc","channel":2,"number":16},"action":"wheel 1","encoding":"twos_complement"},
        {"input":{"kind":"note","channel":1,"number":1},"action":"dbo","button_mode":"toggle","on_value":5,"feedback_to":{"kind":"note","channel":1,"number":2}},
        {"input":{"kind":"cc","channel":1,"number":7},"action":"gm","min":0.1,"max":0.9,"invert":true,"pickup":false,"feedback":false},
        {"input":"/1/fader1","action":"fader 1"}
    ]}"#;
    let (m, warnings) = Mapping::from_json(json).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(m.bindings[0].encoding, Some(Encoding::TwosComplement));
    assert_eq!(m.bindings[1].on_value, 5);
    assert!(m.bindings[1].feedback_to.is_some());
    assert!(m.bindings[2].invert && !m.bindings[2].pickup && !m.bindings[2].feedback);
    assert_eq!(m.bindings[3].input, Input::Osc("/1/fader1".into()));
}

#[test]
fn ctl_22_bad_binding_is_left_out_with_one_warning_naming_it() {
    let json = r#"{"name":"m","device":"*","bindings":[
        {"input":{"kind":"cc","channel":1,"number":0},"action":"fader x"},
        {"input":{"kind":"cc","channel":1,"number":1},"action":"fader 2"},
        {"input":{"kind":"wobble","channel":1,"number":2},"action":"go"}
    ]}"#;
    let (m, warnings) = Mapping::from_json(json).unwrap();
    assert_eq!(m.bindings.len(), 1);
    assert_eq!(m.bindings[0].action, parse("fader 2"));
    assert_eq!(warnings.len(), 2);
    assert!(warnings[0].contains("fader x"), "{}", warnings[0]);
}

#[test]
fn ctl_22_not_a_mapping_is_an_error() {
    assert!(Mapping::from_json("not json").is_err());
    assert!(Mapping::from_json(r#"{"bindings":[]}"#).is_err()); // no name
}

#[test]
fn ctl_22_unknown_fields_are_kept_on_save() {
    let json = r#"{"name":"m","device":"*","colour":"red","bindings":[
        {"input":{"kind":"cc","channel":1,"number":1},"action":"fader 2","note":"left knob"}
    ]}"#;
    let (m, _) = Mapping::from_json(json).unwrap();
    let saved: serde_json::Value = serde_json::from_str(&m.to_json()).unwrap();
    assert_eq!(saved["colour"], "red");
    assert_eq!(saved["bindings"][0]["note"], "left knob");
    assert_eq!(saved["bindings"][0]["action"], "fader 2");
    // And it loads back the same.
    let (again, _) = Mapping::from_json(&m.to_json()).unwrap();
    assert_eq!(again, m);
}

#[test]
fn ctl_22_device_patterns() {
    assert!(device_matches("nanoKONTROL2*", "nanoKONTROL2 SLIDER/KNOB"));
    assert!(device_matches("*", "anything"));
    assert!(device_matches("BCF2000", "BCF2000"));
    assert!(!device_matches("BCF2000", "BCF2000 #2"));
    assert!(device_matches("*X-Touch*", "Behringer X-Touch Extender"));
    assert!(!device_matches("X-Touch*", "iCON"));
}

// --- CTL-27 one input, one action ---------------------------------------------

#[test]
fn ctl_27_binding_an_input_again_replaces_and_reports_the_old_action() {
    let (mut m, _) = Mapping::from_json(SPEC_EXAMPLE).unwrap();
    let input = Input::Midi(MidiInput {
        kind: Kind::Cc,
        channel: 1,
        number: Some(0),
    });
    let old = m.bind(Binding::new(input.clone(), parse("fader 2")));
    assert_eq!(old.map(|b| b.action), Some(parse("fader 1")));
    assert_eq!(m.bindings.len(), 2);
    assert_eq!(m.bindings.iter().filter(|b| b.input == input).count(), 1);
    // An action may have several inputs.
    let other = Input::Midi(MidiInput {
        kind: Kind::Cc,
        channel: 1,
        number: Some(9),
    });
    assert_eq!(m.bind(Binding::new(other, parse("fader 2"))), None);
    assert_eq!(m.bindings.len(), 3);
}

#[test]
fn ctl_25_input_text_for_messages() {
    let i = MidiInput {
        kind: Kind::Cc,
        channel: 1,
        number: Some(7),
    };
    assert_eq!(i.to_string(), "CC 7 ch 1");
    let n = MidiInput {
        kind: Kind::Note,
        channel: 2,
        number: Some(41),
    };
    assert_eq!(n.to_string(), "Note 41 ch 2");
    let p = MidiInput {
        kind: Kind::PitchBend,
        channel: 9,
        number: None,
    };
    assert_eq!(p.to_string(), "Pitch bend ch 9");
}

// --- CTL-26 learn guess -------------------------------------------------------

#[test]
fn ctl_26_note_guesses_a_button() {
    let note = MidiMessage {
        input: MidiInput {
            kind: Kind::Note,
            channel: 1,
            number: Some(41),
        },
        value: 127,
    };
    let b = learn::guess(parse("go"), &[note]).unwrap();
    assert_eq!(b.input, Input::Midi(note.input));
    assert_eq!(b.encoding, None);
}

#[test]
fn ctl_26_pitch_bend_guesses_continuous() {
    let pb = MidiMessage {
        input: MidiInput {
            kind: Kind::PitchBend,
            channel: 1,
            number: None,
        },
        value: 9000,
    };
    let b = learn::guess(parse("fader 1"), &[pb]).unwrap();
    assert_eq!(b.input, Input::Midi(pb.input));
    assert_eq!(b.encoding, None);
}

#[test]
fn ctl_26_cc_with_its_lsb_guesses_cc14() {
    let b = learn::guess(parse("fader 1"), &[cc(7, 64), cc(39, 3)]).unwrap();
    assert_eq!(
        b.input,
        Input::Midi(MidiInput {
            kind: Kind::Cc14,
            channel: 1,
            number: Some(7)
        })
    );
}

#[test]
fn ctl_26_relative_cc_guesses_its_encoding() {
    let b = learn::guess(parse("wheel 1"), &[cc(16, 1), cc(16, 2), cc(16, 127)]).unwrap();
    assert_eq!(b.encoding, Some(Encoding::TwosComplement));
    let b = learn::guess(parse("wheel 1"), &[cc(16, 65), cc(16, 63), cc(16, 66)]).unwrap();
    assert_eq!(b.encoding, Some(Encoding::BinaryOffset));
}

#[test]
fn ctl_26_any_other_cc_guesses_continuous() {
    let b = learn::guess(parse("fader 1"), &[cc(7, 30), cc(7, 40), cc(7, 50)]).unwrap();
    assert_eq!(b.encoding, None);
    assert_eq!(
        b.input,
        Input::Midi(MidiInput {
            kind: Kind::Cc,
            channel: 1,
            number: Some(7)
        })
    );
}

#[test]
fn ctl_26_nothing_observed_guesses_nothing() {
    assert_eq!(learn::guess(parse("go"), &[]), None);
}

// --- CTL-17..20 feedback diffing ----------------------------------------------

#[test]
fn ctl_19_sends_only_on_change() {
    let mut f: Feedback<u8, u16> = Feedback::default();
    assert_eq!(
        f.tick(vec![(1, 10), (2, 20)], &[], &[]),
        vec![(1, 10), (2, 20)]
    );
    assert_eq!(f.tick(vec![(1, 10), (2, 21)], &[], &[]), vec![(2, 21)]);
    assert_eq!(f.tick(vec![(1, 10), (2, 21)], &[], &[]), vec![]);
}

#[test]
fn ctl_19_at_most_once_per_element_per_tick_last_wins() {
    let mut f: Feedback<u8, u16> = Feedback::default();
    assert_eq!(
        f.tick(vec![(1, 10), (1, 11), (1, 12)], &[], &[]),
        vec![(1, 12)]
    );
}

#[test]
fn ctl_20_no_echo_of_an_inputs_own_move() {
    let mut f: Feedback<u8, u16> = Feedback::default();
    f.tick(vec![(1, 10)], &[], &[]);
    // The input moved element 1 to 50 this tick; the engine took it.
    assert_eq!(f.tick(vec![(1, 50)], &[(1, 50)], &[]), vec![]);
    assert_eq!(f.tick(vec![(1, 50)], &[], &[]), vec![]);
    // Moved to 60 but the engine clamped to 55: the next tick corrects it.
    assert_eq!(f.tick(vec![(1, 55)], &[(1, 60)], &[]), vec![]);
    assert_eq!(f.tick(vec![(1, 55)], &[], &[]), vec![(1, 55)]);
}

#[test]
fn ctl_20_touched_motor_fader_gets_its_level_once_at_release() {
    let mut f: Feedback<u8, u16> = Feedback::default();
    f.tick(vec![(1, 10)], &[], &[]);
    assert_eq!(f.tick(vec![(1, 30)], &[], &[1]), vec![]);
    assert_eq!(f.tick(vec![(1, 40)], &[], &[1]), vec![]);
    // Released: the engine level once, even when it equals what was last sent.
    assert_eq!(f.tick(vec![(1, 10)], &[], &[]), vec![(1, 10)]);
    assert_eq!(f.tick(vec![(1, 10)], &[], &[]), vec![]);
}

#[test]
fn ctl_21_clear_sends_the_full_state_again() {
    let mut f: Feedback<String, f32> = Feedback::default();
    f.tick(vec![("/cmdmx/gm".into(), 1.0)], &[], &[]);
    f.clear();
    assert_eq!(
        f.tick(vec![("/cmdmx/gm".into(), 1.0)], &[], &[]),
        vec![("/cmdmx/gm".into(), 1.0)]
    );
}
