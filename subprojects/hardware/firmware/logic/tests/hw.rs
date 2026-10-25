//! Host tests for the desk logic, named after the HW criteria (HW-70).
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use cmdmx_desk_logic::boxmap;
use cmdmx_desk_logic::encoder::{twos_complement, Accumulator, Quadrature};
use cmdmx_desk_logic::fader::{adc12_to_14, Fader, DEFAULT_DEADBAND};
use cmdmx_desk_logic::keys::Debounce;
use cmdmx_desk_logic::link::Link;
use cmdmx_desk_logic::midi::{cc14_packets, cc_packet, note_packet, parse_feedback};
use cmdmx_desk_logic::palette::{colour, Rgb, DISCONNECTED};
use cmdmx_desk_logic::wiremap::{cc, note, Control};

/// A fader that has settled at `pos` (HW-18), with its boot value taken.
fn settled(pos: u16) -> Fader {
    let mut f = Fader::new(DEFAULT_DEADBAND, 4);
    for _ in 0..4 {
        f.update(pos);
    }
    f
}

// --- HW-14 deadband ---

#[test]
fn hw_14_default_deadband_is_quarter_percent_of_travel() {
    assert_eq!(DEFAULT_DEADBAND, 41); // 0.25 % of 16383
}

#[test]
fn hw_14_still_fader_sends_nothing() {
    let mut f = settled(8000);
    for raw in [8000, 8010, 7990, 8041, 7959, 8000] {
        assert_eq!(f.update(raw), None, "raw {raw}");
    }
}

#[test]
fn hw_14_move_beyond_deadband_sends_once() {
    let mut f = settled(8000);
    assert_eq!(f.update(8042), Some(8042));
    assert_eq!(f.update(8042), None);
    assert_eq!(f.update(8060), None); // within deadband of the last sent value
    assert_eq!(f.update(8084), Some(8084));
    assert_eq!(f.update(8042), Some(8042));
}

#[test]
fn hw_14_end_stops_reach_0_and_16383() {
    let mut f = settled(16330);
    assert_eq!(f.update(16345), Some(16383)); // within deadband of the top: snaps to the end
    assert_eq!(f.update(16383), None);
    let mut f = settled(50);
    assert_eq!(f.update(30), Some(0));
    assert_eq!(f.update(0), None);
}

#[test]
fn hw_14_adc_12_bit_scales_to_full_14_bit() {
    assert_eq!(adc12_to_14(0), 0);
    assert_eq!(adc12_to_14(4095), 16383);
    assert_eq!(adc12_to_14(2048), 8192);
    assert_eq!(adc12_to_14(5000), 16383); // out of range clamps, never overflows
}

// --- HW-18 boot rule ---

#[test]
fn hw_18_sends_nothing_until_settled_then_the_real_position() {
    let mut f = Fader::new(DEFAULT_DEADBAND, 4);
    assert_eq!(f.update(9000), None);
    assert_eq!(f.update(9004), None);
    assert_eq!(f.update(8996), None);
    assert_eq!(f.update(9000), Some(9000)); // mean of the settle samples
    assert_eq!(f.update(9000), None);
}

#[test]
fn hw_18_reset_settles_again() {
    let mut f = settled(9000);
    f.reset();
    for _ in 0..3 {
        assert_eq!(f.update(9000), None);
    }
    assert_eq!(f.update(9000), Some(9000));
}

// --- HW-22 encoder ---

/// One quadrature cycle with A leading B, as (a, b) levels.
const FORWARD: [(bool, bool); 4] = [(true, false), (true, true), (false, true), (false, false)];

#[test]
fn hw_22_a_leading_b_counts_up() {
    let mut q = Quadrature::new(false, false);
    let sum: i32 = FORWARD
        .iter()
        .map(|&(a, b)| i32::from(q.update(a, b)))
        .sum();
    assert_eq!(sum, 4);
}

#[test]
fn hw_22_b_leading_a_counts_down() {
    let mut q = Quadrature::new(false, false);
    let backward = [(false, true), (true, true), (true, false), (false, false)];
    let sum: i32 = backward
        .iter()
        .map(|&(a, b)| i32::from(q.update(a, b)))
        .sum();
    assert_eq!(sum, -4);
}

#[test]
fn hw_22_no_change_and_skipped_state_count_nothing() {
    let mut q = Quadrature::new(false, false);
    assert_eq!(q.update(false, false), 0);
    assert_eq!(q.update(true, true), 0); // both lines changed: invalid, ignored
}

#[test]
fn hw_22_four_counts_make_one_step() {
    let mut acc = Accumulator::new(4);
    acc.add(3);
    assert_eq!(acc.take(), None);
    acc.add(1);
    assert_eq!(acc.take(), Some(1));
    assert_eq!(acc.take(), None);
}

#[test]
fn hw_22_remainder_is_kept_across_messages() {
    let mut acc = Accumulator::new(4);
    acc.add(6);
    assert_eq!(acc.take(), Some(1));
    acc.add(2);
    assert_eq!(acc.take(), Some(1));
    acc.add(-5);
    assert_eq!(acc.take(), Some(twos_complement(-1)));
}

#[test]
fn hw_22_clamped_to_63_per_message_and_rest_carried() {
    let mut acc = Accumulator::new(1);
    acc.add(100);
    assert_eq!(acc.take(), Some(63));
    assert_eq!(acc.take(), Some(37));
    acc.add(-70);
    assert_eq!(acc.take(), Some(twos_complement(-63)));
    assert_eq!(acc.take(), Some(twos_complement(-7)));
    assert_eq!(acc.take(), None);
}

#[test]
fn hw_22_twos_complement_matches_ctl_11() {
    assert_eq!(twos_complement(1), 1);
    assert_eq!(twos_complement(63), 63);
    assert_eq!(twos_complement(-3), 0x7D);
    assert_eq!(twos_complement(-63), 65);
    assert_eq!(twos_complement(100), 63); // clamped
    assert_eq!(twos_complement(-100), 65);
}

// --- HW-20 wire map ---

#[test]
fn hw_20_fader_ccs() {
    assert_eq!(cc(Control::PlaybackFader(1)), Some(0));
    assert_eq!(cc(Control::PlaybackFader(10)), Some(9));
    assert_eq!(cc(Control::PresetFader(1)), Some(10));
    assert_eq!(cc(Control::PresetFader(10)), Some(19));
    assert_eq!(cc(Control::GrandMaster), Some(20));
    assert_eq!(cc(Control::Wheel(1)), Some(64));
    assert_eq!(cc(Control::Wheel(4)), Some(67));
    assert_eq!(cc(Control::PlaybackFader(0)), None);
    assert_eq!(cc(Control::PlaybackFader(11)), None);
    assert_eq!(cc(Control::Wheel(5)), None);
    assert_eq!(cc(Control::Upper(1)), None); // a key has no CC
}

#[test]
fn hw_20_key_notes() {
    assert_eq!(note(Control::Upper(1)), Some(0));
    assert_eq!(note(Control::Upper(10)), Some(9));
    assert_eq!(note(Control::Lower(1)), Some(10));
    assert_eq!(note(Control::Lower(10)), Some(19));
    assert_eq!(note(Control::PresetKey(1)), Some(20));
    assert_eq!(note(Control::MacroKey(10)), Some(39));
    assert_eq!(note(Control::WheelPush(1)), Some(40));
    assert_eq!(note(Control::WheelPush(4)), Some(43));
    assert_eq!(note(Control::Dbo), Some(44));
    assert_eq!(note(Control::Go), Some(45));
    assert_eq!(note(Control::Back), Some(46));
    assert_eq!(note(Control::Stop), Some(47));
    assert_eq!(note(Control::Release), Some(48));
    assert_eq!(note(Control::PagePrev), Some(49));
    assert_eq!(note(Control::PageNext), Some(50));
    assert_eq!(note(Control::Fan), Some(51));
    assert_eq!(note(Control::AttributeGroup(1)), Some(52));
    assert_eq!(note(Control::AttributeGroup(8)), Some(59));
    assert_eq!(note(Control::Keypad(0)), Some(60));
    assert_eq!(note(Control::Keypad(67)), Some(127));
    assert_eq!(note(Control::Keypad(68)), None);
    assert_eq!(note(Control::Lower(11)), None);
    assert_eq!(note(Control::GrandMaster), None); // a fader has no note
}

#[test]
fn hw_20_cc14_is_msb_then_lsb_on_channel_1() {
    // 9000 = 70 * 128 + 40
    assert_eq!(
        cc14_packets(1, 9000),
        [[0x0B, 0xB0, 1, 70], [0x0B, 0xB0, 33, 40]]
    );
    assert_eq!(
        cc14_packets(0, 16383),
        [[0x0B, 0xB0, 0, 127], [0x0B, 0xB0, 32, 127]]
    );
    assert_eq!(
        cc14_packets(0, 20000),
        [[0x0B, 0xB0, 0, 127], [0x0B, 0xB0, 32, 127]]
    ); // clamped
}

#[test]
fn hw_20_relative_cc_and_notes() {
    assert_eq!(cc_packet(64, 0x7D), [0x0B, 0xB0, 64, 0x7D]);
    assert_eq!(note_packet(10, true), [0x09, 0x90, 10, 127]);
    assert_eq!(note_packet(10, false), [0x08, 0x80, 10, 0]);
}

#[test]
fn hw_20_feedback_note_velocity() {
    assert_eq!(parse_feedback(&[0x09, 0x90, 11, 40]), Some((11, 40)));
    assert_eq!(parse_feedback(&[0x09, 0x90, 11, 0]), Some((11, 0)));
    assert_eq!(parse_feedback(&[0x08, 0x80, 11, 64]), Some((11, 0))); // note off = off
    assert_eq!(parse_feedback(&[0x09, 0x91, 11, 40]), None); // channel 2
    assert_eq!(parse_feedback(&[0x0B, 0xB0, 0, 70]), None); // fader target: no motor here
    assert_eq!(parse_feedback(&[0x00, 0x00, 0, 0]), None);
}

#[test]
fn hw_20_key_bounce_sends_one_press_and_one_release() {
    let mut k = Debounce::new(3);
    let readings = [
        true, false, true, true, true, true, false, true, false, false, false,
    ];
    let events: Vec<bool> = readings.iter().filter_map(|&r| k.update(r)).collect();
    assert_eq!(events, vec![true, false]);
}

#[test]
fn hw_20_palette_ends_are_off_and_white() {
    assert_eq!(colour(0), Rgb { r: 0, g: 0, b: 0 });
    assert_eq!(
        colour(127),
        Rgb {
            r: 255,
            g: 255,
            b: 255
        }
    );
    assert_eq!(colour(8), Rgb { r: 255, g: 0, b: 0 }); // red, wire-map.md
    assert_eq!(colour(80), Rgb { r: 0, g: 0, b: 255 }); // blue
    assert_eq!(colour(87), colour(80)); // one colour per block of 8
    assert_eq!(colour(200), colour(127)); // out of range is white, never a panic
}

// --- HW-32 disconnected pattern ---

#[test]
fn hw_32_disconnected_until_host_sent_feedback() {
    let mut l = Link::new();
    assert!(l.disconnected());
    l.feedback_received(); // not configured yet: ignored
    l.set_configured(true);
    assert!(l.disconnected());
    l.feedback_received();
    assert!(!l.disconnected());
}

#[test]
fn hw_32_host_lost_shows_pattern_again_until_new_feedback() {
    let mut l = Link::new();
    l.set_configured(true);
    l.feedback_received();
    l.set_configured(false);
    assert!(l.disconnected());
    l.set_configured(true);
    assert!(l.disconnected());
    l.feedback_received();
    assert!(!l.disconnected());
}

#[test]
fn hw_32_pattern_is_dim_red() {
    assert!(DISCONNECTED.r > 0 && DISCONNECTED.r < 64);
    assert_eq!((DISCONNECTED.g, DISCONNECTED.b), (0, 0));
}

// --- HW-61 the box's controls ---

#[test]
fn hw_61_box_uses_the_wire_map() {
    assert_eq!(boxmap::FADERS.map(cc), [Some(0), Some(1)]);
    assert_eq!(
        boxmap::KEYS.map(note),
        [Some(0), Some(1), Some(10), Some(11)]
    );
    assert_eq!(cc(boxmap::WHEEL), Some(64));
    assert_eq!(boxmap::key_for_note(10), Some(2));
    assert_eq!(boxmap::key_for_note(2), None);
}
