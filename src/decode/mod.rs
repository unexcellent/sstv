//! Decoding SSTV tones into images.

mod acquire;
mod assemble;
mod convert;
mod events;
mod images;
mod stream;

#[cfg(test)]
pub use events::testing;

use crate::Demodulator;
use crate::modes::Mode;

pub use events::{Event, Events, RgbRow};
pub use images::{DecodedImage, Images};

/// Decodes SSTV transmissions from an audio sample stream.
///
/// `Decoder` is the streaming inverse of [`Encoder`](crate::Encoder). It runs
/// a [`Demodulator`] over 16-bit PCM samples and walks the mode's timing
/// sequence as specified in the Dayton paper, re-aligning on every sync
/// pulse. Acquisition happens lazily as the output is polled: a stream that
/// never contains an image simply yields nothing, and a stream carrying
/// several images — with or without gaps between them — yields all of them.
///
/// Construct it from a sample stream ([`from_samples`](Self::from_samples))
/// or an existing demodulator ([`from_demodulator`](Self::from_demodulator)),
/// then choose how to consume it: [`events`](Self::events) streams scanlines
/// as they are recovered, holding only about one line group in memory, while
/// [`images`](Self::images) assembles and yields whole images.
///
/// Each image's mode is detected from its header's VIS code; use
/// [`expect_mode`](Self::expect_mode) to decode in a fixed mode instead.
///
/// ```no_run
/// use sstv::Decoder;
///
/// # let samples = std::vec::Vec::<i16>::new().into_iter();
/// for image in Decoder::from_samples(samples, 48000).images() {
///     let _ = (image.mode(), image.pixels());
/// }
/// ```
pub struct Decoder<I: Iterator<Item = i16>> {
    events: Events<I>,
}

impl<I: Iterator<Item = i16>> Decoder<I> {
    /// Decode a stream of PCM samples.
    ///
    /// `sample_rate` must be greater than zero. Construction never fails:
    /// finding images is deferred to iteration.
    pub fn from_samples(samples: I, sample_rate: u32) -> Self {
        let sample_rate = sample_rate.max(1);
        Self::from_demodulator(Demodulator::new(samples, sample_rate))
    }

    /// Decode the frequency stream of an existing demodulator.
    pub const fn from_demodulator(demodulator: Demodulator<I>) -> Self {
        Self {
            events: Events::new(demodulator),
        }
    }

    /// Decode every image in the given mode instead of detecting each image's
    /// mode from its header.
    ///
    /// ```no_run
    /// use sstv::{modes::ROBOT_36, Decoder};
    ///
    /// # let samples = std::vec::Vec::<i16>::new().into_iter();
    /// let decoder = Decoder::from_samples(samples, 48000).expect_mode(ROBOT_36);
    /// ```
    #[must_use]
    pub const fn expect_mode(mut self, mode: Mode) -> Self {
        self.events.expected_mode = Some(mode);
        self
    }

    /// Assume the samples begin directly at the image data and skip searching
    /// for a header.
    ///
    /// Decoding starts immediately at the first line's timing sequence. Use
    /// this when the signal carries no detectable header, or when acquisition
    /// has already been performed upstream. After the first image completes,
    /// the decoder searches for further images as usual. Without a header
    /// there is no VIS code to detect a mode from, so unless
    /// [`expect_mode`](Self::expect_mode) names one, the first image decodes
    /// as [`modes::ROBOT_36`](crate::modes::ROBOT_36).
    ///
    /// ```no_run
    /// use sstv::{modes::PD_120, Decoder};
    ///
    /// # let samples = std::vec::Vec::<i16>::new().into_iter();
    /// let decoder = Decoder::from_samples(samples, 48000)
    ///     .expect_mode(PD_120)
    ///     .without_header();
    /// for image in decoder.images() {
    ///     let _ = image.pixels();
    /// }
    /// ```
    #[must_use]
    pub const fn without_header(mut self) -> Self {
        self.events.skip_header = true;
        self
    }

    /// Stream scanlines as they are recovered, grouped into images by
    /// [`Event::ImageStart`] and [`Event::ImageEnd`] markers.
    pub fn events(self) -> Events<I> {
        self.events
    }

    /// Assemble and stream whole images, one at a time.
    pub fn images(self) -> Images<I> {
        Images::new(self.events)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec::Vec;

    use super::testing::{HEIGHT, encode, header_sample_count, mean_abs_error, test_image};
    use super::*;
    use crate::RgbPixel;
    use crate::modes::ROBOT_36;

    #[test]
    fn round_trip_without_header() {
        let image = test_image();
        // Drop the header so the samples begin at the first line's sync pulse.
        let full = encode(&image, 48_000);
        let header = header_sample_count(48_000);
        let image_samples = full.into_iter().skip(header);

        let mut rows = 0;
        let mut complete = None;
        let mut decoded: Vec<RgbPixel> = Vec::new();
        let events = Decoder::from_samples(image_samples, 48_000)
            .expect_mode(ROBOT_36)
            .without_header()
            .events();
        for event in events {
            match event {
                Event::ImageStart(mode) => assert_eq!(mode, ROBOT_36),
                Event::Row(row) => {
                    assert_eq!(row.index(), rows, "row out of order");
                    rows += 1;
                    decoded.extend_from_slice(row.pixels());
                }
                Event::ImageEnd { complete: flag } => complete = Some(flag),
            }
        }

        assert_eq!(complete, Some(true), "image should decode completely");
        assert_eq!(rows, HEIGHT, "should decode all rows");
        let error = mean_abs_error(&image, &decoded);
        assert!(error < 12.0, "mean abs error {error} too high");
    }

    #[test]
    fn decoding_from_a_demodulator_matches_decoding_from_samples() {
        let image = test_image();
        let samples = encode(&image, 48_000);

        let demodulator = crate::Demodulator::new(samples.clone().into_iter(), 48_000);
        let from_demodulator: Vec<Event> = Decoder::from_demodulator(demodulator)
            .expect_mode(ROBOT_36)
            .events()
            .collect();
        let from_samples: Vec<Event> = Decoder::from_samples(samples.into_iter(), 48_000)
            .expect_mode(ROBOT_36)
            .events()
            .collect();

        assert_eq!(from_demodulator, from_samples);
    }
}
