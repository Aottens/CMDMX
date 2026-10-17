//! The programmer: selection, sub-selection and manual values.
//!
//! Implements `specs/modules/programmer.md` PRG-01..07 and PRG-14. Palette
//! references and merge output come with VS2-04.

use std::collections::BTreeMap;

use crate::patch::FixtureId;

pub type GroupId = u32;

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

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Programmer {
    selection: Vec<FixtureId>,
    sub: Sub,
    /// Set by a value change; the next select starts a new selection (PRG-03).
    changed: bool,
    values: BTreeMap<FixtureId, BTreeMap<String, f32>>,
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

    pub fn value(&self, fixture: FixtureId, attribute: &str) -> Option<f32> {
        self.values.get(&fixture)?.get(attribute).copied()
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

    pub(crate) fn add_group(&mut self, members: &[FixtureId]) {
        self.begin_select();
        for &f in members {
            if !self.selection.contains(&f) {
                self.selection.push(f);
            }
        }
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

    pub(crate) fn set_value(&mut self, fixture: FixtureId, attribute: &str, value: f32) {
        self.values
            .entry(fixture)
            .or_default()
            .insert(attribute.to_string(), value);
        self.changed = true;
    }
}
