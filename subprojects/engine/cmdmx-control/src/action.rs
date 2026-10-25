//! The action catalogue (CTL-05): what an input can do, as one line of text.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::ControlError;

/// How an action wants its value (CTL-07).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    Continuous,
    Button,
    Relative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// The connected playback.
    Connected,
    Slot(u16),
    PageSlot {
        page: u16,
        slot: u16,
    },
    Playback(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Clear,
    Locate,
    Highlight,
    RemDim,
    Undo,
    Redo,
    All,
    StepNext,
    StepPrev,
}

const KEYS: [(&str, Key); 9] = [
    ("clear", Key::Clear),
    ("locate", Key::Locate),
    ("highlight", Key::Highlight),
    ("rem_dim", Key::RemDim),
    ("undo", Key::Undo),
    ("redo", Key::Redo),
    ("all", Key::All),
    ("step_next", Key::StepNext),
    ("step_prev", Key::StepPrev),
];

impl Key {
    pub fn from_name(s: &str) -> Option<Key> {
        KEYS.iter().find(|(n, _)| *n == s).map(|(_, k)| *k)
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = KEYS.iter().find(|(_, k)| k == self).map_or("", |(n, _)| n);
        f.write_str(name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeypadToken {
    Digit(u8),
    Dot,
    At,
    Thro,
    And,
    Not,
    Fixture,
    Group,
    Palette,
    Full,
    Enter,
    Backspace,
}

const TOKENS: [(&str, KeypadToken); 11] = [
    (".", KeypadToken::Dot),
    ("@", KeypadToken::At),
    ("Thro", KeypadToken::Thro),
    ("And", KeypadToken::And),
    ("Not", KeypadToken::Not),
    ("Fixture", KeypadToken::Fixture),
    ("Group", KeypadToken::Group),
    ("Palette", KeypadToken::Palette),
    ("Full", KeypadToken::Full),
    ("Enter", KeypadToken::Enter),
    ("Backspace", KeypadToken::Backspace),
];

impl KeypadToken {
    pub fn from_name(s: &str) -> Option<KeypadToken> {
        let mut chars = s.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            if let Some(d) = c.to_digit(10) {
                return u8::try_from(d).ok().map(KeypadToken::Digit);
            }
        }
        TOKENS.iter().find(|(n, _)| *n == s).map(|(_, t)| *t)
    }
}

impl fmt::Display for KeypadToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let KeypadToken::Digit(d) = self {
            return write!(f, "{d}");
        }
        let name = TOKENS
            .iter()
            .find(|(_, t)| t == self)
            .map_or("", |(n, _)| n);
        f.write_str(name)
    }
}

/// Attribute groups: Intensity, Position, Colour, Gobo, Beam, Effect, Shape, FX.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttrGroup {
    I,
    P,
    C,
    G,
    B,
    E,
    S,
    FX,
}

const GROUPS: [(&str, AttrGroup); 8] = [
    ("I", AttrGroup::I),
    ("P", AttrGroup::P),
    ("C", AttrGroup::C),
    ("G", AttrGroup::G),
    ("B", AttrGroup::B),
    ("E", AttrGroup::E),
    ("S", AttrGroup::S),
    ("FX", AttrGroup::FX),
];

impl AttrGroup {
    pub fn from_name(s: &str) -> Option<AttrGroup> {
        GROUPS.iter().find(|(n, _)| *n == s).map(|(_, g)| *g)
    }
}

impl fmt::Display for AttrGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = GROUPS
            .iter()
            .find(|(_, g)| g == self)
            .map_or("", |(n, _)| n);
        f.write_str(name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Fader(Target),
    Upper(Target),
    Lower(Target),
    Go(Target),
    Back(Target),
    Stop(Target),
    Release(Target),
    Tap(Target),
    PageNext,
    PagePrev,
    Page(u16),
    /// Turn a Stream Deck's own page (CTL-48); not the console page.
    DeckPageNext,
    DeckPagePrev,
    DeckPage(u16),
    /// Move a Mackie unit's window over the page (CTL-32).
    ChannelNext,
    ChannelPrev,
    Gm,
    Dbo,
    Key(Key),
    Keypad(KeypadToken),
    Wheel(u8),
    AttributeGroup(AttrGroup),
    Group(u32),
    Palette(u32),
}

/// A number of at least 1 (0 is refused everywhere).
fn num<T: FromStr + PartialOrd + From<u8>>(s: &str) -> Option<T> {
    s.parse::<T>().ok().filter(|n| *n >= T::from(1))
}

/// `3` or `2.3` (page.slot).
fn slot_target(s: &str) -> Option<Target> {
    match s.split_once('.') {
        Some((p, sl)) => Some(Target::PageSlot {
            page: num(p)?,
            slot: num(sl)?,
        }),
        None => num(s).map(Target::Slot),
    }
}

/// The go/back/stop/release/tap family, shared with the OSC schema (CTL-36).
pub(crate) fn verb(name: &str, t: Target) -> Option<Action> {
    Some(match name {
        "go" => Action::Go(t),
        "back" => Action::Back(t),
        "stop" => Action::Stop(t),
        "release" => Action::Release(t),
        "tap" => Action::Tap(t),
        _ => return None,
    })
}

fn parse_action(s: &str) -> Option<Action> {
    let words: Vec<&str> = s.split_whitespace().collect();
    Some(match words.as_slice() {
        ["fader", t] => Action::Fader(slot_target(t)?),
        ["playback", id, "level"] => Action::Fader(Target::Playback(num(id)?)),
        ["upper", t] => Action::Upper(slot_target(t)?),
        ["lower", t] => Action::Lower(slot_target(t)?),
        ["page", "next"] => Action::PageNext,
        ["page", "prev"] => Action::PagePrev,
        ["page", n] => Action::Page(num(n)?),
        ["deck", "page", "next"] => Action::DeckPageNext,
        ["deck", "page", "prev"] => Action::DeckPagePrev,
        ["deck", "page", n] => Action::DeckPage(num(n)?),
        ["channel", "next"] => Action::ChannelNext,
        ["channel", "prev"] => Action::ChannelPrev,
        ["gm"] => Action::Gm,
        ["dbo"] => Action::Dbo,
        ["key", k] => Action::Key(Key::from_name(k)?),
        ["keypad", t] => Action::Keypad(KeypadToken::from_name(t)?),
        ["wheel", n] => Action::Wheel(num(n)?),
        ["attribute_group", g] => Action::AttributeGroup(AttrGroup::from_name(g)?),
        ["group", id] => Action::Group(num(id)?),
        ["palette", id] => Action::Palette(num(id)?),
        [v] => verb(v, Target::Connected)?,
        [v, "playback", id] => verb(v, Target::Playback(num(id)?))?,
        [v, t] => verb(v, slot_target(t)?)?,
        _ => return None,
    })
}

impl FromStr for Action {
    type Err = ControlError;

    fn from_str(s: &str) -> Result<Action, ControlError> {
        parse_action(s).ok_or_else(|| ControlError::UnknownAction(s.to_string()))
    }
}

impl Action {
    /// True when the action addresses a playback by page slot or id, so it does not follow the
    /// console page (CTL-50).
    pub fn is_fixed(&self) -> bool {
        matches!(
            self,
            Action::Fader(t)
                | Action::Upper(t)
                | Action::Lower(t)
                | Action::Go(t)
                | Action::Back(t)
                | Action::Stop(t)
                | Action::Release(t)
                | Action::Tap(t)
                if matches!(t, Target::PageSlot { .. } | Target::Playback(_))
        )
    }

    pub fn value_kind(&self) -> ValueKind {
        match self {
            Action::Fader(_) | Action::Gm => ValueKind::Continuous,
            Action::Wheel(_) => ValueKind::Relative,
            _ => ValueKind::Button,
        }
    }
}

struct Arg(Target);

impl fmt::Display for Arg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Target::Connected => Ok(()),
            Target::Slot(s) => write!(f, " {s}"),
            Target::PageSlot { page, slot } => write!(f, " {page}.{slot}"),
            Target::Playback(id) => write!(f, " playback {id}"),
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::Fader(Target::Playback(id)) => write!(f, "playback {id} level"),
            Action::Fader(t) => write!(f, "fader{}", Arg(*t)),
            Action::Upper(t) => write!(f, "upper{}", Arg(*t)),
            Action::Lower(t) => write!(f, "lower{}", Arg(*t)),
            Action::Go(t) => write!(f, "go{}", Arg(*t)),
            Action::Back(t) => write!(f, "back{}", Arg(*t)),
            Action::Stop(t) => write!(f, "stop{}", Arg(*t)),
            Action::Release(t) => write!(f, "release{}", Arg(*t)),
            Action::Tap(t) => write!(f, "tap{}", Arg(*t)),
            Action::PageNext => f.write_str("page next"),
            Action::PagePrev => f.write_str("page prev"),
            Action::Page(n) => write!(f, "page {n}"),
            Action::DeckPageNext => f.write_str("deck page next"),
            Action::DeckPagePrev => f.write_str("deck page prev"),
            Action::DeckPage(n) => write!(f, "deck page {n}"),
            Action::ChannelNext => f.write_str("channel next"),
            Action::ChannelPrev => f.write_str("channel prev"),
            Action::Gm => f.write_str("gm"),
            Action::Dbo => f.write_str("dbo"),
            Action::Key(k) => write!(f, "key {k}"),
            Action::Keypad(t) => write!(f, "keypad {t}"),
            Action::Wheel(n) => write!(f, "wheel {n}"),
            Action::AttributeGroup(g) => write!(f, "attribute_group {g}"),
            Action::Group(id) => write!(f, "group {id}"),
            Action::Palette(id) => write!(f, "palette {id}"),
        }
    }
}

impl Serialize for Action {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Action {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Action, D::Error> {
        String::deserialize(d)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}
