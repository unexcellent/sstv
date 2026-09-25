//! PD 160.
//!
//! PD modes transmit two image lines per sequence: the first line's
//! luminance, the red and blue colour differences averaged over both lines,
//! and then the second line's luminance. The sub-modes differ only in scan
//! time and resolution.

use super::Mode;
use super::layout::{Channel, ColorMode, Layout, Step};
use crate::units::Duration;
use crate::{Hz, ms, us};

/// A 512x400 colour image in a 161 second transmission: 200 line pairs of 804.416ms each.
pub const PD_160: Mode = Mode {
    name: "Pd160",
    vis_code: 98,
    starting_sync_pulse: false,
    layout: Layout {
        width: 512,
        height: 400,
        sequences: &[&SEQUENCE],
        lines_per_sequence: 2,
        color: ColorMode::YuvSharedPair,
    },
};

const SYNC_PULSE: Step = Step::control(Hz!(1200), ms!(20));
const PORCH: Step = Step::control(Hz!(1500), us!(2_080));
const SCAN: Duration = us!(195_584);

const SEQUENCE: [Step; 6] = [
    SYNC_PULSE,
    PORCH,
    Step::scan(Channel::Y, SCAN),
    Step::scan(Channel::RY, SCAN),
    Step::scan(Channel::BY, SCAN),
    Step::scan(Channel::YSecond, SCAN),
];
