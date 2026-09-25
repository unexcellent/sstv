//! PD 290.
//!
//! PD modes transmit two image lines per sequence: the first line's
//! luminance, the red and blue colour differences averaged over both lines,
//! and then the second line's luminance. The sub-modes differ only in scan
//! time and resolution.

use super::Mode;
use super::layout::{Channel, ColorMode, Layout, Step};
use crate::units::Duration;
use crate::{Hz, ms, us};

/// An 800x616 colour image in a 289 second transmission: 308 line pairs of 937.28ms each.
pub const PD_290: Mode = Mode {
    name: "Pd290",
    vis_code: 94,
    starting_sync_pulse: false,
    layout: Layout {
        width: 800,
        height: 616,
        sequences: &[&SEQUENCE],
        lines_per_sequence: 2,
        color: ColorMode::YuvSharedPair,
    },
};

const SYNC_PULSE: Step = Step::control(Hz!(1200), ms!(20));
const PORCH: Step = Step::control(Hz!(1500), us!(2_080));
const SCAN: Duration = us!(228_800);

const SEQUENCE: [Step; 6] = [
    SYNC_PULSE,
    PORCH,
    Step::scan(Channel::Y, SCAN),
    Step::scan(Channel::RY, SCAN),
    Step::scan(Channel::BY, SCAN),
    Step::scan(Channel::YSecond, SCAN),
];
