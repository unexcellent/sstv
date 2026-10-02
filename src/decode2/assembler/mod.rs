//! Assembles the demodulated frequency track into the tones of the images it
//! carries. Between images, tones are split internally where the frequency
//! changes, to find each image's calibration header. Within an image, tones
//! follow the mode's timing, anchored on the line sync pulses: one per
//! control step and one per pixel, as the encoder emits them.

mod estimates;
mod sampling;
mod tone_in_progress;

use alloc::collections::VecDeque;

use crate::modes::{LEADER_FREQUENCY, Mode, SYNC_FREQUENCY, VisCode};
use crate::synthesizer::Tone;
use crate::units::{Duration, Frequency};
use crate::{ms, tone};
use estimates::Estimates;
use sampling::ImageTiming;
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

/// Turns demodulated frequencies into the tones of the images they carry,
/// back to back. Only images in the mode the first header announced are
/// assembled; headers announcing another mode are ignored.
pub(super) struct Assembler<I: Iterator<Item = Frequency>> {
    frequencies: I,
    sample_rate: u32,
    /// The most recently completed tones while searching, oldest first.
    tones: [Tone; HEADER_TONES],
    /// The mode the first header announced; `None` until then.
    mode: Option<Mode>,
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
    exhausted: bool,
}

impl<I: Iterator<Item = Frequency>> Assembler<I> {
    pub fn new(frequencies: I, sample_rate: u32) -> Self {
        let sample_rate = sample_rate.max(1);
        Self {
            frequencies,
            sample_rate,
            tones: [tone!(0 Hz, 0 ns); HEADER_TONES],
            mode: None,
            current: ToneInProgress::new(sample_rate),
            current_start: 0,
            estimates: Estimates::default(),
            image: None,
            ready: VecDeque::new(),
            exhausted: false,
        }
    }

    /// The mode the first header announced, and so the mode of every image
    /// whose tones are handed out. Known by the time the first tone is.
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
            let lookback = self.whole_samples_in(HEADER_LOOKBACK);
            let end = self.estimates.end();
            self.estimates.forget_before(end.saturating_sub(lookback));
        }
    }

    /// Take in a tone found while searching, and start the image if it
    /// completes a header in the mode already known, or the first header.
    fn search(&mut self, tone: Tone, end: u64) {
        self.tones.copy_within(1.., 0);
        self.tones[HEADER_TONES - 1] = tone;

        if let Some(mode) = identify_header(&self.tones)
            && self.mode.is_none_or(|known| known == mode)
        {
            self.mode = Some(mode);
            self.start_image(mode, end);
        }
    }

    /// Search afresh for a header from the newest estimate on.
    fn restart_search(&mut self) {
        self.tones = [tone!(0 Hz, 0 ns); HEADER_TONES];
        self.current = ToneInProgress::new(self.sample_rate);
        self.current_start = self.estimates.end();
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
            if self.exhausted {
                return None;
            }
            if let Some(frequency) = self.frequencies.next() {
                self.push(frequency);
            } else {
                self.exhausted = true;
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

    use super::super::demodulator::Demodulator;
    use super::super::testing::{
        PYSSTV_FIXTURE, gradient_image, read_generated_wav, read_gzipped_wav, read_iss_recording,
        transmit,
    };
    use super::*;
    use crate::modes::{MARTIN_1, PD_120, PD_180, ROBOT_36, SCOTTIE_1};
    use crate::{Encoder, RgbPixel, Synthesizer};

    #[test]
    fn identifies_robot_36_in_the_ground_station_recording() {
        let (samples, sample_rate) = read_gzipped_wav("tests/assets/real_recording.wav.gz");

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
    fn headers_announcing_another_mode_are_ignored() {
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
