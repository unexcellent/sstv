include!(concat!(env!("OUT_DIR"), "/sine_table.rs"));

use crate::units::{Duration, Frequency};

/// A single frequency emitted for a certain duration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tone {
    /// The frequency
    pub frequency: Frequency,
    /// The duration
    pub duration: Duration,
}

impl Tone {
    /// Create a new Tone
    pub fn new(frequency: Frequency, duration: Duration) -> Self {
        Self {
            frequency,
            duration,
        }
    }
}

/// Use a `Synthesizer` to encode an iterator of `Tone`s into 16-bit PCM samples.
///
/// You would usually use it to encode an image into single samples to emit them via an audio device.
/// ```rust
/// use sstv::{Encoder, Mode, RgbPixel, Synthesizer};
///
/// let image = [RgbPixel::new(0, 0, 0); 320 * 240];
/// let encoder = Encoder::new(Mode::Robot36, image.into_iter()).expect("error during encoding");
/// for sample in Synthesizer::new(encoder, 8000) {
///     // ...
/// }
/// ```
///
/// It can be used with a single `Tone` as well.
/// ```rust
/// use sstv::{Synthesizer, Tone, Hz, us};
///
/// let tone = Tone::new(Hz!(1500), us!(1000));
/// for sample in Synthesizer::new([tone].into_iter(), 8000) {
///     // ...
/// }
/// ```
pub struct Synthesizer<I: Iterator<Item = Tone>> {
    tones: I,
    sample_rate: u32,
    phase: u32,
    phase_increment: u32,
    samples_remaining: u32,
    sample_carry: u64,
}

impl<I: Iterator<Item = Tone>> Synthesizer<I> {
    /// Create a new `Synthesizer` from a tone iterator and a sample rate in Hz.
    ///
    /// `sample_rate` must be greater than zero.
    pub fn new(tones: I, sample_rate: u32) -> Self {
        Self {
            tones,
            sample_rate: sample_rate.max(1),
            phase: 0,
            phase_increment: 0,
            samples_remaining: 0,
            sample_carry: 0,
        }
    }

    fn load_next_tone(&mut self) -> Option<()> {
        let tone = self.tones.next()?;
        let numerator = tone.duration.ns() * self.sample_rate as u64 + self.sample_carry;
        self.samples_remaining = (numerator / 1_000_000_000) as u32;
        self.sample_carry = numerator % 1_000_000_000;
        self.phase_increment =
            ((tone.frequency.hz() as u64 * (1u64 << 32)) / self.sample_rate as u64) as u32;
        Some(())
    }
}

impl<I: Iterator<Item = Tone>> Iterator for Synthesizer<I> {
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        while self.samples_remaining == 0 {
            self.load_next_tone()?;
        }
        let sample = SINE_TABLE[(self.phase.wrapping_add(1 << 23) >> 24) as usize];
        self.phase = self.phase.wrapping_add(self.phase_increment);
        self.samples_remaining -= 1;
        Some(sample)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::f64::consts::PI;

    // The 16 bit sample should not differ from the pure sinewave ground truth by more than 2%.
    const MAX_SAMPLE_DEVIATION: i16 = i16::MAX / 50;

    fn float_reference(tones: &[Tone], sample_rate: u32) -> Vec<i16> {
        let mut samples = Vec::new();
        let mut phase = 0.0f64;
        let mut carry: u64 = 0;

        for tone in tones {
            let numerator = tone.duration.ns() * sample_rate as u64 + carry;
            let count = (numerator / 1_000_000_000) as usize;
            carry = numerator % 1_000_000_000;

            let increment = 2.0 * PI * tone.frequency.hz() as f64 / sample_rate as f64;
            for _ in 0..count {
                samples.push((phase.sin() * i16::MAX as f64).round() as i16);
                phase += increment;
            }
        }
        samples
    }

    #[test]
    fn synthesizer_matches_pure_sine_wave() {
        let sample_rate = 48000u32;
        let tones = [
            Tone::new(Frequency::from_hz(1200), Duration::from_ms(10)),
            Tone::new(Frequency::from_hz(1500), Duration::from_ms(10)),
            Tone::new(Frequency::from_hz(2300), Duration::from_ms(10)),
        ];

        let dds: Vec<i16> = Synthesizer::new(tones.into_iter(), sample_rate).collect();
        let reference = float_reference(&tones, sample_rate);

        assert_eq!(dds.len(), reference.len(), "sample counts differ");

        for (i, (&dds_sample, &ref_sample)) in dds.iter().zip(reference.iter()).enumerate() {
            let deviation = (dds_sample - ref_sample as i16).abs();
            assert!(
                deviation <= MAX_SAMPLE_DEVIATION,
                "sample {i}: DDS={dds_sample}, reference={ref_sample}, deviation={deviation} > {MAX_SAMPLE_DEVIATION}",
            );
        }
    }
}
