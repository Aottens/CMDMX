//! QLC+ `.qxf` importer (VS2-09): `specs/modules/fixture-library.md` LIB-05..10, 15, 17, 18.
//! LIB-09: the church files in `tests/fixtures/qlcplus/` must reproduce their `.expected.json`.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use std::path::PathBuf;

use cmdmx_core::attribute::{AttributeDef, AttributeGroup, AttributeKind};
use cmdmx_core::library::qlcplus::{import, Import};
use cmdmx_core::profile::{ColourSystem, Mode, Profile, TargetColour};
use cmdmx_core::CoreError;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/qlcplus")
        .canonicalize()
        .expect("tests/fixtures/qlcplus exists at the repo root")
}

fn read(name: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join(name)).unwrap()
}

/// A minimal fixture definition with the given channel definitions and modes.
fn qxf(channels: &str, modes: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE FixtureDefinition>
<FixtureDefinition xmlns="http://www.qlcplus.org/FixtureDefinition">
 <Manufacturer>Test</Manufacturer>
 <Model>Unit</Model>
 <Type>Color Changer</Type>
 {channels}
 {modes}
</FixtureDefinition>"#
    )
}

/// One mode "M" listing `names` in order.
fn mode(names: &[&str]) -> String {
    let channels: String = names
        .iter()
        .enumerate()
        .map(|(i, n)| format!(r#"<Channel Number="{i}">{n}</Channel>"#))
        .collect();
    format!(r#"<Mode Name="M">{channels}</Mode>"#)
}

fn preset(name: &str, preset: &str) -> String {
    format!(r#"<Channel Name="{name}" Preset="{preset}"/>"#)
}

fn ok(xml: &str) -> Import {
    import(xml, "test.qxf").unwrap_or_else(|e| panic!("import failed: {e}"))
}

fn only_mode(i: &Import) -> &Mode {
    assert_eq!(i.profile.modes.len(), 1);
    &i.profile.modes[0]
}

fn attr<'a>(m: &'a Mode, name: &str) -> &'a AttributeDef {
    m.attribute(name)
        .unwrap_or_else(|| panic!("no attribute {name} in {:?}", m.attributes))
}

fn names(m: &Mode) -> Vec<&str> {
    m.attributes.iter().map(|a| a.name.as_str()).collect()
}

// ---- LIB-09: the church fixtures --------------------------------------------------------

#[test]
fn lib_09_church_files_reproduce_their_expected_profiles() {
    for (file, folder) in [
        ("Showtec-LED-Par-56", "Showtec"),
        ("Pro-Lights-LumiPar-7QPro", "Pro-Lights"),
        ("Shehds-LED-Beam+Wash-19x15W-RGBW-Zoom", "Shehds"),
    ] {
        let source = format!("qlcplus:{folder}/{file}.qxf");
        let imported = import(&read(&format!("{file}.qxf")), &source)
            .unwrap_or_else(|e| panic!("{file}: {e}"));
        let expected = Profile::from_json(&read(&format!("{file}.expected.json")))
            .unwrap_or_else(|e| panic!("{file}.expected.json: {e}"));
        assert_eq!(
            imported.profile,
            expected,
            "{file}: imported\n{}\nexpected\n{}",
            imported.profile.to_json(),
            expected.to_json()
        );
        assert_eq!(imported.profile.validate(), Ok(()), "{file} passes LIB-02");
    }
}

#[test]
fn lib_09_report_says_what_each_channel_became() {
    let i = import(
        &read("Showtec-LED-Par-56.qxf"),
        "qlcplus:Showtec/Showtec-LED-Par-56.qxf",
    )
    .unwrap();
    assert_eq!(i.report.channels.len(), 6, "one line per channel per mode");
    let line = |n: u16| {
        i.report
            .channels
            .iter()
            .find(|l| l.mode == "6 Channel" && l.channel == n)
            .unwrap()
    };
    assert_eq!(line(6).source_name, "Mode");
    assert!(
        line(6).became.contains("plain channel"),
        "{}",
        line(6).became
    );
    assert!(
        !line(1).became.contains("plain channel"),
        "{}",
        line(1).became
    );
    assert!(line(1).became.contains("Red"), "{}", line(1).became);
}

// ---- LIB-05 / LIB-08: presets ------------------------------------------------------------

#[test]
fn lib_05_presets_map_to_attributes_and_colour_systems() {
    let xml = qxf(
        &[
            preset("Dim", "IntensityMasterDimmer"),
            preset("Dim fine", "IntensityDimmerFine"),
            preset("C", "IntensityCyan"),
            preset("M", "IntensityMagenta"),
            preset("Y", "IntensityYellow"),
            preset("P", "PositionPan"),
            preset("T", "PositionTilt"),
            preset("T fine", "PositionTiltFine"),
            preset("Shutter", "ShutterStrobeFastSlow"),
            preset("Iris", "ShutterIrisMinToMax"),
            preset("Zoom", "BeamZoomSmallBig"),
            preset("Wheel", "ColorWheel"),
            preset("Gobo", "GoboWheel"),
            preset("Gobo rot", "GoboIndex"),
            preset("Prism", "PrismRotationSlowFast"),
            preset("CTO", "ColorCTOMixer"),
        ]
        .concat(),
        &mode(&[
            "Dim", "Dim fine", "C", "M", "Y", "P", "T", "T fine", "Shutter", "Iris", "Zoom",
            "Wheel", "Gobo", "Gobo rot", "Prism", "CTO",
        ]),
    );
    let i = ok(&xml);
    let m = only_mode(&i);
    assert_eq!(m.footprint, 16);
    assert!(
        !m.virtual_dimmer,
        "LIB-08: a dimmer channel means no virtual dimmer"
    );
    let colour = m.colour.as_ref().unwrap();
    assert_eq!(
        colour.system,
        ColourSystem::Cmy,
        "C/M/Y without RGB is a CMY system"
    );
    let channels: Vec<u16> = colour.components.iter().map(|c| c.channel).collect();
    assert_eq!(channels, [3, 4, 5]);
    let expect = [
        ("Dimmer", AttributeGroup::I, AttributeKind::Htp, 1, Some(2)),
        ("Pan", AttributeGroup::P, AttributeKind::Ltp, 6, None),
        ("Tilt", AttributeGroup::P, AttributeKind::Ltp, 7, Some(8)),
        ("Strobe", AttributeGroup::I, AttributeKind::Ltp, 9, None),
        ("Iris", AttributeGroup::B, AttributeKind::Ltp, 10, None),
        ("Zoom", AttributeGroup::B, AttributeKind::Ltp, 11, None),
        (
            "Colour Wheel",
            AttributeGroup::C,
            AttributeKind::Ltp,
            12,
            None,
        ),
        ("Gobo", AttributeGroup::G, AttributeKind::Ltp, 13, None),
        (
            "Gobo Index",
            AttributeGroup::G,
            AttributeKind::Ltp,
            14,
            None,
        ),
        (
            "Prism Rotation",
            AttributeGroup::E,
            AttributeKind::Ltp,
            15,
            None,
        ),
        ("CTO", AttributeGroup::C, AttributeKind::Ltp, 16, None),
    ];
    assert_eq!(names(m), expect.iter().map(|e| e.0).collect::<Vec<_>>());
    for (name, group, kind, channel, fine) in expect {
        let a = attr(m, name);
        assert_eq!(
            (a.group, a.kind, a.channel, a.fine_channel, a.emitter),
            (group, kind, channel, fine, false),
            "{name}"
        );
    }
}

#[test]
fn lib_08_colour_without_dimmer_has_a_virtual_dimmer() {
    let xml = qxf(
        &[
            preset("R", "IntensityRed"),
            preset("G", "IntensityGreen"),
            preset("B", "IntensityBlue"),
        ]
        .concat(),
        &mode(&["R", "G", "B"]),
    );
    assert!(only_mode(&ok(&xml)).virtual_dimmer);
}

#[test]
fn lib_05_fine_without_its_coarse_is_a_plain_channel_with_a_warning() {
    let xml = qxf(&preset("Pan fine", "PositionPanFine"), &mode(&["Pan fine"]));
    let i = ok(&xml);
    let a = attr(only_mode(&i), "Pan fine");
    assert_eq!((a.channel, a.fine_channel), (1, None));
    assert!(i.report.warnings.iter().any(|w| w.contains("Pan fine")));
}

// ---- LIB-06: channels without a preset -----------------------------------------------------

#[test]
fn lib_06_groups_map_and_colour_hints_count_as_presets() {
    let plain = |name: &str, group: &str, extra: &str| {
        format!(r#"<Channel Name="{name}"><Group Byte="0">{group}</Group>{extra}</Channel>"#)
    };
    let xml = qxf(
        &[
            plain("Red", "Intensity", "<Colour>Red</Colour>"),
            plain("Green", "Intensity", "<Colour>Green</Colour>"),
            plain("Blue", "Intensity", "<Colour>Blue</Colour>"),
            plain("Level", "Intensity", ""),
            plain("Frost", "Beam", ""),
            plain("FX", "Prism", ""),
            plain("Gobo Shake", "Gobo", ""),
            plain("Move", "Pan", ""),
            plain("Lamp", "Maintenance", ""),
            plain("Nothing", "Nothing", ""),
        ]
        .concat(),
        &mode(&[
            "Red",
            "Green",
            "Blue",
            "Level",
            "Frost",
            "FX",
            "Gobo Shake",
            "Move",
            "Lamp",
            "Nothing",
        ]),
    );
    let i = ok(&xml);
    let m = only_mode(&i);
    assert_eq!(m.colour.as_ref().unwrap().system, ColourSystem::Rgb);
    let expect = [
        ("Level", AttributeGroup::I),
        ("Frost", AttributeGroup::B),
        ("FX", AttributeGroup::E),
        ("Gobo Shake", AttributeGroup::G),
        ("Move", AttributeGroup::P),
        ("Lamp", AttributeGroup::S),
        ("Nothing", AttributeGroup::S),
    ];
    assert_eq!(names(m), expect.iter().map(|e| e.0).collect::<Vec<_>>());
    for (name, group) in expect {
        let a = attr(m, name);
        assert_eq!(
            (a.group, a.kind, a.default),
            (group, AttributeKind::Ltp, 0.0),
            "{name}"
        );
    }
    let plain_lines = i
        .report
        .channels
        .iter()
        .filter(|l| l.became.contains("plain channel"))
        .count();
    assert_eq!(plain_lines, 7);
}

// ---- LIB-10: failures and warnings -----------------------------------------------------------

fn failure(xml: &str) -> String {
    match import(xml, "broken.qxf") {
        Err(CoreError::ImportFailed { source, problem }) => {
            assert_eq!(source, "broken.qxf");
            problem
        }
        other => panic!("expected ImportFailed, got {other:?}"),
    }
}

#[test]
fn lib_10_malformed_files_fail_with_the_file_and_the_problem() {
    assert!(!failure("<FixtureDefinition><Model>").is_empty());
    let unknown = failure(&qxf(&preset("R", "IntensityRed"), &mode(&["R", "Ghost"])));
    assert!(unknown.contains("Ghost"), "{unknown}");
    let outside = failure(&qxf(
        &preset("R", "IntensityRed"),
        r#"<Mode Name="M"><Channel Number="3">R</Channel></Mode>"#,
    ));
    assert!(outside.contains('3'), "{outside}");
    let twice = failure(&qxf(
        &[preset("R", "IntensityRed"), preset("G", "IntensityGreen")].concat(),
        r#"<Mode Name="M"><Channel Number="0">R</Channel><Channel Number="0">G</Channel></Mode>"#,
    ));
    assert!(twice.contains('0'), "{twice}");
}

#[test]
fn lib_10_unknown_preset_is_a_plain_channel_with_a_warning() {
    let i = ok(&qxf(&preset("Odd", "IntensityPlasma"), &mode(&["Odd"])));
    let a = attr(only_mode(&i), "Odd");
    assert_eq!(a.channel, 1);
    assert!(i
        .report
        .warnings
        .iter()
        .any(|w| w.contains("IntensityPlasma")));
}

// ---- LIB-15 / LIB-18: slots --------------------------------------------------------------------

#[test]
fn lib_15_wheel_slots_keep_names_and_single_colours() {
    let xml = qxf(
        r##"<Channel Name="Colour" Preset="ColorWheel">
              <Capability Min="0" Max="9" Preset="ColorMacro" Res1="#ff0000">Red</Capability>
              <Capability Min="10" Max="19" Color="#808080">Grey (old format)</Capability>
              <Capability Min="20" Max="29" Preset="ColorDoubleMacro" Res1="#ff0000" Res2="#0000ff">Red/Blue</Capability>
              <Capability Min="30" Max="255">Rotation</Capability>
            </Channel>
            <Channel Name="Gobos" Preset="GoboWheel">
              <Capability Min="0" Max="127" Res1="Others/open.svg">Open</Capability>
              <Capability Min="128" Max="255" Res1="Others/gobo00001.svg">Dots</Capability>
            </Channel>
            <Channel Name="Reset"><Group Byte="0">Maintenance</Group>
              <Capability Min="0" Max="255">Reset</Capability>
            </Channel>"##,
        &mode(&["Colour", "Gobos", "Reset"]),
    );
    let i = ok(&xml);
    let m = only_mode(&i);
    let wheel = &attr(m, "Colour Wheel").slots;
    let named: Vec<(&str, u8, u8)> = wheel
        .iter()
        .map(|s| (s.name.as_str(), s.from, s.to))
        .collect();
    assert_eq!(
        named,
        [
            ("Red", 0, 9),
            ("Grey (old format)", 10, 19),
            ("Red/Blue", 20, 29),
            ("Rotation", 30, 255)
        ]
    );
    assert_eq!(
        wheel[0].colour,
        Some(TargetColour {
            r: 1.0,
            g: 0.0,
            b: 0.0
        })
    );
    let grey = wheel[1].colour.unwrap();
    assert!(
        (grey.r - 0.21586).abs() < 1e-4 && grey.r == grey.g && grey.g == grey.b,
        "{grey:?}"
    );
    assert_eq!(wheel[2].colour, None, "two-colour range has no colour");
    assert_eq!(wheel[3].colour, None);
    let gobos: Vec<&str> = attr(m, "Gobo")
        .slots
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    assert_eq!(gobos, ["Open", "Dots"]);
    assert!(
        attr(m, "Reset").slots.is_empty(),
        "one range is not a list of slots"
    );
}

// ---- LIB-17 / LIB-18: emitters and names ------------------------------------------------------

#[test]
fn lib_17_cyan_next_to_rgb_is_an_emitter() {
    let xml = qxf(
        &[
            preset("R", "IntensityRed"),
            preset("G", "IntensityGreen"),
            preset("B", "IntensityBlue"),
            preset("A", "IntensityAmber"),
            preset("C", "IntensityCyan"),
            preset("L", "IntensityLime"),
            preset("U", "IntensityUV"),
        ]
        .concat(),
        &mode(&["R", "G", "B", "A", "C", "L", "U"]),
    );
    let i = ok(&xml);
    let m = only_mode(&i);
    assert_eq!(m.colour.as_ref().unwrap().system, ColourSystem::Rgb);
    let emitters: Vec<(&str, u16)> = m.emitters().map(|a| (a.name.as_str(), a.channel)).collect();
    assert_eq!(
        emitters,
        [("Amber", 4), ("Cyan", 5), ("Lime", 6), ("UV", 7)]
    );
    assert!(m
        .emitters()
        .all(|a| a.group == AttributeGroup::C && a.kind == AttributeKind::Ltp));
}

#[test]
fn lib_17_two_whites_take_their_channel_names() {
    let xml = qxf(
        &[
            preset("Dimmer", "IntensityDimmer"),
            r#"<Channel Name="Warm White"><Group Byte="0">Intensity</Group><Colour>White</Colour></Channel>"#.to_string(),
            preset("Cold White", "IntensityWhite"),
        ]
        .concat(),
        &mode(&["Dimmer", "Warm White", "Cold White"]),
    );
    let i = ok(&xml);
    let emitters: Vec<&str> = only_mode(&i).emitters().map(|a| a.name.as_str()).collect();
    assert_eq!(emitters, ["Warm White", "Cold White"]);
}

#[test]
fn lib_18_clashing_names_take_their_channel_names() {
    // Both presets map to "Pan/Tilt Speed" (LIB-05); the ` 2` suffix is a safety net only.
    let xml = qxf(
        &[
            preset("Pan speed", "SpeedPanSlowFast"),
            preset("Tilt speed", "SpeedTiltSlowFast"),
            preset("Speed", "SpeedPanTiltSlowFast"),
        ]
        .concat(),
        &mode(&["Pan speed", "Tilt speed", "Speed"]),
    );
    let i = ok(&xml);
    assert_eq!(names(only_mode(&i)), ["Pan speed", "Tilt speed", "Speed"]);
}

#[test]
fn lib_18_qlcplus_default_becomes_the_attribute_default() {
    let xml = qxf(
        r#"<Channel Name="Mode" Default="128"><Group Byte="0">Maintenance</Group></Channel>"#,
        &mode(&["Mode"]),
    );
    let d = attr(only_mode(&ok(&xml)), "Mode").default;
    assert!((d - 128.0 / 255.0).abs() < 1e-6, "{d}");
}

#[test]
fn lib_18_head_channels_form_cells() {
    let xml = qxf(
        &[
            preset("Dimmer", "IntensityMasterDimmer"),
            preset("R1", "IntensityRed"),
            preset("G1", "IntensityGreen"),
            preset("B1", "IntensityBlue"),
            preset("D1", "IntensityDimmer"),
            preset("R2", "IntensityRed"),
            preset("G2", "IntensityGreen"),
            preset("B2", "IntensityBlue"),
            preset("D2", "IntensityDimmer"),
        ]
        .concat(),
        r#"<Mode Name="M">
             <Channel Number="0">Dimmer</Channel>
             <Channel Number="1">R1</Channel><Channel Number="2">G1</Channel>
             <Channel Number="3">B1</Channel><Channel Number="4">D1</Channel>
             <Channel Number="5">R2</Channel><Channel Number="6">G2</Channel>
             <Channel Number="7">B2</Channel><Channel Number="8">D2</Channel>
             <Head><Channel>1</Channel><Channel>2</Channel><Channel>3</Channel><Channel>4</Channel></Head>
             <Head><Channel>5</Channel><Channel>6</Channel><Channel>7</Channel><Channel>8</Channel></Head>
           </Mode>"#,
    );
    let i = ok(&xml);
    let m = only_mode(&i);
    assert!(m.colour.is_none(), "all colour channels are in heads");
    assert_eq!(names(m), ["Dimmer"]);
    assert_eq!(m.cells.len(), 2);
    let cell = &m.cells[1];
    let rgb: Vec<u16> = cell
        .colour
        .as_ref()
        .unwrap()
        .components
        .iter()
        .map(|c| c.channel)
        .collect();
    assert_eq!(rgb, [6, 7, 8]);
    let dimmer = cell.attribute("Dimmer").unwrap();
    assert_eq!((dimmer.channel, dimmer.kind), (9, AttributeKind::Htp));
    assert_eq!(i.profile.validate(), Ok(()));
}

#[test]
fn lib_16_unknown_ranges_are_not_stored() {
    let xml = qxf(
        &preset("D", "IntensityDimmer"),
        &format!(
            r#"{}<Physical><Lens Name="Other" DegreesMin="0" DegreesMax="0"/><Focus Type="Fixed" PanMax="0" TiltMax="0"/></Physical>"#,
            mode(&["D"])
        ),
    );
    let i = ok(&xml);
    let m = only_mode(&i);
    assert_eq!(
        (m.pan_degrees, m.tilt_degrees, m.zoom_degrees),
        (None, None, None)
    );
    assert_eq!(i.profile.manufacturer, "Test");
    assert_eq!(i.profile.model, "Unit");
    assert_eq!(i.profile.source, "test.qxf");
}

// ---- LIB-13 / LIB-18: header and tolerance (VS2-11) -----------------------------------------

#[test]
fn lib_13_header_reads_names_without_converting() {
    let h = cmdmx_core::library::qlcplus::header(&read("Pro-Lights-LumiPar-7QPro.qxf")).unwrap();
    assert_eq!(h.manufacturer, "Pro-Lights");
    assert_eq!(h.model, "LumiPar 7QPro");
    assert_eq!(h.modes, ["10 Ch Mode", "6 Ch Mode", "4 Ch Mode"]);
    assert!(cmdmx_core::library::qlcplus::header("<nope").is_err());
}

#[test]
fn lib_18_channel_names_ignore_surrounding_spaces() {
    let xml = qxf(
        &[
            preset("Red ", "IntensityRed"),
            preset("Green", "IntensityGreen"),
            preset(" Blue", "IntensityBlue"),
        ]
        .concat(),
        &mode(&["Red", "Green ", "Blue"]),
    );
    let i = ok(&xml);
    assert_eq!(
        only_mode(&i).colour.as_ref().unwrap().system,
        ColourSystem::Rgb
    );
    assert!(i
        .report
        .channels
        .iter()
        .all(|l| l.source_name.trim() == l.source_name));
}

#[test]
fn lib_18_a_channel_in_several_heads_belongs_to_the_mode() {
    let xml = qxf(
        &[
            preset("Dimmer", "IntensityMasterDimmer"),
            preset("W1", "IntensityWhite"),
            preset("W2", "IntensityWhite"),
        ]
        .concat(),
        r#"<Mode Name="M">
             <Channel Number="0">Dimmer</Channel><Channel Number="1">W1</Channel><Channel Number="2">W2</Channel>
             <Head><Channel>0</Channel><Channel>1</Channel></Head>
             <Head><Channel>0</Channel><Channel>2</Channel></Head>
           </Mode>"#,
    );
    let i = ok(&xml);
    let m = only_mode(&i);
    assert_eq!(attr(m, "Dimmer").channel, 1);
    assert_eq!(m.cells.len(), 2);
    assert!(m.cells.iter().all(|c| c.attribute("Dimmer").is_none()));
    assert_eq!(m.cells[1].attribute("White").unwrap().channel, 3);
    assert_eq!(i.profile.validate(), Ok(()));
}
