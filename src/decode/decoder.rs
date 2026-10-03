//! The decoder: from samples to the image.

use super::DecodedImage;
use super::Demodulator;
use super::assembler::{ImageSearch, ImageTones};
use super::walk::decode_image;
use crate::modes::{Mode, ROBOT_36};

/// Decodes an SSTV transmission from an audio sample stream.
///
/// `Decoder` is the inverse of [`Encoder`](crate::Encoder). A [`Demodulator`]
/// turns the 16-bit PCM samples into frequencies, which are assembled into
/// tones: the calibration header identifies the mode and where the image
/// starts, and every line sync pulse re-aligns the mode's timing sequence.
/// Walking that sequence recovers the image, one tone per pixel.
///
/// The decoder decodes the first image in the stream and reads only a little
/// past its end, so the remaining samples can be handed to another decoder
/// for the next image.
///
/// ```no_run
/// use sstv::Decoder;
///
/// # let samples = std::vec::Vec::<i16>::new().into_iter();
/// if let Some(image) = Decoder::new(samples, 48000).decode() {
///     let _ = (image.mode(), image.pixels());
/// }
/// ```
pub struct Decoder<I: Iterator<Item = i16>> {
    demodulator: Demodulator<I>,
    /// The mode given via [`with_mode`](Self::with_mode); `None` detects it
    /// from the header.
    mode: Option<Mode>,
    /// Begin decoding right away instead of searching for the image.
    without_header: bool,
}

impl<I: Iterator<Item = i16>> Decoder<I> {
    /// Decode a stream of PCM samples.
    ///
    /// `sample_rate` must be greater than zero. Construction never fails:
    /// finding the image is deferred to [`decode`](Self::decode).
    pub fn new(samples: I, sample_rate: u32) -> Self {
        Self::from_demodulator(Demodulator::new(samples, sample_rate))
    }

    /// Decode the frequencies of an existing demodulator.
    pub const fn from_demodulator(demodulator: Demodulator<I>) -> Self {
        Self {
            demodulator,
            mode: None,
            without_header: false,
        }
    }

    /// Decode in the given mode instead of detecting it from the header.
    ///
    /// The image starts at a header announcing this mode or, if there is
    /// none, at the first three of the mode's line sync pulses spaced one
    /// line apart, so a signal whose header is missing or unreadable still
    /// decodes.
    ///
    /// ```no_run
    /// use sstv::{modes::ROBOT_36, Decoder};
    ///
    /// # let samples = std::vec::Vec::<i16>::new().into_iter();
    /// let image = Decoder::new(samples, 48000).with_mode(ROBOT_36).decode();
    /// ```
    #[must_use]
    pub const fn with_mode(mut self, mode: Mode) -> Self {
        self.mode = Some(mode);
        self
    }

    /// Assume the samples begin directly at the image data and skip searching
    /// for its start.
    ///
    /// Decoding starts immediately at the first line's timing sequence. Use
    /// this when acquisition has already been performed upstream. Without a
    /// header there is no VIS code to detect a mode from, so unless
    /// [`with_mode`](Self::with_mode) names one, the image decodes as
    /// [`modes::ROBOT_36`](crate::modes::ROBOT_36).
    ///
    /// ```no_run
    /// use sstv::{modes::PD_120, Decoder};
    ///
    /// # let samples = std::vec::Vec::<i16>::new().into_iter();
    /// let image = Decoder::new(samples, 48000)
    ///     .with_mode(PD_120)
    ///     .without_header()
    ///     .decode();
    /// ```
    #[must_use]
    pub const fn without_header(mut self) -> Self {
        self.without_header = true;
        self
    }

    /// Decode the image, or `None` if the stream carries none.
    pub fn decode(self) -> Option<DecodedImage> {
        let sample_rate = self.demodulator.sample_rate();
        let tones = if self.without_header {
            let mode = self.mode.unwrap_or(ROBOT_36);
            ImageTones::at_first_sample(self.demodulator, sample_rate, mode)
        } else {
            ImageSearch::new(self.demodulator, sample_rate, self.mode).find_image()?
        };
        Some(decode_image(tones.mode(), tones))
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec::Vec;

    #[cfg(feature = "image")]
    use super::super::testing::{
        GROUND_STATION_RECORDING, PYSSTV_FIXTURE, read_generated_wav, read_gzipped_wav,
    };
    use super::super::testing::{
        gradient_image, header_length, mean_abs_error, read_iss_recording, transmit,
    };
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
    fn decodes_in_the_given_mode_without_a_header() {
        let image = gradient_image(SCOTTIE_1);
        let samples = transmit(SCOTTIE_1, &image, 48_000);
        let without_header = samples[header_length(SCOTTIE_1, 48_000)..].to_vec();

        let decoded = Decoder::new(without_header.into_iter(), 48_000)
            .with_mode(SCOTTIE_1)
            .decode()
            .unwrap();

        assert_decodes_completely(&decoded, SCOTTIE_1);
        let error = mean_abs_error(&image, decoded.pixels());
        assert!(error < 5.0, "mean abs error {error} too high");
    }

    /// Entering the stream at a line pair's second line must not swap the
    /// colour differences: decoding aligns to the next full pair, so the
    /// image shifts up by the two lines skipped.
    #[test]
    fn decoding_from_the_second_line_of_a_pair_keeps_the_colours() {
        let image = gradient_image(ROBOT_36);
        let samples = transmit(ROBOT_36, &image, 48_000);
        let first_line = 48_000 * 150 / 1000;
        let from_second_line = samples[header_length(ROBOT_36, 48_000) + first_line..].to_vec();

        let decoded = Decoder::new(from_second_line.into_iter(), 48_000)
            .with_mode(ROBOT_36)
            .decode()
            .unwrap();

        let (width, height) = (320, 240);
        let error = mean_abs_error(
            &image[width * 2..],
            &decoded.pixels()[..width * (height - 2)],
        );
        assert!(error < 5.0, "mean abs error {error} too high");
    }

    #[test]
    fn decodes_samples_that_begin_at_the_first_line() {
        let image = gradient_image(PD_120);
        let samples = transmit(PD_120, &image, 48_000);
        let first_line_on = samples[header_length(PD_120, 48_000)..].to_vec();

        let decoded = Decoder::new(first_line_on.into_iter(), 48_000)
            .with_mode(PD_120)
            .without_header()
            .decode()
            .unwrap();

        assert_decodes_completely(&decoded, PD_120);
        let error = mean_abs_error(&image, decoded.pixels());
        assert!(error < 5.0, "mean abs error {error} too high");
    }

    #[test]
    fn decoding_a_demodulator_matches_decoding_its_samples() {
        let samples = transmit(ROBOT_36, &gradient_image(ROBOT_36), 48_000);
        let demodulator = Demodulator::new(samples.clone().into_iter(), 48_000);

        let from_demodulator = Decoder::from_demodulator(demodulator).decode();

        assert_eq!(from_demodulator, decode(samples, 48_000));
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

        assert!(!decoded.complete());
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

    fn decode(samples: Vec<i16>, sample_rate: u32) -> Option<DecodedImage> {
        Decoder::new(samples.into_iter(), sample_rate).decode()
    }

    fn assert_round_trip(mode: Mode, sample_rate: u32, max_error: f64) {
        let image = gradient_image(mode);
        let decoded = decode(transmit(mode, &image, sample_rate), sample_rate).unwrap();

        assert_decodes_completely(&decoded, mode);
        let error = mean_abs_error(&image, decoded.pixels());
        assert!(error < max_error, "mean abs error {error} too high");
    }

    fn assert_decodes_completely(image: &DecodedImage, mode: Mode) {
        assert_eq!(image.mode(), mode);
        assert!(image.complete(), "image should decode completely");
    }

    /// The recordings carry no reference image, so this only checks that the
    /// mode is identified and every row decoded.
    fn assert_decodes_iss_recording(name: &str, mode: Mode) {
        let (samples, sample_rate) = read_iss_recording(name);

        let decoded = decode(samples, sample_rate).unwrap();

        assert_decodes_completely(&decoded, mode);
    }

    #[cfg(feature = "image")]
    fn assert_matches_source_image(decoded: &DecodedImage, max_error: f64) {
        use super::super::testing::{SOURCE_IMAGE, read_image};

        assert_decodes_completely(decoded, ROBOT_36);
        let error = mean_abs_error(&read_image(SOURCE_IMAGE), decoded.pixels());
        assert!(error < max_error, "mean abs error {error} too high");
    }
}
