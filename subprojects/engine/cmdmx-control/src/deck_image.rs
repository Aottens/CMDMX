//! What a Stream Deck key shows and how it is drawn (CTL-50, 51). Pure and deterministic: the
//! same look and model always give the same bytes (integer pixel math, fontdue on libm).

use std::sync::OnceLock;

use fontdue::{Font, FontSettings};
use jpeg_encoder::{ColorType, Encoder, SamplingFactor};

use crate::action::Action;
use crate::deck::{ImageFormat, Model};
use crate::mapping::Binding;

pub type Rgb = [u8; 3];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    Off,
    Inactive,
    Active,
    Held,
}

/// Everything the renderer needs for one key.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyLook {
    pub label: String,
    pub sub: String,
    pub halo: Option<Rgb>,
    pub level: Option<f32>,
    pub state: KeyState,
    /// The key's action does not follow the console page (CTL-50): drawn with a lock.
    pub fixed: bool,
}

impl KeyLook {
    pub fn black() -> KeyLook {
        KeyLook {
            label: String::new(),
            sub: String::new(),
            halo: None,
            level: None,
            state: KeyState::Off,
            fixed: false,
        }
    }
}

/// A playback as the key sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackView {
    pub id: u32,
    pub legend: String,
    pub halo: Option<Rgb>,
    pub level: f32,
    pub running: bool,
    pub function: Option<String>,
    pub held: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Subject {
    Empty,
    Playback(PlaybackView),
    Other {
        name: Option<String>,
        active: bool,
        level: Option<f32>,
    },
}

/// `#rrggbb`; anything else is None.
fn parse_colour(s: &str) -> Option<Rgb> {
    let hex = s.strip_prefix('#').filter(|h| h.len() == 6)?;
    let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

/// Label for an action that has no name of its own.
fn action_label(action: &Action) -> String {
    let text = action.to_string();
    match text.as_str() {
        "gm" => return "GM".to_string(),
        "dbo" => return "DBO".to_string(),
        _ => {}
    }
    if let Some(rest) = text.strip_prefix("deck page ") {
        return format!("Deck {rest}");
    }
    let rest = text.strip_prefix("key ").unwrap_or(&text);
    let rest = text.strip_prefix("keypad ").unwrap_or(rest);
    capitalise(&rest.replace('_', " "))
}

/// The key's look from its binding and what it is bound to (CTL-50).
pub fn key_look(binding: Option<&Binding>, subject: &Subject) -> KeyLook {
    let Some(b) = binding.filter(|b| b.feedback) else {
        return KeyLook::black();
    };
    let colour = b.colour.as_deref().and_then(parse_colour);
    match subject {
        Subject::Empty => KeyLook::black(),
        Subject::Playback(v) => {
            let label = match (&b.label, v.legend.as_str()) {
                (Some(l), _) => l.clone(),
                (None, "") => format!("PB {}", v.id),
                (None, l) => l.to_string(),
            };
            let sub = v.function.clone().unwrap_or_else(|| match b.action {
                Action::Fader(_) => "Fader".to_string(),
                a => capitalise(a.to_string().split(' ').next().unwrap_or("")),
            });
            let state = if v.held {
                KeyState::Held
            } else if v.level > 0.0 || v.running {
                KeyState::Active
            } else {
                KeyState::Inactive
            };
            KeyLook {
                label,
                sub,
                halo: colour.or(v.halo),
                level: Some(v.level),
                state,
                fixed: b.action.is_fixed(),
            }
        }
        Subject::Other {
            name,
            active,
            level,
        } => KeyLook {
            label: (b.label.clone().or_else(|| name.clone()))
                .unwrap_or_else(|| action_label(&b.action)),
            sub: String::new(),
            halo: colour,
            level: *level,
            state: if *active {
                KeyState::Active
            } else {
                KeyState::Inactive
            },
            fixed: b.action.is_fixed(),
        },
    }
}

// --- text -------------------------------------------------------------------------------

fn font() -> Option<&'static Font> {
    static FONT: OnceLock<Option<Font>> = OnceLock::new();
    FONT.get_or_init(|| {
        Font::from_bytes(
            include_bytes!("../fonts/AtkinsonHyperlegible-Bold.ttf") as &[u8],
            FontSettings::default(),
        )
        .ok()
    })
    .as_ref()
}

fn round(v: f32) -> i32 {
    (v + 0.5) as i32
}

fn advance(f: &Font, c: char, px: f32) -> i32 {
    round(f.metrics(c, px).advance_width)
}

fn width(f: &Font, s: &str, px: f32) -> i32 {
    s.chars().map(|c| advance(f, c, px)).sum()
}

/// Side margin of the legend area.
fn margin(s: u16) -> i32 {
    (i32::from(s) / 24).max(2)
}

/// Greedy word wrap; a word wider than the line is broken by characters.
fn wrap(f: &Font, text: &str, px: f32, avail: i32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let joined = if cur.is_empty() {
            word.to_string()
        } else {
            format!("{cur} {word}")
        };
        if width(f, &joined, px) <= avail {
            cur = joined;
            continue;
        }
        if !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
        }
        for c in word.chars() {
            let mut next = cur.clone();
            next.push(c);
            if !cur.is_empty() && width(f, &next, px) > avail {
                lines.push(std::mem::replace(&mut cur, c.to_string()));
            } else {
                cur = next;
            }
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Cut characters until `s` plus an ellipsis fits.
fn ellipsise(f: &Font, s: &str, px: f32, avail: i32) -> String {
    let mut cut = s.to_string();
    while !cut.is_empty() && width(f, &format!("{cut}…"), px) > avail {
        cut.pop();
    }
    format!("{cut}…")
}

/// The legend over at most 3 lines, shrinking from about s/4.5 to s/8; past that the last line
/// ends in an ellipsis (CTL-50).
pub fn legend_lines(text: &str, px: u16) -> (Vec<String>, f32) {
    let s = f32::from(px);
    let (big, small) = (s / 4.5, s / 8.0);
    let (Some(f), false) = (font(), text.trim().is_empty()) else {
        return (Vec::new(), big);
    };
    let avail = i32::from(px) - 2 * margin(px);
    let high = i32::from(px) / 2;
    let sizes = || (0..5).map(|i| big - (big - small) * i as f32 / 4.0);
    for size in sizes() {
        let lines = wrap(f, text, size, avail);
        let lh = f
            .horizontal_line_metrics(size)
            .map_or(0, |m| round(m.new_line_size));
        if lines.len() <= 3 && lines.len() as i32 * lh <= high {
            return (lines, size);
        }
    }
    let mut lines = wrap(f, text, small, avail);
    if lines.len() > 3 {
        lines.truncate(3);
        if let Some(last) = lines.last_mut() {
            *last = ellipsise(f, last, small, avail);
        }
    }
    (lines, small)
}

/// Draw `text` centred on `cx` with its baseline at `base`, alpha-blended onto `buf`.
fn draw(buf: &mut [u8], s: usize, f: &Font, text: &str, px: f32, at: (i32, i32), col: Rgb) {
    let (cx, base) = at;
    let mut pen = cx - width(f, text, px) / 2;
    for c in text.chars() {
        let (m, bitmap) = f.rasterize(c, px);
        if m.width > 0 {
            for (i, a) in bitmap.iter().enumerate() {
                let gx = pen + m.xmin + (i % m.width) as i32;
                let gy = base - m.ymin - m.height as i32 + (i / m.width) as i32;
                let (Ok(x), Ok(y)) = (usize::try_from(gx), usize::try_from(gy)) else {
                    continue;
                };
                if x >= s || y >= s {
                    continue;
                }
                let a = u32::from(*a);
                if let Some(p) = buf.get_mut((y * s + x) * 3..(y * s + x) * 3 + 3) {
                    for (o, c) in p.iter_mut().zip(col) {
                        *o = ((u32::from(*o) * (255 - a) + u32::from(c) * a + 127) / 255) as u8;
                    }
                }
            }
        }
        pen += advance(f, c, px);
    }
}

fn fill(buf: &mut [u8], s: usize, rows: std::ops::Range<usize>, cols: usize, col: Rgb) {
    for y in rows {
        for x in 0..cols.min(s) {
            if let Some(p) = buf.get_mut((y * s + x) * 3..(y * s + x) * 3 + 3) {
                p.copy_from_slice(&col);
            }
        }
    }
}

/// A small padlock in the top-right corner under the bar: a body with a shackle above it.
fn lock(buf: &mut [u8], s: usize, halo_h: usize) {
    let w = (s / 6).max(4);
    let (x0, y0) = (s.saturating_sub(w + 3), halo_h + 2);
    let body = y0 + w / 2;
    fill_at(buf, s, (x0, body), (w, w - w / 2));
    // Shackle: two posts and a top, 1 px thick, leaving the middle of the body row free.
    let (l, r) = (x0 + w / 4, x0 + w - w / 4 - 1);
    fill_at(buf, s, (l, y0), (1, w / 2));
    fill_at(buf, s, (r, y0), (1, w / 2));
    fill_at(buf, s, (l, y0), (r - l + 1, 1));
}

fn fill_at(buf: &mut [u8], s: usize, at: (usize, usize), size: (usize, usize)) {
    for y in at.1..at.1 + size.1 {
        for x in at.0..at.0 + size.0 {
            if let Some(p) = buf.get_mut((y * s + x) * 3..(y * s + x) * 3 + 3) {
                p.copy_from_slice(&[255; 3]);
            }
        }
    }
}

/// The key upright, RGB, `px * px * 3` bytes (CTL-51).
pub fn render_rgb(look: &KeyLook, px: u16) -> Vec<u8> {
    let s = usize::from(px);
    let mut buf = vec![0u8; s * s * 3];
    if look.state == KeyState::Off || s == 0 {
        return buf;
    }
    let halo_h = (s / 9).max(2);
    // The top bar: the level in the halo colour (white without), the rest at 25 % (grey 40).
    match (look.level, look.halo) {
        (Some(l), halo) => {
            let w = round(l.clamp(0.0, 1.0) * s as f32) as usize;
            let rest = halo.map_or([40; 3], |h| h.map(|v| ((u16::from(v) + 2) / 4) as u8));
            fill(&mut buf, s, 0..halo_h, s, rest);
            fill(&mut buf, s, 0..halo_h, w, halo.unwrap_or([255; 3]));
        }
        (None, Some(h)) => fill(&mut buf, s, 0..halo_h, s, h),
        (None, None) => {}
    }
    if look.fixed {
        lock(&mut buf, s, halo_h);
    }
    if let Some(f) = font() {
        let cx = s as i32 / 2;
        let mut bottom = s as i32;
        if !look.sub.is_empty() {
            let sub_px = (s as f32 / 8.0).max(8.0);
            let lm = f.horizontal_line_metrics(sub_px);
            let base = bottom - 2 + lm.map_or(0, |m| m.descent as i32);
            let text = if width(f, &look.sub, sub_px) > s as i32 - 4 {
                ellipsise(f, &look.sub, sub_px, s as i32 - 4)
            } else {
                look.sub.clone()
            };
            draw(&mut buf, s, f, &text, sub_px, (cx, base), [170; 3]);
            bottom -= round(sub_px) + 2;
        }
        let (lines, size) = legend_lines(&look.label, px);
        if let Some(lm) = f.horizontal_line_metrics(size) {
            let lh = round(lm.new_line_size);
            let top = halo_h as i32 + 1;
            let y0 = top + ((bottom - top - lh * lines.len() as i32) / 2).max(0);
            for (i, line) in lines.iter().enumerate() {
                let base = y0 + lh * i as i32 + round(lm.ascent);
                draw(&mut buf, s, f, line, size, (cx, base), [255; 3]);
            }
        }
    }
    match look.state {
        KeyState::Inactive => buf
            .iter_mut()
            .for_each(|v| *v = ((u16::from(*v) * 2 + 2) / 5) as u8),
        KeyState::Held => buf.iter_mut().for_each(|v| *v = 255 - *v),
        _ => {}
    }
    buf
}

// --- encoding ---------------------------------------------------------------------------

/// Rotate counter-clockwise, then flip, then BMP or JPEG (CTL-51). Empty when the model has no
/// key images or `rgb` is not `key_px` square.
pub fn encode(rgb: &[u8], model: &Model) -> Vec<u8> {
    let s = usize::from(model.key_px);
    if s == 0 || rgb.len() != s * s * 3 || model.format == ImageFormat::None {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(rgb.len());
    for y in 0..s {
        for x in 0..s {
            let fx = if model.flip[0] { s - 1 - x } else { x };
            let fy = if model.flip[1] { s - 1 - y } else { y };
            let (sx, sy) = match model.rotation % 360 {
                90 => (s - 1 - fy, fx),
                180 => (s - 1 - fx, s - 1 - fy),
                270 => (fy, s - 1 - fx),
                _ => (fx, fy),
            };
            let at = (sy * s + sx) * 3;
            out.extend_from_slice(rgb.get(at..at + 3).unwrap_or(&[0; 3]));
        }
    }
    match model.format {
        ImageFormat::Bmp => bmp(&out, s),
        _ => jpeg(&out, model.key_px),
    }
}

fn bmp(rgb: &[u8], s: usize) -> Vec<u8> {
    let row = (s * 3 + 3) & !3;
    let image = (row * s) as u32;
    let mut b = Vec::with_capacity(54 + image as usize);
    b.extend_from_slice(b"BM");
    b.extend_from_slice(&(54 + image).to_le_bytes());
    b.extend_from_slice(&[0; 4]);
    b.extend_from_slice(&54u32.to_le_bytes());
    b.extend_from_slice(&40u32.to_le_bytes());
    b.extend_from_slice(&(s as i32).to_le_bytes());
    b.extend_from_slice(&(s as i32).to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&24u16.to_le_bytes());
    b.extend_from_slice(&[0; 4]);
    b.extend_from_slice(&image.to_le_bytes());
    b.extend_from_slice(&3780u32.to_le_bytes());
    b.extend_from_slice(&3780u32.to_le_bytes());
    b.extend_from_slice(&[0; 8]);
    for line in rgb.chunks(s * 3).rev() {
        for p in line.chunks(3) {
            b.extend(p.iter().rev());
        }
        b.resize(b.len() + row - s * 3, 0);
    }
    b
}

fn jpeg(rgb: &[u8], s: u16) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = Encoder::new(&mut out, 90);
    enc.set_sampling_factor(SamplingFactor::F_1_1);
    match enc.encode(rgb, s, s, ColorType::Rgb) {
        Ok(()) => out,
        Err(_) => Vec::new(),
    }
}

/// A key image in the model's native format.
pub fn render(look: &KeyLook, model: &Model) -> Vec<u8> {
    encode(&render_rgb(look, model.key_px), model)
}
