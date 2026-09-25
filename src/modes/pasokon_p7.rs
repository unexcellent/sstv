//! Pasokon P7.
//!
//! High-resolution modes transmitting red, green and blue scans of every
//! line, with a porch after every scan. Sync pulse and porch lengths vary
//! with the sub-mode: they were chosen to divide evenly into standard RS232
//! clock rates.

use super::Mode;
use super::layout::{Channel, ColorMode, Layout, Step};
use crate::units::Duration;
use crate::{tone, us};

/// A 640x496 colour image in a 406 second transmission: 496 lines of 818.747ms each.
pub const PASOKON_P7: Mode = Mode {
    name: "PasokonP7",
    vis_code: 115,
    starting_sync_pulse: false,
    layout: Layout {
        width: 640,
        height: 496,
        sequences: &[&SEQUENCE],
        lines_per_sequence: 1,
        color: ColorMode::Rgb,
    },
};

const SYNC_PULSE: Step = Step::Control(tone!(1200 Hz, 10_417 us));
const PORCH: Step = Step::Control(tone!(1500 Hz, 2_083 us));
const SCAN: Duration = us!(266_666);

const SEQUENCE: [Step; 8] = [
    SYNC_PULSE,
    PORCH,
    Step::Scan(Channel::Red, SCAN),
    PORCH,
    Step::Scan(Channel::Green, SCAN),
    PORCH,
    Step::Scan(Channel::Blue, SCAN),
    PORCH,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::testing::{
        assert_line_period, assert_transmission_time, assert_vis_code_round_trips,
    };

    #[test]
    fn line_period_matches_the_paper() {
        assert_line_period(PASOKON_P7, crate::us!(818_747));
    }

    #[test]
    fn transmission_time_matches_the_paper() {
        assert_transmission_time(PASOKON_P7, 406.1);
    }

    #[test]
    fn vis_code_round_trips() {
        assert_vis_code_round_trips(PASOKON_P7);
    }
}
