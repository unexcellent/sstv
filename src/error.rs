use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// An error generated while encoding or decoding.
pub enum Error {
    /// Emitted if now enough pixels could be fetched from the image.
    ///
    /// ```rust
    /// use sstv::{modes, Encoder, Error, RgbPixel};
    ///
    /// let empty_image: Vec<RgbPixel> = vec![];
    ///
    /// assert!(matches!(
    ///     Encoder::new(modes::ROBOT_36, empty_image.into_iter()),
    ///     Err(Error::EmptyImage)
    /// ));
    /// ```
    EmptyImage,
    /// Emitted by [`VisCode::try_new`](crate::VisCode::try_new) if the value
    /// does not fit in 7 bits.
    BadVisCode,
    /// Emitted by `Mode::try_from` if the VIS code does not identify a known
    /// mode.
    UnknownMode,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyImage => write!(
                f,
                "The supplied image is empty. Was the pixel iterator already used?"
            ),
            Self::BadVisCode => write!(f, "VIS codes are 7 bit; the value does not fit."),
            Self::UnknownMode => write!(f, "The VIS code does not identify a known mode."),
        }
    }
}

impl core::error::Error for Error {}

/// Result with the custom sstv Error.
pub type Result<T> = core::result::Result<T, Error>;
