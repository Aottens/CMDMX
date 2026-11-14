//! SHW-25: the printable show report, one self-contained HTML document from a `Show`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use crate::attribute::AttributeGroup;
use crate::palette::{PaletteKind, Window};
use crate::playback::Kind;
use crate::profile::COLOUR;
use crate::show::{Show, ShowPalette};

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn table(out: &mut String, title: &str, head: &[&str], rows: &[Vec<String>]) {
    let _ = write!(out, "<h2>{title}</h2>\n<table>\n<thead><tr>");
    for h in head {
        let _ = write!(out, "<th>{h}</th>");
    }
    out.push_str("</tr></thead>\n<tbody>\n");
    for row in rows {
        out.push_str("<tr>");
        for cell in row {
            let _ = write!(out, "<td>{cell}</td>");
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</tbody>\n</table>\n");
}

/// The group of an attribute name, as `Engine::attribute_group` does.
fn group_of(show: &Show, name: &str) -> AttributeGroup {
    if name == COLOUR {
        return AttributeGroup::C;
    }
    (show.profiles.values())
        .flat_map(|p| &p.modes)
        .find_map(|m| m.logical(name).map(|l| l.group))
        .unwrap_or(AttributeGroup::Fx)
}

fn letter(g: AttributeGroup) -> &'static str {
    match g {
        AttributeGroup::I => "I",
        AttributeGroup::P => "P",
        AttributeGroup::C => "C",
        AttributeGroup::G => "G",
        AttributeGroup::B => "B",
        AttributeGroup::E => "E",
        AttributeGroup::S => "S",
        AttributeGroup::Fx => "FX",
    }
}

pub fn render(show: &Show, time: &str) -> String {
    let title = format!("{} — {time}", esc(&show.meta.name));
    let mut out = String::new();
    let _ = write!(
        out,
        "<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>{title}</title>\n\
         <style>\n@page {{ size: A4; margin: 12mm; }}\n\
         body {{ font-family: sans-serif; font-size: 9pt; }}\n\
         table {{ border-collapse: collapse; width: 100%; margin-bottom: 8mm; }}\n\
         th, td {{ border: 1px solid #888; padding: 2px 4px; text-align: left; }}\n\
         thead {{ display: table-header-group; }}\n</style>\n</head>\n<body>\n<h1>{title}</h1>\n"
    );

    let mut fixtures: Vec<_> = show.patch.fixtures.iter().collect();
    fixtures.sort_by_key(|f| f.number);
    let numbers: BTreeMap<u32, u32> = fixtures.iter().map(|f| (f.id, f.number)).collect();
    let rows: Vec<Vec<String>> = fixtures
        .iter()
        .map(|f| {
            let mode = f.mode.clone().unwrap_or_else(|| {
                (show.profiles.get(&f.profile))
                    .and_then(|p| p.modes.first())
                    .map(|m| m.name.clone())
                    .unwrap_or_default()
            });
            vec![
                f.number.to_string(),
                esc(&f.name),
                esc(&f.profile),
                esc(&mode),
                format!("{}.{}", f.universe, f.address),
                if f.parked { "yes" } else { "" }.into(),
            ]
        })
        .collect();
    table(
        &mut out,
        "Fixtures",
        &["No.", "Name", "Profile", "Mode", "Address", "Parked"],
        &rows,
    );

    let mut groups: Vec<_> = show.groups.iter().collect();
    groups.sort_by_key(|g| g.id);
    let rows: Vec<Vec<String>> = groups
        .iter()
        .map(|g| {
            let members: Vec<String> = (g.items.iter())
                .map(|i| {
                    let n = numbers.get(&i.fixture).copied().unwrap_or(i.fixture);
                    match i.cell {
                        None => n.to_string(),
                        Some(c) => format!("{n}.{c}"),
                    }
                })
                .collect();
            vec![g.id.to_string(), esc(&g.legend), members.join(", ")]
        })
        .collect();
    table(&mut out, "Groups", &["No.", "Legend", "Members"], &rows);

    let mut palettes: Vec<_> = (show.palettes.palettes.iter())
        .filter_map(|p| match p {
            ShowPalette::Palette(p) => Some(p),
            ShowPalette::Unreadable(_) => None,
        })
        .collect();
    palettes.sort_by_key(|p| p.number);
    let rows: Vec<Vec<String>> = palettes
        .iter()
        .map(|p| {
            let count = match p.kind() {
                PaletteKind::Global => "global".to_string(),
                PaletteKind::Shared => "by type".to_string(),
                PaletteKind::Normal => (p.fixtures.keys().map(|i| i.fixture))
                    .collect::<BTreeSet<_>>()
                    .len()
                    .to_string(),
            };
            let names = (p.attribute_names().into_iter()).chain(
                p.effects
                    .iter()
                    .flat_map(|e| e.attributes.iter().map(String::as_str)),
            );
            let groups: BTreeSet<AttributeGroup> = names.map(|n| group_of(show, n)).collect();
            let groups: Vec<&str> = groups.into_iter().map(letter).collect();
            let window = match p.window {
                Window::Colours => "Colour",
                Window::Positions => "Position",
                Window::Beams => "Beam",
            };
            vec![
                p.number.to_string(),
                esc(&p.legend),
                window.into(),
                count,
                groups.join(" "),
            ]
        })
        .collect();
    table(
        &mut out,
        "Palettes",
        &["No.", "Legend", "Kind", "Fixtures", "Groups"],
        &rows,
    );

    let mut playbacks: Vec<_> = show.playbacks.playbacks.iter().collect();
    playbacks.sort_by_key(|p| (p.page, p.slot));
    let mut rows: Vec<Vec<String>> = Vec::new();
    for p in playbacks {
        let kind = match p.kind {
            Kind::Cues if p.is_cue_list() => "Cue list",
            Kind::Cues => "Memory",
            Kind::Chase => "Chase",
            Kind::BpmMaster => "BPM master",
            Kind::RateMaster => "Rate master",
            Kind::GroupMaster => "Group master",
        };
        let mut row = vec![
            p.page.to_string(),
            p.slot.to_string(),
            esc(&p.legend),
            kind.to_string(),
        ];
        row.resize(9, String::new());
        rows.push(row);
        if p.is_cue_list() || p.kind == Kind::Chase {
            for c in &p.cues {
                let mut row = vec![String::new(); 4];
                row.push(c.number.to_string());
                row.push(esc(&c.legend));
                row.push(c.fade_in.to_string());
                row.push(c.fade_out.unwrap_or(c.fade_in).to_string());
                row.push(c.delay.to_string());
                rows.push(row);
            }
        }
    }
    table(
        &mut out,
        "Playbacks",
        &[
            "Page",
            "Handle",
            "Legend",
            "Kind",
            "Cue",
            "Cue legend",
            "Fade in",
            "Fade out",
            "Delay",
        ],
        &rows,
    );
    out.push_str("</body>\n</html>\n");
    out
}
