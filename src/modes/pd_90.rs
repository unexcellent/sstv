//! PD 90.
//!
//! PD modes transmit two image lines per sequence: the first line's
//! luminance, the red and blue colour differences averaged over both lines,
//! and then the second line's luminance. The sub-modes differ only in scan
//! time and resolution.

use super::Mode;
use super::layout::{Channel, ColorMode, Layout, Step};
use crate::units::Duration;
use crate::{tone, us, vis_code};

/// A 320x256 colour image in a 90 second transmission: 128 line pairs of 703.04ms each.
pub const PD_90: Mode = Mode {
    name: "Pd90",
    vis_code: vis_code!(99),
    starting_sync_pulse: false,
    layout: Layout {
        resolution: (320, 256),
        sequences: &[&SEQUENCE],
        lines_per_sequence: 2,
        color: ColorMode::YuvSharedPair,
    },
};

const SYNC_PULSE: Step = Step::Control(tone!(1200 Hz, 20 ms));
const PORCH: Step = Step::Control(tone!(1500 Hz, 2_080 us));
const SCAN: Duration = us!(170_240);

const SEQUENCE: [Step; 6] = [
    SYNC_PULSE,
    PORCH,
    Step::Scan(Channel::Y, SCAN),
    Step::Scan(Channel::RY, SCAN),
    Step::Scan(Channel::BY, SCAN),
    Step::Scan(Channel::YSecond, SCAN),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::testing::{
        assert_line_period, assert_mode_can_be_constructed_from_vis_code, assert_transmission_time,
    };

    #[test]
    fn line_period_matches_the_paper() {
        assert_line_period(PD_90, crate::us!(703_040));
    }

    #[test]
    fn transmission_time_matches_the_paper() {
        assert_transmission_time(PD_90, 90.0);
    }

    #[test]
    fn mode_constructed_from_vis_code() {
        assert_mode_can_be_constructed_from_vis_code(PD_90);
    }
}
