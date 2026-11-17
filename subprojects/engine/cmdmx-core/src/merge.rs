//! Merge from another show (SHW-27..31): the types, the proposed fixture map and the lists the
//! app shows. The merge itself is `Engine::merge`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::engine::Engine;
use crate::palette::Window;
use crate::playback::Kind;
use crate::programmer::{GroupId, Item};
use crate::show::{Show, ShowPalette};

/// SHW-28: the other show's fixture number -> the open show's number; `None` = unmapped.
pub type FixtureMap = BTreeMap<u32, Option<u32>>;

/// A playback's home handle in the other show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Slot {
    pub page: u16,
    pub slot: u16,
}

/// SHW-29: what the operator picked. `map` holds only the lines changed from the proposal.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Request {
    pub map: FixtureMap,
    pub groups: Vec<GroupId>,
    /// Palette numbers.
    pub palettes: Vec<u32>,
    pub playbacks: Vec<Slot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PlacedKind {
    Group,
    Palette,
    Playback,
}

/// One object put into the open show: numbers ("7") or handles ("1.2"); `brought` = not ticked,
/// taken along because a taken object uses it (SHW-30).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Placed {
    pub kind: PlacedKind,
    pub from: String,
    pub to: String,
    pub brought: bool,
}

/// SHW-31: what was dropped from the taken objects.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct LeftOut {
    /// Numbers of the unmapped fixtures, sorted.
    pub fixtures: Vec<u32>,
    /// Attribute values, group items and effect members.
    pub values: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Plan {
    pub placed: Vec<Placed>,
    pub left_out: LeftOut,
}

impl Plan {
    /// SHW-31: "Merged from Kerk: 3 playbacks, 2 palettes, 3 groups; 1 fixture not in this show,
    /// 3 values left out".
    pub fn summary(&self, name: &str) -> String {
        let count = |kind| self.placed.iter().filter(|p| p.kind == kind).count();
        let plural = |n: usize, one: &str, many: &str, tail: &str| {
            (n > 0).then(|| format!("{n} {}{tail}", if n == 1 { one } else { many }))
        };
        let placed: Vec<String> = [
            plural(count(PlacedKind::Playback), "playback", "playbacks", ""),
            plural(count(PlacedKind::Palette), "palette", "palettes", ""),
            plural(count(PlacedKind::Group), "group", "groups", ""),
        ]
        .into_iter()
        .flatten()
        .collect();
        let left: Vec<String> = [
            plural(
                self.left_out.fixtures.len(),
                "fixture",
                "fixtures",
                " not in this show",
            ),
            plural(
                self.left_out.values as usize,
                "value",
                "values",
                " left out",
            ),
        ]
        .into_iter()
        .flatten()
        .collect();
        let mut text = format!("Merged from {name}: {}", placed.join(", "));
        if !left.is_empty() {
            text.push_str("; ");
            text.push_str(&left.join(", "));
        }
        text
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ContentsFixture {
    pub number: u32,
    pub name: String,
    pub profile: String,
    pub mode: Option<String>,
    pub to: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContentsGroup {
    pub number: GroupId,
    pub legend: String,
    /// In the other show's fixture numbers.
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContentsPalette {
    pub number: u32,
    pub window: Window,
    pub legend: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContentsPlayback {
    pub page: u16,
    pub slot: u16,
    pub legend: String,
    pub kind: Kind,
    pub cues: usize,
}

/// SHW-27: what the other show holds and the proposed map.
#[derive(Debug, Clone, Serialize)]
pub struct Contents {
    pub fixtures: Vec<ContentsFixture>,
    pub groups: Vec<ContentsGroup>,
    pub palettes: Vec<ContentsPalette>,
    pub playbacks: Vec<ContentsPlayback>,
}

/// SHW-28: each fixture of `other` onto the open show's fixture with the same number and the
/// same profile key; else unmapped.
pub fn proposed_map(open: &Engine, other: &Show) -> FixtureMap {
    (other.patch.fixtures.iter())
        .map(|f| {
            let to = open
                .fixture_by_number(f.number)
                .filter(|t| t.profile == f.profile)
                .map(|t| t.number);
            (f.number, to)
        })
        .collect()
}

/// SHW-27: the lists of `other`, sorted by number or handle.
pub fn contents(open: &Engine, other: &Show) -> Contents {
    let map = proposed_map(open, other);
    let numbers: BTreeMap<u32, u32> = (other.patch.fixtures.iter())
        .map(|f| (f.id, f.number))
        .collect();
    let mut fixtures: Vec<_> = (other.patch.fixtures.iter())
        .map(|f| ContentsFixture {
            number: f.number,
            name: f.name.clone(),
            profile: f.profile.clone(),
            mode: f.mode.clone(),
            to: map.get(&f.number).copied().flatten(),
        })
        .collect();
    fixtures.sort_by_key(|f| f.number);
    let mut groups: Vec<_> = (other.groups.iter())
        .map(|g| ContentsGroup {
            number: g.id,
            legend: g.legend.clone(),
            items: (g.items.iter())
                .filter_map(|i| {
                    let fixture = *numbers.get(&i.fixture)?;
                    Some(Item { fixture, ..*i })
                })
                .collect(),
        })
        .collect();
    groups.sort_by_key(|g| g.number);
    let mut palettes: Vec<_> = (other.palettes.palettes.iter())
        .filter_map(|p| match p {
            ShowPalette::Palette(p) => Some(ContentsPalette {
                number: p.number,
                window: p.window,
                legend: p.legend.clone(),
            }),
            ShowPalette::Unreadable(_) => None,
        })
        .collect();
    palettes.sort_by_key(|p| p.number);
    let mut playbacks: Vec<_> = (other.playbacks.playbacks.iter())
        .map(|p| ContentsPlayback {
            page: p.page,
            slot: p.slot,
            legend: p.legend.clone(),
            kind: p.kind,
            cues: p.cues.len(),
        })
        .collect();
    playbacks.sort_by_key(|p| (p.page, p.slot));
    Contents {
        fixtures,
        groups,
        palettes,
        playbacks,
    }
}
