//! Whole images assembled from the decoder's event stream.

use alloc::{vec, vec::Vec};

use super::events::{Event, Events};
use crate::RgbPixel;
use crate::modes::Mode;

/// Whole images assembled from a [`Decoder`](super::Decoder)'s event stream, yielded one at
/// a time. Unlike [`Events`], this buffers a full image in memory.
pub struct Images<I: Iterator<Item = i16>> {
    events: Events<I>,
}

impl<I: Iterator<Item = i16>> Images<I> {
    pub(super) const fn new(events: Events<I>) -> Self {
        Self { events }
    }
}

impl<I: Iterator<Item = i16>> Iterator for Images<I> {
    type Item = DecodedImage;

    fn next(&mut self) -> Option<DecodedImage> {
        let mode = loop {
            if let Event::ImageStart(mode) = self.events.next()? {
                break mode;
            }
        };
        let (width, height) = mode.resolution();
        let (width, height) = (width as usize, height as usize);

        let mut pixels = vec![RgbPixel::new(0, 0, 0); width * height];
        let mut complete = false;
        loop {
            match self.events.next() {
                Some(Event::Row(row)) => {
                    let start = row.index() * width;
                    pixels[start..start + width].copy_from_slice(row.pixels());
                }
                Some(Event::ImageEnd { complete: flag }) => {
                    complete = flag;
                    break;
                }
                Some(Event::ImageStart(_)) | None => break,
            }
        }

        Some(DecodedImage {
            mode,
            width,
            height,
            complete,
            pixels,
        })
    }
}

/// One whole image assembled from a [`Decoder`](super::Decoder)'s event stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImage {
    mode: Mode,
    width: usize,
    height: usize,
    complete: bool,
    pixels: Vec<RgbPixel>,
}

impl DecodedImage {
    /// The SSTV mode the image was transmitted in.
    #[must_use]
    pub const fn mode(&self) -> Mode {
        self.mode
    }

    /// The image width in pixels.
    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    /// The image height in pixels.
    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    /// Whether every scanline was decoded. `false` if the image was truncated
    /// before completion (for example, the signal faded out).
    #[must_use]
    pub const fn complete(&self) -> bool {
        self.complete
    }

    /// The pixels in row-major order, always `width() * height()` of them.
    /// Rows the signal did not carry are black.
    #[must_use]
    pub fn pixels(&self) -> &[RgbPixel] {
        &self.pixels
    }
}
