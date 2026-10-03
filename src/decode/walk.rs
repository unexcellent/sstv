//! Walking the image's tones through the mode's timing sequence, as the
//! encoder does: one tone per control step and one per pixel.

use alloc::vec::Vec;

use super::DecodedImage;
use super::rows::assemble_rows;
use crate::RgbPixel;
use crate::modes::step::Step;
use crate::modes::{Mode, frequency_value};
use crate::units::Tone;

/// The image the tones carry, incomplete if they run out first. Rows the
/// tones did not carry are black.
pub(super) fn decode_image(mode: Mode, mut tones: impl Iterator<Item = Tone>) -> DecodedImage {
    let (width, height) = mode.resolution;
    let rows: Vec<Vec<RgbPixel>> = core::iter::from_fn(|| decode_sequence(mode, &mut tones))
        .flatten()
        .take(height)
        .collect();
    let complete = rows.len() == height;

    let mut pixels: Vec<RgbPixel> = rows.into_iter().flatten().collect();
    pixels.resize(width * height, RgbPixel::new(0, 0, 0));
    DecodedImage::new(mode, complete, pixels)
}

/// The rows completed by one pass through the mode's timing sequence, or
/// `None` if the tones run out first.
fn decode_sequence(
    mode: Mode,
    tones: &mut impl Iterator<Item = Tone>,
) -> Option<Vec<Vec<RgbPixel>>> {
    let width = mode.resolution.0;
    let mut scans = Vec::new();
    for step in mode.sequence {
        let Step::Scan(channel, _) = step else {
            tones.next()?;
            continue;
        };

        let values = (0..width)
            .map(|_| tones.next().map(|tone| frequency_value(tone.frequency)))
            .collect::<Option<Vec<u8>>>()?;

        scans.push((*channel, values));
    }
    Some(assemble_rows(mode.color, &scans))
}
