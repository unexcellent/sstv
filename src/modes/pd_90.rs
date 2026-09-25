//! PD 90.
//!
//! PD modes transmit two image lines per sequence: the first line's
//! luminance, the red and blue colour differences averaged over both lines,
//! and then the second line's luminance. The sub-modes differ only in scan
//! time and resolution.

use super::Mode;
use super::layout::{Channel, ColorMode, Layout, Step};
use crate::units::Duration;
use crate::{Hz, ms, us};

/// A 320x256 colour image in a 90 second transmission: 128 line pairs of 703.04ms each.
pub const PD_90: Mode = Mode {
    name: "Pd90",
    vis_code: 99,
    starting_sync_pulse: false,
    layout: Layout {
        width: 320,
        height: 256,
        sequences: &[&SEQUENCE],
        lines_per_sequence: 2,
        color: ColorMode::YuvSharedPair,
    },
};

const SYNC_PULSE: Step = Step::control(Hz!(1200), ms!(20));
const PORCH: Step = Step::control(Hz!(1500), us!(2_080));
const SCAN: Duration = us!(170_240);

const SEQUENCE: [Step; 6] = [
    SYNC_PULSE,
    PORCH,
    Step::scan(Channel::Y, SCAN),
    Step::scan(Channel::RY, SCAN),
    Step::scan(Channel::BY, SCAN),
    Step::scan(Channel::YSecond, SCAN),
];
