//! Assembles the demodulated frequency track into the tones of the first
//! image it carries. Before the image, tones are split internally where the
//! frequency changes, to find its calibration header. Within the image, tones
//! follow the mode's timing, anchored on the line sync pulses: one per
//! control step and one per pixel, as the encoder emits them.

mod estimates;
mod sampling;
mod sync_lock;
mod tone_in_progress;

use alloc::collections::VecDeque;

use crate::modes::{LEADER_FREQUENCY, Mode, SYNC_FREQUENCY, VisCode};
use crate::synthesizer::Tone;
use crate::units::{Duration, Frequency};
use crate::{ms, tone};
use estimates::Estimates;
use sampling::ImageTiming;
use sync_lock::SyncLock;
use tone_in_progress::ToneInProgress;

/// The leader directly before the VIS code.
const LEADER: Tone = Tone::new(LEADER_FREQUENCY, ms!(300));
/// Generous in duration, so a leader clipped by a trimmed recording still
/// counts.
const LEADER_TOLERANCE: Tone = tone!(50 Hz, 150 ms);
/// The start bit opening the VIS code.
const START_BIT: Tone = Tone::new(SYNC_FREQUENCY, ms!(30));
const START_BIT_TOLERANCE: Tone = tone!(50 Hz, 10 ms);
/// The stop bit closing the VIS code merges with the first line's sync
/// pulse, which has the same frequency and lasts up to 20 ms.
const STOP_BIT_AND_SYNC: Tone = Tone::new(SYNC_FREQUENCY, ms!(40));
/// Covers the stop bit alone (30 ms) up to the stop bit plus the longest
/// sync pulse (50 ms), with 10 ms of slack either side.
const STOP_BIT_AND_SYNC_TOLERANCE: Tone = tone!(50 Hz, 20 ms);

/// The calibration header spans at most this many tones: leader, break,
/// leader, start bit, eight data bits and stop bit.
const HEADER_TONES: usize = 13;
/// How long the estimates are kept while searching for a header. The header
/// is identified a few milliseconds after its stop bit and the first sync
/// pulse ended, and the first sequence begins at most that sync pulse
/// earlier.
const HEADER_LOOKBACK: Duration = ms!(200);
/// How many sequences of estimates are kept while locking on to sync pulses:
/// the three pulses span two lines, and the first sequence may begin up to
/// one sequence before the first of them.
const SYNC_LOCK_LOOKBACK_SEQUENCES: u32 = 3;

/// Where the assembler finds the start of the image.
#[derive(Clone, Copy)]
pub enum Start {
    /// At the calibration header, which identifies the mode.
    Header,
    /// At a header announcing this mode, or else at three of the mode's line
    /// sync pulses spaced one line apart.
    HeaderOrSyncs(Mode),
    /// Right at the first sample, in this mode.
    FirstSample(Mode),
}

/// Turns demodulated frequencies into the tones of the first image they
/// carry. Anything after that image is ignored.
pub(super) struct Assembler<I: Iterator<Item = Frequency>> {
    frequencies: I,
    sample_rate: u32,
    /// The most recently completed tones while searching, oldest first.
    tones: [Tone; HEADER_TONES],
    /// The mode of the image; `None` until its start has been found.
    mode: Option<Mode>,
    /// The mode a header must announce to be accepted; `None` accepts any.
    expected_mode: Option<Mode>,
    /// Locks on to the line sync pulses when the mode is known in advance.
    sync_lock: Option<SyncLock>,
    /// How many samples of estimates are kept while searching.
    lookback: u64,
    /// The tone the incoming frequencies currently belong to, split where
    /// the frequency changes.
    current: ToneInProgress,
    /// The sample at which `current` began.
    current_start: u64,
    estimates: Estimates,
    /// The timing of the image being assembled; `None` while searching.
    image: Option<ImageTiming>,
    /// Image tones ready to be handed out, oldest first.
    ready: VecDeque<Tone>,
    /// No more tones will be assembled: the image is complete or the
    /// frequencies ran out.
    done: bool,
}

impl<I: Iterator<Item = Frequency>> Assembler<I> {
    pub fn new(frequencies: I, sample_rate: u32, start: Start) -> Self {
        let sample_rate = sample_rate.max(1);
        let mut assembler = Self {
            frequencies,
            sample_rate,
            tones: [tone!(0 Hz, 0 ns); HEADER_TONES],
            mode: None,
            expected_mode: None,
            sync_lock: None,
            lookback: 0,
            current: ToneInProgress::new(sample_rate),
            current_start: 0,
            estimates: Estimates::default(),
            image: None,
            ready: VecDeque::new(),
            done: false,
        };
        assembler.lookback = assembler.whole_samples_in(HEADER_LOOKBACK);
        match start {
            Start::Header => {}
            Start::HeaderOrSyncs(mode) => {
                assembler.expected_mode = Some(mode);
                assembler.sync_lock = Some(SyncLock::new(mode, sample_rate));
                assembler.lookback += assembler
                    .whole_samples_in(mode.sequence_duration() * SYNC_LOCK_LOOKBACK_SEQUENCES);
            }
            Start::FirstSample(mode) => assembler.start_image(mode, 0.0),
        }
        assembler
    }

    /// The mode of the image. Known by the time its first tone is handed out.
    pub const fn detected_mode(&self) -> Option<Mode> {
        self.mode
    }

    fn push(&mut self, frequency: Frequency) {
        self.estimates.push(frequency);
        if let Some(tone) = self.current.push(frequency) {
            let end = self.current_start + self.whole_samples_in(tone.duration);
            self.current_start = end;
            if self.image.is_some() {
                self.anchor_on_sync(tone, end);
            } else {
                self.search(tone, end);
            }
        }

        if self.image.is_some() {
            self.assemble_ready_sequence();
        } else {
            let end = self.estimates.end();
            self.estimates
                .forget_before(end.saturating_sub(self.lookback));
        }
    }

    /// Take in a tone found while searching, and start the image if it
    /// completes a header or the sync lock.
    fn search(&mut self, tone: Tone, end: u64) {
        self.tones.copy_within(1.., 0);
        self.tones[HEADER_TONES - 1] = tone;

        if let Some(mode) = identify_header(&self.tones)
            && self.expected_mode.is_none_or(|expected| expected == mode)
        {
            self.start_image_after_header(mode, end);
            return;
        }

        let locked = self.sync_lock.as_mut().and_then(|lock| {
            let first_sync_end = lock.push(tone, end)?;
            Some((lock.mode(), first_sync_end))
        });
        if let Some((mode, first_sync_end)) = locked {
            let sequence_start = self.first_sequence_start(mode, first_sync_end);
            self.start_image(mode, sequence_start);
        }
    }

    /// The number of samples a tone of this duration spans. Tone durations
    /// are whole sample counts truncated to nanoseconds, so rounding recovers
    /// the count exactly.
    fn whole_samples_in(&self, duration: Duration) -> u64 {
        (duration.ns() * u64::from(self.sample_rate) + 500_000_000) / 1_000_000_000
    }
}

impl<I: Iterator<Item = Frequency>> Iterator for Assembler<I> {
    type Item = Tone;

    fn next(&mut self) -> Option<Tone> {
        loop {
            if let Some(tone) = self.ready.pop_front() {
                return Some(tone);
            }
            if self.done {
                return None;
            }
            if let Some(frequency) = self.frequencies.next() {
                self.push(frequency);
            } else {
                self.done = true;
                self.assemble_remaining_sequences();
            }
        }
    }
}

/// The mode announced by a header ending at the newest tone.
fn identify_header(tones: &[Tone; HEADER_TONES]) -> Option<Mode> {
    let data_bits = find_vis_data_bits(tones)?;
    let code = VisCode::from_received_tones(data_bits)?;
    Mode::try_from(code).ok()
}

/// The VIS data-bit tones of a header ending at the newest tone: the tones
/// between a start bit that follows a leader and the stop bit. The break and
/// first leader are not required, so a recording trimmed at the front is
/// still identified.
fn find_vis_data_bits(tones: &[Tone]) -> Option<&[Tone]> {
    let (stop_bit, earlier) = tones.split_last()?;
    if !stop_bit.is_near(STOP_BIT_AND_SYNC, STOP_BIT_AND_SYNC_TOLERANCE) {
        return None;
    }

    let start_bit = earlier
        .iter()
        .rposition(|tone| tone.is_near(START_BIT, START_BIT_TOLERANCE))?;
    let leader = earlier.get(..start_bit)?.last()?;
    if !leader.is_near(LEADER, LEADER_TOLERANCE) {
        return None;
    }

    earlier
        .get(start_bit + 1..)
        .filter(|data_bits| !data_bits.is_empty())
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec::Vec;

    use super::super::testing::{
        GROUND_STATION_RECORDING, PYSSTV_FIXTURE, gradient_image, read_generated_wav,
        read_gzipped_wav, read_iss_recording, transmit,
    };
    use super::*;
    use crate::Demodulator;
    use crate::modes::{MARTIN_1, PD_120, PD_180, ROBOT_36, SCOTTIE_1};
    use crate::{Encoder, RgbPixel, Synthesizer};

    #[test]
    fn identifies_robot_36_in_the_ground_station_recording() {
        let (samples, sample_rate) = read_gzipped_wav(GROUND_STATION_RECORDING);

        assert_eq!(identify_mode(samples, sample_rate), Some(ROBOT_36));
    }

    #[test]
    fn identifies_robot_36_in_the_pysstv_fixture() {
        let Some((samples, sample_rate)) = read_generated_wav(PYSSTV_FIXTURE) else {
            return;
        };

        assert_eq!(identify_mode(samples, sample_rate), Some(ROBOT_36));
    }

    #[test]
    fn identifies_pd_180_in_the_gagarin_80_recording() {
        let (samples, sample_rate) = read_iss_recording("pd180-gagarin-80.wav");

        assert_eq!(identify_mode(samples, sample_rate), Some(PD_180));
    }

    #[test]
    fn identifies_pd_180_in_the_apollo_soyuz_recording() {
        let (samples, sample_rate) = read_iss_recording("pd180-apollo-soyuz.wav");

        assert_eq!(identify_mode(samples, sample_rate), Some(PD_180));
    }

    #[test]
    fn identifies_pd_180_in_the_astronauts_qso_recording() {
        let (samples, sample_rate) = read_iss_recording("pd180-ariss-qso-astros.wav");

        assert_eq!(identify_mode(samples, sample_rate), Some(PD_180));
    }

    #[test]
    fn identifies_pd_180_in_the_cristoforetti_qso_recording() {
        let (samples, sample_rate) = read_iss_recording("pd180-ariss-qso-cristoforetti.wav");

        assert_eq!(identify_mode(samples, sample_rate), Some(PD_180));
    }

    #[test]
    fn identifies_pd_180_in_the_mai75_suitsat_recording() {
        let (samples, sample_rate) = read_iss_recording("pd180-mai75-suitsat.wav");

        assert_eq!(identify_mode(samples, sample_rate), Some(PD_180));
    }

    #[test]
    fn identifies_pd_120_in_the_first_ariss_20_year_recording() {
        let (samples, sample_rate) = read_iss_recording("pd120-ariss-20-year-1.wav");

        assert_eq!(identify_mode(samples, sample_rate), Some(PD_120));
    }

    #[test]
    fn identifies_pd_120_in_the_second_ariss_20_year_recording() {
        let (samples, sample_rate) = read_iss_recording("pd120-ariss-20-year-2.wav");

        assert_eq!(identify_mode(samples, sample_rate), Some(PD_120));
    }

    #[test]
    fn identifies_the_mode_of_our_own_transmission() {
        let transmission = synthesize(ROBOT_36, 48_000);

        assert_eq!(identify_mode(transmission, 48_000), Some(ROBOT_36));
    }

    #[test]
    fn identifies_the_mode_when_the_front_of_the_header_is_trimmed() {
        let transmission = synthesize(PD_120, 48_000);
        // The VOX tones, the first leader and the break: 1110 ms.
        let vox_leader_and_break = 48_000 * 1110 / 1000;
        let trimmed = transmission[vox_leader_and_break..].to_vec();

        assert_eq!(identify_mode(trimmed, 48_000), Some(PD_120));
    }

    #[test]
    fn silence_identifies_no_mode() {
        let silence = std::vec![0i16; 48_000];

        assert_eq!(identify_mode(silence, 48_000), None);
    }

    #[test]
    fn image_tones_mirror_the_encoders_tones() {
        let image = gradient_image(MARTIN_1);
        let samples = transmit(MARTIN_1, &image, 48_000);

        let assembled: Vec<Tone> = assemble(samples, 48_000).collect();
        let encoded = encoded_image_tones(MARTIN_1, &image);

        assert_eq!(assembled.len(), encoded.len());
        let close_in_frequency = assembled
            .iter()
            .zip(&encoded)
            .filter(|(assembled, encoded)| {
                assembled.frequency.abs_diff(encoded.frequency).hz() <= 50
            })
            .count();
        assert!(
            close_in_frequency * 100 >= encoded.len() * 95,
            "only {close_in_frequency} of {} tones close in frequency",
            encoded.len()
        );
    }

    #[test]
    fn image_tones_last_as_long_as_the_image() {
        let image = gradient_image(SCOTTIE_1);
        let samples = transmit(SCOTTIE_1, &image, 48_000);

        let assembled: Duration = assemble(samples, 48_000).map(|tone| tone.duration).sum();
        let encoded: Duration = encoded_image_tones(SCOTTIE_1, &image)
            .iter()
            .map(|tone| tone.duration)
            .sum();

        assert!(
            assembled.ns().abs_diff(encoded.ns()) < ms!(1).ns(),
            "{} ns assembled vs {} ns encoded",
            assembled.ns(),
            encoded.ns()
        );
    }

    #[test]
    fn hands_out_only_image_tones() {
        let image = gradient_image(ROBOT_36);
        let samples = transmit(ROBOT_36, &image, 48_000);

        let tone_count = assemble(samples, 48_000).count();

        assert_eq!(tone_count, encoded_image_tones(ROBOT_36, &image).len());
    }

    #[test]
    fn assembles_only_the_first_image() {
        let robot_36_image = gradient_image(ROBOT_36);
        let mut samples = transmit(ROBOT_36, &robot_36_image, 48_000);
        samples.extend(transmit(MARTIN_1, &gradient_image(MARTIN_1), 48_000));
        let mut assembler = assemble(samples, 48_000);

        let tone_count = assembler.by_ref().count();

        assert_eq!(assembler.detected_mode(), Some(ROBOT_36));
        assert_eq!(
            tone_count,
            encoded_image_tones(ROBOT_36, &robot_36_image).len()
        );
    }

    fn identify_mode(samples: Vec<i16>, sample_rate: u32) -> Option<Mode> {
        let mut assembler = assemble(samples, sample_rate);
        assembler.next();
        assembler.detected_mode()
    }

    fn assemble(
        samples: Vec<i16>,
        sample_rate: u32,
    ) -> Assembler<Demodulator<std::vec::IntoIter<i16>>> {
        Assembler::new(
            Demodulator::new(samples.into_iter(), sample_rate),
            sample_rate,
            Start::Header,
        )
    }

    /// The tones the encoder emits for the image, after the header.
    fn encoded_image_tones(mode: Mode, image: &[RgbPixel]) -> Vec<Tone> {
        Encoder::new(mode, image.iter().copied())
            .unwrap()
            .skip(mode.header_tones().count())
            .collect()
    }

    fn synthesize(mode: Mode, sample_rate: u32) -> Vec<i16> {
        let (width, height) = mode.resolution();
        let image = core::iter::repeat_n(RgbPixel::new(128, 128, 128), (width * height) as usize);
        Synthesizer::new(Encoder::new(mode, image).unwrap(), sample_rate).collect()
    }
}
