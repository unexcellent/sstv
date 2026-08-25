//! The SSTV modes specified in the Dayton paper — JL Barber (N7CXI),
//! "Proposal for SSTV Mode Specifications", presented at the Dayton SSTV
//! forum, 20 May 2000.
//!
//! Each mode family lives in its own module mirroring a chapter of the paper,
//! transcribing its timing table into a [`Layout`]. Everything
//! shared between modes — the frequency range, the calibration header and the
//! VIS code — is defined here, as in the paper's common sections.

pub(crate) mod layout;

mod martin;
mod pasokon;
mod pd;
mod robot;
mod scottie;
mod wrasse;

use crate::synthesizer::Tone;
use crate::units::{Duration, Frequency};
use crate::{Hz, ms};
use layout::Layout;

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
    BLACK_FREQUENCY + (WHITE_FREQUENCY - BLACK_FREQUENCY) * value as u32 / 255
}

/// Tuning (VOX) tones customarily sent ahead of the calibration header to
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Mode {
    /// A 320x256 colour image in a 110 second transmission.
    Scottie1,
    /// A 320x256 colour image in a 71 second transmission.
    Scottie2,
    /// A 320x256 colour image in a 269 second transmission.
    ScottieDx,
    /// A 320x256 colour image in a 114 second transmission.
    Martin1,
    /// A 320x256 colour image in a 58 second transmission.
    Martin2,
    /// A 320x240 colour image in a 36 second transmission.
    Robot36,
    /// A 320x240 colour image in a 72 second transmission.
    Robot72,
    /// A 320x256 colour image in a 182 second transmission.
    WrasseSc2180,
    /// A 640x496 colour image in a 203 second transmission.
    PasokonP3,
    /// A 640x496 colour image in a 305 second transmission.
    PasokonP5,
    /// A 640x496 colour image in a 406 second transmission.
    PasokonP7,
    /// A 320x256 colour image in a 50 second transmission.
    Pd50,
    /// A 320x256 colour image in a 90 second transmission.
    Pd90,
    /// A 640x496 colour image in a 126 second transmission.
    Pd120,
    /// A 512x400 colour image in a 161 second transmission.
    Pd160,
    /// A 640x496 colour image in a 187 second transmission.
    Pd180,
    /// A 640x496 colour image in a 248 second transmission.
    Pd240,
    /// An 800x616 colour image in a 289 second transmission.
    Pd290,
}

impl Mode {
    /// Every supported mode, in the paper's order.
    pub const ALL: [Mode; 18] = [
        Mode::Scottie1,
        Mode::Scottie2,
        Mode::ScottieDx,
        Mode::Martin1,
        Mode::Martin2,
        Mode::Robot36,
        Mode::Robot72,
        Mode::WrasseSc2180,
        Mode::PasokonP3,
        Mode::PasokonP5,
        Mode::PasokonP7,
        Mode::Pd50,
        Mode::Pd90,
        Mode::Pd120,
        Mode::Pd160,
        Mode::Pd180,
        Mode::Pd240,
        Mode::Pd290,
    ];

    /// The mode's 7-bit VIS code, identifying it to a receiving system.
    pub const fn vis_code(&self) -> u8 {
        match self {
            Mode::Scottie1 => 60,
            Mode::Scottie2 => 56,
            Mode::ScottieDx => 76,
            Mode::Martin1 => 44,
            Mode::Martin2 => 40,
            Mode::Robot36 => 8,
            Mode::Robot72 => 12,
            Mode::WrasseSc2180 => 55,
            Mode::PasokonP3 => 113,
            Mode::PasokonP5 => 114,
            Mode::PasokonP7 => 115,
            Mode::Pd50 => 93,
            Mode::Pd90 => 99,
            Mode::Pd120 => 95,
            Mode::Pd160 => 98,
            Mode::Pd180 => 96,
            Mode::Pd240 => 97,
            Mode::Pd290 => 94,
        }
    }

    /// Look up a mode by its 7-bit VIS code.
    pub const fn from_vis_code(code: u8) -> Option<Mode> {
        match code {
            60 => Some(Mode::Scottie1),
            56 => Some(Mode::Scottie2),
            76 => Some(Mode::ScottieDx),
            44 => Some(Mode::Martin1),
            40 => Some(Mode::Martin2),
            8 => Some(Mode::Robot36),
            12 => Some(Mode::Robot72),
            55 => Some(Mode::WrasseSc2180),
            113 => Some(Mode::PasokonP3),
            114 => Some(Mode::PasokonP5),
            115 => Some(Mode::PasokonP7),
            93 => Some(Mode::Pd50),
            99 => Some(Mode::Pd90),
            95 => Some(Mode::Pd120),
            98 => Some(Mode::Pd160),
            96 => Some(Mode::Pd180),
            97 => Some(Mode::Pd240),
            94 => Some(Mode::Pd290),
            _ => None,
        }
    }

    /// The mode's scanline structure as specified by its timing-sequence
    /// table in the paper.
    pub(crate) const fn layout(&self) -> Layout {
        match self {
            Mode::Scottie1 => scottie::SCOTTIE_1,
            Mode::Scottie2 => scottie::SCOTTIE_2,
            Mode::ScottieDx => scottie::SCOTTIE_DX,
            Mode::Martin1 => martin::MARTIN_1,
            Mode::Martin2 => martin::MARTIN_2,
            Mode::Robot36 => robot::ROBOT_36,
            Mode::Robot72 => robot::ROBOT_72,
            Mode::WrasseSc2180 => wrasse::WRASSE_SC2_180,
            Mode::PasokonP3 => pasokon::PASOKON_P3,
            Mode::PasokonP5 => pasokon::PASOKON_P5,
            Mode::PasokonP7 => pasokon::PASOKON_P7,
            Mode::Pd50 => pd::PD_50,
            Mode::Pd90 => pd::PD_90,
            Mode::Pd120 => pd::PD_120,
            Mode::Pd160 => pd::PD_160,
            Mode::Pd180 => pd::PD_180,
            Mode::Pd240 => pd::PD_240,
            Mode::Pd290 => pd::PD_290,
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

    /// Whether the mode transmits one extra sync pulse between the header and
    /// the first line. Only Scottie modes do.
    const fn has_starting_sync_pulse(&self) -> bool {
        matches!(self, Mode::Scottie1 | Mode::Scottie2 | Mode::ScottieDx)
    }

    /// The tones sent before the image: the VOX tuning tones, the calibration
    /// header carrying the VIS code, and the starting sync pulse for modes
    /// that transmit one. The image data begins immediately after the last
    /// header tone.
    pub fn header_tones(&self) -> impl Iterator<Item = Tone> + '_ {
        (0..).map_while(move |index| self.header_tone(index))
    }

    /// The `index`-th header tone, or `None` past the end of the header.
    pub(crate) fn header_tone(&self, index: usize) -> Option<Tone> {
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
            8 => Some(Tone::new(LEADER_FREQUENCY, ms!(300))),
            9 => Some(Tone::new(SYNC_FREQUENCY, ms!(10))), // break
            10 => Some(Tone::new(LEADER_FREQUENCY, ms!(300))),
            11 => Some(Tone::new(SYNC_FREQUENCY, VIS_BIT_DURATION)), // start bit
            12..=18 => Some(bit((code >> (index - 12)) & 1 == 1)), // code bits, least significant first
            19 => Some(bit(code.count_ones() % 2 == 1)),           // even parity
            20 => Some(Tone::new(SYNC_FREQUENCY, VIS_BIT_DURATION)), // stop bit
            21 if self.has_starting_sync_pulse() => {
                Some(Tone::new(SYNC_FREQUENCY, self.layout().sync_pulse().1))
            }
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
            let code = mode.vis_code();
            assert_eq!(Mode::from_vis_code(code), Some(mode));
            assert!(code < 128, "VIS codes are 7 bit");
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

    /// The paper publishes each mode's total transmission time (excluding the
    /// header) alongside the per-step timings. Summing our transcribed steps
    /// over all lines must reproduce those times, which catches transcription
    /// mistakes in any single step.
    #[test]
    fn transmission_times_match_paper() {
        let expected_seconds = [
            (Mode::Scottie1, 109.6),
            (Mode::Scottie2, 71.1),
            (Mode::ScottieDx, 268.9),
            (Mode::Martin1, 114.3),
            (Mode::Martin2, 58.06),
            (Mode::Robot36, 36.0),
            (Mode::Robot72, 72.0),
            (Mode::WrasseSc2180, 182.0),
            (Mode::PasokonP3, 203.0),
            (Mode::PasokonP5, 304.6),
            (Mode::PasokonP7, 406.1),
            (Mode::Pd50, 49.7),
            (Mode::Pd90, 90.0),
            (Mode::Pd120, 126.1),
            (Mode::Pd160, 160.9),
            (Mode::Pd180, 187.1),
            (Mode::Pd240, 248.0),
            (Mode::Pd290, 288.7),
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
