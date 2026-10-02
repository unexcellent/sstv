//! Walking the image's tones through the mode's timing sequence, as the
//! encoder does: one tone per control step and one per pixel.

use alloc::vec::Vec;

use super::DecodedImage;
use super::rows::assemble_rows;
use crate::RgbPixel;
use crate::modes::step::Step;
use crate::modes::{BLACK_FREQUENCY, Mode, WHITE_FREQUENCY};
use crate::synthesizer::Tone;

/// The image carried by the next tones, incomplete if they run out first.
pub(super) fn decode_image(mode: Mode, tones: &mut impl Iterator<Item = Tone>) -> DecodedImage {
    let (width, height) = mode.resolution;
    let mut pixels = alloc::vec![RgbPixel::new(0, 0, 0); width * height];

    let mut row = 0;
    while row < height {
        let Some(rows) = decode_sequence(mode, tones) else {
            return DecodedImage::new(mode, false, pixels);
        };
        for decoded in rows {
            if let Some(target) = pixels.get_mut(row * width..(row + 1) * width) {
                target.copy_from_slice(&decoded);
            }
            row += 1;
        }
    }

    DecodedImage::new(mode, true, pixels)
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
            .map(|_| tones.next().map(pixel_value))
            .collect::<Option<Vec<u8>>>()?;

        scans.push((*channel, values));
    }
    Some(assemble_rows(mode.color, &scans))
}

/// The pixel value a tone carries, mapped linearly from black to white.
fn pixel_value(tone: Tone) -> u8 {
    let black = i64::from(BLACK_FREQUENCY.hz());
    let white = i64::from(WHITE_FREQUENCY.hz());
    let value = (i64::from(tone.frequency.hz()) - black) * 255 / (white - black);
    value.clamp(0, 255) as u8
}
