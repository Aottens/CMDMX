//! cmdmx-daemon — the ColourmotionDMX engine process.
//!
//! Runs the deterministic core at 40 Hz with a demo patch and sends line 1 as
//! Art-Net, sACN and ENTTEC Pro serial, each node on its own thread (OUT-11).
//! Node states reach the app in the snapshot and the `outputs` delta (IPC-20).
//! MIDI arrives with VS1-07 and must also stay off the tick thread.
//!
//! Usage: `cmdmx-daemon [--ticks N] [--artnet-target IPV4 | --no-artnet]
//!        [--sacn-unicast IPV4] [--sacn-priority 0..200] [--no-sacn]
//!        [--serial PATH | --no-serial] [--ipc-port N] [--demo]
//!        [--qlcplus-dir PATH] [--profiles-dir PATH] [--show-dir PATH]`.
//! `--ticks N` exits after N ticks (CI smoke test), otherwise runs until killed.
//! The app talks to the daemon over WebSocket on `--ipc-port` (default 7319);
//! `--demo` lets fader 1 breathe. The demo show (IPC-09, IPC-14) has eight dimmers on
//! faders 1..8 plus the church rig (RGBW washes, RGBW, RGB+Lime and RGB pars, front spots)
//! and its groups; the programmer travels over the same socket (IPC-10..13, IPC-18), and so do the
//! patch commands (PAT-09).
//! The fixture library (LIB-12..14) is indexed on its own thread at start-up from `--qlcplus-dir`
//! (default: the bundled `vendor/qlcplus/fixtures`) and `--profiles-dir` (default
//! `~/Library/Application Support/ColourmotionDMX/profiles`); output never waits for it.
//! The daemon always runs a show (SHW-09..15): it opens the last one from `--show-dir` (default
//! `~/Documents/ColourmotionDMX/Shows`, `state.json` beside the app support files, or inside
//! `--show-dir` when that is given), autosaves it on a saver thread and switches on `show.*`.

mod artnet_out;
mod ipc;
mod library;
mod output;
mod sacn_out;
mod serial_out;
mod shows;

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

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
    // Each entry: (hand-off to its thread, its shared state). `statuses` is the IPC-20 view
    // of the same nodes, built once and updated in place each tick.
    let mut outputs = Vec::new();
    let mut statuses: Vec<ipc::OutputStatus> = Vec::new();
    if let Some(target) = cli.artnet {
        eprintln!("art-net: line 1 → port-address 0 at {target}");
        let mut node = artnet_out::ArtNetNode::new(target, 0);
        let send = move |f: &output::FrameData| (node.send(f), node.state());
        outputs.push(output::spawn("art-net", send));
        statuses.push(ipc::OutputStatus {
            name: "art-net",
            state: output::OutputState::Ok,
        });
    }
    if cli.sacn_enabled {
        let (target, priority) = (cli.sacn_target, cli.sacn_priority);
        eprintln!("sacn: line 1 → universe 1 at {target}, priority {priority}");
        let mut node = sacn_out::SacnNode::new(target, 1, priority);
        let send = move |f: &output::FrameData| (node.send(f), node.state());
        outputs.push(output::spawn("sacn", send));
        statuses.push(ipc::OutputStatus {
            name: "sacn",
            state: output::OutputState::Ok,
        });
    }
    if cli.serial_enabled {
        let mut node = serial_out::SerialNode::new(cli.serial_path);
        let send = move |f: &output::FrameData| (node.send(f, Instant::now()), node.state());
        outputs.push(output::spawn("serial", send));
        // Not scanned yet, so no interface until the thread says otherwise (IPC-20).
        statuses.push(ipc::OutputStatus {
            name: "serial",
            state: output::OutputState::NoInterface,
        });
    }

    // SHW-10: the last show, else Demo; a refused show is reported, never written.
    let shows = Arc::new(shows::Shows::new(
        cli.show_dir.clone(),
        cli.state_file.clone(),
    ));
    let opened = shows.start(
        || {
            demo_engine().unwrap_or_else(|e| {
                eprintln!("cmdmx-daemon: cannot build demo engine: {e}");
                std::process::exit(2);
            })
        },
        &shows::iso_utc(SystemTime::now()),
    );
    // The first client to connect gets these right after its snapshot.
    let mut pending = opened.warnings;
    for w in &pending {
        eprintln!("warning: {w}");
    }
    eprintln!("show: {}", opened.name);
    if let Err(e) = shows.remember(&opened.name) {
        eprintln!("warning: cannot remember the open show: {e}");
    }
    let mut engine = opened.engine;
    let mut autosave =
        shows::Autosave::new(shows.path(&opened.name), opened.meta, opened.show, 0.0);
    let (jobs, saved) = match shows::spawn_saver() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cmdmx-daemon: cannot start the saver thread: {e}");
            std::process::exit(2);
        }
    };

    // IPC-01: a bound port that is already taken most likely means another daemon runs.
    let library: ipc::SharedLibrary = Default::default();
    let (events, ipc_addr) =
        match ipc::spawn_server_with(cli.ipc_port, library.clone(), Arc::clone(&shows)) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("cmdmx-daemon: cannot open IPC port {}: {e}", cli.ipc_port);
                std::process::exit(2);
            }
        };
    eprintln!("ipc: listening on ws://{ipc_addr}");
    // LIB-13: index the library in the background; until it is stored, requests get "loading".
    let (qlcplus_dir, profiles_dir) = (cli.qlcplus_dir.clone(), cli.profiles_dir.clone());
    let loader = std::thread::Builder::new()
        .name("library-load".into())
        .spawn(move || {
            // A panic while reading the bundled set must not leave the library "loading" forever.
            let loaded = std::panic::catch_unwind(|| {
                library::Library::load(Some(&qlcplus_dir), &profiles_dir)
            })
            .unwrap_or_else(|_| {
                eprintln!("library: loading the QLC+ set panicked; user profiles only");
                library::Library::load(None, &profiles_dir)
            });
            let (lib, warnings) = loaded;
            for w in warnings {
                eprintln!("library: warning: {w}");
            }
            eprintln!("library: {} profiles", lib.len());
            if let Ok(mut slot) = library.write() {
                *slot = Some(lib);
            }
        });
    if let Err(e) = loader {
        eprintln!("library: cannot start the loader thread: {e}");
    }
    let mut clients = ipc::Clients::default();
    let mut published = ipc::Published::new(&engine, &statuses, &autosave.state());

    eprintln!("cmdmx-daemon 0.1.0 — 40 Hz tick loop running.");
    let started = Instant::now();
    let mut next = started;
    let mut tick: u64 = 0;
    let mut last_report = started;
    let mut last_printed: Vec<(u16, u8)> = Vec::new();

    loop {
        let now = started.elapsed().as_secs_f64();
        let wall = shows::iso_utc(SystemTime::now());
        // IPC-07: commands and new clients are drained here, never waited for.
        while let Ok(event) = events.try_recv() {
            let result = match event {
                ipc::Event::Connected(tx) => {
                    clients.add(tx, &ipc::snapshot(&engine, &statuses, &autosave.state()));
                    for w in std::mem::take(&mut pending) {
                        clients.broadcast(&ipc::warning(&w));
                    }
                    Ok(())
                }
                ipc::Event::Warning(e) => {
                    warn(&mut clients, &e);
                    continue;
                }
                // SHW-11/14/15: write the current show at once, then continue in the next one.
                event
                @ (ipc::Event::ShowOpened(_) | ipc::Event::Command(ipc::Command::Show(_))) => {
                    use shows::ShowRequest::{New, SaveAs};
                    let target = match &event {
                        ipc::Event::ShowOpened(opened) => Some(&opened.name),
                        ipc::Event::Command(ipc::Command::Show(New { name } | SaveAs { name })) => {
                            Some(name)
                        }
                        _ => None,
                    };
                    // Also keeps a New or Save As off the open show's own file.
                    if let Some(n) =
                        target.filter(|n| n.to_lowercase() == autosave.state().name.to_lowercase())
                    {
                        warn(&mut clients, &format!("{n} is already open"));
                        continue;
                    }
                    hand_off(
                        autosave.flush(&engine, &wall),
                        &jobs,
                        &mut autosave,
                        &mut clients,
                    );
                    let fresh = |name: String, engine: Engine| shows::Opened {
                        meta: shows::new_meta(&name, &wall),
                        name,
                        engine,
                        show: None,
                        warnings: Vec::new(),
                    };
                    let next = match event {
                        ipc::Event::ShowOpened(opened) => Some(*opened),
                        ipc::Event::Command(ipc::Command::Show(New { name })) => {
                            Some(fresh(name, Engine::new(Settings::default())))
                        }
                        // Same engine, a new file; the placeholder never runs.
                        ipc::Event::Command(ipc::Command::Show(SaveAs { name })) => Some(fresh(
                            name,
                            std::mem::replace(&mut engine, Engine::new(Settings::default())),
                        )),
                        _ => None,
                    };
                    if let Some(next) = next {
                        engine = next.engine;
                        autosave =
                            shows::Autosave::new(shows.path(&next.name), next.meta, next.show, now);
                        // SHW-15: Save As and New write under the new name at once, so the file
                        // exists for `show.list`, `show.open` and the name check.
                        hand_off(
                            autosave.flush(&engine, &wall),
                            &jobs,
                            &mut autosave,
                            &mut clients,
                        );
                        // ponytail: a tiny write on the tick, once per switch.
                        if let Err(e) = shows.remember(&next.name) {
                            warn(&mut clients, &format!("cannot remember the open show: {e}"));
                        }
                        let state = autosave.state();
                        published = ipc::Published::new(&engine, &statuses, &state);
                        clients.broadcast(&ipc::snapshot(&engine, &statuses, &state));
                        for w in &next.warnings {
                            warn(&mut clients, w);
                        }
                    }
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
        // SHW-11/13: the saver does the disk work; the tick only hands over a copy.
        while let Ok(done) = saved.try_recv() {
            if let Some(w) = autosave.finished(done) {
                warn(&mut clients, &w);
            }
        }
        hand_off(
            autosave.poll(&engine, now, &wall),
            &jobs,
            &mut autosave,
            &mut clients,
        );
        let frame = engine.frame(1).map_or([0; UNIVERSE_SIZE], |f| *f.slots());
        for ((tx, status), out) in outputs.iter().zip(&mut statuses) {
            if !output::publish(tx, &frame) && status.get() != output::OutputState::Stopped {
                eprintln!("{} output thread stopped; tick continues", out.name);
                status.set(output::OutputState::Stopped);
            }
            out.state = status.get();
        }
        for w in engine.drain_warnings() {
            eprintln!("warning @{:.3}: {}", w.at, w.message);
            clients.broadcast(&ipc::warning(&w.message));
        }
        for msg in published.diff(&engine, &statuses, &autosave.state()) {
            clients.broadcast(&msg);
        }

        if last_report.elapsed() >= Duration::from_secs(1) {
            // Finding 3: print only when channel 1 changed since the last print.
            let now_ch1: Vec<_> = engine
                .universes()
                .map(|(u, f)| (*u, f.channel(1).unwrap_or(0)))
                .collect();
            if now_ch1 != last_printed {
                report(&engine);
                last_printed = now_ch1;
            }
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

fn warn(clients: &mut ipc::Clients, msg: &str) {
    eprintln!("warning: {msg}");
    clients.broadcast(&ipc::warning(msg));
}

/// SHW-11/13: give a job to the saver. A saver that has stopped counts as a failed save.
/// Never waits.
fn hand_off(
    job: Option<shows::Job>,
    jobs: &Sender<shows::Job>,
    autosave: &mut shows::Autosave,
    clients: &mut ipc::Clients,
) {
    let Some(job) = job else { return };
    if let Err(e) = jobs.send(job) {
        let done = shows::Done {
            show: e.0.show,
            result: Err("the saver thread has stopped".into()),
            at: String::new(),
        };
        if let Some(w) = autosave.finished(done) {
            warn(clients, &w);
        }
    }
}

/// IPC-04/13: every command maps 1:1 onto an engine method; refusals come back as `Err`.
fn apply(engine: &mut Engine, cmd: ipc::Command, now: f64) -> Result<(), CoreError> {
    use ipc::Command::*;
    match cmd {
        FaderSet { playback, level } => engine.set_fader(playback, level, now),
        GrandMasterSet { level } => engine.set_grand_master(level),
        PageSet { page } => engine.set_page(page),
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
        Colour { hue, saturation } => engine.set_colour(hue, saturation),
        ColourModel { model } => {
            engine.set_colour_model(model);
            Ok(())
        }
        Line { tokens } => {
            let tokens: Vec<&str> = tokens.iter().map(String::as_str).collect();
            engine.execute_command(&tokens)
        }
        RecordGroup { group } => engine.record_group(group),
        // Answered in the client thread; the tick never sees it (LIB-14).
        Library(_) => Ok(()),
        // Handled before `apply` (SHW-15).
        Show(_) => Ok(()),
        PatchAdd { add, profile } => {
            if let Some(p) = profile.filter(|_| !engine.has_profile(&add.profile)) {
                // ponytail: a refused add leaves the installed profile unused in the show.
                engine.add_profile(add.profile.clone(), p)?;
            }
            engine.patch_add(&add).map(drop)
        }
        PatchMove {
            id,
            universe,
            address,
        } => engine.patch_move(id, universe, address),
        PatchRemove { ids } => engine.patch_remove(&ids),
        PatchRenumber { id, number } => engine.patch_renumber(id, number),
        PatchName { id, name } => engine.patch_name(id, &name),
        PatchChangeMode { id, mode } => engine.patch_change_mode(id, &mode),
        PatchHandle { id, handle } => engine.patch_handle(id, handle),
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
    /// `--qlcplus-dir <path>`: the bundled QLC+ fixture files (LIB-13).
    qlcplus_dir: PathBuf,
    /// `--profiles-dir <path>`: the user's saved profiles (LIB-12).
    profiles_dir: PathBuf,
    /// `--show-dir <path>`: the shows folder (SHW-09).
    show_dir: PathBuf,
    /// Where the last open show is remembered; `<show-dir>/state.json` with `--show-dir`.
    state_file: PathBuf,
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Cli, String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
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
        qlcplus_dir: PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../vendor/qlcplus/fixtures"
        )),
        profiles_dir: home.join("Library/Application Support/ColourmotionDMX/profiles"),
        show_dir: home.join("Documents/ColourmotionDMX/Shows"),
        state_file: home.join("Library/Application Support/ColourmotionDMX/state.json"),
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
            "--qlcplus-dir" => {
                cli.qlcplus_dir = args.next().ok_or("--qlcplus-dir: missing path")?.into();
            }
            "--profiles-dir" => {
                cli.profiles_dir = args.next().ok_or("--profiles-dir: missing path")?.into();
            }
            "--show-dir" => {
                cli.show_dir = args.next().ok_or("--show-dir: missing path")?.into();
                cli.state_file = cli.show_dir.join("state.json");
            }
            _ => {}
        }
    }
    Ok(cli)
}

fn demo_engine() -> Result<Engine, cmdmx_core::CoreError> {
    let mut engine = Engine::new(Settings::default());
    engine.add_profile(
        "dimmer",
        Profile::from_attributes(vec![AttributeDef::new(
            "Dimmer",
            AttributeGroup::I,
            AttributeKind::Htp,
            1,
        )]),
    )?;
    // IPC-09: one dimmer per fader, playback n -> fixture n at address n.
    for n in 1..=u32::from(Settings::default().faders_per_page) {
        engine.patch(Fixture {
            id: n,
            number: 0,
            name: String::new(),
            handle: None,
            profile: "dimmer".into(),
            mode: None,
            universe: 1,
            address: n as u16,
        })?;
        engine.add_playback(Playback {
            id: n,
            priority: Priority::Normal,
            fader_mode: FaderMode::Mode0,
            values: BTreeMap::from([(n, BTreeMap::from([("Dimmer".to_string(), 1.0.into())]))]),
        });
    }
    // IPC-14: the church rig on universe 1, each fixture right after the previous one.
    let mut address = 9;
    for (key, ids, json) in [
        ("wash", 101..=104, WASH),
        ("rgbw", 201..=208, RGBW_PAR),
        ("rgbl", 211..=216, RGBL_PAR),
        ("rgb", 221..=226, RGB_PAR),
        ("dimmer", 301..=306, ""),
    ] {
        let footprint = if json.is_empty() {
            1
        } else {
            let profile = Profile::from_json(json)?;
            let footprint = profile.modes.first().map_or(1, |m| m.footprint);
            engine.add_profile(key, profile)?;
            footprint
        };
        for id in ids {
            engine.patch(Fixture {
                id,
                number: 0,
                name: String::new(),
                handle: None,
                profile: key.into(),
                mode: None,
                universe: 1,
                address,
            })?;
            address += footprint;
        }
    }
    // Groups 1 All, 2 Washes, 3 Pars, 4 Front spots; the demo starts with nothing selected.
    let all = || {
        (101..=104)
            .chain(201..=208)
            .chain(211..=216)
            .chain(221..=226)
            .chain(301..=306)
    };
    let groups: [Vec<u32>; 4] = [
        all().collect(),
        (101..=104).collect(),
        (201..=208).chain(211..=216).chain(221..=226).collect(),
        (301..=306).collect(),
    ];
    // PBK-04: page 2 holds the washes (playback 9) and the front spots (playback 10) at full.
    let per_page = u32::from(Settings::default().faders_per_page);
    for (offset, ids) in [(1, 101..=104), (2, 301..=306)] {
        engine.add_playback(Playback {
            id: per_page + offset,
            priority: Priority::Normal,
            fader_mode: FaderMode::Mode0,
            values: ids
                .map(|f| (f, BTreeMap::from([("Dimmer".to_string(), 1.0.into())])))
                .collect(),
        });
    }
    for (group, members) in (1..).zip(groups) {
        for id in members {
            engine.select_fixture(id)?;
        }
        engine.record_group(group)?;
        engine.clear_selection();
    }
    Ok(engine)
}

/// IPC-14: Dimmer, 16-bit Pan, Tilt, Zoom, RGB mixing and a White emitter on nine channels.
const WASH: &str = r#"{"manufacturer":"Demo","model":"Wash","source":"demo","modes":[{"name":"default","footprint":9,"colour":{"system":"RGB","components":[{"channel":6},{"channel":7},{"channel":8}]},"attributes":[{"name":"Dimmer","group":"I","kind":"Htp","channel":1},{"name":"Pan","group":"P","kind":"Ltp","channel":2,"fine_channel":3},{"name":"Tilt","group":"P","kind":"Ltp","channel":4},{"name":"Zoom","group":"B","kind":"Ltp","channel":5},{"name":"White","group":"C","kind":"Ltp","channel":9,"emitter":true}]}]}"#;

/// IPC-14: Dimmer, RGB mixing and a White emitter on five channels.
const RGBW_PAR: &str = r#"{"manufacturer":"Demo","model":"RGBW Par","source":"demo","modes":[{"name":"default","footprint":5,"colour":{"system":"RGB","components":[{"channel":2},{"channel":3},{"channel":4}]},"attributes":[{"name":"Dimmer","group":"I","kind":"Htp","channel":1},{"name":"White","group":"C","kind":"Ltp","channel":5,"emitter":true}]}]}"#;

/// IPC-14: RGB mixing and a Lime emitter on four channels, with a virtual dimmer.
const RGBL_PAR: &str = r#"{"manufacturer":"Demo","model":"RGB+Lime Par","source":"demo","modes":[{"name":"default","footprint":4,"virtual_dimmer":true,"colour":{"system":"RGB","components":[{"channel":1},{"channel":2},{"channel":3}]},"attributes":[{"name":"Lime","group":"C","kind":"Ltp","channel":4,"emitter":true}]}]}"#;

/// IPC-14: Showtec LED Par 56 "6 Channel": RGB, Mode, Speed and Strobe, with a virtual dimmer.
const RGB_PAR: &str = r#"{"manufacturer":"Showtec","model":"LED Par 56 6 Channel","source":"demo","modes":[{"name":"default","footprint":6,"virtual_dimmer":true,"colour":{"system":"RGB","components":[{"channel":1},{"channel":2},{"channel":3}]},"attributes":[{"name":"Mode","group":"S","kind":"Ltp","channel":4},{"name":"Speed","group":"S","kind":"Ltp","channel":5},{"name":"Strobe","group":"I","kind":"Ltp","channel":6}]}]}"#;

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
    use cmdmx_core::colour::ColourModel;
    use cmdmx_core::patch::{Handle, PatchAdd};
    use cmdmx_core::programmer::Resolution;

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

    fn church() -> Vec<u32> {
        (101..=104)
            .chain(201..=208)
            .chain(211..=216)
            .chain(221..=226)
            .chain(301..=306)
            .collect()
    }

    #[test]
    fn shw_09_shows_folder_and_state_file() {
        let cli = parse_args(args("")).unwrap();
        assert!(cli.show_dir.ends_with("Documents/ColourmotionDMX/Shows"));
        assert!(cli
            .state_file
            .ends_with("Library/Application Support/ColourmotionDMX/state.json"));
        let cli = parse_args(args("--show-dir /tmp/shows")).unwrap();
        assert_eq!(cli.show_dir, PathBuf::from("/tmp/shows"));
        assert_eq!(cli.state_file, PathBuf::from("/tmp/shows/state.json"));
        assert!(parse_args(args("--show-dir")).is_err());
    }

    #[test]
    fn shw_03_demo_show_round_trips() {
        let meta = cmdmx_core::show::Meta {
            name: "Demo".into(),
            created: "2026-10-18T09:00:00Z".into(),
            modified: "2026-10-18T09:00:00Z".into(),
            app_version: "0.1.0".into(),
        };
        let show = demo_engine().unwrap().to_show(meta.clone());
        let (engine, warnings) = Engine::from_show(&show);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(engine.to_show(meta), show);
        let (reread, migrations) = cmdmx_core::show::Show::from_json(&show.to_json()).unwrap();
        assert!(migrations.is_empty());
        assert_eq!(reread, show);
    }

    #[test]
    fn ipc_14_demo_show_patches_the_church_rig_and_groups() {
        let engine = demo_engine().unwrap();
        let ids: Vec<u32> = engine.fixtures().map(|f| f.id).collect();
        let expected: Vec<u32> = (1..=8).chain(church()).collect();
        assert_eq!(ids, expected);
        let fixture = |id: u32| engine.fixtures().find(|f| f.id == id).unwrap();
        let addresses: Vec<u16> = [101, 104, 201, 208, 211, 216, 221, 226, 301, 306]
            .iter()
            .map(|&id| fixture(id).address)
            .collect();
        assert_eq!(addresses, [9, 36, 45, 80, 85, 105, 109, 139, 145, 150]);
        // VS2-07 run, finding 1: short profile keys that fit a fixture button.
        for (id, profile) in [
            (101, "wash"),
            (201, "rgbw"),
            (211, "rgbl"),
            (221, "rgb"),
            (301, "dimmer"),
        ] {
            assert_eq!(fixture(id).profile, profile, "fixture {id}");
        }
        assert_eq!(engine.group(1), Some(church().as_slice()));
        assert_eq!(engine.group(2), Some(&[101, 102, 103, 104][..]));
        let pars: Vec<u32> = (201..=208).chain(211..=216).chain(221..=226).collect();
        assert_eq!(engine.group(3), Some(pars.as_slice()));
        assert_eq!(engine.group(4), Some(&[301, 302, 303, 304, 305, 306][..]));
        assert!(engine.programmer().selection().is_empty());
        assert!(engine.programmer().is_empty());
    }

    #[test]
    fn ipc_14_demo_pars_mix_colour_on_every_colour_system() {
        let mut engine = demo_engine().unwrap();
        engine
            .execute_command(&["201", "And", "211", "And", "221", "@", "100", "Enter"])
            .unwrap();
        engine.set_active_group(cmdmx_core::attribute::AttributeGroup::C);
        // White and Lime are emitters with their own wheels (LIB-17, PRG-34).
        assert_eq!(
            engine.wheel_layout(),
            ["Hue", "Saturation", "White", "Lime"]
        );
        // Blue: full saturation (red), then hue to 240°.
        engine
            .turn_wheel("Saturation", 255, Resolution::Normal)
            .unwrap();
        engine.turn_wheel("Hue", 170, Resolution::Normal).unwrap();
        engine.tick(0.0);
        let frame = engine.frame(1).unwrap();
        let slots = |from: u16, to: u16| -> Vec<u8> {
            (from..=to).map(|a| frame.channel(a).unwrap()).collect()
        };
        assert_eq!(
            slots(45, 49),
            [255, 0, 0, 255, 0],
            "RGBW par: Dimmer R G B W"
        );
        assert_eq!(
            slots(85, 88),
            [0, 0, 255, 0],
            "RGB+Lime par, virtual dimmer"
        );
        assert_eq!(
            slots(109, 114),
            [0, 0, 255, 0, 0, 0],
            "Showtec RGB par, virtual dimmer"
        );
    }

    #[test]
    fn ipc_14_demo_wash_is_rgbw_and_starts_white() {
        let mut engine = demo_engine().unwrap();
        engine
            .execute_command(&["101", "@", "100", "Enter"])
            .unwrap();
        engine.tick(0.0);
        let frame = engine.frame(1).unwrap();
        // Dimmer 9, Pan 10/11, Tilt 12, Zoom 13, R G B 14–16, White 17.
        let slots: Vec<u8> = (9..=17).map(|a| frame.channel(a).unwrap()).collect();
        assert_eq!(slots, [255, 0, 0, 0, 0, 255, 255, 255, 0]);
    }

    #[test]
    fn ipc_18_colour_model_command_reaches_the_engine() {
        let mut engine = demo_engine().unwrap();
        apply(
            &mut engine,
            ipc::Command::ColourModel {
                model: ColourModel::Rgb,
            },
            0.0,
        )
        .unwrap();
        assert_eq!(engine.colour_model(), ColourModel::Rgb);
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

    fn add(
        profile: &str,
        mode: &str,
        count: u32,
        address: Option<u16>,
        number: Option<u32>,
    ) -> PatchAdd {
        PatchAdd {
            profile: profile.into(),
            mode: Some(mode.into()),
            count,
            universe: 1,
            address,
            number,
        }
    }

    /// PAT-01's example from a library profile (the client thread attached it, PAT-09).
    #[test]
    fn pat_09_patch_add_installs_a_library_profile_and_patches() {
        let mut engine = demo_engine().unwrap();
        let profile = Profile::from_json(RGB_PAR).unwrap();
        let cmd = ipc::Command::PatchAdd {
            add: add("Showtec/LED Par 56", "default", 6, Some(201), Some(1221)),
            profile: Some(profile),
        };
        apply(&mut engine, cmd, 0.0).unwrap();
        let placed: Vec<(u32, u16)> = engine
            .fixtures()
            .filter(|f| f.profile == "Showtec/LED Par 56")
            .map(|f| (f.number, f.address))
            .collect();
        assert_eq!(
            placed,
            [
                (1221, 201),
                (1222, 207),
                (1223, 213),
                (1224, 219),
                (1225, 225),
                (1226, 231)
            ]
        );
        // A second add reuses the show's copy; no profile needed. PAT-03: first free run.
        let cmd = ipc::Command::PatchAdd {
            add: add("Showtec/LED Par 56", "default", 1, None, None),
            profile: None,
        };
        apply(&mut engine, cmd, 0.0).unwrap();
        assert_eq!(engine.fixture_by_number(1227).unwrap().address, 151);
    }

    #[test]
    fn pat_09_patch_add_keeps_the_shows_copy_of_a_known_profile() {
        let mut engine = demo_engine().unwrap();
        // A different profile under the demo's "rgbw" key must not replace the show's copy.
        let cmd = ipc::Command::PatchAdd {
            add: add("rgbw", "default", 1, None, Some(2001)),
            profile: Some(Profile::from_json(RGB_PAR).unwrap()),
        };
        apply(&mut engine, cmd, 0.0).unwrap();
        assert_eq!(engine.fixture(201).unwrap().address, 45);
        let new = engine.fixture_by_number(2001).unwrap();
        assert_eq!(
            new.address, 151,
            "first free run after the spots, RGBW footprint"
        );
        let next = ipc::Command::PatchAdd {
            add: add("rgbw", "default", 1, None, None),
            profile: None,
        };
        apply(&mut engine, next, 0.0).unwrap();
        assert_eq!(
            engine.fixture_by_number(2002).unwrap().address,
            156,
            "5 channels on"
        );
    }

    #[test]
    fn pat_09_refusals_are_errors_and_change_nothing() {
        let mut engine = demo_engine().unwrap();
        let before: Vec<_> = engine.fixtures().cloned().collect();
        let unknown = ipc::Command::PatchAdd {
            add: add("Nope/Nope", "default", 1, None, None),
            profile: None,
        };
        assert_eq!(
            apply(&mut engine, unknown, 0.0),
            Err(CoreError::UnknownProfile("Nope/Nope".into()))
        );
        // PAT-02: 2 RGBW pars at 140 would overlap 226 at 140.
        let clash = ipc::Command::PatchAdd {
            add: add("rgbw", "default", 2, Some(140), Some(900)),
            profile: None,
        };
        assert!(matches!(
            apply(&mut engine, clash, 0.0),
            Err(CoreError::PatchOverlap { .. })
        ));
        let after: Vec<_> = engine.fixtures().cloned().collect();
        assert_eq!(before, after);
    }

    #[test]
    fn pat_09_edit_commands_reach_the_engine() {
        let mut engine = demo_engine().unwrap();
        let mut run = |cmd| apply(&mut engine, cmd, 0.0).unwrap();
        run(ipc::Command::PatchMove {
            id: 301,
            universe: 2,
            address: 1,
        });
        run(ipc::Command::PatchRenumber {
            id: 302,
            number: 1302,
        });
        run(ipc::Command::PatchName {
            id: 303,
            name: "Pulpit".into(),
        });
        run(ipc::Command::PatchHandle {
            id: 304,
            handle: Handle { page: 3, slot: 7 },
        });
        run(ipc::Command::PatchRemove {
            ids: vec![305, 306],
        });
        let f = |id| engine.fixture(id).cloned();
        assert_eq!(f(301).map(|f| (f.universe, f.address)), Some((2, 1)));
        assert_eq!(f(302).map(|f| f.number), Some(1302));
        assert_eq!(f(303).map(|f| f.name), Some("Pulpit".to_string()));
        assert_eq!(
            f(304).and_then(|f| f.handle),
            Some(Handle { page: 3, slot: 7 })
        );
        assert!(f(305).is_none() && f(306).is_none());
        assert_eq!(engine.group(4), Some(&[301, 302, 303, 304][..]));
    }

    #[test]
    fn pat_09_change_mode_reaches_the_engine() {
        let mut engine = demo_engine().unwrap();
        let cmd = ipc::Command::PatchChangeMode {
            id: 201,
            mode: "Nope".into(),
        };
        assert!(matches!(
            apply(&mut engine, cmd, 0.0),
            Err(CoreError::UnknownMode { .. })
        ));
        let cmd = ipc::Command::PatchChangeMode {
            id: 201,
            mode: "default".into(),
        };
        apply(&mut engine, cmd, 0.0).unwrap();
        assert_eq!(
            engine.fixture(201).unwrap().mode.as_deref(),
            Some("default")
        );
    }

    #[test]
    fn ipc_23_colour_command_reaches_the_engine() {
        let mut engine = demo_engine().unwrap();
        engine.execute_command(&["201", "Enter"]).unwrap();
        apply(
            &mut engine,
            ipc::Command::Colour {
                hue: 2.0 / 3.0,
                saturation: 1.0,
            },
            0.0,
        )
        .unwrap();
        let c = engine.programmer_view().colour.unwrap();
        assert_eq!((c.r, c.g, c.b), (0.0, 0.0, 1.0));
    }

    #[test]
    fn ipc_09_demo_show_has_one_dimmer_per_fader() {
        let engine = demo_engine().unwrap();
        let ids: Vec<_> = engine.playback_states().map(|(id, _)| *id).collect();
        let n = u32::from(engine.settings().faders_per_page);
        // PBK-04: plus two on page 2.
        assert_eq!(ids, (1..=n).chain([n + 1, n + 2]).collect::<Vec<_>>());
    }

    #[test]
    fn pbk_04_demo_page_2_has_the_washes_and_the_front_spots() {
        let mut engine = demo_engine().unwrap();
        engine.set_page(2).unwrap();
        let slots = engine.page_slots();
        assert_eq!(slots.first(), Some(&Some(9)));
        assert_eq!(slots.get(1), Some(&Some(10)));
        assert!(slots.iter().skip(2).all(Option::is_none));
        engine.set_fader(9, 1.0, 0.0).unwrap();
        engine.set_fader(10, 1.0, 0.0).unwrap();
        engine.tick(0.0);
        let ch = |a| engine.frame(1).and_then(|f| f.channel(a)).unwrap();
        // Wash 101's Dimmer is channel 9; front spot 301 is channel 145 (IPC-14 layout).
        assert_eq!(ch(9), 255);
        assert_eq!(ch(145), 255);
        assert_eq!(ch(1), 0, "the page-1 dimmers stay out");
    }

    #[test]
    fn pal_27_palette_commands_reach_the_engine() {
        use cmdmx_core::palette::{Cell, Record, RecordBy, Timing, Window};
        let mut engine = demo_engine().unwrap();
        let run = |engine: &mut Engine, cmd| apply(engine, cmd, 0.0);
        engine.execute_command(&["201", "Enter"]).unwrap();
        run(
            &mut engine,
            ipc::Command::Colour {
                hue: 0.0,
                saturation: 1.0,
            },
        )
        .unwrap();
        run(
            &mut engine,
            ipc::Command::PaletteRecord(Record::at(Cell::new(Window::Colours, 1))),
        )
        .unwrap();
        assert!(engine.palette(1).is_some());
        run(
            &mut engine,
            ipc::Command::PaletteLegend {
                number: 1,
                legend: "Red".into(),
                icon: Some("colour.drop".into()),
            },
        )
        .unwrap();
        assert_eq!(engine.palette(1).unwrap().legend, "Red");
        run(
            &mut engine,
            ipc::Command::PaletteCopy {
                number: 1,
                to: Cell::new(Window::Colours, 2),
            },
        )
        .unwrap();
        run(
            &mut engine,
            ipc::Command::PaletteMove {
                number: 2,
                to: Cell::new(Window::Positions, 1),
            },
        )
        .unwrap();
        run(
            &mut engine,
            ipc::Command::PaletteRenumber { number: 2, to: 20 },
        )
        .unwrap();
        assert_eq!(engine.palette(20).unwrap().window, Window::Positions);
        engine.clear();
        engine.clear();
        run(
            &mut engine,
            ipc::Command::PaletteApply {
                number: 1,
                timing: Timing::default(),
            },
        )
        .unwrap();
        assert!(
            engine.programmer().in_programmer(221),
            "quick palette reaches every colour fixture"
        );
        engine.clear();
        engine.execute_command(&["202", "Enter"]).unwrap();
        run(&mut engine, ipc::Command::PaletteInclude { number: 1 }).unwrap();
        assert!(matches!(
            engine.programmer().stored(202, "Colour"),
            Some(cmdmx_core::value::Stored::Value(_))
        ));
        run(&mut engine, ipc::Command::PaletteDelete { number: 20 }).unwrap();
        assert!(engine.palette(20).is_none());
        run(
            &mut engine,
            ipc::Command::SettingsPalette {
                time: Some(2.0),
                overlap: Some(50.0),
                record_by: Some(RecordBy::Channel),
            },
        )
        .unwrap();
        let s = engine.settings();
        assert_eq!(
            (s.palette_time, s.palette_overlap, s.record_by),
            (2.0, 50.0, RecordBy::Channel)
        );
    }

    #[test]
    fn pal_27_refusals_change_nothing() {
        use cmdmx_core::palette::{Timing, Window};
        let mut engine = demo_engine().unwrap();
        let before = engine.to_show(cmdmx_core::show::Meta {
            name: "Demo".into(),
            created: String::new(),
            modified: String::new(),
            app_version: String::new(),
        });
        for cmd in [
            ipc::Command::PaletteApply {
                number: 99,
                timing: Timing::default(),
            },
            ipc::Command::PaletteDelete { number: 99 },
            ipc::Command::PaletteRecord(cmdmx_core::palette::Record::at(
                cmdmx_core::palette::Cell::new(Window::Colours, 1),
            )),
            ipc::Command::SettingsPalette {
                time: Some(700.0),
                overlap: None,
                record_by: None,
            },
        ] {
            assert!(apply(&mut engine, cmd.clone(), 0.0).is_err(), "{cmd:?}");
        }
        let after = engine.to_show(before.meta.clone());
        assert_eq!(before, after);
    }
}
