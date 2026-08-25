//! Building blocks for describing a mode's transmission exactly as the Dayton
//! paper does: each mode is a repeating per-line "TIMING SEQUENCE" of fixed
//! tones (sync pulses, porches, separator pulses) and channel scans.

use crate::synthesizer::Tone;
use crate::units::{Duration, Frequency};

/// The image values carried by a scan step, named after the scans in the
/// paper's timing sequences ("Green scan", "Y scan", "R-Y scan", ...).
// Some variants are not constructed yet: they cover the scans of the
// Dayton-paper modes still to be added, and the encoder/decoder already
// handle them.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Channel {
    /// "Red scan".
    Red,
    /// "Green scan".
    Green,
    /// "Blue scan".
    Blue,
    /// "Y scan" — luminance. In sequences carrying two lines (PD modes), the
    /// first line's.
    Y,
    /// "Y scan (from even line)" — the second line's luminance (PD modes).
    YSecond,
    /// "R-Y scan" — the red colour difference.
    RY,
    /// "B-Y scan" — the blue colour difference.
    BY,
}

/// One entry of a mode's "TIMING SEQUENCE" table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Step {
    /// A fixed control tone: sync pulse, sync porch, separator pulse or porch.
    Tone(Tone),
    /// A channel scan: one line of pixels spread evenly over the duration.
    Scan(Channel, Duration),
}

impl Step {
    pub(crate) const fn tone(frequency: Frequency, duration: Duration) -> Self {
        Self::Tone(Tone::new(frequency, duration))
    }

    pub(crate) const fn scan(channel: Channel, duration: Duration) -> Self {
        Self::Scan(channel, duration)
    }

    pub(crate) const fn duration(&self) -> Duration {
        match self {
            Self::Tone(tone) => tone.duration,
            Self::Scan(_, duration) => *duration,
        }
    }
}

/// How the scans of one timing sequence combine into image pixels.
// Some variants are not constructed yet: they cover the colour systems of the
// Dayton-paper modes still to be added, and the encoder/decoder already
// handle them.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColorMode {
    /// Red, Green and Blue scans of a single line (Martin, Scottie, Wrasse,
    /// Pasokon).
    Rgb,
    /// Y, R-Y and B-Y scans of a single line (Robot 72).
    Yuv,
    /// Y plus a single colour-difference scan per line: R-Y on even lines,
    /// B-Y on odd lines, each averaged over the line pair (Robot 36).
    YuvAveragedPair,
    /// Y scans of two consecutive lines around shared, pair-averaged R-Y and
    /// B-Y scans (PD modes).
    YuvSharedPair,
    /// A single luminance scan (FAX480).
    Monochrome,
}

/// A mode's scanline structure: the paper's timing sequences plus the image
/// geometry they carry.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Layout {
    /// The horizontal resolution in pixels.
    pub width: usize,
    /// The number of image lines in a full transmission.
    pub height: usize,
    /// Tones sent once between the header and the first line (Scottie's
    /// "starting" sync pulse).
    pub start: &'static [Step],
    /// The repeating timing sequences; lines cycle through them in order.
    /// All sequences of a mode are equally long. Most modes have exactly one;
    /// Robot 36 alternates an even-line and an odd-line sequence.
    pub sequences: &'static [&'static [Step]],
    /// Image lines carried by one pass through one sequence (2 for PD modes).
    pub lines_per_sequence: usize,
    /// How the scans combine into pixels.
    pub color: ColorMode,
}

impl Layout {
    /// Image lines carried by one pass through *all* sequences.
    pub(crate) fn lines_per_cycle(&self) -> usize {
        self.lines_per_sequence * self.sequences.len()
    }

    /// The duration of one timing sequence — the spacing of the sync pulses.
    pub(crate) fn sequence_duration(&self) -> Duration {
        Self::steps_duration(self.sequences[0])
    }

    /// The duration of the tones sent once before the first line.
    pub(crate) fn start_duration(&self) -> Duration {
        Self::steps_duration(self.start)
    }

    fn steps_duration(steps: &[Step]) -> Duration {
        let mut sum = Duration::from_ns(0);
        for step in steps {
            sum = sum + step.duration();
        }
        sum
    }

    /// The sync pulse's offset within a sequence and its duration.
    ///
    /// Zero offset for most modes; Scottie places the sync pulse between the
    /// Blue and Red scans.
    pub(crate) fn sync_pulse(&self) -> (Duration, Duration) {
        let mut offset = Duration::from_ns(0);
        for step in self.sequences[0] {
            if let Step::Tone(tone) = step
                && tone.frequency == super::SYNC_FREQUENCY
            {
                return (offset, tone.duration);
            }
            offset = offset + step.duration();
        }
        // Every mode's sequence contains a sync pulse.
        (Duration::from_ns(0), Duration::from_ns(0))
    }

    /// Where alternating sequences differ (Robot 36's separator pulse): the
    /// index — counting only non-sync tone steps, as a decoder samples them —
    /// of the tone whose frequency identifies the line parity.
    pub(crate) fn parity_tone(&self) -> Option<usize> {
        let (first, second) = match self.sequences {
            [first, second, ..] => (first, second),
            _ => return None,
        };
        let mut tone_index = 0;
        for (a, b) in first.iter().zip(second.iter()) {
            if let (Step::Tone(x), Step::Tone(y)) = (a, b) {
                if x.frequency == super::SYNC_FREQUENCY {
                    continue;
                }
                if x.frequency != y.frequency {
                    return Some(tone_index);
                }
                tone_index += 1;
            }
        }
        None
    }
}
