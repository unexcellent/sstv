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
