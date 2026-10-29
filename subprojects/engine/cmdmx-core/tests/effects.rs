//! Effect core: `specs/modules/effects.md` FX-01..13 and FX-30 (FXB-01). Pure functions only;
//! the programmer, the merge and playbacks come with FXB-02/03.
//!
//! API under test (`cmdmx_core::effect`):
//! - `Wave`, `Path`, `ColourShape`, `Shape` (FX-01..03).
//! - `wave(w, theta, width, seed, k) -> f32`: θ unwrapped (cycles); the shape uses frac(θ),
//!   `Random` also the whole cycle ⌊θ⌋ (FX-01). `width` is a fraction (Square).
//! - `path(p, theta) -> (f32, f32)`, `round_factor(pan_degrees, tilt_degrees) -> f32` (FX-02).
//! - `rainbow(theta)`, `hue_shift(base, turn)`, `colour_mix(&colours, theta, crossfade)
//!   -> Option<TargetColour>` (FX-03).
//! - `Mode`, `default_mode(shape, htp)`, `scalar(mode, base, size, top, w)`,
//!   `colour_absolute(base, effect, size)` (FX-05; no clamp here, ENG-16 clamps in the merge).
//! - `Params` with `Default` and `validate() -> Result<(), CoreError>`; `Direction`,
//!   `SpeedSource` (FX-06).
//! - `positions(n, wings, groups, blocks, flip) -> (Vec<u32>, u32)`, `phase_offset(spread_deg, k, n)`
//!   (FX-07); `member_theta(direction, theta_total, phase_deg, offset) -> f64` (FX-08).
//! - `rate(bpm, beats) -> f64`, `Clock` with `new`, `theta`, `advance(dt, rate)`,
//!   `beat_sync(beats)`, `restart` (FX-09..11).
//! - `LOOKS: &[Look]` (FX-30).

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use cmdmx_core::attribute::AttributeGroup;
use cmdmx_core::colour::TargetColour;
use cmdmx_core::effect::{
    colour_absolute, colour_mix, default_mode, hue_shift, member_theta, path, phase_offset,
    positions, rainbow, rate, round_factor, scalar, wave, Clock, ColourShape, Direction, Mode,
    Params, Path, Shape, Wave, LOOKS,
};

const E: f32 = 1e-5;

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < E
}

fn close_colour(a: TargetColour, b: TargetColour) -> bool {
    close(a.r, b.r) && close(a.g, b.g) && close(a.b, b.b)
}

fn rgb(r: f32, g: f32, b: f32) -> TargetColour {
    TargetColour { r, g, b }
}

fn w(shape: Wave, theta: f64) -> f32 {
    wave(shape, theta, 0.5, 7, 0)
}

// ---------- FX-01 waves ----------

#[test]
fn fx_01_sine_starts_dark_and_tops_at_half() {
    for (t, v) in [(0.0, 0.0), (0.25, 0.5), (0.5, 1.0), (0.75, 0.5)] {
        assert!(close(w(Wave::Sine, t), v), "Sine({t})");
    }
}

#[test]
fn fx_01_triangle_and_ramps() {
    for (t, v) in [(0.0, 0.0), (0.25, 0.5), (0.5, 1.0), (0.75, 0.5)] {
        assert!(close(w(Wave::Triangle, t), v), "Triangle({t})");
    }
    assert!(close(w(Wave::RampUp, 0.25), 0.25));
    assert!(close(w(Wave::RampDown, 0.0), 1.0));
    assert!(close(w(Wave::RampDown, 0.25), 0.75));
}

#[test]
fn fx_01_square_follows_width() {
    assert!(close(wave(Wave::Square, 0.25, 0.5, 0, 0), 1.0));
    assert!(close(wave(Wave::Square, 0.5, 0.5, 0, 0), 0.0));
    assert!(close(wave(Wave::Square, 0.75, 0.5, 0, 0), 0.0));
    assert!(close(wave(Wave::Square, 0.2, 0.25, 0, 0), 1.0));
    assert!(close(wave(Wave::Square, 0.3, 0.25, 0, 0), 0.0));
}

#[test]
fn fx_01_waves_use_the_fraction_of_theta() {
    assert!(close(w(Wave::Sine, 1.25), 0.5));
    assert!(close(w(Wave::Sine, -0.25), 0.5));
    assert!(close(w(Wave::RampUp, 3.25), 0.25));
}

#[test]
fn fx_01_random_holds_per_cycle_and_is_seeded() {
    let r = |theta: f64, seed: u64, k: u32| wave(Wave::Random, theta, 0.5, seed, k);
    for c in 0..20 {
        let v = r(f64::from(c) + 0.1, 1, 0);
        assert!((0.0..=1.0).contains(&v));
        assert_eq!(
            v.to_bits(),
            r(f64::from(c) + 0.9, 1, 0).to_bits(),
            "held over cycle {c}"
        );
    }
    let cycles: Vec<u32> = (0..20).map(|c| r(f64::from(c), 1, 0).to_bits()).collect();
    assert!(
        cycles.windows(2).any(|p| p[0] != p[1]),
        "a new value per cycle"
    );
    let seeds: Vec<u32> = (0..20).map(|s| r(0.5, s, 0).to_bits()).collect();
    assert!(seeds.windows(2).any(|p| p[0] != p[1]), "the seed matters");
    let members: Vec<u32> = (0..20).map(|k| r(0.5, 1, k).to_bits()).collect();
    assert!(
        members.windows(2).any(|p| p[0] != p[1]),
        "the position matters"
    );
    assert_eq!(r(5.5, 3, 2).to_bits(), r(5.5, 3, 2).to_bits());
}

// ---------- FX-02 paths ----------

fn close_xy(a: (f32, f32), b: (f32, f32)) -> bool {
    close(a.0, b.0) && close(a.1, b.1)
}

#[test]
fn fx_02_circle_and_eight() {
    assert!(close_xy(path(Path::Circle, 0.0), (0.0, 0.5)));
    assert!(close_xy(path(Path::Circle, 0.25), (0.5, 0.0)));
    assert!(close_xy(path(Path::Circle, 0.5), (0.0, -0.5)));
    let h = 0.5 * std::f32::consts::FRAC_1_SQRT_2;
    assert!(close_xy(path(Path::Eight, 0.125), (h, 0.5)));
    assert!(close_xy(path(Path::Eight, 0.25), (0.5, 0.0)));
}

#[test]
fn fx_02_square_and_triangle_outline() {
    for (t, xy) in [
        (0.0, (-0.5, -0.5)),
        (0.125, (0.0, -0.5)),
        (0.25, (0.5, -0.5)),
        (0.5, (0.5, 0.5)),
        (0.75, (-0.5, 0.5)),
        (0.875, (-0.5, 0.0)),
    ] {
        assert!(close_xy(path(Path::Square, t), xy), "Square({t})");
    }
    assert!(close_xy(path(Path::Triangle, 0.0), (0.0, 0.5)));
    assert!(close_xy(path(Path::Triangle, 1.0 / 3.0), (0.5, -0.5)));
    assert!(close_xy(path(Path::Triangle, 0.5), (0.0, -0.5)));
    assert!(close_xy(path(Path::Triangle, 2.0 / 3.0), (-0.5, -0.5)));
}

#[test]
fn fx_02_lines() {
    assert!(close_xy(path(Path::LinePan, 0.25), (0.5, 0.0)));
    assert!(close_xy(path(Path::LineTilt, 0.25), (0.0, 0.5)));
    assert!(close_xy(path(Path::LineTilt, 0.0), (0.0, 0.0)));
}

#[test]
fn fx_02_round_on_stage() {
    assert!(close(round_factor(Some(540.0), Some(270.0)), 2.0));
    assert!(close(round_factor(Some(540.0), None), 1.0));
    assert!(close(round_factor(None, Some(270.0)), 1.0));
    assert!(close(round_factor(Some(540.0), Some(0.0)), 1.0));
}

// ---------- FX-03 colours ----------

#[test]
fn fx_03_rainbow_runs_the_hue_circle() {
    assert!(close_colour(rainbow(0.0), rgb(1.0, 0.0, 0.0)));
    assert!(close_colour(rainbow(1.0 / 3.0), rgb(0.0, 1.0, 0.0)));
    assert!(close_colour(rainbow(2.0 / 3.0), rgb(0.0, 0.0, 1.0)));
    assert!(close_colour(rainbow(1.0), rgb(1.0, 0.0, 0.0)));
}

#[test]
fn fx_03_hue_shift_keeps_saturation_and_shows_nothing_on_white() {
    assert!(close_colour(
        hue_shift(rgb(1.0, 0.0, 0.0), 1.0 / 3.0),
        rgb(0.0, 1.0, 0.0)
    ));
    assert!(close_colour(
        hue_shift(rgb(1.0, 0.0, 0.0), -1.0 / 3.0),
        rgb(0.0, 0.0, 1.0)
    ));
    assert!(close_colour(
        hue_shift(TargetColour::WHITE, 0.25),
        TargetColour::WHITE
    ));
}

#[test]
fn fx_03_colour_mix_steps_and_fades() {
    let (red, blue) = (rgb(1.0, 0.0, 0.0), rgb(0.0, 0.0, 1.0));
    let two = [red, blue];
    let m = |theta, cf| colour_mix(&two, theta, cf).unwrap();
    assert!(close_colour(m(0.25, 0.0), red));
    assert!(close_colour(m(0.75, 0.0), blue));
    assert!(close_colour(m(0.0, 1.0), red));
    assert!(close_colour(m(0.25, 1.0), rgb(0.5, 0.0, 0.5)));
    assert!(close_colour(m(0.1, 0.5), red));
    assert!(close_colour(m(0.375, 0.5), rgb(0.5, 0.0, 0.5)));
    let three = [red, rgb(0.0, 1.0, 0.0), blue];
    assert!(close_colour(colour_mix(&three, 0.9, 0.0).unwrap(), blue));
    assert!(close_colour(
        colour_mix(&three, 0.9, 1.0).unwrap(),
        rgb(0.7, 0.0, 0.3)
    ));
    assert!(close_colour(colour_mix(&[red], 0.5, 1.0).unwrap(), red));
    assert!(colour_mix(&[], 0.5, 1.0).is_none());
}

// ---------- FX-04 total functions ----------

#[test]
fn fx_04_non_finite_theta_reads_as_zero() {
    for t in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(close(w(Wave::Sine, t), w(Wave::Sine, 0.0)));
        assert!(close(w(Wave::RampDown, t), 1.0));
        let r = wave(Wave::Random, t, 0.5, 1, 0);
        assert_eq!(r.to_bits(), wave(Wave::Random, 0.0, 0.5, 1, 0).to_bits());
        assert!(close_xy(path(Path::Circle, t), (0.0, 0.5)));
        assert!(close_colour(rainbow(t), rgb(1.0, 0.0, 0.0)));
    }
    for s in [
        Wave::Sine,
        Wave::Triangle,
        Wave::RampUp,
        Wave::RampDown,
        Wave::Square,
    ] {
        for i in 0..1000 {
            let v = wave(s, f64::from(i) * 0.0137 - 5.0, 0.3, 0, 0);
            assert!(v.is_finite() && (0.0..=1.0).contains(&v), "{s:?}");
        }
    }
}

// ---------- FX-05 value on the base ----------

#[test]
fn fx_05_relative_runs_around_the_base() {
    assert!(close(scalar(Mode::Relative, 0.5, 0.2, 1.0, 1.0), 0.6));
    assert!(close(scalar(Mode::Relative, 0.5, 0.2, 1.0, 0.0), 0.4));
    assert!(close(scalar(Mode::Relative, 0.5, 0.2, 1.0, 0.5), 0.5));
}

#[test]
fn fx_05_absolute_hangs_from_top_whatever_the_base() {
    for base in [0.0, 0.3, 1.0] {
        assert!(close(scalar(Mode::Absolute, base, 1.0, 1.0, 0.0), 0.0));
        assert!(close(scalar(Mode::Absolute, base, 1.0, 1.0, 1.0), 1.0));
        assert!(close(scalar(Mode::Absolute, base, 0.5, 1.0, 0.0), 0.5));
    }
    assert!(close(scalar(Mode::Absolute, 0.0, 0.5, 0.8, 0.0), 0.3));
}

#[test]
fn fx_05_colour_absolute_mixes_by_size() {
    let (base, e) = (rgb(1.0, 1.0, 1.0), rgb(0.0, 0.0, 1.0));
    assert!(close_colour(colour_absolute(base, e, 1.0), e));
    assert!(close_colour(colour_absolute(base, e, 0.0), base));
    assert!(close_colour(
        colour_absolute(base, e, 0.5),
        rgb(0.5, 0.5, 1.0)
    ));
}

#[test]
fn fx_05_default_modes() {
    assert_eq!(default_mode(Shape::Wave(Wave::Sine), true), Mode::Absolute);
    assert_eq!(default_mode(Shape::Wave(Wave::Sine), false), Mode::Relative);
    assert_eq!(
        default_mode(Shape::Path(Path::Circle), false),
        Mode::Relative
    );
    assert_eq!(
        default_mode(Shape::Colour(ColourShape::Rainbow), false),
        Mode::Absolute
    );
    assert_eq!(
        default_mode(Shape::Colour(ColourShape::ColourMix), false),
        Mode::Absolute
    );
    assert_eq!(
        default_mode(Shape::Colour(ColourShape::HueShift), false),
        Mode::Relative
    );
}

// ---------- FX-06 parameters ----------

#[test]
fn fx_06_defaults_are_valid() {
    let p = Params::default();
    assert!(p.validate().is_ok());
    assert!(close(p.top, 1.0));
    assert!(close(p.beats, 1.0));
    assert_eq!(p.direction, Direction::Forward);
    assert_eq!((p.wings, p.groups, p.blocks), (1, 0, 1));
}

type Setter = fn(&mut Params);

#[test]
fn fx_06_out_of_range_is_refused() {
    let bad: Vec<(&str, Setter)> = vec![
        ("size", |p| p.size = 1.2),
        ("size", |p| p.size = -0.1),
        ("size", |p| p.size = f32::NAN),
        ("bpm", |p| p.bpm = 601.0),
        ("bpm", |p| p.bpm = -1.0),
        ("beats", |p| p.beats = 3.0),
        ("spread", |p| p.spread = 1441.0),
        ("phase", |p| p.phase = 361.0),
        ("width", |p| p.width = 0.0),
        ("width", |p| p.width = 1.01),
        ("crossfade", |p| p.crossfade = 1.5),
        ("wings", |p| p.wings = 0),
        ("wings", |p| p.wings = 9),
        ("groups", |p| p.groups = 1),
        ("groups", |p| p.groups = 33),
        ("blocks", |p| p.blocks = 0),
        ("blocks", |p| p.blocks = 33),
        ("top", |p| p.top = 1.5),
    ];
    for (what, set) in bad {
        let mut p = Params::default();
        set(&mut p);
        assert!(p.validate().is_err(), "{what} should be refused");
    }
    let mut ok = Params::default();
    for beats in [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0] {
        ok.beats = beats;
        assert!(ok.validate().is_ok(), "beats {beats}");
    }
    ok.groups = 2;
    ok.spread = 1440.0;
    ok.phase = 360.0;
    ok.bpm = 0.0;
    assert!(ok.validate().is_ok());
}

// ---------- FX-07 member positions ----------

#[test]
fn fx_07_plain_spread_over_the_order() {
    assert_eq!(positions(4, 1, 0, 1, false), (vec![0, 1, 2, 3], 4));
    assert_eq!(positions(1, 1, 0, 1, false), (vec![0], 1));
    assert_eq!(positions(0, 1, 0, 1, false), (vec![], 0));
}

#[test]
fn fx_07_wings_mirror_every_odd_part() {
    assert_eq!(
        positions(8, 2, 0, 1, false),
        (vec![0, 1, 2, 3, 3, 2, 1, 0], 4)
    );
    assert_eq!(positions(5, 2, 0, 1, false), (vec![0, 1, 2, 2, 1], 3));
    assert_eq!(
        positions(9, 3, 0, 1, false),
        (vec![0, 1, 2, 2, 1, 0, 0, 1, 2], 3)
    );
}

#[test]
fn fx_07_groups_and_blocks() {
    assert_eq!(positions(6, 1, 3, 1, false), (vec![0, 1, 2, 0, 1, 2], 3));
    assert_eq!(positions(4, 1, 8, 1, false), (vec![0, 1, 2, 3], 4));
    assert_eq!(positions(6, 1, 0, 2, false), (vec![0, 0, 1, 1, 2, 2], 3));
    assert_eq!(
        positions(8, 2, 0, 2, false),
        (vec![0, 0, 1, 1, 1, 1, 0, 0], 2)
    );
}

#[test]
fn fx_07_phase_offsets_from_spread() {
    assert!((phase_offset(360.0, 1, 4) - 0.25).abs() < 1e-9);
    assert!((phase_offset(360.0, 3, 4) - 0.75).abs() < 1e-9);
    assert!(phase_offset(0.0, 3, 4).abs() < 1e-9);
    assert!((phase_offset(720.0, 1, 4) - 0.5).abs() < 1e-9);
    assert!(phase_offset(360.0, 0, 0).abs() < 1e-9);
}

// ---------- FX-08 phase per member ----------

#[test]
fn fx_08_forward_first_member_leads() {
    let offsets: Vec<f64> = (0..4).map(|k| phase_offset(360.0, k, 4)).collect();
    let at = |theta: f64, i: usize| {
        w(
            Wave::Sine,
            member_theta(Direction::Forward, theta, 0.0, offsets[i]),
        )
    };
    assert!(close(at(0.5, 0), 1.0), "201 on top at half a cycle");
    assert!(close(at(0.75, 1), 1.0), "202 a quarter later");
    assert!(close(at(1.25, 3), 1.0), "204 three quarters later");
}

#[test]
fn fx_08_backward_and_phase() {
    let f = |d, t, ph| member_theta(d, t, ph, 0.0).rem_euclid(1.0);
    assert!((f(Direction::Backward, 0.25, 0.0) - 0.75).abs() < 1e-9);
    assert!((f(Direction::Forward, 0.1, 90.0) - 0.35).abs() < 1e-9);
}

#[test]
fn fx_08_bounce_goes_there_and_back() {
    let f = |t| member_theta(Direction::Bounce, t, 0.0, 0.0);
    assert!((f(0.3) - 0.3).abs() < 1e-9);
    assert!((f(1.3) - 0.7).abs() < 1e-9);
    assert!((f(2.3) - 0.3).abs() < 1e-9);
}

// ---------- FX-09..11 clock ----------

#[test]
fn fx_09_clock_advances_by_rate_without_jumps() {
    assert!((rate(120.0, 1.0) - 2.0).abs() < 1e-9);
    assert!((rate(120.0, 4.0) - 0.5).abs() < 1e-9);
    let mut c = Clock::new();
    assert_eq!(c.theta(), 0.0);
    c.advance(0.5, rate(120.0, 1.0));
    assert!((c.theta() - 1.0).abs() < 1e-9);
    c.advance(0.25, rate(60.0, 1.0));
    assert!(
        (c.theta() - 1.25).abs() < 1e-9,
        "a tempo change keeps the phase"
    );
    c.advance(10.0, rate(0.0, 1.0));
    assert!((c.theta() - 1.25).abs() < 1e-9, "BPM 0 stops");
    for dt in [-1.0, f64::NAN, f64::INFINITY] {
        c.advance(dt, 2.0);
        assert!((c.theta() - 1.25).abs() < 1e-9, "dt {dt} ignored");
    }
    c.advance(1.0, f64::NAN);
    assert!(
        (c.theta() - 1.25).abs() < 1e-9,
        "a non-finite rate is ignored"
    );
}

#[test]
fn fx_10_beat_sync_rounds_to_the_nearest_beat() {
    let at = |theta: f64, beats: f32| {
        let mut c = Clock::new();
        c.advance(theta, 1.0);
        c.beat_sync(beats);
        c.theta()
    };
    assert!((at(1.3, 1.0) - 1.0).abs() < 1e-9);
    assert!((at(1.6, 1.0) - 2.0).abs() < 1e-9);
    assert!((at(1.3, 4.0) - 1.25).abs() < 1e-9);
}

#[test]
fn fx_11_restart_sets_zero() {
    let mut c = Clock::new();
    c.advance(3.7, 1.0);
    c.restart();
    assert_eq!(c.theta(), 0.0);
}

#[test]
fn fx_12_same_inputs_same_bits() {
    let run = || {
        let mut c = Clock::new();
        let mut out = Vec::new();
        let (k, n) = positions(12, 2, 0, 1, false);
        for tick in 0..2000u16 {
            c.advance(0.025, rate(97.3 + f32::from(tick % 7), 2.0));
            for (i, kk) in k.iter().enumerate() {
                let th = member_theta(
                    Direction::Bounce,
                    c.theta(),
                    33.0,
                    phase_offset(360.0, *kk, n),
                );
                out.push(wave(Wave::Random, th, 0.5, 42, *kk).to_bits());
                out.push(path(Path::Eight, th).0.to_bits());
                out.push(rainbow(th + i as f64).g.to_bits());
            }
        }
        out
    };
    assert_eq!(run(), run());
}

#[test]
#[ignore = "benchmark (FX-13), run with --ignored --release"]
fn fx_13_five_hundred_members_fit_a_tick() {
    let (k, n) = positions(500, 1, 0, 1, false);
    let offsets: Vec<f64> = k.iter().map(|kk| phase_offset(360.0, *kk, n)).collect();
    let mut c = Clock::new();
    let start = std::time::Instant::now();
    let mut sink = 0.0f32;
    let ticks = 400;
    for _ in 0..ticks {
        c.advance(0.025, rate(120.0, 1.0));
        for off in &offsets {
            let th = member_theta(Direction::Forward, c.theta(), 0.0, *off);
            sink += wave(Wave::Sine, th, 0.5, 0, 0);
            let (x, y) = path(Path::Circle, th);
            sink += x + y;
        }
    }
    let per_tick = start.elapsed() / ticks;
    assert!(sink.is_finite());
    assert!(
        per_tick < std::time::Duration::from_millis(2),
        "{per_tick:?} per tick"
    );
}

// ---------- FX-30 looks ----------

#[test]
fn fx_30_the_gallery() {
    let names: Vec<&str> = LOOKS.iter().map(|l| l.name).collect();
    assert_eq!(
        names,
        [
            "Dimmer chase",
            "Dimmer wave",
            "Breathe",
            "Sparkle",
            "Comet",
            "Circle",
            "Eight",
            "Fan sweep",
            "Tilt wave",
            "Rainbow",
            "Rainbow wave",
            "Colour chase",
            "Colour fade",
            "Zoom pulse",
        ]
    );
    let look = |name: &str| LOOKS.iter().find(|l| l.name == name).unwrap();

    let chase = look("Dimmer chase");
    assert_eq!(chase.group, AttributeGroup::I);
    assert_eq!(chase.shape, Shape::Wave(Wave::Square));
    assert_eq!(chase.attribute, Some("Dimmer"));
    assert_eq!(chase.params.mode, Mode::Absolute);
    assert!(close(chase.params.size, 1.0) && close(chase.params.bpm, 120.0));
    assert!(close(chase.params.spread, 360.0));
    assert!(chase.width_one_per_position, "width 100 %/N");

    let circle = look("Circle");
    assert_eq!(
        (circle.group, circle.shape),
        (AttributeGroup::P, Shape::Path(Path::Circle))
    );
    assert_eq!(circle.attribute, None);
    assert_eq!(circle.params.mode, Mode::Relative);
    assert!(close(circle.params.size, 0.2) && close(circle.params.bpm, 20.0));

    let fan = look("Fan sweep");
    assert_eq!(fan.shape, Shape::Path(Path::LinePan));
    assert_eq!(fan.params.wings, 2);
    assert!(close(fan.params.spread, 180.0));

    let fade = look("Colour fade");
    assert_eq!(
        (fade.group, fade.shape),
        (AttributeGroup::C, Shape::Colour(ColourShape::ColourMix))
    );
    assert!(close(fade.params.crossfade, 1.0) && close(fade.params.bpm, 10.0));
    assert!(close(look("Colour chase").params.crossfade, 0.0));

    let zoom = look("Zoom pulse");
    assert_eq!(
        (zoom.group, zoom.attribute),
        (AttributeGroup::B, Some("Zoom"))
    );
    assert_eq!(zoom.params.mode, Mode::Relative);

    for l in LOOKS {
        assert!(l.params.validate().is_ok(), "{} validates", l.name);
        assert!(!l.width_one_per_position || l.name == "Dimmer chase");
    }
}
