//! Errors of the generic control layer. They are values; the daemon shows them as warnings.

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ControlError {
    #[error("unknown action \"{0}\"")]
    UnknownAction(String),
    #[error("malformed OSC packet: {0}")]
    MalformedOsc(&'static str),
    #[error("malformed HID report: {0}")]
    MalformedHid(&'static str),
    #[error("invalid mapping: {0}")]
    InvalidMapping(String),
}
