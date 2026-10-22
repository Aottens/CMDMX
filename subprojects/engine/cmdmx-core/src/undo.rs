//! Undo history (`specs/modules/undo.md`, ADR-008): snapshots of the edit state, no clock.

use std::collections::VecDeque;

use crate::engine::{EditState, Engine};
use crate::CoreError;

/// UNDO-08: steps kept in each list.
pub const DEPTH: usize = 100;

#[derive(Debug)]
struct Step {
    label: String,
    state: EditState,
}

#[derive(Debug, Default)]
pub struct History {
    undo: VecDeque<Step>,
    redo: VecDeque<Step>,
    /// UNDO-05: the coalescing key of the last edit, `None` after anything else.
    last_key: Option<String>,
}

fn push(list: &mut VecDeque<Step>, step: Step) {
    if list.len() >= DEPTH {
        list.pop_front();
    }
    list.push_back(step);
}

impl History {
    /// UNDO-01, 05, 07: run one edit command; a step is added only if it succeeded and changed
    /// the edit state.
    pub fn edit<T>(
        &mut self,
        engine: &mut Engine,
        label: &str,
        key: Option<&str>,
        f: impl FnOnce(&mut Engine) -> Result<T, CoreError>,
    ) -> Result<T, CoreError> {
        let before = engine.edit_state();
        let result = f(engine);
        if result.is_err() || engine.edit_state() == before {
            if key != self.last_key.as_deref() {
                self.last_key = None;
            }
            return result;
        }
        let merge = key.is_some() && self.last_key.as_deref() == key && !self.undo.is_empty();
        if !merge {
            push(
                &mut self.undo,
                Step {
                    label: label.to_string(),
                    state: before,
                },
            );
            self.redo.clear();
        }
        self.last_key = key.map(str::to_string);
        result
    }

    /// UNDO-05: a live command ends a coalescing run.
    pub fn live(&mut self) {
        self.last_key = None;
    }

    /// UNDO-06: put back the newest step's state; returns its label.
    pub fn undo(&mut self, engine: &mut Engine) -> Result<String, CoreError> {
        let step = self.undo.pop_back().ok_or(CoreError::NothingToUndo)?;
        self.swap(engine, step, false)
    }

    pub fn redo(&mut self, engine: &mut Engine) -> Result<String, CoreError> {
        let step = self.redo.pop_back().ok_or(CoreError::NothingToRedo)?;
        self.swap(engine, step, true)
    }

    /// Restore `step`'s state and keep the state it replaced on the other list.
    fn swap(
        &mut self,
        engine: &mut Engine,
        step: Step,
        undoing: bool,
    ) -> Result<String, CoreError> {
        let current = engine.edit_state();
        engine.restore_edit_state(step.state);
        let other = if undoing {
            &mut self.undo
        } else {
            &mut self.redo
        };
        let label = step.label.clone();
        push(
            other,
            Step {
                label: step.label,
                state: current,
            },
        );
        self.last_key = None;
        Ok(label)
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.undo.back().map(|s| s.label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.redo.back().map(|s| s.label.as_str())
    }

    pub fn len(&self) -> usize {
        self.undo.len()
    }

    pub fn is_empty(&self) -> bool {
        self.undo.is_empty()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    /// UNDO-13.
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.last_key = None;
    }
}
