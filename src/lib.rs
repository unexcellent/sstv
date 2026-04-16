#![doc = "Slow-Scan Television Encoding With Minimal Memory Usage"]
#![cfg_attr(not(test), no_std)]
#![warn(missing_docs)]

extern crate alloc;

mod error;

mod encoder;
mod image;
mod modes;
mod synthesizer;
mod units;

pub use encoder::Encoder;
pub use error::{Error, Result};
pub use image::{RgbPixel, YuvPixel};
pub use modes::Mode;
pub use synthesizer::Tone;
pub use units::{Duration, Frequency};
