//! Pure logic of the desk firmware (HW-70): no I/O, no allocation, unit-tested on the host.
#![no_std]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

pub mod boxmap;
pub mod encoder;
pub mod fader;
pub mod keys;
pub mod link;
pub mod midi;
pub mod palette;
pub mod wiremap;
