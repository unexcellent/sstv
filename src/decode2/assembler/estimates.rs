//! The frequency estimates kept for sampling an image by its timing.

use alloc::collections::VecDeque;

use crate::units::Frequency;

/// A window onto the frequency track, addressed by sample index since the
/// start of the stream.
#[derive(Default)]
pub(super) struct Estimates {
    buffer: VecDeque<Frequency>,
    /// The sample index of `buffer[0]`.
    start: u64,
}

impl Estimates {
    pub fn push(&mut self, frequency: Frequency) {
        self.buffer.push_back(frequency);
    }

    /// The sample index after the newest estimate.
    pub fn end(&self) -> u64 {
        self.start + self.buffer.len() as u64
    }

    /// Drop the estimates before sample `sample`.
    pub fn forget_before(&mut self, sample: u64) {
        while self.start < sample && self.buffer.pop_front().is_some() {
            self.start += 1;
        }
    }

    /// The mean of the estimates from (fractional) sample `start` over
    /// `length` samples, but at least one. Past the newest estimate, the
    /// newest one stands in: the demodulator's warm-up ends the track
    /// slightly before the signal does.
    pub fn mean(&self, start: f64, length: f64) -> Frequency {
        let first = (start.round().max(0.0) as u64).max(self.start);
        let last = ((start + length).round() as u64).max(first + 1);

        let (mut sum, mut count) = (0u64, 0u64);
        for sample in first..last {
            let estimate = usize::try_from(sample - self.start)
                .ok()
                .and_then(|index| self.buffer.get(index))
                .or_else(|| self.buffer.back());
            if let Some(estimate) = estimate {
                sum += u64::from(estimate.hz());
                count += 1;
            }
        }
        let mean = sum.checked_div(count).unwrap_or(0);
        Frequency::from_hz(u32::try_from(mean).unwrap_or(u32::MAX))
    }
}
