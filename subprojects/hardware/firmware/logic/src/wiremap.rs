//! The fixed wire map: control to CC / note (HW-20, wire-map.md).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    PlaybackFader(u8),
    PresetFader(u8),
    GrandMaster,
    Wheel(u8),
    Upper(u8),
    Lower(u8),
    PresetKey(u8),
    MacroKey(u8),
    WheelPush(u8),
    Dbo,
    Go,
    Back,
    Stop,
    Release,
    PagePrev,
    PageNext,
    Fan,
    AttributeGroup(u8),
    /// 0-based row of the keypad table (note 60 + i).
    Keypad(u8),
}

/// `base + n - 1` for a 1-based `n` in `1..=max`.
fn nth(n: u8, max: u8, base: u8) -> Option<u8> {
    (1..=max).contains(&n).then(|| base + n - 1)
}

/// MSB CC of a fader, relative CC of a wheel.
pub fn cc(c: Control) -> Option<u8> {
    match c {
        Control::PlaybackFader(n) => nth(n, 10, 0),
        Control::PresetFader(n) => nth(n, 10, 10),
        Control::GrandMaster => Some(20),
        Control::Wheel(n) => nth(n, 4, 64),
        _ => None,
    }
}

pub fn note(c: Control) -> Option<u8> {
    match c {
        Control::Upper(n) => nth(n, 10, 0),
        Control::Lower(n) => nth(n, 10, 10),
        Control::PresetKey(n) => nth(n, 10, 20),
        Control::MacroKey(n) => nth(n, 10, 30),
        Control::WheelPush(n) => nth(n, 4, 40),
        Control::Dbo => Some(44),
        Control::Go => Some(45),
        Control::Back => Some(46),
        Control::Stop => Some(47),
        Control::Release => Some(48),
        Control::PagePrev => Some(49),
        Control::PageNext => Some(50),
        Control::Fan => Some(51),
        Control::AttributeGroup(n) => nth(n, 8, 52),
        Control::Keypad(i) => 60u8.checked_add(i).filter(|n| *n <= 127),
        _ => None,
    }
}
