//! Generic MIDI in and out as pure bytes (CTL-09, 10, 17).

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Note,
    Cc,
    Cc14,
    PitchBend,
    Program,
}

/// What a message is about, without its value. `channel` is 1..=16.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MidiInput {
    pub kind: Kind,
    pub channel: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<u8>,
}

impl fmt::Display for MidiInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = self.number.unwrap_or(0);
        let c = self.channel;
        match self.kind {
            Kind::Note => write!(f, "Note {n} ch {c}"),
            Kind::Cc => write!(f, "CC {n} ch {c}"),
            Kind::Cc14 => write!(f, "CC14 {n} ch {c}"),
            Kind::PitchBend => write!(f, "Pitch bend ch {c}"),
            Kind::Program => write!(f, "Program {n} ch {c}"),
        }
    }
}

/// `value` is 7-bit, or 14-bit for `Cc14` and `PitchBend`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MidiMessage {
    pub input: MidiInput,
    pub value: u16,
}

/// Stateful byte decoder: running status and CC14 pairs live across packets.
#[derive(Debug, Clone, Default)]
pub struct Decoder {
    /// (channel 1-based, MSB cc 0..31) pairs that are 14-bit.
    cc14: Vec<(u8, u8)>,
    status: Option<u8>,
    /// Inside a system message (sysex and friends): its data bytes are not ours.
    system: bool,
    /// Packets with stray data bytes, and messages cut off at a packet's end (CTL-15).
    malformed: u64,
    msb: HashMap<(u8, u8), u8>,
}

impl Decoder {
    pub fn new(cc14: Vec<(u8, u8)>) -> Decoder {
        Decoder {
            cc14,
            ..Decoder::default()
        }
    }

    /// How many malformed packets and messages were seen (CTL-15).
    pub fn malformed(&self) -> u64 {
        self.malformed
    }

    pub fn decode(&mut self, packet: &[u8]) -> Vec<MidiMessage> {
        let mut out = Vec::new();
        let mut stray = false;
        let mut data: Vec<u8> = Vec::new();
        // MSBs seen in this packet that still wait for their LSB.
        let mut open: Vec<(u8, u8)> = Vec::new();
        for &b in packet {
            match b {
                0xF8..=0xFF => {}
                0xF0..=0xF7 => {
                    self.status = None;
                    self.system = b != 0xF7;
                    data.clear();
                }
                0x80..=0xEF => {
                    self.status = Some(b);
                    self.system = false;
                    data.clear();
                }
                _ => {
                    let Some(status) = self.status else {
                        stray |= !self.system;
                        continue;
                    };
                    data.push(b);
                    let need = if matches!(status >> 4, 0xC | 0xD) {
                        1
                    } else {
                        2
                    };
                    if data.len() == need {
                        self.message(status, &data, &mut out, &mut open);
                        data.clear();
                    }
                }
            }
        }
        self.malformed += u64::from(stray) + u64::from(!data.is_empty());
        for (ch, n) in open {
            let msb = self.msb.get(&(ch, n)).copied().unwrap_or(0);
            out.push(cc14_message(ch, n, u16::from(msb) << 7));
        }
        out
    }

    fn message(
        &mut self,
        status: u8,
        data: &[u8],
        out: &mut Vec<MidiMessage>,
        open: &mut Vec<(u8, u8)>,
    ) {
        let ch = (status & 0x0F) + 1;
        let msg = |kind, number, value| MidiMessage {
            input: MidiInput {
                kind,
                channel: ch,
                number,
            },
            value,
        };
        match (status >> 4, data) {
            (0x8, [n, _]) => out.push(msg(Kind::Note, Some(*n), 0)),
            (0x9, [n, v]) => out.push(msg(Kind::Note, Some(*n), u16::from(*v))),
            (0xB, [n, v]) => {
                if self.cc14.contains(&(ch, *n)) {
                    self.msb.insert((ch, *n), *v);
                    if !open.contains(&(ch, *n)) {
                        open.push((ch, *n));
                    }
                } else if let Some(m) = n.checked_sub(32).filter(|m| self.cc14.contains(&(ch, *m)))
                {
                    let msb = self.msb.get(&(ch, m)).copied().unwrap_or(0);
                    open.retain(|o| *o != (ch, m));
                    out.push(cc14_message(ch, m, (u16::from(msb) << 7) | u16::from(*v)));
                } else {
                    out.push(msg(Kind::Cc, Some(*n), u16::from(*v)));
                }
            }
            (0xC, [n]) => out.push(msg(Kind::Program, Some(*n), 127)),
            (0xE, [lsb, msb]) => out.push(msg(
                Kind::PitchBend,
                None,
                (u16::from(*msb) << 7) | u16::from(*lsb),
            )),
            // Aftertouch and channel pressure are decoded and dropped.
            _ => {}
        }
    }
}

fn cc14_message(channel: u8, number: u8, value: u16) -> MidiMessage {
    MidiMessage {
        input: MidiInput {
            kind: Kind::Cc14,
            channel,
            number: Some(number),
        },
        value,
    }
}

/// Bytes for feedback (CTL-17). Values are clamped to the kind's range.
pub fn encode(input: &MidiInput, value: u16) -> Vec<u8> {
    let c = input.channel.clamp(1, 16) - 1;
    let n = input.number.unwrap_or(0).min(127);
    let v7 = value.min(127) as u8;
    match input.kind {
        Kind::Note => vec![0x90 | c, n, v7],
        Kind::Cc => vec![0xB0 | c, n, v7],
        Kind::Cc14 => {
            let v = value.min(16383);
            vec![
                0xB0 | c,
                n,
                (v >> 7) as u8,
                0xB0 | c,
                n.saturating_add(32).min(127),
                (v & 0x7F) as u8,
            ]
        }
        Kind::PitchBend => {
            let v = value.min(16383);
            vec![0xE0 | c, (v & 0x7F) as u8, (v >> 7) as u8]
        }
        Kind::Program => Vec::new(),
    }
}

/// Level 0..=1 to the raw value of `kind`.
pub fn to_raw(kind: Kind, level: f32) -> u16 {
    let max = match kind {
        Kind::Cc14 | Kind::PitchBend => 16383.0,
        _ => 127.0,
    };
    (level.clamp(0.0, 1.0) * max).round() as u16
}
