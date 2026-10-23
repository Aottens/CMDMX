//! Generic MIDI in: decode, scaling, relative encoders, buttons, conversions
//! and pickup (specs/modules/control-surfaces.md CTL-07, 09..13).
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use cmdmx_control::action::Action;
use cmdmx_control::convert::{ActionValue, BindingState, Pickup, Reading, Resolution};
use cmdmx_control::mapping::{Binding, ButtonMode, Encoding, Input};
use cmdmx_control::midi::{self, Decoder, Kind, MidiInput, MidiMessage};

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.0005
}

fn input(kind: Kind, channel: u8, number: Option<u8>) -> MidiInput {
    MidiInput {
        kind,
        channel,
        number,
    }
}

fn msg(kind: Kind, channel: u8, number: Option<u8>, value: u16) -> MidiMessage {
    MidiMessage {
        input: input(kind, channel, number),
        value,
    }
}

fn binding(kind: Kind, number: Option<u8>, action: &str) -> Binding {
    Binding::new(
        Input::Midi(input(kind, 1, number)),
        action.parse::<Action>().unwrap(),
    )
}

// --- CTL-09 decode -------------------------------------------------------

#[test]
fn ctl_09_note_on_off_and_velocity_zero() {
    let mut d = Decoder::default();
    let out = d.decode(&[0x90, 41, 100, 0x90, 41, 0, 0x80, 42, 64]);
    assert_eq!(
        out,
        vec![
            msg(Kind::Note, 1, Some(41), 100),
            msg(Kind::Note, 1, Some(41), 0),
            msg(Kind::Note, 1, Some(42), 0),
        ]
    );
}

#[test]
fn ctl_09_cc_pitch_bend_and_program() {
    let mut d = Decoder::default();
    let out = d.decode(&[0xB1, 7, 64, 0xE2, 0x7F, 0x7F, 0xC0, 5]);
    assert_eq!(
        out,
        vec![
            msg(Kind::Cc, 2, Some(7), 64),
            msg(Kind::PitchBend, 3, None, 16383),
            msg(Kind::Program, 1, Some(5), 127),
        ]
    );
}

#[test]
fn ctl_09_running_status_and_several_messages_per_packet() {
    let mut d = Decoder::default();
    let out = d.decode(&[0xB0, 1, 10, 2, 20, 3, 30]);
    assert_eq!(
        out,
        vec![
            msg(Kind::Cc, 1, Some(1), 10),
            msg(Kind::Cc, 1, Some(2), 20),
            msg(Kind::Cc, 1, Some(3), 30),
        ]
    );
    // Running status carries into the next packet.
    assert_eq!(d.decode(&[4, 40]), vec![msg(Kind::Cc, 1, Some(4), 40)]);
}

#[test]
fn ctl_09_ignored_messages_and_realtime_inside_running_status() {
    let mut d = Decoder::default();
    // aftertouch, channel pressure, sysex, clock in the middle of a CC run,
    // active sensing; then a CC that still uses running status after the clock.
    let out = d.decode(&[
        0xA0, 60, 10, 0xD0, 5, 0xF0, 1, 2, 3, 0xF7, 0xB0, 7, 1, 0xF8, 8, 2, 0xFE,
    ]);
    assert_eq!(
        out,
        vec![msg(Kind::Cc, 1, Some(7), 1), msg(Kind::Cc, 1, Some(8), 2)]
    );
}

#[test]
fn ctl_09_truncated_message_is_dropped() {
    let mut d = Decoder::default();
    assert_eq!(d.decode(&[0xB0, 7]), vec![]);
    assert_eq!(d.decode(&[0x42]), vec![]); // data byte without any status
}

#[test]
fn ctl_09_cc14_applied_when_lsb_arrives() {
    let mut d = Decoder::new(vec![(1, 7)]);
    let out = d.decode(&[0xB0, 7, 0x40, 0xB0, 39, 0x01]);
    assert_eq!(out, vec![msg(Kind::Cc14, 1, Some(7), (0x40 << 7) | 1)]);
}

#[test]
fn ctl_09_cc14_msb_alone_applies_at_end_of_packet() {
    let mut d = Decoder::new(vec![(1, 7)]);
    assert_eq!(
        d.decode(&[0xB0, 7, 0x40]),
        vec![msg(Kind::Cc14, 1, Some(7), 0x40 << 7)]
    );
    // A later LSB alone combines with the last MSB.
    assert_eq!(
        d.decode(&[0xB0, 39, 0x05]),
        vec![msg(Kind::Cc14, 1, Some(7), (0x40 << 7) | 5)]
    );
}

#[test]
fn ctl_09_cc_without_cc14_binding_stays_7_bit() {
    let mut d = Decoder::new(vec![(2, 7)]); // other channel
    assert_eq!(
        d.decode(&[0xB0, 7, 0x40, 39, 1]),
        vec![
            msg(Kind::Cc, 1, Some(7), 0x40),
            msg(Kind::Cc, 1, Some(39), 1)
        ]
    );
}

#[test]
fn ctl_17_encode_feedback_messages() {
    assert_eq!(
        midi::encode(&input(Kind::Note, 1, Some(41)), 127),
        vec![0x90, 41, 127]
    );
    assert_eq!(
        midi::encode(&input(Kind::Cc, 2, Some(7)), 64),
        vec![0xB1, 7, 64]
    );
    assert_eq!(
        midi::encode(&input(Kind::Cc14, 1, Some(7)), (0x40 << 7) | 1),
        vec![0xB0, 7, 0x40, 0xB0, 39, 1]
    );
    assert_eq!(
        midi::encode(&input(Kind::PitchBend, 3, None), 16383),
        vec![0xE2, 0x7F, 0x7F]
    );
    assert_eq!(
        midi::encode(&input(Kind::Program, 1, Some(5)), 127),
        Vec::<u8>::new()
    );
}

#[test]
fn ctl_17_level_to_raw_matches_scaling() {
    assert_eq!(midi::to_raw(Kind::Cc, 1.0), 127);
    assert_eq!(midi::to_raw(Kind::Cc, 0.504), 64);
    assert_eq!(midi::to_raw(Kind::PitchBend, 1.0), 16383);
    assert_eq!(midi::to_raw(Kind::Cc14, 0.0), 0);
    assert_eq!(midi::to_raw(Kind::Cc, 2.0), 127);
}

// --- CTL-10 scaling -------------------------------------------------------

#[test]
fn ctl_10_7_bit_scaling_127_is_exactly_one() {
    let b = binding(Kind::Cc, Some(7), "fader 1");
    assert_eq!(b.level(&msg(Kind::Cc, 1, Some(7), 127)), 1.0);
    assert_eq!(b.level(&msg(Kind::Cc, 1, Some(7), 0)), 0.0);
    assert!(close(b.level(&msg(Kind::Cc, 1, Some(7), 64)), 0.504));
}

#[test]
fn ctl_10_14_bit_scaling() {
    let b = binding(Kind::PitchBend, None, "fader 1");
    assert_eq!(b.level(&msg(Kind::PitchBend, 1, None, 16383)), 1.0);
    assert!(close(b.level(&msg(Kind::PitchBend, 1, None, 8192)), 0.5));
}

#[test]
fn ctl_10_invert_example() {
    let mut b = binding(Kind::Cc, Some(7), "fader 1");
    b.invert = true;
    assert!(close(b.level(&msg(Kind::Cc, 1, Some(7), 64)), 0.496));
}

#[test]
fn ctl_10_min_max_is_input_range_and_clamps() {
    let mut b = binding(Kind::Cc, Some(7), "fader 1");
    b.min = 0.2;
    b.max = 0.6;
    assert_eq!(b.level(&msg(Kind::Cc, 1, Some(7), 0)), 0.0);
    assert_eq!(b.level(&msg(Kind::Cc, 1, Some(7), 127)), 1.0);
    // 0.4 is halfway through 0.2..0.6.
    let mid = b.level(&msg(Kind::Cc, 1, Some(7), 51)); // 51/127 = 0.4016
    assert!((mid - 0.504).abs() < 0.01, "{mid}");
}

// --- CTL-11 relative encoders ---------------------------------------------

#[test]
fn ctl_11_twos_complement_decodes_minus_3() {
    assert_eq!(Encoding::TwosComplement.steps(0x7D), -3);
    assert_eq!(Encoding::TwosComplement.steps(0x01), 1);
    assert_eq!(Encoding::TwosComplement.steps(63), 63);
    assert_eq!(Encoding::TwosComplement.steps(65), -63);
}

#[test]
fn ctl_11_binary_offset_decodes_plus_3() {
    assert_eq!(Encoding::BinaryOffset.steps(0x43), 3);
    assert_eq!(Encoding::BinaryOffset.steps(64), 0);
    assert_eq!(Encoding::BinaryOffset.steps(63), -1);
}

#[test]
fn ctl_11_sign_magnitude_bit_6_is_down() {
    assert_eq!(Encoding::SignMagnitude.steps(0x03), 3);
    assert_eq!(Encoding::SignMagnitude.steps(0x43), -3);
}

#[test]
fn ctl_11_encoder_binding_reads_steps() {
    let mut b = binding(Kind::Cc, Some(16), "wheel 2");
    b.encoding = Some(Encoding::TwosComplement);
    assert_eq!(
        b.read(&msg(Kind::Cc, 1, Some(16), 0x7D)),
        Reading::Steps(-3.0)
    );
}

// --- CTL-12 buttons ---------------------------------------------------------

#[test]
fn ctl_12_note_cc_and_program_are_buttons() {
    let note = binding(Kind::Note, Some(41), "go");
    assert_eq!(
        note.read(&msg(Kind::Note, 1, Some(41), 100)),
        Reading::Button(true)
    );
    assert_eq!(
        note.read(&msg(Kind::Note, 1, Some(41), 0)),
        Reading::Button(false)
    );
    let cc = binding(Kind::Cc, Some(20), "go");
    assert_eq!(
        cc.read(&msg(Kind::Cc, 1, Some(20), 1)),
        Reading::Button(true)
    );
    assert_eq!(
        cc.read(&msg(Kind::Cc, 1, Some(20), 0)),
        Reading::Button(false)
    );
    let program = binding(Kind::Program, Some(5), "go");
    assert_eq!(
        program.read(&msg(Kind::Program, 1, Some(5), 127)),
        Reading::Button(true)
    );
}

#[test]
fn ctl_12_program_change_is_press_only() {
    let b = binding(Kind::Program, Some(5), "go");
    let mut s = BindingState::default();
    let r = b.read(&msg(Kind::Program, 1, Some(5), 127));
    assert_eq!(
        s.apply(&b, r, None, Resolution::Normal),
        vec![ActionValue::Press]
    );
}

#[test]
fn ctl_12_momentary_passes_press_and_release() {
    let b = binding(Kind::Note, Some(41), "upper 1");
    let mut s = BindingState::default();
    assert_eq!(
        s.apply(&b, Reading::Button(true), None, Resolution::Normal),
        vec![ActionValue::Press]
    );
    assert_eq!(
        s.apply(&b, Reading::Button(false), None, Resolution::Normal),
        vec![ActionValue::Release]
    );
}

#[test]
fn ctl_12_toggle_flips_on_each_press() {
    let mut b = binding(Kind::Note, Some(41), "dbo");
    b.button_mode = ButtonMode::Toggle;
    let mut s = BindingState::default();
    let mut run = |pressed| s.apply(&b, Reading::Button(pressed), None, Resolution::Normal);
    assert_eq!(run(true), vec![ActionValue::Press]);
    assert_eq!(run(false), vec![]);
    assert_eq!(run(true), vec![ActionValue::Release]);
    assert_eq!(run(false), vec![]);
    assert_eq!(run(true), vec![ActionValue::Press]);
}

#[test]
fn ctl_12_tap_reading_is_press_and_release() {
    let b = binding(Kind::Note, Some(41), "go");
    let mut s = BindingState::default();
    assert_eq!(
        s.apply(&b, Reading::Tap, None, Resolution::Normal),
        vec![ActionValue::Press, ActionValue::Release]
    );
}

// --- CTL-07 conversions -----------------------------------------------------

#[test]
fn ctl_07_continuous_input_on_button_action_is_a_flash_at_half() {
    let b = binding(Kind::PitchBend, None, "upper 1");
    let mut s = BindingState::default();
    let mut run = |l| s.apply(&b, Reading::Level(l), None, Resolution::Normal);
    assert_eq!(run(0.3), vec![]);
    assert_eq!(run(0.5), vec![ActionValue::Press]);
    assert_eq!(run(0.9), vec![]);
    assert_eq!(run(0.49), vec![ActionValue::Release]);
    assert_eq!(run(0.1), vec![]);
}

#[test]
fn ctl_07_button_input_on_continuous_action_uses_max_and_min() {
    let mut b = binding(Kind::Note, Some(41), "gm");
    let mut s = BindingState::default();
    assert_eq!(
        s.apply(&b, Reading::Button(true), None, Resolution::Normal),
        vec![ActionValue::Level(1.0)]
    );
    assert_eq!(
        s.apply(&b, Reading::Button(false), None, Resolution::Normal),
        vec![ActionValue::Level(0.0)]
    );
    b.max = 0.5;
    b.min = 0.1;
    assert_eq!(
        s.apply(&b, Reading::Button(true), None, Resolution::Normal),
        vec![ActionValue::Level(0.5)]
    );
    assert_eq!(
        s.apply(&b, Reading::Button(false), None, Resolution::Normal),
        vec![ActionValue::Level(0.1)]
    );
}

#[test]
fn ctl_07_absolute_input_on_wheel_sends_the_difference() {
    let b = binding(Kind::Cc, Some(7), "wheel 1");
    let mut s = BindingState::default();
    // The first value only sets the reference.
    assert_eq!(
        s.apply(&b, Reading::Level(0.2), None, Resolution::Normal),
        vec![]
    );
    // Full travel = one full turn = 255 Normal steps (the engine's wheel step):
    // steps = round(new * 255) - round(previous * 255), so nothing drifts.
    assert_eq!(
        s.apply(&b, Reading::Level(1.0), None, Resolution::Normal),
        vec![ActionValue::Steps(204)]
    );
    assert_eq!(
        s.apply(&b, Reading::Level(0.0), None, Resolution::Normal),
        vec![ActionValue::Steps(-255)]
    );
}

#[test]
fn ctl_07_relative_input_on_continuous_action_steps_and_clamps() {
    let b = binding(Kind::Cc, Some(16), "fader 1");
    let mut s = BindingState::default();
    let v = s.apply(&b, Reading::Steps(4.0), Some(0.5), Resolution::Normal);
    assert!(close(at(v), 0.5 + 4.0 / 256.0));
    let v = s.apply(&b, Reading::Steps(4.0), Some(0.5), Resolution::Fine);
    assert!(close(at(v), 0.5 + 4.0 / 1024.0));
    let v = s.apply(&b, Reading::Steps(-4.0), Some(0.5), Resolution::Coarse);
    assert!(close(at(v), 0.5 - 4.0 / 64.0));
    let v = s.apply(&b, Reading::Steps(100.0), Some(0.9), Resolution::Coarse);
    assert_eq!(at(v), 1.0);
    let v = s.apply(&b, Reading::Steps(-100.0), None, Resolution::Coarse);
    assert_eq!(at(v), 0.0);
}

/// The single level in `v`, NaN otherwise (so `close` fails).
fn at(v: Vec<ActionValue>) -> f32 {
    match v.as_slice() {
        [ActionValue::Level(l)] => *l,
        _ => f32::NAN,
    }
}

#[test]
fn ctl_07_relative_steps_on_wheel_pass_through() {
    let b = binding(Kind::Cc, Some(16), "wheel 1");
    let mut s = BindingState::default();
    assert_eq!(
        s.apply(&b, Reading::Steps(-3.0), None, Resolution::Normal),
        vec![ActionValue::Steps(-3)]
    );
}

// --- CTL-13 pickup / jump ---------------------------------------------------

#[test]
fn ctl_13_pickup_waits_until_within_2_percent() {
    let mut p = Pickup::default();
    assert_eq!(p.feed(0.2, 0.6), None);
    assert_eq!(p.feed(0.5, 0.6), None);
    assert_eq!(p.feed(0.59, 0.6), Some(0.59));
    assert_eq!(p.feed(0.3, 0.59), Some(0.3));
}

#[test]
fn ctl_13_pickup_engages_when_crossing() {
    let mut p = Pickup::default();
    assert_eq!(p.feed(0.2, 0.6), None);
    assert_eq!(p.feed(0.8, 0.6), Some(0.8));
}

#[test]
fn ctl_13_pickup_rearms_when_target_changes_under_it() {
    let mut p = Pickup::default();
    assert_eq!(p.feed(0.6, 0.6), Some(0.6));
    // Another client (or a page change) moved the level to 0.1.
    assert_eq!(p.feed(0.65, 0.1), None);
    assert_eq!(p.feed(0.11, 0.1), Some(0.11));
    p.rearm();
    assert_eq!(p.feed(0.5, 0.11), None);
}

#[test]
fn ctl_13_binding_pickup_default_and_jump() {
    let b = binding(Kind::Cc, Some(7), "fader 1");
    assert!(b.pickup);
    let mut s = BindingState::default();
    assert_eq!(
        s.apply(&b, Reading::Level(0.2), Some(0.8), Resolution::Normal),
        vec![]
    );
    let mut jump = b.clone();
    jump.pickup = false;
    let mut s = BindingState::default();
    assert_eq!(
        s.apply(&jump, Reading::Level(0.2), Some(0.8), Resolution::Normal),
        vec![ActionValue::Level(0.2)]
    );
}

#[test]
fn ctl_13_pickup_without_known_target_follows() {
    let b = binding(Kind::Cc, Some(7), "fader 1");
    let mut s = BindingState::default();
    assert_eq!(
        s.apply(&b, Reading::Level(0.2), None, Resolution::Normal),
        vec![ActionValue::Level(0.2)]
    );
}

// --- CTL-15 malformed input ------------------------------------------------

#[test]
fn ctl_15_decoder_counts_data_without_status_and_truncated_messages() {
    let mut d = Decoder::default();
    assert!(d.decode(&[0x40, 0x41]).is_empty());
    assert_eq!(
        d.malformed(),
        1,
        "data bytes without a status: one per packet"
    );
    assert!(d.decode(&[0xB0, 7]).is_empty());
    assert_eq!(d.malformed(), 2, "a message cut off at the packet's end");
    // Realtime, sysex and aftertouch are not malformed.
    d.decode(&[0xF8, 0xF0, 1, 2, 0xF7, 0xD0, 5, 0xB0, 7, 1]);
    assert_eq!(d.malformed(), 2);
}
