//! PD 240.
//!
//! PD modes transmit two image lines per sequence: the first line's
//! luminance, the red and blue colour differences averaged over both lines,
//! and then the second line's luminance. The sub-modes differ only in scan
//! time and resolution.

use super::Mode;
use super::layout::{Channel, ColorMode, Layout, Step};
use crate::units::Duration;
use crate::{tone, us, vis_code};

/// A 640x496 colour image in a 248 second transmission: 248 line pairs of 1000ms each.
pub const PD_240: Mode = Mode {
    name: "Pd240",
    vis_code: vis_code!(97),
    starting_sync_pulse: false,
    layout: Layout {
        width: 640,
        height: 496,
        sequences: &[&SEQUENCE],
        lines_per_sequence: 2,
        color: ColorMode::YuvSharedPair,
    },
};

const SYNC_PULSE: Step = Step::Control(tone!(1200 Hz, 20 ms));
const PORCH: Step = Step::Control(tone!(1500 Hz, 2_080 us));
const SCAN: Duration = us!(244_480);

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
    use crate::modes::testing::{assert_line_period, assert_transmission_time};

    #[test]
    fn line_period_matches_the_paper() {
        assert_line_period(PD_240, crate::ms!(1_000));
    }

    #[test]
    fn transmission_time_matches_the_paper() {
        assert_transmission_time(PD_240, 248.0);
    }

    #[test]
    fn mode_constructed_from_vis_code() {
        assert_eq!(Mode::try_from(vis_code!(97)), Ok(PD_240));
    }
}
