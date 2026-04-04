//! FM Synthesizer for SSTV audio generation
//!
//! Implements a phase-accumulator based FM synthesizer following the approach
//! used in QSSTV's synthes.cpp for sample-rate independent tone generation.
//!
//! Reference: qsstv/src/dsp/synthes.cpp and synthes.h

use crate::constants::{AUDIO_AMPLITUDE, SINE_TABLE_LEN};
use std::f64::consts::PI;

/// FM Synthesizer using phase accumulator approach
///
/// This generates audio samples for SSTV transmission using the same
/// algorithm as QSSTV for maximum compatibility.
///
/// Derived from qsstv/src/dsp/synthes.h:32-68
pub struct Synthesizer {
    /// Current phase angle (0 to SINE_TABLE_LEN)
    phase: f64,
    /// Sample rate in Hz
    sample_rate: f64,
    /// Pre-computed sine lookup table
    sine_table: [f64; SINE_TABLE_LEN],
    /// Adjustment accumulator for precise timing
    adjust: f64,
}

impl Synthesizer {
    /// Create a new synthesizer with the given sample rate
    ///
    /// # Arguments
    /// * `sample_rate` - Audio sample rate in Hz (e.g., 44100, 48000)
    pub fn new(sample_rate: u32) -> Self {
        let mut sine_table = [0.0; SINE_TABLE_LEN];

        // Generate sine lookup table
        // Derived from qsstv/src/dsp/synthes.cpp:44-47
        // sineTable[i] = (sin(((double)i * M_PI * 2.) / SINTABLEN) * 8000.)
        for i in 0..SINE_TABLE_LEN {
            sine_table[i] = (i as f64 * PI * 2.0 / SINE_TABLE_LEN as f64).sin() * AUDIO_AMPLITUDE;
        }

        Self {
            phase: 0.0,
            sample_rate: sample_rate as f64,
            sine_table,
            adjust: 0.0,
        }
    }

    /// Reset the synthesizer state
    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.adjust = 0.0;
    }

    /// Generate the next sample at the given frequency
    ///
    /// Implements the phase accumulator approach from QSSTV:
    /// ```cpp
    /// // qsstv/src/dsp/synthes.h:37-45
    /// double temp = (freq / txSamplingClock) * SINTABLEN + oldAngle;
    /// oldAngle = fmod(temp, SINTABLEN);
    /// int t = (int)(oldAngle + 0.5);
    /// return sineTable[t % SINTABLEN];
    /// ```
    #[inline]
    pub fn next_sample(&mut self, freq: f64) -> f64 {
        // Calculate phase increment for this frequency
        let phase_increment = (freq / self.sample_rate) * SINE_TABLE_LEN as f64;

        // Advance phase with wraparound
        self.phase = (self.phase + phase_increment) % SINE_TABLE_LEN as f64;

        // Get sample from lookup table with interpolation
        let index = (self.phase + 0.5) as usize % SINE_TABLE_LEN;
        self.sine_table[index]
    }

    /// Generate samples for a tone of given duration
    ///
    /// # Arguments
    /// * `duration` - Duration in seconds
    /// * `freq` - Frequency in Hz
    /// * `concat` - If true, maintains phase continuity; if false, resets adjust
    ///
    /// Derived from qsstv/src/dsp/synthes.cpp:63-76
    pub fn generate_tone(&mut self, duration: f64, freq: f64, concat: bool) -> Vec<f64> {
        if !concat {
            self.adjust = 0.0;
        }

        // Convert duration to number of samples with adjustment for precise timing
        // Derived from qsstv/src/dsp/synthes.cpp:73-74
        let num_samples = ((duration + self.adjust) * self.sample_rate + 0.5) as usize;
        self.adjust += duration - (num_samples as f64 / self.sample_rate);

        let mut samples = Vec::with_capacity(num_samples);
        for _ in 0..num_samples {
            samples.push(self.next_sample(freq));
        }
        samples
    }

    /// Generate samples for a frequency sweep
    ///
    /// # Arguments
    /// * `duration` - Duration in seconds
    /// * `start_freq` - Starting frequency in Hz
    /// * `end_freq` - Ending frequency in Hz
    ///
    /// Derived from qsstv/src/dsp/synthes.cpp:105-114
    pub fn generate_sweep(&mut self, duration: f64, start_freq: f64, end_freq: f64) -> Vec<f64> {
        let num_samples = (duration * self.sample_rate) as usize;
        let delta_freq = (end_freq - start_freq) / num_samples as f64;

        let mut samples = Vec::with_capacity(num_samples);
        for i in 0..num_samples {
            let freq = start_freq + delta_freq * i as f64;
            samples.push(self.next_sample(freq));
        }
        samples
    }

    /// Generate silence (zeros)
    ///
    /// # Arguments
    /// * `duration` - Duration in seconds
    pub fn generate_silence(&mut self, duration: f64) -> Vec<f64> {
        let num_samples = (duration * self.sample_rate + 0.5) as usize;
        vec![0.0; num_samples]
    }

    /// Get the current sample rate
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate as u32
    }

    /// Calculate number of samples for a given duration
    pub fn samples_for_duration(&self, duration: f64) -> usize {
        (duration * self.sample_rate + 0.5) as usize
    }
}

/// Represents a tone to be generated
#[derive(Debug, Clone, Copy)]
pub struct Tone {
    /// Frequency in Hz
    pub freq: f64,
    /// Duration in seconds
    pub duration: f64,
}

impl Tone {
    /// Create a new tone
    pub fn new(freq: f64, duration: f64) -> Self {
        Self { freq, duration }
    }

    /// Create a sync pulse tone
    pub fn sync(duration: f64) -> Self {
        Self::new(crate::constants::FREQ_SYNC, duration)
    }

    /// Create a blanking tone
    pub fn blank(duration: f64) -> Self {
        Self::new(crate::constants::FREQ_BLACK, duration)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthesizer_creation() {
        let synth = Synthesizer::new(48000);
        assert_eq!(synth.sample_rate(), 48000);
    }

    #[test]
    fn test_tone_generation() {
        let mut synth = Synthesizer::new(48000);
        let samples = synth.generate_tone(0.1, 1000.0, false);
        // 0.1 seconds at 48000 Hz = 4800 samples
        assert!((samples.len() as i32 - 4800).abs() <= 1);
    }

    #[test]
    fn test_sample_amplitude() {
        let mut synth = Synthesizer::new(48000);
        let samples = synth.generate_tone(0.01, 1000.0, false);

        // All samples should be within amplitude bounds
        for sample in samples {
            assert!(sample.abs() <= AUDIO_AMPLITUDE + 1.0);
        }
    }

    #[test]
    fn test_silence() {
        let mut synth = Synthesizer::new(48000);
        let samples = synth.generate_silence(0.1);

        for sample in samples {
            assert_eq!(sample, 0.0);
        }
    }
}
