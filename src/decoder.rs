use crate::modes::Mode;
use crate::units::Duration;
use crate::{Demodulator, Error, Frequency, Result, RgbPixel, YuvPixel};

/// The horizontal resolution of Robot36. Fixed while only one mode is supported.
const WIDTH: usize = 320;
/// The vertical resolution of Robot36.
const HEIGHT: usize = 240;
/// How far a frequency may stray from the sync frequency and still count as sync.
const SYNC_TOLERANCE_HZ: u32 = 150;

/// Decodes a Robot36 SSTV transmission back into an image.
///
/// `Decoder` is the inverse of [`Encoder`](crate::Encoder): it consumes 16-bit
/// PCM samples and yields [`RgbPixel`]s in raster order (top-left first, row by
/// row). Internally it runs a [`Demodulator`] over the samples, and pulls from
/// it lazily — it never holds the whole frequency track or the whole image,
/// only about one row-pair at a time.
///
/// A full image cannot be produced sample-by-sample: Robot36 sends the two rows
/// of a pair with a shared chroma (the even row's red difference and the odd
/// row's blue difference), so both rows are reconstructed together once the odd
/// row arrives. This mirrors [`Encoder`](crate::Encoder), which likewise buffers
/// a row-pair while remaining a lazy iterator.
///
/// ```rust
/// use sstv::{Decoder, Encoder, Mode, RgbPixel, Synthesizer};
///
/// let image = [RgbPixel::new(0, 0, 0); 320 * 240];
/// let encoder = Encoder::new(Mode::Robot36, image.into_iter()).unwrap();
/// let samples = Synthesizer::new(encoder, 48000);
///
/// let decoded: Vec<RgbPixel> = Decoder::new(samples, 48000).unwrap().collect();
/// assert_eq!(decoded.len(), 320 * 240);
/// ```
pub struct Decoder<I: Iterator<Item = i16>> {
    demodulator: Demodulator<I>,

    // Timing, in (fractional) samples, derived from the sample rate.
    pixel_luma: f64,
    pixel_chroma: f64,
    luma_len: f64,
    chroma_len: f64,
    blank_len: f64,
    back_porch_len: f64,

    // Frequency band mapping.
    sync_hz: u32,
    black_hz: i64,
    white_hz: i64,

    // Streaming position: `position` counts samples pulled from the demodulator,
    // `current` is the most recently pulled frequency.
    position: usize,
    current: Frequency,
    /// Sample position at which the current row's luma scan begins.
    luma_start: f64,
    /// Index of the next scanline to decode.
    row_index: usize,

    // The two reconstructed rows of the most recent pair, drained one pixel at a
    // time before the next pair is decoded.
    queue: [RgbPixel; 2 * WIDTH],
    queue_len: usize,
    queue_position: usize,
}

impl<I: Iterator<Item = i16>> Decoder<I> {
    /// Decode a transmission that begins with a Robot36 header.
    ///
    /// The leading calibration and VIS header is skipped before decoding the
    /// image. Returns [`Error::EmptyImage`] if no scanlines could be recovered.
    pub fn new(mut samples: I, sample_rate: u32) -> Result<Self> {
        let sample_rate = sample_rate.max(1);
        for _ in 0..header_sample_count(sample_rate) {
            if samples.next().is_none() {
                return Err(Error::EmptyImage);
            }
        }

        let mut decoder = Self::without_header(samples, sample_rate);
        // Decode the first pair eagerly so header-less / truncated input is
        // reported as an error rather than an empty iterator.
        if decoder.decode_pair().is_none() {
            return Err(Error::EmptyImage);
        }
        Ok(decoder)
    }

    /// Decode a transmission whose samples start at the image data, with no
    /// header present.
    ///
    /// Useful for decoding a signal already aligned to the first scanline, and
    /// as a testing entry point.
    pub fn without_header(samples: I, sample_rate: u32) -> Self {
        let sample_rate = sample_rate.max(1);
        let mode = Mode::Robot36;
        let pixel_luma = duration_to_samples(mode.pixel_luma_duration(), sample_rate);
        let pixel_chroma = duration_to_samples(mode.pixel_chroma_duration(), sample_rate);

        Self {
            demodulator: Demodulator::new(samples, sample_rate),
            pixel_luma,
            pixel_chroma,
            luma_len: WIDTH as f64 * pixel_luma,
            chroma_len: WIDTH as f64 * pixel_chroma,
            blank_len: duration_to_samples(mode.blank_duration(), sample_rate),
            back_porch_len: duration_to_samples(mode.back_porch_duration(), sample_rate),
            sync_hz: mode.sync_frequency().hz(),
            black_hz: mode.black_frequency().hz() as i64,
            white_hz: mode.white_frequency().hz() as i64,
            position: 0,
            current: Frequency::from_hz(0),
            luma_start: 0.0,
            row_index: 0,
            queue: [RgbPixel::new(0, 0, 0); 2 * WIDTH],
            queue_len: 0,
            queue_position: 0,
        }
    }

    /// Decode the next even/odd row pair into `queue`. Returns `None` once the
    /// image is complete or the signal runs out mid-pair.
    fn decode_pair(&mut self) -> Option<()> {
        if self.row_index >= HEIGHT {
            return None;
        }

        let (even_luma, even_chroma) = self.read_row()?; // even row carries R-Y
        self.row_index += 1;
        let (odd_luma, odd_chroma) = self.read_row()?; // odd row carries B-Y
        self.row_index += 1;

        // Both rows share the pair's chroma: the even row's red difference and
        // the odd row's blue difference.
        for pixel in 0..WIDTH {
            self.queue[pixel] = RgbPixel::from(YuvPixel::new(
                even_luma[pixel],
                even_chroma[pixel],
                odd_chroma[pixel],
            ));
            self.queue[WIDTH + pixel] = RgbPixel::from(YuvPixel::new(
                odd_luma[pixel],
                even_chroma[pixel],
                odd_chroma[pixel],
            ));
        }
        self.queue_len = 2 * WIDTH;
        self.queue_position = 0;
        Some(())
    }

    /// Read one scanline's luma and chroma, then consume its trailing sync so
    /// the next row is aligned to the actual pulse.
    fn read_row(&mut self) -> Option<([u8; WIDTH], [u8; WIDTH])> {
        let start = self.luma_start;

        let mut luma = [0u8; WIDTH];
        for (pixel, value) in luma.iter_mut().enumerate() {
            let center = start + (pixel as f64 + 0.5) * self.pixel_luma;
            *value = self.value_at(center)?;
        }

        let chroma_start = start + self.luma_len + self.blank_len;
        let mut chroma = [0u8; WIDTH];
        for (pixel, value) in chroma.iter_mut().enumerate() {
            let center = chroma_start + (pixel as f64 + 0.5) * self.pixel_chroma;
            *value = self.value_at(center)?;
        }

        // Advance to the end of the chroma scan, then consume the sync pulse and
        // set the next row's luma start just past the following back porch.
        self.advance_to(chroma_start + self.chroma_len)?;
        while !self.is_sync(self.current) {
            self.pull()?;
        }
        while self.is_sync(self.current) {
            self.pull()?;
        }
        self.luma_start = self.position as f64 + self.back_porch_len;

        Some((luma, chroma))
    }

    /// The pixel value sampled at a fractional sample position.
    fn value_at(&mut self, position: f64) -> Option<u8> {
        let frequency = self.advance_to(position)?;
        let value = (frequency.hz() as i64 - self.black_hz) * 255 / (self.white_hz - self.black_hz);
        Some(value.clamp(0, 255) as u8)
    }

    /// Pull samples until reaching the given (rounded) position and return the
    /// frequency there.
    fn advance_to(&mut self, position: f64) -> Option<Frequency> {
        let target = libm::round(position) as usize;
        while self.position < target {
            self.pull()?;
        }
        Some(self.current)
    }

    fn pull(&mut self) -> Option<()> {
        self.current = self.demodulator.next()?;
        self.position += 1;
        Some(())
    }

    fn is_sync(&self, frequency: Frequency) -> bool {
        frequency.hz().abs_diff(self.sync_hz) <= SYNC_TOLERANCE_HZ
    }
}

impl<I: Iterator<Item = i16>> Iterator for Decoder<I> {
    type Item = RgbPixel;

    fn next(&mut self) -> Option<RgbPixel> {
        if self.queue_position >= self.queue_len {
            self.decode_pair()?;
        }
        let pixel = self.queue[self.queue_position];
        self.queue_position += 1;
        Some(pixel)
    }
}

fn duration_to_samples(duration: Duration, sample_rate: u32) -> f64 {
    duration.ns() as f64 * sample_rate as f64 / 1_000_000_000.0
}

/// The number of samples occupied by the Robot36 header at a given sample rate.
fn header_sample_count(sample_rate: u32) -> usize {
    let total_ns: u64 = Mode::Robot36
        .header_sequence()
        .iter()
        .map(|tone| tone.duration.ns())
        .sum();
    (total_ns * sample_rate as u64 / 1_000_000_000) as usize
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec::Vec;

    use super::*;
    use crate::{Encoder, Synthesizer};

    /// A 320x240 test image with variation in all three channels.
    fn test_image() -> Vec<RgbPixel> {
        let mut pixels = Vec::with_capacity(WIDTH * HEIGHT);
        for y in 0..HEIGHT as u32 {
            for x in 0..WIDTH as u32 {
                let red = (x * 255 / (WIDTH as u32 - 1)) as u8;
                let green = (y * 255 / (HEIGHT as u32 - 1)) as u8;
                let blue = ((x + y) * 255 / (WIDTH as u32 - 1 + HEIGHT as u32 - 1)) as u8;
                pixels.push(RgbPixel::new(red, green, blue));
            }
        }
        pixels
    }

    fn encode(image: &[RgbPixel], sample_rate: u32) -> Vec<i16> {
        let encoder = Encoder::new(Mode::Robot36, image.to_vec().into_iter()).unwrap();
        Synthesizer::new(encoder, sample_rate).collect()
    }

    /// Mean absolute per-channel error between two images of equal length.
    fn mean_abs_error(a: &[RgbPixel], b: &[RgbPixel]) -> f64 {
        assert_eq!(a.len(), b.len());
        let total: u64 = a
            .iter()
            .zip(b)
            .map(|(p, q)| {
                let d = |x: u8, y: u8| (x as i32 - y as i32).unsigned_abs() as u64;
                d(p.red(), q.red()) + d(p.green(), q.green()) + d(p.blue(), q.blue())
            })
            .sum();
        total as f64 / (a.len() as f64 * 3.0)
    }

    #[test]
    fn round_trip_with_header() {
        let image = test_image();
        let samples = encode(&image, 48_000);

        let decoded: Vec<RgbPixel> = Decoder::new(samples.into_iter(), 48_000).unwrap().collect();

        assert_eq!(decoded.len(), WIDTH * HEIGHT);
        let error = mean_abs_error(&image, &decoded);
        assert!(error < 12.0, "mean abs error {error} too high");
    }

    #[test]
    fn round_trip_without_header() {
        let image = test_image();
        // Drop the header so the samples begin at the image data.
        let full = encode(&image, 48_000);
        let header = header_sample_count(48_000);
        let image_samples: Vec<i16> = full.into_iter().skip(header).collect();

        let decoded: Vec<RgbPixel> =
            Decoder::without_header(image_samples.into_iter(), 48_000).collect();

        assert_eq!(decoded.len(), WIDTH * HEIGHT);
        let error = mean_abs_error(&image, &decoded);
        assert!(error < 12.0, "mean abs error {error} too high");
    }

    #[test]
    fn decodes_all_scanlines() {
        let image = test_image();
        let samples = encode(&image, 48_000);
        let count = Decoder::new(samples.into_iter(), 48_000).unwrap().count();
        assert_eq!(count, WIDTH * HEIGHT);
    }

    #[test]
    fn too_short_signal_errors() {
        let samples = std::vec![0i16; 100];
        assert!(matches!(
            Decoder::new(samples.into_iter(), 48_000),
            Err(Error::EmptyImage)
        ));
    }
}
