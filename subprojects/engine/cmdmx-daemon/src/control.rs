//! The daemon side of the generic MIDI control layer (VS7-02, `specs/modules/control-surfaces.md`).
//!
//! `midi_io` owns the CoreMIDI ports on its own thread and sends [`MidiEvent`]s; the tick thread
//! owns [`Control`]: devices and their states (CTL-08, 15, 16, 24), mapping files and the
//! `control` key of `state.json` (CTL-23), conversion of inputs to engine commands coalesced per
//! tick (CTL-06, 14), feedback after the tick (CTL-21), Learn (CTL-25), `last_input` (CTL-28) and
//! the `control` section of the snapshot and its delta (CTL-39).
//!
//! SKELETON: signatures fixed by the tests in `control/tests.rs`; bodies to be written.

use std::path::PathBuf;

use cmdmx_core::engine::Engine;

use crate::ipc;

/// One device the MIDI thread sees now. `name` is already numbered (`BCF2000 #2`, CTL-08).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortInfo {
    pub name: String,
    pub input: bool,
    pub output: bool,
}

/// From the MIDI thread to the tick, over a channel the tick drains without waiting (CTL-14).
#[derive(Debug, Clone, PartialEq)]
pub enum MidiEvent {
    /// Every device connected now; sent whenever the list changes (CTL-08, 16).
    Ports(Vec<PortInfo>),
    /// One CoreMIDI packet from a device's input.
    Packet { device: String, bytes: Vec<u8> },
}

/// From the tick to the MIDI thread: feedback bytes for a device's output (CTL-17..21).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MidiOut {
    pub device: String,
    pub bytes: Vec<u8>,
}

pub struct Control {}

impl Control {
    /// Mapping files from `dir` (created when missing), device choices from the `control` key of
    /// `state_file`. The strings are load warnings (a binding or a file left out).
    pub fn load(_dir: PathBuf, _state_file: PathBuf) -> (Control, Vec<String>) {
        (Control {}, Vec::new())
    }

    /// Called while the tick drains its channels; never blocks.
    pub fn event(&mut self, _event: MidiEvent, _now: f64) {}

    /// Engine commands for the inputs since the last call, each with its device name (for the
    /// refusal text, CTL-06). Values per input coalesced to the last; buttons never (CTL-14).
    pub fn commands(&mut self, _engine: &Engine, _now: f64) -> Vec<(String, ipc::Command)> {
        Vec::new()
    }

    /// A `control.*` command (CTL-40). `Err` is the refusal text.
    pub fn request(&mut self, _req: ipc::ControlRequest) -> Result<(), String> {
        Err("not built".into())
    }

    /// The answer to `control.mapping {name}`, for the asking client only.
    pub fn mapping_reply(&self, _name: &str) -> String {
        String::new()
    }

    /// Feedback after the tick's frame (CTL-19..21).
    pub fn feedback(&mut self, _engine: &Engine) -> Vec<MidiOut> {
        Vec::new()
    }

    /// Texts for the message zone since the last call (`BCF2000 disconnected`, `Learned: …`).
    pub fn take_messages(&mut self) -> Vec<String> {
        Vec::new()
    }

    /// The snapshot's `control` section (CTL-39).
    pub fn snapshot(&self) -> serde_json::Value {
        serde_json::Value::Null
    }

    /// A `control` delta after a change; `last_input` at most 10 per second (CTL-28, 39).
    pub fn delta(&mut self, _now: f64) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests;
