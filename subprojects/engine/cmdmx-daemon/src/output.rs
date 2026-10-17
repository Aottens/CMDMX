//! Shared output plumbing: per-node thread, non-blocking hand-off and health
//! (OUT-11/12), and the per-node state the IPC reports (IPC-20). Protocol nodes live in
//! `artnet_out` and `sacn_out`.

use std::io;
use std::net::{Ipv4Addr, UdpSocket};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::Arc;

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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
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
