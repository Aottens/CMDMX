//! cmdmx-daemon — the ColourmotionDMX engine process.
//!
//! Runs the deterministic core at 40 Hz with a demo patch and sends line 1 as
//! Art-Net, sACN and ENTTEC Pro serial, each node on its own thread (OUT-11).
//! MIDI arrives with VS1-07 and must also stay off the tick thread.
//!
//! Usage: `cmdmx-daemon [--ticks N] [--artnet-target IPV4 | --no-artnet]
//!        [--sacn-unicast IPV4] [--sacn-priority 0..200] [--no-sacn]
//!        [--serial PATH | --no-serial]`.
//! `--ticks N` exits after N ticks (CI smoke test), otherwise runs until killed.

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
use cmdmx_core::UNIVERSE_SIZE;
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

    eprintln!("cmdmx-daemon 0.1.0 — 40 Hz tick loop running.");
    let started = Instant::now();
    let mut next = started;
    let mut tick: u64 = 0;
    let mut last_report = started;

    loop {
        let now = started.elapsed().as_secs_f64();
        // Demo motion: fader 1 breathes so the output visibly changes.
        let level = ((now * 0.5).sin() * 0.5 + 0.5) as f32;
        if let Err(e) = engine.set_fader(1, level, now) {
            eprintln!("warning: {e}");
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
    engine.patch(Fixture {
        id: 1,
        profile: "generic-dimmer".into(),
        universe: 1,
        address: 1,
    })?;
    engine.add_playback(Playback {
        id: 1,
        priority: Priority::Normal,
        fader_mode: FaderMode::Mode0,
        values: BTreeMap::from([(1, BTreeMap::from([("Dimmer".to_string(), 1.0)]))]),
    });
    Ok(engine)
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
    fn ipc_09_demo_show_has_one_dimmer_per_fader() {
        let engine = demo_engine().unwrap();
        let ids: Vec<_> = engine.playback_states().map(|(id, _)| *id).collect();
        let n = u32::from(engine.settings().faders_per_page);
        assert_eq!(ids, (1..=n).collect::<Vec<_>>());
    }
}
