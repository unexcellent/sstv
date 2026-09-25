//! The SSTV modes specified in the Dayton paper — JL Barber (N7CXI),
//! "Proposal for SSTV Mode Specifications", presented at the Dayton SSTV
//! forum, 20 May 2000.
//!
//! Each mode lives in its own module, transcribing its timing table from the
//! paper into a `Layout`. Everything shared between modes — the frequency
//! range, the calibration header and the VIS code — is defined here, as in
//! the paper's common sections.

pub(crate) mod layout;

mod martin_1;
mod martin_2;
mod pasokon_p3;
mod pasokon_p5;
mod pasokon_p7;
mod pd_120;
mod pd_160;
mod pd_180;
mod pd_240;
mod pd_290;
mod pd_50;
mod pd_90;
mod robot_36;
mod robot_72;
mod scottie_1;
mod scottie_2;
mod scottie_dx;
mod wrasse_sc2_180;

use crate::synthesizer::Tone;
use crate::units::{Duration, Frequency};
use crate::{Hz, ms, tone};
use layout::Layout;

pub use martin_1::MARTIN_1;
pub use martin_2::MARTIN_2;
pub use pasokon_p3::PASOKON_P3;
pub use pasokon_p5::PASOKON_P5;
pub use pasokon_p7::PASOKON_P7;
pub use pd_50::PD_50;
pub use pd_90::PD_90;
pub use pd_120::PD_120;
pub use pd_160::PD_160;
pub use pd_180::PD_180;
pub use pd_240::PD_240;
pub use pd_290::PD_290;
pub use robot_36::ROBOT_36;
pub use robot_72::ROBOT_72;
pub use scottie_1::SCOTTIE_1;
pub use scottie_2::SCOTTIE_2;
pub use scottie_dx::SCOTTIE_DX;
pub use wrasse_sc2_180::WRASSE_SC2_180;

/// Every transmission mode, in the paper's order.
pub const ALL: [Mode; 18] = [
    SCOTTIE_1,
    SCOTTIE_2,
    SCOTTIE_DX,
    MARTIN_1,
    MARTIN_2,
    ROBOT_36,
    ROBOT_72,
    WRASSE_SC2_180,
    PASOKON_P3,
    PASOKON_P5,
    PASOKON_P7,
    PD_50,
    PD_90,
    PD_120,
    PD_160,
    PD_180,
    PD_240,
    PD_290,
];

/// The sync pulse frequency, shared by every mode.
pub(crate) const SYNC_FREQUENCY: Frequency = Hz!(1200);
/// Pure black — the lower end of the luminance range.
pub(crate) const BLACK_FREQUENCY: Frequency = Hz!(1500);
/// Pure white — the upper end of the luminance range.
pub(crate) const WHITE_FREQUENCY: Frequency = Hz!(2300);
/// The leader tone of the calibration header.
pub(crate) const LEADER_FREQUENCY: Frequency = Hz!(1900);
const VIS_ONE_FREQUENCY: Frequency = Hz!(1100);
const VIS_ZERO_FREQUENCY: Frequency = Hz!(1300);
/// Every VIS bit (start, data, parity, stop) lasts 30ms.
const VIS_BIT_DURATION: Duration = ms!(30);

/// The frequency representing a pixel value, mapped linearly onto the
/// luminance range.
pub(crate) fn value_frequency(value: u8) -> Frequency {
    BLACK_FREQUENCY + (WHITE_FREQUENCY - BLACK_FREQUENCY) * u32::from(value) / 255
}

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
    name: &'static str,
    /// The 7-bit VIS code identifying the mode to a receiving system.
    vis_code: u8,
    /// Whether one extra sync pulse precedes the first line (Scottie modes).
    starting_sync_pulse: bool,
    /// The scanline structure specified by the mode's timing-sequence table.
    layout: Layout,
}

impl Mode {
    /// The mode's 7-bit VIS code, identifying it to a receiving system.
    #[must_use]
    pub const fn vis_code(&self) -> u8 {
        self.vis_code
    }

    /// Look up a mode by its 7-bit VIS code.
    #[must_use]
    pub fn from_vis_code(code: u8) -> Option<Self> {
        ALL.into_iter().find(|mode| mode.vis_code == code)
    }

    /// The mode's scanline structure as specified by its timing-sequence
    /// table in the paper.
    pub(crate) const fn layout(self) -> Layout {
        self.layout
    }

    /// The horizontal resolution in pixels.
    #[must_use]
    pub const fn image_width(&self) -> u32 {
        self.layout.width as u32
    }

    /// The vertical resolution in pixels.
    #[must_use]
    pub const fn image_height(&self) -> u32 {
        self.layout.height as u32
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
        let code = self.vis_code();
        let bit = |one: bool| {
            let frequency = if one {
                VIS_ONE_FREQUENCY
            } else {
                VIS_ZERO_FREQUENCY
            };
            Tone::new(frequency, VIS_BIT_DURATION)
        };
        match index {
            0..=7 => Some(VOX_TONES[index]),
            8 | 10 => Some(Tone::new(LEADER_FREQUENCY, ms!(300))),
            9 => Some(Tone::new(SYNC_FREQUENCY, ms!(10))), // break
            11 | 20 => Some(Tone::new(SYNC_FREQUENCY, VIS_BIT_DURATION)), // start and stop bits
            12..=18 => Some(bit((code >> (index - 12)) & 1 == 1)), // code bits, least significant first
            19 => Some(bit(code.count_ones() % 2 == 1)),           // even parity
            21 if self.has_starting_sync_pulse() => {
                Some(Tone::new(SYNC_FREQUENCY, self.layout().sync_pulse().1))
            }
            _ => None,
        }
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

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec::Vec;

    use super::*;
    use crate::{ms, tone, us};

    #[test]
    fn header_tones_robot36() {
        assert_eq!(
            ROBOT_36.header_tones().collect::<Vec<_>>(),
            std::vec![
                tone!(1900 Hz, 100 ms),
                tone!(1500 Hz, 100 ms),
                tone!(1900 Hz, 100 ms),
                tone!(1500 Hz, 100 ms),
                tone!(2300 Hz, 100 ms),
                tone!(1500 Hz, 100 ms),
                tone!(2300 Hz, 100 ms),
                tone!(1500 Hz, 100 ms),
                tone!(1900 Hz, 300 ms),
                tone!(1200 Hz, 10 ms),
                tone!(1900 Hz, 300 ms),
                tone!(1200 Hz, 30 ms),
                tone!(1300 Hz, 30 ms),
                tone!(1300 Hz, 30 ms),
                tone!(1300 Hz, 30 ms),
                tone!(1100 Hz, 30 ms),
                tone!(1300 Hz, 30 ms),
                tone!(1300 Hz, 30 ms),
                tone!(1300 Hz, 30 ms),
                tone!(1100 Hz, 30 ms),
                tone!(1200 Hz, 30 ms),
            ]
        );
    }

    #[test]
    fn vis_codes_round_trip() {
        for mode in ALL {
            let code = mode.vis_code();
            assert_eq!(Mode::from_vis_code(code), Some(mode));
            assert!(code < 128, "VIS codes are 7 bit");
        }
    }

    /// Every sequence of a mode must be equally long — the decoder relies on
    /// the sync pulses being evenly spaced.
    #[test]
    fn sequences_are_equally_long() {
        for mode in ALL {
            let layout = mode.layout();
            let duration = layout.sequence_duration();
            for sequence in layout.sequences {
                let sum = sequence
                    .iter()
                    .fold(us!(0), |sum, step| sum + step.duration());
                assert_eq!(sum, duration, "{mode:?}");
            }
        }
    }

    /// The per-line duration from the paper: Robot 36 transmits 240 lines in
    /// 36 seconds — 150.0ms per line.
    #[test]
    fn robot36_line_duration_matches_paper() {
        assert_eq!(ROBOT_36.layout().sequence_duration(), ms!(150));
    }

    /// The paper publishes each mode's total transmission time (excluding the
    /// header) alongside the per-step timings. Summing our transcribed steps
    /// over all lines must reproduce those times, which catches transcription
    /// mistakes in any single step.
    #[test]
    fn transmission_times_match_paper() {
        let expected_seconds = [
            (SCOTTIE_1, 109.6),
            (SCOTTIE_2, 71.1),
            (SCOTTIE_DX, 268.9),
            (MARTIN_1, 114.3),
            (MARTIN_2, 58.06),
            (ROBOT_36, 36.0),
            (ROBOT_72, 72.0),
            (WRASSE_SC2_180, 182.0),
            (PASOKON_P3, 203.0),
            (PASOKON_P5, 304.6),
            (PASOKON_P7, 406.1),
            (PD_50, 49.7),
            (PD_90, 90.0),
            (PD_120, 126.1),
            (PD_160, 160.9),
            (PD_180, 187.1),
            (PD_240, 248.0),
            (PD_290, 288.7),
        ];
        for (mode, expected) in expected_seconds {
            let layout = mode.layout();
            let passes = (layout.height / layout.lines_per_sequence) as f64;
            let seconds = passes * layout.sequence_duration().ns() as f64 / 1e9;
            assert!(
                (seconds - expected).abs() < 0.1,
                "{mode:?}: {seconds}s instead of {expected}s",
            );
        }
    }
}
