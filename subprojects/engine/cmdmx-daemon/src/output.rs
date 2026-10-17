//! Shared output plumbing: per-node thread, non-blocking hand-off and health
//! (OUT-11/12). Protocol nodes live in `artnet_out` and `sacn_out`.

use std::io;
use std::net::{Ipv4Addr, UdpSocket};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};

use cmdmx_core::UNIVERSE_SIZE;

pub type FrameData = [u8; UNIVERSE_SIZE];

/// OUT-12: node state; a message is returned only when the state flips.
#[derive(Debug, Default)]
pub struct Health {
    degraded: bool,
}

impl Health {
    pub fn is_degraded(&self) -> bool {
        self.degraded
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
/// health change; a panic inside it is caught and the thread carries on.
pub fn spawn(
    name: &'static str,
    mut send: impl FnMut(&FrameData) -> Option<String> + Send + 'static,
) -> SyncSender<FrameData> {
    let (tx, rx) = sync_channel(1);
    std::thread::spawn(move || run(name, &mut send, &rx));
    tx
}

fn run(name: &str, send: &mut impl FnMut(&FrameData) -> Option<String>, rx: &Receiver<FrameData>) {
    while let Ok(frame) = rx.recv() {
        match catch_unwind(AssertUnwindSafe(|| send(&frame))) {
            Ok(Some(msg)) => eprintln!("{msg}"),
            Ok(None) => {}
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
