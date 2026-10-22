//! Shared output plumbing: per-node thread, non-blocking hand-off and health
//! (OUT-11/12), the per-node state the IPC reports (IPC-20), and the reconciler that
//! keeps one thread per enabled node of the show (OUT-29). Protocol nodes live in
//! `artnet_out`, `sacn_out` and `serial_out`.

use std::collections::BTreeMap;
use std::io;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cmdmx_core::output::{artnet_address, Node, NodeId, NodeKind};
use cmdmx_core::{UniverseId, UNIVERSE_SIZE};
use cmdmx_protocols::{artnet, sacn};

use crate::artnet_out::ArtNetNode;
use crate::sacn_out::{multicast_target, SacnNode};
use crate::serial_out::{Choice, SerialNode, MIN_WRITE_INTERVAL};

pub type FrameData = [u8; UNIVERSE_SIZE];

/// IPC-20: what the UI shows per output node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputState {
    Ok,
    Degraded,
    /// Serial only: no interface found yet (shown as `Degraded` with a detail).
    NoInterface,
    /// Serial only: the chosen path does not exist (shown as `Degraded`, OUT-31).
    PortNotFound,
    /// The node's thread is gone (OUT-11).
    Stopped,
    /// Not running: disabled in the show, or killed by a `--no-*` flag (OUT-32).
    Disabled,
}

/// IPC-20: a node's state, shared between its thread (writer) and the tick (reader).
/// The detail (the error text, OUT-33) is written before the state, so a reader that
/// sees a new state also sees its detail.
#[derive(Debug, Clone)]
pub struct Status {
    state: Arc<AtomicU8>,
    detail: Arc<Mutex<Option<String>>>,
}

impl Status {
    pub fn new() -> Self {
        Status {
            state: Arc::new(AtomicU8::new(OutputState::Ok as u8)),
            detail: Arc::default(),
        }
    }

    pub fn get(&self) -> OutputState {
        use OutputState::*;
        let x = self.state.load(Ordering::Acquire);
        [Degraded, NoInterface, PortNotFound, Stopped, Disabled]
            .into_iter()
            .find(|s| *s as u8 == x)
            .unwrap_or(Ok)
    }

    pub fn set(&self, state: OutputState) {
        self.state.store(state as u8, Ordering::Release);
    }

    pub fn detail(&self) -> Option<String> {
        self.detail.lock().ok().and_then(|d| d.clone())
    }

    pub fn set_detail(&self, detail: Option<String>) {
        if let Ok(mut d) = self.detail.lock() {
            *d = detail;
        }
    }
}

impl Default for Status {
    fn default() -> Self {
        Self::new()
    }
}

/// OUT-12: node state; a message is returned only when the state flips.
#[derive(Debug, Default)]
pub struct Health {
    degraded: bool,
    detail: Option<String>,
}

impl Health {
    pub fn is_degraded(&self) -> bool {
        self.degraded
    }

    pub fn state(&self) -> OutputState {
        if self.degraded {
            OutputState::Degraded
        } else {
            OutputState::Ok
        }
    }

    /// OUT-33: the error that made the node degraded; a different error while degraded
    /// is not recorded again.
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    pub fn record(&mut self, result: &io::Result<()>) -> Option<String> {
        match (self.degraded, result) {
            (false, Err(e)) => {
                self.degraded = true;
                self.detail = Some(e.to_string());
                Some(format!("Degraded: {e}"))
            }
            (true, Ok(())) => {
                self.degraded = false;
                self.detail = None;
                Some("Ok again".into())
            }
            _ => None,
        }
    }
}

/// OUT-11: run one node on its own thread. `send` returns a log line on a
/// health change plus the node's state (IPC-20, kept in the returned `Status`); a
/// panic inside it is caught and the thread carries on with the last state.
#[cfg(test)]
pub fn spawn(
    name: &'static str,
    send: impl FnMut(&FrameData) -> (Option<String>, OutputState) + Send + 'static,
) -> (SyncSender<FrameData>, Status) {
    spawn_with(Status::new(), name, send)
}

/// Like `spawn`, on a `Status` the caller made, so `send` can set the detail on it (OUT-33).
pub fn spawn_with(
    status: Status,
    name: &'static str,
    mut send: impl FnMut(&FrameData) -> (Option<String>, OutputState) + Send + 'static,
) -> (SyncSender<FrameData>, Status) {
    let (tx, rx) = sync_channel(1);
    let shared = status.clone();
    std::thread::spawn(move || run(name, &mut send, &rx, &shared));
    (tx, status)
}

fn run(
    name: &str,
    send: &mut impl FnMut(&FrameData) -> (Option<String>, OutputState),
    rx: &Receiver<FrameData>,
    status: &Status,
) {
    while let Ok(frame) = rx.recv() {
        match catch_unwind(AssertUnwindSafe(|| send(&frame))) {
            Ok((log, state)) => {
                status.set(state);
                if let Some(msg) = log {
                    eprintln!("{msg}");
                }
            }
            Err(_) => eprintln!("{name} output: panic caught, continuing"),
        }
    }
}

/// OUT-24: like `spawn`, but the thread writes the latest frame on its own timer
/// every `interval` instead of once per received frame.
#[cfg(test)]
pub fn spawn_paced(
    name: &'static str,
    interval: Duration,
    send: impl FnMut(&FrameData, Instant) -> (Option<String>, OutputState) + Send + 'static,
) -> (SyncSender<FrameData>, Status) {
    spawn_paced_with(Status::new(), name, interval, send)
}

/// `spawn_paced` on a `Status` the caller made (OUT-33).
pub fn spawn_paced_with(
    status: Status,
    name: &'static str,
    interval: Duration,
    mut send: impl FnMut(&FrameData, Instant) -> (Option<String>, OutputState) + Send + 'static,
) -> (SyncSender<FrameData>, Status) {
    let (tx, rx) = sync_channel(1);
    let shared = status.clone();
    std::thread::spawn(move || {
        run_paced(
            name,
            interval,
            &mut send,
            &mut |d| rx.recv_timeout(d),
            &mut Instant::now,
            &shared,
        )
    });
    (tx, status)
}

/// OUT-24: nothing is written before the first frame, then the first write is
/// immediate and each next one is due `interval` after the previous write STARTED
/// (a slow write or a panic does not shift the timer). Time and channel come in as
/// closures so the pacing is testable; returns when the sender is gone.
pub fn run_paced(
    name: &str,
    interval: Duration,
    send: &mut impl FnMut(&FrameData, Instant) -> (Option<String>, OutputState),
    recv: &mut impl FnMut(Duration) -> Result<FrameData, RecvTimeoutError>,
    now: &mut impl FnMut() -> Instant,
    status: &Status,
) {
    let Ok(mut latest) = recv(Duration::MAX) else {
        return;
    };
    let mut due = now();
    loop {
        let t = now();
        if t >= due {
            due = t.checked_add(interval).unwrap_or(t);
            match catch_unwind(AssertUnwindSafe(|| send(&latest, t))) {
                Ok((log, state)) => {
                    status.set(state);
                    if let Some(msg) = log {
                        eprintln!("{msg}");
                    }
                }
                Err(_) => eprintln!("{name} output: panic caught, continuing"),
            }
        }
        match recv(due.saturating_duration_since(now())) {
            Ok(f) => latest = f,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// OUT-11: hand a frame to the output thread without blocking. A full
/// channel drops this frame; `false` means the thread is gone.
pub fn publish(tx: &SyncSender<FrameData>, frame: &FrameData) -> bool {
    !matches!(tx.try_send(*frame), Err(TrySendError::Disconnected(_)))
}

/// UDP socket on an ephemeral source port, so a local monitor can own the
/// protocol port (OUT-04/06).
pub fn open_udp() -> io::Result<UdpSocket> {
    UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
}

// ---- OUT-29..33: the nodes of the show, kept running ------------------------------------

/// OUT-32: the `--no-*` flags; a killed kind never sends this run, its nodes stay in the show.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Kill {
    pub artnet: bool,
    pub sacn: bool,
    pub serial: bool,
}

impl Kill {
    /// The flag that kills this node's kind, if it is on.
    fn flag(&self, kind: &NodeKind) -> Option<&'static str> {
        match kind {
            NodeKind::Artnet { .. } => self.artnet.then_some("--no-artnet"),
            NodeKind::Sacn { .. } => self.sacn.then_some("--no-sacn"),
            NodeKind::Serial { .. } => self.serial.then_some("--no-serial"),
        }
    }
}

/// OUT-31: the serial paths that enabled, running serial nodes name; one list shared by all.
pub type Claimed = Arc<Mutex<Vec<String>>>;

/// What the reconciler shares with a running node thread.
#[derive(Debug, Clone)]
pub struct Live {
    /// OUT-29: the sACN node's priority, edited without a restart.
    pub priority: Arc<AtomicU8>,
    pub claimed: Claimed,
}

/// OUT-33: one node as the tick sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputStatus {
    pub node: Node,
    pub state: OutputState,
    pub detail: Option<String>,
}

fn kind_name(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Artnet { .. } => "art-net",
        NodeKind::Sacn { .. } => "sacn",
        NodeKind::Serial { .. } => "serial",
    }
}

/// `art-net #3`: the prefix of every log line of the node (OUT-30).
fn node_name(node: &Node) -> String {
    format!("{} #{}", kind_name(&node.kind), node.id)
}

/// OUT-30: the line printed when a node starts or changes.
pub fn describe(node: &Node) -> String {
    let name = node_name(node);
    let line = node.line;
    match &node.kind {
        NodeKind::Artnet {
            port_address,
            target,
        } => format!(
            "{name}: universe {line} → {} at {target}",
            artnet_address(*port_address)
        ),
        NodeKind::Sacn {
            universe,
            priority,
            unicast,
        } => {
            let to = unicast
                .as_ref()
                .map_or("multicast".to_string(), |ip| format!("at {ip}"));
            format!("{name}: universe {line} → sACN {universe} {to}, priority {priority}")
        }
        NodeKind::Serial { port } => format!("{name}: universe {line} → {port}"),
    }
}

/// OUT-29: a change of where the node sends needs a new thread; line and priority do not.
fn same_destination(a: &NodeKind, b: &NodeKind) -> bool {
    match (a, b) {
        (
            NodeKind::Artnet {
                port_address: pa,
                target: ta,
            },
            NodeKind::Artnet {
                port_address: pb,
                target: tb,
            },
        ) => pa == pb && ta == tb,
        (
            NodeKind::Sacn {
                universe: ua,
                unicast: na,
                ..
            },
            NodeKind::Sacn {
                universe: ub,
                unicast: nb,
                ..
            },
        ) => ua == ub && na == nb,
        (NodeKind::Serial { port: a }, NodeKind::Serial { port: b }) => a == b,
        _ => false,
    }
}

fn priority_of(kind: &NodeKind) -> u8 {
    match kind {
        NodeKind::Sacn { priority, .. } => *priority,
        _ => 0,
    }
}

struct Running {
    tx: SyncSender<FrameData>,
    status: Status,
    live: Live,
    /// The node as it last started or changed.
    node: Node,
}

type Spawner = Box<dyn FnMut(&Node, &Live) -> (SyncSender<FrameData>, Status)>;

/// OUT-29: one thread per enabled, not killed node of the show.
pub struct Outputs {
    kill: Kill,
    spawn: Spawner,
    claimed: Claimed,
    /// The list `reconcile` last saw, to skip the work when nothing changed.
    nodes: Vec<Node>,
    /// Sorted by line, then id; `slots[i]` is the running node of `statuses[i]`, if any.
    statuses: Vec<OutputStatus>,
    slots: Vec<Option<Running>>,
}

impl Outputs {
    pub fn new(kill: Kill, spawn: Spawner) -> Self {
        Outputs {
            kill,
            spawn,
            claimed: Claimed::default(),
            nodes: Vec::new(),
            statuses: Vec::new(),
            slots: Vec::new(),
        }
    }

    pub fn statuses(&self) -> &[OutputStatus] {
        &self.statuses
    }

    /// Bring the running threads in line with the show's nodes. Cheap to call every tick:
    /// an unchanged list does nothing and allocates nothing.
    pub fn reconcile<'a>(&mut self, nodes: impl IntoIterator<Item = &'a Node>) {
        let mut it = nodes.into_iter();
        let mut same = 0;
        let new: Vec<Node> = loop {
            match (it.next(), self.nodes.get(same)) {
                (None, None) => return,
                (Some(a), Some(b)) if a == b => same += 1,
                (a, _) => {
                    let mut v: Vec<Node> = self.nodes.iter().take(same).cloned().collect();
                    v.extend(a.cloned());
                    v.extend(it.cloned());
                    break v;
                }
            }
        };

        // OUT-31: the paths enabled serial nodes name, so no `auto` node takes them.
        let mut claimed: Vec<String> = new
            .iter()
            .filter(|n| n.enabled && self.kill.flag(&n.kind).is_none())
            .filter_map(|n| match &n.kind {
                NodeKind::Serial { port } if port != "auto" => Some(port.clone()),
                _ => None,
            })
            .collect();
        claimed.sort();
        if let Ok(mut shared) = self.claimed.lock() {
            *shared = claimed;
        }

        let mut old: BTreeMap<NodeId, Running> = std::mem::take(&mut self.statuses)
            .into_iter()
            .zip(std::mem::take(&mut self.slots))
            .filter_map(|(s, r)| r.map(|r| (s.node.id, r)))
            .collect();
        let mut sorted: Vec<&Node> = new.iter().collect();
        sorted.sort_by_key(|n| (n.line, n.id));
        for node in sorted {
            let killed = self.kill.flag(&node.kind);
            let previous = old.remove(&node.id);
            if !node.enabled || killed.is_some() {
                // The sender is dropped with `previous`; the thread ends on its own.
                self.statuses.push(OutputStatus {
                    node: node.clone(),
                    state: OutputState::Disabled,
                    detail: killed.map(String::from),
                });
                self.slots.push(None);
                continue;
            }
            let kept = previous.filter(|r| {
                same_destination(&r.node.kind, &node.kind) && r.status.get() != OutputState::Stopped
            });
            let running = match kept {
                Some(mut r) => {
                    r.live
                        .priority
                        .store(priority_of(&node.kind), Ordering::Relaxed);
                    if r.node != *node {
                        eprintln!("{}", describe(node));
                        r.node = node.clone();
                    }
                    r
                }
                None => {
                    eprintln!("{}", describe(node));
                    let live = Live {
                        priority: Arc::new(AtomicU8::new(priority_of(&node.kind))),
                        claimed: self.claimed.clone(),
                    };
                    let (tx, status) = (self.spawn)(node, &live);
                    Running {
                        tx,
                        status,
                        live,
                        node: node.clone(),
                    }
                }
            };
            self.statuses.push(OutputStatus {
                node: node.clone(),
                state: running.status.get(),
                detail: running.status.detail(),
            });
            self.slots.push(Some(running));
        }
        self.nodes = new;
    }

    /// Hand every running node its line's frame (or zeros) and refresh the statuses.
    /// Never blocks and does not allocate.
    pub fn publish<'a>(&mut self, frame: impl Fn(UniverseId) -> Option<&'a FrameData>) {
        static ZEROS: FrameData = [0; UNIVERSE_SIZE];
        for (out, slot) in self.statuses.iter_mut().zip(&self.slots) {
            let Some(r) = slot else { continue };
            if out.state != OutputState::Stopped
                && !publish(&r.tx, frame(out.node.line).unwrap_or(&ZEROS))
            {
                eprintln!(
                    "{} output thread stopped; tick continues",
                    node_name(&out.node)
                );
                r.status.set(OutputState::Stopped);
            }
            let state = r.status.get();
            if state != out.state {
                out.state = state;
                out.detail = r.status.detail();
            }
        }
    }
}

/// A node that cannot start (a target that does not parse): it reports why, never panics.
fn failed(name: String, why: String) -> (SyncSender<FrameData>, Status) {
    let status = Status::new();
    status.set_detail(Some(why.clone()));
    let mut said = false;
    spawn_with(status, "failed", move |_| {
        let log = (!std::mem::replace(&mut said, true)).then(|| format!("{name}: {why}"));
        (log, OutputState::Degraded)
    })
}

/// The real spawner: one output thread for the node, sockets and ports opened on it.
pub fn spawn_node(node: &Node, live: &Live) -> (SyncSender<FrameData>, Status) {
    let name = node_name(node);
    let status = Status::new();
    let shared = status.clone();
    match &node.kind {
        NodeKind::Artnet {
            port_address,
            target,
        } => match target.parse::<Ipv4Addr>() {
            Ok(ip) => {
                let target = SocketAddrV4::new(ip, artnet::PORT);
                let mut n = ArtNetNode::new(name, target, *port_address);
                spawn_with(status, "art-net", move |f| {
                    let log = n.send(f);
                    shared.set_detail(n.detail().map(String::from));
                    (log, n.state())
                })
            }
            Err(_) => failed(name, format!("'{target}' is not an IPv4 address")),
        },
        NodeKind::Sacn {
            universe,
            unicast,
            priority,
        } => {
            let target = match unicast.as_deref().map(str::parse::<Ipv4Addr>) {
                None => Ok(multicast_target(*universe)),
                Some(ip) => ip.map(|ip| SocketAddrV4::new(ip, sacn::PORT)),
            };
            match target {
                Ok(target) => {
                    let mut n = SacnNode::new(name, target, *universe, *priority);
                    let prio = live.priority.clone();
                    spawn_with(status, "sacn", move |f| {
                        n.set_priority(prio.load(Ordering::Relaxed));
                        let log = n.send(f);
                        shared.set_detail(n.detail().map(String::from));
                        (log, n.state())
                    })
                }
                Err(_) => failed(name, "the unicast address is not an IPv4 address".into()),
            }
        }
        NodeKind::Serial { port } => {
            let choice = if port == "auto" {
                // Not scanned yet, so no interface until the thread says otherwise (IPC-20).
                status.set(OutputState::NoInterface);
                Choice::Auto(live.claimed.clone())
            } else {
                Choice::Path(port.clone())
            };
            let mut n = SerialNode::new(name, choice);
            spawn_paced_with(status, "serial", MIN_WRITE_INTERVAL, move |f, now| {
                let log = n.send(f, now);
                shared.set_detail(n.detail());
                (log, n.state())
            })
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::collections::VecDeque;
    use std::rc::Rc;
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::{Duration, Instant};

    #[test]
    fn out_11_publish_never_blocks_when_output_is_not_consuming() {
        let (tx, _rx) = sync_channel(1);
        let start = Instant::now();
        for _ in 0..1000 {
            assert!(publish(&tx, &[0; 512]));
        }
        assert!(start.elapsed() < Duration::from_millis(100));
    }

    #[test]
    fn out_11_publish_reports_a_dead_output_thread() {
        let (tx, rx) = sync_channel(1);
        drop(rx);
        assert!(!publish(&tx, &[0; 512]));
    }

    #[test]
    fn ipc_20_health_gives_the_node_state() {
        let mut h = Health::default();
        assert_eq!(h.state(), OutputState::Ok);
        h.record(&Err(io::Error::other("no route")));
        assert_eq!(h.state(), OutputState::Degraded);
        h.record(&Ok(()));
        assert_eq!(h.state(), OutputState::Ok);
    }

    /// IPC-20: the tick reads each node's state through its `Status` without waiting.
    #[test]
    fn ipc_20_status_follows_what_the_node_reports() {
        let (tx, status) = spawn("test", |f: &FrameData| {
            let state = if f[0] == 1 {
                OutputState::Degraded
            } else {
                OutputState::Ok
            };
            (None, state)
        });
        assert_eq!(status.get(), OutputState::Ok, "a node starts Ok");
        let wait_for = |want: OutputState| {
            let start = Instant::now();
            while status.get() != want {
                assert!(
                    start.elapsed() < Duration::from_secs(2),
                    "never became {want:?}"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        let mut frame = [0u8; 512];
        frame[0] = 1;
        tx.send(frame).unwrap();
        wait_for(OutputState::Degraded);
        tx.send([0; 512]).unwrap();
        wait_for(OutputState::Ok);
        // The tick marks a node whose thread is gone (OUT-11).
        status.set(OutputState::Stopped);
        assert_eq!(status.get(), OutputState::Stopped);
    }

    /// OUT-24: a fake clock plus a scripted channel. `recv(timeout)` returns the next
    /// frame if it arrives within `timeout` (moving the clock to its arrival), else
    /// moves the clock by `timeout` and times out; with no frames left the sender is gone.
    #[allow(clippy::type_complexity)]
    fn scripted(
        t0: Instant,
        arrivals_ms: &[u64],
    ) -> (
        Rc<Cell<Instant>>,
        impl FnMut() -> Instant,
        impl FnMut(Duration) -> Result<FrameData, RecvTimeoutError>,
    ) {
        let clock = Rc::new(Cell::new(t0));
        let mut queue: VecDeque<(Instant, FrameData)> = arrivals_ms
            .iter()
            .enumerate()
            .map(|(i, ms)| (t0 + Duration::from_millis(*ms), [i as u8; UNIVERSE_SIZE]))
            .collect();
        let now = {
            let clock = clock.clone();
            move || clock.get()
        };
        let recv = {
            let clock = clock.clone();
            move |timeout: Duration| match queue.front() {
                None => Err(RecvTimeoutError::Disconnected),
                Some((at, _)) if at.saturating_duration_since(clock.get()) <= timeout => {
                    clock.set(clock.get().max(*at));
                    queue
                        .pop_front()
                        .map(|(_, f)| f)
                        .ok_or(RecvTimeoutError::Disconnected)
                }
                Some(_) => {
                    clock.set(clock.get() + timeout);
                    Err(RecvTimeoutError::Timeout)
                }
            }
        };
        (clock, now, recv)
    }

    #[test]
    fn out_24_paced_node_writes_the_latest_frame_every_30_ms() {
        let t0 = Instant::now();
        // The tick hands over a frame every 25 ms, the first at 10 ms.
        let (_clock, mut now, mut recv) = scripted(t0, &[10, 35, 60, 85, 110, 135]);
        let mut writes = Vec::new();
        let mut send = |f: &FrameData, at: Instant| {
            writes.push((at.duration_since(t0).as_millis() as u64, f[0]));
            (None, OutputState::Ok)
        };
        let status = Status::new();
        run_paced(
            "test",
            Duration::from_millis(30),
            &mut send,
            &mut recv,
            &mut now,
            &status,
        );
        // Nothing before the first frame; then every 30 ms with the newest frame:
        // 10 → frame 0, 40 → 1 (35), 70 → 2 (60), 100 → 3 (85), 130 → 4 (110).
        assert_eq!(writes, [(10, 0), (40, 1), (70, 2), (100, 3), (130, 4)]);
    }

    #[test]
    fn out_24_paced_interval_counts_from_the_start_of_a_slow_write() {
        let t0 = Instant::now();
        let (clock, mut now, mut recv) = scripted(t0, &[0, 25, 50, 75, 100, 125]);
        let mut writes = Vec::new();
        let mut send = |_: &FrameData, at: Instant| {
            writes.push(at.duration_since(t0).as_millis() as u64);
            // A 518-byte frame takes ~21 ms at 250000 baud.
            clock.set(clock.get() + Duration::from_millis(21));
            (None, OutputState::Ok)
        };
        run_paced(
            "test",
            Duration::from_millis(30),
            &mut send,
            &mut recv,
            &mut now,
            &Status::new(),
        );
        assert_eq!(writes, [0, 30, 60, 90, 120]);
    }

    #[test]
    fn out_24_paced_node_survives_a_panic_and_reports_its_state() {
        let t0 = Instant::now();
        let (_clock, mut now, mut recv) = scripted(t0, &[0, 25, 50, 75]);
        let mut calls = 0;
        let mut writes = Vec::new();
        let mut send = |_: &FrameData, at: Instant| {
            calls += 1;
            if calls == 1 {
                panic!("first write panics");
            }
            writes.push(at.duration_since(t0).as_millis() as u64);
            (None, OutputState::Degraded)
        };
        let status = Status::new();
        run_paced(
            "test",
            Duration::from_millis(30),
            &mut send,
            &mut recv,
            &mut now,
            &status,
        );
        assert_eq!(writes, [30, 60], "the timer carries on after the panic");
        assert_eq!(status.get(), OutputState::Degraded);
    }

    /// OUT-24 on a real thread: one frame handed over keeps being written by the timer.
    #[test]
    fn out_24_spawn_paced_writes_without_new_frames() {
        let calls = Arc::new(AtomicU8::new(0));
        let counted = calls.clone();
        let (tx, _status) = spawn_paced("test", Duration::from_millis(30), move |_, _| {
            counted.fetch_add(1, Ordering::Relaxed);
            (None, OutputState::Ok)
        });
        tx.send([0; UNIVERSE_SIZE]).unwrap();
        let start = Instant::now();
        while calls.load(Ordering::Relaxed) < 3 {
            assert!(
                start.elapsed() < Duration::from_secs(2),
                "the timer did not write again"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn out_12_logs_once_per_state_change() {
        let err = || Err(io::Error::other("no route"));
        let mut h = Health::default();
        assert_eq!(h.record(&Ok(())), None);
        assert!(h.record(&err()).unwrap().contains("Degraded"));
        assert_eq!(h.record(&err()), None, "still degraded: silent");
        assert!(h.record(&Ok(())).unwrap().contains("Ok"));
        assert_eq!(h.record(&Ok(())), None);
    }

    // ---- OUT-29..33: nodes from the show, reconciled ----------------------------

    use cmdmx_core::output::{Node, NodeKind};
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    fn artnet(id: u32, line: u16, port_address: u16) -> Node {
        Node {
            id,
            line,
            enabled: true,
            kind: NodeKind::Artnet {
                port_address,
                target: "255.255.255.255".into(),
            },
        }
    }

    fn sacn(id: u32, line: u16) -> Node {
        Node {
            id,
            line,
            enabled: true,
            kind: NodeKind::default_sacn(line),
        }
    }

    fn serial(id: u32, port: &str) -> Node {
        Node {
            id,
            line: 1,
            enabled: true,
            kind: NodeKind::Serial { port: port.into() },
        }
    }

    /// A fake spawner: every spawn is recorded, and the test keeps each node's receiving end
    /// (the latest one per id) and its `Status`, so it sees what the tick hands over.
    struct Fake {
        spawned: Rc<RefCell<Vec<u32>>>,
        rx: Rc<RefCell<BTreeMap<u32, Receiver<FrameData>>>>,
        status: Rc<RefCell<BTreeMap<u32, Status>>>,
        live: Rc<RefCell<BTreeMap<u32, Live>>>,
    }

    fn fake(kill: Kill) -> (Outputs, Fake) {
        let f = Fake {
            spawned: Default::default(),
            rx: Default::default(),
            status: Default::default(),
            live: Default::default(),
        };
        let (spawned, rx, status, live) = (
            f.spawned.clone(),
            f.rx.clone(),
            f.status.clone(),
            f.live.clone(),
        );
        let outputs = Outputs::new(
            kill,
            Box::new(move |node: &Node, l: &Live| {
                let (tx, r) = sync_channel(1);
                let s = Status::new();
                spawned.borrow_mut().push(node.id);
                rx.borrow_mut().insert(node.id, r);
                status.borrow_mut().insert(node.id, s.clone());
                live.borrow_mut().insert(node.id, l.clone());
                (tx, s)
            }),
        );
        (outputs, f)
    }

    fn frames(line: u16) -> Option<&'static FrameData> {
        static ONE: FrameData = [1; UNIVERSE_SIZE];
        (line == 1).then_some(&ONE)
    }

    #[test]
    fn out_29_each_enabled_node_gets_its_lines_frame_or_zeros() {
        let (mut outputs, f) = fake(Kill::default());
        let mut off = sacn(3, 3);
        off.enabled = false;
        outputs.reconcile(&[artnet(1, 1, 0), sacn(2, 2), off]);
        assert_eq!(*f.spawned.borrow(), [1, 2], "a disabled node never starts");
        outputs.publish(frames);
        let rx = f.rx.borrow();
        assert_eq!(rx[&1].try_recv().unwrap()[0], 1, "line 1's frame");
        assert_eq!(
            rx[&2].try_recv().unwrap(),
            [0; UNIVERSE_SIZE],
            "nothing patched on line 2: 512 zeros"
        );
        let states: Vec<_> = outputs
            .statuses()
            .iter()
            .map(|s| (s.node.id, s.state))
            .collect();
        assert_eq!(
            states,
            [
                (1, OutputState::Ok),
                (2, OutputState::Ok),
                (3, OutputState::Disabled)
            ]
        );
    }

    #[test]
    fn out_29_reconcile_keeps_unchanged_nodes_and_restarts_changed_destinations() {
        let (mut outputs, f) = fake(Kill::default());
        let mut nodes = vec![
            artnet(1, 1, 0),
            sacn(2, 1),
            serial(3, "/dev/cu.usbserial-A"),
        ];
        outputs.reconcile(&nodes);
        outputs.reconcile(&nodes);
        assert_eq!(
            *f.spawned.borrow(),
            [1, 2, 3],
            "the same list starts nothing"
        );

        // A new line for #1 is not a new destination: same thread, the other line's frame.
        nodes[0].line = 2;
        // A new sACN universe for #2 is a new destination: restarted. #4 is new.
        nodes[1].kind = NodeKind::default_sacn(5);
        nodes.push(artnet(4, 3, 2));
        outputs.reconcile(&nodes);
        assert_eq!(*f.spawned.borrow(), [1, 2, 3, 2, 4]);
        outputs.publish(frames);
        assert_eq!(
            f.rx.borrow()[&1].try_recv().unwrap(),
            [0; UNIVERSE_SIZE],
            "#1 kept its thread and now sends line 2"
        );

        // A new serial port restarts #3; removing #4 and disabling #1 stops them.
        nodes[2].kind = NodeKind::Serial {
            port: "/dev/cu.usbserial-B".into(),
        };
        nodes[0].enabled = false;
        nodes.pop();
        let old_one = f.rx.borrow_mut().remove(&1).unwrap();
        let old_four = f.rx.borrow_mut().remove(&4).unwrap();
        outputs.reconcile(&nodes);
        assert_eq!(*f.spawned.borrow(), [1, 2, 3, 2, 4, 3]);
        assert!(matches!(
            old_one.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Disconnected)
        ));
        // #4 still holds the frame it was handed before it was removed.
        assert!(old_four.try_recv().is_ok());
        assert!(matches!(
            old_four.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Disconnected)
        ));

        // Enabled again: a fresh thread.
        nodes[0].enabled = true;
        outputs.reconcile(&nodes);
        assert_eq!(*f.spawned.borrow(), [1, 2, 3, 2, 4, 3, 1]);
    }

    #[test]
    fn out_29_a_new_priority_keeps_the_thread_and_reaches_it() {
        let (mut outputs, f) = fake(Kill::default());
        let mut nodes = vec![sacn(2, 1)];
        outputs.reconcile(&nodes);
        nodes[0].kind = NodeKind::Sacn {
            universe: 1,
            priority: 150,
            unicast: None,
        };
        outputs.reconcile(&nodes);
        assert_eq!(*f.spawned.borrow(), [2], "priority is not a destination");
        assert_eq!(f.live.borrow()[&2].priority.load(Ordering::Relaxed), 150);
        assert_eq!(
            outputs.statuses()[0].node,
            nodes[0],
            "the status shows the edit"
        );
    }

    #[test]
    fn out_11_out_29_a_node_whose_thread_is_gone_is_stopped() {
        let (mut outputs, f) = fake(Kill::default());
        outputs.reconcile(&[artnet(1, 1, 0)]);
        f.rx.borrow_mut().clear();
        outputs.publish(frames);
        assert_eq!(outputs.statuses()[0].state, OutputState::Stopped);
    }

    #[test]
    fn out_33_statuses_follow_the_threads_and_carry_the_detail() {
        let (mut outputs, f) = fake(Kill::default());
        outputs.reconcile(&[artnet(1, 1, 0)]);
        let status = f.status.borrow()[&1].clone();
        status.set_detail(Some("no route to host".into()));
        status.set(OutputState::Degraded);
        outputs.publish(frames);
        let s = &outputs.statuses()[0];
        assert_eq!(
            (s.state, s.detail.as_deref()),
            (OutputState::Degraded, Some("no route to host"))
        );
    }

    #[test]
    fn out_33_statuses_are_sorted_by_line_then_id() {
        let (mut outputs, _f) = fake(Kill::default());
        outputs.reconcile(&[artnet(1, 3, 2), sacn(2, 1), artnet(3, 1, 0), sacn(4, 2)]);
        let order: Vec<u32> = outputs.statuses().iter().map(|s| s.node.id).collect();
        assert_eq!(order, [2, 3, 4, 1]);
    }

    #[test]
    fn out_32_kill_switches_keep_the_nodes_but_do_not_send() {
        let kill = Kill {
            artnet: false,
            sacn: true,
            serial: true,
        };
        let (mut outputs, f) = fake(kill);
        outputs.reconcile(&[artnet(1, 1, 0), sacn(2, 1), serial(3, "auto")]);
        assert_eq!(*f.spawned.borrow(), [1]);
        let states: Vec<_> = outputs
            .statuses()
            .iter()
            .map(|s| (s.node.id, s.state, s.detail.clone()))
            .collect();
        assert_eq!(
            states,
            [
                (1, OutputState::Ok, None),
                (2, OutputState::Disabled, Some("--no-sacn".to_string())),
                (3, OutputState::Disabled, Some("--no-serial".to_string())),
            ]
        );
    }

    #[test]
    fn out_31_auto_knows_the_ports_other_serial_nodes_name() {
        let (mut outputs, f) = fake(Kill::default());
        let mut nodes = vec![serial(1, "auto"), serial(2, "/dev/cu.usbserial-A")];
        outputs.reconcile(&nodes);
        let claimed = f.live.borrow()[&1].claimed.clone();
        assert_eq!(*claimed.lock().unwrap(), ["/dev/cu.usbserial-A"]);
        nodes[1].enabled = false;
        outputs.reconcile(&nodes);
        assert!(
            claimed.lock().unwrap().is_empty(),
            "a disabled node names no port"
        );
    }

    #[test]
    fn out_30_log_line_names_the_node() {
        assert_eq!(
            describe(&artnet(3, 2, 1)),
            "art-net #3: universe 2 → 0:0:1 at 255.255.255.255"
        );
        assert_eq!(
            describe(&sacn(4, 2)),
            "sacn #4: universe 2 → sACN 2 multicast, priority 100"
        );
        let mut unicast = sacn(6, 2);
        unicast.kind = NodeKind::Sacn {
            universe: 7,
            priority: 50,
            unicast: Some("10.0.0.5".into()),
        };
        assert_eq!(
            describe(&unicast),
            "sacn #6: universe 2 → sACN 7 at 10.0.0.5, priority 50"
        );
        assert_eq!(describe(&serial(5, "auto")), "serial #5: universe 1 → auto");
    }

    #[test]
    fn out_33_health_keeps_the_last_error_as_detail() {
        let mut h = Health::default();
        assert_eq!(h.detail(), None);
        h.record(&Err(io::Error::other("no route")));
        assert_eq!(h.detail(), Some("no route"));
        h.record(&Ok(()));
        assert_eq!(h.detail(), None);
    }
}
