//! Shared test rig: the church fixture numbers on universe 1.
//!
//! Washes 101–104 use a 5-channel "wash" profile (Dimmer, Pan 16-bit, Tilt, Zoom) at
//! addresses 1, 6, 11, 16; pars 201–208 and 221–226 and spots 301–306 are one-channel
//! dimmers from address 21 on (201 = 21, 202 = 22, …, 306 = 40).

#![allow(dead_code, clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use cmdmx_core::attribute::{AttributeDef, AttributeGroup, AttributeKind};
use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::patch::{Fixture, FixtureId};
use cmdmx_core::profile::Profile;

fn attr(name: &str, group: AttributeGroup, kind: AttributeKind, channel: u16) -> AttributeDef {
    AttributeDef::new(name, group, kind, channel)
}

pub fn wash_profile() -> Profile {
    use AttributeGroup::*;
    use AttributeKind::*;
    Profile::from_attributes(vec![
        attr("Dimmer", I, Htp, 1),
        AttributeDef {
            fine_channel: Some(3),
            ..attr("Pan", P, Ltp, 2)
        },
        attr("Tilt", P, Ltp, 4),
        attr("Zoom", B, Ltp, 5),
    ])
}

pub fn church() -> Engine {
    let mut e = Engine::new(Settings::default());
    e.add_profile("wash", wash_profile()).unwrap();
    e.add_profile(
        "dimmer",
        Profile::from_attributes(vec![attr(
            "Dimmer",
            AttributeGroup::I,
            AttributeKind::Htp,
            1,
        )]),
    )
    .unwrap();
    let mut address = 1;
    for id in 101..=104 {
        e.patch(Fixture {
            id,
            profile: "wash".into(),
            mode: None,
            universe: 1,
            address,
        })
        .unwrap();
        address += 5;
    }
    for id in (201..=208).chain(221..=226).chain(301..=306) {
        e.patch(Fixture {
            id,
            profile: "dimmer".into(),
            mode: None,
            universe: 1,
            address,
        })
        .unwrap();
        address += 1;
    }
    e
}

pub fn select(e: &mut Engine, ids: &[FixtureId]) {
    for &id in ids {
        e.select_fixture(id).unwrap();
    }
}

/// DMX value at a 1-based address of universe 1 after the last tick.
pub fn dmx(e: &Engine, address: u16) -> u8 {
    e.frame(1).and_then(|f| f.channel(address)).unwrap()
}
