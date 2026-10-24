//! cmdmx-daemon — the ColourmotionDMX engine process.
//!
//! Runs the deterministic core at 40 Hz with a demo patch and sends each line to the output
//! nodes of the show (Art-Net, sACN, ENTTEC Pro serial), each node on its own thread (OUT-11,
//! OUT-29). Node states reach the app in the snapshot and the `outputs` delta (OUT-33).
//! MIDI runs on its own thread too (VS7-02): the tick only drains its events and sends feedback.
//!
//! Usage: `cmdmx-daemon [--ticks N] [--no-artnet] [--no-sacn] [--no-serial] [--ipc-port N]
//!        [--demo] [--qlcplus-dir PATH] [--profiles-dir PATH] [--show-dir PATH]`.
//! The output nodes live in the show (set over `output.*`); the `--no-*` flags only keep a
//! kind from sending this run (OUT-32).
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
//! Generic MIDI control (CTL-*): mapping files in `--show-dir`/control (default beside
//! `state.json`), devices enabled over `control.*`; their commands run like the app's (CTL-06).
//! OSC (VS7-03) is the same layer over a UDP socket on its own thread (`osc_io`): off until
//! `control.osc` enables it (CTL-35), feedback goes to the senders heard and the listed clients.
//! Edit commands run through the undo history (UNDO-01..04, IPC-30/31); open and new empty it.

mod artnet_out;
mod control;
mod ipc;
mod library;
mod midi_io;
mod osc_io;
mod output;
mod sacn_out;
mod serial_out;
mod shows;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use cmdmx_core::attribute::{AttributeDef, AttributeGroup, AttributeKind};
use cmdmx_core::engine::{Engine, Settings, ALL_GROUPS};
use cmdmx_core::patch::Fixture;
use cmdmx_core::playback::Playback;
use cmdmx_core::profile::Profile;
use cmdmx_core::undo::History;
use cmdmx_core::CoreError;
const TICK: Duration = Duration::from_millis(25);

fn main() {
    let cli = match parse_args(std::env::args().skip(1)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("cmdmx-daemon: {e}");
            eprint!("{USAGE}");
            std::process::exit(2);
        }
    };
    if cli.help {
        print!("{USAGE}");
        return;
    }
    let ticks_limit = cli.ticks;
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
    // CTL-23, 08: the mapping files and the devices; the MIDI thread runs without any of them.
    let (mut control, warnings) =
        control::Control::load(cli.control_dir.clone(), cli.state_file.clone());
    for w in &warnings {
        eprintln!("warning: {w}");
    }
    pending.extend(warnings);
    let (midi_events, midi_in) = std::sync::mpsc::channel();
    let midi_out = match midi_io::spawn(midi_events) {
        Ok(tx) => Some(tx),
        Err(e) => {
            eprintln!("warning: MIDI is off: {e}");
            pending.push(format!("MIDI is off: {e}"));
            None
        }
    };
    let (osc_events, osc_in) = std::sync::mpsc::channel();
    let osc_out = match osc_io::spawn(osc_events) {
        Ok(tx) => Some(tx),
        Err(e) => {
            eprintln!("warning: OSC is off: {e}");
            pending.push(format!("OSC is off: {e}"));
            None
        }
    };
    // The port the OSC thread was last told to listen on (`None` = not told yet).
    let mut osc_told: Option<Option<u16>> = None;
    // OUT-29: the show's nodes, kept running; reconciled again after every drain of events.
    let mut outputs = output::Outputs::new(cli.kill, Box::new(output::spawn_node));
    outputs.reconcile(engine.nodes());
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
    // The tick's clock; the client threads stamp taps and buttons on it (IPC-25).
    let started = Instant::now();
    let (events, ipc_addr) =
        match ipc::spawn_server_with(cli.ipc_port, library.clone(), Arc::clone(&shows), started) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("cmdmx-daemon: cannot open IPC port {}: {e}", cli.ipc_port);
                std::process::exit(2);
            }
        };
    eprintln!("ipc: listening on ws://{ipc_addr}");
    // LIB-13: index the library in the background; until it is stored, requests get "loading".
    let (qlcplus_dir, profiles_dir) = (cli.qlcplus_dir.clone(), cli.profiles_dir.clone());
    let loading = Arc::clone(&library);
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
            if let Ok(mut slot) = loading.write() {
                *slot = Some(lib);
            }
        });
    if let Err(e) = loader {
        eprintln!("library: cannot start the loader thread: {e}");
    }
    let mut clients = ipc::Clients::default();
    let mut history = History::default();
    let mut published = ipc::Published::new(&engine, outputs.statuses(), &autosave.state());

    eprintln!("cmdmx-daemon 0.1.0 — 40 Hz tick loop running.");
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
                    clients.add(
                        tx,
                        &ipc::snapshot(
                            &engine,
                            outputs.statuses(),
                            &autosave.state(),
                            &undo_info(&history),
                            &control.snapshot(),
                        ),
                    );
                    for w in std::mem::take(&mut pending) {
                        clients.broadcast(&ipc::warning(&w));
                    }
                    Ok(())
                }
                ipc::Event::Warning(e) => {
                    warn(&mut clients, &e);
                    continue;
                }
                // CTL-40: a refusal is a warning for every client, like a failed command.
                ipc::Event::Command(ipc::Command::Control(req)) => {
                    if let Err(e) = control.request(req) {
                        warn(&mut clients, &e);
                    }
                    continue;
                }
                ipc::Event::ControlMapping { name, reply } => {
                    drop(reply.try_send(control.mapping_reply(&name)));
                    Ok(())
                }
                // IPC-24: to the asking client only; a full queue drops the answer (IPC-07).
                ipc::Event::CueView { id, cue, reply } => {
                    ipc::cue_view(&engine, id, cue).map(|msg| drop(reply.try_send(msg)))
                }
                // IPC-37: an unknown key is an error to the asking client only.
                ipc::Event::PatchProfile { key, reply } => {
                    let msg = ipc::patch_profile(&engine, &key)
                        .unwrap_or_else(|e| ipc::error(&e.to_string()));
                    drop(reply.try_send(msg));
                    Ok(())
                }
                // IPC-38: the show first (one undo step), then the library file off the tick.
                ipc::Event::ProfileSave { profile, reply } => {
                    let saved = run(
                        &mut engine,
                        &mut history,
                        ipc::Command::ProfileSave(profile.clone()),
                        now,
                    );
                    saved.map(|()| {
                        let key = format!("{}/{}", profile.manufacturer, profile.model);
                        let fixtures = engine.fixtures().filter(|f| f.profile == key).count();
                        write_library_file(&library, profile, fixtures, reply);
                    })
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
                    let save_as = matches!(
                        &event,
                        ipc::Event::Command(ipc::Command::Show(SaveAs { .. }))
                    );
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
                        // UNDO-13: open and new empty the history, save as keeps it.
                        if !save_as {
                            history.clear();
                        }
                        engine = next.engine;
                        outputs.reconcile(engine.nodes());
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
                        published = ipc::Published::new(&engine, outputs.statuses(), &state);
                        // The snapshot below carries the labels; no `undo` delta for them.
                        let _ = published.undo_delta(&undo_info(&history));
                        clients.broadcast(&ipc::snapshot(
                            &engine,
                            outputs.statuses(),
                            &state,
                            &undo_info(&history),
                            &control.snapshot(),
                        ));
                        for w in &next.warnings {
                            warn(&mut clients, w);
                        }
                    }
                    Ok(())
                }
                ipc::Event::Command(cmd) => run(&mut engine, &mut history, cmd, now),
            };
            if let Err(e) = result {
                eprintln!("warning: {e}");
                clients.broadcast(&ipc::warning(&e.to_string()));
            }
        }
        // CTL-14: every MIDI message waiting is decoded now; commands go out coalesced (CTL-06).
        while let Ok(event) = midi_in.try_recv() {
            control.event(event, now);
        }
        let want = control.osc_listen();
        if osc_told != Some(want) {
            osc_told = Some(want);
            if let Some(tx) = &osc_out {
                let _ = tx.send(osc_io::OscCmd::Listen(want));
            }
        }
        while let Ok(event) = osc_in.try_recv() {
            control.osc_event(event, now);
        }
        for (device, cmd) in control.commands(&engine, now) {
            if let Err(e) = run_control(&mut engine, &mut history, &device, cmd, now) {
                warn(&mut clients, &e);
            }
        }
        outputs.reconcile(engine.nodes());
        if cli.demo {
            // Demo motion: fader 1 breathes so the output visibly changes.
            let level = ((now * 0.5).sin() * 0.5 + 0.5) as f32;
            if let Err(e) = engine.set_fader(1, level, now) {
                eprintln!("warning: {e}");
            }
        }
        engine.tick(now);
        // CTL-21: feedback for what this frame shows; a gone MIDI thread drops it.
        for out in control.feedback(&engine) {
            if let Some(tx) = &midi_out {
                let _ = tx.send(out);
            }
        }
        control.note_warning(&clients.last_warning);
        for (to, m) in control.osc_feedback(&engine, now) {
            if let Some(tx) = &osc_out {
                let bytes = cmdmx_control::osc::encode(&m);
                let _ = tx.send(osc_io::OscCmd::Send { to, bytes });
            }
        }
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
        outputs.publish(|line| engine.frame(line).map(|f| f.slots()));
        for w in engine.drain_warnings() {
            eprintln!("warning @{:.3}: {}", w.at, w.message);
            clients.last_warning.clone_from(&w.message);
            clients.broadcast(&ipc::warning(&w.message));
        }
        // IPC-31: the undo delta goes before the others.
        if let Some(msg) = published.undo_delta(&undo_info(&history)) {
            clients.broadcast(&msg);
        }
        for msg in published.diff(&engine, outputs.statuses(), &autosave.state()) {
            clients.broadcast(&msg);
        }
        for text in control.take_messages() {
            warn(&mut clients, &text);
        }
        if let Some(msg) = control.delta(now) {
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

/// IPC-38: write the saved profile into the user library on a thread of its own, so a slow
/// disk never holds the tick; the outcome goes to the asking client.
fn write_library_file(
    library: &ipc::SharedLibrary,
    profile: cmdmx_core::profile::Profile,
    fixtures: usize,
    reply: std::sync::mpsc::SyncSender<String>,
) {
    let lib = Arc::clone(library);
    let answer = reply.clone();
    let spawned = std::thread::Builder::new()
        .name("profile-save".into())
        .spawn(move || {
            drop(answer.try_send(ipc::write_user_profile(&lib, &profile, fixtures)));
        });
    if let Err(e) = spawned {
        let msg = format!("Saved in the show, not in the library: cannot start the writer: {e}");
        eprintln!("warning: {msg}");
        drop(reply.try_send(ipc::warning(&msg)));
    }
}

fn warn(clients: &mut ipc::Clients, msg: &str) {
    eprintln!("warning: {msg}");
    clients.last_warning = msg.to_string();
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
        ClearWith { groups, time } => match time {
            Some(time) => engine.clear_timed(time, groups.as_deref()),
            None => engine.clear_mask(groups.as_deref().unwrap_or_default()),
        },
        Locate { groups } => engine.locate(groups.as_deref().unwrap_or(&ALL_GROUPS)),
        Highlight { on } => engine.set_highlight(on),
        Off {
            groups: None,
            attribute: Some(attribute),
        } => engine.off_attribute(&attribute),
        Off { groups, .. } => {
            let active = engine.active_group();
            engine.off(groups.as_deref().unwrap_or(&[active]))
        }
        RemDim => engine.rem_dim(),
        SettingsProgrammer(change) => engine.set_programmer_settings(change),
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
        // Handled by the control layer on the tick (CTL-40).
        Control(_) | ControlMapping { .. } => Ok(()),
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
        OutputAdd {
            line,
            enabled,
            kind,
        } => engine.output_add(line, enabled, kind).map(drop),
        OutputRemove { ids } => engine.output_remove(&ids),
        // OUT-34: the changed fields go over the stored node; the engine checks the whole.
        OutputEdit { id, change } => {
            let stored = engine.nodes().find(|n| n.id == id);
            let stored = stored.ok_or(CoreError::UnknownNode(id))?;
            let invalid = |e: serde_json::Error| CoreError::InvalidOutput(e.to_string());
            let serde_json::Value::Object(mut merged) =
                serde_json::to_value(stored).map_err(invalid)?
            else {
                return Err(CoreError::UnknownNode(id));
            };
            if change
                .get("kind")
                .is_some_and(|k| merged.get("kind") != Some(k))
            {
                return Err(CoreError::NodeKindChanged(id));
            }
            for (key, n) in &change {
                if let Some(n) = n.as_i64() {
                    cmdmx_core::output::check_number(key, n)?;
                }
            }
            merged.extend(change);
            merged.insert("id".into(), id.into());
            let node =
                serde_json::from_value(serde_json::Value::Object(merged)).map_err(invalid)?;
            engine.output_edit(node)
        }
        // Answered in the client thread (OUT-35).
        OutputPorts => Ok(()),
        PatchRemove { ids } => engine.patch_remove(&ids),
        PatchRenumber { id, number } => engine.patch_renumber(id, number),
        PatchName { id, name } => engine.patch_name(id, &name),
        PatchChangeMode { id, mode } => engine.patch_change_mode(id, &mode),
        PatchHandle { id, handle } => engine.patch_handle(id, handle),
        PaletteRecord(record) => engine.record_palette(&record).map(drop),
        PaletteApply { number, timing } => engine.apply_palette(number, timing),
        PaletteInclude { number } => engine.include_palette(number),
        PaletteCopy { number, to } => engine.copy_palette(number, to).map(drop),
        PaletteMove { number, to } => engine.move_palette(number, to),
        PaletteRenumber { number, to } => engine.renumber_palette(number, to),
        PaletteDelete { number } => engine.delete_palette(number),
        PaletteLegend {
            number,
            legend,
            icon,
        } => engine.set_palette_legend(number, &legend, icon.as_deref()),
        // PAL-15/27: out of range refuses the whole command, so check before changing either.
        SettingsPalette {
            time,
            overlap,
            record_by,
        } => {
            engine.set_palette_master(time, overlap)?;
            if let Some(record_by) = record_by {
                engine.set_record_by(record_by);
            }
            Ok(())
        }
        PlaybackRecord(record) => engine.record_playback(&record).map(drop),
        PlaybackFlash { id, on } => engine.set_flash(id, on, now),
        PlaybackSwop { id, on } => engine.set_swop(id, on, now),
        PlaybackLatch { id } => engine.latch_flash(id, now),
        PlaybackRelease { id } => engine.release_playback(id, now),
        PlaybackChange { id, change } => engine.change_playback(id, &change),
        PlaybackLegend {
            id,
            legend,
            icon,
            halo,
        } => engine.set_playback_legend(id, &legend, icon.as_deref(), halo),
        PlaybackCopy { id, page, slot } => engine.copy_playback(id, page, slot).map(drop),
        PlaybackMove { id, page, slot } => engine.move_playback(id, page, slot),
        PlaybackDelete { id } => engine.delete_playback(id),
        PageLegend { page, legend } => engine.set_page_legend(page, &legend),
        DboSet { on } => {
            engine.set_dbo(on);
            Ok(())
        }
        SettingsPlayback(change) => engine.set_playback_settings(&change),
        CuelistAction { id, action } => engine.cuelist(id, action, now),
        CuelistConnect { id } => engine.connect_list(id),
        PlaybackButton { id, button, on, at } => engine.press_button(id, button, on, at),
        PlaybackTap { id, at } => engine.tap(id, at),
        PlaybackBpm { id, bpm } => engine.set_bpm(id, bpm),
        // IPC-27: merge the partial objects into the stored ones; the engine checks the whole.
        PlaybackOptions {
            id,
            mut change,
            list,
            chase,
            master,
        } => {
            let stored = engine.playback(id).ok_or(CoreError::UnknownPlayback(id))?;
            change.list = list.map(|p| p.over(stored.list.clone()));
            change.chase = chase.map(|p| p.over(stored.chase.clone()));
            if let Some(patch) = master {
                let base = stored.master.clone();
                let base = base.ok_or_else(|| CoreError::InvalidOption("master".into()))?;
                change.master = Some(patch.over(base)?);
            }
            engine.change_playback(id, &change)
        }
        CueEdit { id, cue, change } => engine.change_cue(id, cue, &change),
        CueDelete { id, cue } => engine.delete_cue(id, cue),
        CueRenumber { id, cue, to } => engine.renumber_cue(id, cue, to),
        CueRenumberAll { id } => engine.renumber_cues(id),
        CueCopy { id, cue, to } => engine.copy_cue(id, cue, to),
        // Answered through `Event::CueView` (IPC-24).
        CueView { .. } => Ok(()),
        // Answered through `Event::PatchProfile` (IPC-37).
        PatchProfile { .. } => Ok(()),
        ProfileSave(p) => engine
            .save_profile(&format!("{}/{}", p.manufacturer, p.model), p)
            .map(drop),
        ProgrammerSet { attribute, value } => engine.set_programmer_level(&attribute, value),
        CueRemove {
            id,
            cue,
            fixture,
            attribute,
        } => engine.remove_cue_value(id, cue, fixture, attribute.as_deref()),
        // IPC-26: no number is the lowest free one.
        MasterRecord {
            page,
            slot,
            kind,
            number,
        } => {
            let number = number.or_else(|| engine.free_master_number(kind));
            let number = number.ok_or_else(|| {
                CoreError::InvalidCommand(format!("no free {kind:?} master number left"))
            })?;
            engine.record_master(page, slot, kind, number).map(drop)
        }
        // GRM-15
        GroupMasterRecord { page, slot, group } => {
            engine.record_group_master(page, slot, group).map(drop)
        }
        // Handled by `run`.
        Undo | Redo => Ok(()),
    }
}

/// UNDO-04: how a command relates to the undo history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    /// A step in the history.
    Edit,
    /// Performance: not undoable, ends a coalescing run.
    Live,
    /// Replaces the whole show (the history is emptied elsewhere).
    Reset,
    /// Neither.
    Neutral,
}

/// UNDO-04: no wildcard, so a new command must be classed here to compile.
fn class(cmd: &ipc::Command) -> Class {
    use ipc::Command::*;
    use shows::ShowRequest::{List, New, Open, SaveAs};
    match cmd {
        FaderSet { .. }
        | GrandMasterSet { .. }
        | DboSet { .. }
        | PageSet { .. }
        | PlaybackButton { .. }
        | PlaybackFlash { .. }
        | PlaybackSwop { .. }
        | PlaybackLatch { .. }
        | PlaybackRelease { .. }
        | PlaybackTap { .. }
        | PlaybackBpm { .. }
        | CuelistAction { .. }
        | CuelistConnect { .. }
        | AttributeGroup { .. }
        | Highlight { .. }
        | ColourModel { .. } => Class::Live,
        Show(New { .. } | Open { .. }) => Class::Reset,
        Show(List | SaveAs { .. })
        | CueView { .. }
        | PatchProfile { .. }
        | Control(_)
        | ControlMapping { .. }
        | Library(_)
        | OutputPorts
        | Undo
        | Redo => Class::Neutral,
        Select { .. }
        | SelectGroup { .. }
        | Step { .. }
        | Pattern { .. }
        | All
        | Clear
        | ClearWith { .. }
        | Locate { .. }
        | Off { .. }
        | RemDim
        | SettingsProgrammer(_)
        | Wheel { .. }
        | ProgrammerSet { .. }
        | ProfileSave(_)
        | Colour { .. }
        | Line { .. }
        | RecordGroup { .. }
        | OutputAdd { .. }
        | OutputEdit { .. }
        | OutputRemove { .. }
        | PatchAdd { .. }
        | PatchMove { .. }
        | PatchRemove { .. }
        | PatchRenumber { .. }
        | PatchName { .. }
        | PatchChangeMode { .. }
        | PatchHandle { .. }
        | PaletteRecord(_)
        | PaletteApply { .. }
        | PaletteInclude { .. }
        | PaletteCopy { .. }
        | PaletteMove { .. }
        | PaletteRenumber { .. }
        | PaletteDelete { .. }
        | PaletteLegend { .. }
        | SettingsPalette { .. }
        | PlaybackRecord(_)
        | PlaybackChange { .. }
        | PlaybackLegend { .. }
        | PlaybackCopy { .. }
        | PlaybackMove { .. }
        | PlaybackDelete { .. }
        | PageLegend { .. }
        | SettingsPlayback(_)
        | PlaybackOptions { .. }
        | CueEdit { .. }
        | CueDelete { .. }
        | CueRenumber { .. }
        | CueRenumberAll { .. }
        | CueCopy { .. }
        | CueRemove { .. }
        | MasterRecord { .. }
        | GroupMasterRecord { .. } => Class::Edit,
    }
}

/// The wire name of an attribute group (IPC-11), for labels.
fn group_name(group: &cmdmx_core::attribute::AttributeGroup) -> String {
    serde_json::to_value(group)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// UNDO-01, 05: the step's label and its coalescing key.
fn label(cmd: &ipc::Command) -> (String, Option<String>) {
    use ipc::Command::*;
    let (label, key) = match cmd {
        Line { tokens } => (tokens.join(" "), None),
        Clear | ClearWith { .. } => ("Clear".into(), None),
        Locate { .. } => ("Locate".into(), None),
        Off {
            groups: Some(groups),
            ..
        } => {
            let names: Vec<String> = groups.iter().map(group_name).collect();
            (format!("Off {}", names.join(" ")), None)
        }
        Off {
            attribute: Some(attribute),
            ..
        } => (format!("Off {attribute}"), None),
        Off { .. } => ("Off".into(), None),
        RemDim => ("Rem Dim".into(), None),
        SettingsProgrammer(_) => ("Settings".into(), None),
        Select { .. } | SelectGroup { .. } | Step { .. } | Pattern { .. } | All => {
            ("Select".into(), Some("select".into()))
        }
        Wheel { attribute, .. } => (
            format!("Wheel {attribute}"),
            Some(format!("wheel:{attribute}")),
        ),
        ProgrammerSet { attribute, .. } => {
            (format!("Set {attribute}"), Some(format!("set:{attribute}")))
        }
        ProfileSave(p) => (format!("Edit profile {}", p.model), None),
        Colour { .. } => ("Colour".into(), Some("colour".into())),
        RecordGroup { group } => (format!("Record group {group}"), None),
        OutputAdd { .. } => ("Add output".into(), None),
        OutputEdit { .. } => ("Edit output".into(), None),
        OutputRemove { .. } => ("Remove output".into(), None),
        PatchAdd { .. } => ("Patch add".into(), None),
        PatchMove { id, .. } => (format!("Patch move {id}"), None),
        PatchRemove { .. } => ("Patch remove".into(), None),
        PatchRenumber { id, .. } => (format!("Patch renumber {id}"), None),
        PatchName { id, .. } => (format!("Patch name {id}"), None),
        PatchChangeMode { id, .. } => (format!("Patch mode {id}"), None),
        PatchHandle { id, .. } => (format!("Patch handle {id}"), None),
        PaletteRecord(_) => ("Record palette".into(), None),
        PaletteApply { number, .. } => (format!("Apply palette {number}"), None),
        PaletteInclude { number } => (format!("Include palette {number}"), None),
        PaletteCopy { number, .. } => (format!("Copy palette {number}"), None),
        PaletteMove { number, .. } => (format!("Move palette {number}"), None),
        PaletteRenumber { number, .. } => (format!("Renumber palette {number}"), None),
        PaletteDelete { number } => (format!("Delete palette {number}"), None),
        PaletteLegend { number, .. } => (format!("Palette {number} legend"), None),
        SettingsPalette { .. } => ("Palette settings".into(), None),
        PlaybackRecord(_) => ("Record playback".into(), None),
        PlaybackChange { id, .. } | PlaybackOptions { id, .. } => {
            (format!("Playback {id} options"), None)
        }
        PlaybackLegend { id, .. } => (format!("Playback {id} legend"), None),
        PlaybackCopy { id, .. } => (format!("Copy playback {id}"), None),
        PlaybackMove { id, .. } => (format!("Move playback {id}"), None),
        PlaybackDelete { id } => (format!("Delete playback {id}"), None),
        PageLegend { page, .. } => (format!("Page {page} legend"), None),
        SettingsPlayback(_) => ("Playback settings".into(), None),
        CueEdit { .. } => ("Edit cue".into(), None),
        CueDelete { .. } => ("Delete cue".into(), None),
        CueRenumber { .. } | CueRenumberAll { .. } => ("Renumber cue".into(), None),
        CueCopy { .. } => ("Copy cue".into(), None),
        CueRemove { .. } => ("Remove cue value".into(), None),
        MasterRecord { .. } | GroupMasterRecord { .. } => ("Record master".into(), None),
        _ => ("Edit".into(), None),
    };
    (label, key)
}

/// UNDO-01, 04, 14, 15: run a command through the history by its class.
fn run(
    engine: &mut Engine,
    history: &mut History,
    cmd: ipc::Command,
    now: f64,
) -> Result<(), CoreError> {
    match (class(&cmd), &cmd) {
        (_, ipc::Command::Undo) => history.undo(engine).map(drop),
        (_, ipc::Command::Redo) => history.redo(engine).map(drop),
        (Class::Edit, _) => {
            let (mut label, key) = label(&cmd);
            if matches!(
                cmd,
                ipc::Command::Off {
                    groups: None,
                    attribute: None
                }
            ) {
                label = format!("Off {}", group_name(&engine.active_group())); // IPC-34
            }
            history.edit(engine, &label, key.as_deref(), |e| apply(e, cmd, now))
        }
        (Class::Live, _) => {
            history.live();
            apply(engine, cmd, now)
        }
        (Class::Reset | Class::Neutral, _) => apply(engine, cmd, now),
    }
}

/// CTL-06: a command from a control surface runs like one from the app; a refusal names the device.
fn run_control(
    engine: &mut Engine,
    history: &mut History,
    device: &str,
    cmd: ipc::Command,
    now: f64,
) -> Result<(), String> {
    run(engine, history, cmd, now).map_err(|e| format!("{device}: {e}"))
}

/// IPC-31
fn undo_info(history: &History) -> ipc::UndoInfo {
    ipc::UndoInfo {
        undo: history.undo_label().map(str::to_string),
        redo: history.redo_label().map(str::to_string),
    }
}

/// SHW-18: printed by `--help` (stdout, exit 0) and after a bad argument (stderr, exit 2).
const USAGE: &str = "\
usage: cmdmx-daemon [options]
  --ticks <n>             stop after n ticks (default: run until stopped)
  --no-artnet             do not send Art-Net this run (the show keeps its nodes)
  --no-sacn               do not send sACN this run (the show keeps its nodes)
  --no-serial             do not send serial DMX this run (the show keeps its nodes)
  --ipc-port <n>          WebSocket port on 127.0.0.1 (default 7319)
  --demo                  fader 1 breathes
  --qlcplus-dir <path>    QLC+ fixture files (default: bundled vendor/qlcplus/fixtures)
  --profiles-dir <path>   saved profiles (default ~/Library/Application Support/ColourmotionDMX/profiles)
  --show-dir <path>       shows folder (default ~/Documents/ColourmotionDMX/Shows)
  -h, --help              print this and exit
";

struct Cli {
    /// `--help`/`-h`: print USAGE and exit 0 (SHW-18).
    help: bool,
    ticks: Option<u64>,
    /// `--no-artnet`, `--no-sacn`, `--no-serial`: kinds that do not send this run (OUT-32).
    kill: output::Kill,
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
    /// Mapping files of the control layer (CTL-23); `<show-dir>/control` with `--show-dir`.
    control_dir: PathBuf,
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Cli, String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let mut cli = Cli {
        help: false,
        ticks: None,
        kill: output::Kill::default(),
        ipc_port: 7319,
        demo: false,
        qlcplus_dir: PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../vendor/qlcplus/fixtures"
        )),
        profiles_dir: home.join("Library/Application Support/ColourmotionDMX/profiles"),
        show_dir: home.join("Documents/ColourmotionDMX/Shows"),
        state_file: home.join("Library/Application Support/ColourmotionDMX/state.json"),
        control_dir: home.join("Library/Application Support/ColourmotionDMX/control"),
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                cli.help = true;
                return Ok(cli);
            }
            "--ticks" => {
                let v = args.next().unwrap_or_default();
                cli.ticks = Some(
                    v.parse()
                        .map_err(|_| format!("--ticks: '{v}' is not a whole number"))?,
                );
            }
            "--no-artnet" => cli.kill.artnet = true,
            "--no-sacn" => cli.kill.sacn = true,
            "--no-serial" => cli.kill.serial = true,
            // OUT-32: the nodes live in the show now.
            "--artnet-target" | "--sacn-unicast" | "--sacn-priority" | "--serial" => {
                return Err(format!(
                    "{arg} is gone: output is set per show on the Output screen"
                ));
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
                cli.control_dir = cli.show_dir.join("control");
            }
            other => return Err(format!("unknown argument '{other}'")),
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
        // PBK-01: where playback n sits, on page 1 while n is up to `faders_per_page`.
        engine.add_playback(Playback::new(
            n,
            1,
            u16::try_from(n).unwrap_or(1),
            BTreeMap::from([(n, BTreeMap::from([("Dimmer".to_string(), 1.0.into())]))]),
        ));
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
    for (slot, ids) in [(1, 101..=104), (2, 301..=306)] {
        engine.add_playback(Playback::new(
            per_page + u32::from(slot),
            2,
            slot,
            ids.map(|f| (f, BTreeMap::from([("Dimmer".to_string(), 1.0.into())])))
                .collect(),
        ));
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

/// Once a second: channel 1 of each universe, so a human can see the tick runs.
fn report(engine: &Engine) {
    for (universe, frame) in engine.universes() {
        eprintln!("universe {universe}: ch1={}", frame.channel(1).unwrap_or(0));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;
    use cmdmx_core::colour::ColourModel;
    use cmdmx_core::patch::{Handle, PatchAdd};
    use cmdmx_core::programmer::Resolution;
    use cmdmx_core::undo::History;

    fn args(s: &str) -> impl Iterator<Item = String> + '_ {
        s.split_whitespace().map(String::from)
    }

    #[test]
    fn out_32_config_flags_are_gone_with_a_pointer_to_the_output_screen() {
        for (flag, value) in [
            ("--artnet-target", " 192.168.1.20"),
            ("--sacn-unicast", " 10.0.0.5"),
            ("--sacn-priority", " 150"),
            ("--serial", " /dev/cu.usbserial-X"),
            ("--serial", ""),
        ] {
            let e = parse_args(args(&format!("{flag}{value}"))).err().unwrap();
            assert_eq!(
                e,
                format!("{flag} is gone: output is set per show on the Output screen")
            );
        }
        for flag in [
            "--artnet-target",
            "--sacn-unicast",
            "--sacn-priority",
            "--serial",
        ] {
            assert!(!USAGE.contains(flag), "{flag} is still in the usage");
        }
    }

    #[test]
    fn out_32_no_flags_are_kill_switches_for_this_run() {
        let cli = parse_args(args("")).unwrap();
        assert_eq!(cli.kill, output::Kill::default(), "everything sends");
        assert_eq!(cli.ticks, None);
        let cli = parse_args(args("--no-sacn --ticks 5")).unwrap();
        assert_eq!(
            cli.kill,
            output::Kill {
                artnet: false,
                sacn: true,
                serial: false
            }
        );
        assert_eq!(cli.ticks, Some(5));
        let cli = parse_args(args("--no-artnet --no-serial")).unwrap();
        assert!(cli.kill.artnet && cli.kill.serial && !cli.kill.sacn);
        for flag in ["--no-artnet", "--no-sacn", "--no-serial"] {
            let line = USAGE
                .lines()
                .find(|l| l.trim_start().starts_with(flag))
                .unwrap();
            assert!(line.contains("this run"), "{line}");
        }
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

    /// Every flag `parse_args` knows, `--help` included.
    const FLAGS: [&str; 11] = [
        "--help",
        "--ticks",
        "--no-artnet",
        "--no-sacn",
        "--no-serial",
        "--ipc-port",
        "--demo",
        "--qlcplus-dir",
        "--profiles-dir",
        "--show-dir",
        "-h",
    ];

    #[test]
    fn shw_18_help_flag_asks_for_usage() {
        assert!(!parse_args(args("")).unwrap().help);
        assert!(parse_args(args("--help")).unwrap().help);
        assert!(parse_args(args("-h")).unwrap().help);
        assert!(parse_args(args("--show-dir /tmp/x --help")).unwrap().help);
    }

    #[test]
    fn shw_18_usage_names_every_flag_one_per_line() {
        // A flag's line starts with it (`-h, --help` shares one line).
        for flag in FLAGS {
            let lines = USAGE
                .lines()
                .filter(|l| {
                    l.split_whitespace()
                        .take(2)
                        .any(|w| w.trim_end_matches(',') == flag)
                })
                .count();
            assert_eq!(lines, 1, "{flag} must start exactly one usage line");
        }
    }

    #[test]
    fn shw_18_unknown_argument_is_an_error() {
        let e = parse_args(args("--bogus")).err().unwrap();
        assert_eq!(e, "unknown argument '--bogus'");
        assert!(parse_args(args("--ticks 1 -x")).is_err());
        assert!(parse_args(args("Kerk")).is_err());
        assert!(parse_args(args("--show-dir /tmp/x --helpme")).is_err());
    }

    #[test]
    fn shw_18_ticks_value_must_be_a_whole_number() {
        assert_eq!(parse_args(args("--ticks 5")).unwrap().ticks, Some(5));
        assert!(parse_args(args("--ticks nope")).is_err());
        assert!(parse_args(args("--ticks -1")).is_err());
        assert!(parse_args(args("--ticks")).is_err());
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
            ids: vec![301],
            universe: 2,
            address: 1,
        });
        run(ipc::Command::PatchRenumber {
            ids: vec![302],
            number: 1302,
        });
        run(ipc::Command::PatchName {
            ids: vec![303],
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
    fn ipc_40_range_commands_reach_the_engine() {
        let mut engine = demo_engine().unwrap();
        // 301..306 are one-channel dimmers.
        let cmd = ipc::Command::PatchMove {
            ids: vec![303, 301, 302],
            universe: 2,
            address: 101,
        };
        apply(&mut engine, cmd, 0.0).unwrap();
        let at = |id| engine.fixture(id).map(|f| (f.universe, f.address));
        assert_eq!(at(301), Some((2, 101)));
        assert_eq!(at(302), Some((2, 102)));
        assert_eq!(at(303), Some((2, 103)));
        let cmd = ipc::Command::PatchName {
            ids: vec![301, 302, 303],
            name: "Par 1".into(),
        };
        apply(&mut engine, cmd, 0.0).unwrap();
        assert_eq!(
            engine.fixture(303).map(|f| f.name.clone()),
            Some("Par 3".into())
        );
        // PAT-19: a refused range changes nothing.
        let before: Vec<_> = engine.fixtures().cloned().collect();
        let cmd = ipc::Command::PatchRenumber {
            ids: vec![301, 301],
            number: 900,
        };
        assert_eq!(
            apply(&mut engine, cmd, 0.0),
            Err(CoreError::RepeatedFixture(301))
        );
        let after: Vec<_> = engine.fixtures().cloned().collect();
        assert_eq!(before, after);
    }

    #[test]
    fn pat_19_range_undo_labels_carry_the_count() {
        use ipc::Command::*;
        let six = || vec![1, 2, 3, 4, 5, 6];
        let cases = [
            (
                PatchMove {
                    ids: six(),
                    universe: 2,
                    address: 1,
                },
                "Patch move 6 fixtures",
            ),
            (
                PatchRenumber {
                    ids: six(),
                    number: 1,
                },
                "Patch renumber 6 fixtures",
            ),
            (
                PatchName {
                    ids: six(),
                    name: "Par 1".into(),
                },
                "Patch name 6 fixtures",
            ),
            (
                PatchChangeMode {
                    ids: six(),
                    mode: "Big".into(),
                },
                "Patch mode 6 fixtures",
            ),
            (PatchRemove { ids: six() }, "Patch remove 6 fixtures"),
            (
                PatchMove {
                    ids: vec![301],
                    universe: 2,
                    address: 1,
                },
                "Patch move 301",
            ),
            (PatchRemove { ids: vec![301] }, "Patch remove"),
        ];
        for (cmd, want) in cases {
            assert_eq!(label(&cmd).0, want);
        }
    }

    #[test]
    fn pat_09_change_mode_reaches_the_engine() {
        let mut engine = demo_engine().unwrap();
        let cmd = ipc::Command::PatchChangeMode {
            ids: vec![201],
            mode: "Nope".into(),
        };
        assert!(matches!(
            apply(&mut engine, cmd, 0.0),
            Err(CoreError::UnknownMode { .. })
        ));
        let cmd = ipc::Command::PatchChangeMode {
            ids: vec![201],
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

    #[test]
    fn pbk_36_playback_commands_reach_the_engine() {
        use cmdmx_core::playback::{FaderMode, PlaybackChange, PlaybackRecord};
        let mut engine = demo_engine().unwrap();
        let run = |engine: &mut Engine, cmd| apply(engine, cmd, 0.0).unwrap();
        engine
            .execute_command(&["201", "@", "100", "Enter"])
            .unwrap();
        run(
            &mut engine,
            ipc::Command::PlaybackRecord(PlaybackRecord::at(3, 1)),
        );
        let id = engine
            .playbacks()
            .find(|p| (p.page, p.slot) == (3, 1))
            .unwrap()
            .id;
        run(
            &mut engine,
            ipc::Command::PlaybackChange {
                id,
                change: PlaybackChange {
                    fader_mode: Some(FaderMode::Mode1),
                    fade_out: Some(2.0),
                    ..PlaybackChange::default()
                },
            },
        );
        assert_eq!(engine.playback(id).unwrap().fader_mode, FaderMode::Mode1);
        assert_eq!(engine.playback(id).unwrap().cues[0].fade_out, 2.0);
        run(
            &mut engine,
            ipc::Command::PlaybackLegend {
                id,
                legend: "Pulpit".into(),
                icon: None,
                halo: None,
            },
        );
        assert_eq!(engine.playback(id).unwrap().legend, "Pulpit");
        run(
            &mut engine,
            ipc::Command::PlaybackCopy {
                id,
                page: 3,
                slot: 2,
            },
        );
        run(
            &mut engine,
            ipc::Command::PlaybackMove {
                id,
                page: 3,
                slot: 3,
            },
        );
        assert_eq!(engine.playback(id).unwrap().slot, 3);
        run(&mut engine, ipc::Command::PlaybackFlash { id, on: true });
        assert!(engine.playback_state(id).unwrap().flash);
        run(&mut engine, ipc::Command::PlaybackFlash { id, on: false });
        run(&mut engine, ipc::Command::PlaybackSwop { id, on: true });
        assert!(engine.playback_state(id).unwrap().swop);
        run(&mut engine, ipc::Command::PlaybackSwop { id, on: false });
        run(&mut engine, ipc::Command::PlaybackLatch { id });
        assert!(engine.playback_state(id).unwrap().latched);
        engine.tick(0.0);
        run(&mut engine, ipc::Command::PlaybackRelease { id });
        engine.tick(0.025);
        assert!(!engine.playback_state(id).unwrap().is_active());
        run(
            &mut engine,
            ipc::Command::PageLegend {
                page: 3,
                legend: "Service".into(),
            },
        );
        assert_eq!(engine.page_legend(3), Some("Service"));
        run(&mut engine, ipc::Command::DboSet { on: true });
        assert!(engine.dbo());
        run(
            &mut engine,
            ipc::Command::SettingsPlayback(cmdmx_core::engine::PlaybackSettingsChange {
                fade_in: Some(1.5),
                ..Default::default()
            }),
        );
        assert_eq!(engine.settings().cue_fade_in, 1.5);
        run(&mut engine, ipc::Command::PlaybackDelete { id });
        assert!(engine.playback(id).is_none());
    }

    #[test]
    fn pbk_36_refusals_change_nothing() {
        use cmdmx_core::playback::{PlaybackChange, PlaybackRecord};
        let mut engine = demo_engine().unwrap();
        let before = engine.to_show(cmdmx_core::show::Meta {
            name: "Demo".into(),
            created: String::new(),
            modified: String::new(),
            app_version: String::new(),
        });
        for cmd in [
            ipc::Command::PlaybackRecord(PlaybackRecord::at(1, 1)),
            ipc::Command::PlaybackRecord(PlaybackRecord::at(3, 1)),
            ipc::Command::PlaybackCopy {
                id: 1,
                page: 1,
                slot: 2,
            },
            ipc::Command::PlaybackDelete { id: 99 },
            ipc::Command::PlaybackFlash { id: 99, on: true },
            ipc::Command::PlaybackChange {
                id: 1,
                change: PlaybackChange {
                    fade_in: Some(700.0),
                    ..PlaybackChange::default()
                },
            },
            ipc::Command::PageLegend {
                page: 11,
                legend: "x".into(),
            },
        ] {
            assert!(apply(&mut engine, cmd.clone(), 0.0).is_err(), "{cmd:?}");
        }
        assert_eq!(engine.to_show(before.meta.clone()), before);
    }

    /// VS3-05: one parsed JSON command applied to the engine.
    fn send(engine: &mut Engine, json: &str) -> Result<(), CoreError> {
        apply(engine, ipc::parse_command(json).unwrap(), 0.0)
    }

    fn at(engine: &Engine, page: u16, slot: u16) -> u32 {
        engine
            .playbacks()
            .find(|p| (p.page, p.slot) == (page, slot))
            .unwrap()
            .id
    }

    fn numbers(engine: &Engine, id: u32) -> Vec<f64> {
        let p = engine.playback(id).unwrap();
        p.cues.iter().map(|c| c.number.as_f64()).collect()
    }

    #[test]
    fn cue_41_record_and_cue_commands_reach_the_engine() {
        use cmdmx_core::playback::{Link, Track};
        let mut engine = demo_engine().unwrap();
        let ok = |engine: &mut Engine, json: &str| send(engine, json).unwrap();
        engine
            .execute_command(&["201", "@", "100", "Enter"])
            .unwrap();
        ok(
            &mut engine,
            r#"{"v":0,"cmd":"playback.record","page":3,"slot":1}"#,
        );
        let id = at(&engine, 3, 1);
        ok(
            &mut engine,
            r#"{"v":0,"cmd":"playback.record","page":3,"slot":1,"mode":"Append"}"#,
        );
        ok(
            &mut engine,
            r#"{"v":0,"cmd":"playback.record","page":3,"slot":1,"mode":"Insert"}"#,
        );
        ok(
            &mut engine,
            r#"{"v":0,"cmd":"playback.record","page":3,"slot":1,"cue":5}"#,
        );
        assert_eq!(numbers(&engine, id), vec![1.0, 2.0, 3.0, 5.0]);
        ok(
            &mut engine,
            r#"{"v":0,"cmd":"playback.record","page":3,"slot":1,"cue":2,"mode":"Merge",
                "update_mode":"CueOnly"}"#,
        );
        let s = format!(r#""id":{id},"cue":2"#);
        ok(
            &mut engine,
            &format!(r#"{{"v":0,"cmd":"cue.legend",{s},"legend":"Welcome"}}"#),
        );
        ok(
            &mut engine,
            &format!(
                r#"{{"v":0,"cmd":"cue.times",{s},"fade_in":3,"link":{{"type":"After","offset":1}},
                    "group_times":{{"P":{{"delay":1,"fade":5}}}}}}"#
            ),
        );
        ok(
            &mut engine,
            &format!(r#"{{"v":0,"cmd":"cue.track",{s},"track":"Block"}}"#),
        );
        let q2 = engine.playback(id).unwrap().cues[1].clone();
        assert_eq!(q2.legend, "Welcome");
        assert_eq!(q2.fade_in, 3.0);
        assert_eq!(q2.link, Link::After { offset: 1.0 });
        assert_eq!(q2.track, Track::Block);
        assert_eq!(q2.group_times.len(), 1);
        ok(
            &mut engine,
            &format!(r#"{{"v":0,"cmd":"cue.copy",{s},"to":7}}"#),
        );
        ok(
            &mut engine,
            &format!(r#"{{"v":0,"cmd":"cue.renumber","id":{id},"cue":7,"to":8}}"#),
        );
        ok(
            &mut engine,
            &format!(r#"{{"v":0,"cmd":"cue.delete","id":{id},"cue":8}}"#),
        );
        ok(
            &mut engine,
            &format!(r#"{{"v":0,"cmd":"cue.renumber_all","id":{id}}}"#),
        );
        assert_eq!(numbers(&engine, id), vec![1.0, 2.0, 3.0, 4.0]);
        ok(
            &mut engine,
            &format!(r#"{{"v":0,"cmd":"cue.remove","id":{id},"cue":1,"fixture":201}}"#),
        );
        assert!(engine.playback(id).unwrap().cues[0].values.is_empty());
    }

    #[test]
    fn cue_41_running_commands_reach_the_engine() {
        let mut engine = demo_engine().unwrap();
        engine
            .execute_command(&["201", "@", "100", "Enter"])
            .unwrap();
        send(
            &mut engine,
            r#"{"v":0,"cmd":"playback.record","page":3,"slot":1}"#,
        )
        .unwrap();
        send(
            &mut engine,
            r#"{"v":0,"cmd":"playback.record","page":3,"slot":1,"mode":"Append"}"#,
        )
        .unwrap();
        send(
            &mut engine,
            r#"{"v":0,"cmd":"playback.record","page":3,"slot":1,"mode":"Append"}"#,
        )
        .unwrap();
        let id = at(&engine, 3, 1);
        send(
            &mut engine,
            &format!(r#"{{"v":0,"cmd":"cuelist.connect","id":{id}}}"#),
        )
        .unwrap();
        assert_eq!(engine.connected(), Some(id));
        send(&mut engine, r#"{"v":0,"cmd":"cuelist.go"}"#).unwrap();
        engine.tick(0.0);
        let cur = |e: &Engine| e.list_live(id).unwrap().current.map(|c| c.as_f64());
        assert_eq!(cur(&engine), Some(1.0));
        send(&mut engine, r#"{"v":0,"cmd":"cuelist.next","cue":3}"#).unwrap();
        assert_eq!(
            engine.list_live(id).unwrap().next.map(|c| c.as_f64()),
            Some(3.0)
        );
        send(
            &mut engine,
            &format!(r#"{{"v":0,"cmd":"playback.button","id":{id},"button":"upper","on":true}}"#),
        )
        .unwrap();
        engine.tick(0.025);
        assert_eq!(
            cur(&engine),
            Some(3.0),
            "the upper button is Go on a cue list"
        );
        send(&mut engine, r#"{"v":0,"cmd":"cuelist.back"}"#).unwrap();
        engine.tick(0.05);
        assert_eq!(cur(&engine), Some(2.0));
        send(
            &mut engine,
            &format!(r#"{{"v":0,"cmd":"cuelist.goto","id":{id},"cue":1}}"#),
        )
        .unwrap();
        engine.tick(0.075);
        assert_eq!(cur(&engine), Some(1.0));
        send(&mut engine, r#"{"v":0,"cmd":"playback.release"}"#).unwrap();
        engine.tick(0.1);
        assert!(!engine.playback_state(id).unwrap().is_active());
        assert!(send(&mut engine, r#"{"v":0,"cmd":"cuelist.goto","cue":9}"#).is_err());
    }

    #[test]
    fn cue_41_options_masters_tempo_and_settings_reach_the_engine() {
        use cmdmx_core::playback::{
            Direction, HandleButtons, Kind, ListFader, LoopAction, Master, UpdateMode,
        };
        let mut engine = demo_engine().unwrap();
        engine
            .execute_command(&["201", "@", "100", "Enter"])
            .unwrap();
        send(
            &mut engine,
            r#"{"v":0,"cmd":"playback.record","page":3,"slot":1}"#,
        )
        .unwrap();
        send(
            &mut engine,
            r#"{"v":0,"cmd":"playback.record","page":3,"slot":1,"mode":"Append"}"#,
        )
        .unwrap();
        let id = at(&engine, 3, 1);
        send(
            &mut engine,
            &format!(r#"{{"v":0,"cmd":"playback.options","id":{id},"list":{{"loop":"Loop"}}}}"#),
        )
        .unwrap();
        let p = engine.playback(id).unwrap();
        assert_eq!(p.list.looping, LoopAction::Loop);
        assert!(p.list.tracking, "fields left out keep their value (IPC-27)");
        send(
            &mut engine,
            &format!(r#"{{"v":0,"cmd":"playback.options","id":{id},"kind":"Chase","chase":{{"bpm":90,"direction":"Bounce"}}}}"#),
        )
        .unwrap();
        let p = engine.playback(id).unwrap();
        assert_eq!(p.kind, Kind::Chase);
        assert_eq!(p.chase.bpm, 90.0);
        assert_eq!(p.chase.direction, Direction::Bounce);
        assert_eq!(p.chase.crossfade, 0.0);
        assert!(send(
            &mut engine,
            &format!(r#"{{"v":0,"cmd":"playback.options","id":{id},"chase":{{"bpm":900}}}}"#)
        )
        .is_err());
        assert_eq!(engine.playback(id).unwrap().chase.bpm, 90.0);
        send(
            &mut engine,
            &format!(r#"{{"v":0,"cmd":"playback.bpm","id":{id},"bpm":100}}"#),
        )
        .unwrap();
        assert_eq!(engine.playback(id).unwrap().chase.bpm, 100.0);

        // IPC-26: no number → the lowest free one.
        send(
            &mut engine,
            r#"{"v":0,"cmd":"master.record","page":3,"slot":4,"kind":"Bpm"}"#,
        )
        .unwrap();
        send(
            &mut engine,
            r#"{"v":0,"cmd":"master.record","page":3,"slot":5,"kind":"Bpm"}"#,
        )
        .unwrap();
        send(
            &mut engine,
            r#"{"v":0,"cmd":"master.record","page":3,"slot":6,"kind":"Rate","number":3}"#,
        )
        .unwrap();
        let number = |e: &Engine, slot| {
            e.playback(at(e, 3, slot))
                .unwrap()
                .master
                .as_ref()
                .unwrap()
                .number()
        };
        assert_eq!(
            (number(&engine, 4), number(&engine, 5), number(&engine, 6)),
            (1, 2, 3)
        );
        let m = at(&engine, 3, 4);
        send(
            &mut engine,
            &format!(r#"{{"v":0,"cmd":"playback.options","id":{m},"master":{{"multiplier":2}}}}"#),
        )
        .unwrap();
        let Some(Master::Bpm(b)) = engine.playback(m).unwrap().master.clone() else {
            panic!()
        };
        assert_eq!((b.multiplier, b.bpm), (2.0, 120.0));

        // IPC-25: taps carry their receive time.
        for t in [0.0, 0.4] {
            let tap =
                ipc::parse_command(&format!(r#"{{"v":0,"cmd":"playback.tap","id":{m}}}"#)).unwrap();
            apply(&mut engine, ipc::stamp(tap, t), 9.0).unwrap();
        }
        let Some(Master::Bpm(b)) = engine.playback(m).unwrap().master.clone() else {
            panic!()
        };
        assert_eq!(b.bpm, 150.0, "0.4 s apart");

        send(
            &mut engine,
            r#"{"v":0,"cmd":"settings.playback","auto_connect":false,"cue_list_fader":"KillAtZero",
                "update_mode":"Both","handle_buttons":"Titan"}"#,
        )
        .unwrap();
        let s = engine.settings();
        assert!(!s.auto_connect);
        assert_eq!(s.cue_list_fader, ListFader::KillAtZero);
        assert_eq!(s.update_mode, UpdateMode::Both);
        assert_eq!(s.handle_buttons, HandleButtons::Titan);
    }

    #[test]
    fn grm_15_group_master_commands_reach_the_engine() {
        use cmdmx_core::playback::{GroupMode, Kind, Master, MasterKind};
        let mut engine = demo_engine().unwrap();
        let group = |e: &Engine, id| match e.playback(id).unwrap().master.clone() {
            Some(Master::Group(m)) => m,
            other => panic!("{other:?}"),
        };

        // GRM-01: the demo's group 4 holds the front spots; several masters per group.
        send(
            &mut engine,
            r#"{"v":0,"cmd":"master.record","page":3,"slot":8,"kind":"Group","group":4}"#,
        )
        .unwrap();
        send(
            &mut engine,
            r#"{"v":0,"cmd":"master.record","page":3,"slot":7,"kind":"Group","group":4}"#,
        )
        .unwrap();
        let id = at(&engine, 3, 8);
        assert_eq!(engine.playback(id).unwrap().kind, Kind::GroupMaster);
        let m = group(&engine, id);
        assert_eq!(
            (m.group, m.mode, m.scale, m.enabled),
            (4, GroupMode::Scale, 100, true)
        );

        // IPC-27: each field optional, the rest kept.
        let options = |e: &mut Engine, master: &str| {
            send(
                e,
                &format!(r#"{{"v":0,"cmd":"playback.options","id":{id},"master":{master}}}"#),
            )
        };
        options(&mut engine, r#"{"mode":"Limit"}"#).unwrap();
        options(&mut engine, r#"{"scale":200}"#).unwrap();
        let m = group(&engine, id);
        assert_eq!((m.mode, m.scale, m.enabled), (GroupMode::Limit, 200, true));

        // GRM-05, 13: an enabled master's fader is its saved level.
        send(
            &mut engine,
            &format!(r#"{{"v":0,"cmd":"fader.set","playback":{id},"level":0.4}}"#),
        )
        .unwrap();
        assert_eq!(group(&engine, id).level, 0.4);

        // GRM-11: disabled, the fader no longer moves the saved level.
        options(&mut engine, r#"{"enabled":false}"#).unwrap();
        send(
            &mut engine,
            &format!(r#"{{"v":0,"cmd":"fader.set","playback":{id},"level":0.9}}"#),
        )
        .unwrap();
        let m = group(&engine, id);
        assert_eq!((m.enabled, m.level, m.mode), (false, 0.4, GroupMode::Limit));

        // GRM-19, IPC-29: refused, nothing changes.
        let bpm = engine.record_master(3, 6, MasterKind::Bpm, 1).unwrap();
        let meta = cmdmx_core::show::Meta {
            name: "Demo".into(),
            created: String::new(),
            modified: String::new(),
            app_version: String::new(),
        };
        let before = engine.to_show(meta.clone());
        for bad in [
            r#"{"v":0,"cmd":"master.record","page":3,"slot":5,"kind":"Group","group":99}"#,
            r#"{"v":0,"cmd":"master.record","page":3,"slot":8,"kind":"Group","group":4}"#,
            r#"{"v":0,"cmd":"master.record","page":1,"slot":1,"kind":"Group","group":4}"#,
        ] {
            assert!(send(&mut engine, bad).is_err(), "{bad}");
        }
        for bad in [r#"{"scale":300}"#, r#"{"multiplier":2}"#, r#"{"number":2}"#] {
            assert!(options(&mut engine, bad).is_err(), "{bad}");
        }
        for bad in [
            r#"{"mode":"Htp"}"#,
            r#"{"scale":200}"#,
            r#"{"enabled":false}"#,
        ] {
            let r = send(
                &mut engine,
                &format!(r#"{{"v":0,"cmd":"playback.options","id":{bpm},"master":{bad}}}"#),
            );
            assert!(r.is_err(), "{bad} on a BPM master");
        }
        assert_eq!(engine.to_show(meta), before);
    }

    // ---- undo (VS5-02: UNDO-04, 13..15, IPC-30/31) ----------------------------

    /// UNDO-04: one sample per command name; `class` is the table under test.
    const CLASSES: &[(&str, Class)] = &[
        (
            r#"{"v":0,"cmd":"fader.set","playback":2,"level":0.5}"#,
            Class::Live,
        ),
        (
            r#"{"v":0,"cmd":"grand_master.set","level":0.5}"#,
            Class::Live,
        ),
        (r#"{"v":0,"cmd":"dbo.set","on":true}"#, Class::Live),
        (r#"{"v":0,"cmd":"page.set","page":2}"#, Class::Live),
        (
            r#"{"v":0,"cmd":"playback.button","id":2,"button":"upper","on":true}"#,
            Class::Live,
        ),
        (
            r#"{"v":0,"cmd":"playback.flash","id":3,"on":true}"#,
            Class::Live,
        ),
        (
            r#"{"v":0,"cmd":"playback.swop","id":3,"on":false}"#,
            Class::Live,
        ),
        (r#"{"v":0,"cmd":"playback.latch","id":3}"#, Class::Live),
        (r#"{"v":0,"cmd":"playback.release","id":3}"#, Class::Live),
        (r#"{"v":0,"cmd":"playback.tap","id":3}"#, Class::Live),
        (
            r#"{"v":0,"cmd":"playback.bpm","id":3,"bpm":128}"#,
            Class::Live,
        ),
        (r#"{"v":0,"cmd":"cuelist.go"}"#, Class::Live),
        (r#"{"v":0,"cmd":"cuelist.back"}"#, Class::Live),
        (r#"{"v":0,"cmd":"cuelist.stop","id":2}"#, Class::Live),
        (r#"{"v":0,"cmd":"cuelist.goto","cue":2.5}"#, Class::Live),
        (
            r#"{"v":0,"cmd":"cuelist.next","id":2,"cue":3}"#,
            Class::Live,
        ),
        (r#"{"v":0,"cmd":"cuelist.connect","id":2}"#, Class::Live),
        (
            r#"{"v":0,"cmd":"programmer.attribute_group","group":"P"}"#,
            Class::Live,
        ),
        (
            r#"{"v":0,"cmd":"programmer.colour_model","model":"Rgb"}"#,
            Class::Live,
        ),
        (r#"{"v":0,"cmd":"show.new","name":"Kerst"}"#, Class::Reset),
        (r#"{"v":0,"cmd":"show.open","name":"Kerk"}"#, Class::Reset),
        (r#"{"v":0,"cmd":"show.list"}"#, Class::Neutral),
        (
            r#"{"v":0,"cmd":"output.add","line":2,"kind":"sacn"}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"output.edit","id":1,"enabled":false}"#,
            Class::Edit,
        ),
        (r#"{"v":0,"cmd":"output.remove","ids":[1]}"#, Class::Edit),
        (r#"{"v":0,"cmd":"output.ports"}"#, Class::Neutral),
        (
            r#"{"v":0,"cmd":"show.save_as","name":"Kerk 2"}"#,
            Class::Neutral,
        ),
        (r#"{"v":0,"cmd":"cue.view","id":2,"cue":2}"#, Class::Neutral),
        (
            r#"{"v":0,"cmd":"patch.profile","profile":"Showtec/Phantom 25 LED Spot"}"#,
            Class::Neutral,
        ),
        (
            r#"{"v":0,"cmd":"profile.save","profile":{"manufacturer":"A","model":"B","modes":[{"name":"m","footprint":1}]}}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"programmer.set","attribute":"Pan","value":0.5}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"library.search","query":"par"}"#,
            Class::Neutral,
        ),
        (
            r#"{"v":0,"cmd":"library.manufacturers","query":"par"}"#,
            Class::Neutral,
        ),
        (
            r#"{"v":0,"cmd":"library.models","manufacturer":"Showtec"}"#,
            Class::Neutral,
        ),
        (
            r#"{"v":0,"cmd":"programmer.select","fixture":201}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"programmer.select_group","group":3}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"programmer.step","direction":-1}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"programmer.pattern","stages":2}"#,
            Class::Edit,
        ),
        (r#"{"v":0,"cmd":"programmer.all"}"#, Class::Edit),
        (r#"{"v":0,"cmd":"programmer.clear"}"#, Class::Edit),
        (
            r#"{"v":0,"cmd":"programmer.clear","groups":["C"],"time":3.0}"#,
            Class::Edit,
        ),
        (r#"{"v":0,"cmd":"programmer.locate"}"#, Class::Edit),
        (
            r#"{"v":0,"cmd":"programmer.highlight","on":true}"#,
            Class::Live,
        ),
        (
            r#"{"v":0,"cmd":"programmer.off","groups":["C"]}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"programmer.off","attribute":"Hue"}"#,
            Class::Edit,
        ),
        (r#"{"v":0,"cmd":"programmer.rem_dim"}"#, Class::Edit),
        (
            r#"{"v":0,"cmd":"settings.programmer","clear_ltp":"Freeze"}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"programmer.wheel","attribute":"Pan","steps":-3}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"programmer.colour","hue":0.66,"saturation":1.0}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"programmer.command","tokens":["201","Enter"]}"#,
            Class::Edit,
        ),
        (r#"{"v":0,"cmd":"group.record","group":3}"#, Class::Edit),
        (
            r#"{"v":0,"cmd":"patch.add","profile":"rgbw","mode":"default","count":2,"universe":1}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"patch.move","id":3,"universe":2,"address":101}"#,
            Class::Edit,
        ),
        (r#"{"v":0,"cmd":"patch.remove","ids":[3,4]}"#, Class::Edit),
        (
            r#"{"v":0,"cmd":"patch.renumber","id":3,"number":1201}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"patch.name","id":3,"name":"Front left"}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"patch.change_mode","id":3,"mode":"Big"}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"patch.handle","id":3,"page":2,"slot":5}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"palette.record","window":"Colours","handle":4}"#,
            Class::Edit,
        ),
        (r#"{"v":0,"cmd":"palette.apply","number":3}"#, Class::Edit),
        (r#"{"v":0,"cmd":"palette.include","number":3}"#, Class::Edit),
        (
            r#"{"v":0,"cmd":"palette.copy","number":3,"to":{"window":"Colours","handle":5}}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"palette.move","number":3,"to":{"window":"Colours","handle":5}}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"palette.renumber","number":3,"to":30}"#,
            Class::Edit,
        ),
        (r#"{"v":0,"cmd":"palette.delete","number":3}"#, Class::Edit),
        (
            r#"{"v":0,"cmd":"palette.legend","number":3,"legend":"Warm","icon":"colour.drop"}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"settings.palette","time":2.0}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"settings.playback","record_by":"Mixed"}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"playback.record","page":1,"slot":2}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"playback.options","id":3,"release_mask":null,"release_time":null,"group":null}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"playback.times","id":3,"delay":1.0,"fade_in":3.0,"fade_out":4.0,"overlap":50}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"playback.legend","id":3,"legend":"x","icon":null,"halo":null}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"playback.copy","id":3,"to":{"page":2,"slot":4}}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"playback.move","id":3,"to":{"page":2,"slot":4}}"#,
            Class::Edit,
        ),
        (r#"{"v":0,"cmd":"playback.delete","id":3}"#, Class::Edit),
        (
            r#"{"v":0,"cmd":"page.legend","page":2,"legend":"Service"}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"cue.times","id":2,"cue":2,"fade_in":3}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"cue.legend","id":2,"cue":2,"legend":"Welcome"}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"cue.track","id":2,"cue":2,"track":"CueOnly"}"#,
            Class::Edit,
        ),
        (r#"{"v":0,"cmd":"cue.delete","id":2,"cue":2}"#, Class::Edit),
        (
            r#"{"v":0,"cmd":"cue.renumber","id":2,"cue":2,"to":2.5}"#,
            Class::Edit,
        ),
        (r#"{"v":0,"cmd":"cue.renumber_all","id":2}"#, Class::Edit),
        (
            r#"{"v":0,"cmd":"cue.copy","id":2,"cue":2,"to":7}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"cue.remove","id":2,"cue":2,"fixture":201}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"master.record","page":1,"slot":4,"kind":"Bpm"}"#,
            Class::Edit,
        ),
        (
            r#"{"v":0,"cmd":"control.mapping","name":"nano"}"#,
            Class::Neutral,
        ),
    ];

    #[test]
    fn undo_04_every_command_has_its_class() {
        for (json, expected) in CLASSES {
            let cmd = ipc::parse_command(json).unwrap_or_else(|e| panic!("{json}: {e}"));
            assert_eq!(class(&cmd), *expected, "{json}");
        }
    }

    /// UNDO-04: a command name `parse_command` knows but this table does not fails here.
    #[test]
    fn undo_04_a_command_without_a_class_fails() {
        let src = include_str!("ipc.rs");
        let body = src
            .split("pub fn parse_command")
            .nth(1)
            .and_then(|s| s.split("\n}\n").next())
            .unwrap();
        let known: Vec<&str> = CLASSES
            .iter()
            .filter_map(|(j, _)| j.split("\"cmd\":\"").nth(1)?.split('"').next())
            .chain(["undo", "redo"])
            .collect();
        for part in body.split("Some(\"").skip(1) {
            let name = part.split('"').next().unwrap();
            if name.contains('.') && !name.starts_with("library.")
                || name == "undo"
                || name == "redo"
            {
                assert!(known.contains(&name), "{name} has no class (UNDO-04)");
            }
        }
        // `Some("playback.options" | "playback.times")`: the names after the first.
        for arm in body.split("Some(\"").skip(1) {
            let arm = arm.split(')').next().unwrap();
            for name in arm.split(" | \"").skip(1) {
                let name = name.trim_end_matches('"');
                assert!(known.contains(&name), "{name} has no class (UNDO-04)");
            }
        }
    }

    fn undo_run(engine: &mut Engine, history: &mut History, json: &str) -> Result<(), CoreError> {
        run(engine, history, ipc::parse_command(json).unwrap(), 0.0)
    }

    const SELECT_101: &str =
        r#"{"v":0,"cmd":"programmer.command","tokens":["101","@","50","Enter"]}"#;

    #[test]
    fn undo_01_ipc_30_an_edit_is_undone_and_redone() {
        let mut e = demo_engine().unwrap();
        let mut h = History::default();
        let before = e.edit_state();
        undo_run(&mut e, &mut h, SELECT_101).unwrap();
        let after = e.edit_state();
        assert_eq!(h.undo_label(), Some("101 @ 50 Enter"));
        undo_run(&mut e, &mut h, r#"{"v":0,"cmd":"undo"}"#).unwrap();
        assert_eq!(e.edit_state(), before);
        assert_eq!(
            (h.undo_label(), h.redo_label()),
            (None, Some("101 @ 50 Enter"))
        );
        undo_run(&mut e, &mut h, r#"{"v":0,"cmd":"redo"}"#).unwrap();
        assert_eq!(e.edit_state(), after);
    }

    #[test]
    fn ipc_30_refused_undo_and_redo_are_errors() {
        let mut e = demo_engine().unwrap();
        let mut h = History::default();
        let undo = undo_run(&mut e, &mut h, r#"{"v":0,"cmd":"undo"}"#).unwrap_err();
        assert_eq!(undo.to_string(), "Nothing to undo");
        let redo = undo_run(&mut e, &mut h, r#"{"v":0,"cmd":"redo"}"#).unwrap_err();
        assert_eq!(redo.to_string(), "Nothing to redo");
    }

    #[test]
    fn undo_01_refused_or_unchanged_commands_add_no_step() {
        let mut e = demo_engine().unwrap();
        let mut h = History::default();
        undo_run(&mut e, &mut h, r#"{"v":0,"cmd":"programmer.clear"}"#).unwrap();
        assert!(undo_run(&mut e, &mut h, r#"{"v":0,"cmd":"playback.delete","id":99}"#).is_err());
        assert!(h.is_empty());
    }

    #[test]
    fn undo_04_live_commands_are_not_in_the_history() {
        let mut e = demo_engine().unwrap();
        let mut h = History::default();
        undo_run(
            &mut e,
            &mut h,
            r#"{"v":0,"cmd":"fader.set","playback":1,"level":0.5}"#,
        )
        .unwrap();
        undo_run(
            &mut e,
            &mut h,
            r#"{"v":0,"cmd":"programmer.attribute_group","group":"P"}"#,
        )
        .unwrap();
        assert!(h.is_empty());
    }

    #[test]
    fn undo_05_wheel_turns_coalesce_until_another_command() {
        let mut e = demo_engine().unwrap();
        let mut h = History::default();
        undo_run(&mut e, &mut h, SELECT_101).unwrap();
        let wheel = r#"{"v":0,"cmd":"programmer.wheel","attribute":"Pan","steps":3}"#;
        undo_run(&mut e, &mut h, wheel).unwrap();
        undo_run(&mut e, &mut h, wheel).unwrap();
        assert_eq!((h.len(), h.undo_label()), (2, Some("Wheel Pan")));
        undo_run(
            &mut e,
            &mut h,
            r#"{"v":0,"cmd":"fader.set","playback":1,"level":0.5}"#,
        )
        .unwrap();
        undo_run(&mut e, &mut h, wheel).unwrap();
        assert_eq!(h.len(), 3, "a live command ends the run");
    }

    #[test]
    fn undo_01_labels_name_the_command() {
        let mut e = demo_engine().unwrap();
        let mut h = History::default();
        undo_run(&mut e, &mut h, SELECT_101).unwrap();
        undo_run(&mut e, &mut h, r#"{"v":0,"cmd":"programmer.clear"}"#).unwrap();
        assert_eq!(h.undo_label(), Some("Clear"));
        undo_run(
            &mut e,
            &mut h,
            r#"{"v":0,"cmd":"programmer.select","fixture":101}"#,
        )
        .unwrap();
        assert_eq!(h.undo_label(), Some("Select"));
        let id = e.playbacks().next().unwrap().id;
        let delete = format!(r#"{{"v":0,"cmd":"playback.delete","id":{id}}}"#);
        undo_run(&mut e, &mut h, &delete).unwrap();
        assert_eq!(
            h.undo_label(),
            Some(format!("Delete playback {id}").as_str())
        );
    }

    // ---- VS2-24: IPC-38, IPC-39 ------------------------------------------------------------

    const SAVE_SPOT: &str = r#"{"v":0,"cmd":"profile.save","profile":{"manufacturer":"Showtec",
        "model":"Phantom 25 LED Spot","source":"user","modes":[{"name":"11 Channel","footprint":11,
        "attributes":[{"name":"Pan","group":"P","kind":"Ltp","channel":1,"default":0.5}]}]}}"#;

    #[test]
    fn ipc_38_profile_save_is_one_undo_step_named_after_the_model() {
        let mut e = demo_engine().unwrap();
        let mut h = History::default();
        undo_run(&mut e, &mut h, SAVE_SPOT).unwrap();
        assert_eq!(
            (h.len(), h.undo_label()),
            (1, Some("Edit profile Phantom 25 LED Spot"))
        );
        assert!(e.has_profile("Showtec/Phantom 25 LED Spot"));
        undo_run(&mut e, &mut h, SAVE_SPOT).unwrap();
        assert_eq!(h.len(), 1, "an unchanged save is no step (UNDO-01)");
        let other = SAVE_SPOT.replace(r#""default":0.5"#, r#""default":0.25"#);
        undo_run(&mut e, &mut h, &other).unwrap();
        assert_eq!(h.len(), 2, "two different saves are two steps");
        undo_run(&mut e, &mut h, r#"{"v":0,"cmd":"undo"}"#).unwrap();
        undo_run(&mut e, &mut h, r#"{"v":0,"cmd":"undo"}"#).unwrap();
        assert!(!e.has_profile("Showtec/Phantom 25 LED Spot"));
    }

    #[test]
    fn ipc_38_a_refused_save_adds_no_step() {
        let mut e = demo_engine().unwrap();
        let mut h = History::default();
        let bad = SAVE_SPOT.replace(r#""default":0.5"#, r#""default":2.0"#);
        assert!(undo_run(&mut e, &mut h, &bad).is_err());
        assert!(h.is_empty());
    }

    #[test]
    fn ipc_39_programmer_set_coalesces_per_attribute_like_a_wheel() {
        let mut e = demo_engine().unwrap();
        let mut h = History::default();
        undo_run(&mut e, &mut h, SELECT_101).unwrap();
        let set =
            |v: f32| format!(r#"{{"v":0,"cmd":"programmer.set","attribute":"Pan","value":{v}}}"#);
        undo_run(&mut e, &mut h, &set(0.2)).unwrap();
        undo_run(&mut e, &mut h, &set(0.4)).unwrap();
        assert_eq!((h.len(), h.undo_label()), (2, Some("Set Pan")));
        assert_eq!(
            e.programmer().value(101, "Pan"),
            Some(cmdmx_core::value::Value::Level(0.4))
        );
        undo_run(&mut e, &mut h, r#"{"v":0,"cmd":"undo"}"#).unwrap();
        assert_eq!(
            e.programmer().value(101, "Pan"),
            None,
            "one undo takes the run back"
        );
        assert!(undo_run(&mut e, &mut h, &set(1.5)).is_err());
    }

    // ---- VS6-02: OUT-34 ----------------------------------------------------------------

    fn node(e: &Engine, id: u32) -> Option<cmdmx_core::output::Node> {
        e.nodes().find(|n| n.id == id).cloned()
    }

    #[test]
    fn out_34_output_commands_reach_the_engine_through_the_history() {
        use cmdmx_core::output::NodeKind;
        let mut e = Engine::new(Settings::default());
        let mut h = History::default();
        let before = e.edit_state();
        let first: Vec<u32> = e.nodes().map(|n| n.id).collect();
        undo_run(
            &mut e,
            &mut h,
            r#"{"v":0,"cmd":"output.add","line":2,"kind":"sacn","priority":120}"#,
        )
        .unwrap();
        assert_eq!(h.undo_label(), Some("Add output"));
        let added = e
            .nodes()
            .map(|n| n.id)
            .find(|id| !first.contains(id))
            .unwrap();
        assert_eq!(
            node(&e, added).unwrap().kind,
            NodeKind::Sacn {
                universe: 2,
                priority: 120,
                unicast: None
            }
        );

        // Partial: only the named fields change.
        let edit = format!(r#"{{"v":0,"cmd":"output.edit","id":{added},"unicast":"10.0.0.5"}}"#);
        undo_run(&mut e, &mut h, &edit).unwrap();
        assert_eq!(h.undo_label(), Some("Edit output"));
        let edited = node(&e, added).unwrap();
        assert_eq!(
            edited.kind,
            NodeKind::Sacn {
                universe: 2,
                priority: 120,
                unicast: Some("10.0.0.5".into())
            }
        );
        assert_eq!((edited.line, edited.enabled), (2, true));

        let remove = format!(r#"{{"v":0,"cmd":"output.remove","ids":[{added}]}}"#);
        undo_run(&mut e, &mut h, &remove).unwrap();
        assert_eq!(h.undo_label(), Some("Remove output"));
        assert_eq!(node(&e, added), None);

        for _ in 0..3 {
            undo_run(&mut e, &mut h, r#"{"v":0,"cmd":"undo"}"#).unwrap();
        }
        assert_eq!(e.edit_state(), before, "all three undone");
    }

    #[test]
    fn out_34_refused_output_edits_change_nothing() {
        let mut e = Engine::new(Settings::default());
        let mut h = History::default();
        let artnet = e
            .nodes()
            .find(|n| matches!(n.kind, cmdmx_core::output::NodeKind::Artnet { .. }))
            .unwrap()
            .id;
        let before = e.edit_state();
        for bad in [
            format!(r#"{{"v":0,"cmd":"output.edit","id":{artnet},"kind":"sacn"}}"#),
            format!(r#"{{"v":0,"cmd":"output.edit","id":{artnet},"port_address":40000}}"#),
            format!(r#"{{"v":0,"cmd":"output.edit","id":{artnet},"target":"nope"}}"#),
            format!(r#"{{"v":0,"cmd":"output.edit","id":{artnet},"line":"two"}}"#),
            r#"{"v":0,"cmd":"output.edit","id":999,"enabled":false}"#.to_string(),
            r#"{"v":0,"cmd":"output.remove","ids":[999]}"#.to_string(),
            // OUT-28: line 1 already goes out on Art-Net 0:0:0 at broadcast.
            r#"{"v":0,"cmd":"output.add","line":1,"kind":"artnet"}"#.to_string(),
        ] {
            assert!(undo_run(&mut e, &mut h, &bad).is_err(), "{bad}");
        }
        assert_eq!(e.edit_state(), before);
        assert!(h.is_empty());
    }

    #[test]
    fn out_28_priority_300_refused_with_range_text() {
        use cmdmx_core::output::NodeKind;
        let mut e = Engine::new(Settings::default());
        let mut h = History::default();
        let id_of = |e: &Engine, artnet: bool| {
            e.nodes()
                .find(|n| matches!(n.kind, NodeKind::Artnet { .. }) == artnet)
                .unwrap()
                .id
        };
        let (artnet, sacn) = (id_of(&e, true), id_of(&e, false));
        let before = e.edit_state();
        for (id, field, text) in [
            (
                sacn,
                r#""priority":300"#,
                "sACN priority 300 is out of range (0 to 200)",
            ),
            (
                sacn,
                r#""priority":-1"#,
                "sACN priority -1 is out of range (0 to 200)",
            ),
            (
                sacn,
                r#""universe":70000"#,
                "sACN universe 70000 is out of range (1 to 63999)",
            ),
            (
                artnet,
                r#""port_address":70000"#,
                "Art-Net port-address 70000 is out of range (0 to 32767)",
            ),
            (
                artnet,
                r#""port_address":-1"#,
                "Art-Net port-address -1 is out of range (0 to 32767)",
            ),
            (
                artnet,
                r#""line":70000"#,
                "universe 70000 is out of range (1 to 65535)",
            ),
            (
                artnet,
                r#""line":-1"#,
                "universe -1 does not exist (universes start at 1)",
            ),
        ] {
            let edit = format!(r#"{{"v":0,"cmd":"output.edit","id":{id},{field}}}"#);
            let refusal = undo_run(&mut e, &mut h, &edit).unwrap_err();
            assert_eq!(refusal.to_string(), text, "{edit}");
        }
        assert_eq!(e.edit_state(), before);
        assert!(h.is_empty());
    }

    // ---- VS2-05: IPC-32..34 ------------------------------------------------------------

    fn json_run(engine: &mut Engine, history: &mut History, json: &str) -> Result<(), CoreError> {
        undo_run(engine, history, json)
    }

    #[test]
    fn ipc_32_locate_off_rem_dim_and_clear_reach_the_engine() {
        let mut e = demo_engine().unwrap();
        let mut h = History::default();
        let sel = r#"{"v":0,"cmd":"programmer.select","fixture":101}"#;
        json_run(&mut e, &mut h, sel).unwrap();
        json_run(
            &mut e,
            &mut h,
            r#"{"v":0,"cmd":"programmer.locate","groups":["P"]}"#,
        )
        .unwrap();
        assert!(e.located().get(&101).is_some_and(|a| a.contains_key("Pan")));
        assert!(!e
            .located()
            .get(&101)
            .is_some_and(|a| a.contains_key("Dimmer")));
        json_run(
            &mut e,
            &mut h,
            r#"{"v":0,"cmd":"programmer.off","attribute":"Pan"}"#,
        )
        .unwrap();
        assert!(!e.located().get(&101).is_some_and(|a| a.contains_key("Pan")));
        json_run(&mut e, &mut h, r#"{"v":0,"cmd":"programmer.locate"}"#).unwrap();
        assert!(e
            .located()
            .get(&101)
            .is_some_and(|a| a.contains_key("Dimmer")));
        // No field: Off of the active group.
        json_run(
            &mut e,
            &mut h,
            r#"{"v":0,"cmd":"programmer.attribute_group","group":"I"}"#,
        )
        .unwrap();
        json_run(&mut e, &mut h, r#"{"v":0,"cmd":"programmer.off"}"#).unwrap();
        assert!(!e
            .located()
            .get(&101)
            .is_some_and(|a| a.contains_key("Dimmer")));
        json_run(&mut e, &mut h, r#"{"v":0,"cmd":"programmer.rem_dim"}"#).unwrap();
        assert!(e.programmer().in_programmer(102));
        json_run(
            &mut e,
            &mut h,
            r#"{"v":0,"cmd":"programmer.clear","time":2.0}"#,
        )
        .unwrap();
        assert!(e.programmer().is_empty() && e.located().is_empty());
        assert_eq!(e.programmer().selection(), &[101]);
        json_run(
            &mut e,
            &mut h,
            r#"{"v":0,"cmd":"programmer.highlight","on":true}"#,
        )
        .unwrap();
        assert!(e.highlight());
    }

    #[test]
    fn ipc_32_bad_fields_are_errors() {
        for json in [
            r#"{"v":0,"cmd":"programmer.off","groups":["C"],"attribute":"Hue"}"#,
            r#"{"v":0,"cmd":"programmer.off","groups":"C"}"#,
            r#"{"v":0,"cmd":"programmer.highlight"}"#,
            r#"{"v":0,"cmd":"programmer.clear","time":"3"}"#,
            r#"{"v":0,"cmd":"programmer.locate","groups":["Q"]}"#,
            r#"{"v":0,"cmd":"settings.programmer","clear_ltp":"Melt"}"#,
            r#"{"v":0,"cmd":"settings.programmer","clear_precedence":"Never"}"#,
        ] {
            assert!(ipc::parse_command(json).is_err(), "{json}");
        }
    }

    #[test]
    fn ipc_32_settings_programmer_is_all_or_nothing() {
        let mut e = demo_engine().unwrap();
        let mut h = History::default();
        let before = e.settings().clone();
        assert!(json_run(
            &mut e,
            &mut h,
            r#"{"v":0,"cmd":"settings.programmer","clear_ltp":"Freeze","lowlight_level":2.0}"#,
        )
        .is_err());
        assert_eq!(e.settings(), &before);
        json_run(
            &mut e,
            &mut h,
            r#"{"v":0,"cmd":"settings.programmer","clear_precedence":"SelectionWithProgrammer","clear_ltp":"Freeze","lowlight_level":0.5}"#,
        )
        .unwrap();
        assert_eq!(
            e.settings().clear_ltp,
            cmdmx_core::programmer::ClearLtp::Freeze
        );
        assert_eq!(e.settings().lowlight_level, 0.5);
    }

    #[test]
    fn ipc_34_labels_and_highlight_stays_out_of_the_history() {
        let mut e = demo_engine().unwrap();
        let mut h = History::default();
        json_run(
            &mut e,
            &mut h,
            r#"{"v":0,"cmd":"programmer.select","fixture":201}"#,
        )
        .unwrap();
        let n = h.len();
        json_run(
            &mut e,
            &mut h,
            r#"{"v":0,"cmd":"programmer.highlight","on":true}"#,
        )
        .unwrap();
        assert_eq!(h.len(), n);
        json_run(&mut e, &mut h, r#"{"v":0,"cmd":"programmer.locate"}"#).unwrap();
        assert_eq!(h.undo_label(), Some("Locate"));
        json_run(
            &mut e,
            &mut h,
            r#"{"v":0,"cmd":"programmer.off","groups":["C"]}"#,
        )
        .unwrap();
        assert_eq!(h.undo_label(), Some("Off C"));
        json_run(
            &mut e,
            &mut h,
            r#"{"v":0,"cmd":"programmer.off","attribute":"Dimmer"}"#,
        )
        .unwrap();
        assert_eq!(h.undo_label(), Some("Off Dimmer"));
        json_run(&mut e, &mut h, r#"{"v":0,"cmd":"programmer.rem_dim"}"#).unwrap();
        assert_eq!(h.undo_label(), Some("Rem Dim"));
        json_run(
            &mut e,
            &mut h,
            r#"{"v":0,"cmd":"programmer.clear","time":1.0}"#,
        )
        .unwrap();
        assert_eq!(h.undo_label(), Some("Clear"));
        json_run(
            &mut e,
            &mut h,
            r#"{"v":0,"cmd":"settings.programmer","lowlight_level":0.3}"#,
        )
        .unwrap();
        assert_eq!(h.undo_label(), Some("Settings"));
        h.undo(&mut e).unwrap();
        assert_eq!(e.settings().lowlight_level, 0.25);
    }
}
