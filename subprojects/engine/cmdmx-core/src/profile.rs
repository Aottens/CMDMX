//! Fixture profiles.
//!
//! Phase 1 vertical slice uses this minimal JSON profile. The real library is
//! GDTF plus an overlay (ADR-004, `specs/modules/fixture-library.md`); this
//! struct is what GDTF import will produce, so the engine never sees GDTF.

use serde::{Deserialize, Serialize};

use crate::attribute::AttributeDef;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub attributes: Vec<AttributeDef>,
}

impl Profile {
    /// Number of DMX channels this profile occupies (highest channel offset used).
    pub fn footprint(&self) -> u16 {
        self.attributes
            .iter()
            .map(|a| a.fine_channel.unwrap_or(0).max(a.channel))
            .max()
            .unwrap_or(0)
    }

    pub fn attribute(&self, name: &str) -> Option<&AttributeDef> {
        self.attributes.iter().find(|a| a.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribute::{AttributeGroup, AttributeKind};

    #[test]
    fn footprint_counts_fine_channels() {
        let p = Profile {
            attributes: vec![
                AttributeDef {
                    name: "Pan".into(),
                    group: AttributeGroup::P,
                    kind: AttributeKind::Ltp,
                    channel: 1,
                    fine_channel: Some(2),
                },
                AttributeDef {
                    name: "Dimmer".into(),
                    group: AttributeGroup::I,
                    kind: AttributeKind::Htp,
                    channel: 3,
                    fine_channel: None,
                },
            ],
        };
        assert_eq!(p.footprint(), 3);
    }
}
