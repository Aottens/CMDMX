//! Profile from a channel list (VS2-11): `specs/modules/fixture-library.md` LIB-11 (and LIB-17).

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use cmdmx_core::attribute::{AttributeGroup, AttributeKind};
use cmdmx_core::library::channel_list::{build, ChannelList, ChannelType};
use cmdmx_core::profile::ColourSystem;
use cmdmx_core::CoreError;

fn list(channels: Vec<ChannelType>) -> ChannelList {
    ChannelList {
        manufacturer: "U'King".into(),
        model: "RGBL Par".into(),
        mode: "4 Channel".into(),
        channels,
    }
}

#[test]
fn lib_11_uking_rgbl_from_four_channel_types() {
    use ChannelType::*;
    let p = build(&list(vec![Red, Green, Blue, Lime])).unwrap();
    assert_eq!(
        (p.manufacturer.as_str(), p.model.as_str()),
        ("U'King", "RGBL Par")
    );
    assert_eq!(p.source, "channel-list");
    let m = p.mode(None).unwrap();
    assert_eq!((m.name.as_str(), m.footprint), ("4 Channel", 4));
    assert!(m.virtual_dimmer);
    let colour = m.colour.as_ref().unwrap();
    assert_eq!(colour.system, ColourSystem::Rgb);
    let rgb: Vec<u16> = colour.components.iter().map(|c| c.channel).collect();
    assert_eq!(rgb, [1, 2, 3]);
    let lime = m.attribute("Lime").unwrap();
    assert_eq!(
        (lime.channel, lime.group, lime.emitter),
        (4, AttributeGroup::C, true)
    );
}

#[test]
fn lib_11_every_channel_type_maps_like_the_importer() {
    use ChannelType::*;
    let p = build(&list(vec![
        Dimmer,
        DimmerFine,
        Red,
        Green,
        Blue,
        WarmWhite,
        ColdWhite,
        Amber,
        Indigo,
        UV,
        Cyan,
        Emitter("Royal Blue".into()),
        Pan,
        PanFine,
        Tilt,
        TiltFine,
        Strobe,
        Zoom,
        Focus,
        ColourMacro,
        Other {
            name: "Mode".into(),
            group: AttributeGroup::S,
            default: 0.5,
        },
    ]))
    .unwrap();
    let m = p.mode(None).unwrap();
    assert_eq!(m.footprint, 21);
    assert!(!m.virtual_dimmer);
    assert_eq!(m.colour.as_ref().unwrap().system, ColourSystem::Rgb);
    let emitters: Vec<(&str, u16)> = m.emitters().map(|a| (a.name.as_str(), a.channel)).collect();
    assert_eq!(
        emitters,
        [
            ("Warm White", 6),
            ("Cold White", 7),
            ("Amber", 8),
            ("Indigo", 9),
            ("UV", 10),
            ("Cyan", 11),
            ("Royal Blue", 12)
        ],
        "Cyan next to RGB is an emitter (LIB-17)"
    );
    let a = |n: &str| m.attribute(n).unwrap_or_else(|| panic!("{n}"));
    assert_eq!(
        (
            a("Dimmer").channel,
            a("Dimmer").fine_channel,
            a("Dimmer").kind
        ),
        (1, Some(2), AttributeKind::Htp)
    );
    assert_eq!((a("Pan").channel, a("Pan").fine_channel), (13, Some(14)));
    assert_eq!((a("Tilt").channel, a("Tilt").fine_channel), (15, Some(16)));
    assert_eq!(
        (a("Strobe").group, a("Strobe").kind),
        (AttributeGroup::I, AttributeKind::Ltp)
    );
    assert_eq!(a("Zoom").group, AttributeGroup::B);
    assert_eq!(a("Focus").group, AttributeGroup::B);
    assert_eq!(a("Colour Macro").group, AttributeGroup::C);
    assert_eq!(
        (a("Mode").group, a("Mode").default, a("Mode").channel),
        (AttributeGroup::S, 0.5, 21)
    );
    assert_eq!(p.validate(), Ok(()));
}

#[test]
fn lib_11_cmy_without_rgb_is_a_cmy_system() {
    use ChannelType::*;
    let p = build(&list(vec![Dimmer, Cyan, Magenta, Yellow])).unwrap();
    let colour = p.mode(None).unwrap().colour.clone().unwrap();
    assert_eq!(colour.system, ColourSystem::Cmy);
}

#[test]
fn lib_11_channel_list_json_uses_the_spec_names() {
    let json = r#"{ "manufacturer": "U'King", "model": "RGBL Par", "mode": "4 Channel",
        "channels": ["Red", "Green", "Blue", "Warm White", "Dimmer fine", "Pan fine", "Colour Macro",
                     {"Emitter": "Royal Blue"}, {"Other": {"name": "Mode", "group": "S", "default": 0.0}}] }"#;
    let l: ChannelList = serde_json::from_str(json).unwrap();
    assert_eq!(l.channels[3], ChannelType::WarmWhite);
    assert_eq!(l.channels[4], ChannelType::DimmerFine);
    assert_eq!(l.channels[5], ChannelType::PanFine);
    assert_eq!(l.channels[6], ChannelType::ColourMacro);
    assert_eq!(l.channels[7], ChannelType::Emitter("Royal Blue".into()));
    // `default` may be left out.
    let other: ChannelType =
        serde_json::from_str(r#"{"Other": {"name": "Mode", "group": "S"}}"#).unwrap();
    assert!(matches!(other, ChannelType::Other { default, .. } if default == 0.0));
}

#[test]
fn lib_11_bad_lists_are_refused() {
    use ChannelType::*;
    assert!(build(&list(vec![])).is_err(), "no channels");
    assert!(matches!(
        build(&list(vec![Other {
            name: "Mode".into(),
            group: AttributeGroup::S,
            default: 1.5
        }])),
        Err(CoreError::InvalidProfile { .. })
    ));
    assert!(
        build(&ChannelList {
            manufacturer: " ".into(),
            ..list(vec![Red])
        })
        .is_err(),
        "blank manufacturer"
    );
    assert!(
        build(&ChannelList {
            model: String::new(),
            ..list(vec![Red])
        })
        .is_err(),
        "blank model"
    );
}
