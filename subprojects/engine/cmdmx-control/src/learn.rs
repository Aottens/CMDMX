//! MIDI learn: guess a binding from the messages seen (CTL-26).

use crate::action::Action;
use crate::mapping::{Binding, Encoding, Input};
use crate::midi::{Kind, MidiInput, MidiMessage};

pub fn guess(action: Action, observed: &[MidiMessage]) -> Option<Binding> {
    let first = observed.first()?.input;
    let mut input = first;
    let mut encoding = None;
    if first.kind == Kind::Cc {
        let n = first.number?;
        let has_lsb = n < 32
            && observed.iter().any(|m| {
                m.input.kind == Kind::Cc
                    && m.input.channel == first.channel
                    && m.input.number == Some(n + 32)
            });
        if has_lsb {
            input = MidiInput {
                kind: Kind::Cc14,
                ..first
            };
        } else {
            let values: Vec<u16> = observed
                .iter()
                .filter(|m| m.input == first)
                .map(|m| m.value)
                .collect();
            if values
                .iter()
                .all(|v| (1..=15).contains(v) || (113..=127).contains(v))
            {
                encoding = Some(Encoding::TwosComplement);
            } else if values.iter().all(|v| (56..=72).contains(v)) {
                encoding = Some(Encoding::BinaryOffset);
            }
        }
    }
    let mut b = Binding::new(Input::Midi(input), action);
    b.encoding = encoding;
    Some(b)
}
