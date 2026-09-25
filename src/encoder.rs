#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::image::{RgbPixel, YuvPixel};
use crate::modes::layout::{Channel, ColorMode, Layout, Step};
use crate::modes::{Mode, value_frequency};
use crate::synthesizer::Tone;
use crate::{Error, Result};

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
/// It encodes any mode by walking its layout: the header, then for each
/// group of buffered lines the mode's timing sequences, emitting fixed tones
/// verbatim and expanding each scan step into one tone per pixel.
pub struct Encoder<'a, I>
where
    I: Iterator<Item = RgbPixel>,
{
    mode: Mode,
    pixels: I,
    lines: RgbLines<'a>,
    state: State,
}

/// The image lines the scan steps sample — one line for most modes, the line
/// pair for Robot 36 and PD modes — stored line after line.
struct RgbLines<'a> {
    storage: Storage<'a>,
    /// The width of one line in pixels.
    width: usize,
    /// How the buffered lines combine into colour channels.
    color: ColorMode,
}

/// The backing storage of the buffered lines: allocated by [`Encoder::new`],
/// or caller-provided through [`Encoder::new_in`].
enum Storage<'a> {
    #[cfg(feature = "alloc")]
    Owned(Vec<RgbPixel>),
    Borrowed(&'a mut [RgbPixel]),
}

impl Storage<'_> {
    fn as_slice(&self) -> &[RgbPixel] {
        match self {
            #[cfg(feature = "alloc")]
            Self::Owned(pixels) => pixels,
            Self::Borrowed(pixels) => pixels,
        }
    }

    fn as_mut_slice(&mut self) -> &mut [RgbPixel] {
        match self {
            #[cfg(feature = "alloc")]
            Self::Owned(pixels) => pixels,
            Self::Borrowed(pixels) => pixels,
        }
    }
}

impl RgbLines<'_> {
    /// The number of buffered rows.
    fn row_count(&self) -> usize {
        self.storage.as_slice().len() / self.width
    }

    /// Replace the buffered lines with the next ones from the pixel iterator.
    /// Returns `None` once the image runs out of complete line groups.
    fn fill_next(&mut self, pixels: &mut impl Iterator<Item = RgbPixel>) -> Option<()> {
        for slot in self.storage.as_mut_slice() {
            *slot = pixels.next()?;
        }
        Some(())
    }

    /// The pixel value a scan step transmits at horizontal position `column`.
    /// The channel determines which buffered row it reads.
    fn value(&self, column: usize, channel: Channel) -> u8 {
        match channel {
            Channel::Red => self.rgb(0, column).red(),
            Channel::Green => self.rgb(0, column).green(),
            Channel::Blue => self.rgb(0, column).blue(),
            Channel::Y => self.yuv(0, column).luma(),
            Channel::YSecond => self.yuv(1, column).luma(),
            Channel::RY => self.chroma(column, YuvPixel::chroma_red),
            Channel::BY => self.chroma(column, YuvPixel::chroma_blue),
        }
    }

    fn rgb(&self, row: usize, column: usize) -> RgbPixel {
        self.storage.as_slice()[row * self.width + column]
    }

    fn yuv(&self, row: usize, column: usize) -> YuvPixel {
        YuvPixel::from(self.rgb(row, column))
    }

    /// One colour-difference component, averaged over all buffered lines
    /// where the mode calls for it (Robot 36 and PD modes).
    fn chroma(&self, column: usize, component: fn(YuvPixel) -> u8) -> u8 {
        match self.color {
            ColorMode::YuvSharedPair => {
                let row_count = self.row_count();
                let sum: u16 = (0..row_count)
                    .map(|row| u16::from(component(self.yuv(row, column))))
                    .sum();
                (sum / row_count as u16) as u8
            }
            _ => component(self.yuv(0, column)),
        }
    }
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
            width: mode.layout().resolution.0,
            color: mode.layout().color,
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

    /// The tone belonging to the current state.
    fn emit(&self) -> Option<Tone> {
        match self.state {
            State::NotStarted | State::Finished => None,
            State::Header(index) => self.mode.header_tone(index),
            State::Image { step, pixel, .. } => Some(self.emit_image_tone(step, pixel)),
        }
    }

    fn emit_image_tone(&self, step: usize, pixel: usize) -> Tone {
        let current_step = self.mode.layout().sequence[step];
        match current_step {
            Step::Control(tone) => tone,
            Step::Scan(channel, duration) => {
                let value = self.lines.value(pixel, channel);
                Tone::new(
                    value_frequency(value),
                    duration / self.mode.layout().resolution.0 as u32,
                )
            }
        }
    }
}

/// The transmission as a whole, packed into common audio containers.
impl<I> Encoder<'_, I>
where
    I: Iterator<Item = RgbPixel>,
{
    /// The transmission as a complete mono 16-bit PCM WAV at the given sample
    /// rate; see [`Synthesizer::to_wav`].
    #[cfg(feature = "wav")]
    #[must_use]
    pub fn to_wav(self, sample_rate: u32) -> Vec<u8> {
        crate::Synthesizer::new(self, sample_rate).to_wav()
    }

    /// The transmission as a complete mono 128 kbps MP3 at the given sample
    /// rate; see [`Synthesizer::to_mp3`].
    ///
    /// # Errors
    ///
    /// Fails if LAME rejects the sample rate.
    #[cfg(feature = "mp3")]
    pub fn to_mp3(
        self,
        sample_rate: u32,
    ) -> core::result::Result<Vec<u8>, mp3lame_encoder::BuildError> {
        crate::Synthesizer::new(self, sample_rate).to_mp3()
    }
}

#[cfg(feature = "image")]
impl Encoder<'static, alloc::vec::IntoIter<RgbPixel>> {
    /// Encode an image loaded with the `image` crate.
    ///
    /// The image is resized to the mode's resolution if it does not match,
    /// stretching it to fit.
    ///
    /// ```no_run
    /// use sstv::{modes::ROBOT_36, Encoder};
    ///
    /// let image = image::open("image.png")?;
    /// let encoder = Encoder::from_image(ROBOT_36, &image)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`Error::EmptyImage`] if the mode has no pixels, which cannot happen
    /// for the supported modes.
    pub fn from_image(mode: Mode, image: &image::DynamicImage) -> Result<Self> {
        let mut image = image.to_rgb8();

        let image_resolution = (image.width(), image.height());
        if image_resolution != mode.resolution() {
            image = image::imageops::resize(
                &image,
                mode.resolution().0,
                mode.resolution().1,
                image::imageops::FilterType::Triangle,
            );
        }

        // The pixels must outlive the image buffer this function drops, so
        // collecting them is not needless.
        #[allow(clippy::needless_collect)]
        let pixels: Vec<RgbPixel> = image
            .pixels()
            .map(|pixel| RgbPixel::new(pixel[0], pixel[1], pixel[2]))
            .collect();

        Self::new(mode, pixels.into_iter())
    }
}

impl<I> Iterator for Encoder<'_, I>
where
    I: Iterator<Item = RgbPixel>,
{
    type Item = Tone;

    fn next(&mut self) -> Option<Tone> {
        self.state.advance(self.mode, &self.mode.layout());

        let pixel_iterator_is_empty =
            self.state.needs_next_lines() && self.lines.fill_next(&mut self.pixels).is_none();
        if pixel_iterator_is_empty {
            self.state = State::Finished;
        }

        self.emit()
    }
}

/// Where the encoder is within the transmission.
#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    NotStarted,
    Header(usize),
    Image {
        /// The first image row of the currently buffered line group.
        row: usize,
        /// The position within the timing sequence's steps.
        step: usize,
        /// The horizontal position within a scan step.
        pixel: usize,
    },
    Finished,
}

impl State {
    /// Step to the state that emits the next tone.
    fn advance(&mut self, mode: Mode, layout: &Layout) {
        match *self {
            Self::NotStarted => *self = Self::Header(0),
            Self::Header(index) => {
                *self = if mode.header_tone(index + 1).is_some() {
                    Self::Header(index + 1)
                } else {
                    Self::Image {
                        row: 0,
                        step: 0,
                        pixel: 0,
                    }
                };
            }
            Self::Image { row, step, pixel } => {
                let steps = layout.sequence;
                let mid_scan =
                    matches!(steps[step], Step::Scan(..)) && pixel + 1 < layout.resolution.0;
                *self = if mid_scan {
                    Self::Image {
                        row,
                        step,
                        pixel: pixel + 1,
                    }
                } else if step + 1 < steps.len() {
                    Self::Image {
                        row,
                        step: step + 1,
                        pixel: 0,
                    }
                } else if row + layout.lines_per_sequence < layout.resolution.1 {
                    Self::Image {
                        row: row + layout.lines_per_sequence,
                        step: 0,
                        pixel: 0,
                    }
                } else {
                    Self::Finished
                };
            }
            Self::Finished => (),
        }
    }

    /// Whether the state just moved onto the first tone of a line group whose
    /// lines are not buffered yet. The first group's lines are already
    /// buffered at construction.
    const fn needs_next_lines(&self) -> bool {
        matches!(
            self,
            Self::Image {
                row,
                step: 0,
                pixel: 0,
            } if *row > 0
        )
    }
}
