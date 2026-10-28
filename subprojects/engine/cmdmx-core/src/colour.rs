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

/// CIE xy (Y = 1) as linear BT.2020 RGB scaled so its largest component is 1; negative
/// components are kept. `None` when `cie` is not a point or has no positive component.
fn xy_to_rgb([x, y]: [f32; 2]) -> Option<[f32; 3]> {
    if y <= 0.0 || !x.is_finite() || !y.is_finite() {
        return None;
    }
    let (xx, zz) = (x / y, (1.0 - x - y) / y);
    let mut rgb = [
        1.716_651_2 * xx - 0.355_670_8 - 0.253_366_3 * zz,
        -0.666_684_4 * xx + 1.616_481_2 + 0.015_768_5 * zz,
        0.017_639_9 * xx - 0.042_770_6 + 0.942_103_1 * zz,
    ];
    let max = rgb.iter().copied().fold(0.0_f32, f32::max);
    if max <= 0.0 || !max.is_finite() {
        return None;
    }
    for c in &mut rgb {
        *c /= max;
    }
    Some(rgb)
}

/// ENG-20: the linear BT.2020 RGB an emitter at CIE xy `cie` adds, scaled so its largest
/// component is 1. Components under 0.05 are 0 (an emitter that is not quite on the gamut edge
/// still mixes); `None` when it is outside the gamut (below -0.05, e.g. UV, deep indigo) or
/// `cie` is not a point.
pub fn rgb_equivalent(cie: [f32; 2]) -> Option<[f32; 3]> {
    let rgb = xy_to_rgb(cie)?;
    if rgb.iter().any(|&c| c < -0.05) {
        return None;
    }
    // Rounded to 0.001 so float noise cannot tip a channel across a DMX rounding step (D65 is exactly 1, 1, 1).
    Some(rgb.map(|c| {
        if c < 0.05 {
            0.0
        } else {
            (c * 1000.0).round() / 1000.0
        }
    }))
}

/// LIB-37: the default chromaticity of a well-known emitter name, filled in when auto-mix is
/// switched on in the profile editor.
pub fn default_cie(name: &str) -> Option<[f32; 2]> {
    const TABLE: [(&str, [f32; 2]); 5] = [
        ("White", [0.3127, 0.3290]),
        ("Cold White", [0.3135, 0.3237]),
        ("Warm White", [0.4366, 0.4042]),
        ("Amber", [0.5752, 0.4242]),
        ("Lime", [0.4100, 0.5500]),
    ];
    TABLE
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|&(_, cie)| cie)
}

/// Which wheels the Colour attribute is shown and turned as (PRG-17, PRG-30). Display only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColourModel {
    #[default]
    Hsi,
    Rgb,
    Cmy,
    /// PRG-63: one wheel along the Planckian locus.
    Kelvin,
}

impl ColourModel {
    /// The wheel names this model replaces Colour with, in wheel order.
    pub fn wheels(self) -> &'static [&'static str] {
        match self {
            ColourModel::Hsi => &["Hue", "Saturation"],
            ColourModel::Rgb => &["Red", "Green", "Blue"],
            ColourModel::Cmy => &["Cyan", "Magenta", "Yellow"],
            ColourModel::Kelvin => &["Kelvin"],
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
            (ColourModel::Kelvin, "Kelvin") => {
                (MIRED_AT_0 - 1e6 / kelvin(colour).unwrap_or(6500.0)) / MIRED_SPAN
            }
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
            (ColourModel::Kelvin, "Kelvin") => {
                let value = (ColourModel::Kelvin.wheel_value("Kelvin", c)? + delta).clamp(0.0, 1.0);
                let k = 1e6 / (MIRED_AT_0 - value * MIRED_SPAN);
                planck(k).scaled(max_or_one(c))
            }
            _ => return None,
        })
    }
}

/// PRG-63: the Kelvin wheel runs 1800 K (0.0) to 10000 K (1.0), linear in mired.
const MIRED_AT_0: f32 = 555.6;
const MIRED_SPAN: f32 = 455.6;

/// The largest component, or 1.0 for black (a Kelvin move out of black comes up at full).
fn max_or_one(c: TargetColour) -> f32 {
    let m = c.r.max(c.g).max(c.b);
    if m > 0.0 {
        m
    } else {
        1.0
    }
}

/// CIE xy of the Planckian locus at `k` (Kim et al. 2002 spline), `k` clamped to 1667..=25000.
fn locus_xy(k: f64) -> (f64, f64) {
    let t = k.clamp(1667.0, 25000.0);
    let (t2, t3) = (t * t, t * t * t);
    let x = if t <= 4000.0 {
        -0.2661239e9 / t3 - 0.2343589e6 / t2 + 0.8776956e3 / t + 0.179910
    } else {
        -3.0258469e9 / t3 + 2.1070379e6 / t2 + 0.2226347e3 / t + 0.240390
    };
    let (x2, x3) = (x * x, x * x * x);
    let y = if t <= 2222.0 {
        -1.1063814 * x3 - 1.34811020 * x2 + 2.18555832 * x - 0.20219683
    } else if t <= 4000.0 {
        -0.9549476 * x3 - 1.37418593 * x2 + 2.09137015 * x - 0.16748867
    } else {
        3.0817580 * x3 - 5.87338670 * x2 + 3.75112997 * x - 0.37001483
    };
    (x, y)
}

/// CIE 1960 uv of xy.
fn xy_to_uv(x: f64, y: f64) -> (f64, f64) {
    let d = -2.0 * x + 12.0 * y + 3.0;
    (4.0 * x / d, 6.0 * y / d)
}

/// PRG-63: the Planckian-locus colour at `kelvin` (clamped to 1667..=25000) as linear BT.2020,
/// largest component 1.
pub fn planck(kelvin: f32) -> TargetColour {
    let (x, y) = locus_xy(f64::from(kelvin));
    let [r, g, b] = xy_to_rgb([x as f32, y as f32])
        .map(|rgb| rgb.map(|c| c.max(0.0)))
        .unwrap_or([1.0, 1.0, 1.0]);
    let max = r.max(g).max(b);
    TargetColour {
        r: r / max,
        g: g / max,
        b: b / max,
    }
}

/// PRG-63: the temperature of the Planckian-locus point nearest to `c` in CIE 1960 uv, clamped
/// to 1800..=10000 K; `None` for black. Coarse scan over the spline's mired range, then a
/// fixed-count ternary refinement (no allocation).
pub fn kelvin(c: TargetColour) -> Option<f32> {
    if c.r.max(c.g).max(c.b) <= 0.0 {
        return None;
    }
    let (r, g, b) = (f64::from(c.r), f64::from(c.g), f64::from(c.b));
    let x = 0.6369580 * r + 0.1446169 * g + 0.1688810 * b;
    let y = 0.2627002 * r + 0.6779981 * g + 0.0593017 * b;
    let z = 0.0280727 * g + 1.0609851 * b;
    let d = x + 15.0 * y + 3.0 * z;
    let (u, v) = (4.0 * x / d, 6.0 * y / d);
    let dist = |mired: f64| {
        let (lx, ly) = locus_xy(1e6 / mired);
        let (lu, lv) = xy_to_uv(lx, ly);
        (lu - u).powi(2) + (lv - v).powi(2)
    };
    const STEP: f64 = 2.0;
    let (mut best, mut best_d) = (40.0, f64::MAX);
    for i in 0..=280 {
        let m = 40.0 + STEP * f64::from(i);
        let dm = dist(m);
        if dm < best_d {
            (best, best_d) = (m, dm);
        }
    }
    let (mut lo, mut hi) = ((best - STEP).max(40.0), (best + STEP).min(600.0));
    for _ in 0..40 {
        let (a, b) = (lo + (hi - lo) / 3.0, hi - (hi - lo) / 3.0);
        if dist(a) < dist(b) {
            hi = b;
        } else {
            lo = a;
        }
    }
    Some(((1e6 / ((lo + hi) / 2.0)) as f32).clamp(1800.0, 10000.0))
}

/// PRG-65/66: a CTO or CTB step, in mired (positive is warmer). CTO is +159/+81/+42/+20,
/// CTB is −137/−78/−35/−18.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CtStep {
    CtoFull,
    CtoHalf,
    CtoQuarter,
    CtoEighth,
    CtbFull,
    CtbHalf,
    CtbQuarter,
    CtbEighth,
}

impl CtStep {
    pub fn mired(self) -> f32 {
        match self {
            CtStep::CtoFull => 159.0,
            CtStep::CtoHalf => 81.0,
            CtStep::CtoQuarter => 42.0,
            CtStep::CtoEighth => 20.0,
            CtStep::CtbFull => -137.0,
            CtStep::CtbHalf => -78.0,
            CtStep::CtbQuarter => -35.0,
            CtStep::CtbEighth => -18.0,
        }
    }
}
