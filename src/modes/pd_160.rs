//! PD 160.
//!
//! PD modes transmit two image lines per sequence: the first line's
//! luminance, the red and blue colour differences averaged over both lines,
//! and then the second line's luminance. The sub-modes differ only in scan
//! time and resolution.

use super::Mode;
use super::layout::{Channel, ColorMode, Layout, Step};
use crate::units::Duration;
use crate::{tone, us, vis_code};

/// A 512x400 colour image in a 161 second transmission: 200 line pairs of 804.416ms each.
pub const PD_160: Mode = Mode {
    name: "Pd160",
    vis_code: vis_code!(98),
    starting_sync_pulse: false,
    layout: Layout {
        width: 512,
        height: 400,
        sequences: &[&SEQUENCE],
        lines_per_sequence: 2,
        color: ColorMode::YuvSharedPair,
    },
};

const SYNC_PULSE: Step = Step::Control(tone!(1200 Hz, 20 ms));
const PORCH: Step = Step::Control(tone!(1500 Hz, 2_080 us));
const SCAN: Duration = us!(195_584);

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
        assert_line_period, assert_transmission_time, assert_vis_code_round_trips,
    };

    #[test]
    fn line_period_matches_the_paper() {
        assert_line_period(PD_160, crate::us!(804_416));
    }

    #[test]
    fn transmission_time_matches_the_paper() {
        assert_transmission_time(PD_160, 160.9);
    }

    #[test]
    fn vis_code_round_trips() {
        assert_vis_code_round_trips(PD_160);
    }
}
