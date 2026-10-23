//! Own OSC 1.0 codec and the built-in `/cmdmx` schema (CTL-34, 36).

use crate::action::{verb, Action, AttrGroup, Key, KeypadToken, Target};
use crate::convert::Reading;
use crate::ControlError;

#[derive(Debug, Clone, PartialEq)]
pub enum OscArg {
    Int(i32),
    Float(f32),
    Double(f64),
    Long(i64),
    True,
    False,
    Str(String),
}

impl OscArg {
    /// The number, any numeric type; NaN and infinities are not numbers.
    pub fn as_f64(&self) -> Option<f64> {
        let v = match self {
            OscArg::Int(v) => Some(f64::from(*v)),
            OscArg::Float(v) => Some(f64::from(*v)),
            OscArg::Double(v) => Some(*v),
            OscArg::Long(v) => Some(*v as f64),
            OscArg::True => Some(1.0),
            OscArg::False => Some(0.0),
            OscArg::Str(_) => None,
        };
        v.filter(|v| v.is_finite())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OscMessage {
    pub address: String,
    pub args: Vec<OscArg>,
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(s.as_bytes());
    out.push(0);
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

pub fn encode(m: &OscMessage) -> Vec<u8> {
    let mut out = Vec::new();
    put_str(&mut out, &m.address);
    let mut tags = String::from(",");
    let mut data = Vec::new();
    for a in &m.args {
        match a {
            OscArg::Int(v) => {
                tags.push('i');
                data.extend_from_slice(&v.to_be_bytes());
            }
            OscArg::Float(v) => {
                tags.push('f');
                data.extend_from_slice(&v.to_be_bytes());
            }
            OscArg::Double(v) => {
                tags.push('d');
                data.extend_from_slice(&v.to_be_bytes());
            }
            OscArg::Long(v) => {
                tags.push('h');
                data.extend_from_slice(&v.to_be_bytes());
            }
            OscArg::True => tags.push('T'),
            OscArg::False => tags.push('F'),
            OscArg::Str(s) => {
                tags.push('s');
                put_str(&mut data, s);
            }
        }
    }
    put_str(&mut out, &tags);
    out.extend_from_slice(&data);
    out
}

const BUNDLE: &[u8] = b"#bundle\0";
const MAX_DEPTH: u8 = 8;

fn bad<T>(why: &'static str) -> Result<T, ControlError> {
    Err(ControlError::MalformedOsc(why))
}

/// Reads from the front of a packet.
struct Cursor<'a>(&'a [u8]);

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], ControlError> {
        match (self.0.get(..n), self.0.get(n..)) {
            (Some(head), Some(rest)) => {
                self.0 = rest;
                Ok(head)
            }
            _ => bad("truncated"),
        }
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], ControlError> {
        let mut a = [0u8; N];
        a.copy_from_slice(self.take(N)?);
        Ok(a)
    }

    /// A NUL-terminated string padded to a multiple of 4.
    fn string(&mut self) -> Result<&'a str, ControlError> {
        let Some(len) = self.0.iter().position(|b| *b == 0) else {
            return bad("string without NUL");
        };
        let padded = self.take((len / 4 + 1) * 4)?;
        let text = padded.get(..len).unwrap_or_default();
        std::str::from_utf8(text).or_else(|_| bad("string is not UTF-8"))
    }
}

fn decode_message(packet: &[u8]) -> Result<OscMessage, ControlError> {
    let mut c = Cursor(packet);
    let address = c.string()?;
    if !address.starts_with('/') {
        return bad("address without '/'");
    }
    let Some(tags) = c.string()?.strip_prefix(',') else {
        return bad("missing type tags");
    };
    let mut args = Vec::new();
    for tag in tags.chars() {
        args.push(match tag {
            'i' => OscArg::Int(i32::from_be_bytes(c.array()?)),
            'f' => OscArg::Float(f32::from_be_bytes(c.array()?)),
            'd' => OscArg::Double(f64::from_be_bytes(c.array()?)),
            'h' => OscArg::Long(i64::from_be_bytes(c.array()?)),
            'T' => OscArg::True,
            'F' => OscArg::False,
            's' => OscArg::Str(c.string()?.to_string()),
            _ => return bad("unknown type tag"),
        });
    }
    Ok(OscMessage {
        address: address.to_string(),
        args,
    })
}

fn decode_into(packet: &[u8], depth: u8, out: &mut Vec<OscMessage>) -> Result<(), ControlError> {
    let Some(body) = packet.strip_prefix(BUNDLE) else {
        out.push(decode_message(packet)?);
        return Ok(());
    };
    if depth >= MAX_DEPTH {
        return bad("bundles nested too deep");
    }
    let mut c = Cursor(body);
    c.take(8)?; // time tag, ignored
    while !c.0.is_empty() {
        let size = u32::from_be_bytes(c.array()?) as usize;
        if !size.is_multiple_of(4) {
            return bad("bundle element size not a multiple of 4");
        }
        decode_into(c.take(size)?, depth + 1, out)?;
    }
    Ok(())
}

/// A message or a bundle, flattened in order.
pub fn decode(packet: &[u8]) -> Result<Vec<OscMessage>, ControlError> {
    let mut out = Vec::new();
    decode_into(packet, 0, &mut out)?;
    Ok(out)
}

/// Map a `/cmdmx` message to an action and what it says (CTL-36).
pub fn schema(m: &OscMessage) -> Option<(Action, Reading)> {
    let path = m.address.strip_prefix("/cmdmx/")?;
    let seg: Vec<&str> = path.split('/').collect();
    let arg = m.args.first();
    let level = || {
        let v = arg?.as_f64()?;
        Some(Reading::Level(v.clamp(0.0, 1.0) as f32))
    };
    let button = || match arg {
        None => Some(Reading::Tap),
        Some(a) => a.as_f64().map(|v| Reading::Button(v != 0.0)),
    };
    let n = |s: &str| s.parse::<u16>().ok().filter(|v| *v >= 1);
    let id = |s: &str| s.parse::<u32>().ok().filter(|v| *v >= 1);
    Some(match seg.as_slice() {
        ["fader", s, "upper"] => (Action::Upper(Target::Slot(n(s)?)), button()?),
        ["fader", s, "lower"] => (Action::Lower(Target::Slot(n(s)?)), button()?),
        ["fader", s] => (Action::Fader(Target::Slot(n(s)?)), level()?),
        ["fader", p, s] => {
            let t = Target::PageSlot {
                page: n(p)?,
                slot: n(s)?,
            };
            (Action::Fader(t), level()?)
        }
        ["playback", i, "level"] => (Action::Fader(Target::Playback(id(i)?)), level()?),
        ["playback", i, v] => (verb(v, Target::Playback(id(i)?))?, button()?),
        ["gm"] => (Action::Gm, level()?),
        ["page"] => {
            let v = arg?.as_f64().filter(|v| v.fract() == 0.0)?;
            let page = u16::try_from(v as i64).ok().filter(|p| *p >= 1)?;
            (Action::Page(page), Reading::Tap)
        }
        ["page", "next"] => (Action::PageNext, button()?),
        ["page", "prev"] => (Action::PagePrev, button()?),
        ["dbo"] => (Action::Dbo, button()?),
        ["key", k] => (Action::Key(Key::from_name(k)?), button()?),
        ["keypad", t] => (Action::Keypad(KeypadToken::from_name(t)?), button()?),
        ["attribute_group", g] => (Action::AttributeGroup(AttrGroup::from_name(g)?), button()?),
        ["group", i] => (Action::Group(id(i)?), button()?),
        ["palette", i] => (Action::Palette(id(i)?), button()?),
        ["wheel", w] => {
            let w = u8::try_from(n(w)?).ok()?;
            (Action::Wheel(w), Reading::Steps(arg?.as_f64()? as f32))
        }
        [v] => (verb(v, Target::Connected)?, button()?),
        _ => return None,
    })
}
