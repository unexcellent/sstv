//! Scottie 1.
//!
//! Scottie modes transmit green, blue and red scans of every line. They are
//! unusual in two ways: the sync pulse sits between the blue and the red scan
//! instead of at the line break, and a single extra sync pulse precedes the
//! very first line (emitted here as the last header tone).

use super::Mode;
use super::layout::{Channel, ColorMode, Layout, Step};
use crate::units::Duration;
use crate::{tone, us};

/// A 320x256 colour image in a 110 second transmission: 256 lines of 428.22ms each.
pub const SCOTTIE_1: Mode = Mode {
    name: "Scottie1",
    vis_code: 60,
    starting_sync_pulse: true,
    layout: Layout {
        width: 320,
        height: 256,
        sequences: &[&SEQUENCE],
        lines_per_sequence: 1,
        color: ColorMode::Rgb,
    },
};

const SYNC_PULSE: Step = Step::Control(tone!(1200 Hz, 9 ms));
const SYNC_PORCH: Step = Step::Control(tone!(1500 Hz, 1_500 us));
const SEPARATOR_PULSE: Step = Step::Control(tone!(1500 Hz, 1_500 us));
const SCAN: Duration = us!(138_240);

const SEQUENCE: [Step; 7] = [
    SEPARATOR_PULSE,
    Step::Scan(Channel::Green, SCAN),
    SEPARATOR_PULSE,
    Step::Scan(Channel::Blue, SCAN),
    SYNC_PULSE,
    SYNC_PORCH,
    Step::Scan(Channel::Red, SCAN),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::testing::{
        assert_line_period, assert_transmission_time, assert_vis_code_round_trips,
    };

    #[test]
    fn line_period_matches_the_paper() {
        assert_line_period(SCOTTIE_1, crate::us!(428_220));
    }

    #[test]
    fn transmission_time_matches_the_paper() {
        assert_transmission_time(SCOTTIE_1, 109.6);
    }

    #[test]
    fn vis_code_round_trips() {
        assert_vis_code_round_trips(SCOTTIE_1);
    }
}
