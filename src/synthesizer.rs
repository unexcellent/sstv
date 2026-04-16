use crate::units::{Duration, Frequency};

#[derive(Debug, Clone, Copy, PartialEq)]
/// A single frequency emitted for a certain duration.
pub struct Tone(pub Frequency, pub Duration);
