use alloc::collections::VecDeque;
use alloc::{vec, vec::Vec};

use crate::modes::layout::{Channel, ColorMode, Layout, Step};
use crate::modes::{BLACK_FREQUENCY, LEADER_FREQUENCY, Mode, SYNC_FREQUENCY, WHITE_FREQUENCY};
use crate::units::Duration;
use crate::{Demodulator, Frequency, RgbPixel, YuvPixel};

/// How far a frequency may stray from a nominal tone and still count as it.
const TONE_TOLERANCE_HZ: u32 = 150;

/// A single decoded scanline: [`ImageInfo::width`] pixels in left-to-right order.
#[derive(Debug, Clone, PartialEq)]
pub struct RgbRow {
    index: usize,
    pixels: Vec<RgbPixel>,
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
/// over the samples and walks the mode's timing sequence as specified in the
/// Dayton paper, re-aligning on every sync pulse. It pulls from the stream
/// lazily, holding at most about one line group at a time — never the whole
/// frequency track or image.
///
/// Unlike a one-shot decoder it keeps scanning after an image completes, so a
/// signal that carries images only part of the time, or several images one
/// after another, is decoded as a sequence of image event-groups. Acquisition
/// happens lazily as the iterator is polled; a stream that never contains an
/// image simply yields no events.
///
/// ```no_run
/// use sstv::{Event, Mode, RowDecoder};
///
/// # let samples = std::vec::Vec::<i16>::new().into_iter();
/// for event in RowDecoder::new(Mode::Robot36, samples, 48000) {
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
    /// The mode requested at construction, possibly [`Mode::Auto`].
    requested_mode: Mode,
    /// The mode of the image currently being decoded.
    mode: Mode,
    layout: Layout,

    // Frequencies buffered while locking onto a line sync, replayed before
    // pulling live samples so no signal is lost to the search. Cleared at the
    // start of each acquisition.
    prefix: Vec<Frequency>,
    prefix_position: usize,

    // Streaming position: `position` counts samples pulled from the current
    // acquisition, `current` is the most recently pulled frequency.
    position: usize,
    current: Frequency,

    /// Fractional sample position at which the next timing sequence begins.
    sequence_start: f64,
    /// Which of the mode's timing sequences the next line uses.
    sequence_index: usize,
    /// Index of the next image line to decode within the current image.
    row_index: usize,
    /// Robot 36: the decoded first line of a pair, awaiting its partner.
    pending_pair: Option<PendingLine>,

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

/// One half of a Robot 36 line pair: its luminance, its colour-difference
/// scan, and the sampled separator-pulse frequency identifying which colour
/// difference it carries.
struct PendingLine {
    luma: Vec<u8>,
    chroma: Vec<u8>,
    separator_hz: u32,
}

/// The sampled contents of one pass through a timing sequence.
struct SequenceData {
    /// Each scan step's channel and its sampled pixel values.
    scans: Vec<(Channel, Vec<u8>)>,
    /// The frequency sampled at the centre of each non-sync tone step, in the
    /// order the steps occur. Robot 36 reads its separator pulse from here.
    tone_hz: Vec<u32>,
}

impl<I: Iterator<Item = i16>> RowDecoder<I> {
    /// Create a `RowDecoder` that scans a stream of PCM samples for images in
    /// the given mode.
    ///
    /// With [`Mode::Auto`], the mode of each image is detected from its
    /// header's VIS code, and [`ImageInfo`] reports the detected mode.
    ///
    /// `sample_rate` must be greater than zero. Construction never fails:
    /// finding images is deferred to iteration.
    pub fn new(mode: Mode, samples: I, sample_rate: u32) -> Self {
        let sample_rate = sample_rate.max(1);
        let active = match mode {
            Mode::Auto => Mode::Robot36,
            mode => mode,
        };
        Self {
            demodulator: Demodulator::new(samples, sample_rate),
            sample_rate,
            requested_mode: mode,
            mode: active,
            layout: active.layout(),
            prefix: Vec::new(),
            prefix_position: 0,
            position: 0,
            current: Frequency::from_hz(0),
            sequence_start: 0.0,
            sequence_index: 0,
            row_index: 0,
            pending_pair: None,
            state: State::Searching,
            events: VecDeque::new(),
        }
    }

    /// Create a `RowDecoder` for a stream whose samples begin at the image
    /// data, with no header to skip.
    ///
    /// Decoding starts immediately at the first line's timing sequence: the
    /// first event is an [`Event::ImageStart`], followed by the image's rows.
    /// Use this when the signal carries no detectable header, or when
    /// acquisition has already been performed upstream and the samples are
    /// aligned to the first line. After the image completes, the decoder
    /// resumes searching for further images exactly as [`new`](Self::new)
    /// does.
    ///
    /// [`Mode::Auto`] cannot be detected without a header and decodes as
    /// [`Mode::Robot36`].
    pub fn without_header(mode: Mode, samples: I, sample_rate: u32) -> Self {
        let mut decoder = Self::new(mode, samples, sample_rate);
        decoder
            .events
            .push_back(Event::ImageStart(decoder.image_info()));
        decoder.state = State::Decoding;
        decoder
    }

    /// Metadata for the mode being decoded.
    fn image_info(&self) -> ImageInfo {
        ImageInfo {
            mode: self.mode,
            width: self.layout.width,
            height: self.layout.height,
        }
    }

    /// The duration expressed in (fractional) samples at our sample rate.
    fn samples_in(&self, duration: Duration) -> f64 {
        duration_to_samples(duration, self.sample_rate)
    }

    /// Scan for the next image's first line sync. On success, queue an
    /// [`Event::ImageStart`] and begin decoding; on stream exhaustion, finish.
    fn search(&mut self) {
        self.prefix.clear();
        self.prefix_position = 0;
        self.position = 0;

        let acquired = if self.requested_mode == Mode::Auto {
            self.detect_mode()
        } else {
            self.lock_onto_first_line()
                .map(|sequence_start| (self.requested_mode, sequence_start))
        };

        match acquired {
            Some((mode, sequence_start)) => {
                self.mode = mode;
                self.layout = mode.layout();
                // Drop everything before the first line so only image data is
                // replayed.
                let trim = (sequence_start.max(0.0) as usize).min(self.prefix.len());
                self.prefix.drain(0..trim);
                self.sequence_start = sequence_start - trim as f64;
                self.position = 0;
                self.prefix_position = 0;
                self.sequence_index = 0;
                self.row_index = 0;
                self.pending_pair = None;
                self.events.push_back(Event::ImageStart(self.image_info()));
                self.state = State::Decoding;
            }
            None => self.state = State::Done,
        }
    }

    /// Buffer frequencies from the demodulator until a calibration header is
    /// found, returning the detected mode and the sample position at which
    /// its first timing sequence begins.
    ///
    /// The header is located by its break: a short burst of the sync
    /// frequency splitting the two leader tones. From there the VIS bits lie
    /// at fixed offsets and are sampled near their centres; the mode is
    /// accepted once the start, stop and parity bits check out and the code
    /// is known.
    fn detect_mode(&mut self) -> Option<(Mode, f64)> {
        let sample_rate = self.sample_rate as f64;
        let samples = move |milliseconds: f64| milliseconds / 1000.0 * sample_rate;
        let min_leader = samples(150.0) as usize;
        let min_break = samples(4.0) as usize;
        let max_break = samples(25.0) as usize;
        let max_gap = samples(3.0) as usize;

        // The most recently completed run of leader-tone frequencies, as
        // (end, length); a break qualifies only right after a long one.
        let mut leader: Option<(usize, usize)> = None;
        let mut leader_start: Option<usize> = None;
        let mut break_start: Option<usize> = None;
        let mut index = 0usize;
        loop {
            let frequency = self.buffered_frequency(index)?;

            if self.is_leader(frequency) {
                leader_start.get_or_insert(index);
            } else if let Some(start) = leader_start.take() {
                leader = Some((index, index - start));
            }

            if self.is_sync(frequency) {
                break_start.get_or_insert(index);
            } else if let Some(start) = break_start.take() {
                let run = index - start;
                let after_leader = matches!(
                    leader,
                    Some((end, length))
                        if start.saturating_sub(end) <= max_gap && length >= min_leader
                );
                if after_leader
                    && (min_break..=max_break).contains(&run)
                    && let Some(found) = self.read_vis(index)
                {
                    return Some(found);
                }
            }

            index += 1;
        }
    }

    /// Read the VIS code whose break ended at `break_end`, returning the mode
    /// and the position where its image data begins. `None` if anything about
    /// the bits is off — the search then simply continues.
    fn read_vis(&mut self, break_end: usize) -> Option<(Mode, f64)> {
        let sample_rate = self.sample_rate as f64;
        let samples = move |milliseconds: f64| milliseconds / 1000.0 * sample_rate;

        // The second leader tone fills the 300ms between break and start bit.
        for probe in [100.0, 200.0] {
            let frequency = self.buffered_frequency(break_end + samples(probe) as usize)?;
            if !self.is_leader(frequency) {
                return None;
            }
        }

        // Ten 30ms bits follow the leader: start, seven code bits
        // (least-significant first), parity and stop. Each is judged by the
        // median of three samples around its centre.
        let mut bits = [0u32; 10];
        for (slot, bit) in bits.iter_mut().enumerate() {
            let mut medians = [0u32; 3];
            for (sample, offset) in medians.iter_mut().zip([8.0, 15.0, 22.0]) {
                let position = break_end + samples(300.0 + slot as f64 * 30.0 + offset) as usize;
                *sample = self.buffered_frequency(position)?.hz();
            }
            medians.sort_unstable();
            *bit = medians[1];
        }

        let is_sync_bit = |hz: u32| hz.abs_diff(SYNC_FREQUENCY.hz()) <= TONE_TOLERANCE_HZ;
        if !is_sync_bit(bits[0]) || !is_sync_bit(bits[9]) {
            return None;
        }
        let mut code = 0u8;
        let mut ones = 0u32;
        for (bit, hz) in bits[1..=8].iter().enumerate() {
            if !(950..=1450).contains(hz) {
                return None;
            }
            if *hz < 1200 {
                ones += 1;
                if bit < 7 {
                    code |= 1 << bit;
                }
            }
        }
        if !ones.is_multiple_of(2) {
            return None;
        }

        let mode = Mode::from_vis_code(code)?;
        let mut sequence_start = break_end as f64 + samples(600.0);
        if mode.has_starting_sync_pulse() {
            sequence_start += self.samples_in(mode.layout().sync_pulse().1);
        }
        Some((mode, sequence_start))
    }

    /// Buffer frequencies from the demodulator until a line sync is located,
    /// and return the sample position at which that line's timing sequence
    /// begins.
    ///
    /// A candidate sync is a run near the sync frequency lasting roughly one
    /// sync-pulse duration (long enough to exclude glitches, short enough to
    /// exclude the header's longer tones). An over-long run's tail is also a
    /// candidate: the VIS stop bit runs directly into the first line's sync
    /// pulse, merging both into one run. Three candidates evenly spaced one
    /// sequence period apart — the later two being clean, properly sized runs
    /// — are the line syncs of consecutive lines; noise does not produce that
    /// pattern. The image begins one sync offset before the first of them
    /// (zero for most modes — Scottie places the sync pulse mid-sequence).
    fn lock_onto_first_line(&mut self) -> Option<f64> {
        let (sync_offset, sync_duration) = self.layout.sync_pulse();
        let sync_len = self.samples_in(sync_duration);
        let period = self.samples_in(self.layout.sequence_duration());

        let min_run = (sync_len * 0.5) as usize;
        let max_run = (sync_len * 2.0) as usize;
        let spacing_tolerance = period * 0.15;
        let spaced =
            |from: usize, to: usize| ((to - from) as f64 - period).abs() <= spacing_tolerance;

        // Recent candidates: (position, whether the run was clean).
        let mut candidates: Vec<(usize, bool)> = Vec::new();
        let mut index = 0usize;
        loop {
            if !self.buffered_is_sync(index)? {
                index += 1;
                continue;
            }

            let start = index;
            while self.buffered_is_sync(index)? {
                index += 1;
            }
            let run = index - start;
            let (position, clean) = if (min_run..=max_run).contains(&run) {
                (start, true)
            } else if run > max_run {
                // Assume the tail of the over-long run is a merged sync pulse.
                (index - sync_len as usize, false)
            } else {
                continue;
            };

            // A clean candidate can complete a triple (a, b, this).
            if clean {
                for &(b, b_clean) in candidates.iter().rev() {
                    if !b_clean || !spaced(b, position) {
                        continue;
                    }
                    for &(a, _) in candidates.iter().rev() {
                        if a < b && spaced(a, b) {
                            return Some(a as f64 - self.samples_in(sync_offset));
                        }
                    }
                }
            }

            candidates.push((position, clean));
            // Only candidates within two periods (plus slack) can still form
            // a triple with future syncs.
            candidates.retain(|&(p, _)| (index - p) as f64 <= period * 2.5);
        }
    }

    /// The buffered frequency at `index`, extending the buffer from the
    /// demodulator as needed.
    fn buffered_frequency(&mut self, index: usize) -> Option<Frequency> {
        while self.prefix.len() <= index {
            let frequency = self.demodulator.next()?;
            self.prefix.push(frequency);
        }
        Some(self.prefix[index])
    }

    fn buffered_is_sync(&mut self, index: usize) -> Option<bool> {
        self.buffered_frequency(index)
            .map(|frequency| self.is_sync(frequency))
    }

    /// Decode the next timing sequence, queueing its rows. Ends the image with
    /// an [`Event::ImageEnd`] once every line is decoded, or if the signal
    /// runs out mid-image.
    fn decode_step(&mut self) {
        if self.row_index >= self.layout.height {
            self.events.push_back(Event::ImageEnd { complete: true });
            self.state = State::Searching;
            return;
        }

        match self.read_sequence() {
            Some(data) => self.assemble(data),
            None => {
                self.events.push_back(Event::ImageEnd { complete: false });
                self.state = State::Done;
            }
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
    fn read_sequence(&mut self) -> Option<SequenceData> {
        let sequence = self.layout.sequences[self.sequence_index];
        let width = self.layout.width;
        let expected_scans = sequence
            .iter()
            .filter(|step| matches!(step, Step::Scan(..)))
            .count();
        let mut t = self.sequence_start;
        let mut scans: Vec<(Channel, Vec<u8>)> = Vec::with_capacity(4);
        let mut tone_hz = Vec::with_capacity(4);
        let mut stream_ended = false;

        'steps: for step in sequence {
            match step {
                // Re-align on the actual sync pulse rather than trusting the
                // nominal timing.
                Step::Tone(tone) if tone.frequency == SYNC_FREQUENCY => {
                    if self.advance_to(t).is_none() || self.consume_sync().is_none() {
                        stream_ended = true;
                        break 'steps;
                    }
                    t = self.position as f64;
                }
                Step::Tone(tone) => {
                    let len = self.samples_in(tone.duration);
                    match self.advance_to(t + len / 2.0) {
                        Some(frequency) => tone_hz.push(frequency.hz()),
                        None => {
                            stream_ended = true;
                            break 'steps;
                        }
                    }
                    t += len;
                }
                Step::Scan(channel, duration) => {
                    let len = self.samples_in(*duration);
                    let pixel_len = len / width as f64;
                    let mut values = vec![0u8; width];
                    let mut last = 0u8;
                    for (x, value) in values.iter_mut().enumerate() {
                        match self.value_at(t + (x as f64 + 0.5) * pixel_len) {
                            Some(sampled) => {
                                *value = sampled;
                                last = sampled;
                            }
                            None => {
                                *value = last;
                                stream_ended = true;
                            }
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
        self.sequence_index = (self.sequence_index + 1) % self.layout.sequences.len();
        Some(SequenceData { scans, tone_hz })
    }

    /// Turn one sequence's sampled scans into rows according to the mode's
    /// colour system, and queue them.
    fn assemble(&mut self, data: SequenceData) {
        let scan = |channel: Channel| {
            data.scans
                .iter()
                .find(|(c, _)| *c == channel)
                .map(|(_, values)| values.as_slice())
                .expect("the mode's timing sequence contains this scan")
        };

        let mut rows: Vec<Vec<RgbPixel>> = Vec::new();
        match self.layout.color {
            ColorMode::Rgb => {
                let (red, green, blue) = (
                    scan(Channel::Red),
                    scan(Channel::Green),
                    scan(Channel::Blue),
                );
                rows.push(
                    (0..self.layout.width)
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
            ColorMode::YuvAveragedPair => {
                // One line carries only one colour difference; hold it until
                // its partner arrives, then reconstruct both lines from the
                // shared differences. Which line carries which is read from
                // the separator pulse (1500hz on even, 2300hz on odd) rather
                // than assumed from read order — so a decode that began on an
                // odd line does not swap red and blue.
                let separator = self
                    .layout
                    .parity_tone()
                    .expect("paired sequences differ in their separator pulse");
                let luma = scan(Channel::Y).to_vec();
                let (_, chroma) = data
                    .scans
                    .iter()
                    .find(|(c, _)| matches!(c, Channel::RY | Channel::BY))
                    .expect("each Robot 36 line carries one colour difference");
                let line = PendingLine {
                    luma,
                    chroma: chroma.clone(),
                    separator_hz: data.tone_hz.get(separator).copied().unwrap_or(0),
                };

                match self.pending_pair.take() {
                    None => self.pending_pair = Some(line),
                    Some(first) => {
                        let first_is_even = first.separator_hz <= line.separator_hz;
                        let (ry, by) = if first_is_even {
                            (&first.chroma, &line.chroma)
                        } else {
                            (&line.chroma, &first.chroma)
                        };
                        rows.push(yuv_row(&first.luma, ry, by));
                        rows.push(yuv_row(&line.luma, ry, by));
                    }
                }
            }
        }

        for pixels in rows {
            let index = self.row_index;
            self.row_index += 1;
            self.events.push_back(Event::Row(RgbRow { index, pixels }));
        }
    }

    /// Consume the sync pulse ahead: skip to its start, then through it.
    /// Returns `None` if the stream ends first.
    fn consume_sync(&mut self) -> Option<()> {
        while !self.is_sync(self.current) {
            self.pull()?;
        }
        while self.is_sync(self.current) {
            self.pull()?;
        }
        Some(())
    }

    /// The pixel value sampled at a fractional sample position.
    fn value_at(&mut self, position: f64) -> Option<u8> {
        let frequency = self.advance_to(position)?;
        let black = BLACK_FREQUENCY.hz() as i64;
        let white = WHITE_FREQUENCY.hz() as i64;
        let value = (frequency.hz() as i64 - black) * 255 / (white - black);
        Some(value.clamp(0, 255) as u8)
    }

    /// Pull samples until reaching the given (rounded) position and return the
    /// frequency there.
    fn advance_to(&mut self, position: f64) -> Option<Frequency> {
        let target = libm::round(position.max(0.0)) as usize;
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

    fn is_leader(&self, frequency: Frequency) -> bool {
        frequency.hz().abs_diff(LEADER_FREQUENCY.hz()) <= TONE_TOLERANCE_HZ
    }

    fn is_sync(&self, frequency: Frequency) -> bool {
        frequency.hz().abs_diff(SYNC_FREQUENCY.hz()) <= TONE_TOLERANCE_HZ
    }
}

/// Combine luminance and the two colour differences into one row of pixels.
fn yuv_row(luma: &[u8], chroma_red: &[u8], chroma_blue: &[u8]) -> Vec<RgbPixel> {
    luma.iter()
        .zip(chroma_red.iter().zip(chroma_blue.iter()))
        .map(|(&y, (&ry, &by))| RgbPixel::from(YuvPixel::new(y, ry, by)))
        .collect()
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

    const WIDTH: usize = 320;
    const HEIGHT: usize = 240;

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
            .header_tones()
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

        let decoded = collect_images(RowDecoder::new(Mode::Robot36, samples.into_iter(), 48_000));

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

        let decoded = collect_images(RowDecoder::new(Mode::Robot36, samples.into_iter(), 48_000));

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
        // Drop the header so the samples begin at the first line's sync pulse.
        let full = encode(&image, 48_000);
        let header = header_sample_count(48_000);
        let image_samples: Vec<i16> = full.into_iter().skip(header).collect();

        let decoded = collect_images(RowDecoder::without_header(
            Mode::Robot36,
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
        let decoded = collect_images(RowDecoder::new(
            Mode::Robot36,
            std::vec![0i16; 48_000].into_iter(),
            48_000,
        ));
        assert!(decoded.is_empty(), "silence should not produce an image");
    }
}
