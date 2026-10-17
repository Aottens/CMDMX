//! QLC+ `.qxf` importer (`specs/modules/fixture-library.md` LIB-05..10, 15, 17, 18).
//!
//! The XML is read into a small serde structure ([`Def`]) and converted mode by mode. Within a
//! mode (and within each head, which becomes a cell) the channels are sorted by number,
//! classified by preset or QLC+ group, and assembled into a colour system plus attributes.
//! Nothing here panics: every problem is an [`CoreError::ImportFailed`] or a report warning.
//! [`header`] reads only the names, for the search index (LIB-13); the channel-list builder
//! (LIB-11) feeds its channels through the same conversion ([`mode_of_channels`]).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::attribute::{AttributeDef, AttributeGroup, AttributeKind};
use crate::colour::TargetColour;
use crate::profile::{Cell, ColourMixing, ColourSystem, Component, Mode, Profile, Slot, DIMMER};
use crate::CoreError;

/// A converted profile and what happened to every source channel.
#[derive(Debug, Clone, PartialEq)]
pub struct Import {
    pub profile: Profile,
    pub report: ImportReport,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ImportReport {
    /// One line per channel per mode, in channel order.
    pub channels: Vec<ReportLine>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReportLine {
    pub mode: String,
    /// 1-based DMX channel offset within the mode.
    pub channel: u16,
    pub source_name: String,
    /// Human text: what the channel became, or "kept as a plain channel".
    pub became: String,
}

// ---- the XML shape ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct Def {
    #[serde(rename = "Manufacturer", default)]
    manufacturer: String,
    #[serde(rename = "Model", default)]
    model: String,
    #[serde(rename = "Channel", default)]
    channels: Vec<Chan>,
    #[serde(rename = "Mode", default)]
    modes: Vec<ModeDef>,
    #[serde(rename = "Physical")]
    physical: Option<Physical>,
}

#[derive(Deserialize)]
pub(super) struct Chan {
    #[serde(rename = "@Name")]
    name: String,
    #[serde(rename = "@Preset")]
    preset: Option<String>,
    #[serde(rename = "@Default")]
    default: Option<String>,
    #[serde(rename = "Group")]
    group: Option<String>,
    #[serde(rename = "Colour")]
    colour: Option<String>,
    #[serde(rename = "Capability", default)]
    caps: Vec<Cap>,
    /// A default given as 0.0..=1.0 (channel lists); wins over `Default`.
    #[serde(skip)]
    unit_default: Option<f32>,
}

#[derive(Deserialize)]
struct Cap {
    #[serde(rename = "@Min", default)]
    min: u32,
    #[serde(rename = "@Max", default)]
    max: u32,
    #[serde(rename = "@Res1")]
    res1: Option<String>,
    #[serde(rename = "@Res2")]
    res2: Option<String>,
    /// The older way to give a slot colour.
    #[serde(rename = "@Color")]
    color: Option<String>,
    #[serde(rename = "$text", default)]
    text: String,
}

#[derive(Deserialize)]
struct ModeDef {
    #[serde(rename = "@Name")]
    name: String,
    #[serde(rename = "Channel", default)]
    channels: Vec<ModeChan>,
    #[serde(rename = "Head", default)]
    heads: Vec<Head>,
    #[serde(rename = "Physical")]
    physical: Option<Physical>,
}

#[derive(Deserialize)]
struct ModeChan {
    #[serde(rename = "@Number")]
    number: u16,
    #[serde(rename = "$text", default)]
    name: String,
}

#[derive(Deserialize)]
struct Head {
    #[serde(rename = "Channel", default)]
    channels: Vec<u16>,
}

#[derive(Deserialize)]
struct Physical {
    #[serde(rename = "Lens")]
    lens: Option<Lens>,
    #[serde(rename = "Focus")]
    focus: Option<Focus>,
}

#[derive(Deserialize)]
struct Lens {
    #[serde(rename = "@DegreesMin", default)]
    min: f32,
    #[serde(rename = "@DegreesMax", default)]
    max: f32,
}

#[derive(Deserialize)]
struct Focus {
    #[serde(rename = "@PanMax", default)]
    pan: f32,
    #[serde(rename = "@TiltMax", default)]
    tilt: f32,
}

// ---- entry point -----------------------------------------------------------------------------

/// Convert the text of a `.qxf` file. `source` names the file in errors and in the profile
/// (e.g. `qlcplus:Showtec/Showtec-LED-Par-56.qxf`).
pub fn import(xml: &str, source: &str) -> Result<Import, CoreError> {
    let fail = |problem: String| CoreError::ImportFailed {
        source: source.into(),
        problem,
    };
    let mut def: Def = quick_xml::de::from_str(xml).map_err(|e| fail(e.to_string()))?;
    for c in &mut def.channels {
        c.name = c.name.trim().into();
    }
    let mut report = ImportReport::default();
    let modes = def
        .modes
        .iter()
        .map(|m| convert_mode(&def, m, &mut report))
        .collect::<Result<Vec<_>, String>>()
        .map_err(fail)?;
    let profile = Profile {
        manufacturer: def.manufacturer.clone(),
        model: def.model.clone(),
        source: source.into(),
        modes,
    };
    profile.validate().map_err(|e| fail(e.to_string()))?;
    Ok(Import { profile, report })
}

/// The names of a `.qxf` file: what the search index needs (LIB-13). No conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub manufacturer: String,
    pub model: String,
    pub modes: Vec<String>,
}

pub fn header(xml: &str) -> Result<Header, CoreError> {
    #[derive(Deserialize)]
    struct Names {
        #[serde(rename = "Manufacturer", default)]
        manufacturer: String,
        #[serde(rename = "Model", default)]
        model: String,
        #[serde(rename = "Mode", default)]
        modes: Vec<ModeName>,
    }
    #[derive(Deserialize)]
    struct ModeName {
        #[serde(rename = "@Name")]
        name: String,
    }
    let n: Names = quick_xml::de::from_str(xml).map_err(|e| CoreError::ImportFailed {
        source: String::new(),
        problem: e.to_string(),
    })?;
    Ok(Header {
        manufacturer: n.manufacturer,
        model: n.model,
        modes: n.modes.into_iter().map(|m| m.name).collect(),
    })
}

// ---- mode ------------------------------------------------------------------------------------

/// A channel of a mode: its 0-based number and its definition.
struct Item<'a> {
    number: u16,
    ch: &'a Chan,
}

fn convert_mode(def: &Def, m: &ModeDef, rep: &mut ImportReport) -> Result<Mode, String> {
    let footprint = u16::try_from(m.channels.len())
        .map_err(|_| format!("mode '{}' has too many channels", m.name))?;
    let mut seen = BTreeSet::new();
    let mut items = Vec::new();
    for mc in &m.channels {
        let name = mc.name.trim();
        let ch = def
            .channels
            .iter()
            .find(|c| c.name == name)
            .ok_or_else(|| {
                format!(
                    "mode '{}' uses channel '{name}', which is not defined",
                    m.name
                )
            })?;
        if mc.number >= footprint {
            return Err(format!(
                "mode '{}': channel '{name}' has number {} but the mode has {footprint} channels",
                m.name, mc.number
            ));
        }
        if !seen.insert(mc.number) {
            return Err(format!(
                "mode '{}': channel number {} is used twice",
                m.name, mc.number
            ));
        }
        items.push(Item {
            number: mc.number,
            ch,
        });
    }
    items.sort_by_key(|i| i.number);
    for n in m.heads.iter().flat_map(|h| &h.channels) {
        if !seen.contains(n) {
            return Err(format!(
                "mode '{}': a head names channel {n}, which is not in the mode",
                m.name
            ));
        }
    }
    let physical = m.physical.as_ref().or(def.physical.as_ref());
    Ok(assemble(
        &m.name, footprint, &items, &m.heads, physical, rep,
    ))
}

/// A mode of bare channels, no heads: the channel-list builder's way in (LIB-11).
pub(super) fn mode_of_channels(name: &str, chans: &[Chan]) -> Result<Mode, String> {
    let footprint =
        u16::try_from(chans.len()).map_err(|_| format!("mode '{name}' has too many channels"))?;
    let items: Vec<Item> = (0..footprint)
        .zip(chans)
        .map(|(number, ch)| Item { number, ch })
        .collect();
    Ok(assemble(
        name,
        footprint,
        &items,
        &[],
        None,
        &mut ImportReport::default(),
    ))
}

/// A channel for [`mode_of_channels`]: a QLC+ preset (or none) and group, and a 0.0..=1.0 default.
pub(super) fn channel(name: &str, preset: Option<&str>, group: &str, default: f32) -> Chan {
    Chan {
        name: name.into(),
        preset: preset.map(Into::into),
        default: None,
        group: Some(group.into()),
        colour: None,
        caps: Vec::new(),
        unit_default: Some(default),
    }
}

/// The channels (sorted by number) become the mode's colour system, attributes and cells.
fn assemble(
    name: &str,
    footprint: u16,
    items: &[Item],
    heads: &[Head],
    physical: Option<&Physical>,
    rep: &mut ImportReport,
) -> Mode {
    // A channel in more than one head (a master dimmer in every head) belongs to the mode (LIB-18).
    let shared = |n: u16| heads.iter().filter(|h| h.channels.contains(&n)).count() > 1;
    let own_of = |head: &Head| -> Vec<&Item> {
        items
            .iter()
            .filter(|i| head.channels.contains(&i.number) && !shared(i.number))
            .collect()
    };
    let start = rep.channels.len();
    let loose: Vec<&Item> = items
        .iter()
        .filter(|i| !heads.iter().any(|h| h.channels.contains(&i.number)) || shared(i.number))
        .collect();
    let (colour, attributes) = convert_scope(&loose, "", name, rep);
    let cells: Vec<Cell> = heads
        .iter()
        .enumerate()
        .map(|(h, head)| {
            let (colour, attributes) =
                convert_scope(&own_of(head), &format!("cell {}: ", h + 1), name, rep);
            Cell { colour, attributes }
        })
        .collect();
    if let Some(lines) = rep.channels.get_mut(start..) {
        lines.sort_by_key(|l| l.channel);
    }

    let has_dimmer = attributes
        .iter()
        .chain(cells.iter().flat_map(|c| &c.attributes))
        .any(|a| a.name == DIMMER);
    let virtual_dimmer =
        (colour.is_some() || cells.iter().any(|c| c.colour.is_some())) && !has_dimmer;
    let focus = physical.and_then(|p| p.focus.as_ref());
    let degrees = |d: Option<f32>| d.filter(|d| *d > 0.0);
    Mode {
        name: name.into(),
        footprint,
        attributes,
        colour,
        virtual_dimmer,
        cells,
        pan_degrees: degrees(focus.map(|f| f.pan)),
        tilt_degrees: degrees(focus.map(|f| f.tilt)),
        zoom_degrees: physical
            .and_then(|p| p.lens.as_ref())
            .filter(|l| l.min > 0.0 || l.max > 0.0)
            .map(|l| [l.min, l.max]),
    }
}

// ---- channel roles ---------------------------------------------------------------------------

const COMPONENTS: [&str; 6] = ["Red", "Green", "Blue", "Cyan", "Magenta", "Yellow"];
const EMITTERS: [&str; 5] = ["White", "Amber", "UV", "Indigo", "Lime"];

fn comp_name(i: usize) -> &'static str {
    COMPONENTS.get(i).copied().unwrap_or("Colour")
}

enum Role {
    /// Index into [`COMPONENTS`].
    Comp(usize),
    Attr {
        name: String,
        group: AttributeGroup,
        kind: AttributeKind,
        emitter: bool,
    },
    /// The fine byte of the channel whose preset is this.
    Fine(String),
    /// Kept under its own name; the preset that had no mapping, if any.
    Plain(AttributeGroup, Option<String>),
}

/// The preset of a channel; a colour hint on an Intensity channel counts as the matching one
/// (LIB-06), and a master dimmer is a dimmer.
fn preset_of(ch: &Chan) -> Option<String> {
    if let Some(p) = &ch.preset {
        return Some(p.replace("MasterDimmer", "Dimmer"));
    }
    let hint = ch.colour.as_deref()?.trim();
    let known = COMPONENTS.contains(&hint) || EMITTERS.contains(&hint);
    (known && ch.group.as_deref().map(str::trim) == Some("Intensity"))
        .then(|| format!("Intensity{hint}"))
}

/// Attribute group from a QLC+ group name or from the start of a preset name (LIB-06).
fn group_of(s: &str) -> AttributeGroup {
    let has = |p: &str| s.starts_with(p);
    if has("Intensity") || has("Shutter") {
        AttributeGroup::I
    } else if has("Colo") {
        AttributeGroup::C
    } else if has("Gobo") {
        AttributeGroup::G
    } else if has("Prism") {
        AttributeGroup::E
    } else if has("FX") {
        AttributeGroup::Fx
    } else if has("Beam") {
        AttributeGroup::B
    } else if has("Pan") || has("Tilt") || has("Position") {
        AttributeGroup::P
    } else {
        AttributeGroup::S
    }
}

fn classify(ch: &Chan, preset: Option<&str>) -> Role {
    use AttributeGroup::{B, C, E, G, I, P, S};
    use AttributeKind::{Htp, Ltp};
    let group = ch.group.as_deref().map(str::trim);
    let Some(p) = preset else {
        return Role::Plain(group_of(group.unwrap_or("")), None);
    };
    // Not a QLC+ preset: how the channel-list builder asks for an emitter of any name (LIB-11).
    if let Some(name) = p.strip_prefix("Emitter:") {
        return Role::Attr {
            name: name.into(),
            group: C,
            kind: Ltp,
            emitter: true,
        };
    }
    if let Some(coarse) = p.strip_suffix("Fine") {
        return Role::Fine(coarse.into());
    }
    if let Some(c) = p.strip_prefix("Intensity") {
        if let Some(i) = COMPONENTS.iter().position(|n| *n == c) {
            return Role::Comp(i);
        }
        if let Some(name) = EMITTERS.iter().find(|n| **n == c) {
            return Role::Attr {
                name: (*name).into(),
                group: C,
                kind: Ltp,
                emitter: true,
            };
        }
    }
    let (name, group, kind) = match p {
        "IntensityDimmer" => ("Dimmer", I, Htp),
        "PositionPan" => ("Pan", P, Ltp),
        "PositionTilt" => ("Tilt", P, Ltp),
        "ColorWheel" => ("Colour Wheel", C, Ltp),
        "ColorMacro" => ("Colour Macro", C, Ltp),
        "ColorCTOMixer" => ("CTO", C, Ltp),
        "ColorCTCMixer" => ("CTC", C, Ltp),
        "ColorCTBMixer" => ("CTB", C, Ltp),
        "GoboWheel" => ("Gobo", G, Ltp),
        "GoboIndex" => ("Gobo Index", G, Ltp),
        _ if p.starts_with("ShutterStrobe") => ("Strobe", I, Ltp),
        _ if p.starts_with("ShutterIris") => ("Iris", B, Ltp),
        _ if p.starts_with("BeamZoom") => ("Zoom", B, Ltp),
        _ if p.starts_with("BeamFocus") => ("Focus", B, Ltp),
        _ if p.starts_with("PrismRotation") => ("Prism Rotation", E, Ltp),
        _ if p.starts_with("Speed") => ("Pan/Tilt Speed", S, Ltp),
        _ => return Role::Plain(group_of(group.unwrap_or(p)), Some(p.into())),
    };
    Role::Attr {
        name: name.into(),
        group,
        kind,
        emitter: false,
    }
}

struct Entry<'a> {
    number: u16,
    ch: &'a Chan,
    preset: Option<String>,
    role: Role,
    /// 1-based channel of this entry's fine byte.
    fine: Option<u16>,
    /// For a fine channel: the index of its coarse entry.
    fine_of: Option<usize>,
}

impl Entry<'_> {
    fn channel(&self) -> u16 {
        self.number.saturating_add(1)
    }

    /// Name, group, kind and emitter flag of the attribute this entry becomes, if any.
    fn spec(&self) -> Option<(String, AttributeGroup, AttributeKind, bool)> {
        match &self.role {
            Role::Attr {
                name,
                group,
                kind,
                emitter,
            } => Some((name.clone(), *group, *kind, *emitter)),
            Role::Comp(i) => Some((
                comp_name(*i).into(),
                AttributeGroup::C,
                AttributeKind::Ltp,
                true,
            )),
            Role::Plain(g, _) => Some((self.ch.name.clone(), *g, AttributeKind::Ltp, false)),
            Role::Fine(_) => None,
        }
    }
}

/// One mode's loose channels, or one head's: its colour system and attributes (LIB-05..08, 18).
/// `label` prefixes the report lines ("cell 2: ").
fn convert_scope(
    items: &[&Item],
    label: &str,
    mode: &str,
    rep: &mut ImportReport,
) -> (Option<ColourMixing>, Vec<AttributeDef>) {
    let mut entries: Vec<Entry> = items
        .iter()
        .map(|i| {
            let preset = preset_of(i.ch);
            Entry {
                number: i.number,
                ch: i.ch,
                role: classify(i.ch, preset.as_deref()),
                preset,
                fine: None,
                fine_of: None,
            }
        })
        .collect();

    // Fine channels attach to the coarse channel with the same preset; the rest stay plain.
    let pairs: Vec<(usize, usize)> = entries
        .iter()
        .enumerate()
        .filter_map(|(f, e)| {
            let Role::Fine(coarse) = &e.role else {
                return None;
            };
            let c = entries.iter().position(|x| {
                !matches!(x.role, Role::Fine(_)) && x.preset.as_deref() == Some(coarse)
            })?;
            Some((f, c))
        })
        .collect();
    for (f, c) in pairs {
        let channel = entries.get(f).map(Entry::channel);
        if let (Some(ce), Some(channel)) = (entries.get_mut(c), channel) {
            if ce.fine.is_none() {
                ce.fine = Some(channel);
                if let Some(fe) = entries.get_mut(f) {
                    fe.fine_of = Some(c);
                }
            }
        }
    }
    for e in &mut entries {
        if let (Role::Fine(_), None) = (&e.role, e.fine_of) {
            let p = e.preset.clone().unwrap_or_default();
            e.role = Role::Plain(group_of(&p), Some(p));
        }
    }

    // Colour system: the first complete of RGB, CMY; other components become emitters.
    let first = |i: usize| {
        entries
            .iter()
            .position(|e| matches!(e.role, Role::Comp(c) if c == i))
    };
    let comp = |i: usize| {
        first(i).and_then(|k| entries.get(k)).map(|e| Component {
            channel: e.channel(),
            fine_channel: e.fine,
        })
    };
    let system = [(ColourSystem::Rgb, 0), (ColourSystem::Cmy, 3)]
        .into_iter()
        .find_map(|(system, b)| match (comp(b), comp(b + 1), comp(b + 2)) {
            (Some(x), Some(y), Some(z)) => Some((
                b,
                ColourMixing {
                    system,
                    components: [x, y, z],
                },
            )),
            _ => None,
        });
    let in_system: Vec<usize> = system
        .iter()
        .flat_map(|(b, _)| (*b..*b + 3).filter_map(first))
        .collect();

    // Attributes, by channel; clashing names take their channel names, then ` 2`, ` 3`, …
    let mut defs: Vec<(usize, AttributeDef)> = entries
        .iter()
        .enumerate()
        .filter(|(k, _)| !in_system.contains(k))
        .filter_map(|(k, e)| {
            let (name, group, kind, emitter) = e.spec()?;
            Some((
                k,
                AttributeDef {
                    fine_channel: e.fine,
                    default: default_of(e.ch),
                    emitter,
                    slots: slots_of(e.ch),
                    ..AttributeDef::new(name, group, kind, e.channel())
                },
            ))
        })
        .collect();
    let clash: Vec<bool> = defs
        .iter()
        .map(|(_, a)| defs.iter().filter(|(_, b)| b.name == a.name).count() > 1)
        .collect();
    for ((k, d), clashing) in defs.iter_mut().zip(clash) {
        if let (true, Some(e)) = (clashing, entries.get(*k)) {
            d.name = e.ch.name.clone();
        }
    }
    let mut taken = BTreeSet::new();
    for (_, d) in &mut defs {
        let base = d.name.clone();
        let mut n = 1;
        while !taken.insert(d.name.clone()) {
            n += 1;
            d.name = format!("{base} {n}");
        }
    }

    // The report.
    let name_of = |k: usize| {
        let role = entries.get(k).map(|e| &e.role);
        defs.iter()
            .find(|(i, _)| *i == k)
            .map(|(_, d)| d.name.clone())
            .or_else(|| match role {
                Some(Role::Comp(i)) => Some(comp_name(*i).into()),
                _ => None,
            })
            .unwrap_or_default()
    };
    let system_name = match system.as_ref().map(|(b, _)| *b) {
        Some(0) => "RGB",
        _ => "CMY",
    };
    for (k, e) in entries.iter().enumerate() {
        let def = defs.iter().find(|(i, _)| *i == k).map(|(_, d)| d);
        let became = match (e.fine_of, def, &e.role) {
            (Some(c), _, _) => format!("fine channel of {}", name_of(c)),
            (_, Some(d), Role::Plain(_, unmapped)) => {
                if let Some(p) = unmapped {
                    rep.warnings.push(format!(
                        "mode '{mode}': {label}channel {} '{}': preset {p} has no mapping \
                         (unknown, or a fine channel without its coarse channel)",
                        e.channel(),
                        e.ch.name
                    ));
                }
                format!("kept as a plain channel ({:?}, {:?})", d.group, d.kind)
            }
            (_, Some(d), _) if d.emitter => format!("emitter {}", d.name),
            (_, Some(d), _) => format!("attribute {} ({:?}, {:?})", d.name, d.group, d.kind),
            _ => format!(
                "{} component of the {system_name} colour system",
                name_of(k)
            ),
        };
        rep.channels.push(ReportLine {
            mode: mode.into(),
            channel: e.channel(),
            source_name: e.ch.name.clone(),
            became: format!("{label}{became}"),
        });
    }
    (
        system.map(|(_, colour)| colour),
        defs.into_iter().map(|(_, d)| d).collect(),
    )
}

// ---- defaults, slots, colours ----------------------------------------------------------------

/// QLC+ `Default` (0..=255) as 0.0..=1.0; 0 when missing or unreadable.
fn default_of(ch: &Chan) -> f32 {
    if let Some(unit) = ch.unit_default {
        return unit;
    }
    let n = ch
        .default
        .as_deref()
        .and_then(|d| d.trim().parse::<f32>().ok());
    (n.unwrap_or(0.0) / 255.0).clamp(0.0, 1.0)
}

/// Every channel with two or more capability ranges keeps them as slots (LIB-15, LIB-18).
fn slots_of(ch: &Chan) -> Vec<Slot> {
    if ch.caps.len() < 2 {
        return Vec::new();
    }
    ch.caps
        .iter()
        .map(|c| Slot {
            name: c.text.trim().into(),
            from: c.min.min(255) as u8,
            to: c.max.min(255) as u8,
            colour: colour_of(c),
        })
        .collect()
}

/// A single `#rrggbb` in `Res1` (or the old `Color`); none when `Res2` is a colour too.
fn colour_of(c: &Cap) -> Option<TargetColour> {
    fn hex(s: &Option<String>) -> Option<&str> {
        s.as_deref().filter(|s| s.starts_with('#'))
    }
    let (res1, res2, old) = (hex(&c.res1), hex(&c.res2), hex(&c.color));
    if res2.is_some() {
        return None;
    }
    let h = res1.or(old)?;
    let byte = |a: usize| u8::from_str_radix(h.get(a..a + 2)?, 16).ok();
    // sRGB to linear, taken as nominal primaries (LIB-18).
    let linear = |v: u8| {
        let c = f64::from(v) / 255.0;
        (if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }) as f32
    };
    Some(TargetColour {
        r: linear(byte(1)?),
        g: linear(byte(3)?),
        b: linear(byte(5)?),
    })
}
