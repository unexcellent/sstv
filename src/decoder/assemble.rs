//! Turns the sampled scans of one timing sequence into rows of pixels,
//! according to the mode's colour system.

use alloc::vec::Vec;

use crate::image::{RgbPixel, YuvPixel};
use crate::modes::layout::{Channel, ColorMode, Layout};

/// The sampled contents of one pass through a timing sequence.
pub(super) struct SequenceData {
    /// Each scan step's channel and its sampled pixel values.
    pub scans: Vec<(Channel, Vec<u8>)>,
}

/// Combines each sequence's scans into image rows.
pub(super) struct Assembler {
    color: ColorMode,
}

impl Assembler {
    pub const fn new(layout: &Layout) -> Self {
        Self {
            color: layout.color,
        }
    }

    /// The rows completed by one sequence pass, in transmission order.
    // expect: the layouts guarantee every scan their colour mode relies on.
    #[allow(clippy::expect_used)]
    pub fn assemble(&self, data: &SequenceData) -> Vec<Vec<RgbPixel>> {
        let scan = |channel: Channel| {
            data.scans
                .iter()
                .find(|(c, _)| *c == channel)
                .map(|(_, values)| values.as_slice())
                .expect("the mode's timing sequence contains this scan")
        };

        let mut rows: Vec<Vec<RgbPixel>> = Vec::new();
        match self.color {
            ColorMode::Rgb => {
                let (red, green, blue) = (
                    scan(Channel::Red),
                    scan(Channel::Green),
                    scan(Channel::Blue),
                );
                rows.push(
                    (0..red.len())
                        .map(|x| RgbPixel::new(red[x], green[x], blue[x]))
                        .collect(),
                );
            }
            ColorMode::Yuv => {
                rows.push(yuv_row(
                    scan(Channel::Y),
                    scan(Channel::RY),
                    scan(Channel::BY),
                ));
            }
            ColorMode::YuvSharedPair => {
                let (ry, by) = (scan(Channel::RY), scan(Channel::BY));
                rows.push(yuv_row(scan(Channel::Y), ry, by));
                rows.push(yuv_row(scan(Channel::YSecond), ry, by));
            }
        }
        rows
    }
}

fn yuv_row(luma: &[u8], chroma_red: &[u8], chroma_blue: &[u8]) -> Vec<RgbPixel> {
    luma.iter()
        .zip(chroma_red.iter().zip(chroma_blue.iter()))
        .map(|(&y, (&ry, &by))| RgbPixel::from(YuvPixel::new(y, ry, by)))
        .collect()
}
