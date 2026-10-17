//! Colour as one value (ADR-005): a linear RGB target plus the model the wheels show it in
//! (`specs/modules/programmer.md` PRG-30..34). HSI, RGB and CMY are views of the same colour.

use serde::{Deserialize, Serialize};

/// A colour as linear 0.0..=1.0 RGB (ADR-005): a wheel slot, a macro or the Colour attribute.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TargetColour {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl TargetColour {
    /// Open white: what Colour outputs until a source proposes one (LIB-03).
    pub const WHITE: TargetColour = TargetColour {
        r: 1.0,
        g: 1.0,
        b: 1.0,
    };

    /// Hue as a fraction of 360°, saturation `(max − min) / max` (0 for black), and
    /// value `max(r, g, b)` (PRG-30). The hue of a grey is 0.
    pub fn to_hsv(self) -> (f32, f32, f32) {
        let max = self.r.max(self.g).max(self.b);
        let d = max - self.r.min(self.g).min(self.b);
        let sector = if d <= 0.0 {
            0.0
        } else if max == self.r {
            ((self.g - self.b) / d).rem_euclid(6.0)
        } else if max == self.g {
            (self.b - self.r) / d + 2.0
        } else {
            (self.r - self.g) / d + 4.0
        };
        let s = if max <= 0.0 { 0.0 } else { d / max };
        (sector / 6.0, s, max)
    }

    /// Inverse of [`TargetColour::to_hsv`]; `h` wraps, `s` and `v` clamp to 0.0..=1.0.
    pub fn from_hsv(h: f32, s: f32, v: f32) -> TargetColour {
        let (s, v) = (s.clamp(0.0, 1.0), v.clamp(0.0, 1.0));
        let h6 = h.rem_euclid(1.0) * 6.0;
        let channel = |n: f32| {
            let k = (n + h6).rem_euclid(6.0);
            (v - v * s * k.min(4.0 - k).clamp(0.0, 1.0)).clamp(0.0, 1.0)
        };
        TargetColour {
            r: channel(5.0),
            g: channel(3.0),
            b: channel(1.0),
        }
    }

    /// Every component multiplied by `factor`.
    pub fn scaled(self, factor: f32) -> TargetColour {
        TargetColour {
            r: self.r * factor,
            g: self.g * factor,
            b: self.b * factor,
        }
    }
}

/// Which wheels the Colour attribute is shown and turned as (PRG-17, PRG-30). Display only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColourModel {
    #[default]
    Hsi,
    Rgb,
    Cmy,
}

impl ColourModel {
    /// The wheel names this model replaces Colour with, in wheel order.
    pub fn wheels(self) -> &'static [&'static str] {
        match self {
            ColourModel::Hsi => &["Hue", "Saturation"],
            ColourModel::Rgb => &["Red", "Green", "Blue"],
            ColourModel::Cmy => &["Cyan", "Magenta", "Yellow"],
        }
    }

    /// Position of `wheel` for `colour` (Hue as a fraction of 360°); `None` if it is
    /// not a wheel of this model.
    pub fn wheel_value(self, wheel: &str, colour: TargetColour) -> Option<f32> {
        let (h, s, _) = colour.to_hsv();
        Some(match (self, wheel) {
            (ColourModel::Hsi, "Hue") => h,
            (ColourModel::Hsi, "Saturation") => s,
            (ColourModel::Rgb, "Red") => colour.r,
            (ColourModel::Rgb, "Green") => colour.g,
            (ColourModel::Rgb, "Blue") => colour.b,
            (ColourModel::Cmy, "Cyan") => 1.0 - colour.r,
            (ColourModel::Cmy, "Magenta") => 1.0 - colour.g,
            (ColourModel::Cmy, "Yellow") => 1.0 - colour.b,
            _ => return None,
        })
    }

    /// Move `wheel` by `delta` (PRG-32): Hue wraps, everything else clamps; Hue and
    /// Saturation keep the brightest component. `None` if it is not a wheel of this model.
    pub fn turn(self, wheel: &str, colour: TargetColour, delta: f32) -> Option<TargetColour> {
        let (h, s, v) = colour.to_hsv();
        let clamp = |x: f32| x.clamp(0.0, 1.0);
        let c = colour;
        Some(match (self, wheel) {
            (ColourModel::Hsi, "Hue") => TargetColour::from_hsv(h + delta, s, v),
            (ColourModel::Hsi, "Saturation") => TargetColour::from_hsv(h, s + delta, v),
            (ColourModel::Rgb, "Red") => TargetColour {
                r: clamp(c.r + delta),
                ..c
            },
            (ColourModel::Rgb, "Green") => TargetColour {
                g: clamp(c.g + delta),
                ..c
            },
            (ColourModel::Rgb, "Blue") => TargetColour {
                b: clamp(c.b + delta),
                ..c
            },
            (ColourModel::Cmy, "Cyan") => TargetColour {
                r: clamp(c.r - delta),
                ..c
            },
            (ColourModel::Cmy, "Magenta") => TargetColour {
                g: clamp(c.g - delta),
                ..c
            },
            (ColourModel::Cmy, "Yellow") => TargetColour {
                b: clamp(c.b - delta),
                ..c
            },
            _ => return None,
        })
    }
}
