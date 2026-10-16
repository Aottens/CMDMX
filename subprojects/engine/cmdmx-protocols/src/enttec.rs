//! ENTTEC DMX USB Pro API framing (also spoken by DMXking and the
//! Eurolite USB-DMX512 PRO Interface MK2 — see OUT-09).
//!
//! Every message:
//! ```text
//! 0x7E  label  len_lsb  len_msb  <data …>  0xE7
//! ```
//! Label 6 "Output Only Send DMX Packet Request": data = start code + slots.
//! The device wants at least 24 slots in a frame (data length ≥ 25 incl.
//! start code); we pad shorter frames with zeros.

use crate::{FrameError, DMX_START_CODE, MAX_CHANNELS};

pub const START_OF_MESSAGE: u8 = 0x7E;
pub const END_OF_MESSAGE: u8 = 0xE7;
pub const LABEL_SEND_DMX: u8 = 6;
pub const LABEL_GET_WIDGET_PARAMS: u8 = 3;
pub const LABEL_GET_SERIAL: u8 = 10;
pub const MIN_CHANNELS: usize = 24;

/// Serial settings for the macOS VCP device. The FTDI chip re-times the DMX
/// line itself; the host baud rate only governs USB-serial transfer.
pub const VCP_BAUD: u32 = 115_200;

/// USB product-string fragments that identify ENTTEC-Pro-protocol devices
/// sharing the generic FTDI VID/PID 0403:6001. Match case-insensitively.
pub const KNOWN_PRODUCT_STRINGS: &[&str] = &["DMX USB PRO", "DMX512 Pro", "ultraDMX"];

/// Does this USB product string belong to a device we should drive as ENTTEC Pro?
pub fn is_pro_product_string(product: &str) -> bool {
    let p = product.to_ascii_lowercase();
    KNOWN_PRODUCT_STRINGS
        .iter()
        .any(|k| p.contains(&k.to_ascii_lowercase()))
}

fn message(label: u8, data: &[u8]) -> Vec<u8> {
    let len = data.len() as u16;
    let mut m = Vec::with_capacity(data.len() + 5);
    m.push(START_OF_MESSAGE);
    m.push(label);
    m.extend_from_slice(&len.to_le_bytes());
    m.extend_from_slice(data);
    m.push(END_OF_MESSAGE);
    m
}

/// Label 6: send one DMX frame. `slots` is the universe without start code.
pub fn send_dmx_frame(slots: &[u8]) -> Result<Vec<u8>, FrameError> {
    if slots.len() > MAX_CHANNELS {
        return Err(FrameError::TooManyChannels(slots.len()));
    }
    let mut data = Vec::with_capacity(1 + slots.len().max(MIN_CHANNELS));
    data.push(DMX_START_CODE);
    data.extend_from_slice(slots);
    if slots.len() < MIN_CHANNELS {
        data.resize(1 + MIN_CHANNELS, 0);
    }
    Ok(message(LABEL_SEND_DMX, &data))
}

/// Label 3 request: ask the widget for its parameters (firmware, break
/// length, MAB, refresh rate). Used at connect to confirm the device talks Pro.
pub fn get_widget_parameters_request() -> Vec<u8> {
    message(LABEL_GET_WIDGET_PARAMS, &[0, 0])
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn out_08_full_frame_is_byte_exact() {
        let slots = [0x55u8; 512];
        let m = send_dmx_frame(&slots).unwrap();
        assert_eq!(m.len(), 5 + 513);
        assert_eq!(m[0], 0x7E);
        assert_eq!(m[1], 6);
        assert_eq!(&m[2..4], &[0x01, 0x02]); // 513 little-endian
        assert_eq!(m[4], 0x00); // start code
        assert!(m[5..517].iter().all(|&b| b == 0x55));
        assert_eq!(m[517], 0xE7);
    }

    #[test]
    fn out_08_short_frame_is_padded_to_24_channels() {
        let m = send_dmx_frame(&[255, 128]).unwrap();
        assert_eq!(&m[2..4], &[25, 0]);
        assert_eq!(&m[4..7], &[0x00, 255, 128]);
        assert!(m[7..29].iter().all(|&b| b == 0));
        assert_eq!(m[29], 0xE7);
    }

    #[test]
    fn out_08_rejects_oversized_frame() {
        assert_eq!(
            send_dmx_frame(&[0u8; 513]),
            Err(FrameError::TooManyChannels(513))
        );
    }

    #[test]
    fn out_09_eurolite_mk2_product_string_is_recognised_as_pro() {
        assert!(is_pro_product_string("Eurolite DMX512 Pro"));
        assert!(is_pro_product_string("DMX USB PRO"));
        assert!(is_pro_product_string("DMXking ultraDMX Micro"));
        assert!(!is_pro_product_string("FT232R USB UART")); // Open DMX clone: never Pro
    }

    #[test]
    fn widget_parameters_request_shape() {
        assert_eq!(
            get_widget_parameters_request(),
            vec![0x7E, 3, 2, 0, 0, 0, 0xE7]
        );
    }
}
