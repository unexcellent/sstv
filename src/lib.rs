// The README examples use the `image` and `wav` features; without them the
// crate falls back to a plain description so its doctests always compile.
#![cfg_attr(
    all(feature = "image", feature = "wav"),
    doc = include_str!("../README.md")
)]
#![cfg_attr(
    not(all(feature = "image", feature = "wav")),
    doc = "Slow-Scan Television Encoding With Minimal Memory Usage"
)]
#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

mod error;

#[cfg(feature = "alloc")]
mod decoder;
mod demodulator;
mod encoder;
mod image;
pub mod modes;
mod synthesizer;
mod units;

#[cfg(feature = "alloc")]
pub use decoder::{DecodedImage, Decoder, Event, Events, Images, RgbRow};
pub use demodulator::Demodulator;
pub use encoder::Encoder;
pub use error::{Error, Result};
pub use image::{RgbPixel, YuvPixel};
pub use modes::{Mode, VisCode};
pub use synthesizer::{Synthesizer, Tone};
pub use units::{Duration, Frequency};
