//! GDTF importer (VS2-41): `specs/modules/fixture-library.md` LIB-28..33.
//! The project's GDTF sources in `tests/fixtures/gdtf/<name>/description.xml` are zipped in
//! memory and must reproduce their `expected.json`; so must the `gdtf` crate's own test file.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use std::io::{Cursor, Write};
use std::path::PathBuf;

use cmdmx_core::attribute::{AttributeDef, AttributeGroup, AttributeKind};
use cmdmx_core::library::gdtf::{import, Import};
use cmdmx_core::profile::{ColourSystem, Mode, Profile};
use cmdmx_core::CoreError;
use serde_json::Value;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/gdtf")
        .canonicalize()
        .expect("tests/fixtures/gdtf exists at the repo root")
}

/// A `.gdtf` archive holding `files` (name, bytes).
fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, bytes) in files {
        w.start_file(*name, opts).unwrap();
        w.write_all(bytes).unwrap();
    }
    w.finish().unwrap().into_inner()
}

/// The archive of a fixture folder's `description.xml`.
fn archive(folder: &str) -> Vec<u8> {
    let xml = std::fs::read(fixtures_dir().join(folder).join("description.xml")).unwrap();
    zip_of(&[("description.xml", &xml)])
}

fn ok(bytes: &[u8], source: &str) -> Import {
    import(bytes, source).unwrap_or_else(|e| panic!("{source}: import failed: {e}"))
}

/// Equal JSON, numbers within `1e-3` (colour conversion, f32 rounding).
fn assert_close(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(e)) => {
            let (a, e) = (a.as_f64().unwrap(), e.as_f64().unwrap());
            assert!((a - e).abs() < 1e-3, "{path}: {a} != {e}");
        }
        (Value::Array(a), Value::Array(e)) => {
            assert_eq!(a.len(), e.len(), "{path}: length");
            for (i, (x, y)) in a.iter().zip(e).enumerate() {
                assert_close(x, y, &format!("{path}[{i}]"));
            }
        }
        (Value::Object(a), Value::Object(e)) => {
            let keys = |o: &serde_json::Map<String, Value>| o.keys().cloned().collect::<Vec<_>>();
            assert_eq!(keys(a), keys(e), "{path}: keys");
            for (k, v) in a {
                assert_close(v, &e[k], &format!("{path}.{k}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

fn assert_profile(imported: &Profile, expected_file: &str) {
    let text = std::fs::read_to_string(fixtures_dir().join(expected_file)).unwrap();
    let expected = Profile::from_json(&text).unwrap_or_else(|e| panic!("{expected_file}: {e}"));
    let (a, e) = (
        serde_json::to_value(imported).unwrap(),
        serde_json::to_value(&expected).unwrap(),
    );
    let context = format!(
        "{expected_file}: imported\n{}\nexpected\n{}",
        imported.to_json(),
        expected.to_json()
    );
    let result = std::panic::catch_unwind(|| assert_close(&a, &e, "profile"));
    assert!(result.is_ok(), "{context}");
    assert_eq!(imported.validate(), Ok(()), "{expected_file} passes LIB-02");
}

fn only_mode(i: &Import) -> &Mode {
    assert_eq!(i.profile.modes.len(), 1, "{:?}", i.profile.modes);
    &i.profile.modes[0]
}

fn attr<'a>(m: &'a Mode, name: &str) -> &'a AttributeDef {
    m.attribute(name)
        .unwrap_or_else(|| panic!("no attribute {name} in {:?}", m.attributes))
}

// ---- inline GDTF ---------------------------------------------------------------------------

/// Every Annex A attribute the inline tests use, with its feature.
const ATTRIBUTES: &[(&str, &str)] = &[
    ("Dimmer", "Dimmer.Dimmer"),
    ("Pan", "Position.PanTilt"),
    ("Tilt", "Position.PanTilt"),
    ("PanTiltSpeed", "Control.Control"),
    ("PositionMSpeed", "Control.Control"),
    ("Shutter1", "Beam.Beam"),
    ("Shutter2Strobe", "Beam.Beam"),
    ("Iris", "Beam.Beam"),
    ("Zoom", "Focus.Focus"),
    ("Focus1", "Focus.Focus"),
    ("Frost1", "Beam.Beam"),
    ("Gobo1", "Gobo.Gobo"),
    ("Gobo1Pos", "Gobo.Gobo"),
    ("Gobo1PosRotate", "Gobo.Gobo"),
    ("Gobo2", "Gobo.Gobo"),
    ("Prism1", "Beam.Beam"),
    ("Prism1PosRotate", "Beam.Beam"),
    ("Color1", "Color.Color"),
    ("Color2", "Color.Color"),
    ("ColorMacro1", "Color.Color"),
    ("CTO", "Color.HSB"),
    ("CTB", "Color.HSB"),
    ("ColorAdd_R", "Color.RGB"),
    ("ColorAdd_G", "Color.RGB"),
    ("ColorAdd_B", "Color.RGB"),
    ("ColorAdd_C", "Color.RGB"),
    ("ColorAdd_M", "Color.RGB"),
    ("ColorAdd_Y", "Color.RGB"),
    ("ColorAdd_RY", "Color.RGB"),
    ("ColorAdd_GY", "Color.RGB"),
    ("ColorAdd_UV", "Color.RGB"),
    ("ColorAdd_WW", "Color.RGB"),
    ("ColorAdd_CW", "Color.RGB"),
    ("ColorSub_C", "Color.CMY"),
    ("ColorSub_M", "Color.CMY"),
    ("ColorSub_Y", "Color.CMY"),
    ("ColorRGB_Red", "Color.RGB"),
    ("ColorRGB_Green", "Color.RGB"),
    ("ColorRGB_Blue", "Color.RGB"),
    ("Blade1A", "Shapers.Shapers"),
    ("Video1", "Video.Video"),
];

/// A one-fixture-type description with the given `<DMXMode>`s. Geometries: `Body`, `Head`.
fn gdtf(modes: &str) -> Vec<u8> {
    let attributes: String = ATTRIBUTES
        .iter()
        .map(|(n, f)| {
            format!(r#"<Attribute Name="{n}" Pretty="" Feature="{f}" PhysicalUnit="None"/>"#)
        })
        .collect();
    let groups: String = [
        ("Dimmer", "Dimmer"),
        ("Position", "PanTilt"),
        ("Control", "Control"),
        ("Beam", "Beam"),
        ("Focus", "Focus"),
        ("Gobo", "Gobo"),
        ("Color", "Color"),
        ("Shapers", "Shapers"),
        ("Video", "Video"),
    ]
    .iter()
    .map(|(g, f)| {
        let extra = if *g == "Color" {
            r#"<Feature Name="RGB"/><Feature Name="CMY"/><Feature Name="HSB"/>"#
        } else {
            ""
        };
        format!(
            r#"<FeatureGroup Name="{g}" Pretty="{g}"><Feature Name="{f}"/>{extra}</FeatureGroup>"#
        )
    })
    .collect();
    let m = "{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}";
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<GDTF DataVersion="1.2">
 <FixtureType Name="Unit" ShortName="Unit" LongName="Test Unit" Manufacturer="Test" Description="" FixtureTypeID="0F6B6A57-7C3B-4E8B-9C1E-2D7B1A0000FF" RefFT="" Thumbnail="">
  <AttributeDefinitions><ActivationGroups/><FeatureGroups>{groups}</FeatureGroups><Attributes>{attributes}</Attributes></AttributeDefinitions>
  <Wheels/>
  <PhysicalDescriptions><Emitters/><Filters/><ColorSpace Mode="sRGB"/><DMXProfiles/><CRIs/></PhysicalDescriptions>
  <Models><Model Name="Body" Length="1" Width="1" Height="1" PrimitiveType="Cube"/></Models>
  <Geometries><Geometry Name="Body" Model="Body" Position="{m}"><Geometry Name="Head" Model="Body" Position="{m}"/></Geometry></Geometries>
  <DMXModes>{modes}</DMXModes>
  <Revisions/><FTPresets/><Protocols/>
 </FixtureType>
</GDTF>"#
    );
    zip_of(&[("description.xml", xml.as_bytes())])
}

/// A DMX channel on `geometry` with one channel function, Default `default`.
fn chan_on(geometry: &str, offset: &str, attribute: &str, default: &str, extra: &str) -> String {
    format!(
        r#"<DMXChannel DMXBreak="1" Offset="{offset}" Highlight="None" Geometry="{geometry}">
 <LogicalChannel Attribute="{attribute}" Snap="No" Master="None" MibFade="0" DMXChangeTimeLimit="0">
  <ChannelFunction Name="F" Attribute="{attribute}" OriginalAttribute="" DMXFrom="0/1" Default="{default}" PhysicalFrom="0" PhysicalTo="1" RealFade="0" RealAcceleration="0" {extra}/>
 </LogicalChannel>
</DMXChannel>"#
    )
}

fn chan(offset: &str, attribute: &str) -> String {
    chan_on("Body", offset, attribute, "0/1", "")
}

/// A mode named `name` with channels `attributes` at offsets 1, 2, 3, …
fn mode_of(name: &str, attributes: &[&str]) -> String {
    let chans: String = attributes
        .iter()
        .enumerate()
        .map(|(i, a)| chan(&(i + 1).to_string(), a))
        .collect();
    mode_xml(name, &chans)
}

fn mode_xml(name: &str, channels: &str) -> String {
    format!(
        r#"<DMXMode Name="{name}" Geometry="Body"><DMXChannels>{channels}</DMXChannels><Relations/><FTMacros/></DMXMode>"#
    )
}

fn report_text(i: &Import) -> String {
    let lines = i.report.channels.iter().map(|l| l.became.as_str());
    lines
        .chain(i.report.warnings.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join("\n")
}

// ---- LIB-28: the project's files and the crate's ---------------------------------------------

#[test]
fn lib_28_project_files_reproduce_their_expected_profiles() {
    for folder in ["rgbw-par", "spot", "pixel-bar"] {
        let imported = ok(&archive(folder), &format!("gdtf:{folder}.gdtf"));
        assert_profile(&imported.profile, &format!("{folder}/expected.json"));
    }
}

#[test]
fn lib_28_crate_test_file_reproduces_its_expected_profile() {
    let name = "Generic@RGBW8@test.gdtf";
    let bytes = std::fs::read(fixtures_dir().join(name)).unwrap();
    let imported = ok(&bytes, &format!("gdtf:{name}"));
    assert_profile(&imported.profile, "Generic@RGBW8@test.expected.json");
}

#[test]
fn lib_28_model_is_long_name_else_name() {
    let par = ok(&archive("rgbw-par"), "gdtf:rgbw-par.gdtf");
    assert_eq!(par.profile.manufacturer, "Colourmotion Test");
    assert_eq!(par.profile.model, "Test RGBW Par");
    assert_eq!(par.profile.source, "gdtf:rgbw-par.gdtf");
    let bar = ok(&archive("pixel-bar"), "gdtf:pixel-bar.gdtf");
    assert_eq!(bar.profile.model, "PixelBar4", "LongName is empty");
}

#[test]
fn lib_28_every_mode_with_its_name_and_highest_offset() {
    let i = ok(
        &gdtf(&format!(
            "{}{}",
            mode_of("A", &["Dimmer", "Pan"]),
            // Offsets 1 and 5: the footprint is the highest offset used, not the channel count.
            mode_xml("B", &(chan("1", "Dimmer") + &chan("5", "Zoom")))
        )),
        "gdtf:unit.gdtf",
    );
    let modes: Vec<(&str, u16)> = (i.profile.modes.iter())
        .map(|m| (m.name.as_str(), m.footprint))
        .collect();
    assert_eq!(modes, [("A", 2), ("B", 5)]);
}

// ---- LIB-29: attribute mapping -------------------------------------------------------------

#[test]
fn lib_29_annex_a_attributes_map_to_names_groups_and_kinds() {
    use AttributeGroup::{B, C, E, G, I, P, S};
    use AttributeKind::{Htp, Ltp};
    let expected: &[(&str, &str, AttributeGroup, AttributeKind)] = &[
        ("Dimmer", "Dimmer", I, Htp),
        ("Pan", "Pan", P, Ltp),
        ("Tilt", "Tilt", P, Ltp),
        ("Shutter1", "Strobe", I, Ltp),
        ("Shutter2Strobe", "Strobe 2", I, Ltp),
        ("Iris", "Iris", B, Ltp),
        ("Zoom", "Zoom", B, Ltp),
        ("Focus1", "Focus", B, Ltp),
        ("Frost1", "Frost", B, Ltp),
        ("Gobo1", "Gobo", G, Ltp),
        ("Gobo1Pos", "Gobo Index", G, Ltp),
        ("Gobo1PosRotate", "Gobo Rotation", G, Ltp),
        ("Gobo2", "Gobo 2", G, Ltp),
        ("Prism1", "Prism", E, Ltp),
        ("Prism1PosRotate", "Prism Rotation", E, Ltp),
        ("Color1", "Colour Wheel", C, Ltp),
        ("Color2", "Colour Wheel 2", C, Ltp),
        ("ColorMacro1", "Colour Macro", C, Ltp),
        ("CTO", "CTO", C, Ltp),
        ("CTB", "CTB", C, Ltp),
        ("PanTiltSpeed", "Pan/Tilt Speed", S, Ltp),
    ];
    let attrs: Vec<&str> = expected.iter().map(|e| e.0).collect();
    let i = ok(&gdtf(&mode_of("M", &attrs)), "gdtf:unit.gdtf");
    let m = only_mode(&i);
    for (k, (gdtf_name, name, group, kind)) in expected.iter().enumerate() {
        let a = attr(m, name);
        assert_eq!(a.channel as usize, k + 1, "{gdtf_name} → {name}: channel");
        assert_eq!((a.group, a.kind), (*group, *kind), "{gdtf_name} → {name}");
        assert!(!a.emitter, "{gdtf_name} is not an emitter");
    }
    assert_eq!(m.colour, None);
    assert!(!m.virtual_dimmer);

    let speed = ok(&gdtf(&mode_of("M", &["PositionMSpeed"])), "gdtf:unit.gdtf");
    assert_eq!(attr(only_mode(&speed), "Pan/Tilt Speed").group, S);
}

#[test]
fn lib_29_rgb_and_cmy_systems_and_emitters() {
    let i = ok(
        &gdtf(&format!(
            "{}{}{}{}",
            // RGB beside subtractive C/M/Y: the C/M/Y are emitters (LIB-17).
            mode_of(
                "rgb+cmy",
                &[
                    "ColorAdd_R",
                    "ColorAdd_G",
                    "ColorAdd_B",
                    "ColorSub_C",
                    "ColorSub_M",
                    "ColorSub_Y"
                ]
            ),
            mode_of("cmy", &["Dimmer", "ColorSub_C", "ColorSub_M", "ColorSub_Y"]),
            mode_of(
                "rgb+emitters",
                &[
                    "ColorRGB_Red",
                    "ColorRGB_Green",
                    "ColorRGB_Blue",
                    "ColorAdd_RY",
                    "ColorAdd_GY",
                    "ColorAdd_UV",
                    "ColorAdd_WW",
                    "ColorAdd_CW",
                ]
            ),
            // Additive C/M/Y are always emitters, never a CMY system.
            mode_of("add-cmy", &["ColorAdd_C", "ColorAdd_M", "ColorAdd_Y"]),
        )),
        "gdtf:unit.gdtf",
    );
    let mode = |n: &str| i.profile.mode(Some(n)).unwrap();
    let emitters = |m: &Mode| m.emitters().map(|a| a.name.clone()).collect::<Vec<_>>();

    let m = mode("rgb+cmy");
    let colour = m.colour.as_ref().unwrap();
    assert_eq!(colour.system, ColourSystem::Rgb);
    assert_eq!(colour.components.map(|c| c.channel), [1, 2, 3]);
    assert_eq!(emitters(m), ["Cyan", "Magenta", "Yellow"]);
    assert!(m.virtual_dimmer, "colour without a Dimmer channel (LIB-08)");

    let m = mode("cmy");
    let colour = m.colour.as_ref().unwrap();
    assert_eq!(colour.system, ColourSystem::Cmy);
    assert_eq!(colour.components.map(|c| c.channel), [2, 3, 4]);
    assert!(!m.virtual_dimmer, "has a Dimmer");

    let m = mode("rgb+emitters");
    assert_eq!(m.colour.as_ref().unwrap().system, ColourSystem::Rgb);
    assert_eq!(
        emitters(m),
        ["Amber", "Lime", "UV", "Warm White", "Cold White"]
    );
    for a in m.emitters() {
        assert_eq!((a.group, a.kind), (AttributeGroup::C, AttributeKind::Ltp));
    }

    let m = mode("add-cmy");
    assert_eq!(m.colour, None);
    assert_eq!(emitters(m), ["Cyan", "Magenta", "Yellow"]);
}

#[test]
fn lib_29_other_attributes_are_plain_channels_grouped_by_feature() {
    let i = ok(
        &gdtf(&mode_of("M", &["Blade1A", "Video1"])),
        "gdtf:unit.gdtf",
    );
    let m = only_mode(&i);
    // Pretty is empty, so the attribute's Name is used.
    assert_eq!(attr(m, "Blade1A").group, AttributeGroup::B, "Shapers → B");
    assert_eq!(attr(m, "Video1").group, AttributeGroup::S, "Video → S");
    assert_eq!(attr(m, "Video1").kind, AttributeKind::Ltp);
    let plain = i
        .report
        .channels
        .iter()
        .filter(|l| l.became.contains("kept as a plain channel"))
        .count();
    assert_eq!(plain, 2, "{:?}", i.report.channels);
    assert!(
        !i.report.warnings.is_empty(),
        "unknown attributes warn (LIB-33)"
    );

    // The spot's Control1 has Pretty "Ctrl".
    let spot = ok(&archive("spot"), "gdtf:spot.gdtf");
    assert_eq!(attr(only_mode(&spot), "Ctrl").group, AttributeGroup::S);
}

#[test]
fn lib_29_clashing_names_stay_unique() {
    let chans = chan_on("Body", "1", "Zoom", "0/1", "") + &chan_on("Head", "2", "Zoom", "0/1", "");
    let i = ok(&gdtf(&mode_xml("M", &chans)), "gdtf:unit.gdtf");
    let m = only_mode(&i);
    assert_eq!(m.attributes.len(), 2);
    assert_ne!(m.attributes[0].name, m.attributes[1].name);
    assert_eq!(i.profile.validate(), Ok(()));
}

// ---- LIB-30: channels and values -----------------------------------------------------------

#[test]
fn lib_30_fine_bytes_and_unused_third_byte() {
    let chans = chan("1,2,3", "Dimmer")
        + &chan("4,5", "ColorAdd_R")
        + &chan("6", "ColorAdd_G")
        + &chan("7", "ColorAdd_B");
    let i = ok(&gdtf(&mode_xml("M", &chans)), "gdtf:unit.gdtf");
    let m = only_mode(&i);
    let dimmer = attr(m, "Dimmer");
    assert_eq!((dimmer.channel, dimmer.fine_channel), (1, Some(2)));
    let red = m.colour.as_ref().unwrap().components[0];
    assert_eq!((red.channel, red.fine_channel), (4, Some(5)));
    assert_eq!(m.footprint, 7, "the third byte still counts");
    assert!(
        i.report.warnings.iter().any(|w| w.contains("Dimmer")),
        "a 24-bit channel warns: {:?}",
        i.report.warnings
    );
}

#[test]
fn lib_30_virtual_channels_are_reported_not_imported() {
    let i = ok(&archive("rgbw-par"), "gdtf:rgbw-par.gdtf");
    let m = i.profile.mode(Some("4ch")).unwrap();
    assert!(m.attribute("Dimmer").is_none());
    assert!(m.virtual_dimmer, "LIB-08 gives the mode a virtual Dimmer");
    assert!(
        report_text(&i).to_lowercase().contains("virtual"),
        "{:#?}",
        i.report
    );
}

#[test]
fn lib_30_defaults_and_highlight_scale_by_byte_count() {
    let chans = chan_on("Body", "1", "Focus1", "128/1", "")
        + &chan_on("Body", "2,3", "Zoom", "32768/2", "");
    let i = ok(&gdtf(&mode_xml("M", &chans)), "gdtf:unit.gdtf");
    let m = only_mode(&i);
    assert!((attr(m, "Focus").default - 128.0 / 255.0).abs() < 1e-6);
    assert!((attr(m, "Zoom").default - 32768.0 / 65535.0).abs() < 1e-6);

    let spot = ok(&archive("spot"), "gdtf:spot.gdtf");
    assert_eq!(attr(only_mode(&spot), "Dimmer").highlight, Some(1.0));
    assert_eq!(attr(only_mode(&spot), "Pan").highlight, None);
}

#[test]
fn lib_30_slots_from_channel_sets_or_functions() {
    let par = ok(&archive("rgbw-par"), "gdtf:rgbw-par.gdtf");
    let strobe = attr(par.profile.mode(Some("6ch")).unwrap(), "Strobe");
    let slots: Vec<(&str, u8, u8)> = (strobe.slots.iter())
        .map(|s| (s.name.as_str(), s.from, s.to))
        .collect();
    assert_eq!(
        slots,
        [("Open", 0, 9), ("Strobe", 10, 199), ("Random", 200, 255)]
    );

    let spot = ok(&archive("spot"), "gdtf:spot.gdtf");
    let wheel = attr(only_mode(&spot), "Colour Wheel");
    let red = wheel.slots.iter().find(|s| s.name == "Red").unwrap();
    let c = red.colour.unwrap();
    assert!(
        (c.r - 1.0).abs() < 1e-3 && c.g < 1e-3 && c.b < 1e-3,
        "{c:?}"
    );
    let open = wheel
        .slots
        .iter()
        .find(|s| s.name == "Open")
        .unwrap()
        .colour
        .unwrap();
    assert!(
        open.r > 0.999 && open.g > 0.999 && open.b > 0.999,
        "D65 → white: {open:?}"
    );
    let gobo = attr(only_mode(&spot), "Gobo");
    assert!(
        gobo.slots.iter().all(|s| s.colour.is_none()),
        "only colour wheels get colours"
    );
}

#[test]
fn lib_30_physical_ranges_and_gdtf_default_range_is_unknown() {
    let spot = ok(&archive("spot"), "gdtf:spot.gdtf");
    let m = only_mode(&spot);
    assert_eq!(m.pan_degrees, Some(540.0));
    assert_eq!(m.tilt_degrees, Some(270.0));
    assert_eq!(m.zoom_degrees, Some([10.0, 40.0]));

    // chan() writes PhysicalFrom 0, PhysicalTo 1: GDTF's default, so unknown.
    let i = ok(
        &gdtf(&mode_of("M", &["Pan", "Tilt", "Zoom"])),
        "gdtf:unit.gdtf",
    );
    let m = only_mode(&i);
    assert_eq!(
        (m.pan_degrees, m.tilt_degrees, m.zoom_degrees),
        (None, None, None)
    );
}

// ---- LIB-31: cells from geometry -----------------------------------------------------------

#[test]
fn lib_31_geometry_references_become_cells_with_break_offsets() {
    let i = ok(&archive("pixel-bar"), "gdtf:pixel-bar.gdtf");
    let m = only_mode(&i);
    assert_eq!(m.footprint, 13);
    assert_eq!(
        attr(m, "Dimmer").channel,
        1,
        "the master dimmer belongs to the mode"
    );
    let starts: Vec<u16> = (m.cells.iter())
        .map(|c| c.colour.as_ref().unwrap().components[0].channel)
        .collect();
    assert_eq!(starts, [2, 5, 8, 11]);
    assert!(!m.virtual_dimmer);
}

#[test]
fn lib_31_colour_on_two_geometries_makes_two_cells() {
    let on = |g: &str, o: u16, a: &str| chan_on(g, &o.to_string(), a, "0/1", "");
    let chans = on("Body", 1, "Dimmer")
        + &on("Body", 2, "ColorAdd_R")
        + &on("Body", 3, "ColorAdd_G")
        + &on("Body", 4, "ColorAdd_B")
        + &on("Head", 5, "ColorAdd_R")
        + &on("Head", 6, "ColorAdd_G")
        + &on("Head", 7, "ColorAdd_B")
        + &on("Head", 8, "ColorAdd_UV");
    let i = ok(&gdtf(&mode_xml("M", &chans)), "gdtf:unit.gdtf");
    let m = only_mode(&i);
    assert_eq!(m.cells.len(), 2);
    assert_eq!(m.colour, None, "colour lives in the cells");
    // Every channel of a cell's geometry belongs to the cell, the Dimmer on Body too:
    // Body carries colour here, so Body is a cell.
    assert!(m.cells[0].attribute("Dimmer").is_some());
    assert!(m.cells[1].attribute("UV").is_some_and(|a| a.emitter));
}

// ---- LIB-32: emitter colours ---------------------------------------------------------------

#[test]
fn lib_32_linked_emitters_store_their_cie() {
    let i = ok(&archive("rgbw-par"), "gdtf:rgbw-par.gdtf");
    let m = i.profile.mode(Some("4ch")).unwrap();
    let c = m.colour.as_ref().unwrap().components;
    assert_eq!(c[0].cie, Some([0.7, 0.29]));
    assert_eq!(c[1].cie, Some([0.17, 0.7]));
    assert_eq!(c[2].cie, None, "Blue links no emitter");
    assert_eq!(attr(m, "White").cie, Some([0.33, 0.34]));
    let blue = (i.report.channels.iter())
        .find(|l| l.mode == "4ch" && l.channel == 3)
        .unwrap();
    assert!(
        blue.became.to_lowercase().contains("cie"),
        "the report says Blue has no CIE colour: {}",
        blue.became
    );
}

#[test]
fn lib_32_cie_round_trips_and_is_validated() {
    let i = ok(&archive("rgbw-par"), "gdtf:rgbw-par.gdtf");
    let json = i.profile.to_json();
    assert!(json.contains("\"cie\""));
    assert_eq!(Profile::from_json(&json), Ok(i.profile.clone()), "LIB-01");

    let profile = |attr: &str, comp: &str| {
        format!(
            r#"{{"modes":[{{"name":"M","footprint":4,
              "attributes":[{attr}],
              "colour":{{"system":"RGB","components":[{comp},{{"channel":2}},{{"channel":3}}]}}}}]}}"#
        )
    };
    let white =
        |extra: &str| format!(r#"{{"name":"White","group":"C","kind":"Ltp","channel":4{extra}}}"#);
    let good = profile(
        &white(r#","emitter":true,"cie":[0.3,0.3]"#),
        r#"{"channel":1,"cie":[0.7,0.29]}"#,
    );
    assert!(Profile::from_json(&good).is_ok(), "{good}");
    for (what, json) in [
        (
            "cie on an attribute that is not an emitter",
            profile(&white(r#","cie":[0.3,0.3]"#), r#"{"channel":1}"#),
        ),
        (
            "x outside 0..1",
            profile(
                &white(r#","emitter":true,"cie":[1.2,0.1]"#),
                r#"{"channel":1}"#,
            ),
        ),
        (
            "x + y > 1 on a component",
            profile(
                &white(r#","emitter":true"#),
                r#"{"channel":1,"cie":[0.7,0.5]}"#,
            ),
        ),
    ] {
        assert!(
            matches!(
                Profile::from_json(&json),
                Err(CoreError::InvalidProfile { .. })
            ),
            "LIB-02 rejects {what}"
        );
    }
}

// ---- LIB-33: failure -----------------------------------------------------------------------

fn fails(bytes: &[u8], case: &str) {
    let source = format!("gdtf:{case}.gdtf");
    match import(bytes, &source) {
        Err(CoreError::ImportFailed { source: s, problem }) => {
            assert_eq!(s, source, "{case}: names the file");
            assert!(!problem.is_empty(), "{case}: names the problem");
        }
        other => panic!("{case}: expected ImportFailed, got {other:?}"),
    }
}

#[test]
fn lib_33_broken_files_fail_with_the_file_and_the_problem() {
    fails(b"this is not a zip archive", "not-a-zip");
    fails(&zip_of(&[("other.xml", b"<GDTF/>")]), "no-description");
    for case in ["broken-xml", "no-fixture-type", "no-mode-left"] {
        fails(&archive(case), case);
    }
}

#[test]
fn lib_33_a_mode_on_another_dmx_break_is_skipped_with_a_warning() {
    let broken = mode_of("two", &["Dimmer"]).replace(r#"DMXBreak="1""#, r#"DMXBreak="2""#);
    let i = ok(
        &gdtf(&format!("{}{broken}", mode_of("one", &["Dimmer"]))),
        "gdtf:unit.gdtf",
    );
    let names: Vec<&str> = i.profile.modes.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, ["one"]);
    assert!(
        i.report.warnings.iter().any(|w| w.contains("two")),
        "{:?}",
        i.report.warnings
    );
}
