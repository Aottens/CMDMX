//! cmdmx-daemon — the ColourmotionDMX engine process.
//!
//! Runs the deterministic core at 40 Hz with a demo patch and sends line 1 as
//! Art-Net from its own thread (OUT-11). sACN, serial and MIDI arrive with
//! VS1-03..VS1-07 and must also stay off the tick thread.
//!
//! Usage: `cmdmx-daemon [--ticks N] [--artnet-target IPV4 | --no-artnet]`.
//! `--ticks N` exits after N ticks (CI smoke test), otherwise runs until killed.

mod artnet_out;

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::{Duration, Instant};

use cmdmx_core::attribute::{AttributeDef, AttributeGroup, AttributeKind};
use cmdmx_core::engine::{Engine, Settings};
use cmdmx_core::patch::Fixture;
use cmdmx_core::playback::{FaderMode, Playback, Priority};
use cmdmx_core::profile::Profile;
use cmdmx_core::UNIVERSE_SIZE;
use cmdmx_protocols::{artnet, enttec, sacn};

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
    // OUT-15: slice-only output config until node config lives in the show file.
    let artnet_tx = cli.artnet.map(|target| {
        eprintln!("art-net: line 1 → port-address 0 at {target}");
        artnet_out::spawn(artnet_out::ArtNetNode::new(target, 0))
    });
    let mut artnet_alive = true;

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
        if let Some(tx) = &artnet_tx {
            let frame = engine.frame(1).map_or([0; UNIVERSE_SIZE], |f| *f.slots());
            if !artnet_out::publish(tx, &frame) && artnet_alive {
                eprintln!("art-net output thread stopped; tick continues");
                artnet_alive = false;
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
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Cli, String> {
    let mut cli = Cli {
        ticks: None,
        artnet: Some(artnet_out::default_target()),
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--ticks" => cli.ticks = args.next().and_then(|v| v.parse().ok()),
            "--no-artnet" => cli.artnet = None,
            "--artnet-target" => {
                let v = args.next().unwrap_or_default();
                let ip: Ipv4Addr = v
                    .parse()
                    .map_err(|_| format!("--artnet-target: '{v}' is not an IPv4 address"))?;
                cli.artnet = Some(SocketAddrV4::new(ip, cmdmx_protocols::artnet::PORT));
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

/// Show that all three transports can frame the current universe. Nothing is
/// sent yet; this is the seam where the output threads plug in.
fn report(engine: &Engine) {
    for (universe, frame) in engine.universes() {
        let slots = frame.slots();
        let art = artnet::art_dmx(universe.saturating_sub(1), 1, 0, slots).map(|p| p.len());
        let acn = sacn::data_packet(&[0u8; 16], "ColourmotionDMX", 100, 1, *universe, slots)
            .map(|p| p.len());
        let pro = enttec::send_dmx_frame(slots).map(|p| p.len());
        eprintln!(
            "universe {universe}: ch1={} | framed bytes art-net={:?} sacn={:?} enttec={:?}",
            frame.channel(1).unwrap_or(0),
            art,
            acn,
            pro
        );
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
}
