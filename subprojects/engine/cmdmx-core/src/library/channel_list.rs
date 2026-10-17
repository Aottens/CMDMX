//! Profile from a channel list (`specs/modules/fixture-library.md` LIB-11): the owner names
//! what each DMX channel is, in order, and gets a profile with the same mapping rules as the
//! QLC+ importer (LIB-05..08, 17). Each [`ChannelType`] becomes the QLC+ preset the importer
//! already understands, so there is one rule set, not two. Pure: no files.

use serde::{Deserialize, Serialize};

use super::qlcplus::{channel, mode_of_channels};
use crate::attribute::AttributeGroup;
use crate::profile::Profile;
use crate::CoreError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelList {
    pub manufacturer: String,
    pub model: String,
    /// Name of the one mode the list describes.
    pub mode: String,
    /// Channel 1, 2, 3, … in order.
    pub channels: Vec<ChannelType>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ChannelType {
    Dimmer,
    #[serde(rename = "Dimmer fine")]
    DimmerFine,
    Red,
    Green,
    Blue,
    White,
    #[serde(rename = "Warm White")]
    WarmWhite,
    #[serde(rename = "Cold White")]
    ColdWhite,
    Amber,
    Lime,
    UV,
    Indigo,
    Cyan,
    Magenta,
    Yellow,
    /// Any other colour emitter, by name (LIB-17).
    Emitter(String),
    Pan,
    #[serde(rename = "Pan fine")]
    PanFine,
    Tilt,
    #[serde(rename = "Tilt fine")]
    TiltFine,
    Strobe,
    Zoom,
    Focus,
    #[serde(rename = "Colour Macro")]
    ColourMacro,
    /// A plain channel under its own name, in the given group.
    Other {
        name: String,
        group: AttributeGroup,
        /// 0.0..=1.0.
        #[serde(default)]
        default: f32,
    },
}

impl ChannelType {
    fn to_channel(&self) -> Result<super::qlcplus::Chan, String> {
        use ChannelType::*;
        let preset =
            |name: &str, preset: &str, group: &str| Ok(channel(name, Some(preset), group, 0.0));
        match self {
            Dimmer => preset("Dimmer", "IntensityDimmer", "Intensity"),
            DimmerFine => preset("Dimmer fine", "IntensityDimmerFine", "Intensity"),
            Red => preset("Red", "IntensityRed", "Intensity"),
            Green => preset("Green", "IntensityGreen", "Intensity"),
            Blue => preset("Blue", "IntensityBlue", "Intensity"),
            White => preset("White", "IntensityWhite", "Intensity"),
            Amber => preset("Amber", "IntensityAmber", "Intensity"),
            Lime => preset("Lime", "IntensityLime", "Intensity"),
            UV => preset("UV", "IntensityUV", "Intensity"),
            Indigo => preset("Indigo", "IntensityIndigo", "Intensity"),
            Cyan => preset("Cyan", "IntensityCyan", "Intensity"),
            Magenta => preset("Magenta", "IntensityMagenta", "Intensity"),
            Yellow => preset("Yellow", "IntensityYellow", "Intensity"),
            WarmWhite => preset("Warm White", "Emitter:Warm White", "Intensity"),
            ColdWhite => preset("Cold White", "Emitter:Cold White", "Intensity"),
            Emitter(name) if name.trim().is_empty() => Err("an emitter needs a name".into()),
            Emitter(name) => preset(name, &format!("Emitter:{name}"), "Intensity"),
            Pan => preset("Pan", "PositionPan", "Pan"),
            PanFine => preset("Pan fine", "PositionPanFine", "Pan"),
            Tilt => preset("Tilt", "PositionTilt", "Tilt"),
            TiltFine => preset("Tilt fine", "PositionTiltFine", "Tilt"),
            Strobe => preset("Strobe", "ShutterStrobeSlowFast", "Shutter"),
            Zoom => preset("Zoom", "BeamZoomSmallBig", "Beam"),
            Focus => preset("Focus", "BeamFocusNearFar", "Beam"),
            ColourMacro => preset("Colour Macro", "ColorMacro", "Colour"),
            Other { name, .. } if name.trim().is_empty() => Err("a channel needs a name".into()),
            Other {
                name,
                group,
                default,
            } => {
                if !(0.0..=1.0).contains(default) {
                    return Err(format!("{name}: default must be 0.0..=1.0"));
                }
                // The group name the importer maps back to `group`.
                let group = match group {
                    AttributeGroup::I => "Intensity",
                    AttributeGroup::P => "Position",
                    AttributeGroup::C => "Colour",
                    AttributeGroup::G => "Gobo",
                    AttributeGroup::B => "Beam",
                    AttributeGroup::E => "Prism",
                    AttributeGroup::S => "Maintenance",
                    AttributeGroup::Fx => "FX",
                };
                Ok(channel(name, None, group, *default))
            }
        }
    }
}

/// LIB-11: the profile of a channel list, validated (LIB-02), source `channel-list`.
pub fn build(list: &ChannelList) -> Result<Profile, CoreError> {
    let invalid = |problem: String| CoreError::InvalidProfile {
        mode: list.mode.clone(),
        problem,
    };
    if [&list.manufacturer, &list.model, &list.mode]
        .iter()
        .any(|s| s.trim().is_empty())
    {
        return Err(invalid("manufacturer, model and mode need a name".into()));
    }
    if list.channels.is_empty() {
        return Err(invalid("the list has no channels".into()));
    }
    let chans = list
        .channels
        .iter()
        .map(ChannelType::to_channel)
        .collect::<Result<Vec<_>, _>>()
        .map_err(invalid)?;
    let profile = Profile {
        manufacturer: list.manufacturer.clone(),
        model: list.model.clone(),
        source: "channel-list".into(),
        modes: vec![mode_of_channels(&list.mode, &chans).map_err(invalid)?],
    };
    profile.validate()?;
    Ok(profile)
}
