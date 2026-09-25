//! The [`Mode`] type and the calibration header preceding a mode's image
//! data.

use super::layout::Layout;
use super::{ALL, LEADER_FREQUENCY, SYNC_FREQUENCY, VisCode};
use crate::synthesizer::Tone;
use crate::{Error, ms, tone};

/// Tuning (VOX) tones customarily sent ahead of the calibration header to
/// open receiver squelch. They are not part of the paper's specification.
const VOX_TONES: [Tone; 8] = [
    tone!(1900 Hz, 100 ms),
    tone!(1500 Hz, 100 ms),
    tone!(1900 Hz, 100 ms),
    tone!(1500 Hz, 100 ms),
    tone!(2300 Hz, 100 ms),
    tone!(1500 Hz, 100 ms),
    tone!(2300 Hz, 100 ms),
    tone!(1500 Hz, 100 ms),
];

/// A specific protocol for encoding an image as a tone sequence.
///
/// The modes themselves are constants in [`modes`](crate::modes)
/// ([`modes::ROBOT_36`](crate::modes::ROBOT_36),
/// [`modes::SCOTTIE_1`](crate::modes::SCOTTIE_1), …), each defined in its own
/// module.
#[derive(Clone, Copy)]
pub struct Mode {
    /// The mode's name, as [`Debug`](core::fmt::Debug) prints it.
    pub(super) name: &'static str,
    /// The VIS code identifying the mode to a receiving system.
    pub(super) vis_code: VisCode,
    /// Whether one extra sync pulse precedes the first line (Scottie modes).
    pub(super) starting_sync_pulse: bool,
    /// The scanline structure specified by the mode's timing-sequence table.
    pub(super) layout: Layout,
}

impl Mode {
    /// The mode's VIS code, identifying it to a receiving system.
    #[must_use]
    pub const fn vis_code(&self) -> VisCode {
        self.vis_code
    }

    /// The mode's scanline structure as specified by its timing-sequence
    /// table in the paper.
    pub(crate) const fn layout(self) -> Layout {
        self.layout
    }

    /// The number of pixels the encoder buffers for this mode — one full
    /// line group. This is the length [`Encoder::new_in`](crate::Encoder)
    /// requires of its buffer.
    #[must_use]
    pub const fn encoder_buffer_len(&self) -> usize {
        self.layout.lines_per_cycle() * self.layout.resolution.0
    }

    /// The image resolution in pixels, as (width, height).
    #[must_use]
    pub const fn resolution(&self) -> (u32, u32) {
        (
            self.layout.resolution.0 as u32,
            self.layout.resolution.1 as u32,
        )
    }

    /// Whether the mode transmits one extra sync pulse between the header and
    /// the first line. Only Scottie modes do.
    pub(crate) const fn has_starting_sync_pulse(self) -> bool {
        self.starting_sync_pulse
    }

    /// The tones sent before the image: the VOX tuning tones, the calibration
    /// header carrying the VIS code, and the starting sync pulse for modes
    /// that transmit one. The image data begins immediately after the last
    /// header tone.
    pub fn header_tones(&self) -> impl Iterator<Item = Tone> + '_ {
        (0..).map_while(move |index| self.header_tone(index))
    }

    /// The `index`-th header tone, or `None` past the end of the header.
    pub(crate) fn header_tone(self, index: usize) -> Option<Tone> {
        match index {
            0..=7 => Some(VOX_TONES[index]),
            8 | 10 => Some(Tone::new(LEADER_FREQUENCY, ms!(300))),
            9 => Some(Tone::new(SYNC_FREQUENCY, ms!(10))), // break
            11..=20 => self.vis_code.tone(index - 11),
            21 if self.has_starting_sync_pulse() => {
                Some(Tone::new(SYNC_FREQUENCY, self.layout().sync_pulse().1))
            }
            _ => None,
        }
    }
}

/// Look up the mode a [`VisCode`] identifies.
impl TryFrom<VisCode> for Mode {
    type Error = Error;

    /// # Errors
    ///
    /// [`Error::UnknownMode`] if no mode carries the code.
    fn try_from(code: VisCode) -> Result<Self, Error> {
        ALL.into_iter()
            .find(|mode| mode.vis_code == code)
            .ok_or(Error::UnknownMode)
    }
}

// The VIS code is unique to each mode, so it serves as the mode's identity —
// comparing the layouts would walk their timing sequences.
impl PartialEq for Mode {
    fn eq(&self, other: &Self) -> bool {
        self.vis_code == other.vis_code
    }
}

impl Eq for Mode {}

impl core::hash::Hash for Mode {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.vis_code.hash(state);
    }
}

impl core::fmt::Debug for Mode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name)
    }
}

/// The shared assertions behind every mode's tests, and the tests of the
/// mode-independent header structure.
#[cfg(test)]
pub mod testing {
    extern crate std;
    use std::vec::Vec;

    use super::*;
    use crate::units::Duration;
    use crate::us;

    /// Assert that every timing sequence sums to the paper's line period —
    /// the decoder relies on the sync pulses being evenly spaced.
    pub fn assert_line_period(mode: Mode, expected: Duration) {
        for sequence in mode.layout().sequences {
            let sum = sequence
                .iter()
                .fold(us!(0), |sum, step| sum + step.duration());
            assert_eq!(sum, expected);
        }
    }

    /// Assert that the transcribed steps, summed over all lines, reproduce
    /// the transmission time the paper publishes alongside the per-step
    /// timings — this catches a transcription mistake in any single step.
    pub fn assert_transmission_time(mode: Mode, expected_seconds: f64) {
        let layout = mode.layout();
        let passes = (layout.resolution.1 / layout.lines_per_sequence) as f64;
        let seconds = passes * layout.sequence_duration().ns() as f64 / 1e9;
        assert!(
            (seconds - expected_seconds).abs() < 0.1,
            "{seconds}s instead of {expected_seconds}s",
        );
    }

    /// Assert that looking up the mode's VIS code yields the mode again — a
    /// collision between two modes fails the lookup.
    pub fn assert_mode_can_be_constructed_from_vis_code(mode: Mode) {
        assert_eq!(Mode::try_from(mode.vis_code()), Ok(mode));
    }

    #[test]
    fn header_tones_are_vox_leader_and_vis_code() {
        let expected: Vec<_> = VOX_TONES
            .into_iter()
            .chain([
                tone!(1900 Hz, 300 ms), // leader
                tone!(1200 Hz, 10 ms),  // break
                tone!(1900 Hz, 300 ms), // leader
            ])
            .chain(crate::modes::ROBOT_36.vis_code().tones())
            .collect();

        assert_eq!(
            crate::modes::ROBOT_36.header_tones().collect::<Vec<_>>(),
            expected
        );
    }
}
