//! Error types for the SSTV library

use thiserror::Error;

/// Errors that can occur during SSTV encoding
#[derive(Error, Debug)]
pub enum SstvError {
    /// Image dimensions don't match the mode requirements
    #[error("Image dimensions {actual_width}x{actual_height} don't match mode requirements {expected_width}x{expected_height}")]
    DimensionMismatch {
        expected_width: u32,
        expected_height: u32,
        actual_width: u32,
        actual_height: u32,
    },

    /// Invalid sample rate
    #[error("Invalid sample rate: {0}. Must be positive.")]
    InvalidSampleRate(u32),

    /// Invalid pixel value
    #[error("Invalid pixel value: {0}")]
    InvalidPixel(u8),

    /// Invalid mode name
    #[error("Unknown SSTV mode: {0}")]
    UnknownMode(String),

    /// Invalid VIS code
    #[error("Invalid VIS code: {0:#04x}")]
    InvalidVisCode(u8),

    /// Encoding error
    #[error("Encoding error: {0}")]
    EncodingError(String),

    /// Internal error
    #[error("Internal error: {0}")]
    InternalError(String),
}

/// Result type alias for SSTV operations
pub type Result<T> = std::result::Result<T, SstvError>;
