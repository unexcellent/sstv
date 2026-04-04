use alloc::string::String;
use core::fmt;

#[derive(Debug)]
pub enum Error {
    DimensionMismatch {
        expected_width: u32,
        expected_height: u32,
        actual_width: u32,
        actual_height: u32,
    },
    EncodingError(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DimensionMismatch {
                expected_width,
                expected_height,
                actual_width,
                actual_height,
            } => write!(
                f,
                "Image dimensions {}x{} don't match mode requirements {}x{}",
                actual_width, actual_height, expected_width, expected_height
            ),
            Self::EncodingError(msg) => write!(f, "Encoding error: {}", msg),
        }
    }
}

impl core::error::Error for Error {}

pub type Result<T> = core::result::Result<T, Error>;
