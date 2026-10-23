//! Mackie Control Universal — the subset ColourmotionDMX uses.
//!
//! Wire format (MCU is plain MIDI on one port):
//! - Fader n (0..=7): pitch bend on MIDI channel n, 14-bit value, of which the
//!   top 10 bits are meaningful. Master fader is channel 8. The host echoes the
//!   same message to move a motor fader.
//! - Buttons: note on, MIDI channel 0; velocity 0x7F = pressed, 0x00 = released.
//!   Note map: REC 0..=7, SOLO 8..=15, MUTE 16..=23, SELECT 24..=31,
//!   V-Pot push 32..=39, Bank ◀ 46, Bank ▶ 47, Channel ◀ 48, Channel ▶ 49,
//!   fader touch 104..=112.
//! - V-Pots: CC 16..=23 relative: bits 0..=5 magnitude, bit 6 set = decrement.
//!   Ring LEDs: CC 48..=55.
//!
//! Everything here is verified against the public MCU reverse-engineering
//! notes and must be confirmed on the BCF2000 (tests/hardware/bcf2000.md).

use crate::{Button, SurfaceEvent};

pub const CHANNELS: u8 = 8;
pub const PITCH_BEND: u8 = 0xE0;
pub const NOTE_ON: u8 = 0x90;
pub const CONTROL_CHANGE: u8 = 0xB0;

pub const NOTE_REC: u8 = 0;
pub const NOTE_SOLO: u8 = 8;
pub const NOTE_MUTE: u8 = 16;
pub const NOTE_SELECT: u8 = 24;
pub const NOTE_VPOT_PUSH: u8 = 32;
pub const NOTE_BANK_LEFT: u8 = 46;
pub const NOTE_BANK_RIGHT: u8 = 47;
pub const NOTE_CHANNEL_LEFT: u8 = 48;
pub const NOTE_CHANNEL_RIGHT: u8 = 49;
pub const NOTE_FADER_TOUCH: u8 = 104;

pub const CC_VPOT: u8 = 16;
pub const CC_VPOT_RING: u8 = 48;

/// Full-scale 10-bit fader value as MCU devices report it.
pub const FADER_MAX_10BIT: u16 = 0x3FF;

/// Decode one complete MIDI message (status + data bytes). Returns `None` for
/// messages the surface model does not cover (never an error; unknown input
/// is simply ignored).
pub fn decode(message: &[u8]) -> Option<SurfaceEvent> {
    let (&status, rest) = message.split_first()?;
    let kind = status & 0xF0;
    let channel = status & 0x0F;
    match kind {
        PITCH_BEND => {
            let lsb = *rest.first()?;
            let msb = *rest.get(1)?;
            let value14 = (u16::from(msb & 0x7F) << 7) | u16::from(lsb & 0x7F);
            let level = level(value14);
            if channel < CHANNELS {
                Some(SurfaceEvent::Fader {
                    index: channel,
                    level,
                })
            } else if channel == CHANNELS {
                Some(SurfaceEvent::MasterFader { level })
            } else {
                None
            }
        }
        NOTE_ON if channel == 0 => {
            let note = *rest.first()?;
            let velocity = *rest.get(1)?;
            let pressed = velocity > 0;
            if (NOTE_FADER_TOUCH..NOTE_FADER_TOUCH + CHANNELS).contains(&note) {
                return Some(SurfaceEvent::FaderTouch {
                    index: note - NOTE_FADER_TOUCH,
                    touched: pressed,
                });
            }
            Some(SurfaceEvent::Button {
                button: button_for_note(note),
                pressed,
            })
        }
        CONTROL_CHANGE if channel == 0 => {
            let cc = *rest.first()?;
            let value = *rest.get(1)?;
            if (CC_VPOT..CC_VPOT + CHANNELS).contains(&cc) {
                let magnitude = i8::try_from(value & 0x3F).ok()?;
                let delta = if value & 0x40 != 0 {
                    -magnitude
                } else {
                    magnitude
                };
                return Some(SurfaceEvent::Encoder {
                    index: cc - CC_VPOT,
                    delta,
                });
            }
            None
        }
        _ => None,
    }
}

fn button_for_note(note: u8) -> Button {
    match note {
        n if n < NOTE_SOLO => Button::ChannelUpper(n - NOTE_REC),
        n if n < NOTE_MUTE => Button::ChannelSolo(n - NOTE_SOLO),
        n if n < NOTE_SELECT => Button::ChannelMute(n - NOTE_MUTE),
        n if n < NOTE_VPOT_PUSH => Button::ChannelSelect(n - NOTE_SELECT),
        n if n < NOTE_VPOT_PUSH + CHANNELS => Button::EncoderPush(n - NOTE_VPOT_PUSH),
        NOTE_BANK_LEFT => Button::BankLeft,
        NOTE_BANK_RIGHT => Button::BankRight,
        NOTE_CHANNEL_LEFT => Button::ChannelLeft,
        NOTE_CHANNEL_RIGHT => Button::ChannelRight,
        n => Button::Other(n),
    }
}

/// A fader's 14-bit value as a level: only the top 10 bits count, so full travel is exactly 1.0 (CTL-01).
pub fn level(value14: u16) -> f32 {
    f32::from(value14.min(0x3FFF) >> 4) / f32::from(FADER_MAX_10BIT)
}

/// The 14-bit pitch-bend value that puts a motor fader at `level` (CTL-02).
pub fn position(level: f32) -> u16 {
    ((level.clamp(0.0, 1.0) * f32::from(FADER_MAX_10BIT)).round() as u16) << 4
}

/// The fader channel (1..=9) a touch note (104..=112) belongs to (CTL-20).
pub fn touch(note: u8) -> Option<u8> {
    note.checked_sub(NOTE_FADER_TOUCH)
        .filter(|n| *n <= CHANNELS)
        .map(|n| n + 1)
}

/// Ring CC value in single-dot mode: off without a wheel, else dot 1..=11 (CTL-33).
pub fn ring(level: Option<f32>) -> u16 {
    level.map_or(0, |l| 1 + (l.clamp(0.0, 1.0) * 10.0).round() as u16)
}

/// Which playback slots the Mackie units show: `offset` slots into the page (CTL-31, 32).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub offset: u16,
}

impl Window {
    /// The page slot behind unit position `pos` (1-based, base included); `None` when unbound.
    pub fn slot(&self, pos: u16, faders_per_page: u16) -> Option<u16> {
        let s = self.offset.saturating_add(pos);
        (pos >= 1 && s <= faders_per_page).then_some(s)
    }

    /// Channel ◀ / ▶: move by `width`, stopping at 0 and at the last window (CTL-32).
    pub fn channel(&mut self, forward: bool, width: u16, faders_per_page: u16) {
        if forward {
            self.offset = self.offset.saturating_add(width);
        } else {
            self.offset = self.offset.saturating_sub(width);
        }
        self.clamp(width, faders_per_page);
    }

    /// Keep the offset on a window start that exists (the page or the width shrank).
    pub fn clamp(&mut self, width: u16, faders_per_page: u16) {
        let last = faders_per_page.saturating_sub(1).checked_div(width);
        self.offset = last.map_or(0, |n| self.offset.min(n * width));
    }

    pub fn reset(&mut self) {
        self.offset = 0;
    }
}

/// Move motor fader `index` (0..=7, or 8 for master) to `level` 0.0..=1.0.
pub fn encode_fader_position(index: u8, level: f32) -> Option<[u8; 3]> {
    if index > CHANNELS {
        return None;
    }
    let value14 = position(level);
    Some([
        PITCH_BEND | index,
        (value14 & 0x7F) as u8,
        ((value14 >> 7) & 0x7F) as u8,
    ])
}

/// Light or extinguish a button LED by its MCU note number.
pub fn encode_led(note: u8, on: bool) -> [u8; 3] {
    [NOTE_ON, note & 0x7F, if on { 0x7F } else { 0x00 }]
}

/// Ring LED display around V-Pot `index` (0..=7).
///
/// `position` 0..=11 selects the lit LED; `mode` selects the drawing style
/// (0 single dot, 1 boost/cut from centre, 2 wrap/fill, 3 spread).
pub fn encode_vpot_ring(index: u8, mode: u8, position: u8) -> Option<[u8; 3]> {
    if index >= CHANNELS {
        return None;
    }
    let value = ((mode & 0x03) << 4) | (position.min(11) & 0x0F);
    Some([CONTROL_CHANGE, CC_VPOT_RING + index, value])
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn ctl_01_fader_pitch_bend_decodes_to_level() {
        // Channel 3, 14-bit 0x3FF0 (top 10 bits full) → 1.0
        assert_eq!(
            decode(&[0xE2, 0x70, 0x7F]),
            Some(SurfaceEvent::Fader {
                index: 2,
                level: 1.0
            })
        );
        // Zero
        assert_eq!(
            decode(&[0xE0, 0x00, 0x00]),
            Some(SurfaceEvent::Fader {
                index: 0,
                level: 0.0
            })
        );
        // Master fader on channel 9
        assert!(matches!(
            decode(&[0xE8, 0x00, 0x40]),
            Some(SurfaceEvent::MasterFader { .. })
        ));
    }

    #[test]
    fn ctl_02_fader_position_round_trips_through_encode_and_decode() {
        for &level in &[0.0f32, 0.25, 0.5, 0.75, 1.0] {
            let msg = encode_fader_position(4, level).unwrap();
            let back = decode(&msg).unwrap();
            match back {
                SurfaceEvent::Fader { index, level: l } => {
                    assert_eq!(index, 4);
                    assert!((l - level).abs() < 1.0 / 1023.0, "{level} → {l}");
                }
                other => panic_free_fail(&format!("unexpected {other:?}")),
            }
        }
        assert_eq!(encode_fader_position(9, 0.5), None);
    }

    #[test]
    fn ctl_03_buttons_map_to_rows_and_navigation() {
        assert_eq!(
            decode(&[0x90, 5, 0x7F]),
            Some(SurfaceEvent::Button {
                button: Button::ChannelUpper(5),
                pressed: true
            })
        );
        assert_eq!(
            decode(&[0x90, 27, 0x00]),
            Some(SurfaceEvent::Button {
                button: Button::ChannelSelect(3),
                pressed: false
            })
        );
        assert_eq!(
            decode(&[0x90, 47, 0x7F]),
            Some(SurfaceEvent::Button {
                button: Button::BankRight,
                pressed: true
            })
        );
        assert_eq!(
            decode(&[0x90, 106, 0x7F]),
            Some(SurfaceEvent::FaderTouch {
                index: 2,
                touched: true
            })
        );
    }

    #[test]
    fn ctl_04_vpot_relative_cc_decodes_signed_delta() {
        assert_eq!(
            decode(&[0xB0, 16, 0x01]),
            Some(SurfaceEvent::Encoder { index: 0, delta: 1 })
        );
        assert_eq!(
            decode(&[0xB0, 23, 0x43]),
            Some(SurfaceEvent::Encoder {
                index: 7,
                delta: -3
            })
        );
        assert_eq!(encode_vpot_ring(1, 2, 6), Some([0xB0, 49, 0x26]));
        assert_eq!(encode_vpot_ring(8, 0, 0), None);
    }

    #[test]
    fn unknown_or_truncated_messages_are_ignored_not_errors() {
        assert_eq!(decode(&[]), None);
        assert_eq!(decode(&[0xE0]), None);
        assert_eq!(decode(&[0xF8]), None); // MIDI clock
        assert_eq!(decode(&[0xB1, 16, 1]), None); // CC on another channel
    }

    #[allow(clippy::panic)]
    fn panic_free_fail<T>(msg: &str) -> T {
        panic!("{msg}")
    }
}
