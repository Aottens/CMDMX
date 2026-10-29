//! Effects: shape functions, member positions, phase and clock (`specs/modules/effects.md`
//! FX-01..13, FX-30; FXB-01). Pure and deterministic: time comes in as `dt`, never from a clock.
//! The programmer and the merge use these (FXB-02); playbacks follow in FXB-03.

use serde::{Deserialize, Serialize};

use crate::attribute::AttributeGroup;
use crate::colour::TargetColour;
use crate::programmer::Item;
use crate::value::Value;
use crate::CoreError;

use std::f64::consts::TAU;

/// FX-01: scalar waves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Wave {
    Sine,
    Triangle,
    RampUp,
    RampDown,
    Square,
    Random,
}

/// FX-02: position paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Path {
    Circle,
    Eight,
    Square,
    Triangle,
    LinePan,
    LineTilt,
}

/// FX-03: colour shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColourShape {
    Rainbow,
    HueShift,
    ColourMix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Shape {
    Wave(Wave),
    Path(Path),
    Colour(ColourShape),
}

/// FX-05: how the effect sits on the base value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Relative,
    Absolute,
}

/// FX-08.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Forward,
    Backward,
    Bounce,
}

/// FX-06 / FX-09: where the tempo comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpeedSource {
    Free,
    Bpm(u8),
    Rate(u8),
}

/// θ made safe: FX-04 reads a non-finite θ as 0.
fn sane(theta: f64) -> f64 {
    if theta.is_finite() {
        theta
    } else {
        0.0
    }
}

fn frac(theta: f64) -> f64 {
    sane(theta).rem_euclid(1.0)
}

/// splitmix64 finalizer over the three inputs.
fn hash(seed: u64, k: u32, cycle: i64) -> u64 {
    let mut z = seed
        .wrapping_add(u64::from(k).wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add((cycle as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9))
        .wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// FX-01 / FX-04: a wave value in 0..=1. `theta` is unwrapped (cycles); `width` is the
/// fraction a `Square` stays high; `Random` holds one value per whole cycle per position `k`.
pub fn wave(w: Wave, theta: f64, width: f32, seed: u64, k: u32) -> f32 {
    let f = frac(theta) as f32;
    let v = match w {
        Wave::Sine => 0.5 - 0.5 * (TAU * f64::from(f)).cos() as f32,
        Wave::Triangle => 1.0 - (2.0 * f - 1.0).abs(),
        Wave::RampUp => f,
        Wave::RampDown => 1.0 - f,
        Wave::Square => {
            if f < width {
                1.0
            } else {
                0.0
            }
        }
        Wave::Random => {
            let cycle = sane(theta).floor() as i64;
            (hash(seed, k, cycle) >> 40) as f32 / 16_777_215.0
        }
    };
    v.clamp(0.0, 1.0)
}

fn outline(corners: &[(f32, f32)], f: f64) -> (f32, f32) {
    let n = corners.len();
    let p = f * n as f64;
    let i = (p.floor() as usize).min(n.saturating_sub(1));
    let t = (p - i as f64) as f32;
    match (corners.get(i), corners.get((i + 1) % n.max(1))) {
        (Some(a), Some(b)) => (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t),
        _ => (0.0, 0.0),
    }
}

/// FX-02 / FX-04: (x, y) of a path, each in -0.5..=0.5.
pub fn path(p: Path, theta: f64) -> (f32, f32) {
    let f = frac(theta);
    let (s, c) = (TAU * f).sin_cos();
    match p {
        Path::Circle => (0.5 * s as f32, 0.5 * c as f32),
        Path::Eight => (0.5 * s as f32, 0.5 * (2.0 * TAU * f).sin() as f32),
        Path::Square => outline(&[(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)], f),
        Path::Triangle => outline(&[(0.0, 0.5), (0.5, -0.5), (-0.5, -0.5)], f),
        Path::LinePan => (0.5 * s as f32, 0.0),
        Path::LineTilt => (0.0, 0.5 * s as f32),
    }
}

/// FX-02: factor for y so a circle is round on stage: pan degrees / tilt degrees when both
/// are known and positive, else 1.
pub fn round_factor(pan_degrees: Option<f32>, tilt_degrees: Option<f32>) -> f32 {
    match (pan_degrees, tilt_degrees) {
        (Some(p), Some(t)) if p.is_finite() && t.is_finite() && p > 0.0 && t > 0.0 => p / t,
        _ => 1.0,
    }
}

/// FX-03: the target colour of hue θ · 360°, saturation 1, intensity 1.
pub fn rainbow(theta: f64) -> TargetColour {
    TargetColour::from_hsv(frac(theta) as f32, 1.0, 1.0)
}

/// FX-03: the base with its hue turned by `turn` (fractions of the circle); saturation and
/// intensity kept, so white stays white.
pub fn hue_shift(base: TargetColour, turn: f32) -> TargetColour {
    let (h, s, v) = base.to_hsv();
    let turn = if turn.is_finite() { turn } else { 0.0 };
    TargetColour::from_hsv(h + turn, s, v)
}

fn lerp_colour(a: TargetColour, b: TargetColour, t: f32) -> TargetColour {
    TargetColour {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
    }
}

/// FX-03: steps through `colours` once per cycle, fading over the last `crossfade` part of
/// each step (0 snaps). `None` for an empty list.
pub fn colour_mix(colours: &[TargetColour], theta: f64, crossfade: f32) -> Option<TargetColour> {
    let k = colours.len();
    let p = frac(theta) * k as f64;
    let s = (p.floor() as usize).min(k.saturating_sub(1));
    let f = (p - s as f64) as f32;
    let cf = if crossfade.is_finite() {
        crossfade.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let t = if cf > 0.0 {
        ((f - (1.0 - cf)) / cf).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let a = *colours.get(s)?;
    let b = *colours.get((s + 1) % k)?;
    Some(lerp_colour(a, b, t))
}

/// FX-05: Absolute for a wave on an HTP attribute, Rainbow and ColourMix; Relative otherwise.
pub fn default_mode(shape: Shape, htp: bool) -> Mode {
    match shape {
        Shape::Wave(_) if htp => Mode::Absolute,
        Shape::Colour(ColourShape::Rainbow | ColourShape::ColourMix) => Mode::Absolute,
        _ => Mode::Relative,
    }
}

/// FX-05: a scalar wave value on the base. Not clamped here (ENG-16 clamps in the merge).
pub fn scalar(mode: Mode, base: f32, size: f32, top: f32, w: f32) -> f32 {
    match mode {
        Mode::Relative => base + size * (w - 0.5),
        Mode::Absolute => top - size * (1.0 - w),
    }
}

/// FX-05: absolute colour effect: lerp(base, effect, size).
pub fn colour_absolute(base: TargetColour, effect: TargetColour, size: f32) -> TargetColour {
    lerp_colour(base, effect, size)
}

/// FX-06: the values `beats` takes.
pub const BEATS: [f32; 7] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0];

/// FX-06: the parameters of an effect entry.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Params {
    pub size: f32,
    pub top: f32,
    pub bpm: f32,
    pub beats: f32,
    pub speed_source: SpeedSource,
    pub spread: f32,
    pub phase: f32,
    /// Fraction 0.01..=1 (Square).
    pub width: f32,
    /// Fraction 0..=1 (ColourMix).
    pub crossfade: f32,
    pub direction: Direction,
    pub wings: u8,
    pub groups: u8,
    pub blocks: u8,
    pub mode: Mode,
}

impl Params {
    pub const DEFAULT: Params = Params {
        size: 0.5,
        top: 1.0,
        bpm: 120.0,
        beats: 1.0,
        speed_source: SpeedSource::Free,
        spread: 0.0,
        phase: 0.0,
        width: 0.5,
        crossfade: 0.0,
        direction: Direction::Forward,
        wings: 1,
        groups: 0,
        blocks: 1,
        mode: Mode::Relative,
    };

    /// FX-06: refuse anything outside the ranges (NaN included), naming the parameter.
    pub fn validate(&self) -> Result<(), CoreError> {
        fn bad(name: &str, shown: impl std::fmt::Display, range: &str) -> CoreError {
            CoreError::InvalidOption(format!("{name} {shown} (expected {range})"))
        }
        let float = |name: &str, v: f32, lo: f32, hi: f32| {
            if (lo..=hi).contains(&v) {
                Ok(())
            } else {
                Err(bad(name, v, &format!("{lo} to {hi}")))
            }
        };
        let int = |name: &str, v: u8, lo: u8, hi: u8| {
            if (lo..=hi).contains(&v) {
                Ok(())
            } else {
                Err(bad(name, v, &format!("{lo} to {hi}")))
            }
        };
        float("size", self.size, 0.0, 1.0)?;
        float("top", self.top, 0.0, 1.0)?;
        float("bpm", self.bpm, 0.0, 600.0)?;
        if !BEATS.contains(&self.beats) {
            return Err(bad("beats", self.beats, "1/4, 1/2, 1, 2, 4, 8 or 16"));
        }
        match self.speed_source {
            SpeedSource::Free => {}
            SpeedSource::Bpm(n) => int("bpm master", n, 1, 8)?,
            SpeedSource::Rate(n) => int("rate master", n, 1, 4)?,
        }
        float("spread", self.spread, 0.0, 1440.0)?;
        float("phase", self.phase, 0.0, 360.0)?;
        float("width", self.width, 0.01, 1.0)?;
        float("crossfade", self.crossfade, 0.0, 1.0)?;
        int("wings", self.wings, 1, 8)?;
        if self.groups != 0 {
            int("groups", self.groups, 2, 32)?;
        }
        int("blocks", self.blocks, 1, 32)
    }
}

impl Default for Params {
    fn default() -> Self {
        Params::DEFAULT
    }
}

/// FX-07: position `k` per member (in order) and the number of positions `N`.
/// Wings mirror every odd part, groups repeat every `groups`-th member, blocks merge
/// neighbours. Computed when the entry or its members change, never per tick.
pub fn positions(n: usize, wings: u8, groups: u8, blocks: u8) -> (Vec<u32>, u32) {
    if n == 0 {
        return (Vec::new(), 0);
    }
    let w = usize::from(wings.max(1));
    let b = usize::from(blocks.max(1));
    let g = usize::from(groups);
    let l = n.div_ceil(w);
    let mut total = l;
    if g > 0 {
        total = total.min(g);
    }
    let total = total.div_ceil(b);
    let ks = (0..n)
        .map(|i| {
            let mut j = i % l;
            if (i / l) % 2 == 1 {
                j = l - 1 - j;
            }
            if g > 0 {
                j %= g;
            }
            (j / b) as u32
        })
        .collect();
    (ks, total as u32)
}

/// FX-07: φ = spread/360° · k/N (cycles); 0 when there are no positions.
pub fn phase_offset(spread_deg: f32, k: u32, n: u32) -> f64 {
    if n == 0 {
        return 0.0;
    }
    f64::from(spread_deg) / 360.0 * f64::from(k) / f64::from(n)
}

/// FX-08: the member's unwrapped θ; the shape functions take the fraction.
pub fn member_theta(direction: Direction, theta_total: f64, phase_deg: f32, offset: f64) -> f64 {
    let dir = match direction {
        Direction::Forward => theta_total,
        Direction::Backward => -theta_total,
        Direction::Bounce => {
            let u = sane(theta_total).rem_euclid(2.0);
            if u < 1.0 {
                u
            } else {
                2.0 - u
            }
        }
    };
    dir + f64::from(phase_deg) / 360.0 - offset
}

/// FX-09: cycles per second, `bpm / (60 · beats)`; 0 for a bad `beats` or `bpm`.
pub fn rate(bpm: f32, beats: f32) -> f64 {
    if !(beats.is_finite() && beats > 0.0 && bpm.is_finite() && bpm > 0.0) {
        return 0.0;
    }
    f64::from(bpm) / (60.0 * f64::from(beats))
}

/// FX-09..11: an entry's clock, θ_total in cycles. A speed change never jumps the phase.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Clock {
    theta: f64,
}

impl Clock {
    pub fn new() -> Clock {
        Clock { theta: 0.0 }
    }

    pub fn theta(&self) -> f64 {
        self.theta
    }

    /// FX-09: advance by `dt` seconds at `rate` cycles per second. A negative or non-finite
    /// `dt` or a non-finite `rate` changes nothing.
    pub fn advance(&mut self, dt: f64, rate: f64) {
        if dt.is_finite() && dt >= 0.0 && rate.is_finite() {
            self.theta += dt * rate;
        }
    }

    /// FX-10: θ ← round(θ · beats) / beats.
    pub fn beat_sync(&mut self, beats: f32) {
        if beats.is_finite() && beats > 0.0 {
            let b = f64::from(beats);
            self.theta = (self.theta * b).round() / b;
        }
    }

    /// FX-11.
    pub fn restart(&mut self) {
        self.theta = 0.0;
    }
}

pub type EntryId = u32;

/// FX-14: a programmer effect entry. `attributes` is `["Pan", "Tilt"]` for a path, `["Colour"]`
/// for a colour shape, else the wave's attribute; `positions`/`count` are FX-07's *k* per member
/// and *N*, kept up to date by [`Entry::refresh`], never computed per tick.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub id: EntryId,
    pub look: Option<String>,
    pub shape: Shape,
    pub attributes: Vec<String>,
    pub params: Params,
    pub members: Vec<Item>,
    pub positions: Vec<u32>,
    pub count: u32,
    pub seed: u64,
    /// FX-03: the `ColourMix` list; empty for other shapes.
    pub colours: Vec<TargetColour>,
    /// The attribute group of `attributes`, for FX-19.
    pub(crate) group: AttributeGroup,
}

/// FX-14: what `effect.apply` makes an entry from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectApply {
    Look(String),
    /// A wave without an attribute means Dimmer.
    Shape {
        shape: Shape,
        attribute: Option<String>,
    },
}

/// FX-16: one parameter of the current entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectSet {
    Size(f32),
    Top(f32),
    Bpm(f32),
    Beats(f32),
    Spread(f32),
    Phase(f32),
    Width(f32),
    Crossfade(f32),
    SpeedSource(SpeedSource),
    Direction(Direction),
    Mode(Mode),
    Wings(u8),
    Groups(u8),
    Blocks(u8),
    Colours(Vec<TargetColour>),
}

impl EffectSet {
    /// The parameter's name, for undo labels and coalescing keys (FX-20).
    pub fn name(&self) -> &'static str {
        match self {
            EffectSet::Size(_) => "size",
            EffectSet::Top(_) => "top",
            EffectSet::Bpm(_) => "bpm",
            EffectSet::Beats(_) => "beats",
            EffectSet::Spread(_) => "spread",
            EffectSet::Phase(_) => "phase",
            EffectSet::Width(_) => "width",
            EffectSet::Crossfade(_) => "crossfade",
            EffectSet::SpeedSource(_) => "speed_source",
            EffectSet::Direction(_) => "direction",
            EffectSet::Mode(_) => "mode",
            EffectSet::Wings(_) => "wings",
            EffectSet::Groups(_) => "groups",
            EffectSet::Blocks(_) => "blocks",
            EffectSet::Colours(_) => "colours",
        }
    }
}

/// ENG-16: each component into 0..=1 (a NaN becomes 0).
fn clamp01(v: Value) -> Value {
    let c = |x: f32| {
        if x.is_finite() {
            x.clamp(0.0, 1.0)
        } else {
            0.0
        }
    };
    match v {
        Value::Level(l) => Value::Level(c(l)),
        Value::Colour(t) => Value::Colour(TargetColour {
            r: c(t.r),
            g: c(t.g),
            b: c(t.b),
        }),
    }
}

impl Entry {
    /// FX-07: recompute *k* and *N* after the members or the parameters changed.
    pub(crate) fn refresh(&mut self) {
        let p = &self.params;
        (self.positions, self.count) = positions(self.members.len(), p.wings, p.groups, p.blocks);
    }

    /// ENG-14..16: `base` with member `i`'s effect on `attribute` at clock `theta`, clamped;
    /// `base` itself for an attribute the entry does not drive. `round` is FX-02's factor for
    /// Tilt. Allocation-free (FX-13).
    pub(crate) fn at(
        &self,
        i: usize,
        theta: f64,
        attribute: &str,
        base: Value,
        round: f32,
    ) -> Value {
        if !self.attributes.iter().any(|a| a == attribute) {
            return base;
        }
        let p = &self.params;
        let k = self.positions.get(i).copied().unwrap_or(0);
        let t = member_theta(
            p.direction,
            theta,
            p.phase,
            phase_offset(p.spread, k, self.count),
        );
        let v = match (self.shape, base) {
            (Shape::Wave(w), Value::Level(b)) => Value::Level(scalar(
                p.mode,
                b,
                p.size,
                p.top,
                wave(w, t, p.width, self.seed, k),
            )),
            // ponytail: an Absolute path acts as Relative; absolute positions come with a need.
            (Shape::Path(shape), Value::Level(b)) => {
                let (x, y) = path(shape, t);
                if attribute == "Pan" {
                    Value::Level(b + p.size * x)
                } else {
                    Value::Level(b + p.size * y * round)
                }
            }
            (Shape::Colour(shape), Value::Colour(b)) => Value::Colour(match shape {
                ColourShape::Rainbow => colour_absolute(b, rainbow(t), p.size),
                ColourShape::ColourMix => colour_mix(&self.colours, t, p.crossfade)
                    .map_or(b, |m| colour_absolute(b, m, p.size)),
                ColourShape::HueShift => {
                    hue_shift(b, p.size * (wave(Wave::Sine, t, 0.5, 0, 0) - 0.5))
                }
            }),
            _ => return base,
        };
        clamp01(v)
    }
}

/// FX-30: a built-in look. Looks are code, not stored; the entry is.
#[derive(Debug, Clone, Copy)]
pub struct Look {
    pub name: &'static str,
    pub group: AttributeGroup,
    pub shape: Shape,
    /// The attribute a wave look drives; path and colour looks use the group's attributes.
    pub attribute: Option<&'static str>,
    pub params: Params,
    /// Square width is 100 % / N once the selection is known.
    pub width_one_per_position: bool,
}

const fn p(size: f32, bpm: f32, spread: f32, mode: Mode) -> Params {
    Params {
        size,
        bpm,
        spread,
        mode,
        ..Params::DEFAULT
    }
}

const fn look(
    name: &'static str,
    group: AttributeGroup,
    shape: Shape,
    attribute: Option<&'static str>,
    params: Params,
) -> Look {
    Look {
        name,
        group,
        shape,
        attribute,
        params,
        width_one_per_position: false,
    }
}

use AttributeGroup::{B, C, I, P};
use Mode::{Absolute as Abs, Relative as Rel};

/// FX-30: the gallery, in this order.
pub static LOOKS: &[Look] = &[
    Look {
        width_one_per_position: true,
        ..look(
            "Dimmer chase",
            I,
            Shape::Wave(Wave::Square),
            Some("Dimmer"),
            p(1.0, 120.0, 360.0, Abs),
        )
    },
    look(
        "Dimmer wave",
        I,
        Shape::Wave(Wave::Sine),
        Some("Dimmer"),
        p(1.0, 60.0, 360.0, Abs),
    ),
    look(
        "Breathe",
        I,
        Shape::Wave(Wave::Sine),
        Some("Dimmer"),
        p(0.5, 20.0, 0.0, Abs),
    ),
    look(
        "Sparkle",
        I,
        Shape::Wave(Wave::Random),
        Some("Dimmer"),
        p(1.0, 240.0, 360.0, Abs),
    ),
    look(
        "Comet",
        I,
        Shape::Wave(Wave::RampDown),
        Some("Dimmer"),
        p(1.0, 90.0, 360.0, Abs),
    ),
    look(
        "Circle",
        P,
        Shape::Path(Path::Circle),
        None,
        p(0.2, 20.0, 0.0, Rel),
    ),
    look(
        "Eight",
        P,
        Shape::Path(Path::Eight),
        None,
        p(0.25, 15.0, 0.0, Rel),
    ),
    look(
        "Fan sweep",
        P,
        Shape::Path(Path::LinePan),
        None,
        Params {
            wings: 2,
            ..p(0.3, 15.0, 180.0, Rel)
        },
    ),
    look(
        "Tilt wave",
        P,
        Shape::Path(Path::LineTilt),
        None,
        p(0.2, 20.0, 360.0, Rel),
    ),
    look(
        "Rainbow",
        C,
        Shape::Colour(ColourShape::Rainbow),
        None,
        p(1.0, 6.0, 0.0, Abs),
    ),
    look(
        "Rainbow wave",
        C,
        Shape::Colour(ColourShape::Rainbow),
        None,
        p(1.0, 6.0, 360.0, Abs),
    ),
    look(
        "Colour chase",
        C,
        Shape::Colour(ColourShape::ColourMix),
        None,
        p(1.0, 60.0, 360.0, Abs),
    ),
    look(
        "Colour fade",
        C,
        Shape::Colour(ColourShape::ColourMix),
        None,
        Params {
            crossfade: 1.0,
            ..p(1.0, 10.0, 0.0, Abs)
        },
    ),
    look(
        "Zoom pulse",
        B,
        Shape::Wave(Wave::Sine),
        Some("Zoom"),
        p(0.3, 30.0, 0.0, Rel),
    ),
];
