//! Shared output plumbing: per-node thread, non-blocking hand-off and health
//! (OUT-11/12), and the per-node state the IPC reports (IPC-20). Protocol nodes live in
//! `artnet_out` and `sacn_out`.

use std::io;
use std::net::{Ipv4Addr, UdpSocket};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cmdmx_core::UNIVERSE_SIZE;

pub type FrameData = [u8; UNIVERSE_SIZE];

/// IPC-20: what the UI shows per output node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputState {
    Ok,
    Degraded,
    /// Serial only: no interface found yet (shown as `Degraded` with a detail).
    NoInterface,
    /// The node's thread is gone (OUT-11).
    Stopped,
}

/// IPC-20: a node's state, shared between its thread (writer) and the tick (reader).
#[derive(Debug, Clone)]
pub struct Status(Arc<AtomicU8>);

impl Status {
    fn new() -> Self {
        Status(Arc::new(AtomicU8::new(OutputState::Ok as u8)))
    }

    pub fn get(&self) -> OutputState {
        match self.0.load(Ordering::Relaxed) {
            x if x == OutputState::Degraded as u8 => OutputState::Degraded,
            x if x == OutputState::NoInterface as u8 => OutputState::NoInterface,
            x if x == OutputState::Stopped as u8 => OutputState::Stopped,
            _ => OutputState::Ok,
        }
    }

    pub fn set(&self, state: OutputState) {
        self.0.store(state as u8, Ordering::Relaxed);
    }
}

/// OUT-12: node state; a message is returned only when the state flips.
#[derive(Debug, Default)]
pub struct Health {
    degraded: bool,
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

    pub fn record(&mut self, result: &io::Result<()>) -> Option<String> {
        match (self.degraded, result) {
            (false, Err(e)) => {
                self.degraded = true;
                Some(format!("Degraded: {e}"))
            }
            (true, Ok(())) => {
                self.degraded = false;
                Some("Ok again".into())
            }
            _ => None,
        }
    }
}

/// OUT-11: run one node on its own thread. `send` returns a log line on a
/// health change plus the node's state (IPC-20, kept in the returned `Status`); a
/// panic inside it is caught and the thread carries on with the last state.
pub fn spawn(
    name: &'static str,
    mut send: impl FnMut(&FrameData) -> (Option<String>, OutputState) + Send + 'static,
) -> (SyncSender<FrameData>, Status) {
    let (tx, rx) = sync_channel(1);
    let status = Status::new();
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
pub fn spawn_paced(
    name: &'static str,
    interval: Duration,
    mut send: impl FnMut(&FrameData, Instant) -> (Option<String>, OutputState) + Send + 'static,
) -> (SyncSender<FrameData>, Status) {
    let (tx, rx) = sync_channel(1);
    let status = Status::new();
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
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
}
