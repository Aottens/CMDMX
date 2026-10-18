//! The programmer: selection, sub-selection and manual values.
//!
//! Implements `specs/modules/programmer.md` PRG-01..07 and PRG-14; the merge
//! reads its values at `Priority::Programmer` (PRG-12/13). A value is manual or a palette
//! reference (PAL-08). The view also carries the values and the picker colour (IPC-21, IPC-22).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::attribute::AttributeGroup;
use crate::colour::ColourModel;
use crate::colour::TargetColour;
use crate::palette::PaletteId;
use crate::patch::FixtureId;
use crate::value::{Stored, Value};
use crate::Timestamp;

pub type GroupId = u32;

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
}

/// IPC-21: one fixture's programmer values.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FixtureValues {
    pub fixture: FixtureId,
    pub values: BTreeMap<String, Value>,
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

    /// PAL-13: linear from `from` to `to`; Colour per component in linear target RGB.
    pub(crate) fn at(&self, to: Value, now: Timestamp) -> Value {
        let Some(e) = self.elapsed(now) else {
            return self.from;
        };
        let k = (e / self.duration).min(1.0);
        match (self.from, to) {
            (Value::Level(a), Value::Level(b)) => Value::Level(a + (b - a) * k),
            (Value::Colour(a), Value::Colour(b)) => Value::Colour(TargetColour {
                r: a.r + (b.r - a.r) * k,
                g: a.g + (b.g - a.g) * k,
                b: a.b + (b.b - a.b) * k,
            }),
            _ => to,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Programmer {
    selection: Vec<FixtureId>,
    sub: Sub,
    /// Set by a value change; the next select starts a new selection (PRG-03).
    changed: bool,
    values: BTreeMap<FixtureId, BTreeMap<String, Stored>>,
    /// Running palette fades (PAL-13..16); an entry always has a value here.
    fades: BTreeMap<FixtureId, BTreeMap<String, Fade>>,
    /// Engine activation counter at the last value change: orders the
    /// programmer against other sources in LTP ties (ENG-04).
    activated: u64,
}

impl Programmer {
    pub fn selection(&self) -> &[FixtureId] {
        &self.selection
    }

    /// Fixtures that receive the next change: the sub-selection, else everything selected.
    pub fn targets(&self) -> Vec<FixtureId> {
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
        for fade in self.fades.values_mut().flat_map(|a| a.values_mut()) {
            fade.start.get_or_insert(now);
        }
    }

    /// Called at the end of a tick: finished fades are dropped.
    pub(crate) fn drop_finished_fades(&mut self, now: Timestamp) {
        for attrs in self.fades.values_mut() {
            attrs.retain(|_, f| {
                !f.start
                    .is_some_and(|s| (now - s) as f32 >= f.delay + f.duration)
            });
        }
        self.fades.retain(|_, a| !a.is_empty());
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

    pub(crate) fn toggle(&mut self, fixture: FixtureId) {
        self.begin_select();
        match self.selection.iter().position(|&f| f == fixture) {
            Some(i) => {
                self.selection.remove(i);
            }
            None => self.selection.push(fixture),
        }
    }

    /// PRG-02 (IDEA-11): append the group's fixtures; if all of them are already selected
    /// (after PRG-03's new-selection check) remove them instead, like `toggle` for one fixture.
    pub(crate) fn add_group(&mut self, members: &[FixtureId]) {
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
    pub(crate) fn replace_selection(&mut self, selection: Vec<FixtureId>) {
        self.selection = selection;
        self.sub = Sub::None;
        self.changed = false;
    }

    /// PAT-05: the fixture is gone; drop it from the selection (ending any sub-selection) and the values.
    pub(crate) fn remove_fixture(&mut self, fixture: FixtureId) {
        self.values.remove(&fixture);
        self.fades.remove(&fixture);
        let before = self.selection.len();
        self.selection.retain(|&f| f != fixture);
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

    /// PRG-29: empty the programmer's values.
    pub(crate) fn clear_values(&mut self) {
        self.values.clear();
        self.fades.clear();
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
            self.selection = self.values.keys().copied().collect();
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
        // PAL-16: a new value takes over from the faded one.
        if let Some(attrs) = self.fades.get_mut(&fixture) {
            attrs.remove(attribute);
        }
        self.values
            .entry(fixture)
            .or_default()
            .insert(attribute.to_string(), value);
        self.changed = true;
    }
}
