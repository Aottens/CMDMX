//! The programmer: selection, sub-selection and manual values.
//!
//! Implements `specs/modules/programmer.md` PRG-01..07, PRG-14 and the located set (PRG-23..25,
//! 38); the merge reads its values at `Priority::Programmer` (PRG-12/13). A value is manual or a palette
//! reference (PAL-08). The view also carries the values and the picker colour (IPC-21, IPC-22).
//! Values, fades, located values and clear fades are kept per [`Item`] (PRG-46, 47, 51, 52): a
//! whole fixture and each of its cells have their own entry.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::attribute::AttributeGroup;
use crate::colour::ColourModel;
use crate::effect::{positions, Entry, EntryId, Params, Shape};
use crate::gel::GelRef;
use crate::palette::PaletteId;
use crate::patch::FixtureId;
use crate::value::{Stored, Value};
use crate::{CoreError, Timestamp};

pub type GroupId = u32;

/// PRG-46: a selection item, a whole fixture (`cell == None`) or one cell (1-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Item {
    pub fixture: FixtureId,
    pub cell: Option<u16>,
}

/// PRG-52: `"901"` for a whole fixture, `"901.2"` for a cell (cells are 1-based).
impl fmt::Display for Item {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.cell {
            None => write!(f, "{}", self.fixture),
            Some(c) => write!(f, "{}.{c}", self.fixture),
        }
    }
}

impl FromStr for Item {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let bad = || format!("not an item: {s:?}");
        let (fixture, cell) = match s.split_once('.') {
            None => (s, None),
            Some((f, c)) => (
                f,
                Some(c.parse::<u16>().ok().filter(|c| *c > 0).ok_or_else(bad)?),
            ),
        };
        Ok(Item {
            fixture: fixture.parse().map_err(|_| bad())?,
            cell,
        })
    }
}

impl Serialize for Item {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

/// IPC-42: the wire form of an item, an integer for a whole fixture, `"901.2"` for a cell.
pub fn serialize_wire_item<S: Serializer>(item: &Item, serializer: S) -> Result<S::Ok, S::Error> {
    match item.cell {
        None => serializer.serialize_u32(item.fixture),
        Some(_) => serializer.serialize_str(&item.to_string()),
    }
}

/// IPC-42: a list of items in wire form.
pub fn serialize_wire_items<S: Serializer>(
    items: &[Item],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeSeq;
    let mut seq = serializer.serialize_seq(Some(items.len()))?;
    for item in items {
        seq.serialize_element(&WireItem(item))?;
    }
    seq.end()
}

/// A [`Serialize`] wrapper over [`serialize_wire_item`].
pub struct WireItem<'a>(pub &'a Item);

impl Serialize for WireItem<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_wire_item(self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for Item {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl From<FixtureId> for Item {
    fn from(fixture: FixtureId) -> Self {
        Item::fixture(fixture)
    }
}

impl Item {
    pub fn fixture(fixture: FixtureId) -> Self {
        Item {
            fixture,
            cell: None,
        }
    }

    pub fn cell(fixture: FixtureId, cell: u16) -> Self {
        Item {
            fixture,
            cell: Some(cell),
        }
    }
}

type Attrs<V> = BTreeMap<String, V>;

/// Remove one attribute of an item; an emptied map goes too.
fn take<V>(map: &mut BTreeMap<Item, Attrs<V>>, item: Item, attribute: &str) {
    if let Some(attrs) = map.get_mut(&item) {
        attrs.remove(attribute);
        if attrs.is_empty() {
            map.remove(&item);
        }
    }
}

/// Wheel step size (PRG-21); the sizes live in the engine `Settings`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Resolution {
    Normal,
    Fine,
    Coarse,
}

/// PRG-73: gives the selection's items shared positions (FX-07's steps, then `flip`).
/// `Grid::default()` is *off*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grid {
    pub wings: u8,
    pub groups: u8,
    pub blocks: u8,
    pub flip: bool,
}

impl Default for Grid {
    fn default() -> Self {
        Grid {
            wings: 1,
            groups: 0,
            blocks: 1,
            flip: false,
        }
    }
}

impl Grid {
    /// PRG-73: refuse anything outside FX-06's ranges (`groups` 0 = off or 2 to 32).
    pub fn validate(&self) -> Result<(), CoreError> {
        let check = |name: &str, v: u8, lo: u8, hi: u8| {
            if (lo..=hi).contains(&v) {
                Ok(())
            } else {
                Err(CoreError::InvalidGrid(format!(
                    "grid: {name} {v} out of range ({lo} to {hi})"
                )))
            }
        };
        check("wings", self.wings, 1, 8)?;
        if self.groups != 0 {
            check("groups", self.groups, 2, 32)?;
        }
        check("blocks", self.blocks, 1, 32)
    }
}

/// PRG-71: the order tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Order {
    Reverse,
    Shuffle,
    Sort,
    Revert,
}

/// PRG-75: the grid shortcuts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridShortcut {
    OddEven,
    OneIn(u8),
    /// `n` in `m`: groups `m`, blocks `n`.
    NInM(u8, u8),
    Blocks(u8),
    Divide(u8),
    OutsideIn,
    InsideOut,
    Off,
}

/// PRG-29 / PRG-42: what Clear does first.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClearPrecedence {
    /// The first Clear deselects, the next empties the programmer.
    #[default]
    SelectionThenProgrammer,
    SelectionWithProgrammer,
    ProgrammerThenSelection,
}

/// PRG-43 / PRG-42: what an LTP attribute does when the programmer gives it up.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClearLtp {
    /// Back to what the playbacks give, else the profile default.
    #[default]
    Release,
    /// Keep the current output while no playback proposes it.
    Freeze,
}

/// PRG-42 (`settings.programmer`): every field optional, applied all or nothing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProgrammerSettingsChange {
    pub clear_precedence: Option<ClearPrecedence>,
    pub clear_ltp: Option<ClearLtp>,
    pub lowlight_level: Option<f32>,
}

/// PRG-42: the default lowlight level.
pub const DEFAULT_LOWLIGHT: f32 = 0.25;

pub(crate) fn default_lowlight() -> f32 {
    DEFAULT_LOWLIGHT
}

/// PRG-35 / IPC-11: one attribute group as the attribute bar shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AttributeGroupState {
    pub group: AttributeGroup,
    /// Some target has an attribute in this group.
    pub available: bool,
    /// Some target has a programmer value in this group.
    pub has_values: bool,
}

/// PRG-35 / IPC-11: one wheel of the active group.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WheelState {
    pub attribute: String,
    pub value: f32,
    /// The value is a programmer value (else it is the current output, PRG-20).
    pub touched: bool,
    /// The attribute has a fine channel in its profile.
    pub sixteen_bit: bool,
    /// PAL-26: the palette the first target's value references, if any.
    pub palette: Option<PaletteId>,
    /// IPC-46: a target auto-mixes this emitter (LIB-36); absent otherwise.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub auto: bool,
}

/// IPC-21: one fixture's programmer values.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FixtureValues {
    #[serde(serialize_with = "serialize_wire_item")]
    pub fixture: Item,
    pub values: BTreeMap<String, Value>,
    /// PAL-26: the attributes among `values` that are palette references.
    pub palettes: BTreeMap<String, PaletteId>,
    /// PAL-26: attribute → progress 0.0–1.0 of a palette fade still running at the last tick.
    pub fading: BTreeMap<String, f32>,
}

/// PAL-26: which palettes the programmer and the selection touch.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct PaletteUse {
    /// Palette ids referenced in the programmer, sorted.
    pub in_use: Vec<PaletteId>,
    /// Palettes that resolve something for a target (all when nothing is selected), by id.
    pub relevant: Vec<PaletteId>,
}

/// IPC-22: the first colour target's colour as hue/saturation (0.0-1.0) and sRGB as stored.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ColourView {
    pub hue: f32,
    pub saturation: f32,
    pub r: f32,
    pub g: f32,
    pub b: f32,
    /// IPC-46: correlated colour temperature rounded to 10 K; `null` for black.
    pub kelvin: Option<f32>,
    /// PRG-69: the gel the colour reads as, at any level.
    pub gel: Option<GelRef>,
}

/// PRG-35 / IPC-11: everything the UI renders of the programmer.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProgrammerView {
    #[serde(serialize_with = "serialize_wire_items")]
    pub selection: Vec<Item>,
    #[serde(serialize_with = "serialize_wire_items")]
    pub targets: Vec<Item>,
    pub active_group: AttributeGroup,
    pub attribute_groups: Vec<AttributeGroupState>,
    pub wheels: Vec<WheelState>,
    #[serde(serialize_with = "serialize_wire_items")]
    pub in_programmer: Vec<Item>,
    pub colour_model: ColourModel,
    pub contents: Vec<FixtureValues>,
    pub colour: Option<ColourView>,
    pub palettes: PaletteUse,
    /// PRG-23: fixture → sorted names of the located attributes (IPC-33).
    pub located: BTreeMap<Item, Vec<String>>,
    /// PRG-26.
    pub highlight: bool,
    /// PRG-56, PRG-62: Fan mode, `None` when off.
    pub fan: Option<FanCurve>,
    /// PRG-56: the curve the Fan key turns on.
    pub fan_curve: FanCurve,
    /// FX-33: the programmer's effect entries, in order. Never the clock.
    pub effects: Vec<EffectView>,
    /// FX-17: the wheels are the effect wheels.
    pub effect_wheels: bool,
    /// FX-37: 1-based catalogue ids of the looks at least one target can run.
    pub runnable_looks: Vec<usize>,
}

/// FX-33: one programmer effect entry as the UI renders it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EffectView {
    pub id: EntryId,
    pub look: Option<String>,
    pub shape: Shape,
    pub attributes: Vec<String>,
    #[serde(serialize_with = "serialize_wire_items")]
    pub members: Vec<Item>,
    /// FX-07: k/N per member.
    pub positions: Vec<f32>,
    /// FX-23: the pointed palette's parameters while it exists.
    pub params: Params,
    pub palette: Option<PaletteId>,
    pub current: bool,
    /// FX-38: the `ColourMix` list; empty for other shapes.
    pub colours: Vec<crate::colour::TargetColour>,
    /// FX-42: a Hue shift on a white base shows nothing.
    pub white_base: bool,
    /// FX-42: a colour effect on a base with intensity 0 shows nothing.
    pub dark: bool,
}

/// PRG-56: the shape of a fan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FanCurve {
    #[default]
    Line,
    Mirror,
    Wings,
    Arrow,
}

/// PRG-58: the factor k (-1..1) of each of `n` fan positions.
pub fn fan_factors(curve: FanCurve, n: usize) -> Vec<f32> {
    if n < 2 {
        return vec![0.0; n];
    }
    // A line over `m` positions, position `i`; one position stays put.
    let line = |i: usize, m: usize| {
        if m < 2 {
            0.0
        } else {
            2.0 * i as f32 / (m - 1) as f32 - 1.0
        }
    };
    let half = n.div_ceil(2);
    let wing = n / 3;
    (0..n)
        .map(|i| match curve {
            FanCurve::Line => line(i, n),
            FanCurve::Arrow => line(i, n).abs(),
            FanCurve::Mirror => line(i.min(n - 1 - i), half),
            FanCurve::Wings if wing == 0 => 0.0,
            FanCurve::Wings if i < wing => -((wing - i) as f32) / wing as f32,
            FanCurve::Wings if i >= n - wing => (i + 1 + wing - n) as f32 / wing as f32,
            FanCurve::Wings => 0.0,
        })
        .collect()
}

/// PAL-13/14: a palette fade of one attribute. Live state, never saved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Fade {
    /// The output value when the palette was applied.
    pub(crate) from: Value,
    /// Seconds after `start` before this fixture begins (the overlap stagger).
    pub(crate) delay: f32,
    pub(crate) duration: f32,
    /// Set by the first tick after the apply.
    pub(crate) start: Option<Timestamp>,
}

impl Fade {
    /// Seconds since this fade began moving, `None` while it has not started.
    fn elapsed(&self, now: Timestamp) -> Option<f32> {
        let e = (now - self.start?) as f32 - self.delay;
        (e > 0.0).then_some(e)
    }

    /// PAL-26: 0.0–1.0 at `now`; 0.0 before the clock starts or while waiting for the overlap turn.
    pub(crate) fn progress(&self, now: Timestamp) -> f32 {
        self.elapsed(now)
            .map_or(0.0, |e| (e / self.duration).min(1.0))
    }

    /// PAL-13: linear from `from` to `to`; Colour per component in linear target RGB.
    pub(crate) fn at(&self, to: Value, now: Timestamp) -> Value {
        let Some(e) = self.elapsed(now) else {
            return self.from;
        };
        self.from.lerp(to, (e / self.duration).min(1.0))
    }
}

/// UNDO: the part of the programmer an undo step saves (ADR-008). Not the fades or `activated`;
/// the located set is part of it (PRG-44).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EditPart {
    selection: Vec<Item>,
    select_order: Vec<Item>,
    grid: Grid,
    stage: Option<usize>,
    changed: bool,
    values: BTreeMap<Item, Attrs<Stored>>,
    located: BTreeMap<Item, Attrs<Value>>,
    effects: Vec<Entry>,
    current: Option<EntryId>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Programmer {
    selection: Vec<Item>,
    /// PRG-71: the selection in the order its items were selected; a permutation of `selection`.
    select_order: Vec<Item>,
    grid: Grid,
    /// PRG-74: the position whose items are the targets; `None` = the whole selection.
    stage: Option<usize>,
    /// PRG-73, 84: each selection item's position and *N*, recomputed when the selection or the
    /// grid changes.
    positions: Vec<u32>,
    position_count: u32,
    /// Set by a value change; the next select starts a new selection (PRG-03).
    changed: bool,
    values: BTreeMap<Item, Attrs<Stored>>,
    /// Running palette fades (PAL-13..16); an entry always has a value here.
    fades: BTreeMap<Item, Attrs<Fade>>,
    /// Located values (PRG-23): not contents, under the programmer value in the merge (PRG-25).
    located: BTreeMap<Item, Attrs<Value>>,
    /// Fades of values a timed Clear removed (ENG-10): live state; HTP `from` → 0, LTP `from`
    /// → what the other sources give. Never has a programmer value of its own.
    clearing: BTreeMap<Item, Attrs<Fade>>,
    /// Engine activation counter at the last value change: orders the
    /// programmer against other sources in LTP ties (ENG-04).
    activated: u64,
    /// FX-14: the programmer's effect entries and the current one (FX-18). Undoable (FX-20);
    /// their clocks are live state of the engine.
    pub(crate) effects: Vec<Entry>,
    pub(crate) current: Option<EntryId>,
}

fn whole(items: &[Item]) -> Vec<FixtureId> {
    items
        .iter()
        .filter(|i| i.cell.is_none())
        .map(|i| i.fixture)
        .collect()
}

impl Programmer {
    pub(crate) fn edit_part(&self) -> EditPart {
        EditPart {
            selection: self.selection.clone(),
            select_order: self.select_order.clone(),
            grid: self.grid,
            stage: self.stage,
            changed: self.changed,
            values: self.values.clone(),
            located: self.located.clone(),
            effects: self.effects.clone(),
            current: self.current,
        }
    }

    /// UNDO-12: put a saved part back. A palette fade survives only while its value is unchanged
    /// (a fade always has a value); `activated` stays.
    pub(crate) fn restore_edit_part(&mut self, part: EditPart) {
        let old = std::mem::replace(&mut self.values, part.values);
        self.selection = part.selection;
        self.select_order = part.select_order;
        self.grid = part.grid;
        self.stage = part.stage;
        self.recompute();
        self.changed = part.changed;
        self.located = part.located;
        self.effects = part.effects;
        self.current = part.current;
        self.clearing.clear(); // ENG-10: undo during a timed Clear puts the programmer back at once
        let values = &self.values;
        for (item, attrs) in &mut self.fades {
            attrs.retain(|a, _| {
                let get =
                    |m: &BTreeMap<Item, Attrs<Stored>>| m.get(item).and_then(|v| v.get(a)).copied();
                get(values).is_some() && get(values) == get(&old)
            });
        }
        self.fades.retain(|_, a| !a.is_empty());
    }

    /// PRG-46: the selected items in selection order.
    pub fn items(&self) -> &[Item] {
        &self.selection
    }

    /// The whole-fixture items of the selection (the fixture-keyed view; see [`Programmer::items`]).
    pub fn selection(&self) -> Vec<FixtureId> {
        whole(&self.selection)
    }

    /// PRG-71: the select order, for Revert.
    pub fn select_order(&self) -> &[Item] {
        &self.select_order
    }

    /// PRG-73: the grid, `None` when off.
    pub fn grid(&self) -> Option<Grid> {
        (self.grid != Grid::default()).then_some(self.grid)
    }

    /// PRG-74: the selected stage, `None` = the whole selection.
    pub fn stage(&self) -> Option<usize> {
        self.stage
    }

    /// PRG-73: each selection item's position, in selection order.
    pub fn positions(&self) -> &[u32] {
        &self.positions
    }

    /// PRG-73: the number of positions *N*.
    pub fn position_count(&self) -> u32 {
        self.position_count
    }

    /// PRG-84: refresh the cached positions after the selection or the grid changed.
    fn recompute(&mut self) {
        let g = self.grid;
        (self.positions, self.position_count) =
            positions(self.selection.len(), g.wings, g.groups, g.blocks, g.flip);
    }

    /// Target positions over a list of `n` items: with a stage selected *k* = *i* and *N* = `n`,
    /// else the grid's (grid off: *k* = *i*). PRG-76..78.
    pub fn list_positions(&self, n: usize) -> (Vec<u32>, u32) {
        let g = if self.stage.is_some() {
            Grid::default()
        } else {
            self.grid
        };
        positions(n, g.wings, g.groups, g.blocks, g.flip)
    }

    /// PRG-04/05/50/74: the items that receive the next change: the stage, else all.
    pub fn target_items(&self) -> Vec<Item> {
        let Some(stage) = self.stage else {
            return self.selection.clone();
        };
        (self.selection.iter().zip(&self.positions))
            .filter(|(_, &k)| k as usize == stage)
            .map(|(i, _)| *i)
            .collect()
    }

    /// The whole-fixture items of [`Programmer::target_items`].
    pub fn targets(&self) -> Vec<FixtureId> {
        whole(&self.target_items())
    }

    /// The manual value; `None` for a palette reference (see [`Programmer::stored`]).
    pub fn value(&self, fixture: FixtureId, attribute: &str) -> Option<Value> {
        match self.stored(fixture, attribute)? {
            Stored::Value(v) => Some(v),
            Stored::Reference(_) => None,
        }
    }

    /// The programmer's entry for an attribute: a manual value or a palette reference.
    pub fn stored(&self, fixture: FixtureId, attribute: &str) -> Option<Stored> {
        self.stored_item(Item::fixture(fixture), attribute)
    }

    /// PRG-47: like [`Programmer::stored`] for a whole fixture or a cell.
    pub fn stored_item(&self, item: Item, attribute: &str) -> Option<Stored> {
        self.values.get(&item)?.get(attribute).copied()
    }

    /// PRG-47: the item's manual values and palette references.
    pub(crate) fn values_of(&self, item: Item) -> Option<&Attrs<Stored>> {
        self.values.get(&item)
    }

    /// The cell items that have a value, a located value or a clear fade.
    pub(crate) fn cell_items(&self) -> impl Iterator<Item = Item> + '_ {
        (self.values.keys().chain(self.located.keys()))
            .chain(self.clearing.keys())
            .copied()
            .filter(|i| i.cell.is_some())
    }

    /// Every item with a value, by item (a cell next to its fixture).
    pub fn values(&self) -> &BTreeMap<Item, Attrs<Stored>> {
        &self.values
    }

    pub(crate) fn fade(&self, item: Item, attribute: &str) -> Option<&Fade> {
        self.fades.get(&item)?.get(attribute)
    }

    pub(crate) fn set_fade(&mut self, item: Item, attribute: &str, fade: Fade) {
        self.fades
            .entry(item)
            .or_default()
            .insert(attribute.to_string(), fade);
    }

    /// Called at the start of a tick: fades that have not begun start now (PAL-13).
    pub(crate) fn start_fades(&mut self, now: Timestamp) {
        let all = (self.fades.values_mut()).chain(self.clearing.values_mut());
        for fade in all.flat_map(|a| a.values_mut()) {
            fade.start.get_or_insert(now);
        }
    }

    /// Called at the end of a tick: finished fades are dropped.
    pub(crate) fn drop_finished_fades(&mut self, now: Timestamp) {
        let done = |f: &Fade| {
            f.start
                .is_some_and(|s| (now - s) as f32 >= f.delay + f.duration)
        };
        for fades in [&mut self.fades, &mut self.clearing] {
            for attrs in fades.values_mut() {
                attrs.retain(|_, f| !done(f));
            }
            fades.retain(|_, a| !a.is_empty());
        }
    }

    /// ENG-10: the fade of a value a timed Clear removed.
    pub(crate) fn clearing(&self, item: Item, attribute: &str) -> Option<&Fade> {
        self.clearing.get(&item)?.get(attribute)
    }

    pub(crate) fn set_clearing(&mut self, item: Item, attribute: &str, fade: Fade) {
        self.clearing
            .entry(item)
            .or_default()
            .insert(attribute.to_string(), fade);
    }

    /// The located values by item.
    pub fn located(&self) -> &BTreeMap<Item, Attrs<Value>> {
        &self.located
    }

    pub(crate) fn located_value(&self, item: Item, attribute: &str) -> Option<Value> {
        self.located.get(&item)?.get(attribute).copied()
    }

    /// PRG-47: the item's located values.
    pub(crate) fn located_of(&self, item: Item) -> Option<&Attrs<Value>> {
        self.located.get(&item)
    }

    /// Some item has a located value.
    pub(crate) fn has_located(&self) -> bool {
        !self.located.is_empty()
    }

    /// PRG-38: Locate again overwrites. The attribute is touched, so no clear fade runs on it.
    pub(crate) fn set_located(&mut self, item: Item, attribute: &str, value: Value) {
        take(&mut self.clearing, item, attribute);
        self.located
            .entry(item)
            .or_default()
            .insert(attribute.to_string(), value);
    }

    pub(crate) fn remove_located(&mut self, item: Item, attribute: &str) {
        take(&mut self.located, item, attribute);
    }

    /// PRG-03: Locate and Rem Dim count as a change, so the next select starts afresh.
    pub(crate) fn mark_changed(&mut self) {
        self.changed = true;
    }

    /// Every item (fixture or cell) with a value or a located value.
    pub(crate) fn touched_items(&self) -> Vec<Item> {
        let mut items: Vec<Item> = (self.values.keys().chain(self.located.keys()))
            .copied()
            .collect();
        items.sort_unstable();
        items.dedup();
        items
    }

    pub fn activated(&self) -> u64 {
        self.activated
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn in_programmer(&self, fixture: FixtureId) -> bool {
        self.values.contains_key(&Item::fixture(fixture))
    }

    /// PRG-03: the first select after a change starts a new selection; PRG-73: so does a select
    /// into an empty selection, and it ends the grid. Tapping into a selection ends the stage.
    fn begin_select(&mut self) {
        if self.changed {
            self.selection.clear();
            self.select_order.clear();
            self.changed = false;
        }
        if self.selection.is_empty() {
            self.grid = Grid::default();
        }
        self.stage = None;
    }

    pub(crate) fn toggle(&mut self, item: Item) {
        self.begin_select();
        if self.selection.contains(&item) {
            self.selection.retain(|&f| f != item);
            self.select_order.retain(|&f| f != item);
        } else {
            self.selection.push(item);
            self.select_order.push(item);
        }
        self.recompute();
    }

    /// PRG-02 (IDEA-11): append the group's fixtures; if all of them are already selected
    /// (after PRG-03's new-selection check) remove them instead, like `toggle` for one fixture.
    /// PRG-79: into a new selection the group's own grid comes back, with no stage.
    pub(crate) fn add_group(&mut self, members: &[Item], grid: Option<Grid>) {
        self.begin_select();
        if let (true, Some(grid)) = (self.selection.is_empty(), grid) {
            self.grid = grid;
        }
        if !members.is_empty() && members.iter().all(|f| self.selection.contains(f)) {
            self.selection.retain(|f| !members.contains(f));
            self.select_order.retain(|f| !members.contains(f));
        } else {
            for &f in members {
                if !self.selection.contains(&f) {
                    self.selection.push(f);
                    self.select_order.push(f);
                }
            }
        }
        self.recompute();
    }

    /// A new selection: its own select order, no grid, no stage.
    fn start_selection(&mut self, selection: Vec<Item>) {
        self.select_order = selection.clone();
        self.selection = selection;
        self.grid = Grid::default();
        self.stage = None;
        self.changed = false;
        self.recompute();
    }

    /// PRG-08: a typed selection replaces the selection and ends the stage and the grid.
    pub(crate) fn replace_selection(&mut self, selection: Vec<Item>) {
        self.start_selection(selection);
    }

    /// PAT-05: the fixture is gone; drop it from the selection (ending any stage) and the values.
    pub(crate) fn remove_fixture(&mut self, fixture: FixtureId) {
        self.values.retain(|i, _| i.fixture != fixture);
        self.fades.retain(|i, _| i.fixture != fixture);
        self.located.retain(|i, _| i.fixture != fixture);
        self.clearing.retain(|i, _| i.fixture != fixture);
        for e in &mut self.effects {
            e.members.retain(|i| i.fixture != fixture);
            e.refresh();
        }
        self.effects.retain(|e| !e.members.is_empty());
        if self
            .current
            .is_some_and(|c| self.effects.iter().all(|e| e.id != c))
        {
            self.current = None;
        }
        let before = self.selection.len();
        self.selection.retain(|i| i.fixture != fixture);
        if self.selection.len() != before {
            self.select_order.retain(|i| i.fixture != fixture);
            self.stage = None;
            self.recompute();
        }
    }

    /// PAT-13: drop one attribute value of an item.
    pub(crate) fn remove_value(&mut self, item: Item, attribute: &str) {
        take(&mut self.fades, item, attribute);
        take(&mut self.values, item, attribute);
    }

    /// PRG-29: empty the programmer's values and the located set.
    pub(crate) fn clear_values(&mut self) {
        self.values.clear();
        self.fades.clear();
        self.located.clear();
    }

    pub(crate) fn clear_selection(&mut self) {
        self.start_selection(Vec::new());
    }

    /// PRG-71: the order tools put the selection in a new order; the grid and the stage stay.
    /// No programmer change for PRG-03.
    pub(crate) fn set_order(&mut self, selection: Vec<Item>) {
        self.selection = selection;
        self.recompute();
    }

    /// PRG-73/74: set the whole grid (validated by the caller); a grid that is on selects stage 0.
    pub(crate) fn set_grid(&mut self, grid: Grid) {
        self.grid = grid;
        self.stage = self.grid().map(|_| 0);
        self.recompute();
    }

    /// PRG-79: a group's grid back, with no stage.
    pub(crate) fn restore_grid(&mut self, grid: Grid) {
        self.grid = grid;
        self.recompute();
    }

    /// PRG-04/74: step one stage (grid off: one item), forward or back, wrapping.
    pub(crate) fn step(&mut self, forward: bool) {
        let len = self.position_count as usize;
        if self.selection.is_empty() || len == 0 {
            return;
        }
        let bump = |i: usize, n: usize| {
            if forward {
                i.saturating_add(1).checked_rem(n).unwrap_or(0)
            } else {
                i.checked_sub(1).unwrap_or(n.saturating_sub(1))
            }
        };
        self.stage = Some(match self.stage {
            None if forward => 0,
            None => len.saturating_sub(1),
            Some(s) => bump(s, len),
        });
    }

    /// PRG-06/74: end the stage (the grid stays), or else select every item in the programmer.
    pub(crate) fn select_all(&mut self) {
        if self.stage.is_some() {
            self.stage = None;
        } else {
            // PRG-50: every item in the programmer, a whole fixture before its cells.
            self.start_selection(self.values.keys().copied().collect());
        }
    }

    /// PAL-18: references to a deleted palette become their fallback values.
    pub(crate) fn dereference(&mut self, palette: PaletteId) {
        for stored in self.values.values_mut().flat_map(|a| a.values_mut()) {
            *stored = stored.without_palette(palette);
        }
    }

    pub(crate) fn set_value(
        &mut self,
        item: impl Into<Item>,
        attribute: &str,
        value: Stored,
        activated: u64,
    ) {
        let item = item.into();
        self.activated = activated;
        // PAL-16: a new value takes over from the faded one; ENG-10 and PRG-24 likewise.
        take(&mut self.fades, item, attribute);
        take(&mut self.clearing, item, attribute);
        self.remove_located(item, attribute);
        self.values
            .entry(item)
            .or_default()
            .insert(attribute.to_string(), value);
        self.changed = true;
    }
}
