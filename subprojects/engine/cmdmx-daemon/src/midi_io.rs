//! CoreMIDI ports through `midir`, on their own thread (VS7-02, CTL-08, 14, 16).

use std::sync::mpsc::Sender;

use crate::control::{MidiEvent, MidiOut};

/// CTL-08: a second device with the same name is `name #2`, a third `name #3`, in port order.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn number_names(names: &[String]) -> Vec<String> {
    let mut seen: Vec<&str> = Vec::new();
    names
        .iter()
        .map(|n| {
            seen.push(n);
            match seen.iter().filter(|s| **s == n).count() {
                1 => n.clone(),
                k => format!("{n} #{k}"),
            }
        })
        .collect()
}

/// Off macOS there are no ports: the list is empty and feedback goes nowhere.
#[cfg(not(target_os = "macos"))]
pub fn spawn(events: Sender<MidiEvent>) -> std::io::Result<Sender<MidiOut>> {
    let _ = events.send(MidiEvent::Ports(Vec::new()));
    Ok(std::sync::mpsc::channel().0)
}

/// Start the MIDI thread: it scans the ports every 250 ms, opens every input and output, sends
/// `Ports` when the list changes and every message as it arrives, and writes what arrives on the
/// returned sender to the device's output (CTL-08, 14, 16).
#[cfg(target_os = "macos")]
pub fn spawn(events: Sender<MidiEvent>) -> std::io::Result<Sender<MidiOut>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("midi".into())
        .spawn(move || {
            // A panic here ends MIDI, never the daemon.
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| mac::run(&events, &rx)))
                .is_err()
            {
                eprintln!("midi: thread panicked; MIDI is off");
            }
        })?;
    Ok(tx)
}

#[cfg(target_os = "macos")]
mod mac {
    use std::collections::{BTreeMap, HashSet};
    use std::sync::mpsc::{Receiver, Sender, TryRecvError};
    use std::time::{Duration, Instant};

    use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};

    use super::number_names;
    use crate::control::{MidiEvent, MidiOut, PortInfo};

    const SCAN: Duration = Duration::from_millis(250);
    const POLL: Duration = Duration::from_millis(5);

    /// An open port, remembered with the CoreMIDI id it was opened on: a device that went and
    /// came back between two scans has a new id and is opened again.
    struct Open<C> {
        id: String,
        conn: C,
    }

    pub fn run(events: &Sender<MidiEvent>, rx: &Receiver<MidiOut>) {
        let (Ok(scan_in), Ok(scan_out)) =
            (MidiInput::new("cmdmx scan"), MidiOutput::new("cmdmx scan"))
        else {
            eprintln!("midi: cannot open CoreMIDI; MIDI is off");
            return;
        };
        let mut inputs: BTreeMap<String, Open<MidiInputConnection<()>>> = BTreeMap::new();
        let mut outputs: BTreeMap<String, Open<MidiOutputConnection>> = BTreeMap::new();
        let mut sent: Option<Vec<PortInfo>> = None;
        let mut failed: HashSet<String> = HashSet::new();
        let mut last_scan: Option<Instant> = None;
        loop {
            if last_scan.is_none_or(|t| t.elapsed() >= SCAN) {
                last_scan = Some(Instant::now());
                // Names in port order, numbered per direction (CTL-08).
                let ins = scan_in.ports();
                let in_names = number_names(
                    &ins.iter()
                        .map(|p| scan_in.port_name(p).unwrap_or_default())
                        .collect::<Vec<_>>(),
                );
                let outs = scan_out.ports();
                let out_names = number_names(
                    &outs
                        .iter()
                        .map(|p| scan_out.port_name(p).unwrap_or_default())
                        .collect::<Vec<_>>(),
                );
                let ids: BTreeMap<_, _> = in_names.iter().zip(ins.iter().map(|p| p.id())).collect();
                inputs.retain(|n, o| ids.get(n) == Some(&o.id));
                for (name, port) in in_names.iter().zip(&ins) {
                    if inputs.contains_key(name) {
                        continue;
                    }
                    let Ok(mut client) = MidiInput::new("cmdmx in") else {
                        continue;
                    };
                    client.ignore(Ignore::All);
                    let (tx, device) = (events.clone(), name.clone());
                    // Never blocks: an unbounded channel (CTL-14).
                    let conn = client.connect(
                        port,
                        "in",
                        move |_, bytes, _| {
                            let _ = tx.send(MidiEvent::Packet {
                                device: device.clone(),
                                bytes: bytes.to_vec(),
                            });
                        },
                        (),
                    );
                    if let Ok(conn) = conn {
                        inputs.insert(
                            name.clone(),
                            Open {
                                id: port.id(),
                                conn,
                            },
                        );
                    }
                }
                let ids: BTreeMap<_, _> =
                    out_names.iter().zip(outs.iter().map(|p| p.id())).collect();
                outputs.retain(|n, o| ids.get(n) == Some(&o.id));
                for (name, port) in out_names.iter().zip(&outs) {
                    if outputs.contains_key(name) {
                        continue;
                    }
                    let Ok(client) = MidiOutput::new("cmdmx out") else {
                        continue;
                    };
                    if let Ok(conn) = client.connect(port, "out") {
                        failed.remove(name);
                        outputs.insert(
                            name.clone(),
                            Open {
                                id: port.id(),
                                conn,
                            },
                        );
                    }
                }
                // Only what is open counts as there.
                let mut list: BTreeMap<String, PortInfo> = BTreeMap::new();
                let named = inputs
                    .keys()
                    .map(|n| (n, true))
                    .chain(outputs.keys().map(|n| (n, false)));
                for (name, is_input) in named {
                    let p = list.entry(name.clone()).or_insert_with(|| PortInfo {
                        name: name.clone(),
                        input: false,
                        output: false,
                    });
                    if is_input {
                        p.input = true;
                    } else {
                        p.output = true;
                    }
                }
                let list: Vec<PortInfo> = list.into_values().collect();
                if sent.as_ref() != Some(&list) {
                    if events.send(MidiEvent::Ports(list.clone())).is_err() {
                        return;
                    }
                    sent = Some(list);
                }
            }
            loop {
                match rx.try_recv() {
                    Ok(out) => {
                        if let Some(o) = outputs.get_mut(&out.device) {
                            if o.conn.send(&out.bytes).is_err() && failed.insert(out.device.clone())
                            {
                                eprintln!("midi: cannot send to {}", out.device);
                            }
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return,
                }
            }
            // CTL-16: CoreMIDI updates this process's device list through the run loop of the
            // thread that made the clients; without it a device plugged in later never shows up.
            // SAFETY: plain C calls on this thread's own run loop; returns at once (timeout 0).
            unsafe {
                use core_foundation_sys::runloop::{kCFRunLoopDefaultMode, CFRunLoopRunInMode};
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.0, 1);
            }
            std::thread::sleep(POLL);
        }
    }
}
