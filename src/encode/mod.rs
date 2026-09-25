//! Encoding an image into SSTV tones.

mod convert;
mod emit;
mod rgb_lines;

use crate::image::RgbPixel;
use crate::modes::Mode;
use crate::{Error, Result};

use emit::State;
use rgb_lines::{RgbLines, Storage};

/// `Encoder` is the main struct for converting an image into SSTV tones.
///
/// Construct `Encoder` with your desired mode and an iterator over the pixels
/// you want to encode, supplied row by row, top to bottom. The image must be
/// [`Mode::resolution`] pixels in size.
/// ```rust
/// use sstv::{modes::ROBOT_36, Encoder, Error, RgbPixel};
///
/// let image = [RgbPixel::new(0, 0, 0); 320 * 240];
/// let encoder = Encoder::new(ROBOT_36, image.into_iter())?;
/// for tone in encoder {
///     // emit or save the tones
/// }
/// # Ok::<(), Error>(())
/// ```
///
/// It encodes any mode by walking its timing sequence: the header, then for
/// each group of buffered lines one pass through the sequence, emitting fixed
/// tones verbatim and expanding each scan step into one tone per pixel.
pub struct Encoder<'a, I>
where
    I: Iterator<Item = RgbPixel>,
{
    mode: Mode,
    pixels: I,
    lines: RgbLines<'a>,
    state: State,
}

#[cfg(feature = "alloc")]
impl<I> Encoder<'static, I>
where
    I: Iterator<Item = RgbPixel>,
{
    /// Construct an `Encoder` from the mode and a pixel iterator, buffering
    /// lines in an allocation of its own. Available with the `alloc` feature
    /// (on by default); [`Encoder::new_in`] encodes without allocating.
    ///
    /// # Errors
    ///
    /// [`Error::EmptyImage`] if the iterator cannot fill the mode's first
    /// lines.
    pub fn new(mode: Mode, pixels: I) -> Result<Self> {
        let storage = alloc::vec![RgbPixel::new(0, 0, 0); mode.encoder_buffer_len()];
        Self::with_storage(mode, pixels, Storage::Owned(storage))
    }
}

impl<'a, I> Encoder<'a, I>
where
    I: Iterator<Item = RgbPixel>,
{
    /// Construct an `Encoder` that buffers lines in the given storage instead
    /// of allocating. The buffer must hold at least one of the mode's line
    /// groups — [`Mode::encoder_buffer_len`] pixels.
    ///
    /// ```rust
    /// use sstv::{modes::ROBOT_36, Encoder, RgbPixel};
    ///
    /// let image = [RgbPixel::new(0, 0, 0); 320 * 240];
    /// let mut buffer = [RgbPixel::new(0, 0, 0); ROBOT_36.encoder_buffer_len()];
    /// let encoder = Encoder::new_in(ROBOT_36, image.into_iter(), &mut buffer)?;
    /// # Ok::<(), sstv::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`Error::BufferTooSmall`] if the buffer cannot hold a line group, and
    /// [`Error::EmptyImage`] if the iterator cannot fill the mode's first
    /// lines.
    pub fn new_in(mode: Mode, pixels: I, buffer: &'a mut [RgbPixel]) -> Result<Self> {
        let Some(storage) = buffer.get_mut(..mode.encoder_buffer_len()) else {
            return Err(Error::BufferTooSmall);
        };
        Self::with_storage(mode, pixels, Storage::Borrowed(storage))
    }

    fn with_storage(mode: Mode, mut pixels: I, storage: Storage<'a>) -> Result<Self> {
        let mut lines = RgbLines {
            storage,
            width: mode.resolution.0,
            color: mode.color,
        };

        if lines.fill_next(&mut pixels).is_none() {
            return Err(Error::EmptyImage);
        }

        Ok(Self {
            mode,
            pixels,
            lines,
            state: State::NotStarted,
        })
    }
}
