use core::fmt;

#[derive(Debug)]
/// An error generated while encoding or decoding.
#[non_exhaustive]
pub enum Error {
    /// Emitted if now enough pixels could be fetched from the image.
    ///
    /// ```rust
    /// use sstv::{Encoder, Error, Mode, RgbPixel};
    ///
    /// let empty_image: Vec<RgbPixel> = vec![];
    ///
    /// assert!(matches!(
    ///     Encoder::new(Mode::Robot36, empty_image.into_iter()),
    ///     Err(Error::EmptyImage)
    /// ));
    /// ```
    EmptyImage,
    /// The image could not be read or decoded.
    #[cfg(feature = "image")]
    Image(image::ImageError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyImage => write!(
                f,
                "The supplied image is empty. Was the pixel iterator already used?"
            ),
            #[cfg(feature = "image")]
            Self::Image(error) => write!(f, "The image could not be loaded: {error}"),
        }
    }
}

impl core::error::Error for Error {}

#[cfg(feature = "image")]
impl From<image::ImageError> for Error {
    fn from(error: image::ImageError) -> Self {
        Self::Image(error)
    }
}

/// Result with the custom sstv Error.
pub type Result<T> = core::result::Result<T, Error>;
