//! Walking the transmission: event emission and the state machine behind it.

use alloc::collections::VecDeque;
use alloc::{vec, vec::Vec};

use crate::modes::step::Step;
use crate::modes::{BLACK_FREQUENCY, Mode, ROBOT_36, SYNC_FREQUENCY, WHITE_FREQUENCY};
use crate::{Demodulator, RgbPixel};

use super::acquire::{detect_mode, is_sync, lock_onto_first_line};
use super::assemble::{Assembler, SequenceData};
use super::stream::FrequencyStream;

/// A single decoded scanline: one image width of pixels in left-to-right order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbRow {
    index: usize,
    pixels: Vec<RgbPixel>,
}

impl RgbRow {
    /// The row's vertical position within its image; `0` is the top row.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// The row's pixels, left to right.
    #[must_use]
    pub fn pixels(&self) -> &[RgbPixel] {
        &self.pixels
    }
}

/// An event produced by a [`Decoder`](super::Decoder) while scanning a sample stream.
///
/// A single image is reported as an [`ImageStart`](Event::ImageStart), followed
/// by one [`Row`](Event::Row) per decoded scanline in top-to-bottom order,
/// followed by an [`ImageEnd`](Event::ImageEnd). A stream that contains several
/// images — with or without gaps between them — yields these groups back to
/// back, one per image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A new image in the given mode has been acquired; its rows follow. The
    /// image dimensions are the mode's [`resolution`](Mode::resolution).
    ImageStart(Mode),
    /// One decoded scanline of the current image.
    Row(RgbRow),
    /// The current image finished.
    ImageEnd {
        /// Whether every scanline was decoded. `false` if the image was
        /// truncated before completion (for example, the signal faded out).
        complete: bool,
    },
}

/// The event stream of a [`Decoder`](super::Decoder): scanlines as they are recovered,
/// grouped into images by [`Event::ImageStart`] and [`Event::ImageEnd`]
/// markers.
///
/// It holds at most about one line group at a time — never the whole
/// frequency track or image.
pub struct Events<I: Iterator<Item = i16>> {
    stream: FrequencyStream<I>,
    /// The mode pinned via [`Decoder::expect_mode`](super::Decoder::expect_mode); `None` detects each
    /// image's mode from its header.
    pub(super) expected_mode: Option<Mode>,
    /// Begin decoding immediately instead of searching for the first header.
    pub(super) skip_header: bool,
    state: State,
    /// Decoded events waiting to be handed out, oldest first.
    queue: VecDeque<Event>,
}

/// Where the decoder is in the acquire → decode → acquire cycle.
enum State {
    /// Scanning for the next image.
    Searching,
    /// Decoding the rows of the current image.
    Decoding(ImageState),
    /// The sample stream is exhausted; no more events will be produced.
    Done,
}

/// Everything needed to decode the image currently being worked on.
struct ImageState {
    mode: Mode,
    /// Fractional sample position at which the next timing sequence begins.
    sequence_start: f64,
    /// Index of the next image line to decode.
    row_index: usize,
    assembler: Assembler,
}

impl ImageState {
    const fn new(mode: Mode, sequence_start: f64) -> Self {
        Self {
            mode,
            sequence_start,
            row_index: 0,
            assembler: Assembler::new(mode.color),
        }
    }

    /// Walk one timing sequence: consume sync pulses to re-align, sample the
    /// centre of every other tone, and sample each scan's pixels.
    ///
    /// A transmission ends flush with its final scan, and the demodulator's
    /// warm-up makes the frequency stream end slightly before the signal does
    /// — so if the stream runs out inside the last scan, the missing tail is
    /// padded with the last sampled value and the sequence is still returned.
    /// Returns `None` only when the stream ends before every scan was (at
    /// least partially) sampled.
    fn read_sequence<I: Iterator<Item = i16>>(
        &mut self,
        stream: &mut FrequencyStream<I>,
    ) -> Option<SequenceData> {
        let sequence = self.mode.sequence;
        let width = self.mode.resolution.0;
        let expected_scans = sequence
            .iter()
            .filter(|step| matches!(step, Step::Scan(..)))
            .count();
        let mut t = self.sequence_start;
        let mut scans = Vec::with_capacity(4);
        let mut stream_ended = false;

        'steps: for step in sequence {
            match step {
                // Re-align on the actual sync pulse rather than trusting the
                // nominal timing.
                Step::Control(tone) if tone.frequency == SYNC_FREQUENCY => {
                    if stream.advance_to(t).is_none() || consume_sync(stream).is_none() {
                        stream_ended = true;
                        break 'steps;
                    }
                    t = stream.position() as f64;
                }
                Step::Control(tone) => {
                    let len = stream.samples_in(tone.duration);
                    if stream.advance_to(t + len / 2.0).is_none() {
                        stream_ended = true;
                        break 'steps;
                    }
                    t += len;
                }
                Step::Scan(channel, duration) => {
                    let len = stream.samples_in(*duration);
                    let pixel_len = len / width as f64;
                    let mut values = vec![0u8; width];
                    let mut last = 0u8;
                    for (x, value) in values.iter_mut().enumerate() {
                        if let Some(sampled) = value_at(stream, t + (x as f64 + 0.5) * pixel_len) {
                            *value = sampled;
                            last = sampled;
                        } else {
                            *value = last;
                            stream_ended = true;
                        }
                    }
                    t += len;
                    scans.push((*channel, values));
                    if stream_ended {
                        break 'steps;
                    }
                }
            }
        }

        if stream_ended && scans.len() < expected_scans {
            return None;
        }
        self.sequence_start = t;
        Some(SequenceData { scans })
    }
}

/// Consume the sync pulse ahead: skip to its start, then through it.
/// Returns `None` if the stream ends first.
fn consume_sync<I: Iterator<Item = i16>>(stream: &mut FrequencyStream<I>) -> Option<()> {
    while !is_sync(stream.current()) {
        stream.advance()?;
    }
    while is_sync(stream.current()) {
        stream.advance()?;
    }
    Some(())
}

/// The pixel value sampled at a fractional sample position.
fn value_at<I: Iterator<Item = i16>>(stream: &mut FrequencyStream<I>, position: f64) -> Option<u8> {
    let frequency = stream.advance_to(position)?;
    let black = i64::from(BLACK_FREQUENCY.hz());
    let white = i64::from(WHITE_FREQUENCY.hz());
    let value = (i64::from(frequency.hz()) - black) * 255 / (white - black);
    Some(value.clamp(0, 255) as u8)
}

impl<I: Iterator<Item = i16>> Events<I> {
    pub(super) const fn new(demodulator: Demodulator<I>) -> Self {
        Self {
            stream: FrequencyStream::new(demodulator),
            expected_mode: None,
            skip_header: false,
            state: State::Searching,
            queue: VecDeque::new(),
        }
    }

    /// Begin decoding at the first line's timing sequence instead of
    /// searching for a header. Without a header there is no VIS code, so an
    /// unpinned mode falls back to Robot 36.
    fn start_without_header(&mut self) {
        let mode = self.expected_mode.unwrap_or(ROBOT_36);
        let image = ImageState::new(mode, 0.0);
        self.queue.push_back(Event::ImageStart(image.mode));
        self.state = State::Decoding(image);
    }

    /// Scan for the next image. On success, queue an [`Event::ImageStart`]
    /// and begin decoding; on stream exhaustion, finish.
    fn search(&mut self) {
        self.stream.reset();

        let acquired = match self.expected_mode {
            None => detect_mode(&mut self.stream),
            Some(mode) => lock_onto_first_line(&mut self.stream, mode)
                .map(|sequence_start| (mode, sequence_start)),
        };

        match acquired {
            Some((mode, sequence_start)) => {
                // Replay only from the first line onward.
                let origin = self.stream.start_at(sequence_start.max(0.0) as usize);
                let image = ImageState::new(mode, sequence_start - origin as f64);
                self.queue.push_back(Event::ImageStart(image.mode));
                self.state = State::Decoding(image);
            }
            None => self.state = State::Done,
        }
    }

    /// Decode the next timing sequence, queueing its rows. Ends the image with
    /// an [`Event::ImageEnd`] once every line is decoded, or if the signal
    /// runs out mid-image.
    fn decode_step(&mut self) {
        let State::Decoding(image) = &mut self.state else {
            return;
        };

        if image.row_index >= image.mode.resolution.1 {
            self.queue.push_back(Event::ImageEnd { complete: true });
            self.state = State::Searching;
            return;
        }

        if let Some(data) = image.read_sequence(&mut self.stream) {
            for pixels in image.assembler.assemble(&data) {
                let index = image.row_index;
                image.row_index += 1;
                self.queue.push_back(Event::Row(RgbRow { index, pixels }));
            }
        } else {
            self.queue.push_back(Event::ImageEnd { complete: false });
            self.state = State::Done;
        }
    }
}

impl<I: Iterator<Item = i16>> Iterator for Events<I> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        loop {
            if let Some(event) = self.queue.pop_front() {
                return Some(event);
            }
            match self.state {
                State::Done => return None,
                State::Searching if core::mem::take(&mut self.skip_header) => {
                    self.start_without_header();
                }
                State::Searching => self.search(),
                State::Decoding(_) => self.decode_step(),
            }
        }
    }
}

/// Test images and round trips through the encoder, shared by the decoder's
/// tests.
#[cfg(test)]
pub mod testing {
    extern crate std;
    use std::vec::Vec;

    use crate::modes::{Mode, ROBOT_36};
    use crate::{DecodedImage, Encoder, RgbPixel, Synthesizer};

    pub const WIDTH: usize = 320;
    pub const HEIGHT: usize = 240;

    /// A 320x240 test image with variation in all three channels.
    pub fn test_image() -> Vec<RgbPixel> {
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

    pub fn encode(image: &[RgbPixel], sample_rate: u32) -> Vec<i16> {
        let encoder = Encoder::new(ROBOT_36, image.iter().copied()).unwrap();
        Synthesizer::new(encoder, sample_rate).collect()
    }

    /// Mean absolute per-channel error between two images of equal length.
    pub fn mean_abs_error(a: &[RgbPixel], b: &[RgbPixel]) -> f64 {
        assert_eq!(a.len(), b.len());
        let total: u64 = a
            .iter()
            .zip(b)
            .map(|(p, q)| {
                let d = |x: u8, y: u8| u64::from((i32::from(x) - i32::from(y)).unsigned_abs());
                d(p.red(), q.red()) + d(p.green(), q.green()) + d(p.blue(), q.blue())
            })
            .sum();
        total as f64 / (a.len() as f64 * 3.0)
    }

    /// The number of samples occupied by our encoder's header at a sample rate.
    pub fn header_sample_count(sample_rate: u32) -> usize {
        let total_ns: u64 = ROBOT_36.header_tones().map(|tone| tone.duration.ns()).sum();
        (total_ns * u64::from(sample_rate) / 1_000_000_000) as usize
    }

    pub fn assert_matches(decoded: &DecodedImage, image: &[RgbPixel]) {
        assert!(decoded.complete(), "image should decode completely");
        assert_eq!(decoded.pixels().len(), WIDTH * HEIGHT);
        let error = mean_abs_error(image, decoded.pixels());
        assert!(error < 12.0, "mean abs error {error} too high");
    }

    /// An image at the mode's resolution, brightening to the right in red and
    /// downwards in green.
    pub fn gradient_image(mode: Mode) -> Vec<RgbPixel> {
        let (width, height) = mode.resolution();
        let pixel_at = |row: u32, column: u32| {
            RgbPixel::new(
                (column * 255 / width) as u8,
                (row * 255 / height) as u8,
                128,
            )
        };
        (0..height)
            .flat_map(|row| (0..width).map(move |column| pixel_at(row, column)))
            .collect()
    }

    /// The samples of the image's transmission, starting right after the
    /// header.
    pub fn encode_without_header(mode: Mode, image: &[RgbPixel], sample_rate: u32) -> Vec<i16> {
        let encoder = Encoder::new(mode, image.iter().copied()).unwrap();
        let header_ns: u64 = mode.header_tones().map(|tone| tone.duration.ns()).sum();
        let header_samples = (header_ns * u64::from(sample_rate) / 1_000_000_000) as usize;
        Synthesizer::new(encoder, sample_rate)
            .skip(header_samples)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec::Vec;

    use super::testing::{
        HEIGHT, WIDTH, assert_matches, encode, encode_without_header, gradient_image,
        header_sample_count, mean_abs_error, test_image,
    };
    use super::*;
    use crate::modes::SCOTTIE_1;
    use crate::{DecodedImage, Decoder};

    /// Entering the stream at a pair's second line must not swap the colour
    /// differences: acquisition resolves which of the sequence's two sync
    /// pulses it locked onto and aligns to the next full pair.
    #[test]
    fn sync_lock_on_an_odd_line_does_not_swap_colours() {
        let image = test_image();
        let full = encode(&image, 48_000);

        // Drop the header and the pair's first 150ms line, so the stream
        // begins at an odd line's sync pulse.
        let line_samples = (48_000.0 * 0.150) as usize;
        let skip = header_sample_count(48_000) + line_samples;

        let decoded: Vec<DecodedImage> = Decoder::from_samples(full.into_iter().skip(skip), 48_000)
            .expect_mode(ROBOT_36)
            .images()
            .collect();

        assert_eq!(decoded.len(), 1, "expected one image");
        // Decoding aligns to the next full pair, so the image shifts up by
        // the two dropped lines; the last two rows stay unfilled.
        let decoded_rows = &decoded[0].pixels()[..WIDTH * (HEIGHT - 2)];
        let original_rows = &image[WIDTH * 2..];
        let error = mean_abs_error(original_rows, decoded_rows);
        assert!(error < 12.0, "mean abs error {error} too high");
    }

    /// Scottie's sync pulse sits mid-sequence, so a sync lock must step back
    /// by the sync offset to find where the line begins.
    #[test]
    fn scottie_decodes_by_sync_lock_without_a_header() {
        let image = gradient_image(SCOTTIE_1);
        let transmission = encode_without_header(SCOTTIE_1, &image, 48_000);

        let decoded_images: Vec<DecodedImage> =
            Decoder::from_samples(transmission.into_iter(), 48_000)
                .expect_mode(SCOTTIE_1)
                .images()
                .collect();

        assert_eq!(decoded_images.len(), 1, "expected one image");
        assert!(
            decoded_images[0].complete(),
            "image should decode completely"
        );
        let error = mean_abs_error(&image, decoded_images[0].pixels());
        assert!(error < 12.0, "mean abs error {error} too high");
    }

    #[test]
    fn round_trip_two_images_back_to_back() {
        let image = test_image();
        let mut samples = encode(&image, 48_000);
        samples.extend(encode(&image, 48_000));

        let decoded: Vec<DecodedImage> = Decoder::from_samples(samples.into_iter(), 48_000)
            .expect_mode(ROBOT_36)
            .images()
            .collect();

        assert_eq!(decoded.len(), 2, "expected two images");
        for decoded_image in &decoded {
            assert_matches(decoded_image, &image);
        }
    }

    #[test]
    fn round_trip_two_images_with_gap() {
        let image = test_image();
        let mut samples = encode(&image, 48_000);
        // Half a second of silence between the two transmissions.
        samples.extend(std::vec![0i16; 24_000]);
        samples.extend(encode(&image, 48_000));

        let decoded: Vec<DecodedImage> = Decoder::from_samples(samples.into_iter(), 48_000)
            .expect_mode(ROBOT_36)
            .images()
            .collect();

        assert_eq!(decoded.len(), 2, "expected two images across the gap");
        for decoded_image in &decoded {
            assert_matches(decoded_image, &image);
        }
    }

    #[test]
    fn auto_detects_mode_when_front_of_header_is_trimmed() {
        let image = test_image();
        let full = encode(&image, 48_000);

        // Emulate a recording clipped at the front (e.g. a trimmed video): drop
        // the VOX tones, first leader and break, leaving the second leader
        // running straight into the VIS bits. Auto detection must still lock on.
        let trimmed: u64 = (0..=9)
            .map(|index| ROBOT_36.header_tone(index).unwrap().duration.ns())
            .sum();
        let skip = (trimmed * 48_000 / 1_000_000_000) as usize;

        let decoded: Vec<DecodedImage> = Decoder::from_samples(full.into_iter().skip(skip), 48_000)
            .images()
            .collect();

        assert_eq!(decoded.len(), 1, "expected one image");
        assert_eq!(decoded[0].mode(), ROBOT_36);
        assert_matches(&decoded[0], &image);
    }

    #[test]
    fn silence_yields_no_images() {
        let decoded = Decoder::from_samples(std::vec![0i16; 48_000].into_iter(), 48_000)
            .expect_mode(ROBOT_36)
            .images()
            .next();
        assert!(decoded.is_none(), "silence should not produce an image");
    }
}
