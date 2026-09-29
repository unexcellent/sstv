//! Assembles the demodulated frequency track into tones and identifies the
//! calibration header among them.

use alloc::vec::Vec;

use crate::Frequency;
use crate::modes::{LEADER_FREQUENCY, Mode, VisCode};
use crate::synthesizer::Tone;
use crate::units::Duration;
use crate::{ms, tone};

/// How far an estimate may stray from the current tone's frequency and still
/// belong to it. Half the 100 Hz step between the VIS start bit and a data
/// bit.
const SPLIT_THRESHOLD_HZ: u32 = 50;
/// How long the frequency must stay away from the current tone before a new
/// tone begins. Shorter excursions are noise and are absorbed into the
/// current tone; this also rides out the demodulator smearing a transition
/// over about one period.
const MIN_EXCURSION: Duration = ms!(3);
/// The leader directly before the VIS code.
const LEADER: Tone = Tone::new(LEADER_FREQUENCY, ms!(300));
/// Generous in duration, so a leader clipped by a trimmed recording still
/// counts.
const LEADER_TOLERANCE: Tone = tone!(50 Hz, 150 ms);

/// The calibration header spans at most this many tones: leader, break,
/// leader, start bit, eight data bits and stop bit.
const HEADER_TONES: usize = 13;

/// Turns demodulated frequencies into tones and, while no mode is known,
/// identifies the mode from the calibration header.
pub(super) struct Assembler {
    /// The most recently completed tones, oldest first.
    tones: [Tone; HEADER_TONES],
    /// The mode being decoded; `None` while searching for a header.
    mode: Option<Mode>,
    sample_rate: u32,
    /// The tone the incoming frequencies currently belong to.
    current: ToneInProgress,
}

impl Assembler {
    pub fn new(sample_rate: u32, mode: Option<Mode>) -> Self {
        let sample_rate = sample_rate.max(1);
        let min_excursion = samples_in(MIN_EXCURSION, sample_rate).max(1);

        Self {
            tones: [tone!(0 Hz, 0 ns); HEADER_TONES],
            mode,
            sample_rate,
            current: ToneInProgress::new(min_excursion),
        }
    }

    /// Feed the next demodulated frequency. Returns the mode once a header
    /// identifying it has been completed.
    pub fn push(&mut self, frequency: Frequency) -> Option<Mode> {
        let completed_samples = self.current.push(frequency)?;
        let completed = Tone::new(
            completed_samples.frequency,
            duration_of(completed_samples.length, self.sample_rate),
        );
        self.tones.copy_within(1.., 0);
        self.tones[HEADER_TONES - 1] = completed;

        if self.mode.is_some() {
            return None;
        }
        self.mode = identify_header(&self.tones);
        self.mode
    }
}

/// A completed tone, measured in samples.
struct MeasuredTone {
    frequency: Frequency,
    length: usize,
}

/// The estimates collected for the tone currently being received.
struct ToneInProgress {
    /// Sum and count of the estimates that set the tone's frequency.
    frequency_sum: u64,
    frequency_count: u64,
    /// Every estimate attributed to the tone, including absorbed excursions.
    length: usize,
    /// The estimates of an ongoing excursion away from the tone's frequency.
    excursion: Vec<Frequency>,
    /// Excursion length at which the excursion becomes the next tone.
    min_excursion: usize,
}

impl ToneInProgress {
    fn new(min_excursion: usize) -> Self {
        Self {
            frequency_sum: 0,
            frequency_count: 0,
            length: 0,
            excursion: Vec::with_capacity(min_excursion),
            min_excursion,
        }
    }

    fn frequency(&self) -> Option<Frequency> {
        let mean = self.frequency_sum.checked_div(self.frequency_count)?;
        Some(Frequency::from_hz(u32::try_from(mean).unwrap_or(u32::MAX)))
    }

    /// Add an estimate. Returns the previous tone if this estimate completes
    /// an excursion long enough to start a new one.
    fn push(&mut self, frequency: Frequency) -> Option<MeasuredTone> {
        let Some(tone_frequency) = self.frequency() else {
            self.accept(frequency);
            return None;
        };

        if frequency.hz().abs_diff(tone_frequency.hz()) <= SPLIT_THRESHOLD_HZ {
            self.length += self.excursion.len();
            self.excursion.clear();
            self.accept(frequency);
            return None;
        }

        self.excursion.push(frequency);
        if self.excursion.len() < self.min_excursion {
            return None;
        }

        let completed = MeasuredTone {
            frequency: tone_frequency,
            length: self.length,
        };
        self.start_from_excursion();
        Some(completed)
    }

    fn accept(&mut self, frequency: Frequency) {
        self.frequency_sum += u64::from(frequency.hz());
        self.frequency_count += 1;
        self.length += 1;
    }

    /// Begin the next tone with the excursion's estimates. Its frequency is
    /// taken from the estimates near the excursion's median, which excludes
    /// the demodulator's smeared estimates at the transition.
    fn start_from_excursion(&mut self) {
        let mut excursion = core::mem::take(&mut self.excursion);
        excursion.sort_unstable();
        let median = excursion[excursion.len() / 2];

        self.frequency_sum = 0;
        self.frequency_count = 0;
        self.length = 0;
        for &frequency in &excursion {
            if frequency.hz().abs_diff(median.hz()) <= SPLIT_THRESHOLD_HZ {
                self.accept(frequency);
            } else {
                self.length += 1;
            }
        }

        excursion.clear();
        self.excursion = excursion;
    }
}

/// The mode announced by a header ending at the newest tone, if the tones
/// form one: a leader followed by the VIS code. The break and first leader
/// are not required, so a recording trimmed at the front is still
/// identified.
fn identify_header(tones: &[Tone; HEADER_TONES]) -> Option<Mode> {
    let (code, start_bit) = VisCode::from_received_tones(tones)?;
    let leader = tones.get(..start_bit)?.last()?;
    if !leader.is_near(LEADER, LEADER_TOLERANCE) {
        return None;
    }
    Mode::try_from(code).ok()
}

fn samples_in(duration: Duration, sample_rate: u32) -> usize {
    usize::try_from(duration.ns() * u64::from(sample_rate) / 1_000_000_000).unwrap_or(usize::MAX)
}

fn duration_of(samples: usize, sample_rate: u32) -> Duration {
    let samples = u64::try_from(samples).unwrap_or(u64::MAX);
    Duration::from_ns(samples.saturating_mul(1_000_000_000) / u64::from(sample_rate))
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::io::Read;
    use std::string::String;
    use std::vec::Vec;

    use super::super::demodulator::Demodulator;
    use super::*;
    use crate::modes::{PD_120, PD_180, ROBOT_36};
    use crate::{Encoder, RgbPixel, Synthesizer};

    #[test]
    fn identifies_robot_36_in_the_ground_station_recording() {
        let (samples, sample_rate) = read_gzipped_wav("tests/assets/real_recording.wav.gz");

        assert_eq!(identify_mode(samples, sample_rate), Some(ROBOT_36));
    }

    #[test]
    fn identifies_robot_36_in_the_pysstv_fixture() {
        let (samples, sample_rate) = read_wav("tests/assets/patch-robot36-pysstv.wav");

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
        let vox_leader_and_break = samples_in(ms!(800 + 300 + 10), 48_000);
        let trimmed = transmission[vox_leader_and_break..].to_vec();

        assert_eq!(identify_mode(trimmed, 48_000), Some(PD_120));
    }

    #[test]
    fn silence_identifies_no_mode() {
        let silence = std::vec![0i16; 48_000];

        assert_eq!(identify_mode(silence, 48_000), None);
    }

    fn identify_mode(samples: Vec<i16>, sample_rate: u32) -> Option<Mode> {
        let mut assembler = Assembler::new(sample_rate, None);
        Demodulator::new(samples.into_iter(), sample_rate)
            .find_map(|frequency| assembler.push(frequency))
    }

    fn synthesize(mode: Mode, sample_rate: u32) -> Vec<i16> {
        let (width, height) = mode.resolution();
        let image = core::iter::repeat_n(RgbPixel::new(128, 128, 128), (width * height) as usize);
        Synthesizer::new(Encoder::new(mode, image).unwrap(), sample_rate).collect()
    }

    fn read_iss_recording(name: &str) -> (Vec<i16>, u32) {
        read_wav(&std::format!("tests/assets/iss/{name}"))
    }

    fn read_gzipped_wav(path: &str) -> (Vec<i16>, u32) {
        let mut wav = Vec::new();
        flate2::read::GzDecoder::new(open_asset(path))
            .read_to_end(&mut wav)
            .unwrap();
        wav_samples(&wav)
    }

    fn read_wav(path: &str) -> (Vec<i16>, u32) {
        let mut wav = Vec::new();
        open_asset(path).read_to_end(&mut wav).unwrap();
        wav_samples(&wav)
    }

    fn open_asset(path: &str) -> std::fs::File {
        let full_path: String = std::format!("{}/{path}", env!("CARGO_MANIFEST_DIR"));
        std::fs::File::open(&full_path).unwrap_or_else(|_| {
            panic!("{path} not found; the tests in tests/ fetch or generate it")
        })
    }

    /// The first channel of a WAV, scaled to 16 bit, and its sample rate.
    fn wav_samples(wav: &[u8]) -> (Vec<i16>, u32) {
        let reader = hound::WavReader::new(std::io::Cursor::new(wav)).unwrap();
        let spec = reader.spec();
        let channels = usize::from(spec.channels);
        let samples = match spec.sample_format {
            hound::SampleFormat::Int => reader
                .into_samples::<i32>()
                .step_by(channels)
                .map(|sample| (sample.unwrap() >> (spec.bits_per_sample - 16)) as i16)
                .collect(),
            hound::SampleFormat::Float => reader
                .into_samples::<f32>()
                .step_by(channels)
                .map(|sample| (sample.unwrap() * f32::from(i16::MAX)) as i16)
                .collect(),
        };
        (samples, spec.sample_rate)
    }
}
