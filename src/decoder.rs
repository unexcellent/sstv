use alloc::collections::VecDeque;
use alloc::vec::Vec;

use crate::modes::Mode;
use crate::units::Duration;
use crate::{Demodulator, Frequency, RgbPixel, YuvPixel};

/// Robot36 horizontal resolution. Fixed while only one mode is supported.
const WIDTH: usize = 320;
/// Robot36 vertical resolution.
const HEIGHT: usize = 240;
/// How far a frequency may stray from the sync frequency and still count as sync.
const SYNC_TOLERANCE_HZ: u32 = 150;

/// A single decoded scanline: `WIDTH` pixels in left-to-right order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RgbRow {
    index: usize,
    pixels: [RgbPixel; WIDTH],
}

impl RgbRow {
    /// The row's vertical position within its image; `0` is the top row.
    pub fn index(&self) -> usize {
        self.index
    }

    /// The row's pixels, left to right.
    pub fn pixels(&self) -> &[RgbPixel] {
        &self.pixels
    }
}

/// Metadata describing an image, reported when its decoding begins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageInfo {
    mode: Mode,
    width: usize,
    height: usize,
}

impl ImageInfo {
    /// The SSTV mode the image is encoded in.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// The image width in pixels.
    pub fn width(&self) -> usize {
        self.width
    }

    /// The image height in pixels.
    pub fn height(&self) -> usize {
        self.height
    }
}

/// An event produced by [`RowDecoder`] while scanning a sample stream.
///
/// A single image is reported as an [`ImageStart`](Event::ImageStart), followed
/// by one [`Row`](Event::Row) per decoded scanline in top-to-bottom order,
/// followed by an [`ImageEnd`](Event::ImageEnd). A stream that contains several
/// images — with or without gaps between them — yields these groups back to
/// back, one per image.
// `Row` carries a full scanline inline; that dwarfs the other variants, but the
// decoder emits events one at a time, so boxing it would only add an allocation.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A new image has been acquired; its rows follow.
    ImageStart(ImageInfo),
    /// One decoded scanline of the current image.
    Row(RgbRow),
    /// The current image finished.
    ImageEnd {
        /// Whether every scanline was decoded. `false` if the image was
        /// truncated before completion (for example, the signal faded out).
        complete: bool,
    },
}

/// Decodes a continuous stream of PCM samples into a stream of [`Event`]s.
///
/// `RowDecoder` is the streaming inverse of [`Encoder`](crate::Encoder): it
/// consumes 16-bit PCM samples and reports whole scanlines as it recovers them,
/// grouped into images by [`ImageStart`](Event::ImageStart) /
/// [`ImageEnd`](Event::ImageEnd) markers. Internally it runs a [`Demodulator`]
/// over the samples and pulls from it lazily, holding at most about one
/// row-pair at a time — never the whole frequency track or image.
///
/// Unlike a one-shot decoder it keeps scanning after an image completes, so a
/// signal that carries images only part of the time, or several images one
/// after another, is decoded as a sequence of image event-groups. Acquisition
/// happens lazily as the iterator is polled; a stream that never contains an
/// image simply yields no events.
///
/// ```no_run
/// use sstv::{Event, RowDecoder};
///
/// # let samples = std::vec::Vec::<i16>::new().into_iter();
/// for event in RowDecoder::new(samples, 48000) {
///     match event {
///         Event::ImageStart(info) => { let _ = info.width(); }
///         Event::Row(row) => { let _ = row.pixels(); }
///         Event::ImageEnd { complete } => { let _ = complete; }
///     }
/// }
/// ```
pub struct RowDecoder<I: Iterator<Item = i16>> {
    demodulator: Demodulator<I>,
    sample_rate: u32,

    // Frequencies buffered while locking onto a line sync, replayed before
    // pulling live samples so the header search is not lost. Cleared at the
    // start of each acquisition.
    prefix: Vec<Frequency>,
    prefix_position: usize,

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

    // Streaming position: `position` counts samples pulled from the current
    // acquisition, `current` is the most recently pulled frequency.
    position: usize,
    current: Frequency,
    /// Sample position at which the current row's luma scan begins.
    luma_start: f64,
    /// Index of the next scanline to decode within the current image.
    row_index: usize,

    state: State,
    /// Decoded events waiting to be handed out, oldest first.
    events: VecDeque<Event>,
}

/// Where the decoder is in the acquire → decode → acquire cycle.
enum State {
    /// Scanning for the next image's first line sync.
    Searching,
    /// Emitting the rows of the image currently being decoded.
    Decoding,
    /// The sample stream is exhausted; no more events will be produced.
    Done,
}

impl<I: Iterator<Item = i16>> RowDecoder<I> {
    /// Create a `RowDecoder` over a stream of PCM samples.
    ///
    /// `sample_rate` must be greater than zero. Construction never fails:
    /// finding images is deferred to iteration.
    pub fn new(samples: I, sample_rate: u32) -> Self {
        let sample_rate = sample_rate.max(1);
        let mode = Mode::Robot36;
        let pixel_luma = duration_to_samples(mode.pixel_luma_duration(), sample_rate);
        let pixel_chroma = duration_to_samples(mode.pixel_chroma_duration(), sample_rate);

        Self {
            demodulator: Demodulator::new(samples, sample_rate),
            sample_rate,
            prefix: Vec::new(),
            prefix_position: 0,
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
            state: State::Searching,
            events: VecDeque::new(),
        }
    }

    /// Create a `RowDecoder` for a stream whose samples begin at the image
    /// data, with no header to search for.
    ///
    /// Decoding starts immediately at the first scanline: the first event is an
    /// [`Event::ImageStart`], followed by the image's rows. Use this when the
    /// signal carries no detectable header, or when acquisition has already been
    /// performed upstream and the samples are aligned to the first row's luma.
    /// After the image completes, the decoder resumes searching for further
    /// images exactly as [`new`](Self::new) does.
    pub fn without_header(samples: I, sample_rate: u32) -> Self {
        let mut decoder = Self::new(samples, sample_rate);
        let info = decoder.image_info();
        decoder.events.push_back(Event::ImageStart(info));
        decoder.state = State::Decoding;
        decoder
    }

    /// Metadata for the mode currently being decoded.
    fn image_info(&self) -> ImageInfo {
        ImageInfo {
            mode: Mode::Robot36,
            width: WIDTH,
            height: HEIGHT,
        }
    }

    /// Scan for the next image's first line sync. On success, queue an
    /// [`Event::ImageStart`] and begin decoding; on stream exhaustion, finish.
    fn search(&mut self) {
        // Start a fresh acquisition buffer for this image.
        self.prefix.clear();
        self.prefix_position = 0;

        match self.lock_onto_first_row() {
            Some(luma_start) => {
                // Drop the buffered header so only the image data is replayed.
                let trim = (luma_start.max(0.0) as usize).min(self.prefix.len());
                self.prefix.drain(0..trim);
                self.luma_start = luma_start - trim as f64;
                self.position = 0;
                self.prefix_position = 0;
                self.row_index = 0;
                self.events.push_back(Event::ImageStart(self.image_info()));
                self.state = State::Decoding;
            }
            None => self.state = State::Done,
        }
    }

    /// Decode the next row-pair, queueing its [`Event::Row`]s. Ends the image
    /// with an [`Event::ImageEnd`] once every row is decoded, or if the signal
    /// runs out mid-image.
    fn decode_step(&mut self) {
        if self.row_index >= HEIGHT {
            self.events.push_back(Event::ImageEnd { complete: true });
            self.state = State::Searching;
            return;
        }

        if self.decode_pair().is_none() {
            self.events.push_back(Event::ImageEnd { complete: false });
            self.state = State::Done;
        }
    }

    /// Decode one even/odd row pair and queue both rows. Returns `None` if the
    /// signal runs out before the pair is complete.
    fn decode_pair(&mut self) -> Option<()> {
        let first_index = self.row_index;
        let (first_luma, first_chroma, first_porch) = self.read_row()?;
        self.row_index += 1;
        let second_index = self.row_index;
        let (second_luma, second_chroma, second_porch) = self.read_row()?;
        self.row_index += 1;

        // A pair is one even (R-Y) and one odd (B-Y) line; both are reconstructed
        // from the shared red and blue differences. Which line carries which is
        // read from the porch (black on even, white on odd) rather than assumed
        // from read order — so a decode that began on an odd line doesn't swap
        // red and blue. The even line's chroma is the red difference.
        let first_is_even = first_porch <= second_porch;
        let (red_chroma, blue_chroma) = if first_is_even {
            (&first_chroma, &second_chroma)
        } else {
            (&second_chroma, &first_chroma)
        };

        let mut first_pixels = [RgbPixel::new(0, 0, 0); WIDTH];
        let mut second_pixels = [RgbPixel::new(0, 0, 0); WIDTH];
        for pixel in 0..WIDTH {
            first_pixels[pixel] = RgbPixel::from(YuvPixel::new(
                first_luma[pixel],
                red_chroma[pixel],
                blue_chroma[pixel],
            ));
            second_pixels[pixel] = RgbPixel::from(YuvPixel::new(
                second_luma[pixel],
                red_chroma[pixel],
                blue_chroma[pixel],
            ));
        }

        self.events.push_back(Event::Row(RgbRow {
            index: first_index,
            pixels: first_pixels,
        }));
        self.events.push_back(Event::Row(RgbRow {
            index: second_index,
            pixels: second_pixels,
        }));
        Some(())
    }

    /// Read one scanline's luma and chroma, then consume its trailing sync so
    /// the next row is aligned to the actual pulse.
    fn read_row(&mut self) -> Option<([u8; WIDTH], [u8; WIDTH], u8)> {
        let start = self.luma_start;

        let mut luma = [0u8; WIDTH];
        for (pixel, value) in luma.iter_mut().enumerate() {
            let center = start + (pixel as f64 + 0.5) * self.pixel_luma;
            *value = self.value_at(center)?;
        }

        // Sample the porch — the first part of the blank between luma and chroma.
        // It's black on even (R-Y) rows and white on odd (B-Y) rows, so it marks
        // this line's parity independently of where decoding started.
        let porch = self.value_at(start + self.luma_len + self.blank_len / 3.0)?;

        let chroma_start = start + self.luma_len + self.blank_len;
        let mut chroma = [0u8; WIDTH];
        for (pixel, value) in chroma.iter_mut().enumerate() {
            let center = chroma_start + (pixel as f64 + 0.5) * self.pixel_chroma;
            *value = self.value_at(center)?;
        }

        // The pixels are read; consume the trailing sync to align the next row.
        // If the stream ends here (e.g. the final line carries no trailing
        // sync), keep the row we just decoded rather than discarding it.
        if self.advance_to(chroma_start + self.chroma_len).is_none() || !self.consume_sync() {
            self.luma_start = self.position as f64;
        }

        Some((luma, chroma, porch))
    }

    /// Consume the current sync pulse and set the next row's luma start just
    /// past the following back porch. Returns `false` if the stream ends first.
    fn consume_sync(&mut self) -> bool {
        while !self.is_sync(self.current) {
            if self.pull().is_none() {
                return false;
            }
        }
        while self.is_sync(self.current) {
            if self.pull().is_none() {
                return false;
            }
        }
        self.luma_start = self.position as f64 + self.back_porch_len;
        true
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
        self.current = if self.prefix_position < self.prefix.len() {
            let frequency = self.prefix[self.prefix_position];
            self.prefix_position += 1;
            frequency
        } else {
            self.demodulator.next()?
        };
        self.position += 1;
        Some(())
    }

    fn is_sync(&self, frequency: Frequency) -> bool {
        frequency.hz().abs_diff(self.sync_hz) <= SYNC_TOLERANCE_HZ
    }

    /// Buffer frequencies from the demodulator until a line sync is located, and
    /// return the sample position at which that row's luma begins.
    ///
    /// A candidate sync is a run near the sync frequency lasting roughly one
    /// sync duration (long enough to exclude glitches, short enough to exclude
    /// the header's longer VIS tones). The first candidate that is followed by
    /// another candidate about one line later is a row's trailing sync; the
    /// row's data is the luma, blank and chroma scans preceding it.
    fn lock_onto_first_row(&mut self) -> Option<f64> {
        let mode = Mode::Robot36;
        let sync_len = duration_to_samples(mode.sync_duration(), self.sample_rate);
        let line_len = duration_to_samples(mode.line_duration(), self.sample_rate);
        let row_data_len = self.luma_len + self.blank_len + self.chroma_len;

        let min_run = (sync_len * 0.5) as usize;
        let max_run = (sync_len * 2.0) as usize;
        let spacing_tolerance = line_len * 0.15;

        let mut previous_sync: Option<usize> = None;
        let mut index = 0usize;
        loop {
            if !self.buffered_is_sync(index)? {
                index += 1;
                continue;
            }

            // Measure the length of this sync run.
            let start = index;
            while self.buffered_is_sync(index)? {
                index += 1;
            }
            let run = index - start;
            if run < min_run || run > max_run {
                continue;
            }

            match previous_sync {
                Some(previous)
                    if ((start - previous) as f64 - line_len).abs() <= spacing_tolerance =>
                {
                    return Some(previous as f64 - row_data_len);
                }
                _ => previous_sync = Some(start),
            }
        }
    }

    /// Whether the buffered frequency at `index` is a sync, extending the buffer
    /// from the demodulator as needed.
    fn buffered_is_sync(&mut self, index: usize) -> Option<bool> {
        while self.prefix.len() <= index {
            let frequency = self.demodulator.next()?;
            self.prefix.push(frequency);
        }
        Some(self.is_sync(self.prefix[index]))
    }
}

impl<I: Iterator<Item = i16>> Iterator for RowDecoder<I> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        loop {
            if let Some(event) = self.events.pop_front() {
                return Some(event);
            }
            match self.state {
                State::Done => return None,
                State::Searching => self.search(),
                State::Decoding => self.decode_step(),
            }
        }
    }
}

fn duration_to_samples(duration: Duration, sample_rate: u32) -> f64 {
    duration.ns() as f64 * sample_rate as f64 / 1_000_000_000.0
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
        // `to_vec` is required: `Encoder::new` needs an owned (`'static`)
        // iterator, so borrowing with `iter().copied()` would not compile.
        #[allow(clippy::unnecessary_to_owned)]
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

    /// One image reassembled from the decoder's event stream.
    struct DecodedImage {
        #[allow(dead_code)]
        info: ImageInfo,
        rows: Vec<RgbRow>,
        complete: bool,
    }

    impl DecodedImage {
        /// Flatten the rows into a single raster-order pixel buffer.
        fn pixels(&self) -> Vec<RgbPixel> {
            let mut pixels = Vec::with_capacity(self.rows.len() * WIDTH);
            for row in &self.rows {
                pixels.extend_from_slice(row.pixels());
            }
            pixels
        }
    }

    /// The number of samples occupied by our encoder's header at a sample rate.
    fn header_sample_count(sample_rate: u32) -> usize {
        let total_ns: u64 = Mode::Robot36
            .header_sequence()
            .iter()
            .map(|tone| tone.duration.ns())
            .sum();
        (total_ns * sample_rate as u64 / 1_000_000_000) as usize
    }

    /// Drive a decoder to completion, grouping its events into images.
    fn collect_images<I: Iterator<Item = i16>>(decoder: RowDecoder<I>) -> Vec<DecodedImage> {
        let mut images = Vec::new();
        let mut current: Option<(ImageInfo, Vec<RgbRow>)> = None;

        for event in decoder {
            match event {
                Event::ImageStart(info) => current = Some((info, Vec::new())),
                Event::Row(row) => {
                    let (_, rows) = current
                        .as_mut()
                        .expect("Row without a preceding ImageStart");
                    rows.push(row);
                }
                Event::ImageEnd { complete } => {
                    let (info, rows) = current.take().expect("ImageEnd without an ImageStart");
                    images.push(DecodedImage {
                        info,
                        rows,
                        complete,
                    });
                }
            }
        }

        images
    }

    #[test]
    fn round_trip_two_images_back_to_back() {
        let image = test_image();
        let mut samples = encode(&image, 48_000);
        samples.extend(encode(&image, 48_000));

        let decoded = collect_images(RowDecoder::new(samples.into_iter(), 48_000));

        assert_eq!(decoded.len(), 2, "expected two images");
        for decoded_image in &decoded {
            assert!(decoded_image.complete);
            assert_eq!(decoded_image.rows.len(), HEIGHT);
            let error = mean_abs_error(&image, &decoded_image.pixels());
            assert!(error < 12.0, "mean abs error {error} too high");
        }
    }

    #[test]
    fn round_trip_two_images_with_gap() {
        let image = test_image();
        let mut samples = encode(&image, 48_000);
        // Half a second of silence between the two transmissions.
        samples.extend(std::vec![0i16; 24_000]);
        samples.extend(encode(&image, 48_000));

        let decoded = collect_images(RowDecoder::new(samples.into_iter(), 48_000));

        assert_eq!(decoded.len(), 2, "expected two images across the gap");
        for decoded_image in &decoded {
            assert!(decoded_image.complete);
            assert_eq!(decoded_image.rows.len(), HEIGHT);
            let error = mean_abs_error(&image, &decoded_image.pixels());
            assert!(error < 12.0, "mean abs error {error} too high");
        }
    }

    #[test]
    fn round_trip_without_header() {
        let image = test_image();
        // Drop the header so the samples begin at the first scanline's data.
        let full = encode(&image, 48_000);
        let header = header_sample_count(48_000);
        let image_samples: Vec<i16> = full.into_iter().skip(header).collect();

        let decoded = collect_images(RowDecoder::without_header(
            image_samples.into_iter(),
            48_000,
        ));

        assert_eq!(decoded.len(), 1, "expected exactly one image");
        let decoded_image = &decoded[0];
        assert!(decoded_image.complete, "image should decode completely");
        assert_eq!(decoded_image.rows.len(), HEIGHT, "should decode all rows");
        // Rows arrive top to bottom.
        for (y, row) in decoded_image.rows.iter().enumerate() {
            assert_eq!(row.index(), y, "row {y} out of order");
        }
        let error = mean_abs_error(&image, &decoded_image.pixels());
        assert!(error < 12.0, "mean abs error {error} too high");
    }

    #[test]
    fn silence_yields_no_images() {
        let decoded = collect_images(RowDecoder::new(std::vec![0i16; 48_000].into_iter(), 48_000));
        assert!(decoded.is_empty(), "silence should not produce an image");
    }
}
