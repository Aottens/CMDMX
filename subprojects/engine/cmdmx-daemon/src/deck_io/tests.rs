//! VS7-06: the `deck` thread against a fake HID layer (CTL-44..47, 52..54). Times are short
//! (`Timing`) so Busy retries and rescans happen within a test.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cmdmx_control::deck::{self, brightness_report, image_reports};
use cmdmx_control::deck_image::{render, KeyLook, KeyState};

use super::{deck_names, spawn_with, DeckCmd, DeckEvent, DeckInfo, DeckState, Hid, HidDevice};
use super::{HidInfo, Timing};

const ORIGINAL: u16 = 0x0060;

/// Everything the fake devices did, shared with the test.
#[derive(Default)]
struct World {
    /// What `list` returns.
    plugged: Vec<HidInfo>,
    /// Paths another program holds: `open` fails.
    busy: BTreeSet<String>,
    /// Paths opened so far, in order.
    opened: Vec<String>,
    /// Open handles per path (dropped = closed).
    open_now: BTreeMap<String, usize>,
    /// Output reports per path.
    writes: BTreeMap<String, Vec<Vec<u8>>>,
    /// Feature reports per path.
    features: BTreeMap<String, Vec<Vec<u8>>>,
    /// Input reports waiting per path.
    inputs: BTreeMap<String, Vec<Vec<u8>>>,
    /// Every write sleeps this long (a slow deck).
    write_delay: Duration,
    /// Writes to these paths fail.
    fail_writes: BTreeSet<String>,
    /// The next `list` call panics.
    panic_next_list: bool,
}

type Shared = Arc<Mutex<World>>;

struct FakeHid(Shared);
struct FakeDev {
    path: String,
    world: Shared,
}

impl Hid for FakeHid {
    fn list(&mut self) -> Vec<HidInfo> {
        let mut w = self.0.lock().unwrap();
        if std::mem::take(&mut w.panic_next_list) {
            drop(w);
            panic!("fake hid panics");
        }
        w.plugged.clone()
    }

    fn open(&mut self, path: &str) -> Result<Box<dyn HidDevice>, String> {
        let mut w = self.0.lock().unwrap();
        if w.busy.contains(path) {
            return Err("exclusive access".into());
        }
        if !w.plugged.iter().any(|d| d.path == path) {
            return Err("no such device".into());
        }
        w.opened.push(path.to_string());
        *w.open_now.entry(path.to_string()).or_default() += 1;
        Ok(Box::new(FakeDev {
            path: path.to_string(),
            world: self.0.clone(),
        }))
    }
}

impl Drop for FakeDev {
    fn drop(&mut self) {
        if let Ok(mut w) = self.world.lock() {
            if let Some(n) = w.open_now.get_mut(&self.path) {
                *n -= 1;
            }
        }
    }
}

impl HidDevice for FakeDev {
    fn read(&mut self, _ms: i32) -> Result<Option<Vec<u8>>, String> {
        let mut w = self.world.lock().unwrap();
        if !w.plugged.iter().any(|d| d.path == self.path) {
            return Err("device gone".into());
        }
        let q = w.inputs.entry(self.path.clone()).or_default();
        Ok((!q.is_empty()).then(|| q.remove(0)))
    }

    fn write(&mut self, report: &[u8]) -> Result<(), String> {
        let delay = self.world.lock().unwrap().write_delay;
        std::thread::sleep(delay);
        let mut w = self.world.lock().unwrap();
        if w.fail_writes.contains(&self.path) || !w.plugged.iter().any(|d| d.path == self.path) {
            return Err("write failed".into());
        }
        w.writes
            .entry(self.path.clone())
            .or_default()
            .push(report.to_vec());
        Ok(())
    }

    fn feature(&mut self, report: &[u8]) -> Result<(), String> {
        let mut w = self.world.lock().unwrap();
        w.features
            .entry(self.path.clone())
            .or_default()
            .push(report.to_vec());
        Ok(())
    }
}

fn info(path: &str, pid: u16, serial: Option<&str>) -> HidInfo {
    HidInfo {
        path: path.into(),
        product_id: pid,
        serial: serial.map(str::to_string),
    }
}

const FAST: Timing = Timing {
    scan: Duration::from_millis(10),
    retry: Duration::from_millis(60),
};

/// A deck thread on a fake world with one Original plugged in (serial ending A1B2).
fn start(world: &Shared) -> (Sender<DeckCmd>, Receiver<DeckEvent>) {
    let (events, rx) = channel();
    let w = world.clone();
    let tx = spawn_with(move || Box::new(FakeHid(w.clone())) as Box<dyn Hid>, events, FAST).unwrap();
    (tx, rx)
}

fn world_with_original() -> Shared {
    let world: Shared = Default::default();
    world
        .lock()
        .unwrap()
        .plugged
        .push(info("p1", ORIGINAL, Some("AL12A1B2")));
    world
}

const NAME: &str = "Stream Deck Original (A1B2)";

/// The next event that `pick` accepts, within 2 s; the others are dropped.
fn wait_for<T>(rx: &Receiver<DeckEvent>, mut pick: impl FnMut(&DeckEvent) -> Option<T>) -> T {
    let until = Instant::now() + Duration::from_secs(2);
    while let Some(left) = until.checked_duration_since(Instant::now()) {
        if let Ok(e) = rx.recv_timeout(left) {
            if let Some(t) = pick(&e) {
                return t;
            }
        }
    }
    panic!("event not seen within 2 s");
}

fn wait_state(rx: &Receiver<DeckEvent>, want: DeckState) {
    wait_for(rx, |e| match e {
        DeckEvent::State { name, state } if name == NAME && *state == want => Some(()),
        _ => None,
    });
}

fn wait_until(world: &Shared, what: &str, mut ok: impl FnMut(&World) -> bool) {
    let until = Instant::now() + Duration::from_secs(2);
    while Instant::now() < until {
        if ok(&world.lock().unwrap()) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("not within 2 s: {what}");
}

fn original() -> deck::Model {
    deck::model(ORIGINAL).unwrap()
}

fn look(label: &str) -> KeyLook {
    KeyLook {
        label: label.into(),
        sub: String::new(),
        halo: None,
        level: None,
        state: KeyState::Active,
    }
}

fn open(tx: &Sender<DeckCmd>, brightness: u8) {
    tx.send(DeckCmd::Open {
        name: NAME.into(),
        brightness,
    })
    .unwrap();
}

/// The writes that put `look` on `key` of the Original.
fn image_of(key: u16, look: &KeyLook) -> Vec<Vec<u8>> {
    let m = original();
    image_reports(&m, key, &render(look, &m))
}

// --- CTL-44 names ---------------------------------------------------------

#[test]
fn ctl_44_a_deck_is_named_by_model_and_the_last_four_of_its_serial() {
    let names = deck_names(&[
        info("a", ORIGINAL, Some("AL12A1B2")),
        info("b", 0x006c, Some("CL99ZZ01")),
    ]);
    assert_eq!(names, ["Stream Deck Original (A1B2)", "Stream Deck XL (ZZ01)"]);
}

#[test]
fn ctl_44_without_a_serial_the_names_are_numbered() {
    let names = deck_names(&[
        info("a", ORIGINAL, None),
        info("b", ORIGINAL, Some("")),
        info("c", 0x006c, Some("CL99ZZ01")),
    ]);
    assert_eq!(
        names,
        [
            "Stream Deck Original",
            "Stream Deck Original #2",
            "Stream Deck XL (ZZ01)"
        ]
    );
}

#[test]
fn ctl_45_an_unknown_product_id_is_listed_but_never_opened() {
    let world: Shared = Default::default();
    world
        .lock()
        .unwrap()
        .plugged
        .push(info("x", 0x0fff, Some("QQ0042")));
    let (tx, rx) = start(&world);
    let list = wait_for(&rx, |e| match e {
        DeckEvent::Decks(l) if !l.is_empty() => Some(l.clone()),
        _ => None,
    });
    assert_eq!(
        list,
        [DeckInfo {
            name: "Elgato 0x0fff (0042)".into(),
            product_id: 0x0fff,
        }]
    );
    tx.send(DeckCmd::Open {
        name: "Elgato 0x0fff (0042)".into(),
        brightness: 70,
    })
    .unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert!(world.lock().unwrap().opened.is_empty());
}

// --- CTL-46 open only when asked, Busy retry -------------------------------

#[test]
fn ctl_46_a_deck_is_listed_but_not_opened_until_open_is_asked() {
    let world = world_with_original();
    let (tx, rx) = start(&world);
    let list = wait_for(&rx, |e| match e {
        DeckEvent::Decks(l) if !l.is_empty() => Some(l.clone()),
        _ => None,
    });
    assert_eq!(
        list,
        [DeckInfo {
            name: NAME.into(),
            product_id: ORIGINAL,
        }]
    );
    std::thread::sleep(Duration::from_millis(100));
    assert!(world.lock().unwrap().opened.is_empty(), "opened unasked");
    open(&tx, 70);
    wait_state(&rx, DeckState::Ok);
    assert_eq!(world.lock().unwrap().opened, ["p1"]);
}

#[test]
fn ctl_46_a_held_deck_is_busy_and_retried_until_it_is_free() {
    let world = world_with_original();
    world.lock().unwrap().busy.insert("p1".into());
    let (tx, rx) = start(&world);
    open(&tx, 70);
    wait_state(&rx, DeckState::Busy);
    // Still held over a few retries: no second Busy event, nothing opened.
    std::thread::sleep(FAST.retry * 3);
    assert!(world.lock().unwrap().opened.is_empty());
    while let Ok(e) = rx.try_recv() {
        assert!(
            !matches!(e, DeckEvent::State { state: DeckState::Busy, .. }),
            "Busy reported twice"
        );
    }
    world.lock().unwrap().busy.clear();
    wait_state(&rx, DeckState::Ok);
}

#[test]
fn ctl_46_close_blanks_every_key_then_closes() {
    let world = world_with_original();
    let (tx, rx) = start(&world);
    open(&tx, 70);
    wait_state(&rx, DeckState::Ok);
    tx.send(DeckCmd::Close { name: NAME.into() }).unwrap();
    wait_until(&world, "closed", |w| w.open_now.get("p1") == Some(&0));
    let w = world.lock().unwrap();
    let writes = &w.writes["p1"];
    for key in 1..=15 {
        for r in image_of(key, &KeyLook::black()) {
            assert!(writes.contains(&r), "key {key} not blanked");
        }
    }
}

// --- CTL-53 brightness, quit ----------------------------------------------

#[test]
fn ctl_53_brightness_is_set_on_open_and_on_change() {
    let world = world_with_original();
    let (tx, rx) = start(&world);
    open(&tx, 40);
    wait_state(&rx, DeckState::Ok);
    let m = original();
    wait_until(&world, "brightness 40", |w| {
        w.features.get("p1").is_some_and(|f| f.contains(&brightness_report(&m, 40)))
    });
    tx.send(DeckCmd::Brightness {
        name: NAME.into(),
        percent: 90,
    })
    .unwrap();
    wait_until(&world, "brightness 90", |w| {
        w.features.get("p1").and_then(|f| f.last()) == Some(&brightness_report(&m, 90))
    });
}

#[test]
fn ctl_53_quit_blanks_every_open_deck_and_answers() {
    let world = world_with_original();
    let (tx, rx) = start(&world);
    open(&tx, 70);
    wait_state(&rx, DeckState::Ok);
    tx.send(DeckCmd::Looks {
        name: NAME.into(),
        keys: vec![(3, look("Wash"))],
    })
    .unwrap();
    wait_until(&world, "key 3 drawn", |w| {
        w.writes.get("p1").is_some_and(|ws| ws.ends_with(&image_of(3, &look("Wash"))))
    });
    let (done, answered) = channel();
    tx.send(DeckCmd::Quit { done }).unwrap();
    answered.recv_timeout(Duration::from_secs(2)).unwrap();
    let w = world.lock().unwrap();
    assert!(w.writes["p1"].ends_with(&image_of(15, &KeyLook::black())));
    for r in image_of(3, &KeyLook::black()) {
        assert!(w.writes["p1"].contains(&r));
    }
    assert_eq!(w.open_now.get("p1"), Some(&0));
}

// --- CTL-52 newest look only -----------------------------------------------

#[test]
fn ctl_52_a_slow_deck_skips_to_the_newest_look_and_never_queues() {
    let world = world_with_original();
    let (tx, rx) = start(&world);
    open(&tx, 70);
    wait_state(&rx, DeckState::Ok);
    world.lock().unwrap().write_delay = Duration::from_millis(20);
    let sent = Instant::now();
    for i in 0..50 {
        tx.send(DeckCmd::Looks {
            name: NAME.into(),
            keys: vec![(1, look(&format!("L{i}")))],
        })
        .unwrap();
    }
    // Sending never waits for USB.
    assert!(sent.elapsed() < Duration::from_millis(50));
    let last = image_of(1, &look("L49"));
    wait_until(&world, "the newest look on key 1", |w| {
        w.writes.get("p1").is_some_and(|ws| ws.ends_with(&last))
    });
    let w = world.lock().unwrap();
    // Each image is 2 writes on the Original; far fewer than 50 images were drawn.
    let images = w.writes["p1"].len() / last.len();
    assert!(images <= 10, "{images} images drawn for 50 looks");
}

// --- CTL-47 inputs reach the tick -----------------------------------------

#[test]
fn ctl_47_input_reports_reach_the_tick_with_the_deck_name() {
    let world = world_with_original();
    let (tx, rx) = start(&world);
    open(&tx, 70);
    wait_state(&rx, DeckState::Ok);
    let mut report = vec![0u8; 17];
    report[0] = 0x01;
    report[5] = 1;
    world
        .lock()
        .unwrap()
        .inputs
        .entry("p1".into())
        .or_default()
        .push(report.clone());
    let got = wait_for(&rx, |e| match e {
        DeckEvent::Report { name, bytes } if name == NAME => Some(bytes.clone()),
        _ => None,
    });
    assert_eq!(got, report);
}

// --- CTL-54 unplug, failed write, panic ------------------------------------

#[test]
fn ctl_54_unplugged_goes_missing_and_plugged_back_it_reopens() {
    let world = world_with_original();
    let (tx, rx) = start(&world);
    open(&tx, 70);
    wait_state(&rx, DeckState::Ok);
    let plugged = std::mem::take(&mut world.lock().unwrap().plugged);
    wait_for(&rx, |e| match e {
        DeckEvent::Decks(l) if l.is_empty() => Some(()),
        _ => None,
    });
    wait_until(&world, "closed", |w| w.open_now.get("p1") == Some(&0));
    world.lock().unwrap().plugged = plugged;
    wait_for(&rx, |e| match e {
        DeckEvent::Decks(l) if l.len() == 1 => Some(()),
        _ => None,
    });
    // The tick asks again for a deck that comes back (it is still enabled there).
    open(&tx, 70);
    wait_state(&rx, DeckState::Ok);
}

#[test]
fn ctl_54_a_failed_write_counts_as_unplugged_and_it_is_opened_again() {
    let world = world_with_original();
    let (tx, rx) = start(&world);
    open(&tx, 70);
    wait_state(&rx, DeckState::Ok);
    world.lock().unwrap().fail_writes.insert("p1".into());
    tx.send(DeckCmd::Looks {
        name: NAME.into(),
        keys: vec![(1, look("Go"))],
    })
    .unwrap();
    wait_state(&rx, DeckState::Missing);
    world.lock().unwrap().fail_writes.clear();
    // Still plugged and still wanted: opened again after the retry interval.
    wait_state(&rx, DeckState::Ok);
    assert_eq!(world.lock().unwrap().opened, ["p1", "p1"]);
}

#[test]
fn ctl_54_a_panic_is_caught_the_decks_go_missing_and_the_thread_starts_again() {
    let world = world_with_original();
    let (tx, rx) = start(&world);
    open(&tx, 70);
    wait_state(&rx, DeckState::Ok);
    world.lock().unwrap().panic_next_list = true;
    wait_for(&rx, |e| match e {
        DeckEvent::Decks(l) if l.is_empty() => Some(()),
        _ => None,
    });
    wait_until(&world, "closed by the unwind", |w| w.open_now.get("p1") == Some(&0));
    wait_for(&rx, |e| match e {
        DeckEvent::Decks(l) if l.len() == 1 => Some(()),
        _ => None,
    });
    open(&tx, 70);
    wait_state(&rx, DeckState::Ok);
}
