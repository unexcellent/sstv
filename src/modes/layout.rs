//! Building blocks for describing a mode's transmission as the Dayton paper
//! does: each mode is a repeating timing sequence of fixed tones (sync
//! pulses, porches, separator pulses) and channel scans.

use crate::synthesizer::Tone;
use crate::units::Duration;

/// The image values carried by a scan step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Red,
    Green,
    Blue,
    /// Luminance. In sequences carrying two lines (Robot 36 and PD modes),
    /// the first line's.
    Y,
    /// The second line's luminance (Robot 36 and PD modes).
    YSecond,
    /// The red colour difference.
    RY,
    /// The blue colour difference.
    BY,
}

/// One entry of a mode's timing sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// A fixed control tone: sync pulse, sync porch, separator pulse or porch.
    Control(Tone),
    /// A channel scan: one line of pixels spread evenly over the duration.
    Scan(Channel, Duration),
}

impl Step {
    pub(crate) const fn duration(&self) -> Duration {
        match self {
            Self::Control(tone) => tone.duration,
            Self::Scan(_, duration) => *duration,
        }
    }
}

/// How the scans of one timing sequence combine into image pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    /// Red, Green and Blue scans of a single line (Martin, Scottie, Wrasse,
    /// Pasokon).
    Rgb,
    /// Y, R-Y and B-Y scans of a single line (Robot 72).
    Yuv,
    /// Y scans of two consecutive lines sharing pair-averaged R-Y and B-Y
    /// scans (Robot 36 and PD modes).
    YuvSharedPair,
}

/// A mode's scanline structure: the paper's timing sequence plus the image
/// geometry it carries.
#[derive(Debug, Clone, Copy)]
pub struct Layout {
    /// The horizontal and vertical resolution in pixels.
    pub resolution: (usize, usize),
    /// The repeating timing sequence. Its sync pulses are evenly spaced —
    /// the decoder relies on that to acquire and re-align.
    pub sequence: &'static [Step],
    /// Image lines carried by one pass through the sequence (2 for Robot 36
    /// and PD modes).
    pub lines_per_sequence: usize,
    /// How the scans combine into pixels.
    pub color: ColorMode,
}

impl Layout {
    /// The duration of one pass through the timing sequence.
    pub(crate) fn sequence_duration(&self) -> Duration {
        let mut sum = Duration::from_ns(0);
        for step in self.sequence {
            sum = sum + step.duration();
        }
        sum
    }

    /// Each step of the sequence with the offset at which it begins.
    pub(crate) fn step_offsets(&self) -> impl Iterator<Item = (Duration, &'static Step)> {
        self.sequence.iter().scan(Duration::from_ns(0), |at, step| {
            let offset = *at;
            *at = *at + step.duration();
            Some((offset, step))
        })
    }

    /// The offset at which each sync pulse starts.
    pub(crate) fn sync_offsets(&self) -> impl Iterator<Item = Duration> {
        self.step_offsets().filter_map(|(offset, step)| {
            matches!(step, Step::Control(tone) if tone.frequency == super::SYNC_FREQUENCY)
                .then_some(offset)
        })
    }

    /// The number of sync pulses in the sequence (2 for Robot 36).
    pub(crate) fn sync_count(&self) -> usize {
        self.sync_offsets().count()
    }

    /// The spacing of the sync pulses — the duration of one transmitted line.
    pub(crate) fn sync_spacing(&self) -> Duration {
        self.sequence_duration() / self.sync_count() as u32
    }

    /// The first sync pulse's offset within the sequence and its duration.
    ///
    /// Zero offset for most modes; Scottie places the sync pulse between the
    /// Blue and Red scans.
    pub(crate) fn sync_pulse(&self) -> (Duration, Duration) {
        for (offset, step) in self.step_offsets() {
            if let Step::Control(tone) = step
                && tone.frequency == super::SYNC_FREQUENCY
            {
                return (offset, tone.duration);
            }
        }
        // Every mode's sequence contains a sync pulse.
        (Duration::from_ns(0), Duration::from_ns(0))
    }
}
