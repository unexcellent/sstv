//! Decodes images from the assembler's tones, walking the mode's timing
//! sequence as the encoder does: one tone per control step and one per
//! pixel.

use alloc::vec::Vec;

use super::assembler::Assembler;
use super::demodulator::Demodulator;
use super::rows::assemble_rows;
use crate::RgbPixel;
use crate::modes::step::Step;
use crate::modes::{BLACK_FREQUENCY, Mode, WHITE_FREQUENCY};
use crate::synthesizer::Tone;

/// An image decoded from the sample stream.
pub(super) struct Image {
    pub mode: Mode,
    /// Whether every row was decoded before the signal ended.
    pub complete: bool,
    /// Row-major pixels at the mode's resolution; rows the signal did not
    /// carry are black.
    pub pixels: Vec<RgbPixel>,
}

/// Decodes the first image in a sample stream.
pub(super) struct Decoder<I: Iterator<Item = i16>> {
    tones: Assembler<Demodulator<I>>,
}

impl<I: Iterator<Item = i16>> Decoder<I> {
    pub fn new(samples: I, sample_rate: u32) -> Self {
        let sample_rate = sample_rate.max(1);
        let frequencies = Demodulator::new(samples, sample_rate);
        Self {
            tones: Assembler::new(frequencies, sample_rate),
        }
    }

    /// The image, or `None` if the stream carries no header.
    pub fn decode(mut self) -> Option<Image> {
        // The mode is known once the image's first tone has been assembled.
        let first = self.tones.next()?;
        let mode = self.tones.detected_mode()?;
        let mut tones = core::iter::once(first).chain(&mut self.tones);
        Some(decode_image(mode, &mut tones))
    }
}

/// The image carried by the next tones, incomplete if they run out first.
fn decode_image(mode: Mode, tones: &mut impl Iterator<Item = Tone>) -> Image {
    let (width, height) = mode.resolution;
    let mut pixels = alloc::vec![RgbPixel::new(0, 0, 0); width * height];

    let mut row = 0;
    while row < height {
        let Some(rows) = decode_sequence(mode, tones) else {
            return Image {
                mode,
                complete: false,
                pixels,
            };
        };
        for decoded in rows {
            if let Some(target) = pixels.get_mut(row * width..(row + 1) * width) {
                target.copy_from_slice(&decoded);
            }
            row += 1;
        }
    }

    Image {
        mode,
        complete: true,
        pixels,
    }
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
        match step {
            Step::Control(_) => {
                tones.next()?;
            }
            Step::Scan(channel, _) => {
                let values = (0..width)
                    .map(|_| tones.next().map(pixel_value))
                    .collect::<Option<Vec<u8>>>()?;
                scans.push((*channel, values));
            }
        }
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

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec::Vec;

    #[cfg(feature = "image")]
    use super::super::testing::{
        GROUND_STATION_RECORDING, PYSSTV_FIXTURE, read_generated_wav, read_gzipped_wav,
    };
    use super::super::testing::{gradient_image, mean_abs_error, read_iss_recording, transmit};
    use super::*;
    use crate::modes::{MARTIN_1, PD_120, PD_180, ROBOT_36, ROBOT_72, SCOTTIE_1};

    #[test]
    fn decodes_our_own_robot_36_transmission() {
        assert_round_trip(ROBOT_36, 48_000, 5.0);
    }

    #[test]
    fn decodes_our_own_robot_72_transmission() {
        assert_round_trip(ROBOT_72, 48_000, 5.0);
    }

    /// Martin opens every sequence with a short (4.862 ms) sync pulse.
    #[test]
    fn decodes_our_own_martin_1_transmission() {
        assert_round_trip(MARTIN_1, 48_000, 3.0);
    }

    /// Scottie sends a starting sync pulse and places each line's sync
    /// pulse in the middle of the sequence.
    #[test]
    fn decodes_our_own_scottie_1_transmission() {
        assert_round_trip(SCOTTIE_1, 48_000, 3.0);
    }

    #[test]
    fn decodes_our_own_pd_120_transmission() {
        assert_round_trip(PD_120, 48_000, 5.0);
    }

    /// At 8 kHz a pixel spans only a couple of samples, so the error is
    /// higher, but the image still decodes completely.
    #[test]
    fn decodes_a_transmission_sampled_at_8_khz() {
        assert_round_trip(ROBOT_36, 8_000, 15.0);
    }

    #[test]
    fn decodes_only_the_first_image() {
        let mut samples = transmit(ROBOT_36, &gradient_image(ROBOT_36), 48_000);
        samples.extend(transmit(MARTIN_1, &gradient_image(MARTIN_1), 48_000));

        let decoded = decode(samples, 48_000).unwrap();

        assert_decodes_completely(&decoded, ROBOT_36);
    }

    /// A decoder stops reading shortly after its image, so the samples left
    /// over hold the next transmission for a fresh decoder.
    #[test]
    fn decodes_consecutive_images_with_one_decoder_each() {
        let image = gradient_image(ROBOT_36);
        let mut samples = transmit(ROBOT_36, &image, 48_000);
        samples.extend(transmit(ROBOT_36, &image, 48_000));
        let mut samples = samples.into_iter();

        let first = Decoder::new(samples.by_ref(), 48_000).decode().unwrap();
        let second = Decoder::new(samples.by_ref(), 48_000).decode().unwrap();
        let after_the_last = Decoder::new(samples.by_ref(), 48_000).decode();

        assert_decodes_completely(&first, ROBOT_36);
        assert_decodes_completely(&second, ROBOT_36);
        assert!(after_the_last.is_none());
    }

    #[test]
    fn transmission_cut_short_decodes_an_incomplete_image() {
        let image = gradient_image(ROBOT_36);
        let samples = transmit(ROBOT_36, &image, 48_000);
        let first_half = samples[..samples.len() / 2].to_vec();

        let decoded = decode(first_half, 48_000).unwrap();

        assert!(!decoded.complete);
    }

    #[test]
    fn silence_decodes_no_image() {
        let silence = std::vec![0i16; 48_000];

        assert!(decode(silence, 48_000).is_none());
    }

    #[cfg(feature = "image")]
    #[test]
    fn decodes_the_ground_station_recording() {
        let (samples, sample_rate) = read_gzipped_wav(GROUND_STATION_RECORDING);

        assert_matches_source_image(&decode(samples, sample_rate).unwrap(), 15.0);
    }

    #[cfg(feature = "image")]
    #[test]
    fn decodes_the_pysstv_fixture() {
        let Some((samples, sample_rate)) = read_generated_wav(PYSSTV_FIXTURE) else {
            return;
        };

        assert_matches_source_image(&decode(samples, sample_rate).unwrap(), 10.0);
    }

    #[test]
    fn decodes_the_gagarin_80_recording() {
        assert_decodes_iss_recording("pd180-gagarin-80.wav", PD_180);
    }

    #[test]
    fn decodes_the_apollo_soyuz_recording() {
        assert_decodes_iss_recording("pd180-apollo-soyuz.wav", PD_180);
    }

    #[test]
    fn decodes_the_astronauts_qso_recording() {
        assert_decodes_iss_recording("pd180-ariss-qso-astros.wav", PD_180);
    }

    #[test]
    fn decodes_the_cristoforetti_qso_recording() {
        assert_decodes_iss_recording("pd180-ariss-qso-cristoforetti.wav", PD_180);
    }

    #[test]
    fn decodes_the_mai75_suitsat_recording() {
        assert_decodes_iss_recording("pd180-mai75-suitsat.wav", PD_180);
    }

    #[test]
    fn decodes_the_first_ariss_20_year_recording() {
        assert_decodes_iss_recording("pd120-ariss-20-year-1.wav", PD_120);
    }

    #[test]
    fn decodes_the_second_ariss_20_year_recording() {
        assert_decodes_iss_recording("pd120-ariss-20-year-2.wav", PD_120);
    }

    fn decode(samples: Vec<i16>, sample_rate: u32) -> Option<Image> {
        Decoder::new(samples.into_iter(), sample_rate).decode()
    }

    fn assert_round_trip(mode: Mode, sample_rate: u32, max_error: f64) {
        let image = gradient_image(mode);
        let decoded = decode(transmit(mode, &image, sample_rate), sample_rate).unwrap();

        assert_decodes_completely(&decoded, mode);
        let error = mean_abs_error(&image, &decoded.pixels);
        assert!(error < max_error, "mean abs error {error} too high");
    }

    fn assert_decodes_completely(image: &Image, mode: Mode) {
        assert_eq!(image.mode, mode);
        assert!(image.complete, "image should decode completely");
    }

    /// The recordings carry no reference image, so this only checks that the
    /// mode is identified and every row decoded.
    fn assert_decodes_iss_recording(name: &str, mode: Mode) {
        let (samples, sample_rate) = read_iss_recording(name);

        let decoded = decode(samples, sample_rate).unwrap();

        assert_decodes_completely(&decoded, mode);
    }

    #[cfg(feature = "image")]
    fn assert_matches_source_image(decoded: &Image, max_error: f64) {
        use super::super::testing::{SOURCE_IMAGE, read_image};

        assert_decodes_completely(decoded, ROBOT_36);
        let error = mean_abs_error(&read_image(SOURCE_IMAGE), &decoded.pixels);
        assert!(error < max_error, "mean abs error {error} too high");
    }
}
