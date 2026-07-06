use crate::units::Frequency;

/// Estimates the instantaneous frequency of a stream of PCM samples.
///
/// `Demodulator` is the inverse of [`Synthesizer`](crate::Synthesizer): it turns
/// 16-bit samples back into a stream of [`Frequency`] estimates. It performs no
/// protocol interpretation — grouping the frequency track into pixels and
/// scanlines is the decoder's job.
///
/// This is a deliberately simple *zero-crossing* estimator. It measures the
/// number of samples between successive zero crossings of the waveform and holds
/// that estimate until the next crossing is seen. The crossing position is
/// linearly interpolated between the two straddling samples for sub-sample
/// accuracy. The approach is cheap and allocation-free, but only moderately
/// noise resistant and less accurate when there are few samples per cycle.
///
/// Two crossings are needed before a frequency can be determined, so the
/// iterator yields *nothing* during that initial warm-up rather than a
/// placeholder value. Once the first estimate is available it yields one
/// estimate per input sample. The dropped warm-up is bounded to roughly one
/// period and, for SSTV, always falls inside the leader tone.
///
/// ```rust
/// use sstv::{Demodulator, Synthesizer, Tone, Hz, ms};
///
/// let samples = Synthesizer::new([Tone::new(Hz!(1900), ms!(20))].into_iter(), 48000);
/// for frequency in Demodulator::new(samples, 48000) {
///     // inspect the recovered frequency track
///     let _ = frequency;
/// }
/// ```
pub struct Demodulator<I: Iterator<Item = i16>> {
    samples: I,
    sample_rate: u32,
    previous_sample: i16,
    index: u64,
    last_crossing: Option<f64>,
    frequency: Frequency,
    has_started: bool,
}

impl<I: Iterator<Item = i16>> Demodulator<I> {
    /// Create a new `Demodulator` from a sample iterator and a sample rate in Hz.
    ///
    /// `sample_rate` must be greater than zero.
    pub fn new(mut samples: I, sample_rate: u32) -> Self {
        let first_sample = samples.next();
        Self {
            samples,
            sample_rate: sample_rate.max(1),
            previous_sample: first_sample.unwrap_or_default(),
            index: 0,
            last_crossing: None,
            frequency: Frequency::from_hz(0),
            has_started: false,
        }
    }
}

impl<I: Iterator<Item = i16>> Iterator for Demodulator<I> {
    type Item = Frequency;

    fn next(&mut self) -> Option<Frequency> {
        // Consume samples until we can emit: during the warm-up (before the
        // first estimate) this loops without producing anything; afterwards it
        // returns exactly once per sample.
        loop {
            let current_sample = self.samples.next()?;
            let index = self.index;
            self.index += 1;

            // A sign change between the two samples marks a zero crossing.
            if (self.previous_sample >= 0) != (current_sample >= 0) {
                let prev_f = self.previous_sample as f64;
                let curr_f = current_sample as f64;
                // Fraction of the way from `prev` to `curr` at which the
                // straight line between them reaches zero, in `[0, 1)`.
                let frac = prev_f / (prev_f - curr_f);
                let crossing = (index as f64 - 1.0) + frac;

                if let Some(last) = self.last_crossing {
                    // Successive crossings are half a period apart.
                    let half_period = crossing - last;
                    if half_period > 0.0 {
                        let hz = self.sample_rate as f64 / (2.0 * half_period);
                        self.frequency = Frequency::from_hz(round_hz(hz));
                        self.has_started = true;
                    }
                }
                self.last_crossing = Some(crossing);
            }

            self.previous_sample = current_sample;

            if self.has_started {
                return Some(self.frequency);
            }
        }
    }
}

/// Round a positive, finite frequency to the nearest whole Hertz, mapping
/// anything else (non-finite, zero, negative) to zero.
fn round_hz(hz: f64) -> u32 {
    if hz.is_finite() && hz > 0.0 {
        (hz + 0.5) as u32
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthesizer::{Synthesizer, Tone};
    use crate::units::Duration;

    #[test]
    fn pure_1500hz_at_48000() {
        let actual_frequency = Frequency::from_hz(1500);
        let sample_rate: u32 = 48_000;
        test_pure_frequency(actual_frequency, sample_rate);
    }

    #[test]
    fn pure_2300hz_at_48000() {
        let actual_frequency = Frequency::from_hz(2300);
        let sample_rate: u32 = 48_000;
        test_pure_frequency(actual_frequency, sample_rate);
    }

    #[test]
    fn pure_1200hz_at_8000() {
        let actual_frequency = Frequency::from_hz(1200);
        let sample_rate: u32 = 8_000;
        test_pure_frequency(actual_frequency, sample_rate);
    }

    #[test]
    fn pure_1000hz_at_8000() {
        let actual_frequency = Frequency::from_hz(1000);
        let sample_rate: u32 = 8_000;
        test_pure_frequency(actual_frequency, sample_rate);
    }

    #[test]
    fn noisy_2300hz_at_48000() {
        let actual_frequency = Frequency::from_hz(2300);
        let sample_rate: u32 = 48_000;
        let signal_to_noise_ratio_db = 20.0;

        let clean = synthesize(vec![actual_frequency], sample_rate);
        let samples = add_noise(&clean, signal_to_noise_ratio_db);
        let mut estimates: Vec<Frequency> =
            Demodulator::new(samples.into_iter(), sample_rate).collect();

        assert!(!estimates.is_empty());

        estimates.sort();
        let median = estimates[estimates.len() / 2];
        assert!(
            frequencies_match(median, actual_frequency),
            "median {} Hz ≉ {} Hz",
            median.hz(),
            actual_frequency.hz(),
        )
    }

    #[test]
    fn switch_between_two_frequencies() {
        let first_actual = Frequency::from_hz(1500);
        let second_actual = Frequency::from_hz(2300);
        let sample_rate: u32 = 48_000;

        let samples = synthesize(vec![first_actual, second_actual], sample_rate);
        let estimates: Vec<Frequency> =
            Demodulator::new(samples.clone().into_iter(), sample_rate).collect();

        for estimated in estimates {
            if !frequencies_match(estimated, first_actual)
                && !frequencies_match(estimated, second_actual)
            {
                assert!(
                    false,
                    "{} not in [{}, {}]",
                    estimated.hz(),
                    first_actual.hz(),
                    second_actual.hz()
                )
            }
        }
    }

    #[test]
    fn empty_samples_yields_empty_frequencies() {
        let samples = vec![];
        let estimates: Vec<Frequency> = Demodulator::new(samples.into_iter(), 48_000).collect();

        assert!(estimates.is_empty());
    }

    fn synthesize(frequencies: Vec<Frequency>, sample_rate: u32) -> Vec<i16> {
        let tones: Vec<Tone> = frequencies
            .into_iter()
            .map(|freq| Tone::new(freq, Duration::from_ms(10)))
            .collect();
        Synthesizer::new(tones.into_iter(), sample_rate).collect()
    }

    fn test_pure_frequency(actual_frequency: Frequency, sample_rate: u32) {
        let samples = synthesize(vec![actual_frequency], sample_rate);
        let estimates: Vec<Frequency> =
            Demodulator::new(samples.clone().into_iter(), sample_rate).collect();

        assert!(!estimates.is_empty());

        for estimated_frequency in estimates {
            assert!(
                frequencies_match(estimated_frequency, actual_frequency),
                "{} Hz ≉ {} Hz",
                estimated_frequency.hz(),
                actual_frequency.hz(),
            )
        }
    }

    fn frequencies_match(estimated: Frequency, actual: Frequency) -> bool {
        let allowed_deviation = actual / 100;
        let deviation = estimated.abs_diff(actual);
        deviation <= allowed_deviation
    }

    struct Noise(u64);
    impl Noise {
        fn next_u64(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
            z ^ (z >> 31)
        }
        fn next_gaussian(&mut self) -> f64 {
            let u1 = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64 + f64::MIN_POSITIVE;
            let u2 = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
            (-2.0 * u1.ln()).sqrt() * (core::f64::consts::TAU * u2).cos()
        }
    }

    fn add_noise(samples: &[i16], snr_db: f64) -> Vec<i16> {
        let signal_rms = i16::MAX as f64 / core::f64::consts::SQRT_2;
        let sigma = signal_rms / 10f64.powf(snr_db / 20.0);

        let seed: u64 = 0x5EED;
        let mut noise = Noise(seed);
        samples
            .iter()
            .map(|&s| {
                (s as f64 + noise.next_gaussian() * sigma)
                    .round()
                    .clamp(i16::MIN as f64, i16::MAX as f64) as i16
            })
            .collect()
    }
}
