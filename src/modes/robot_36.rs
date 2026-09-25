//! Robot 36.
//!
//! Robot modes encode colour as luminance plus two colour differences, with
//! the colour differences scanned at half the luminance scan period. Robot 36
//! halves the colour information further by averaging it over a line pair:
//! even lines carry the red difference, odd lines the blue one, with a
//! separator pulse marking the parity (1500hz on even lines, 2300hz on odd
//! lines).

use super::Mode;
use super::layout::{Channel, ColorMode, Layout, Step};
use crate::{ms, tone, vis_code};

/// A 320x240 colour image in a 36 second transmission: 240 lines of 150ms each.
pub const ROBOT_36: Mode = Mode {
    name: "Robot36",
    vis_code: vis_code!(8),
    starting_sync_pulse: false,
    layout: Layout {
        width: 320,
        height: 240,
        sequences: &[&EVEN_SEQUENCE, &ODD_SEQUENCE],
        lines_per_sequence: 1,
        color: ColorMode::YuvAveragedPair,
    },
};

const SYNC_PULSE: Step = Step::Control(tone!(1200 Hz, 9 ms));
const SYNC_PORCH: Step = Step::Control(tone!(1500 Hz, 3 ms));
const PORCH: Step = Step::Control(tone!(1900 Hz, 1_500 us));

/// The even line of a pair.
const EVEN_SEQUENCE: [Step; 6] = [
    SYNC_PULSE,
    SYNC_PORCH,
    Step::Scan(Channel::Y, ms!(88)),
    Step::Control(tone!(1500 Hz, 4_500 us)), // even-line separator pulse
    PORCH,
    Step::Scan(Channel::RY, ms!(44)),
];

/// The odd line of a pair.
const ODD_SEQUENCE: [Step; 6] = [
    SYNC_PULSE,
    SYNC_PORCH,
    Step::Scan(Channel::Y, ms!(88)),
    Step::Control(tone!(2300 Hz, 4_500 us)), // odd-line separator pulse
    PORCH,
    Step::Scan(Channel::BY, ms!(44)),
];

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec::Vec;

    use super::*;
    use crate::modes::testing::{assert_line_period, assert_transmission_time};

    #[test]
    fn line_period_matches_the_paper() {
        assert_line_period(ROBOT_36, crate::ms!(150));
    }

    #[test]
    fn transmission_time_matches_the_paper() {
        assert_transmission_time(ROBOT_36, 36.0);
    }

    #[test]
    fn mode_constructed_from_vis_code() {
        assert_eq!(Mode::try_from(vis_code!(8)), Ok(ROBOT_36));
    }

    #[test]
    fn header_tones_match_the_vis_encoding() {
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
}
