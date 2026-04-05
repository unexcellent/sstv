#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod encode;
pub mod encoder;
pub mod error;
pub mod image;
pub mod modes;
pub mod synthesizer;
pub mod units;

use error::Error;
use error::Result;
