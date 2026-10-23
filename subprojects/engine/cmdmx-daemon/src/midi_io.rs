//! CoreMIDI ports through `midir`, on their own thread (VS7-02, CTL-08, 14, 16).
//!
//! SKELETON: signatures fixed by the tests in `control/tests.rs`; bodies to be written.

use std::sync::mpsc::Sender;

use crate::control::{MidiEvent, MidiOut};

/// CTL-08: a second device with the same name is `name #2`, a third `name #3`, in port order.
pub fn number_names(names: &[String]) -> Vec<String> {
    names.to_vec()
}

/// Start the MIDI thread: it scans the ports at least once a second, opens every input and
/// output, sends `Ports` when the list changes and every packet as it arrives, and writes what
/// arrives on the returned sender to the device's output. Off macOS it only sends `Ports([])`.
pub fn spawn(_events: Sender<MidiEvent>) -> std::io::Result<Sender<MidiOut>> {
    Err(std::io::Error::other("not built"))
}
