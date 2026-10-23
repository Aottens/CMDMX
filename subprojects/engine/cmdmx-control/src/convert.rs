//! Input readings to action values: buttons, toggles, pickup, wheels (CTL-07, 12, 13).

use crate::action::ValueKind;
use crate::mapping::{Binding, ButtonMode};

/// What an input says, before the action's needs are applied.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Reading {
    Level(f32),
    Button(bool),
    /// Press and release at once (OSC without arguments).
    Tap,
    Steps(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActionValue {
    Level(f32),
    Press,
    Release,
    Steps(i32),
}

/// Step size for a relative input on a continuous level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    Normal,
    Fine,
    Coarse,
}

impl Resolution {
    fn step(self) -> f32 {
        match self {
            Resolution::Normal => 1.0 / 256.0,
            Resolution::Fine => 1.0 / 1024.0,
            Resolution::Coarse => 1.0 / 64.0,
        }
    }
}

/// Soft takeover (CTL-13): a fader only acts once it reaches the engine level.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pickup {
    engaged: bool,
    last_passed: f32,
    prev: Option<f32>,
}

impl Pickup {
    pub fn feed(&mut self, position: f32, target: f32) -> Option<f32> {
        if self.engaged && (target - self.last_passed).abs() > 0.005 {
            self.engaged = false;
        }
        if !self.engaged {
            let crossed = self.prev.is_some_and(|p| {
                (p < target && position > target) || (p > target && position < target)
            });
            self.engaged = (position - target).abs() <= 0.02 || crossed;
        }
        self.prev = Some(position);
        if self.engaged {
            self.last_passed = position;
            Some(position)
        } else {
            None
        }
    }

    pub fn rearm(&mut self) {
        self.engaged = false;
        self.prev = None;
    }
}

/// Per-binding memory between readings.
#[derive(Debug, Clone, Copy, Default)]
pub struct BindingState {
    toggle: bool,
    above: bool,
    pickup: Pickup,
    wheel: Option<f32>,
    remainder: f32,
}

impl BindingState {
    pub fn apply(
        &mut self,
        b: &Binding,
        r: Reading,
        current: Option<f32>,
        res: Resolution,
    ) -> Vec<ActionValue> {
        match b.action.value_kind() {
            ValueKind::Button => self.as_button(b, r),
            ValueKind::Continuous => self.as_level(b, r, current, res),
            ValueKind::Relative => self.as_wheel(r),
        }
    }

    fn press(&mut self, b: &Binding, pressed: bool) -> Vec<ActionValue> {
        match (b.button_mode, pressed) {
            (ButtonMode::Momentary, true) => vec![ActionValue::Press],
            (ButtonMode::Momentary, false) => vec![ActionValue::Release],
            (ButtonMode::Toggle, true) => {
                self.toggle = !self.toggle;
                vec![if self.toggle {
                    ActionValue::Press
                } else {
                    ActionValue::Release
                }]
            }
            (ButtonMode::Toggle, false) => Vec::new(),
        }
    }

    fn as_button(&mut self, b: &Binding, r: Reading) -> Vec<ActionValue> {
        match r {
            Reading::Button(p) => self.press(b, p),
            Reading::Tap => match b.button_mode {
                ButtonMode::Momentary => vec![ActionValue::Press, ActionValue::Release],
                ButtonMode::Toggle => self.press(b, true),
            },
            Reading::Level(l) if l >= 0.5 && !self.above => {
                self.above = true;
                self.press(b, true)
            }
            Reading::Level(l) if l < 0.5 && self.above => {
                self.above = false;
                self.press(b, false)
            }
            _ => Vec::new(),
        }
    }

    fn as_level(
        &mut self,
        b: &Binding,
        r: Reading,
        current: Option<f32>,
        res: Resolution,
    ) -> Vec<ActionValue> {
        match r {
            Reading::Level(l) => match current {
                Some(t) if b.pickup => self
                    .pickup
                    .feed(l, t)
                    .map(ActionValue::Level)
                    .into_iter()
                    .collect(),
                _ => vec![ActionValue::Level(l)],
            },
            Reading::Button(true) => vec![ActionValue::Level(b.max)],
            Reading::Button(false) => vec![ActionValue::Level(b.min)],
            Reading::Tap => vec![ActionValue::Level(b.max), ActionValue::Level(b.min)],
            Reading::Steps(s) => {
                let l = current.unwrap_or(0.0) + s * res.step();
                vec![ActionValue::Level(l.clamp(0.0, 1.0))]
            }
        }
    }

    fn as_wheel(&mut self, r: Reading) -> Vec<ActionValue> {
        let n = match r {
            Reading::Steps(s) => {
                self.remainder += s;
                let whole = self.remainder.trunc();
                self.remainder -= whole;
                whole as i32
            }
            Reading::Level(l) => {
                // 255 steps = one full turn, the engine's Normal wheel step.
                let prev = self.wheel.replace(l);
                prev.map_or(0, |p| {
                    (l * 255.0).round() as i32 - (p * 255.0).round() as i32
                })
            }
            Reading::Button(_) | Reading::Tap => 0,
        };
        if n == 0 {
            Vec::new()
        } else {
            vec![ActionValue::Steps(n)]
        }
    }
}
