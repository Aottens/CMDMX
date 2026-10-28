//! Gel numbers: a range's table of CIE xy rows and their colours (PRG-67..70,
//! `specs/modules/programmer.md`). Pure data; the LEE table is compiled in.

use serde::{Deserialize, Serialize};

use crate::colour::{xy_to_rgb, TargetColour};
use crate::CoreError;

/// PRG-67: the compiled-in LEE table (`subprojects/engine/gels/lee.json`).
pub const LEE_JSON: &str = include_str!("../../gels/lee.json");

/// PRG-69: how far a colour may be from a row, per component, and still read as it.
const READING_TOLERANCE: f32 = 0.002;

/// One row of a range's table: CIE xy (Y = 1) of the filter in front of tungsten.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Gel {
    pub number: u32,
    pub name: String,
    pub x: f32,
    pub y: f32,
}

/// PRG-69: the gel a colour reads as, for the UI.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GelRef {
    pub range: &'static str,
    pub number: u32,
    pub name: String,
}

/// PRG-67: rows sorted by number, each with its colour.
#[derive(Debug, Clone, Default)]
pub struct GelTable {
    gels: Vec<Gel>,
    colours: Vec<TargetColour>,
}

impl GelTable {
    /// A JSON array of rows. A row that does not convert to a colour, or a repeated number,
    /// is refused.
    pub fn from_json(json: &str) -> Result<GelTable, CoreError> {
        let mut gels: Vec<Gel> =
            serde_json::from_str(json).map_err(|e| CoreError::InvalidGel(e.to_string()))?;
        gels.sort_by_key(|g| g.number);
        let mut colours = Vec::with_capacity(gels.len());
        let mut previous = None;
        for gel in &gels {
            if previous == Some(gel.number) {
                return Err(CoreError::InvalidGel(format!(
                    "{} is listed twice",
                    gel.number
                )));
            }
            previous = Some(gel.number);
            // x, y must also be a point of the chromaticity diagram (x >= 0, x + y <= 1).
            let on_diagram = gel.x >= 0.0 && gel.x + gel.y <= 1.0;
            let [r, g, b] = xy_to_rgb([gel.x, gel.y])
                .filter(|_| on_diagram)
                .ok_or_else(|| {
                    CoreError::InvalidGel(format!(
                        "{} has no colour at x {}, y {}",
                        gel.number, gel.x, gel.y
                    ))
                })?;
            colours.push(TargetColour {
                r: r.max(0.0),
                g: g.max(0.0),
                b: b.max(0.0),
            });
        }
        Ok(GelTable { gels, colours })
    }

    /// The compiled-in LEE table; empty if it does not parse.
    pub fn lee() -> GelTable {
        GelTable::from_json(LEE_JSON).unwrap_or_default()
    }

    pub fn gels(&self) -> &[Gel] {
        &self.gels
    }

    /// PRG-67: linear BT.2020, negatives 0, largest component 1.
    pub fn colour(&self, number: u32) -> Option<TargetColour> {
        let at = self.gels.iter().position(|g| g.number == number)?;
        self.colours.get(at).copied()
    }

    /// PRG-69: the lowest-numbered gel whose colour `c` has at any level; black reads none.
    pub fn reading(&self, c: TargetColour) -> Option<&Gel> {
        let max = c.r.max(c.g).max(c.b);
        if max <= 0.0 {
            return None;
        }
        let c = c.scaled(1.0 / max);
        let near = |a: f32, b: f32| (a - b).abs() <= READING_TOLERANCE;
        let at = self
            .colours
            .iter()
            .position(|t| near(c.r, t.r) && near(c.g, t.g) && near(c.b, t.b))?;
        self.gels.get(at)
    }
}
