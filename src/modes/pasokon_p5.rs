//! Pasokon P5.
//!
//! High-resolution modes transmitting red, green and blue scans of every
//! line, with a porch after every scan. Sync pulse and porch lengths vary
//! with the sub-mode: they were chosen to divide evenly into standard RS232
//! clock rates.

use super::Mode;
use super::layout::{Channel, ColorMode, Layout, Step};
use crate::units::Duration;
use crate::{tone, us, vis_code};

/// A 640x496 colour image in a 305 second transmission: 496 lines of 614.065ms each.
pub const PASOKON_P5: Mode = Mode {
    name: "PasokonP5",
    vis_code: vis_code!(114),
    starting_sync_pulse: false,
    layout: Layout {
        width: 640,
        height: 496,
        sequences: &[&SEQUENCE],
        lines_per_sequence: 1,
        color: ColorMode::Rgb,
    },
};

const SYNC_PULSE: Step = Step::Control(tone!(1200 Hz, 7_813 us));
const PORCH: Step = Step::Control(tone!(1500 Hz, 1_563 us));
const SCAN: Duration = us!(200_000);

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
        assert_line_period(PASOKON_P5, crate::us!(614_065));
    }

    #[test]
    fn transmission_time_matches_the_paper() {
        assert_transmission_time(PASOKON_P5, 304.6);
    }

    #[test]
    fn vis_code_round_trips() {
        assert_vis_code_round_trips(PASOKON_P5);
    }
}
