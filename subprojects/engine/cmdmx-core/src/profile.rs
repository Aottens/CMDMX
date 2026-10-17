//! Fixture profiles (own model, ADR-006; `specs/modules/fixture-library.md` LIB-01..03, 16, 17).
//!
//! A profile is one fixture type with one or more modes; a mode is one channel layout.
//! Importers (QLC+, channel list) produce this model, so the engine never sees a foreign
//! format. A fixture is patched in one mode; everything the engine reads comes from it.
//! Cells and the colour mixing system are model-only here: the engine does not output them yet.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::attribute::{AttributeDef, AttributeGroup};
use crate::CoreError;

/// Name of the mode of a profile built from a bare attribute list.
const DEFAULT_MODE: &str = "default";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "ProfileRepr")]
pub struct Profile {
    pub manufacturer: String,
    pub model: String,
    /// Where it came from, e.g. `qlcplus:Shehds/….qxf` or `channel-list`.
    pub source: String,
    pub modes: Vec<Mode>,
}

/// What is accepted on disk: the full shape, or the legacy `{ "attributes": [...] }`
/// (every scenario file) which becomes a single mode named "default". One struct rather
/// than an untagged enum, so a typo gets serde's precise message (LIB-02).
#[derive(Deserialize)]
struct ProfileRepr {
    #[serde(default)]
    manufacturer: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    source: String,
    modes: Option<Vec<Mode>>,
    #[serde(default)]
    attributes: Vec<AttributeDef>,
}

impl From<ProfileRepr> for Profile {
    fn from(repr: ProfileRepr) -> Self {
        match repr.modes {
            Some(modes) => Profile {
                manufacturer: repr.manufacturer,
                model: repr.model,
                source: repr.source,
                modes,
            },
            None => Profile::from_attributes(repr.attributes),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mode {
    pub name: String,
    /// DMX channels this mode occupies.
    pub footprint: u16,
    #[serde(default)]
    pub attributes: Vec<AttributeDef>,
    /// Colour mixing channels of the whole fixture (LIB-04..06 use them later).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<ColourMixing>,
    /// True when the mode mixes colour but has no dimmer channel (LIB-04).
    #[serde(default, skip_serializing_if = "crate::attribute::is_false")]
    pub virtual_dimmer: bool,
    /// Heads of a multi-head mode (LIB-07).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cells: Vec<Cell>,
    /// Physical ranges (LIB-16).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pan_degrees: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tilt_degrees: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zoom_degrees: Option<[f32; 2]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<ColourMixing>,
    #[serde(default)]
    pub attributes: Vec<AttributeDef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColourMixing {
    pub system: ColourSystem,
    /// Red/green/blue or cyan/magenta/yellow, in that order.
    pub components: [Component; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColourSystem {
    #[serde(rename = "RGB")]
    Rgb,
    #[serde(rename = "CMY")]
    Cmy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Component {
    /// 1-based DMX channel offset within the mode's footprint.
    pub channel: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fine_channel: Option<u16>,
}

/// A colour a wheel slot or macro shows, as linear 0.0..=1.0 RGB (ADR-005).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TargetColour {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

/// A named DMX value range of an attribute (a wheel slot or a macro).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Slot {
    pub name: String,
    /// Raw DMX range, inclusive.
    pub from: u8,
    pub to: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<TargetColour>,
}

impl Profile {
    /// A single-mode profile from a bare attribute list (the scenario shape).
    pub fn from_attributes(attributes: Vec<AttributeDef>) -> Profile {
        let footprint = attributes
            .iter()
            .map(|a| a.fine_channel.unwrap_or(0).max(a.channel))
            .max()
            .unwrap_or(0);
        Profile {
            manufacturer: String::new(),
            model: String::new(),
            source: String::new(),
            modes: vec![Mode {
                name: DEFAULT_MODE.into(),
                footprint,
                attributes,
                colour: None,
                virtual_dimmer: false,
                cells: Vec::new(),
                pan_degrees: None,
                tilt_degrees: None,
                zoom_degrees: None,
            }],
        }
    }

    /// Parse and validate (LIB-02). Malformed JSON is `InvalidProfile` with mode "".
    pub fn from_json(json: &str) -> Result<Profile, CoreError> {
        let profile: Profile =
            serde_json::from_str(json).map_err(|e| CoreError::InvalidProfile {
                mode: String::new(),
                problem: e.to_string(),
            })?;
        profile.validate()?;
        Ok(profile)
    }

    /// Pretty JSON, keys in declaration order, so the output is stable (LIB-01).
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// The named mode, or the first one when `name` is `None`.
    pub fn mode(&self, name: Option<&str>) -> Option<&Mode> {
        match name {
            None => self.modes.first(),
            Some(n) => self.modes.iter().find(|m| m.name == n),
        }
    }

    /// LIB-02 / LIB-17: every mode must be internally consistent.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.modes.is_empty() {
            return Err(CoreError::InvalidProfile {
                mode: String::new(),
                problem: "profile has no modes".into(),
            });
        }
        for mode in &self.modes {
            mode.check().map_err(|problem| CoreError::InvalidProfile {
                mode: mode.name.clone(),
                problem,
            })?;
        }
        Ok(())
    }
}

impl Mode {
    pub fn attribute(&self, name: &str) -> Option<&AttributeDef> {
        self.attributes.iter().find(|a| a.name == name)
    }

    /// Extra emitters (LIB-17): found by flag, never by name, in profile order.
    pub fn emitters(&self) -> impl Iterator<Item = &AttributeDef> {
        self.attributes.iter().filter(|a| a.emitter)
    }

    /// The first problem found, naming the offending attribute or channel.
    fn check(&self) -> Result<(), String> {
        let mut claimed: BTreeSet<u16> = BTreeSet::new();
        let mut claim = |what: &str, channel: u16| {
            if channel == 0 || channel > self.footprint {
                Err(format!(
                    "{what}: channel {channel} is outside the footprint 1..={}",
                    self.footprint
                ))
            } else if !claimed.insert(channel) {
                Err(format!("{what}: channel {channel} is already used"))
            } else {
                Ok(())
            }
        };
        check_attributes(&self.attributes, &mut claim)?;
        if let Some(colour) = &self.colour {
            check_colour(colour, &mut claim)?;
        }
        for cell in &self.cells {
            if let Some(colour) = &cell.colour {
                check_colour(colour, &mut claim)?;
            }
            check_attributes(&cell.attributes, &mut claim)?;
        }
        Ok(())
    }
}

type Claim<'a> = dyn FnMut(&str, u16) -> Result<(), String> + 'a;

fn check_colour(colour: &ColourMixing, claim: &mut Claim) -> Result<(), String> {
    for c in &colour.components {
        claim("colour component", c.channel)?;
        if let Some(fine) = c.fine_channel {
            claim("colour component fine channel", fine)?;
        }
    }
    Ok(())
}

/// Channels, unit-range values and emitter rules for one attribute list (mode or cell).
fn check_attributes(attributes: &[AttributeDef], claim: &mut Claim) -> Result<(), String> {
    let mut emitters: BTreeSet<&str> = BTreeSet::new();
    for a in attributes {
        let name = &a.name;
        claim(name, a.channel)?;
        if let Some(fine) = a.fine_channel {
            claim(&format!("{name} fine channel"), fine)?;
        }
        let unit = |v: f32| (0.0..=1.0).contains(&v);
        if !unit(a.default)
            || a.locate.is_some_and(|v| !unit(v))
            || a.highlight.is_some_and(|v| !unit(v))
        {
            return Err(format!(
                "{name}: default, locate and highlight must be 0.0..=1.0"
            ));
        }
        if a.emitter {
            if a.group != AttributeGroup::C {
                return Err(format!("{name}: an emitter must be in group C"));
            }
            if !emitters.insert(name) {
                return Err(format!("{name}: two emitters share this name"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribute::{AttributeGroup, AttributeKind};

    #[test]
    fn footprint_counts_fine_channels() {
        let p = Profile::from_attributes(vec![
            AttributeDef {
                fine_channel: Some(2),
                ..AttributeDef::new("Pan", AttributeGroup::P, AttributeKind::Ltp, 1)
            },
            AttributeDef::new("Dimmer", AttributeGroup::I, AttributeKind::Htp, 3),
        ]);
        assert_eq!(p.modes.first().map(|m| m.footprint), Some(3));
    }
}
