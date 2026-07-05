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
    /// The previously seen sample, or `None` before the first one.
    prev: Option<i16>,
    /// Index of the *next* sample to be produced by `samples`.
    index: u64,
    /// Interpolated position (in samples) of the most recent zero crossing.
    last_crossing: Option<f64>,
    /// The most recent frequency estimate, valid once `started` is true.
    frequency: Frequency,
    /// Whether a first estimate has been determined and emission has begun.
    started: bool,
}

impl<I: Iterator<Item = i16>> Demodulator<I> {
    /// Create a new `Demodulator` from a sample iterator and a sample rate in Hz.
    ///
    /// `sample_rate` must be greater than zero.
    pub fn new(samples: I, sample_rate: u32) -> Self {
        Self {
            samples,
            sample_rate: sample_rate.max(1),
            prev: None,
            index: 0,
            last_crossing: None,
            frequency: Frequency::from_hz(0),
            started: false,
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
            let curr = self.samples.next()?;
            let idx = self.index;
            self.index += 1;

            if let Some(prev) = self.prev {
                // A sign change between the two samples marks a zero crossing.
                if (prev >= 0) != (curr >= 0) {
                    let prev_f = prev as f64;
                    let curr_f = curr as f64;
                    // Fraction of the way from `prev` to `curr` at which the
                    // straight line between them reaches zero, in `[0, 1)`.
                    let frac = prev_f / (prev_f - curr_f);
                    let crossing = (idx as f64 - 1.0) + frac;

                    if let Some(last) = self.last_crossing {
                        // Successive crossings are half a period apart.
                        let half_period = crossing - last;
                        if half_period > 0.0 {
                            let hz = self.sample_rate as f64 / (2.0 * half_period);
                            self.frequency = Frequency::from_hz(round_hz(hz));
                            self.started = true;
                        }
                    }
                    self.last_crossing = Some(crossing);
                }
            }

            self.prev = Some(curr);

            if self.started {
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

    fn synthesize(frequency: Frequency, sample_rate: u32) -> Vec<i16> {
        let tone = Tone::new(frequency, Duration::from_ms(10));
        Synthesizer::new([tone].into_iter(), sample_rate).collect()
    }

    fn test_pure_frequency(actual_frequency: Frequency, sample_rate: u32) {
        let samples = synthesize(actual_frequency, sample_rate);
        let estimates: Vec<Frequency> =
            Demodulator::new(samples.clone().into_iter(), sample_rate).collect();

        assert!(!estimates.is_empty());

        let allowed_deviation = actual_frequency / 100;
        for estimated_frequency in estimates {
            let deviation = estimated_frequency.abs_diff(actual_frequency);
            assert!(
                deviation < allowed_deviation,
                "|{} Hz - {} Hz| > {} Hz",
                estimated_frequency.hz(),
                actual_frequency.hz(),
                allowed_deviation.hz()
            )
        }
    }

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
}
