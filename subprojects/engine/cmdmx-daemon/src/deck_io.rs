//! Stream Decks over USB HID on their own thread (VS7-06, CTL-44..47, 52..54).
//!
//! The thread lists Elgato devices, opens the ones the tick asks for (never unasked, CTL-46),
//! reads their input reports, and draws key images; it keeps only the newest look per key
//! (CTL-52). The tick talks to it with [`DeckCmd`] and [`DeckEvent`]; it never waits for USB.

use std::collections::BTreeMap;
use std::io;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};

use cmdmx_control::deck::{self, brightness_report, image_reports, Model};
use cmdmx_control::deck_image::{render, KeyLook};

use crate::midi_io::number_names;

/// One Elgato HID device as the OS lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HidInfo {
    pub path: String,
    pub product_id: u16,
    pub serial: Option<String>,
}

/// The HID layer, so the thread can be tested against a fake.
pub trait Hid {
    /// Every Elgato device now (vendor 0x0fd9).
    fn list(&mut self) -> Vec<HidInfo>;
    /// `Err` when another program holds the device (Busy) or it is gone.
    fn open(&mut self, path: &str) -> Result<Box<dyn HidDevice>, String>;
}

pub trait HidDevice {
    /// One input report, or `None` after `ms` milliseconds without one.
    fn read(&mut self, ms: i32) -> Result<Option<Vec<u8>>, String>;
    fn write(&mut self, report: &[u8]) -> Result<(), String>;
    fn feature(&mut self, report: &[u8]) -> Result<(), String>;
}

#[derive(Debug, Clone, Copy)]
pub struct Timing {
    /// How often the device list is read.
    pub scan: Duration,
    /// How often a wished deck that is not open is tried again.
    pub retry: Duration,
}

impl Timing {
    pub const DEFAULT: Timing = Timing {
        scan: Duration::from_millis(250),
        retry: Duration::from_secs(2),
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckInfo {
    pub name: String,
    pub product_id: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeckState {
    Ok,
    Busy,
    Missing,
}

/// From the deck thread to the tick.
#[derive(Debug, Clone, PartialEq)]
pub enum DeckEvent {
    /// Every deck connected now; sent whenever the list changes.
    Decks(Vec<DeckInfo>),
    State {
        name: String,
        state: DeckState,
    },
    /// One input report.
    Report {
        name: String,
        bytes: Vec<u8>,
    },
}

/// From the tick to the deck thread.
#[derive(Debug)]
pub enum DeckCmd {
    /// Open this deck now and keep it open (CTL-46).
    Open {
        name: String,
        brightness: u8,
    },
    /// Blank, close and forget.
    Close {
        name: String,
    },
    /// Changed key looks; printed key numbers from 1.
    Looks {
        name: String,
        keys: Vec<(u16, KeyLook)>,
    },
    Brightness {
        name: String,
        percent: u8,
    },
    /// Blank every open deck, close them, answer on `done`, end the thread (CTL-53).
    Quit {
        done: Sender<()>,
    },
}

/// CTL-44: the model name, and the last 4 characters of the serial number; numbered
/// (`X`, `X #2`) for decks without one.
pub fn deck_names(list: &[HidInfo]) -> Vec<String> {
    let bases: Vec<String> = list
        .iter()
        .map(|d| match deck::model(d.product_id) {
            Some(m) => m.name,
            None => format!("Elgato 0x{:04x}", d.product_id),
        })
        .collect();
    let serial = |d: &HidInfo| d.serial.clone().filter(|s| !s.is_empty());
    let plain: Vec<String> = (list.iter().zip(&bases))
        .filter(|(d, _)| serial(d).is_none())
        .map(|(_, b)| b.clone())
        .collect();
    let mut numbered = number_names(&plain).into_iter();
    (list.iter().zip(bases))
        .map(|(d, base)| match serial(d) {
            Some(s) => {
                let n = s.chars().count();
                let tail: String = s.chars().skip(n.saturating_sub(4)).collect();
                format!("{base} ({tail})")
            }
            None => numbered.next().unwrap_or(base),
        })
        .collect()
}

/// Start the `deck` thread. A panic inside is caught, logged once, the decks go Missing and the
/// thread starts again (CTL-54).
pub fn spawn_with<F: FnMut() -> Box<dyn Hid> + Send + 'static>(
    mut make: F,
    events: Sender<DeckEvent>,
    timing: Timing,
) -> io::Result<Sender<DeckCmd>> {
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("deck".into())
        .spawn(move || {
            let mut logged = false;
            loop {
                let run = catch_unwind(AssertUnwindSafe(|| {
                    run(&mut *make(), &events, &rx, timing);
                }));
                if run.is_ok() {
                    return;
                }
                if !std::mem::replace(&mut logged, true) {
                    eprintln!("deck: thread panicked; restarting");
                }
                let _ = events.send(DeckEvent::Decks(Vec::new()));
                std::thread::sleep(timing.retry);
            }
        })?;
    Ok(tx)
}

pub fn spawn(events: Sender<DeckEvent>) -> io::Result<Sender<DeckCmd>> {
    spawn_with(system, events, Timing::DEFAULT)
}

#[cfg(target_os = "macos")]
fn system() -> Box<dyn Hid> {
    Box::new(mac::Mac::new())
}

/// Off macOS there are no decks.
#[cfg(not(target_os = "macos"))]
fn system() -> Box<dyn Hid> {
    struct None;
    impl Hid for None {
        fn list(&mut self) -> Vec<HidInfo> {
            Vec::new()
        }
        fn open(&mut self, _: &str) -> Result<Box<dyn HidDevice>, String> {
            Err("no HID on this platform".into())
        }
    }
    Box::new(None)
}

struct Wish {
    brightness: u8,
    busy_reported: bool,
    retry_at: Option<Instant>,
}

struct Opened {
    dev: Box<dyn HidDevice>,
    model: Model,
    /// The newest look per key, not yet drawn (CTL-52).
    pending: BTreeMap<u16, KeyLook>,
}

#[derive(PartialEq)]
enum Flow {
    Go,
    Stop,
}

struct Thread<'a> {
    events: &'a Sender<DeckEvent>,
    rx: &'a Receiver<DeckCmd>,
    timing: Timing,
    listed: Vec<(String, HidInfo)>,
    sent: Option<Vec<DeckInfo>>,
    wishes: BTreeMap<String, Wish>,
    open: BTreeMap<String, Opened>,
}

fn run(hid: &mut dyn Hid, events: &Sender<DeckEvent>, rx: &Receiver<DeckCmd>, timing: Timing) {
    let mut t = Thread {
        events,
        rx,
        timing,
        listed: Vec::new(),
        sent: None,
        wishes: BTreeMap::new(),
        open: BTreeMap::new(),
    };
    let mut last_scan: Option<Instant> = None;
    loop {
        if last_scan.is_none_or(|s| s.elapsed() >= timing.scan) {
            last_scan = Some(Instant::now());
            t.scan(hid);
        }
        if t.drain() == Flow::Stop {
            return;
        }
        t.open_wished(hid);
        if t.write_pending() == Flow::Stop {
            return;
        }
        if !t.read_inputs() {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

/// Black on every key, in order; a write error stops it (the deck is going anyway).
fn blank(o: &mut Opened) {
    for key in 1..=o.model.keys() {
        let image = render(&KeyLook::black(), &o.model);
        for r in image_reports(&o.model, key, &image) {
            if o.dev.write(&r).is_err() {
                return;
            }
        }
    }
}

impl Thread<'_> {
    fn send(&self, e: DeckEvent) {
        let _ = self.events.send(e);
    }

    /// CTL-44: list, name, dedupe by path; tell the tick when the list changed.
    fn scan(&mut self, hid: &mut dyn Hid) {
        let mut list = hid.list();
        list.dedup_by(|a, b| a.path == b.path);
        let names = deck_names(&list);
        self.listed = names.into_iter().zip(list).collect();
        let infos: Vec<DeckInfo> = (self.listed.iter())
            .map(|(name, d)| DeckInfo {
                name: name.clone(),
                product_id: d.product_id,
            })
            .collect();
        // A deck that left the list: close it, forget the wish (the tick asks again).
        let listed = &self.listed;
        self.open.retain(|n, _| listed.iter().any(|(l, _)| l == n));
        self.wishes
            .retain(|n, _| listed.iter().any(|(l, _)| l == n));
        if self.sent.as_ref() != Some(&infos) {
            self.sent = Some(infos.clone());
            self.send(DeckEvent::Decks(infos));
        }
    }

    fn drain(&mut self) -> Flow {
        loop {
            match self.rx.try_recv() {
                Ok(DeckCmd::Quit { done }) => {
                    self.close_all();
                    let _ = done.send(());
                    return Flow::Stop;
                }
                Ok(cmd) => self.handle(cmd),
                Err(TryRecvError::Empty) => return Flow::Go,
                Err(TryRecvError::Disconnected) => {
                    self.close_all();
                    return Flow::Stop;
                }
            }
        }
    }

    fn close_all(&mut self) {
        for o in self.open.values_mut() {
            blank(o);
        }
        self.open.clear();
    }

    fn handle(&mut self, cmd: DeckCmd) {
        match cmd {
            DeckCmd::Open { name, brightness } => {
                // CTL-45: an unsupported model is never opened.
                let supported = (self.listed.iter())
                    .find(|(n, _)| *n == name)
                    .is_some_and(|(_, d)| deck::model(d.product_id).is_some());
                if supported {
                    let wish = self.wishes.entry(name).or_insert(Wish {
                        brightness,
                        busy_reported: false,
                        retry_at: None,
                    });
                    wish.brightness = brightness;
                }
            }
            DeckCmd::Close { name } => {
                if let Some(mut o) = self.open.remove(&name) {
                    blank(&mut o);
                }
                self.wishes.remove(&name);
            }
            DeckCmd::Brightness { name, percent } => {
                if let Some(w) = self.wishes.get_mut(&name) {
                    w.brightness = percent;
                }
                let failed = self.open.get_mut(&name).is_some_and(|o| {
                    o.dev
                        .feature(&brightness_report(&o.model, percent))
                        .is_err()
                });
                if failed {
                    self.lose(&name);
                }
            }
            DeckCmd::Looks { name, keys } => {
                if let Some(o) = self.open.get_mut(&name) {
                    o.pending.extend(keys);
                }
            }
            DeckCmd::Quit { .. } => {}
        }
    }

    /// CTL-54: a failed read or write counts as unplugged; the wish stays, so it is opened
    /// again after the retry interval.
    fn lose(&mut self, name: &str) {
        if self.open.remove(name).is_none() {
            return;
        }
        if let Some(w) = self.wishes.get_mut(name) {
            (w.busy_reported, w.retry_at) = (false, Some(Instant::now() + self.timing.retry));
        }
        self.send(DeckEvent::State {
            name: name.to_string(),
            state: DeckState::Missing,
        });
    }

    /// CTL-46: wished and not open: try at once, then every `retry` while it fails.
    fn open_wished(&mut self, hid: &mut dyn Hid) {
        let now = Instant::now();
        let mut lost = Vec::new();
        for (name, wish) in &mut self.wishes {
            if self.open.contains_key(name) || wish.retry_at.is_some_and(|t| now < t) {
                continue;
            }
            let Some((_, info)) = self.listed.iter().find(|(n, _)| n == name) else {
                continue;
            };
            let Some(model) = deck::model(info.product_id) else {
                continue;
            };
            match hid.open(&info.path) {
                Ok(dev) => {
                    wish.busy_reported = false;
                    wish.retry_at = None;
                    let mut opened = Opened {
                        dev,
                        model,
                        pending: BTreeMap::new(),
                    };
                    let report = brightness_report(&opened.model, wish.brightness);
                    if opened.dev.feature(&report).is_err() {
                        wish.retry_at = Some(now + self.timing.retry);
                        lost.push(name.clone());
                        continue;
                    }
                    self.open.insert(name.clone(), opened);
                    let _ = self.events.send(DeckEvent::State {
                        name: name.clone(),
                        state: DeckState::Ok,
                    });
                }
                Err(_) => {
                    wish.retry_at = Some(now + self.timing.retry);
                    if !std::mem::replace(&mut wish.busy_reported, true) {
                        let _ = self.events.send(DeckEvent::State {
                            name: name.clone(),
                            state: DeckState::Busy,
                        });
                    }
                }
            }
        }
        for name in lost {
            self.send(DeckEvent::State {
                name,
                state: DeckState::Missing,
            });
        }
    }

    /// Draw the newest look per key. Commands are drained between keys, so a slow deck always
    /// gets the newest look and nothing queues (CTL-52).
    fn write_pending(&mut self) -> Flow {
        loop {
            if self.drain() == Flow::Stop {
                return Flow::Stop;
            }
            let next = self.open.iter_mut().find_map(|(name, o)| {
                let key = *o.pending.keys().next()?;
                Some((name.clone(), key, o.pending.remove(&key)?))
            });
            let Some((name, key, look)) = next else {
                return Flow::Go;
            };
            let failed = self.open.get_mut(&name).is_some_and(|o| {
                let image = render(&look, &o.model);
                let reports = image_reports(&o.model, key, &image);
                reports.iter().any(|r| o.dev.write(r).is_err())
            });
            if failed {
                self.lose(&name);
            }
        }
    }

    /// Input reports to the tick; true when anything was read.
    fn read_inputs(&mut self) -> bool {
        let mut any = false;
        let mut lost = Vec::new();
        for (name, o) in &mut self.open {
            for _ in 0..32 {
                match o.dev.read(1) {
                    Ok(Some(bytes)) => {
                        any = true;
                        let _ = self.events.send(DeckEvent::Report {
                            name: name.clone(),
                            bytes,
                        });
                    }
                    Ok(None) => break,
                    Err(_) => {
                        lost.push(name.clone());
                        break;
                    }
                }
            }
        }
        for name in lost {
            self.lose(&name);
        }
        any
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::CString;

    use hidapi::{HidApi, HidDevice as Raw};

    use cmdmx_control::deck::ELGATO_VENDOR;

    use super::{Hid, HidDevice, HidInfo};

    pub struct Mac {
        api: Option<HidApi>,
    }

    impl Mac {
        pub fn new() -> Mac {
            // No device discovery at start: `list` indexes the Elgato devices only.
            #[allow(deprecated)]
            Mac {
                api: HidApi::new_without_enumerate().ok(),
            }
        }
    }

    impl Hid for Mac {
        fn list(&mut self) -> Vec<HidInfo> {
            let Some(api) = &mut self.api else {
                return Vec::new();
            };
            if api.reset_devices().is_err() || api.add_devices(ELGATO_VENDOR, 0).is_err() {
                return Vec::new();
            }
            (api.device_list())
                .map(|d| HidInfo {
                    path: d.path().to_string_lossy().into_owned(),
                    product_id: d.product_id(),
                    serial: d.serial_number().map(str::to_string),
                })
                .collect()
        }

        /// Exclusive: macOS hidapi seizes the device unless `macos-shared-device` is on (CTL-46).
        fn open(&mut self, path: &str) -> Result<Box<dyn HidDevice>, String> {
            let api = self.api.as_ref().ok_or("HID is not available")?;
            let path = CString::new(path).map_err(|e| e.to_string())?;
            let dev = api.open_path(&path).map_err(|e| e.to_string())?;
            Ok(Box::new(Device(dev)))
        }
    }

    struct Device(Raw);

    impl HidDevice for Device {
        fn read(&mut self, ms: i32) -> Result<Option<Vec<u8>>, String> {
            let mut buf = [0u8; 1024];
            let n = self
                .0
                .read_timeout(&mut buf, ms)
                .map_err(|e| e.to_string())?;
            Ok(buf.get(..n).filter(|b| !b.is_empty()).map(<[u8]>::to_vec))
        }

        fn write(&mut self, report: &[u8]) -> Result<(), String> {
            self.0.write(report).map(|_| ()).map_err(|e| e.to_string())
        }

        fn feature(&mut self, report: &[u8]) -> Result<(), String> {
            self.0
                .send_feature_report(report)
                .map_err(|e| e.to_string())
        }
    }
}

#[cfg(test)]
mod tests;
