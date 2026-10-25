//! The programmer: selection, sub-selection and manual values.
//!
//! Implements `specs/modules/programmer.md` PRG-01..07, PRG-14 and the located set (PRG-23..25,
//! 38); the merge reads its values at `Priority::Programmer` (PRG-12/13). A value is manual or a palette
//! reference (PAL-08). The view also carries the values and the picker colour (IPC-21, IPC-22).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::attribute::AttributeGroup;
use crate::colour::ColourModel;
use crate::palette::PaletteId;
use crate::patch::FixtureId;
use crate::value::{Stored, Value};
use crate::Timestamp;

pub type GroupId = u32;

/// PRG-46: a selection item, a whole fixture (`cell == None`) or one cell (1-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Item {
    pub fixture: FixtureId,
    pub cell: Option<u16>,
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

/// Wheel step size (PRG-21); the sizes live in the engine `Settings`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Resolution {
    Normal,
    Fine,
    Coarse,
}

/// Narrowing of the selection for stepping (PRG-04) and patterns (PRG-05).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
enum Sub {
    #[default]
    None,
    Step(usize),
    Pattern {
        stages: usize,
        stage: usize,
    },
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
}

/// IPC-21: one fixture's programmer values.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FixtureValues {
    pub fixture: FixtureId,
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
}

/// PRG-35 / IPC-11: everything the UI renders of the programmer.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProgrammerView {
    pub selection: Vec<FixtureId>,
    pub targets: Vec<FixtureId>,
    pub active_group: AttributeGroup,
    pub attribute_groups: Vec<AttributeGroupState>,
    pub wheels: Vec<WheelState>,
    pub in_programmer: Vec<FixtureId>,
    pub colour_model: ColourModel,
    pub contents: Vec<FixtureValues>,
    pub colour: Option<ColourView>,
    pub palettes: PaletteUse,
    /// PRG-23: fixture → sorted names of the located attributes (IPC-33).
    pub located: BTreeMap<FixtureId, Vec<String>>,
    /// PRG-26.
    pub highlight: bool,
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
    sub: Sub,
    changed: bool,
    values: BTreeMap<FixtureId, BTreeMap<String, Stored>>,
    located: BTreeMap<FixtureId, BTreeMap<String, Value>>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Programmer {
    selection: Vec<Item>,
    sub: Sub,
    /// Set by a value change; the next select starts a new selection (PRG-03).
    changed: bool,
    values: BTreeMap<FixtureId, BTreeMap<String, Stored>>,
    /// Running palette fades (PAL-13..16); an entry always has a value here.
    fades: BTreeMap<FixtureId, BTreeMap<String, Fade>>,
    /// Located values (PRG-23): not contents, under the programmer value in the merge (PRG-25).
    located: BTreeMap<FixtureId, BTreeMap<String, Value>>,
    /// Fades of values a timed Clear removed (ENG-10): live state; HTP `from` → 0, LTP `from`
    /// → what the other sources give. Never has a programmer value of its own.
    clearing: BTreeMap<FixtureId, BTreeMap<String, Fade>>,
    /// Engine activation counter at the last value change: orders the
    /// programmer against other sources in LTP ties (ENG-04).
    activated: u64,
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
            sub: self.sub,
            changed: self.changed,
            values: self.values.clone(),
            located: self.located.clone(),
        }
    }

    /// UNDO-12: put a saved part back. A palette fade survives only while its value is unchanged
    /// (a fade always has a value); `activated` stays.
    pub(crate) fn restore_edit_part(&mut self, part: EditPart) {
        let old = std::mem::replace(&mut self.values, part.values);
        self.selection = part.selection;
        self.sub = part.sub;
        self.changed = part.changed;
        self.located = part.located;
        self.clearing.clear(); // ENG-10: undo during a timed Clear puts the programmer back at once
        let values = &self.values;
        for (fixture, attrs) in &mut self.fades {
            attrs.retain(|a, _| {
                let get = |m: &BTreeMap<FixtureId, BTreeMap<String, Stored>>| {
                    m.get(fixture).and_then(|v| v.get(a)).copied()
                };
                get(values).is_some() && get(values) == get(&old)
            });
        }
        self.fades.retain(|_, a| !a.is_empty());
    }

    /// PRG-46: the selected items in selection order.
    pub fn items(&self) -> &[Item] {
        &self.selection
    }

    /// The whole-fixture items of the selection (cells act with VS2-31).
    pub fn selection(&self) -> Vec<FixtureId> {
        whole(&self.selection)
    }

    /// PRG-04/05/50: the items that receive the next change: the sub-selection, else all.
    pub fn target_items(&self) -> Vec<Item> {
        match self.sub {
            Sub::None => self.selection.clone(),
            Sub::Step(i) => self.selection.get(i).copied().into_iter().collect(),
            Sub::Pattern { stages, stage } => self
                .selection
                .iter()
                .enumerate()
                .filter(|(i, _)| i.checked_rem(stages) == Some(stage))
                .map(|(_, f)| *f)
                .collect(),
        }
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
        self.values.get(&fixture)?.get(attribute).copied()
    }

    pub fn values(&self) -> &BTreeMap<FixtureId, BTreeMap<String, Stored>> {
        &self.values
    }

    pub(crate) fn fade(&self, fixture: FixtureId, attribute: &str) -> Option<&Fade> {
        self.fades.get(&fixture)?.get(attribute)
    }

    pub(crate) fn set_fade(&mut self, fixture: FixtureId, attribute: &str, fade: Fade) {
        self.fades
            .entry(fixture)
            .or_default()
            .insert(attribute.to_string(), fade);
    }

    /// Called at the start of a tick: fades that have not begun start now (PAL-13).
    pub(crate) fn start_fades(&mut self, now: Timestamp) {
        let all = self.fades.values_mut().chain(self.clearing.values_mut());
        for fade in all.flat_map(|a| a.values_mut()) {
            fade.start.get_or_insert(now);
        }
    }

    /// Called at the end of a tick: finished fades are dropped.
    pub(crate) fn drop_finished_fades(&mut self, now: Timestamp) {
        for fades in [&mut self.fades, &mut self.clearing] {
            for attrs in fades.values_mut() {
                attrs.retain(|_, f| {
                    !f.start
                        .is_some_and(|s| (now - s) as f32 >= f.delay + f.duration)
                });
            }
            fades.retain(|_, a| !a.is_empty());
        }
    }

    /// ENG-10: the fade of a value a timed Clear removed.
    pub(crate) fn clearing(&self, fixture: FixtureId, attribute: &str) -> Option<&Fade> {
        self.clearing.get(&fixture)?.get(attribute)
    }

    pub(crate) fn set_clearing(&mut self, fixture: FixtureId, attribute: &str, fade: Fade) {
        self.clearing
            .entry(fixture)
            .or_default()
            .insert(attribute.to_string(), fade);
    }

    pub fn located(&self) -> &BTreeMap<FixtureId, BTreeMap<String, Value>> {
        &self.located
    }

    pub(crate) fn located_value(&self, fixture: FixtureId, attribute: &str) -> Option<Value> {
        self.located.get(&fixture)?.get(attribute).copied()
    }

    /// PRG-38: Locate again overwrites. The attribute is touched, so no clear fade runs on it.
    pub(crate) fn set_located(&mut self, fixture: FixtureId, attribute: &str, value: Value) {
        if let Some(attrs) = self.clearing.get_mut(&fixture) {
            attrs.remove(attribute);
        }
        self.located
            .entry(fixture)
            .or_default()
            .insert(attribute.to_string(), value);
    }

    pub(crate) fn remove_located(&mut self, fixture: FixtureId, attribute: &str) {
        if let Some(attrs) = self.located.get_mut(&fixture) {
            attrs.remove(attribute);
            if attrs.is_empty() {
                self.located.remove(&fixture);
            }
        }
    }

    /// PRG-03: Locate and Rem Dim count as a change, so the next select starts afresh.
    pub(crate) fn mark_changed(&mut self) {
        self.changed = true;
    }

    /// Every fixture with a value or a located value.
    pub(crate) fn touched_fixtures(&self) -> Vec<FixtureId> {
        let mut ids: Vec<FixtureId> = self
            .values
            .keys()
            .chain(self.located.keys())
            .copied()
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    pub fn activated(&self) -> u64 {
        self.activated
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn in_programmer(&self, fixture: FixtureId) -> bool {
        self.values.contains_key(&fixture)
    }

    /// PRG-03: the first select after a change starts a new selection.
    fn begin_select(&mut self) {
        if self.changed {
            self.selection.clear();
            self.changed = false;
        }
        self.sub = Sub::None;
    }

    pub(crate) fn toggle(&mut self, item: Item) {
        self.begin_select();
        match self.selection.iter().position(|&f| f == item) {
            Some(i) => {
                self.selection.remove(i);
            }
            None => self.selection.push(item),
        }
    }

    /// PRG-02 (IDEA-11): append the group's fixtures; if all of them are already selected
    /// (after PRG-03's new-selection check) remove them instead, like `toggle` for one fixture.
    pub(crate) fn add_group(&mut self, members: &[Item]) {
        self.begin_select();
        if !members.is_empty() && members.iter().all(|f| self.selection.contains(f)) {
            self.selection.retain(|f| !members.contains(f));
            return;
        }
        for &f in members {
            if !self.selection.contains(&f) {
                self.selection.push(f);
            }
        }
    }

    /// PRG-08: a typed selection replaces the selection and ends any sub-selection.
    pub(crate) fn replace_selection(&mut self, selection: Vec<Item>) {
        self.selection = selection;
        self.sub = Sub::None;
        self.changed = false;
    }

    /// PAT-05: the fixture is gone; drop it from the selection (ending any sub-selection) and the values.
    pub(crate) fn remove_fixture(&mut self, fixture: FixtureId) {
        self.values.remove(&fixture);
        self.fades.remove(&fixture);
        self.located.remove(&fixture);
        self.clearing.remove(&fixture);
        let before = self.selection.len();
        self.selection.retain(|i| i.fixture != fixture);
        if self.selection.len() != before {
            self.sub = Sub::None;
        }
    }

    /// PAT-13: drop one attribute value of a fixture.
    pub(crate) fn remove_value(&mut self, fixture: FixtureId, attribute: &str) {
        if let Some(attrs) = self.fades.get_mut(&fixture) {
            attrs.remove(attribute);
        }
        if let Some(attrs) = self.values.get_mut(&fixture) {
            attrs.remove(attribute);
            if attrs.is_empty() {
                self.values.remove(&fixture);
            }
        }
    }

    /// PRG-29: empty the programmer's values and the located set.
    pub(crate) fn clear_values(&mut self) {
        self.values.clear();
        self.fades.clear();
        self.located.clear();
    }

    pub(crate) fn clear_selection(&mut self) {
        self.selection.clear();
        self.sub = Sub::None;
        self.changed = false;
    }

    /// PRG-04: step one fixture, or one pattern stage, forward or back, wrapping.
    pub(crate) fn step(&mut self, forward: bool) {
        let len = self.selection.len();
        if len == 0 {
            return;
        }
        let bump = |i: usize, n: usize| {
            if forward {
                i.saturating_add(1).checked_rem(n).unwrap_or(0)
            } else {
                i.checked_sub(1).unwrap_or(n.saturating_sub(1))
            }
        };
        self.sub = match self.sub {
            Sub::None => Sub::Step(if forward { 0 } else { len.saturating_sub(1) }),
            Sub::Step(i) => Sub::Step(bump(i, len)),
            Sub::Pattern { stages, stage } => Sub::Pattern {
                stages,
                stage: bump(stage, stages),
            },
        };
    }

    pub(crate) fn set_pattern(&mut self, stages: usize) {
        self.sub = Sub::Pattern { stages, stage: 0 };
    }

    /// PRG-06: end the sub-selection, or else select every fixture in the programmer.
    pub(crate) fn select_all(&mut self) {
        if self.sub != Sub::None {
            self.sub = Sub::None;
        } else {
            self.selection = self.values.keys().map(|&f| Item::fixture(f)).collect();
            self.changed = false;
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
        fixture: FixtureId,
        attribute: &str,
        value: Stored,
        activated: u64,
    ) {
        self.activated = activated;
        // PAL-16: a new value takes over from the faded one; ENG-10 and PRG-24 likewise.
        for fades in [&mut self.fades, &mut self.clearing] {
            if let Some(attrs) = fades.get_mut(&fixture) {
                attrs.remove(attribute);
            }
        }
        self.remove_located(fixture, attribute);
        self.values
            .entry(fixture)
            .or_default()
            .insert(attribute.to_string(), value);
        self.changed = true;
    }
}
