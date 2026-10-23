//! Own OSC 1.0 codec and the built-in `/cmdmx` schema
//! (specs/modules/control-surfaces.md CTL-34, 36).
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use cmdmx_control::action::Action;
use cmdmx_control::convert::Reading;
use cmdmx_control::osc::{self, OscArg, OscMessage};

fn message(address: &str, args: Vec<OscArg>) -> OscMessage {
    OscMessage {
        address: address.into(),
        args,
    }
}

// --- CTL-34 codec ---------------------------------------------------------------

/// `/cmdmx/gm ,f 0.5` byte for byte (OSC 1.0: strings NUL-padded to 4, big endian).
const GM_HALF: [u8; 20] = [
    b'/', b'c', b'm', b'd', b'm', b'x', b'/', b'g', b'm', 0, 0, 0, // address
    b',', b'f', 0, 0, // type tags
    0x3F, 0x00, 0x00, 0x00, // 0.5f
];

#[test]
fn ctl_34_encodes_a_message_byte_exact() {
    assert_eq!(
        osc::encode(&message("/cmdmx/gm", vec![OscArg::Float(0.5)])),
        GM_HALF.to_vec()
    );
}

#[test]
fn ctl_34_decodes_a_message_byte_exact() {
    assert_eq!(
        osc::decode(&GM_HALF).unwrap(),
        vec![message("/cmdmx/gm", vec![OscArg::Float(0.5)])]
    );
}

#[test]
fn ctl_34_address_length_multiple_of_4_gets_four_nuls() {
    // "/abc" is 4 bytes, so 4 NULs follow; no type tag arguments.
    let bytes = osc::encode(&message("/abc", vec![]));
    assert_eq!(bytes, b"/abc\0\0\0\0,\0\0\0".to_vec());
    assert_eq!(osc::decode(&bytes).unwrap(), vec![message("/abc", vec![])]);
}

#[test]
fn ctl_34_every_argument_type_round_trips() {
    let m = message(
        "/t",
        vec![
            OscArg::Int(-7),
            OscArg::Float(1.25),
            OscArg::Double(-2.5),
            OscArg::Long(1 << 40),
            OscArg::True,
            OscArg::False,
            OscArg::Str("Go".into()),
            OscArg::Str("four".into()),
        ],
    );
    let bytes = osc::encode(&m);
    assert_eq!(bytes.len() % 4, 0);
    assert_eq!(osc::decode(&bytes).unwrap(), vec![m]);
}

#[test]
fn ctl_34_int_bytes_are_big_endian() {
    let bytes = osc::encode(&message("/i", vec![OscArg::Int(258)]));
    assert_eq!(bytes, b"/i\0\0,i\0\0\x00\x00\x01\x02".to_vec());
}

#[test]
fn ctl_34_bundle_elements_in_order_time_tag_ignored() {
    let a = osc::encode(&message("/a", vec![OscArg::Int(1)]));
    let b = osc::encode(&message("/b", vec![]));
    let mut inner = b"#bundle\0".to_vec();
    inner.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
    inner.extend_from_slice(&(b.len() as u32).to_be_bytes());
    inner.extend_from_slice(&b);
    let mut packet = b"#bundle\0".to_vec();
    packet.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF, 0, 0, 0, 0]); // any time tag
    packet.extend_from_slice(&(a.len() as u32).to_be_bytes());
    packet.extend_from_slice(&a);
    packet.extend_from_slice(&(inner.len() as u32).to_be_bytes());
    packet.extend_from_slice(&inner);
    assert_eq!(
        osc::decode(&packet).unwrap(),
        vec![message("/a", vec![OscArg::Int(1)]), message("/b", vec![])]
    );
}

#[test]
fn ctl_34_malformed_packets_are_errors_never_panics() {
    let mut bad: Vec<Vec<u8>> = vec![
        vec![],
        b"/a".to_vec(),                                  // no NUL
        b"nope\0\0\0\0,\0\0\0".to_vec(),                 // address without '/'
        b"/a\0\0,f\0\0\x00\x00".to_vec(),                // truncated float
        b"/a\0\0,x\0\0".to_vec(),                        // unknown type tag
        b"/a\0\0,s\0\0abc".to_vec(),                     // string without NUL
        b"#bundle\0\0\0\0\0\0\0\0\0\0\0\0\xFF".to_vec(), // element longer than packet
    ];
    // Every prefix of a good packet is also malformed.
    for n in 1..GM_HALF.len() {
        bad.push(GM_HALF[..n].to_vec());
    }
    for b in bad {
        assert!(osc::decode(&b).is_err(), "{b:?}");
    }
}

#[test]
fn ctl_34_numbers_accepted_as_any_numeric_type() {
    for a in [
        OscArg::Int(1),
        OscArg::Float(1.0),
        OscArg::Double(1.0),
        OscArg::Long(1),
        OscArg::True,
    ] {
        assert_eq!(a.as_f64(), Some(1.0));
    }
    assert_eq!(OscArg::False.as_f64(), Some(0.0));
    assert_eq!(OscArg::Str("1".into()).as_f64(), None);
    // NaN and infinities never reach a level or a wheel.
    assert_eq!(OscArg::Float(f32::NAN).as_f64(), None);
    assert_eq!(OscArg::Double(f64::INFINITY).as_f64(), None);
    assert_eq!(schema("/cmdmx/gm", vec![OscArg::Float(f32::NAN)]), None);
}

// --- CTL-36 /cmdmx schema -------------------------------------------------------

fn schema(address: &str, args: Vec<OscArg>) -> Option<(Action, Reading)> {
    osc::schema(&message(address, args))
}

fn act(s: &str) -> Action {
    s.parse().unwrap()
}

#[test]
fn ctl_36_faders_and_levels() {
    assert_eq!(
        schema("/cmdmx/fader/3", vec![OscArg::Float(0.25)]),
        Some((act("fader 3"), Reading::Level(0.25)))
    );
    assert_eq!(
        schema("/cmdmx/fader/2/3", vec![OscArg::Float(1.0)]),
        Some((act("fader 2.3"), Reading::Level(1.0)))
    );
    assert_eq!(
        schema("/cmdmx/playback/12/level", vec![OscArg::Double(0.5)]),
        Some((act("playback 12 level"), Reading::Level(0.5)))
    );
    assert_eq!(
        schema("/cmdmx/gm", vec![OscArg::Int(1)]),
        Some((act("gm"), Reading::Level(1.0)))
    );
}

#[test]
fn ctl_36_buttons_nonzero_press_zero_release_none_tap() {
    assert_eq!(
        schema("/cmdmx/fader/3/upper", vec![OscArg::Float(1.0)]),
        Some((act("upper 3"), Reading::Button(true)))
    );
    assert_eq!(
        schema("/cmdmx/fader/3/lower", vec![OscArg::Float(0.0)]),
        Some((act("lower 3"), Reading::Button(false)))
    );
    assert_eq!(schema("/cmdmx/go", vec![]), Some((act("go"), Reading::Tap)));
    assert_eq!(
        schema("/cmdmx/back", vec![OscArg::Int(1)]),
        Some((act("back"), Reading::Button(true)))
    );
    assert_eq!(
        schema("/cmdmx/playback/12/go", vec![]),
        Some((act("go playback 12"), Reading::Tap))
    );
    assert_eq!(
        schema("/cmdmx/playback/12/release", vec![]),
        Some((act("release playback 12"), Reading::Tap))
    );
    assert_eq!(
        schema("/cmdmx/dbo", vec![OscArg::True]),
        Some((act("dbo"), Reading::Button(true)))
    );
    assert_eq!(
        schema("/cmdmx/key/clear", vec![]),
        Some((act("key clear"), Reading::Tap))
    );
    assert_eq!(
        schema("/cmdmx/keypad/Thro", vec![OscArg::Float(1.0)]),
        Some((act("keypad Thro"), Reading::Button(true)))
    );
    assert_eq!(
        schema("/cmdmx/attribute_group/C", vec![]),
        Some((act("attribute_group C"), Reading::Tap))
    );
    assert_eq!(
        schema("/cmdmx/group/4", vec![]),
        Some((act("group 4"), Reading::Tap))
    );
    assert_eq!(
        schema("/cmdmx/palette/9", vec![]),
        Some((act("palette 9"), Reading::Tap))
    );
}

#[test]
fn ctl_36_pages() {
    assert_eq!(
        schema("/cmdmx/page", vec![OscArg::Int(3)]),
        Some((act("page 3"), Reading::Tap))
    );
    assert_eq!(
        schema("/cmdmx/page", vec![OscArg::Float(2.0)]),
        Some((act("page 2"), Reading::Tap))
    );
    assert_eq!(
        schema("/cmdmx/page/next", vec![]),
        Some((act("page next"), Reading::Tap))
    );
    assert_eq!(
        schema("/cmdmx/page/prev", vec![OscArg::Int(1)]),
        Some((act("page prev"), Reading::Button(true)))
    );
}

#[test]
fn ctl_36_wheel_steps() {
    assert_eq!(
        schema("/cmdmx/wheel/2", vec![OscArg::Int(-3)]),
        Some((act("wheel 2"), Reading::Steps(-3.0)))
    );
    assert_eq!(
        schema("/cmdmx/wheel/1", vec![OscArg::Float(0.5)]),
        Some((act("wheel 1"), Reading::Steps(0.5)))
    );
}

#[test]
fn ctl_36_unknown_or_ill_typed_is_none() {
    for (a, args) in [
        ("/cmdmx/nope", vec![]),
        ("/cmdmx/fader/x", vec![OscArg::Float(0.5)]),
        ("/cmdmx/fader/3", vec![]), // level needs a value
        ("/cmdmx/fader/3", vec![OscArg::Str("a".into())]),
        ("/cmdmx/page", vec![]), // page needs a number
        ("/cmdmx/page", vec![OscArg::Int(0)]),
        ("/cmdmx/key/nuke", vec![]),
        ("/other/gm", vec![OscArg::Float(1.0)]),
        ("/cmdmx/wheel/2", vec![]),
    ] {
        assert_eq!(schema(a, args.clone()), None, "{a} {args:?}");
    }
}
