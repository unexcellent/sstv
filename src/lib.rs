#![cfg_attr(not(test), no_std)]

extern crate alloc;

mod error;

pub mod encoder;
pub mod image;
pub mod modes;
pub mod synthesizer;
pub mod units;

pub use error::{Error, Result};
