//! PD 120.
//!
//! PD modes transmit two image lines per sequence: the first line's
//! luminance, the red and blue colour differences averaged over both lines,
//! and then the second line's luminance. The sub-modes differ only in scan
//! time and resolution.

use super::layout::{Channel, ColorMode, Layout, Step};
use crate::units::Duration;
use crate::{Hz, ms, us};

const SYNC_PULSE: Step = Step::control(Hz!(1200), ms!(20));
const PORCH: Step = Step::control(Hz!(1500), us!(2_080));
const SCAN: Duration = us!(121_600);

const SEQUENCE: [Step; 6] = [
    SYNC_PULSE,
    PORCH,
    Step::scan(Channel::Y, SCAN),
    Step::scan(Channel::RY, SCAN),
    Step::scan(Channel::BY, SCAN),
    Step::scan(Channel::YSecond, SCAN),
];

/// 248 line pairs of 508.48ms each: a 126 second transmission.
pub const PD_120: Layout = Layout {
    width: 640,
    height: 496,
    sequences: &[&SEQUENCE],
    lines_per_sequence: 2,
    color: ColorMode::YuvSharedPair,
};
