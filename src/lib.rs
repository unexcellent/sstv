#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod encode;
pub mod error;

use error::Error;
use error::Result;
