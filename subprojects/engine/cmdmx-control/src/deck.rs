//! Elgato Stream Deck as pure bytes: model table, HID report codec, deck pages and the default
//! mapping (CTL-45, 47, 48, 49). Packet layouts follow python-elgato-streamdeck. No USB here.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::action::Action;
use crate::convert::Reading;
use crate::mapping::{Binding, Input, Mapping};
use crate::ControlError;

pub const ELGATO_VENDOR: u16 = 0x0fd9;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageFormat {
    Jpeg,
    Bmp,
    None,
}

/// Which report layout a model speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    V1,
    Mini,
    V2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Size {
    pub w: u16,
    pub h: u16,
}

/// One row of `profiles/streamdeck-models.json` (CTL-45).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Model {
    #[serde(deserialize_with = "hex_u16")]
    pub product_id: u16,
    pub name: String,
    pub cols: u16,
    pub rows: u16,
    pub key_px: u16,
    pub format: ImageFormat,
    /// Degrees counter-clockwise before the flips.
    pub rotation: u16,
    pub flip: [bool; 2],
    pub family: Family,
    pub verified: bool,
    #[serde(default)]
    pub dials: u16,
    #[serde(default)]
    pub touch_keys: u16,
    #[serde(default)]
    pub strip: Option<Size>,
    #[serde(default)]
    pub info: Option<Size>,
}

impl Model {
    pub fn keys(&self) -> u16 {
        self.cols * self.rows
    }
}

fn hex_u16<'de, D: Deserializer<'de>>(d: D) -> Result<u16, D::Error> {
    let s = String::deserialize(d)?;
    let digits = s.strip_prefix("0x").unwrap_or(&s);
    u16::from_str_radix(digits, 16).map_err(serde::de::Error::custom)
}

#[derive(Deserialize)]
struct Table {
    models: Vec<Model>,
}

pub fn models() -> Result<Vec<Model>, ControlError> {
    serde_json::from_str::<Table>(include_str!("../../profiles/streamdeck-models.json"))
        .map(|t| t.models)
        .map_err(|e| ControlError::InvalidMapping(format!("stream deck model table: {e}")))
}

pub fn model(pid: u16) -> Option<Model> {
    models().ok()?.into_iter().find(|m| m.product_id == pid)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeckKind {
    Key,
    Touch,
    Dial,
    DialPush,
    Strip,
}

/// A Stream Deck input; `number` counts from 1 as printed (CTL-47).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeckInput {
    pub kind: DeckKind,
    pub number: u16,
}

impl fmt::Display for DeckInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.kind {
            DeckKind::Key => "key",
            DeckKind::Touch => "touch",
            DeckKind::Dial => "dial",
            DeckKind::DialPush => "dial push",
            DeckKind::Strip => "strip",
        };
        write!(f, "{kind} {}", self.number)
    }
}

fn input(kind: DeckKind, number: usize) -> DeckInput {
    DeckInput {
        kind,
        number: u16::try_from(number).unwrap_or(u16::MAX),
    }
}

/// Printed (0-based) and device key index of the Original, which mirrors each row.
fn mirror(i: usize, cols: usize) -> usize {
    let col = i.checked_rem(cols).unwrap_or(0);
    (i - col) + cols.saturating_sub(1).saturating_sub(col)
}

/// Turns HID input reports into changes (CTL-47).
#[derive(Debug, Clone)]
pub struct DeckReader {
    model: Model,
    keys: Vec<bool>,
    pushes: Vec<bool>,
}

impl DeckReader {
    pub fn new(model: &Model) -> DeckReader {
        DeckReader {
            model: model.clone(),
            keys: vec![false; usize::from(model.keys() + model.touch_keys)],
            pushes: vec![false; usize::from(model.dials)],
        }
    }

    /// Only what changed since the last report, keys first, then touch keys.
    pub fn read(&mut self, report: &[u8]) -> Result<Vec<(DeckInput, Reading)>, ControlError> {
        let bad = ControlError::MalformedHid;
        if report.first() != Some(&0x01) {
            return Err(bad("not an input report"));
        }
        let keys = usize::from(self.model.keys());
        let all = self.keys.len();
        let now: Vec<bool> = match self.model.family {
            Family::V1 | Family::Mini => {
                let body = report.get(1..1 + all).ok_or(bad("too short"))?;
                let cols = usize::from(self.model.cols);
                (0..all)
                    .map(|p| {
                        let d = if self.model.family == Family::V1 {
                            mirror(p, cols)
                        } else {
                            p
                        };
                        body.get(d).is_some_and(|b| *b != 0)
                    })
                    .collect()
            }
            Family::V2 => match report.get(1) {
                Some(0x00) => report
                    .get(4..4 + all)
                    .ok_or(bad("too short"))?
                    .iter()
                    .map(|b| *b != 0)
                    .collect(),
                Some(0x02) => return self.strip(report),
                Some(0x03) => return self.dials(report),
                _ => return Err(bad("unknown report type")),
            },
        };
        let mut events = Vec::new();
        for (i, (was, is)) in self.keys.iter_mut().zip(now).enumerate() {
            if *was != is {
                *was = is;
                let kind = if i < keys {
                    DeckKind::Key
                } else {
                    DeckKind::Touch
                };
                let n = if i < keys { i + 1 } else { i - keys + 1 };
                events.push((input(kind, n), Reading::Button(is)));
            }
        }
        Ok(events)
    }

    /// Touch strip: a short tap hits one of as many segments as there are dials.
    fn strip(&self, report: &[u8]) -> Result<Vec<(DeckInput, Reading)>, ControlError> {
        let bad = ControlError::MalformedHid;
        let strip = self.model.strip.ok_or(bad("no touch strip"))?;
        let (Some(kind), Some(lo), Some(hi)) = (report.get(4), report.get(6), report.get(7)) else {
            return Err(bad("too short"));
        };
        if *kind != 1 {
            return Ok(Vec::new());
        }
        let x = u32::from(u16::from_le_bytes([*lo, *hi]));
        let dials = u32::from(self.model.dials);
        let seg = (x * dials).checked_div(u32::from(strip.w)).unwrap_or(0) + 1;
        let seg = usize::try_from(seg.min(dials)).unwrap_or(1);
        Ok(vec![(input(DeckKind::Strip, seg), Reading::Tap)])
    }

    fn dials(&mut self, report: &[u8]) -> Result<Vec<(DeckInput, Reading)>, ControlError> {
        let bad = ControlError::MalformedHid;
        let n = usize::from(self.model.dials);
        let body = report.get(5..5 + n).ok_or(bad("too short"))?;
        match report.get(4) {
            Some(0x00) => {
                let mut events = Vec::new();
                for (i, (was, b)) in self.pushes.iter_mut().zip(body).enumerate() {
                    let is = *b != 0;
                    if *was != is {
                        *was = is;
                        events.push((input(DeckKind::DialPush, i + 1), Reading::Button(is)));
                    }
                }
                Ok(events)
            }
            Some(0x01) => Ok(body
                .iter()
                .enumerate()
                .filter(|(_, b)| **b != 0)
                .map(|(i, b)| {
                    let steps = f32::from(i8::from_ne_bytes([*b]));
                    (input(DeckKind::Dial, i + 1), Reading::Steps(steps))
                })
                .collect()),
            _ => Err(bad("unknown dial event")),
        }
    }
}

/// Output reports that put `image` (already in the model's format) on printed key `key`
/// (from 1). Empty for a key the model lacks or a model without key images.
pub fn image_reports(model: &Model, key: u16, image: &[u8]) -> Vec<Vec<u8>> {
    if key == 0 || key > model.keys() || model.format == ImageFormat::None {
        return Vec::new();
    }
    let printed = usize::from(key - 1);
    let wrap = |header: &[u8], chunk: &[u8], size: usize| {
        let mut r = header.to_vec();
        r.extend_from_slice(chunk);
        r.resize(size, 0);
        r
    };
    match model.family {
        Family::V1 => {
            let device = mirror(printed, usize::from(model.cols));
            let (a, b) = image.split_at(image.len() / 2);
            [a, b]
                .iter()
                .enumerate()
                .map(|(i, half)| {
                    let h = v1_header(i as u8 + 1, i == 1, device as u8 + 1);
                    wrap(&h, half, 8191)
                })
                .collect()
        }
        Family::Mini => chunks(image, 1008)
            .map(|(page, last, c)| wrap(&v1_header(page as u8, last, key as u8), c, 1024))
            .collect(),
        Family::V2 => chunks(image, 1016)
            .map(|(page, last, c)| {
                let len = (c.len() as u16).to_le_bytes();
                let page = (page as u16).to_le_bytes();
                let h = [
                    2,
                    7,
                    printed as u8,
                    last as u8,
                    len[0],
                    len[1],
                    page[0],
                    page[1],
                ];
                wrap(&h, c, 1024)
            })
            .collect(),
    }
}

fn v1_header(page: u8, last: bool, key: u8) -> [u8; 16] {
    [2, 1, page, 0, last as u8, key, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
}

/// `(page from 0, is last, chunk)`.
fn chunks(image: &[u8], size: usize) -> impl Iterator<Item = (usize, bool, &[u8])> {
    let n = image.chunks(size).len();
    image
        .chunks(size)
        .enumerate()
        .map(move |(i, c)| (i, i + 1 == n, c))
}

/// Feature report that sets the brightness, 0..=100 percent.
pub fn brightness_report(model: &Model, percent: u8) -> Vec<u8> {
    let p = percent.min(100);
    let (mut r, len) = match model.family {
        Family::V1 | Family::Mini => (vec![0x05, 0x55, 0xAA, 0xD1, 0x01, p], 17),
        Family::V2 => (vec![0x03, 0x08, p], 32),
    };
    r.resize(len, 0);
    r
}

/// Number of deck pages a mapping uses (CTL-48).
pub fn deck_pages(mapping: &Mapping) -> u16 {
    (mapping.bindings.iter())
        .map(|b| b.deck_page)
        .max()
        .unwrap_or(1)
        .max(1)
}

/// The deck page after `action`; other actions and unknown pages leave it alone.
pub fn turn_page(page: u16, pages: u16, action: &Action) -> u16 {
    match action {
        Action::DeckPageNext => page.saturating_add(1).min(pages).max(page),
        Action::DeckPagePrev => page.saturating_sub(1).max(1).min(page),
        Action::DeckPage(n) if (1..=pages).contains(n) => *n,
        _ => page,
    }
}

/// Key bindings belong to one deck page; every other input works on all of them.
pub fn on_page(b: &Binding, page: u16) -> bool {
    !matches!(
        b.input,
        Input::Deck(DeckInput {
            kind: DeckKind::Key,
            ..
        })
    ) || b.deck_page == page
}

/// The mapping a freshly seen deck gets (CTL-49).
pub fn default_mapping(model: &Model, faders_per_page: u16) -> Mapping {
    let mut bindings = Vec::new();
    let mut add = |kind: DeckKind, n: u16, action: String| {
        if let Ok(a) = action.parse::<Action>() {
            bindings.push(Binding::new(Input::Deck(DeckInput { kind, number: n }), a));
        }
    };
    let keys = model.keys();
    if model.format == ImageFormat::None {
        for (n, a) in (1..).zip(["back", "go", "stop"]) {
            add(DeckKind::Key, n, a.to_string());
        }
    } else {
        let (tail, area) = if keys >= 8 {
            (
                vec![
                    (keys - 2, "page prev"),
                    (keys - 1, "page next"),
                    (keys, "go"),
                ],
                keys - 3,
            )
        } else {
            (vec![(keys, "go")], keys.saturating_sub(1))
        };
        for n in 1..=area.min(faders_per_page) {
            add(DeckKind::Key, n, format!("upper {n}"));
        }
        for (n, a) in tail {
            add(DeckKind::Key, n, a.to_string());
        }
    }
    if model.touch_keys >= 2 {
        add(DeckKind::Touch, 1, "deck page prev".to_string());
        add(DeckKind::Touch, 2, "deck page next".to_string());
    }
    for n in 1..=model.dials {
        add(DeckKind::Dial, n, format!("wheel {n}"));
    }
    let n = &model.name;
    Mapping {
        name: n.clone(),
        device: format!("{n} (*|{n}|{n} #*"),
        bindings,
        extra: Default::default(),
    }
}
