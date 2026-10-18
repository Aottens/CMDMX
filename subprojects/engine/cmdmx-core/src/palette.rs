//! Palettes: storage, kind detection, resolving and automatic icons
//! (`specs/modules/palettes.md` PAL-01..07, 20..22). Pure; the engine owns the state and
//! the actions (`record_palette`, `apply_palette`, …).
//!
//! A palette holds values on three levels: global (`attribute → value`), shared (fixture type,
//! the profile key, `→ attribute → value`) and per fixture. Programmer and playbacks store a
//! [`Reference`](crate::value::Reference) to its id, so a change shows on the next tick.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::attribute::{AttributeGroup, AttributeKind};
use crate::colour::TargetColour;
use crate::patch::{Fixture, FixtureId};
use crate::profile::{Mode, COLOUR};
use crate::value::Value;
use crate::CoreError;

/// Internal palette id: assigned by the engine, never reused (PAL, Definitions).
pub type PaletteId = u32;

/// Pan and Tilt, the global attributes that are stored in degrees (PAL-04).
pub const PAN: &str = "Pan";
pub const TILT: &str = "Tilt";

/// PAL-17: user numbers run from 1 to this.
pub const MAX_NUMBER: u32 = 9999;
/// PAL-20.
pub const MAX_LEGEND: usize = 32;
/// PAL-22.
pub const MAX_ICON: usize = 48;

/// Where a palette lives (shown as Colours, Positions and Gobos & Beams).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Window {
    Colours,
    Positions,
    Beams,
}

impl Window {
    /// PAL-01: the default record mask of the window (Titan Quick Record).
    pub fn mask(self) -> &'static [AttributeGroup] {
        use AttributeGroup::*;
        match self {
            Window::Colours => &[C],
            Window::Positions => &[P],
            Window::Beams => &[I, G, B, E, S],
        }
    }
}

/// A window and a handle (1-based) in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Cell {
    pub window: Window,
    pub handle: u16,
}

impl Cell {
    pub fn new(window: Window, handle: u16) -> Self {
        Cell { window, handle }
    }
}

/// PAL-02.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordBy {
    Channel,
    Mixed,
}

/// PAL-04/05.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordAs {
    #[default]
    Auto,
    Normal,
    Shared,
}

/// PAL-06.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordMode {
    Replace,
    Merge,
    QuickMerge,
}

/// The kind tag: `G`, `S` or `N`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaletteKind {
    #[serde(rename = "G")]
    Global,
    #[serde(rename = "S")]
    Shared,
    #[serde(rename = "N")]
    Normal,
}

/// The fields of `palette.record` (PAL-27); everything but the cell is optional.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    #[serde(flatten)]
    pub cell: Cell,
    /// Only for a new palette.
    #[serde(default)]
    pub number: Option<u32>,
    /// Default: the window's mask.
    #[serde(default)]
    pub mask: Option<Vec<AttributeGroup>>,
    /// Default: the setting `record_by`.
    #[serde(default)]
    pub record_by: Option<RecordBy>,
    #[serde(default)]
    pub record_as: RecordAs,
    /// Only for a used cell.
    #[serde(default)]
    pub mode: Option<RecordMode>,
}

impl Record {
    pub fn at(cell: Cell) -> Self {
        Record {
            cell,
            number: None,
            mask: None,
            record_by: None,
            record_as: RecordAs::Auto,
            mode: None,
        }
    }
}

/// Fade time (s) and overlap (%) of an apply (PAL-13..15). `None` takes the master setting.
/// ponytail: VS2-17 applies instantly and ignores both; VS2-18 adds the fades.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Timing {
    pub time: Option<f32>,
    pub overlap: Option<f32>,
}

/// A value on the global level: a value, or for Pan and Tilt degrees from centre.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GlobalValue {
    Value(Value),
    Degrees { degrees: f32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Palette {
    pub id: PaletteId,
    pub number: u32,
    pub window: Window,
    pub handle: u16,
    /// 0–32 characters (PAL-20).
    pub legend: String,
    /// A built-in icon id, else the automatic icon (PAL-20, 22).
    pub icon: Option<String>,
    pub global: BTreeMap<String, GlobalValue>,
    /// Fixture type (profile key) → attribute → value.
    pub shared: BTreeMap<String, BTreeMap<String, Value>>,
    pub fixtures: BTreeMap<FixtureId, BTreeMap<String, Value>>,
}

/// PAL-21: what a client draws for a palette without a picked icon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoIcon {
    Colours(Vec<TargetColour>),
    Position(Vec<[f32; 2]>),
    Value { attribute: String, value: f32 },
}

impl Palette {
    pub fn cell(&self) -> Cell {
        Cell::new(self.window, self.handle)
    }

    /// `G` when the global level is not empty, else `S` when the shared level is not empty.
    pub fn kind(&self) -> PaletteKind {
        if !self.global.is_empty() {
            PaletteKind::Global
        } else if !self.shared.is_empty() {
            PaletteKind::Shared
        } else {
            PaletteKind::Normal
        }
    }

    /// PAL-07: per fixture, then shared by type, then global (if the fixture has the attribute);
    /// an extra emitter is 0 when the palette resolves Colour for the fixture. Nothing when the
    /// mode lacks the attribute (PAL-19).
    pub fn resolve(&self, fixture: &Fixture, mode: &Mode, attribute: &str) -> Option<Value> {
        let logical = mode.logical(attribute)?;
        if let Some(v) = self.level_value(fixture, mode, attribute) {
            return Some(v);
        }
        let colour =
            mode.logical(COLOUR).is_some() && self.level_value(fixture, mode, COLOUR).is_some();
        (logical.emitter && colour).then_some(Value::Level(0.0))
    }

    fn level_value(&self, fixture: &Fixture, mode: &Mode, attribute: &str) -> Option<Value> {
        if let Some(v) = self
            .fixtures
            .get(&fixture.id)
            .and_then(|a| a.get(attribute))
            .or_else(|| {
                self.shared
                    .get(&fixture.profile)
                    .and_then(|a| a.get(attribute))
            })
        {
            return Some(*v);
        }
        match self.global.get(attribute)? {
            GlobalValue::Value(v) => Some(*v),
            GlobalValue::Degrees { degrees } => {
                let range = match attribute {
                    PAN => mode.pan_degrees,
                    TILT => mode.tilt_degrees,
                    _ => None,
                }
                .filter(|r| *r > 0.0)?;
                Some(Value::Level((0.5 + degrees / range).clamp(0.0, 1.0)))
            }
        }
    }

    /// PAL-06: write detected values over the existing ones at their own level.
    pub(crate) fn merge(&mut self, d: Detected) {
        self.global.extend(d.global);
        for (profile, values) in d.shared {
            self.shared.entry(profile).or_default().extend(values);
        }
        for (fixture, values) in d.fixtures {
            self.fixtures.entry(fixture).or_default().extend(values);
        }
    }

    /// PAL-06 Quick Merge: does the palette hold this attribute on any level?
    pub(crate) fn holds(&self, attribute: &str) -> bool {
        self.global.contains_key(attribute)
            || self.shared.values().any(|a| a.contains_key(attribute))
            || self.fixtures.values().any(|a| a.contains_key(attribute))
    }

    /// Every attribute name the palette holds on any level, sorted.
    pub fn attribute_names(&self) -> BTreeSet<&str> {
        (self.global.keys().map(String::as_str))
            .chain(
                self.shared
                    .values()
                    .flat_map(|a| a.keys().map(String::as_str)),
            )
            .chain(
                self.fixtures
                    .values()
                    .flat_map(|a| a.keys().map(String::as_str)),
            )
            .collect()
    }

    /// PAL-21. `group_of` gives an attribute's group (for the first-value icon).
    pub fn auto_icon(&self, group_of: impl Fn(&str) -> AttributeGroup) -> Option<AutoIcon> {
        // Global, shared by type, per fixture: each level as name → value (degrees as drawn).
        let global = self
            .global
            .iter()
            .map(|(name, v)| {
                let value = match v {
                    GlobalValue::Value(v) => *v,
                    GlobalValue::Degrees { degrees } => {
                        let range = if name == PAN { 540.0 } else { 270.0 };
                        Value::Level(0.5 + degrees / range)
                    }
                };
                (name.as_str(), value)
            })
            .collect::<BTreeMap<&str, Value>>();
        let sets: Vec<BTreeMap<&str, Value>> = std::iter::once(global)
            .chain(self.shared.values().map(as_set))
            .chain(self.fixtures.values().map(as_set))
            .collect();
        auto_icon_of(&sets, group_of)
    }
}

/// PAL-21, PBK-32: the icon for sets of `attribute → value` (a palette's levels, a cue's
/// fixtures): colours, else positions, else the first stored attribute.
pub(crate) fn auto_icon_of(
    sets: &[BTreeMap<&str, Value>],
    group_of: impl Fn(&str) -> AttributeGroup,
) -> Option<AutoIcon> {
    let mut colours: Vec<TargetColour> = Vec::new();
    for set in sets {
        if let Some(Value::Colour(c)) = set.get(COLOUR) {
            if colours.len() < 4 && !colours.contains(c) {
                colours.push(*c);
            }
        }
    }
    if !colours.is_empty() {
        return Some(AutoIcon::Colours(colours));
    }

    let mut points: Vec<[f32; 2]> = Vec::new();
    for set in sets {
        if set.contains_key(PAN) || set.contains_key(TILT) {
            let axis = |name: &str| set.get(name).map_or(0.5, Value::level);
            let point = [axis(PAN), axis(TILT)];
            if points.len() < 8 && !points.contains(&point) {
                points.push(point);
            }
        }
    }
    if !points.is_empty() {
        return Some(AutoIcon::Position(points));
    }

    let (attribute, value) = sets
        .iter()
        .flat_map(|set| set.iter())
        .min_by(|a, b| (group_of(a.0), a.0).cmp(&(group_of(b.0), b.0)))?;
    Some(AutoIcon::Value {
        attribute: (*attribute).to_string(),
        value: value.level(),
    })
}

pub(crate) fn as_set(values: &BTreeMap<String, Value>) -> BTreeMap<&str, Value> {
    values.iter().map(|(k, v)| (k.as_str(), *v)).collect()
}

/// The attribute group's letters as shown on the console.
pub(crate) fn letter(group: AttributeGroup) -> &'static str {
    use AttributeGroup::*;
    match group {
        I => "I",
        P => "P",
        C => "C",
        G => "G",
        B => "B",
        E => "E",
        S => "S",
        Fx => "FX",
    }
}

/// PAL-22: `category.name`, lower case `[a-z0-9.-]`, at most 48 characters.
pub fn valid_icon(icon: &str) -> bool {
    icon.len() <= MAX_ICON
        && icon
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
        && icon
            .split_once('.')
            .is_some_and(|(category, name)| !category.is_empty() && !name.is_empty())
}

// ---- kind detection (PAL-04/05) -------------------------------------------------

/// One fixture's values going into a record, with the type and mode they belong to.
pub(crate) struct Entry<'a> {
    pub profile: &'a str,
    pub mode: &'a Mode,
    pub values: BTreeMap<String, Value>,
}

/// What detection decided: the three levels of the new contents.
#[derive(Debug, Default)]
pub(crate) struct Detected {
    pub global: BTreeMap<String, GlobalValue>,
    pub shared: BTreeMap<String, BTreeMap<String, Value>>,
    pub fixtures: BTreeMap<FixtureId, BTreeMap<String, Value>>,
}

/// How far detection may go: `Record as` narrowed by the kind a merge must not widen (PAL-05/06).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Limit {
    pub global: bool,
    pub shared: bool,
    /// Refuse (rather than fall back to per fixture) when values differ inside a type.
    pub strict: bool,
}

impl Limit {
    pub fn from_record_as(record_as: RecordAs) -> Limit {
        match record_as {
            RecordAs::Auto => Limit {
                global: true,
                shared: true,
                strict: false,
            },
            RecordAs::Normal => Limit {
                global: false,
                shared: false,
                strict: false,
            },
            RecordAs::Shared => Limit {
                global: false,
                shared: true,
                strict: true,
            },
        }
    }

    /// A merge never goes wider than the palette's current kind.
    pub fn capped_by(self, kind: PaletteKind) -> Limit {
        let (global, shared) = match kind {
            PaletteKind::Global => (true, true),
            PaletteKind::Shared => (false, true),
            PaletteKind::Normal => (false, false),
        };
        let shared = self.shared && shared;
        Limit {
            global: self.global && global,
            shared,
            strict: self.strict && shared,
        }
    }
}

/// 16-bit comparison: `round(v · 65535)`, each colour component on its own.
fn quantised(v: f32) -> i64 {
    (f64::from(v.clamp(0.0, 1.0)) * 65535.0).round() as i64
}

fn same_value(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Level(x), Value::Level(y)) => quantised(*x) == quantised(*y),
        (Value::Colour(x), Value::Colour(y)) => {
            quantised(x.r) == quantised(y.r)
                && quantised(x.g) == quantised(y.g)
                && quantised(x.b) == quantised(y.b)
        }
        _ => false,
    }
}

fn same_values(a: &BTreeMap<String, Value>, b: &BTreeMap<String, Value>) -> bool {
    a.len() == b.len()
        && a.iter()
            .all(|(k, v)| b.get(k).is_some_and(|w| same_value(v, w)))
}

/// PAL-04: global attributes are intensity (group I, HTP), Pan, Tilt and Colour.
fn is_global(mode: &Mode, name: &str) -> bool {
    name == COLOUR
        || name == PAN
        || name == TILT
        || mode
            .logical(name)
            .is_some_and(|l| l.group == AttributeGroup::I && l.kind == AttributeKind::Htp)
}

/// Degrees from centre of a Pan or Tilt value on a fixture with a physical range.
fn degrees(entry: &Entry, name: &str) -> Option<f32> {
    let range = match name {
        PAN => entry.mode.pan_degrees,
        _ => entry.mode.tilt_degrees,
    }
    .filter(|r| *r > 0.0)?;
    Some((entry.values.get(name)?.level() - 0.5) * range)
}

/// PAL-04/05: sort the values of a record into global, shared and per-fixture levels.
pub(crate) fn detect(
    entries: &BTreeMap<FixtureId, Entry>,
    limit: Limit,
) -> Result<Detected, CoreError> {
    let per_fixture = || Detected {
        fixtures: entries
            .iter()
            .map(|(&id, e)| (id, e.values.clone()))
            .collect(),
        ..Detected::default()
    };
    if !limit.shared {
        return Ok(per_fixture());
    }
    let mut types: BTreeMap<&str, Vec<&Entry>> = BTreeMap::new();
    for entry in entries.values() {
        types.entry(entry.profile).or_default().push(entry);
    }
    let mut shared: BTreeMap<String, BTreeMap<String, Value>> = BTreeMap::new();
    for (&profile, members) in &types {
        let Some((first, rest)) = members.split_first() else {
            continue;
        };
        if !rest.iter().all(|e| same_values(&e.values, &first.values)) {
            return if limit.strict {
                Err(CoreError::SharedValuesDiffer)
            } else {
                Ok(per_fixture())
            };
        }
        shared.insert(profile.to_string(), first.values.clone());
    }

    let mut global = BTreeMap::new();
    if limit.global {
        let representatives: Vec<&Entry> =
            types.values().filter_map(|m| m.first().copied()).collect();
        let candidates: Vec<(String, bool)> = representatives
            .first()
            .map(|e| {
                e.values
                    .keys()
                    .filter(|name| is_global(e.mode, name))
                    .map(|name| (name.clone(), name == PAN || name == TILT))
                    .collect()
            })
            .unwrap_or_default();
        for (name, is_angle) in candidates {
            let value = if is_angle {
                let all: Option<Vec<f32>> = entries.values().map(|e| degrees(e, &name)).collect();
                all.and_then(|all| {
                    let first = all.first().copied()?;
                    all.iter()
                        .all(|d| (d - first).abs() <= 0.01)
                        .then_some(GlobalValue::Degrees { degrees: first })
                })
            } else {
                let first = representatives.first().and_then(|e| e.values.get(&name));
                first
                    .filter(|f| {
                        representatives
                            .iter()
                            .all(|e| e.values.get(&name).is_some_and(|v| same_value(v, f)))
                    })
                    .map(|f| GlobalValue::Value(*f))
            };
            if let Some(value) = value {
                global.insert(name.clone(), value);
                for values in shared.values_mut() {
                    values.remove(&name);
                }
            }
        }
        shared.retain(|_, values| !values.is_empty());
    }
    Ok(Detected {
        global,
        shared,
        fixtures: BTreeMap::new(),
    })
}
