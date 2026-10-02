//! Assembles the demodulated frequency track into tones and identifies the
//! calibration header among them.

use crate::modes::{LEADER_FREQUENCY, Mode, SYNC_FREQUENCY, VisCode};
use crate::synthesizer::Tone;
use crate::units::{Duration, Frequency};
use crate::{Hz, ms, tone};

/// How far an estimate may stray from the current tone's frequency and still
/// belong to it, and from the candidate for the next tone and still continue
/// it.
/// Half the 100 Hz step between the VIS start bit and a data bit.
const SPLIT_THRESHOLD: Frequency = Hz!(50);
/// How long the frequency must stay away from the current tone, and
/// consistently at one frequency, before a new tone begins. Shorter or
/// erratic off-tone stretches are noise and are absorbed into the current
/// tone; the
/// consistency also keeps the demodulator's smeared estimates at a
/// transition out of the new tone's frequency.
const MIN_NEW_TONE: Duration = ms!(3);
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

/// Turns demodulated frequencies into tones and, while no mode is known,
/// identifies the mode from the calibration header.
pub(super) struct Assembler {
    /// The most recently completed tones, oldest first.
    tones: [Tone; HEADER_TONES],
    /// The mode being decoded; `None` while searching for a header.
    mode: Option<Mode>,
    /// The tone the incoming frequencies currently belong to.
    current: ToneInProgress,
}

impl Assembler {
    pub fn new(sample_rate: u32, mode: Option<Mode>) -> Self {
        Self {
            tones: [tone!(0 Hz, 0 ns); HEADER_TONES],
            mode,
            current: ToneInProgress::new(sample_rate),
        }
    }

    /// Feed the next demodulated frequency.
    pub fn push(&mut self, frequency: Frequency) {
        let Some(completed) = self.current.push(frequency) else {
            return;
        };
        self.tones.copy_within(1.., 0);
        self.tones[HEADER_TONES - 1] = completed;

        if self.mode.is_none() {
            self.mode = identify_header(&self.tones);
        }
    }

    /// The mode being decoded: the one passed to [`new`](Self::new), or the
    /// one identified from the header. `None` while still searching.
    pub const fn detected_mode(&self) -> Option<Mode> {
        self.mode
    }
}

/// The estimates collected for the tone currently being received.
struct ToneInProgress {
    /// The estimates that set the tone's frequency.
    tone: RunningMean,
    /// Every estimate attributed to the tone, including absorbed off-tone
    /// stretches.
    length: usize,
    /// The number of estimates since the frequency last matched the tone.
    off_tone_length: usize,
    /// The most recent off-tone estimates that agree with each other: the
    /// candidate for the next tone.
    candidate: RunningMean,
    /// Candidate length at which the candidate becomes the next tone.
    min_new_tone: u64,
    sample_rate: u32,
}

impl ToneInProgress {
    fn new(sample_rate: u32) -> Self {
        let sample_rate = sample_rate.max(1);
        let min_new_tone = samples_in(MIN_NEW_TONE, sample_rate).max(1);

        Self {
            tone: RunningMean::default(),
            length: 0,
            off_tone_length: 0,
            candidate: RunningMean::default(),
            min_new_tone: u64::try_from(min_new_tone).unwrap_or(u64::MAX),
            sample_rate,
        }
    }

    /// Add an estimate. Returns the previous tone if this estimate completes
    /// an off-tone stretch long enough to start a new one.
    fn push(&mut self, frequency: Frequency) -> Option<Tone> {
        let tone_frequency = self.tone.frequency().unwrap_or(frequency);

        let no_tone_switch_has_been_detected =
            frequency.abs_diff(tone_frequency) <= SPLIT_THRESHOLD;
        if no_tone_switch_has_been_detected {
            self.length += self.off_tone_length + 1;
            self.off_tone_length = 0;
            self.candidate = RunningMean::default();
            self.tone.add(frequency);
            return None;
        }

        self.off_tone_length += 1;

        let candidate_frequency_is_distinct_enough =
            frequency.abs_diff(self.candidate.frequency().unwrap_or(frequency)) > SPLIT_THRESHOLD;
        if candidate_frequency_is_distinct_enough {
            self.candidate = RunningMean::default();
        }

        self.candidate.add(frequency);
        if self.candidate.count < self.min_new_tone {
            return None;
        }

        let completed = Tone::new(tone_frequency, duration_of(self.length, self.sample_rate));
        self.tone = core::mem::take(&mut self.candidate);
        self.length = self.off_tone_length;
        self.off_tone_length = 0;
        Some(completed)
    }
}

/// The mean of a run of frequency estimates.
#[derive(Default)]
struct RunningMean {
    sum: u64,
    count: u64,
}

impl RunningMean {
    fn frequency(&self) -> Option<Frequency> {
        let mean = self.sum.checked_div(self.count)?;
        Some(Frequency::from_hz(u32::try_from(mean).unwrap_or(u32::MAX)))
    }

    fn add(&mut self, frequency: Frequency) {
        self.sum += u64::from(frequency.hz());
        self.count += 1;
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
    use std::vec::Vec;

    use super::super::demodulator::Demodulator;
    use super::*;
    use crate::modes::{PD_120, PD_180, ROBOT_36};
    use crate::{Encoder, RgbPixel, Synthesizer};

    /// A Robot 36 transmission generated by `PySSTV`
    /// (`tests/scripts/encode_with_pysstv.py`); not committed.
    const PYSSTV_FIXTURE: &str = "tests/assets/patch-robot36-pysstv.wav";

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
        for frequency in Demodulator::new(samples.into_iter(), sample_rate) {
            assembler.push(frequency);
            if assembler.detected_mode().is_some() {
                break;
            }
        }
        assembler.detected_mode()
    }

    fn synthesize(mode: Mode, sample_rate: u32) -> Vec<i16> {
        let (width, height) = mode.resolution();
        let image = core::iter::repeat_n(RgbPixel::new(128, 128, 128), (width * height) as usize);
        Synthesizer::new(Encoder::new(mode, image).unwrap(), sample_rate).collect()
    }

    /// Read one of the ISS recordings, fetching the set on first use as
    /// `tests/iss_recordings.rs` does.
    fn read_iss_recording(name: &str) -> (Vec<i16>, u32) {
        static FETCH: std::sync::Once = std::sync::Once::new();
        let path = std::format!("tests/assets/iss/{name}");
        FETCH.call_once(|| {
            if asset_path(&path).exists() {
                return;
            }
            let status = std::process::Command::new("python3")
                .arg(asset_path("tests/scripts/fetch_iss_recordings.py"))
                .current_dir(env!("CARGO_MANIFEST_DIR"))
                .status()
                .expect("run tests/scripts/fetch_iss_recordings.py");
            assert!(status.success(), "fetching the ISS recordings failed");
        });
        read_wav(&path)
    }

    /// Read a fixture generated by one of the `tests/scripts/` tools, or
    /// `None` on machines (like CI) that have not generated it, as
    /// `tests/interop.rs` does.
    fn read_generated_wav(path: &str) -> Option<(Vec<i16>, u32)> {
        if asset_path(path).exists() {
            return Some(read_wav(path));
        }
        std::eprintln!("skipping: {path} not found, generate it with tests/scripts/");
        None
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
        std::fs::File::open(asset_path(path)).unwrap_or_else(|_| panic!("{path} not found"))
    }

    fn asset_path(path: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
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
