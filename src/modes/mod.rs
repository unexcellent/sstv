//! The SSTV modes specified in the Dayton paper — JL Barber (N7CXI),
//! "Proposal for SSTV Mode Specifications", presented at the Dayton SSTV
//! forum, 20 May 2000.
//!
//! Each mode family lives in its own module mirroring a chapter of the paper,
//! transcribing its "TIMING SEQUENCE" table into a [`Layout`]. Everything
//! shared between modes — the frequency range, the calibration header and the
//! VIS code — is defined here, as in the paper's common sections.

pub(crate) mod layout;

mod robot;

use crate::synthesizer::Tone;
use crate::units::{Duration, Frequency};
use crate::{Hz, ms};
use layout::Layout;

/// The "Sync pulse" frequency, shared by every mode.
pub(crate) const SYNC_FREQUENCY: Frequency = Hz!(1200);
/// Pure black. Appendix A: "SSTV systems use the frequency range of
/// 1500-2300hz to represent the range of brightness values".
pub(crate) const BLACK_FREQUENCY: Frequency = Hz!(1500);
/// Pure white — the upper end of the 1500-2300hz luminance range.
pub(crate) const WHITE_FREQUENCY: Frequency = Hz!(2300);
/// The calibration header's "Leader tone".
pub(crate) const LEADER_FREQUENCY: Frequency = Hz!(1900);
/// VIS bits: "1100hz = '1', 1300hz = '0'".
const VIS_ONE_FREQUENCY: Frequency = Hz!(1100);
const VIS_ZERO_FREQUENCY: Frequency = Hz!(1300);
/// Every VIS bit (start, data, parity, stop) lasts 30ms.
const VIS_BIT_DURATION: Duration = ms!(30);

/// Appendix A: "Frequency = 1500 + (ColorByte * 3.1372549)".
pub(crate) fn value_frequency(value: u8) -> Frequency {
    BLACK_FREQUENCY + (WHITE_FREQUENCY - BLACK_FREQUENCY) * value as u32 / 255
}

/// Tuning ("VOX") tones customarily sent ahead of the calibration header to
/// open receiver squelch. They are not part of the paper's specification.
const VOX_TONES: [Tone; 8] = [
    Tone::new(Hz!(1900), ms!(100)),
    Tone::new(Hz!(1500), ms!(100)),
    Tone::new(Hz!(1900), ms!(100)),
    Tone::new(Hz!(1500), ms!(100)),
    Tone::new(Hz!(2300), ms!(100)),
    Tone::new(Hz!(1500), ms!(100)),
    Tone::new(Hz!(2300), ms!(100)),
    Tone::new(Hz!(1500), ms!(100)),
];

/// A specific protocol for encoding an image as a tone sequence.
///
/// Naming follows the Dayton paper. Every mode transmits the shared
/// "Calibration header with VIS code" followed by its own repeating per-line
/// timing sequence; see [`Mode::header_tones`] and the `modes` submodules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Mode {
    /// "ROBOT 36 COLOR": Y, R-Y, B-Y encoded 320x240 in a 36s transmission.
    Robot36,
}

impl Mode {
    /// Every supported mode, in the paper's order.
    pub const ALL: [Mode; 1] = [Mode::Robot36];

    /// The mode's 7-bit VIS code ("VIS CODE" in the paper), identifying it to
    /// a receiving system. `None` for modes transmitted without a VIS code
    /// (the paper's FAX480 uses its own header instead).
    pub const fn vis_code(&self) -> Option<u8> {
        match self {
            Mode::Robot36 => Some(8),
        }
    }

    /// Look up a mode by its 7-bit VIS code.
    pub const fn from_vis_code(code: u8) -> Option<Mode> {
        match code {
            8 => Some(Mode::Robot36),
            _ => None,
        }
    }

    /// The mode's scanline structure as specified by its timing-sequence
    /// table in the paper.
    pub(crate) const fn layout(&self) -> Layout {
        match self {
            Mode::Robot36 => robot::ROBOT_36,
        }
    }

    /// The horizontal resolution in pixels.
    pub const fn image_width(&self) -> u32 {
        self.layout().width as u32
    }

    /// The vertical resolution in pixels.
    pub const fn image_height(&self) -> u32 {
        self.layout().height as u32
    }

    /// The tones sent before the image: the VOX tuning tones followed by the
    /// paper's "Calibration header with VIS code". Modes without a VIS code
    /// supply their own header here instead.
    ///
    /// "Note that all mode specifications begin immediately after the VIS
    /// stop bit."
    pub fn header_tones(&self) -> impl Iterator<Item = Tone> + '_ {
        (0..).map_while(move |index| self.header_tone(index))
    }

    /// The `index`-th header tone, or `None` past the end of the header.
    pub(crate) fn header_tone(&self, index: usize) -> Option<Tone> {
        let code = self
            .vis_code()
            .expect("every current mode transmits a VIS code");
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
            // "Calibration header with VIS code":
            8 => Some(Tone::new(LEADER_FREQUENCY, ms!(300))), // Leader tone
            9 => Some(Tone::new(SYNC_FREQUENCY, ms!(10))),    // break
            10 => Some(Tone::new(LEADER_FREQUENCY, ms!(300))), // Leader tone
            11 => Some(Tone::new(SYNC_FREQUENCY, VIS_BIT_DURATION)), // VIS start bit
            // "The seven-bit code is transmitted least-significant-bit first"
            12..=18 => Some(bit((code >> (index - 12)) & 1 == 1)),
            // "and uses 'even' parity."
            19 => Some(bit(code.count_ones() % 2 == 1)),
            20 => Some(Tone::new(SYNC_FREQUENCY, VIS_BIT_DURATION)), // VIS stop bit
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec::Vec;

    use super::*;
    use crate::synthesizer::Tone;
    use crate::{Hz, ms, us};

    #[test]
    fn header_tones_robot36() {
        assert_eq!(
            Mode::Robot36.header_tones().collect::<Vec<_>>(),
            std::vec![
                Tone::new(Hz!(1900), ms!(100)),
                Tone::new(Hz!(1500), ms!(100)),
                Tone::new(Hz!(1900), ms!(100)),
                Tone::new(Hz!(1500), ms!(100)),
                Tone::new(Hz!(2300), ms!(100)),
                Tone::new(Hz!(1500), ms!(100)),
                Tone::new(Hz!(2300), ms!(100)),
                Tone::new(Hz!(1500), ms!(100)),
                Tone::new(Hz!(1900), ms!(300)),
                Tone::new(Hz!(1200), ms!(10)),
                Tone::new(Hz!(1900), ms!(300)),
                Tone::new(Hz!(1200), ms!(30)),
                Tone::new(Hz!(1300), ms!(30)),
                Tone::new(Hz!(1300), ms!(30)),
                Tone::new(Hz!(1300), ms!(30)),
                Tone::new(Hz!(1100), ms!(30)),
                Tone::new(Hz!(1300), ms!(30)),
                Tone::new(Hz!(1300), ms!(30)),
                Tone::new(Hz!(1300), ms!(30)),
                Tone::new(Hz!(1100), ms!(30)),
                Tone::new(Hz!(1200), ms!(30)),
            ]
        )
    }

    #[test]
    fn vis_codes_round_trip() {
        for mode in Mode::ALL {
            if let Some(code) = mode.vis_code() {
                assert_eq!(Mode::from_vis_code(code), Some(mode));
                assert!(code < 128, "VIS codes are 7 bit");
            }
        }
    }

    /// Every sequence of a mode must be equally long — the decoder relies on
    /// the sync pulses being evenly spaced.
    #[test]
    fn sequences_are_equally_long() {
        for mode in Mode::ALL {
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
        assert_eq!(Mode::Robot36.layout().sequence_duration(), ms!(150));
    }
}
