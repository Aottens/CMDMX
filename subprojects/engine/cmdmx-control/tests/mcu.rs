//! VS1-07: the pure part of the Mackie Control profile (CTL-01, 02, 20, 22, 31, 32, 33).
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use cmdmx_control::action::Action;
use cmdmx_control::mapping::device_matches;
use cmdmx_control::mcu::{self, Window};

#[test]
fn ctl_01_faders_read_as_10_bit_so_full_travel_is_exactly_one() {
    assert_eq!(mcu::level(0x3FFF), 1.0);
    assert_eq!(mcu::level(0x3FF0), 1.0);
    assert_eq!(mcu::level(0), 0.0);
    assert_eq!(mcu::level(512 << 4), 512.0 / 1023.0);
    // Out-of-range input is clamped, never wraps.
    assert_eq!(mcu::level(u16::MAX), 1.0);
}

#[test]
fn ctl_02_positions_round_trip_through_level() {
    assert_eq!(mcu::position(1.0), 0x3FF0);
    assert_eq!(mcu::position(0.0), 0);
    assert_eq!(mcu::position(2.0), 0x3FF0);
    for l in [0.0f32, 0.1, 0.25, 0.5, 0.999, 1.0] {
        assert!(
            (mcu::level(mcu::position(l)) - l).abs() <= 0.5 / 1023.0,
            "{l}"
        );
    }
}

#[test]
fn ctl_20_touch_notes_name_the_fader_channel() {
    assert_eq!(mcu::touch(104), Some(1));
    assert_eq!(mcu::touch(111), Some(8));
    assert_eq!(mcu::touch(112), Some(9), "the master fader");
    assert_eq!(mcu::touch(103), None);
    assert_eq!(mcu::touch(113), None);
}

#[test]
fn ctl_33_ring_shows_a_single_dot_off_without_a_wheel() {
    assert_eq!(mcu::ring(None), 0);
    assert_eq!(mcu::ring(Some(0.0)), 1);
    assert_eq!(mcu::ring(Some(0.5)), 6);
    assert_eq!(mcu::ring(Some(1.0)), 11);
    assert_eq!(mcu::ring(Some(7.0)), 11);
}

#[test]
fn ctl_31_eight_per_page_one_bcf2000() {
    let w = Window::default();
    assert_eq!(w.offset, 0);
    assert_eq!(w.slot(1, 8), Some(1));
    assert_eq!(w.slot(8, 8), Some(8));
    assert_eq!(w.slot(0, 8), None);
}

#[test]
fn ctl_31_ten_per_page_eight_faders() {
    let mut w = Window::default();
    assert_eq!(w.slot(8, 10), Some(8));
    w.channel(true, 8, 10);
    assert_eq!(w.offset, 8);
    assert_eq!(w.slot(1, 10), Some(9));
    assert_eq!(w.slot(2, 10), Some(10));
    assert_eq!(w.slot(3, 10), None, "faders 3–8 unbound");
}

#[test]
fn ctl_31_sixteen_per_page_with_an_extender_and_six_per_page() {
    let w = Window::default();
    assert_eq!(w.slot(16, 16), Some(16));
    assert_eq!(w.slot(9, 16), Some(9));
    assert_eq!(w.slot(6, 6), Some(6));
    assert_eq!(w.slot(7, 6), None);
}

#[test]
fn ctl_32_channel_moves_by_the_width_and_stops_at_both_ends() {
    let mut w = Window::default();
    w.channel(false, 8, 10);
    assert_eq!(w.offset, 0, "stops at 0");
    w.channel(true, 8, 10);
    w.channel(true, 8, 10);
    assert_eq!(w.offset, 8, "stops at the last window");
    let mut w = Window::default();
    for _ in 0..3 {
        w.channel(true, 8, 24);
    }
    assert_eq!(w.offset, 16);
    w.channel(false, 8, 24);
    assert_eq!(w.offset, 8);
    // One window covers the page: nothing moves.
    let mut w = Window::default();
    w.channel(true, 16, 16);
    w.channel(true, 8, 6);
    assert_eq!(w.offset, 0);
    let mut w = Window::default();
    w.channel(true, 8, 10);
    w.reset();
    assert_eq!(w.offset, 0);
}

#[test]
fn ctl_32_channel_actions_are_in_the_catalogue() {
    for text in ["channel next", "channel prev"] {
        let a: Action = text.parse().unwrap();
        assert_eq!(a.to_string(), text);
    }
    assert_eq!(
        "channel next".parse::<Action>().unwrap(),
        Action::ChannelNext
    );
    assert_eq!(
        "channel prev".parse::<Action>().unwrap(),
        Action::ChannelPrev
    );
    assert!("channel 3".parse::<Action>().is_err());
}

#[test]
fn ctl_22_device_patterns_separated_by_a_bar() {
    let p = "*BCF2000*|*X-Touch*|*X-TOUCH*|*iCON*";
    for name in [
        "BCF2000",
        "BCF2000 Port 1",
        "X-Touch",
        "X-Touch-Ext",
        "X-TOUCH_EXT",
        "iCON QCon Pro G2 V1.13",
    ] {
        assert!(device_matches(p, name), "{name}");
    }
    assert!(!device_matches(p, "nanoKONTROL2"));
    assert!(device_matches("nano*", "nanoKONTROL2"));
}
