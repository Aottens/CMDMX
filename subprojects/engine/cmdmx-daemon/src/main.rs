//! cmdmx-daemon — the ColourmotionDMX engine process.
//!
//! Runs the deterministic core at 40 Hz with a demo patch and sends line 1 as
//! Art-Net, sACN and ENTTEC Pro serial, each node on its own thread (OUT-11).
//! MIDI arrives with VS1-07 and must also stay off the tick thread.
//!
//! Usage: `cmdmx-daemon [--ticks N] [--artnet-target IPV4 | --no-artnet]
//!        [--sacn-unicast IPV4] [--sacn-priority 0..200] [--no-sacn]
//!        [--serial PATH | --no-serial] [--ipc-port N] [--demo]`.
//! `--ticks N` exits after N ticks (CI smoke test), otherwise runs until killed.
//! The app talks to the daemon over WebSocket on `--ipc-port` (default 7319);
//! `--demo` lets fader 1 breathe. The demo show (IPC-09, IPC-14) has eight dimmers on
//! faders 1..8 plus the church rig and its groups; the programmer travels over the same
//! socket (IPC-10..13).

mod artnet_out;
mod ipc;
mod output;
mod sacn_out;
mod serial_out;

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::{Duration, Instant};

use cmdmx_core::attribute::{AttributeDef, AttributeGroup, AttributeKind};
use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::patch::Fixture;
use cmdmx_core::playback::{FaderMode, Playback, Priority};
use cmdmx_core::profile::Profile;
use cmdmx_core::{CoreError, UNIVERSE_SIZE};
const TICK: Duration = Duration::from_millis(25);

fn main() {
    let cli = match parse_args(std::env::args().skip(1)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("cmdmx-daemon: {e}");
            std::process::exit(2);
        }
    };
    let ticks_limit = cli.ticks;
    // OUT-15/16/22: slice-only output config until node config lives in the show file.
    // Each entry: (name, hand-off to its thread, thread still alive).
    let mut outputs = Vec::new();
    if let Some(target) = cli.artnet {
        eprintln!("art-net: line 1 → port-address 0 at {target}");
        let mut node = artnet_out::ArtNetNode::new(target, 0);
        outputs.push((
            "art-net",
            output::spawn("art-net", move |f| node.send(f)),
            true,
        ));
    }
    if cli.sacn_enabled {
        let (target, priority) = (cli.sacn_target, cli.sacn_priority);
        eprintln!("sacn: line 1 → universe 1 at {target}, priority {priority}");
        let mut node = sacn_out::SacnNode::new(target, 1, priority);
        outputs.push(("sacn", output::spawn("sacn", move |f| node.send(f)), true));
    }
    if cli.serial_enabled {
        let mut node = serial_out::SerialNode::new(cli.serial_path);
        let send = move |f: &output::FrameData| node.send(f, Instant::now());
        outputs.push(("serial", output::spawn("serial", send), true));
    }

    let mut engine = match demo_engine() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("cmdmx-daemon: cannot build demo engine: {e}");
            std::process::exit(2);
        }
    };

    // IPC-01: a bound port that is already taken most likely means another daemon runs.
    let (events, ipc_addr) = match ipc::spawn_server(cli.ipc_port) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cmdmx-daemon: cannot open IPC port {}: {e}", cli.ipc_port);
            std::process::exit(2);
        }
    };
    eprintln!("ipc: listening on ws://{ipc_addr}");
    let mut clients = ipc::Clients::default();
    let mut published = ipc::Published::new(&engine);

    eprintln!("cmdmx-daemon 0.1.0 — 40 Hz tick loop running.");
    let started = Instant::now();
    let mut next = started;
    let mut tick: u64 = 0;
    let mut last_report = started;

    loop {
        let now = started.elapsed().as_secs_f64();
        // IPC-07: commands and new clients are drained here, never waited for.
        while let Ok(event) = events.try_recv() {
            let result = match event {
                ipc::Event::Connected(tx) => {
                    clients.add(tx, &ipc::snapshot(&engine));
                    Ok(())
                }
                ipc::Event::Command(cmd) => apply(&mut engine, cmd, now),
            };
            if let Err(e) = result {
                eprintln!("warning: {e}");
                clients.broadcast(&ipc::warning(&e.to_string()));
            }
        }
        if cli.demo {
            // Demo motion: fader 1 breathes so the output visibly changes.
            let level = ((now * 0.5).sin() * 0.5 + 0.5) as f32;
            if let Err(e) = engine.set_fader(1, level, now) {
                eprintln!("warning: {e}");
            }
        }
        engine.tick(now);
        let frame = engine.frame(1).map_or([0; UNIVERSE_SIZE], |f| *f.slots());
        for (name, tx, alive) in &mut outputs {
            if !output::publish(tx, &frame) && *alive {
                eprintln!("{name} output thread stopped; tick continues");
                *alive = false;
            }
        }
        for w in engine.drain_warnings() {
            eprintln!("warning @{:.3}: {}", w.at, w.message);
            clients.broadcast(&ipc::warning(&w.message));
        }
        for msg in published.diff(&engine) {
            clients.broadcast(&msg);
        }

        if last_report.elapsed() >= Duration::from_secs(1) {
            report(&engine);
            last_report = Instant::now();
        }

        tick += 1;
        if ticks_limit.is_some_and(|limit| tick >= limit) {
            report(&engine);
            eprintln!("cmdmx-daemon: {tick} ticks done, exiting.");
            return;
        }

        next += TICK;
        let wait = next.saturating_duration_since(Instant::now());
        std::thread::sleep(wait);
    }
}

/// IPC-04/13: every command maps 1:1 onto an engine method; refusals come back as `Err`.
fn apply(engine: &mut Engine, cmd: ipc::Command, now: f64) -> Result<(), CoreError> {
    use ipc::Command::*;
    match cmd {
        FaderSet { playback, level } => engine.set_fader(playback, level, now),
        GrandMasterSet { level } => engine.set_grand_master(level),
        Select { fixture } => engine.select_fixture(fixture),
        SelectGroup { group } => engine.select_group(group),
        Step { forward: true } => {
            engine.next_fixture();
            Ok(())
        }
        Step { forward: false } => {
            engine.prev_fixture();
            Ok(())
        }
        Pattern { stages } => engine.select_pattern(stages),
        All => {
            engine.select_all();
            Ok(())
        }
        Clear => {
            engine.clear();
            Ok(())
        }
        AttributeGroup { group } => {
            engine.set_active_group(group);
            Ok(())
        }
        Wheel {
            attribute,
            steps,
            resolution,
        } => engine.turn_wheel(&attribute, steps, resolution),
        Line { tokens } => {
            let tokens: Vec<&str> = tokens.iter().map(String::as_str).collect();
            engine.execute_command(&tokens)
        }
        RecordGroup { group } => engine.record_group(group),
    }
}

struct Cli {
    ticks: Option<u64>,
    /// `None` = Art-Net disabled (`--no-artnet`).
    artnet: Option<SocketAddrV4>,
    sacn_enabled: bool,
    sacn_target: SocketAddrV4,
    sacn_priority: u8,
    serial_enabled: bool,
    /// `--serial <path>`; `None` = detect the interface (OUT-09).
    serial_path: Option<String>,
    /// `--ipc-port <n>`: WebSocket port on 127.0.0.1 (IPC-01).
    ipc_port: u16,
    /// `--demo`: fader 1 breathes (IPC-09).
    demo: bool,
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Cli, String> {
    let mut cli = Cli {
        ticks: None,
        artnet: Some(artnet_out::default_target()),
        sacn_enabled: true,
        sacn_target: sacn_out::multicast_target(1),
        sacn_priority: cmdmx_protocols::sacn::DEFAULT_PRIORITY,
        serial_enabled: true,
        serial_path: None,
        ipc_port: 7319,
        demo: false,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--ticks" => cli.ticks = args.next().and_then(|v| v.parse().ok()),
            "--no-artnet" => cli.artnet = None,
            "--artnet-target" => {
                let ip = parse_ipv4("--artnet-target", args.next())?;
                cli.artnet = Some(SocketAddrV4::new(ip, cmdmx_protocols::artnet::PORT));
            }
            "--no-sacn" => cli.sacn_enabled = false,
            "--sacn-unicast" => {
                let ip = parse_ipv4("--sacn-unicast", args.next())?;
                cli.sacn_target = SocketAddrV4::new(ip, cmdmx_protocols::sacn::PORT);
            }
            "--sacn-priority" => {
                let v = args.next().unwrap_or_default();
                cli.sacn_priority = v
                    .parse()
                    .ok()
                    .filter(|p| *p <= 200)
                    .ok_or(format!("--sacn-priority: '{v}' is not 0..200"))?;
            }
            "--no-serial" => cli.serial_enabled = false,
            "--serial" => {
                cli.serial_path = Some(args.next().ok_or("--serial: missing device path")?);
            }
            "--ipc-port" => {
                let v = args.next().unwrap_or_default();
                cli.ipc_port = v
                    .parse()
                    .map_err(|_| format!("--ipc-port: '{v}' is not a port number"))?;
            }
            "--demo" => cli.demo = true,
            _ => {}
        }
    }
    Ok(cli)
}

fn demo_engine() -> Result<Engine, cmdmx_core::CoreError> {
    let mut engine = Engine::new(Settings::default());
    engine.add_profile(
        "generic-dimmer",
        Profile {
            attributes: vec![AttributeDef {
                name: "Dimmer".into(),
                group: AttributeGroup::I,
                kind: AttributeKind::Htp,
                channel: 1,
                fine_channel: None,
            }],
        },
    );
    // IPC-09: one dimmer per fader, playback n -> fixture n at address n.
    for n in 1..=u32::from(Settings::default().faders_per_page) {
        engine.patch(Fixture {
            id: n,
            profile: "generic-dimmer".into(),
            universe: 1,
            address: n as u16,
        })?;
        engine.add_playback(Playback {
            id: n,
            priority: Priority::Normal,
            fader_mode: FaderMode::Mode0,
            values: BTreeMap::from([(n, BTreeMap::from([("Dimmer".to_string(), 1.0)]))]),
        });
    }
    // IPC-14: the church rig on universe 1, after the demo dimmers.
    engine.add_profile("wash", wash_profile());
    let mut address = 9;
    for id in 101..=104 {
        engine.patch(Fixture {
            id,
            profile: "wash".into(),
            universe: 1,
            address,
        })?;
        address += 5;
    }
    address = 29;
    for id in (201..=208).chain(221..=226).chain(301..=306) {
        engine.patch(Fixture {
            id,
            profile: "generic-dimmer".into(),
            universe: 1,
            address,
        })?;
        address += 1;
    }
    // Groups 1 All, 2 Washes, 3 Pars, 4 Front spots; the demo starts with nothing selected.
    let all = || {
        (101..=104)
            .chain(201..=208)
            .chain(221..=226)
            .chain(301..=306)
    };
    let groups: [Vec<u32>; 4] = [
        all().collect(),
        (101..=104).collect(),
        (201..=208).chain(221..=226).collect(),
        (301..=306).collect(),
    ];
    for (group, members) in (1..).zip(groups) {
        for id in members {
            engine.select_fixture(id)?;
        }
        engine.record_group(group)?;
        engine.clear_selection();
    }
    Ok(engine)
}

/// IPC-14: Dimmer, 16-bit Pan, Tilt and Zoom on five channels.
fn wash_profile() -> Profile {
    let attr = |name: &str, group, kind, channel, fine_channel| AttributeDef {
        name: name.into(),
        group,
        kind,
        channel,
        fine_channel,
    };
    Profile {
        attributes: vec![
            attr("Dimmer", AttributeGroup::I, AttributeKind::Htp, 1, None),
            attr("Pan", AttributeGroup::P, AttributeKind::Ltp, 2, Some(3)),
            attr("Tilt", AttributeGroup::P, AttributeKind::Ltp, 4, None),
            attr("Zoom", AttributeGroup::B, AttributeKind::Ltp, 5, None),
        ],
    }
}

fn parse_ipv4(flag: &str, value: Option<String>) -> Result<Ipv4Addr, String> {
    let v = value.unwrap_or_default();
    v.parse()
        .map_err(|_| format!("{flag}: '{v}' is not an IPv4 address"))
}

/// Once a second: channel 1 of each universe, so a human can see the tick runs.
fn report(engine: &Engine) {
    for (universe, frame) in engine.universes() {
        eprintln!("universe {universe}: ch1={}", frame.channel(1).unwrap_or(0));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn args(s: &str) -> impl Iterator<Item = String> + '_ {
        s.split_whitespace().map(String::from)
    }

    #[test]
    fn out_15_artnet_is_on_by_default_to_limited_broadcast() {
        let cli = parse_args(args("")).unwrap();
        assert_eq!(cli.artnet, Some(artnet_out::default_target()));
        assert_eq!(cli.ticks, None);
    }

    #[test]
    fn out_15_artnet_target_and_no_artnet_flags() {
        let cli = parse_args(args("--artnet-target 192.168.1.20 --ticks 5")).unwrap();
        assert_eq!(cli.artnet, Some("192.168.1.20:6454".parse().unwrap()));
        assert_eq!(cli.ticks, Some(5));
        assert_eq!(parse_args(args("--no-artnet")).unwrap().artnet, None);
    }

    #[test]
    fn out_15_bad_target_is_an_error_not_a_fallback() {
        assert!(parse_args(args("--artnet-target 300.1.1.1")).is_err());
        assert!(parse_args(args("--artnet-target")).is_err());
    }

    #[test]
    fn out_16_sacn_is_on_by_default_multicast_priority_100() {
        let cli = parse_args(args("")).unwrap();
        assert!(cli.sacn_enabled);
        assert_eq!(cli.sacn_target, sacn_out::multicast_target(1));
        assert_eq!(cli.sacn_priority, 100);
    }

    #[test]
    fn out_16_sacn_flags() {
        let cli = parse_args(args("--sacn-unicast 10.0.0.5 --sacn-priority 200")).unwrap();
        assert_eq!(cli.sacn_target, "10.0.0.5:5568".parse().unwrap());
        assert_eq!(cli.sacn_priority, 200);
        let cli = parse_args(args("--no-sacn")).unwrap();
        assert!(!cli.sacn_enabled);
        assert!(cli.artnet.is_some(), "Art-Net unaffected");
    }

    #[test]
    fn out_07_out_16_bad_sacn_values_are_errors() {
        assert!(parse_args(args("--sacn-priority 201")).is_err());
        assert!(parse_args(args("--sacn-priority -1")).is_err());
        assert!(parse_args(args("--sacn-unicast nope")).is_err());
    }

    #[test]
    fn out_22_serial_is_on_by_default_with_detection() {
        let cli = parse_args(args("")).unwrap();
        assert!(cli.serial_enabled);
        assert_eq!(cli.serial_path, None);
    }

    #[test]
    fn out_22_serial_flags() {
        let cli = parse_args(args("--serial /dev/cu.usbserial-X")).unwrap();
        assert_eq!(cli.serial_path.as_deref(), Some("/dev/cu.usbserial-X"));
        assert!(!parse_args(args("--no-serial")).unwrap().serial_enabled);
        assert!(parse_args(args("--serial")).is_err());
    }

    #[test]
    fn ipc_01_ipc_port_flag() {
        assert_eq!(parse_args(args("")).unwrap().ipc_port, 7319);
        assert_eq!(parse_args(args("--ipc-port 9001")).unwrap().ipc_port, 9001);
        assert!(parse_args(args("--ipc-port nope")).is_err());
        assert!(parse_args(args("--ipc-port")).is_err());
    }

    #[test]
    fn ipc_09_breathing_demo_only_with_demo_flag() {
        assert!(!parse_args(args("")).unwrap().demo);
        assert!(parse_args(args("--demo")).unwrap().demo);
    }

    #[test]
    fn ipc_14_demo_show_patches_the_church_rig_and_groups() {
        let engine = demo_engine().unwrap();
        let ids: Vec<u32> = engine.fixtures().map(|f| f.id).collect();
        let church: Vec<u32> = (101..=104)
            .chain(201..=208)
            .chain(221..=226)
            .chain(301..=306)
            .collect();
        let expected: Vec<u32> = (1..=8).chain(church.iter().copied()).collect();
        assert_eq!(ids, expected);
        let address = |id: u32| engine.fixtures().find(|f| f.id == id).unwrap().address;
        assert_eq!(
            (
                address(101),
                address(104),
                address(201),
                address(221),
                address(306)
            ),
            (9, 24, 29, 37, 48)
        );
        let wash = engine.fixtures().find(|f| f.id == 101).unwrap();
        assert_eq!(wash.profile, "wash");
        // VS2-07 run, finding 1: a short profile key that fits a fixture button.
        assert!(engine
            .fixtures()
            .filter(|f| f.profile != "wash")
            .all(|f| f.profile == "dimmer"));
        assert_eq!(engine.group(1), Some(church.as_slice()));
        assert_eq!(engine.group(2), Some(&[101, 102, 103, 104][..]));
        let pars: Vec<u32> = (201..=208).chain(221..=226).collect();
        assert_eq!(engine.group(3), Some(pars.as_slice()));
        assert_eq!(engine.group(4), Some(&[301, 302, 303, 304, 305, 306][..]));
        assert!(engine.programmer().selection().is_empty());
        assert!(engine.programmer().is_empty());
    }

    #[test]
    fn ipc_14_demo_wash_has_dimmer_16_bit_pan_tilt_zoom() {
        let mut engine = demo_engine().unwrap();
        engine.execute_command(&["101", "Enter"]).unwrap();
        engine.set_active_group(cmdmx_core::attribute::AttributeGroup::P);
        assert_eq!(
            engine.wheel_layout(),
            vec!["Pan".to_string(), "Tilt".to_string()]
        );
        engine.set_programmer_value(101, "Pan", 1.0).unwrap();
        engine.tick(0.0);
        let frame = engine.frame(1).unwrap();
        // Pan coarse + fine at offsets 2 and 3 of the wash at address 9.
        assert_eq!(
            (frame.channel(10), frame.channel(11)),
            (Some(255), Some(255))
        );
    }

    #[test]
    fn ipc_09_demo_show_has_one_dimmer_per_fader() {
        let engine = demo_engine().unwrap();
        let ids: Vec<_> = engine.playback_states().map(|(id, _)| *id).collect();
        let n = u32::from(engine.settings().faders_per_page);
        assert_eq!(ids, (1..=n).collect::<Vec<_>>());
    }
}
