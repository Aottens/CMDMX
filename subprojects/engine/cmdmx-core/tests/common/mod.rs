//! Shared test rig: the church fixture numbers on a one-channel dimmer profile.

#![allow(dead_code, clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use cmdmx_core::attribute::{AttributeDef, AttributeGroup, AttributeKind};
use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::patch::{Fixture, FixtureId};
use cmdmx_core::profile::Profile;

pub fn church() -> Engine {
    let mut e = Engine::new(Settings::default());
    e.add_profile(
        "dimmer",
        Profile {
            attributes: vec![AttributeDef {
                name: "Dimmer".into(),
                group: AttributeGroup::I,
                kind: AttributeKind::Htp,
                channel: 1,
                fine_channel: None,
            }],
        },
    );
    let ids = (101..=104)
        .chain(201..=208)
        .chain(221..=226)
        .chain(301..=306);
    for (n, id) in ids.enumerate() {
        e.patch(Fixture {
            id,
            profile: "dimmer".into(),
            universe: 1,
            address: n as u16 + 1,
        })
        .unwrap();
    }
    e
}

pub fn select(e: &mut Engine, ids: &[FixtureId]) {
    for &id in ids {
        e.select_fixture(id).unwrap();
    }
}
