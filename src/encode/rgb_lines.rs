//! The buffered image lines the encoder's scan steps sample from.

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::image::{RgbPixel, YuvPixel};
use crate::modes::step::{Channel, ColorMode};

/// The image lines the scan steps sample — one line for most modes, the line
/// pair for Robot 36 and PD modes — stored line after line.
pub(super) struct RgbLines<'a> {
    pub(super) storage: Storage<'a>,
    /// The width of one line in pixels.
    pub(super) width: usize,
    /// How the buffered lines combine into colour channels.
    pub(super) color: ColorMode,
}

impl RgbLines<'_> {
    /// The number of buffered rows.
    fn row_count(&self) -> usize {
        self.storage.as_slice().len() / self.width
    }

    /// Replace the buffered lines with the next ones from the pixel iterator.
    /// Returns `None` once the image runs out of complete line groups.
    pub(super) fn fill_next(&mut self, pixels: &mut impl Iterator<Item = RgbPixel>) -> Option<()> {
        for slot in self.storage.as_mut_slice() {
            *slot = pixels.next()?;
        }
        Some(())
    }

    /// The pixel value a scan step transmits at horizontal position `column`.
    /// The channel determines which buffered row it reads.
    pub(super) fn value(&self, column: usize, channel: Channel) -> u8 {
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

/// The backing storage of the buffered lines: allocated by
/// [`Encoder::new`](super::Encoder::new), or caller-provided through
/// [`Encoder::new_in`](super::Encoder::new_in).
pub(super) enum Storage<'a> {
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
