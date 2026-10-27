//! GDTF importer (`specs/modules/fixture-library.md` LIB-28..33).
//!
//! A `.gdtf` file is a zip with a `description.xml`; the `gdtf` crate reads it. The first fixture
//! type becomes the profile (LIB-28), every DMX mode a mode. Each DMX channel is classified by
//! its logical channel's attribute (LIB-29) and fed to the QLC+ importer's scope builder
//! ([`build_scope`]), which assembles the colour system, emitters, clash names and report, so
//! both formats follow LIB-05..08 and LIB-17/18 alike. Fine bytes, defaults, slots and physical
//! ranges are LIB-30, cells from geometry LIB-31, emitter CIE colours LIB-32, failures LIB-33.
//! Nothing here panics: the crate's panicking helpers (`name()` of channels) are not used.

use std::io::Cursor;

use ::gdtf::dmx_mode::{ChannelFunction, DmxBreak, DmxChannel, DmxMode, LogicalChannel};
use ::gdtf::fixture_type::FixtureType;
use ::gdtf::geometry::{AnyGeometry, Geometry};
use ::gdtf::physical_descriptions::EmitterOptic;
use ::gdtf::values::DmxValue;
use ::gdtf::wheel::WheelSlotOptic;
use ::gdtf::GdtfFile;

use super::qlcplus::{build_scope, mode_of_scopes, Entry, Role};
pub use super::qlcplus::{Import, ImportReport, ReportLine};
use crate::attribute::{AttributeGroup, AttributeKind};
use crate::colour::TargetColour;
use crate::profile::{Cell, Mode, Profile, Slot};
use crate::CoreError;

/// Import the bytes of a `.gdtf` archive. `source` names the file in errors and in the profile
/// (e.g. `gdtf:Generic@RGBW8@test.gdtf`).
pub fn import(bytes: &[u8], source: &str) -> Result<Import, CoreError> {
    let fail = |problem: String| CoreError::ImportFailed {
        source: source.into(),
        problem,
    };
    let file = GdtfFile::new(Cursor::new(bytes.to_vec())).map_err(|e| fail(e.to_string()))?;
    let ft = (file.description.fixture_types.first())
        .ok_or_else(|| fail("the file has no fixture type".into()))?;
    let mut report = ImportReport::default();
    let modes: Vec<Mode> = (ft.dmx_modes.iter())
        .filter_map(|m| convert_mode(ft, m, &mut report))
        .collect();
    if modes.is_empty() {
        return Err(fail("no DMX mode could be imported".into()));
    }
    let model = match ft.long_name.trim() {
        "" => ft.name.as_deref().unwrap_or_default().into(),
        long => long.into(),
    };
    let profile = Profile {
        manufacturer: ft.manufacturer.clone(),
        model,
        source: source.into(),
        modes,
    };
    profile.validate().map_err(|e| fail(e.to_string()))?;
    Ok(Import { profile, report })
}

// ---- mode ------------------------------------------------------------------------------------

/// A DMX channel (or one instance of it, LIB-31) with its initial function resolved.
struct Chan<'a> {
    /// The geometry, or the geometry reference, this instance sits on.
    geometry: String,
    /// 1-based offsets, coarse first.
    offsets: Vec<u16>,
    name: String,
    dmx: &'a DmxChannel,
    lc: &'a LogicalChannel,
    func: &'a ChannelFunction,
}

/// The mode, or `None` (with a warning) when it cannot be imported.
fn convert_mode(ft: &FixtureType, m: &DmxMode, rep: &mut ImportReport) -> Option<Mode> {
    let name = m.name.as_deref().unwrap_or_default();
    let breaks = m.dmx_channels.iter().map(|c| c.dmx_break);
    if breaks
        .clone()
        .any(|b| matches!(b, DmxBreak::Value(n) if n != 1))
    {
        rep.warnings.push(format!(
            "mode '{name}' skipped: it uses a DMX break other than 1 (one fixture = one footprint)"
        ));
        return None;
    }
    let start = rep.channels.len();
    let mut chans = Vec::new();
    for dmx in &m.dmx_channels {
        instances(ft, m, name, dmx, &mut chans, rep);
    }
    chans.sort_by_key(|c| c.offsets.first().copied());

    // Entries, with the mode's physical ranges taken from Pan, Tilt and Zoom.
    let mut physical = Physical::default();
    let mut entries: Vec<(String, Entry)> = Vec::new();
    for c in &chans {
        let role = classify(ft, c.lc);
        physical.take(&role, c.func);
        let colour = matches!(role, Role::Comp(_) | Role::Attr { emitter: true, .. });
        let number = c.offsets.first().map_or(0, |o| o.saturating_sub(1));
        let mut e = Entry::new(number, c.name.clone(), role);
        e.fine = c.offsets.get(1).copied();
        if c.offsets.len() > 2 {
            rep.warnings.push(format!(
                "mode '{name}': channel {} '{}' ({}) has a third byte, which is not used",
                number + 1,
                c.name,
                attribute_name(c.lc)
            ));
        }
        e.default = unit(c.func.default);
        e.highlight = c.dmx.highlight.map(unit);
        e.slots = slots_of(
            ft,
            c.lc,
            matches!(&e.role, Role::Attr { name, .. } if name.starts_with("Colour Wheel")),
        );
        if colour {
            let cie = cie_of(ft, c.func);
            e.cie = cie;
            if cie.is_none() {
                e.note = " (no CIE colour)".into();
            }
        }
        entries.push((c.geometry.clone(), e));
    }

    // Cells: when colour sits on two or more geometries, each of them is a cell (LIB-31).
    let is_colour = |e: &Entry| matches!(e.role, Role::Comp(_) | Role::Attr { emitter: true, .. });
    let mut cell_geometries: Vec<&str> = Vec::new();
    for (g, e) in &entries {
        if is_colour(e) && !cell_geometries.contains(&g.as_str()) {
            cell_geometries.push(g);
        }
    }
    if cell_geometries.len() < 2 {
        cell_geometries.clear();
    }
    let cell_names: Vec<String> = cell_geometries.iter().map(|g| (*g).into()).collect();
    let (mut loose, mut per_cell): (Vec<Entry>, Vec<Vec<Entry>>) =
        (Vec::new(), cell_names.iter().map(|_| Vec::new()).collect());
    for (g, e) in entries {
        match cell_names
            .iter()
            .position(|c| *c == g)
            .and_then(|i| per_cell.get_mut(i))
        {
            Some(cell) => cell.push(e),
            None => loose.push(e),
        }
    }
    let scope = build_scope(loose, "", name, rep);
    let cells: Vec<Cell> = per_cell
        .into_iter()
        .enumerate()
        .map(|(i, es)| {
            let (colour, attributes) = build_scope(es, &format!("cell {}: ", i + 1), name, rep);
            Cell { colour, attributes }
        })
        .collect();
    if let Some(lines) = rep.channels.get_mut(start..) {
        lines.sort_by_key(|l| l.channel);
    }

    let footprint = chans
        .iter()
        .flat_map(|c| c.offsets.iter().copied())
        .max()
        .unwrap_or(0);
    let mut mode = mode_of_scopes(name, footprint, scope, cells);
    mode.pan_degrees = physical.pan;
    mode.tilt_degrees = physical.tilt;
    mode.zoom_degrees = physical.zoom;
    Some(mode)
}

/// The instances of one DMX channel: itself, or once per geometry reference when its break is
/// `Overwrite`. Channels that cannot be placed go to the report instead (LIB-30, LIB-33).
fn instances<'a>(
    ft: &FixtureType,
    m: &DmxMode,
    mode: &str,
    dmx: &'a DmxChannel,
    out: &mut Vec<Chan<'a>>,
    rep: &mut ImportReport,
) {
    let name = format!(
        "{}_{}",
        &*dmx.geometry,
        dmx.logical_channels
            .first()
            .map_or("", |l| attribute_name(l))
    );
    let mut skip = |channel: u16, why: &str, warn: bool| {
        let became = format!("{why}, not imported");
        if warn {
            rep.warnings
                .push(format!("mode '{mode}': channel '{name}': {became}"));
        }
        rep.channels.push(ReportLine {
            mode: mode.into(),
            channel,
            source_name: name.clone(),
            became,
        });
    };
    let Some(offsets) = dmx.offset.as_ref() else {
        return skip(0, "virtual channel (no DMX offset)", false);
    };
    let Some((lc, func)) = initial(dmx) else {
        return skip(0, "no channel function", true);
    };
    let offsets: Vec<u16> = offsets
        .iter()
        .map_while(|o| u16::try_from(*o).ok().filter(|o| *o > 0))
        .collect();
    if offsets.is_empty() {
        return skip(0, "invalid DMX offset", true);
    }
    // Where it sits: (geometry, offset added to every byte).
    let mut places = vec![(dmx.geometry.to_string(), 0u16)];
    if dmx.dmx_break == DmxBreak::Overwrite {
        let mut refs = Vec::new();
        let roots: Vec<&Geometry> = match m.geometry.as_deref().and_then(|g| ft.root_geometry(g)) {
            Some(root) => vec![root],
            None => ft.geometries.iter().collect(),
        };
        for g in roots {
            references(g, &dmx.geometry, &mut refs);
        }
        if refs.is_empty() {
            return skip(
                offsets.first().copied().unwrap_or(0),
                "DMXBreak Overwrite on a geometry nothing references",
                true,
            );
        }
        places = refs;
    }
    for (geometry, add) in places {
        let shifted: Option<Vec<u16>> = offsets.iter().map(|o| o.checked_add(add)).collect();
        match shifted {
            Some(offsets) => out.push(Chan {
                name: format!("{geometry}_{}", attribute_name(lc)),
                geometry,
                offsets,
                dmx,
                lc,
                func,
            }),
            None => skip(0, "DMX offset out of range", true),
        }
    }
}

/// Every geometry reference to `target` below `g`: its name and its DMX offset − 1 (the first
/// `Break` on DMX break 1).
fn references(g: &Geometry, target: &str, out: &mut Vec<(String, u16)>) {
    if let Geometry::Reference(r) = g {
        let same = r.geometry.as_deref() == Some(target);
        let brk = r.breaks.iter().find(|b| b.dmx_break == 1);
        if let (true, Some(b)) = (same, brk) {
            let add = u16::try_from(b.dmx_offset.absolute().saturating_sub(1)).unwrap_or(u16::MAX);
            out.push((r.name.as_deref().unwrap_or_default().into(), add));
        }
    }
    for c in g.children() {
        references(c, target, out);
    }
}

/// The initial channel function: the one `InitialFunction` ends in, else the first one.
fn initial(c: &DmxChannel) -> Option<(&LogicalChannel, &ChannelFunction)> {
    let wanted = c.initial_function.as_ref().and_then(|n| n.last());
    let named = wanted.and_then(|w| {
        c.logical_channels.iter().find_map(|lc| {
            let f = lc.channel_functions.iter();
            let f = f.into_iter().find(|f| f.name.as_deref() == Some(&**w))?;
            Some((lc, f))
        })
    });
    named.or_else(|| {
        let lc = c.logical_channels.first()?;
        Some((lc, lc.channel_functions.first()?))
    })
}

fn attribute_name(lc: &LogicalChannel) -> &str {
    lc.attribute.first().map_or("", |n| &**n)
}

// ---- attribute mapping (LIB-29) --------------------------------------------------------------

use AttributeGroup::{B, C, E, G, I, P, S};

/// `(prefix, suffix, name, group)`: the attribute is `prefix` + a number + `suffix`.
const NUMBERED: [(&str, &str, &str, AttributeGroup); 11] = [
    ("Shutter", "", "Strobe", I),
    ("Shutter", "Strobe", "Strobe", I),
    ("Focus", "", "Focus", B),
    ("Frost", "", "Frost", B),
    ("Gobo", "", "Gobo", G),
    ("Gobo", "Pos", "Gobo Index", G),
    ("Gobo", "PosRotate", "Gobo Rotation", G),
    ("Prism", "", "Prism", E),
    ("Prism", "PosRotate", "Prism Rotation", E),
    ("Color", "", "Colour Wheel", C),
    ("ColorMacro", "", "Colour Macro", C),
];

/// Index into the QLC+ importer's component order (Red, Green, Blue, Cyan, Magenta, Yellow).
const COMPONENTS: [(&str, usize); 12] = [
    ("ColorAdd_R", 0),
    ("ColorRGB_Red", 0),
    ("ColorAdd_G", 1),
    ("ColorRGB_Green", 1),
    ("ColorAdd_B", 2),
    ("ColorRGB_Blue", 2),
    ("ColorSub_C", 3),
    ("ColorRGB_Cyan", 3),
    ("ColorSub_M", 4),
    ("ColorRGB_Magenta", 4),
    ("ColorSub_Y", 5),
    ("ColorRGB_Yellow", 5),
];

const EMITTERS: [(&str, &str); 13] = [
    ("ColorAdd_W", "White"),
    ("ColorAdd_WW", "Warm White"),
    ("ColorAdd_CW", "Cold White"),
    ("ColorAdd_RY", "Amber"),
    ("ColorAdd_GY", "Lime"),
    ("ColorAdd_UV", "UV"),
    ("ColorAdd_BM", "Indigo"),
    ("ColorAdd_GC", "Blue-Green"),
    ("ColorAdd_BC", "Light Blue"),
    ("ColorAdd_RM", "Pink"),
    ("ColorAdd_C", "Cyan"),
    ("ColorAdd_M", "Magenta"),
    ("ColorAdd_Y", "Yellow"),
];

/// The role of a logical channel; an unknown attribute is a plain channel with a warning.
fn classify(ft: &FixtureType, lc: &LogicalChannel) -> Role {
    let a = attribute_name(lc);
    let attr = |name: &str, group, kind| Role::Attr {
        name: name.into(),
        group,
        kind,
        emitter: false,
    };
    let number = |prefix: &str, suffix: &str| -> Option<u32> {
        let digits = a.strip_prefix(prefix)?.strip_suffix(suffix)?;
        let all_digits = !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit());
        all_digits.then(|| digits.parse().ok()).flatten()
    };
    let known = match a {
        "Dimmer" => Some(attr("Dimmer", I, AttributeKind::Htp)),
        "Pan" => Some(attr("Pan", P, AttributeKind::Ltp)),
        "Tilt" => Some(attr("Tilt", P, AttributeKind::Ltp)),
        "CTO" | "CTC" | "CTB" => Some(attr(a, C, AttributeKind::Ltp)),
        "Iris" | "Zoom" => Some(attr(a, B, AttributeKind::Ltp)),
        "PanTiltSpeed" | "PositionMSpeed" => Some(attr("Pan/Tilt Speed", S, AttributeKind::Ltp)),
        _ => None,
    };
    let known = known
        .or_else(|| {
            let (_, i) = COMPONENTS.iter().find(|(n, _)| *n == a)?;
            Some(Role::Comp(*i))
        })
        .or_else(|| {
            let (_, name) = EMITTERS.iter().find(|(n, _)| *n == a)?;
            Some(Role::Attr {
                name: (*name).into(),
                group: C,
                kind: AttributeKind::Ltp,
                emitter: true,
            })
        })
        .or_else(|| {
            NUMBERED.iter().find_map(|(prefix, suffix, name, group)| {
                let n = number(prefix, suffix)?;
                let name = if n > 1 {
                    format!("{name} {n}")
                } else {
                    (*name).into()
                };
                Some(attr(&name, *group, AttributeKind::Ltp))
            })
        });
    if let Some(role) = known {
        return role;
    }
    // Any other attribute: a plain channel, named and grouped from the attribute definition.
    let def = ft.attribute_definitions.attribute(a);
    let pretty = def.map(|d| d.pretty.trim()).filter(|p| !p.is_empty());
    let feature_group = def.and_then(|d| d.feature.as_ref()).and_then(|f| f.first());
    let group = match feature_group.map(|g| &**g) {
        Some("Dimmer") => I,
        Some("Position") => P,
        Some("Color") => C,
        Some("Gobo") => G,
        Some("Beam" | "Focus" | "Shapers") => B,
        _ => S,
    };
    Role::Plain(
        pretty.unwrap_or(a).into(),
        group,
        Some(format!("attribute {a} is not mapped")),
    )
}

// ---- values, slots, colours (LIB-30, 32) -----------------------------------------------------

/// A GDTF value `v/n` as 0.0..=1.0: v / (2^(8n) − 1).
fn unit(v: DmxValue) -> f32 {
    let max = 256f64.powi(i32::from(v.bytes().get())) - 1.0;
    (v.value() as f64 / max).clamp(0.0, 1.0) as f32
}

/// The coarse byte of a value, for slot ranges.
fn coarse(v: DmxValue) -> u8 {
    if v.bytes().get() > 8 {
        return 0;
    }
    v.to_u8()
}

/// Named channel sets, else one slot per function (two or more); ranges end where the next begins.
/// `wheel` slots take their colour from the wheel (LIB-30).
fn slots_of(ft: &FixtureType, lc: &LogicalChannel, wheel: bool) -> Vec<Slot> {
    let mut raw: Vec<(u8, String, Option<TargetColour>)> = Vec::new();
    for f in &lc.channel_functions {
        let wheel_of = f.wheel(ft).filter(|_| wheel);
        for s in &f.channel_sets {
            let name = s.name.as_deref().unwrap_or_default();
            let colour = wheel_of.and_then(|w| match &s.wheel_slot(w)?.optic {
                WheelSlotOptic::Color(c) => target_of(c.x, c.y),
                WheelSlotOptic::Filter(_) => None,
            });
            if !name.is_empty() {
                raw.push((coarse(s.dmx_from), name.into(), colour));
            }
        }
    }
    if raw.is_empty() && lc.channel_functions.len() >= 2 {
        for f in &lc.channel_functions {
            let name = f.name.as_deref().unwrap_or_else(|| attribute_name(lc));
            raw.push((coarse(f.dmx_from), name.into(), None));
        }
    }
    raw.sort_by_key(|r| r.0);
    let ends: Vec<u8> = (raw.iter().skip(1))
        .map(|r| r.0.saturating_sub(1))
        .chain([255])
        .collect();
    raw.into_iter()
        .zip(ends)
        .map(|((from, name, colour), to)| Slot {
            name,
            from,
            to: to.max(from),
            colour,
        })
        .collect()
}

/// CIE xy (Y = 1) to a target colour: linear sRGB (D65), negatives clipped, largest scaled to 1.
fn target_of(x: f64, y: f64) -> Option<TargetColour> {
    if y <= 0.0 {
        return None;
    }
    let (cx, cz) = (x / y, (1.0 - x - y) / y);
    let rgb = [
        3.2406 * cx - 1.5372 - 0.4986 * cz,
        -0.9689 * cx + 1.8758 + 0.0415 * cz,
        0.0557 * cx - 0.2040 + 1.0570 * cz,
    ]
    .map(|v| v.max(0.0));
    let max = rgb.iter().copied().fold(0.0, f64::max);
    if max <= 0.0 {
        return None;
    }
    let [r, g, b] = rgb.map(|v| (v / max) as f32);
    Some(TargetColour { r, g, b })
}

/// The CIE xy of the emitter a channel function links, when the emitter has a colour (LIB-32).
fn cie_of(ft: &FixtureType, f: &ChannelFunction) -> Option<[f32; 2]> {
    match f.emitter(ft)?.optic {
        EmitterOptic::Color { color, .. } => Some([color.x as f32, color.y as f32]),
        EmitterOptic::WaveLength { .. } => None,
    }
}

/// Pan and Tilt span, Zoom range, from the first function of each (LIB-30).
#[derive(Default)]
struct Physical {
    pan: Option<f32>,
    tilt: Option<f32>,
    zoom: Option<[f32; 2]>,
}

impl Physical {
    fn take(&mut self, role: &Role, f: &ChannelFunction) {
        let Role::Attr { name, .. } = role else {
            return;
        };
        // GDTF's own default 0..1 counts as unknown.
        let (from, to) = (f.physical_from, f.physical_to);
        if (from, to) == (0.0, 1.0) || from == to {
            return;
        }
        let span = (to - from).abs() as f32;
        let range = [from.min(to) as f32, from.max(to) as f32];
        match name.as_str() {
            "Pan" => self.pan = self.pan.or(Some(span)),
            "Tilt" => self.tilt = self.tilt.or(Some(span)),
            "Zoom" => self.zoom = self.zoom.or(Some(range)),
            _ => {}
        }
    }
}
