//! The church patch, once (`specs/modules/patch.md` PAT-11, PAT-15..18; VS2-13).
//!
//! Reads the owner's Lightkey patch export (tab-separated, outside git), picks a profile per
//! fixture type, numbers the fixtures in blocks of 100 per type, patches them one by one with
//! `patch_add` and writes the show `Kerk` plus the user profiles it made. Never overwrites.
//!
//! Usage: `cargo run -p cmdmx-daemon --example church -- <export.tsv> [--show-dir PATH]
//!        [--profiles-dir PATH] [--qlcplus-dir PATH]`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use cmdmx_core::attribute::AttributeGroup::{self, C, E, G, I, S};
use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::library::channel_list::{self, ChannelList, ChannelType};
use cmdmx_core::library::qlcplus;
use cmdmx_core::patch::PatchAdd;
use cmdmx_core::profile::Profile;
use cmdmx_core::show::Meta;

#[cfg(test)]
mod tests;

/// One fixture row of the export (PAT-15).
#[derive(Debug, Clone, PartialEq)]
struct Row {
    /// 1-based line in the file (the header is line 1).
    line: usize,
    universe: u16,
    address: u16,
    manufacturer: String,
    model: String,
    channels: u16,
    short_name: String,
}

/// The profile a Lightkey type maps to (PAT-16).
#[derive(Debug, Clone)]
struct Picked {
    /// Engine profile key, `"<manufacturer>/<model>"` (PAT-09).
    key: String,
    mode: String,
    profile: Profile,
    /// Saved as a user profile by the run (channel lists and the Shehds).
    user: bool,
}

struct Dirs {
    show_dir: PathBuf,
    profiles_dir: PathBuf,
    qlcplus_dir: PathBuf,
}

const HEADER: &str = "Universe\tAddress\tCount\tManufacturer\tModel\tManufacturer & model\tMode\tChannels\tShort name\tComments";

/// PAT-15.
fn parse_export(tsv: &str) -> Result<Vec<Row>, String> {
    let mut lines = tsv.lines().map(|l| l.trim_end_matches('\r')).enumerate();
    if lines.next().map(|(_, l)| l) != Some(HEADER) {
        return Err("line 1: not a Lightkey patch export (unexpected header)".into());
    }
    let mut rows = Vec::new();
    for (i, text) in lines.filter(|(_, l)| !l.trim().is_empty()) {
        let line = i + 1;
        let cols: Vec<&str> = text.split('\t').collect();
        let col = |n: usize| cols.get(n).copied().unwrap_or("").trim();
        let num = |n: usize, what: &str| {
            col(n)
                .parse::<u16>()
                .map_err(|_| format!("line {line}: {what} '{}' is not a number", col(n)))
        };
        if num(2, "Count")? != 1 {
            return Err(format!("line {line}: Count must be 1, got '{}'", col(2)));
        }
        rows.push(Row {
            line,
            universe: num(0, "Universe")?,
            address: num(1, "Address")?,
            manufacturer: col(3).into(),
            model: col(4).into(),
            channels: num(7, "Channels")?,
            short_name: col(8).into(),
        });
    }
    Ok(rows)
}

fn other(name: &str, group: AttributeGroup, default: f32) -> ChannelType {
    ChannelType::Other {
        name: name.into(),
        group,
        default,
    }
}

/// PAT-16: QLC+ files are read from `qlcplus_dir`.
fn profile_for(manufacturer: &str, model: &str, qlcplus_dir: &Path) -> Result<Picked, String> {
    use ChannelType::{Blue, ColdWhite, Dimmer, Focus, Green, Lime, Pan, PanFine, Red, Tilt};
    use ChannelType::{TiltFine, WarmWhite, White};
    let qlc = |file: &str, mode: &str| -> Result<Picked, String> {
        let xml = fs::read_to_string(qlcplus_dir.join(file)).map_err(|e| format!("{file}: {e}"))?;
        let profile = qlcplus::import(&xml, &format!("qlcplus:{file}"))
            .map_err(|e| e.to_string())?
            .profile;
        Ok(Picked {
            key: format!("{}/{}", profile.manufacturer, profile.model),
            mode: mode.into(),
            profile,
            user: false,
        })
    };
    let list = |m: &str, model: &str, mode: &str, channels: Vec<ChannelType>| {
        let profile = channel_list::build(&ChannelList {
            manufacturer: m.into(),
            model: model.into(),
            mode: mode.into(),
            channels,
        })
        .map_err(|e| e.to_string())?;
        Ok(Picked {
            key: format!("{m}/{model}"),
            mode: mode.into(),
            profile,
            user: true,
        })
    };
    let shutter = other("Shutter", I, 1.0);
    match (manufacturer, model) {
        ("Showtec", "LED PAR 56 Short DMX") => qlc("Showtec/Showtec-LED-Par-56.qxf", "6 Channel"),
        ("Showtec", "Compact Par 7 Tri") => {
            qlc("Showtec/Showtec-Compact-Par-7-Tri.qxf", "3 Channel")
        }
        ("Stairville", "Hz-200 DMX") => qlc("Stairville/Stairville-Hz-200-DMX.qxf", "Default"),
        ("Shehds", "LED Beam+Wash 19x15W RGBW Zoom Light") => {
            let mut picked = qlc(
                "Shehds/Shehds-LED-Beam+Wash-19x15W-RGBW-Zoom.qxf",
                "24 Channel",
            )?;
            // Lightkey calls it Zoom (closes fixture-library open question 1).
            let modes = picked.profile.modes.iter_mut();
            for attr in modes.flat_map(|m| {
                let cells = m.cells.iter_mut().flat_map(|c| c.attributes.iter_mut());
                m.attributes.iter_mut().chain(cells)
            }) {
                if attr.name == "Focus" {
                    attr.name = "Zoom".into();
                }
            }
            picked.user = true;
            Ok(picked)
        }
        ("Showtec", "Phantom 25 LED Spot") => list(
            "Showtec",
            "Phantom 25 LED Spot",
            "11 Channel",
            vec![
                Pan,
                Tilt,
                PanFine,
                TiltFine,
                other("Pan/Tilt Speed", S, 0.0),
                other("Colour Wheel", C, 0.0),
                shutter,
                Dimmer,
                other("Gobo", G, 0.0),
                other("Control", S, 0.0),
                other("Programs", S, 0.0),
            ],
        ),
        ("Prolights", "Lumipar6UQ") => list(
            "Pro-Lights",
            "Lumipar6UQ",
            "4 Channel",
            vec![Red, Green, Blue, White],
        ),
        ("Generic", "PAR Spot") => list("Generic", "PAR Spot", "1 Channel", vec![Dimmer]),
        ("Generic", "PAR Wash (RGBW)") => list(
            "Generic",
            "PAR Wash (RGBW)",
            "4 Channel",
            vec![Red, Green, Blue, White],
        ),
        ("beamZ", "BTF50Z Mini Fresnel") => list(
            "beamZ",
            "BTF50Z Mini Fresnel",
            "3 Channel",
            vec![Dimmer, WarmWhite, ColdWhite],
        ),
        ("Generic", "PAR Spot (RGBL)") => list(
            "Generic",
            "PAR Spot (RGBL)",
            "4 Channel",
            vec![Red, Green, Blue, Lime],
        ),
        ("beamZ", "Fuze 75S Spot") => list(
            "beamZ",
            "Fuze 75S Spot",
            "16 Bit",
            vec![
                Pan,
                PanFine,
                Tilt,
                TiltFine,
                other("Colour Wheel", C, 0.0),
                other("Gobo", G, 0.0),
                shutter,
                Dimmer,
                Focus,
                other("Prism", E, 0.0),
                other("Gobo/Prism Rotation", G, 0.0),
                other("Pan/Tilt Speed", S, 0.0),
                other("Programs", S, 0.0),
            ],
        ),
        ("Generic", "PAR Spot (RGB)") => list(
            "Generic",
            "PAR Spot (RGB)",
            "3 Channel",
            vec![Red, Green, Blue],
        ),
        _ => Err(format!(
            "no profile for fixture type '{manufacturer} {model}'"
        )),
    }
}

/// `(letters before the first digit, the number after them)`; `None` without a number.
fn split_name(name: &str) -> Option<(&str, u32)> {
    let at = name.find(|c: char| c.is_ascii_digit())?;
    let (prefix, digits) = name.split_at(at);
    Some((prefix, digits.parse().ok()?))
}

/// PAT-17: `(number, name)` for each row, in the order of `rows`.
fn numbers_and_names(rows: &[Row]) -> Result<Vec<(u32, String)>, String> {
    let mut sorted: Vec<(usize, &Row)> = rows.iter().enumerate().collect();
    sorted.sort_by_key(|(_, r)| (r.universe, r.address));
    let mut types: Vec<((&str, &str), u32)> = Vec::new();
    // Every name in the export, plus the ones handed out instead of duplicates.
    let mut known: Vec<String> = rows.iter().map(|r| r.short_name.clone()).collect();
    let mut taken: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for (i, row) in sorted {
        let ty = (row.manufacturer.as_str(), row.model.as_str());
        let block = match types.iter().position(|(t, _)| *t == ty) {
            Some(b) => b,
            None => {
                types.push((ty, 0));
                types.len() - 1
            }
        };
        let (_, count) = types.get_mut(block).ok_or("type table out of sync")?;
        *count += 1;
        if *count > 99 {
            return Err(format!(
                "line {}: more than 99 fixtures of '{} {}'",
                row.line, row.manufacturer, row.model
            ));
        }
        let number = (block as u32 + 1) * 100 + *count;
        let mut name = row.short_name.clone();
        if let Some((prefix, _)) = split_name(&name).filter(|_| taken.contains(&name)) {
            let top = known
                .iter()
                .filter_map(|n| split_name(n))
                .filter(|(p, _)| *p == prefix)
                .map(|(_, n)| n)
                .max()
                .unwrap_or(0);
            name = format!("{prefix}{}", top + 1);
            known.push(name.clone());
        }
        taken.push(name.clone());
        out.push((i, number, name));
    }
    out.sort_by_key(|(i, _, _)| *i);
    Ok(out.into_iter().map(|(_, n, name)| (n, name)).collect())
}

/// `<manufacturer> - <model>.json`, as the daemon library names user profiles.
fn file_name(manufacturer: &str, model: &str) -> String {
    let name: String = format!("{manufacturer} - {model}")
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':') || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    format!("{name}.json")
}

fn write_atomic(path: &Path, contents: &str) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", path.display());
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(io)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, contents).map_err(io)?;
    fs::rename(&tmp, path).map_err(io)
}

/// `YYYY-MM-DDTHH:MM:SSZ`, as `cmdmx-daemon/src/shows.rs` writes it.
fn iso_utc(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z % 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// PAT-16..18: the whole run. Returns the report to print (Markdown table first).
fn run(tsv: &str, dirs: &Dirs) -> Result<String, String> {
    let rows = parse_export(tsv)?;
    let numbered = numbers_and_names(&rows)?;
    // One profile per type, footprint checked against the export (PAT-16).
    let mut picks: Vec<((&str, &str), Picked)> = Vec::new();
    for row in &rows {
        let ty = (row.manufacturer.as_str(), row.model.as_str());
        if !picks.iter().any(|(t, _)| *t == ty) {
            picks.push((ty, profile_for(ty.0, ty.1, &dirs.qlcplus_dir)?));
        }
    }
    let pick = |row: &Row| {
        let ty = (row.manufacturer.as_str(), row.model.as_str());
        picks.iter().find(|(t, _)| *t == ty).map(|(_, p)| p)
    };
    // Fixtures in number order, so the handles group by type (PAT-17).
    let mut fixtures: Vec<(u32, &str, &Row, &Picked)> = Vec::new();
    for (row, (number, name)) in rows.iter().zip(&numbered) {
        let picked = pick(row).ok_or("type table out of sync")?;
        let footprint = picked.profile.mode(Some(&picked.mode)).map(|m| m.footprint);
        if footprint != Some(row.channels) {
            return Err(format!(
                "line {}: '{} {}' mode '{}' has {} channels, the export says {}",
                row.line,
                row.manufacturer,
                row.model,
                picked.mode,
                footprint.unwrap_or(0),
                row.channels
            ));
        }
        fixtures.push((*number, name, row, picked));
    }
    fixtures.sort_by_key(|f| f.0);

    // PAT-18: every fixture goes through `patch_add`; a refusal stops before anything is written.
    let mut engine = Engine::new(Settings::default());
    for (_, picked) in &picks {
        engine
            .add_profile(&picked.key, picked.profile.clone())
            .map_err(|e| e.to_string())?;
    }
    let mut report = String::from(
        "| Number | Name | Profile | Mode | Address | Footprint |\n| --- | --- | --- | --- | --- | --- |\n",
    );
    for (number, name, row, picked) in &fixtures {
        let ids = engine
            .patch_add(&PatchAdd {
                profile: picked.key.clone(),
                mode: Some(picked.mode.clone()),
                count: 1,
                universe: row.universe,
                address: Some(row.address),
                number: Some(*number),
            })
            .map_err(|e| format!("line {}: {number} {name}: {e}", row.line))?;
        let id = ids.first().ok_or("patch_add returned no fixture")?;
        engine.patch_name(*id, name).map_err(|e| e.to_string())?;
        report.push_str(&format!(
            "| {number} | {name} | {} | {} | {}.{} | {} |\n",
            picked.key, picked.mode, row.universe, row.address, row.channels
        ));
    }

    let show_path = dirs.show_dir.join("Kerk.cmdmx.json");
    if show_path.exists() {
        return Err(format!(
            "the show Kerk already exists ({}); nothing written",
            show_path.display()
        ));
    }
    report.push('\n');
    for (_, picked) in picks.iter().filter(|(_, p)| p.user) {
        let file = file_name(&picked.profile.manufacturer, &picked.profile.model);
        let path = dirs.profiles_dir.join(&file);
        if path.exists() {
            report.push_str(&format!("- kept the existing profile {file}\n"));
        } else {
            write_atomic(&path, &picked.profile.to_json())?;
            report.push_str(&format!("- wrote the profile {file}\n"));
        }
    }
    let now = iso_utc(SystemTime::now());
    let meta = Meta {
        name: "Kerk".into(),
        created: now.clone(),
        modified: now,
        app_version: env!("CARGO_PKG_VERSION").into(),
    };
    write_atomic(&show_path, &engine.to_show(meta).to_json())?;
    report.push_str(&format!("- wrote the show {}\n", show_path.display()));
    Ok(report)
}

fn main() {
    if let Err(e) = real_main() {
        eprintln!("church: {e}");
        std::process::exit(1);
    }
}

fn real_main() -> Result<(), String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut dirs = Dirs {
        show_dir: PathBuf::from(&home).join("Documents/CMDMX/Shows"),
        profiles_dir: PathBuf::from(&home).join("Library/Application Support/CMDMX/profiles"),
        qlcplus_dir: PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../vendor/qlcplus/fixtures"
        )),
    };
    let mut export = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let dir = match arg.as_str() {
            "--show-dir" => &mut dirs.show_dir,
            "--profiles-dir" => &mut dirs.profiles_dir,
            "--qlcplus-dir" => &mut dirs.qlcplus_dir,
            _ => {
                export = Some(arg);
                continue;
            }
        };
        *dir = args.next().ok_or(format!("{arg} needs a path"))?.into();
    }
    let export = export.ok_or(
        "usage: church <export.tsv> [--show-dir PATH] [--profiles-dir PATH] [--qlcplus-dir PATH]",
    )?;
    let tsv = fs::read_to_string(&export).map_err(|e| format!("{export}: {e}"))?;
    print!("{}", run(&tsv, &dirs)?);
    Ok(())
}
