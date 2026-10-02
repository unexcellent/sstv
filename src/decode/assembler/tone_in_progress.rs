//! Splits the frequency track into tones where the frequency changes.

use crate::Hz;
use crate::ms;
use crate::synthesizer::Tone;
use crate::units::{Duration, Frequency};

/// How far an estimate may stray from the current tone's frequency and still
/// belong to it, and from the candidate for the next tone and still continue
/// it. Half the 100 Hz step between the VIS start bit and a data bit.
const SPLIT_THRESHOLD: Frequency = Hz!(50);
/// How long the frequency must stay away from the current tone, and
/// consistently at one frequency, before a new tone begins. Shorter or
/// erratic off-tone stretches are noise and are absorbed into the current
/// tone; the consistency also keeps the demodulator's smeared estimates at a
/// transition out of the new tone's frequency.
const MIN_NEW_TONE: Duration = ms!(3);

/// The estimates collected for the tone currently being received.
pub(super) struct ToneInProgress {
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
    pub fn new(sample_rate: u32) -> Self {
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
    /// an off-tone stretch long enough to start a new one. Tones follow each
    /// other without gaps: each one begins where the previous one ended.
    pub fn push(&mut self, frequency: Frequency) -> Option<Tone> {
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

fn samples_in(duration: Duration, sample_rate: u32) -> usize {
    usize::try_from(duration.ns() * u64::from(sample_rate) / 1_000_000_000).unwrap_or(usize::MAX)
}

fn duration_of(samples: usize, sample_rate: u32) -> Duration {
    let samples = u64::try_from(samples).unwrap_or(u64::MAX);
    Duration::from_ns(samples.saturating_mul(1_000_000_000) / u64::from(sample_rate))
}
