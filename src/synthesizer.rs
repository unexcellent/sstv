use crate::units::{Duration, Frequency};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tone(pub Frequency, pub Duration);
