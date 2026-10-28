//! Attribute groups and kinds.
//!
//! Titan groups every attribute into eight banks, IPCGBES-FX, and uses that
//! grouping as the mask everywhere (record, clear, release, locate). The group
//! is a property of the attribute in the fixture profile, never of the UI.

use serde::{Deserialize, Serialize};

use crate::profile::Slot;

/// Titan's attribute banks, in console order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AttributeGroup {
    /// Dimmer, shutter/strobe.
    I,
    /// Pan, tilt.
    P,
    /// Colour wheel, CMY/RGB/HSI mixing, colour temperature.
    C,
    /// Gobo wheels, rotation, index.
    G,
    /// Iris, focus, zoom, frost, blades.
    B,
    /// Prism and similar effects.
    E,
    /// Motor speeds, control channels.
    S,
    /// Shapes and pixel mapper (own layer; not a DMX attribute as such).
    #[serde(rename = "FX")] // IPC-11: the wire name is "FX"
    Fx,
}

/// How values from several sources combine for this attribute.
///
/// Intensity is HTP (highest takes precedence); everything else is LTP
/// (latest takes precedence). Shutter/strobe is in group I but is LTP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AttributeKind {
    Htp,
    Ltp,
}

/// One controllable attribute of a fixture profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttributeDef {
    /// Name as shown on the wheels, e.g. "Dimmer", "Pan", "Cyan".
    pub name: String,
    pub group: AttributeGroup,
    pub kind: AttributeKind,
    /// 1-based DMX channel offset within the fixture's footprint (coarse byte).
    pub channel: u16,
    /// Optional 1-based offset of the fine (low) byte for 16-bit attributes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fine_channel: Option<u16>,
    /// What the attribute outputs until some source proposes a value (LIB-03), 0.0..=1.0.
    #[serde(default)]
    pub default: f32,
    /// Value Locate drives it to, if any (LIB-01).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locate: Option<f32>,
    /// Value Highlight drives it to, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<f32>,
    /// An extra colour emitter (White, Amber, UV, …; LIB-17). Must be group C.
    #[serde(default, skip_serializing_if = "is_false")]
    pub emitter: bool,
    /// CIE 1931 xy chromaticity of an emitter, if known (LIB-32).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cie: Option<[f32; 2]>,
    /// LIB-38: how many times brighter than its RGB equivalent the emitter is (auto-mix only).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub output: f32,
    /// Named DMX ranges: wheel slots, macros (LIB-15).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<Slot>,
}

fn one() -> f32 {
    1.0
}

fn is_one(v: &f32) -> bool {
    *v == 1.0
}

pub(crate) fn is_false(b: &bool) -> bool {
    !*b
}

impl AttributeDef {
    /// An attribute with no fine byte, default 0 and nothing optional set.
    pub fn new(
        name: impl Into<String>,
        group: AttributeGroup,
        kind: AttributeKind,
        channel: u16,
    ) -> Self {
        AttributeDef {
            name: name.into(),
            group,
            kind,
            channel,
            fine_channel: None,
            default: 0.0,
            locate: None,
            highlight: None,
            emitter: false,
            cie: None,
            output: 1.0,
            slots: Vec::new(),
        }
    }

    /// Convert a normalised value (0.0..=1.0) to the coarse and optional fine
    /// DMX bytes. 8-bit attributes round to the nearest step; 16-bit attributes
    /// use the full 0..=65535 range.
    pub fn to_dmx(&self, value: f32) -> (u8, Option<u8>) {
        split_dmx(value, self.fine_channel.is_some())
    }
}

/// [`AttributeDef::to_dmx`] for anything with an optional fine byte (colour components too).
pub(crate) fn split_dmx(value: f32, sixteen_bit: bool) -> (u8, Option<u8>) {
    let v = value.clamp(0.0, 1.0);
    if sixteen_bit {
        let wide = (v * 65535.0).round() as u16;
        ((wide >> 8) as u8, Some((wide & 0xFF) as u8))
    } else {
        ((v * 255.0).round() as u8, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dimmer(fine: Option<u16>) -> AttributeDef {
        AttributeDef {
            fine_channel: fine,
            ..AttributeDef::new("Dimmer", AttributeGroup::I, AttributeKind::Htp, 1)
        }
    }

    #[test]
    fn eight_bit_rounds_to_nearest_step() {
        assert_eq!(dimmer(None).to_dmx(0.6), (153, None));
        assert_eq!(dimmer(None).to_dmx(1.0), (255, None));
        assert_eq!(dimmer(None).to_dmx(0.0), (0, None));
    }

    #[test]
    fn sixteen_bit_splits_coarse_and_fine() {
        assert_eq!(dimmer(Some(2)).to_dmx(1.0), (255, Some(255)));
        // 0.5 * 65535 = 32767.5 → rounds away from zero → 0x8000
        assert_eq!(dimmer(Some(2)).to_dmx(0.5), (0x80, Some(0x00)));
        // 1/65535 is the smallest 16-bit step
        assert_eq!(dimmer(Some(2)).to_dmx(1.0 / 65535.0), (0x00, Some(0x01)));
    }

    #[test]
    fn values_are_clamped() {
        assert_eq!(dimmer(None).to_dmx(1.7), (255, None));
        assert_eq!(dimmer(None).to_dmx(-3.0), (0, None));
    }
}
