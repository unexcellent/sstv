/// Numerically Controlled Oscillator for generating continuous phase sine waves.
pub struct Nco {
    phase: f32,
    sample_rate: f32,
}

impl Nco {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            phase: 0.0,
            sample_rate: sample_rate as f32,
        }
    }

    pub fn next_sample(&mut self, frequency_hz: f32) -> f32 {
        let sample = libm::sinf(self.phase);
        let phase_increment = (core::f32::consts::TAU * frequency_hz) / self.sample_rate;
        self.phase = (self.phase + phase_increment) % core::f32::consts::TAU;
        sample
    }
}
