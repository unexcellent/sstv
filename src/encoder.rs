use alloc::boxed::Box;

use crate::Result;
use crate::image::RgbPixel;
use crate::modes::Mode;
use crate::synthesizer::Tone;

/// `Encoder` is the main struct for converting an image into SSTV tones.
///
/// Construct `Encoder` with your desired mode and an iterator over the pixels you want to encode.
/// ```rust
/// use sstv::{Encoder, Error, Mode, RgbPixel};
///
/// let image = [RgbPixel::new(0, 0, 0); 320];
/// let encoder = Encoder::new(Mode::Robot36, image.into_iter());
/// for tone in encoder {
///     // emit or save the tones
/// }
/// ```
pub struct Encoder {
    inner: Box<dyn Iterator<Item = Tone>>,
}

impl Encoder {
    /// Construct an `Encoder` from the mode and a pixel iterator.
    pub fn new<I>(mode: Mode, pixels: I) -> Result<Self>
    where
        I: Iterator<Item = RgbPixel> + 'static,
    {
        Ok(Self {
            inner: mode.encoder(pixels)?,
        })
    }
}

impl Iterator for Encoder {
    type Item = Tone;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}
