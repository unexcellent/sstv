//! Martin 1.
//!
//! Martin modes transmit green, blue and red scans of every line, with the
//! sync pulse at the line break and a short separator pulse after each scan.

use super::layout::{Channel, ColorMode, Layout, Step};
use crate::units::Duration;
use crate::{Hz, us};

const SYNC_PULSE: Step = Step::control(Hz!(1200), us!(4_862));
const SYNC_PORCH: Step = Step::control(Hz!(1500), us!(572));
const SEPARATOR_PULSE: Step = Step::control(Hz!(1500), us!(572));
const SCAN: Duration = us!(146_432);

const SEQUENCE: [Step; 8] = [
    SYNC_PULSE,
    SYNC_PORCH,
    Step::scan(Channel::Green, SCAN),
    SEPARATOR_PULSE,
    Step::scan(Channel::Blue, SCAN),
    SEPARATOR_PULSE,
    Step::scan(Channel::Red, SCAN),
    SEPARATOR_PULSE,
];

/// 256 lines of 446.446ms each: a 114 second transmission.
pub const MARTIN_1: Layout = Layout {
    width: 320,
    height: 256,
    sequences: &[&SEQUENCE],
    lines_per_sequence: 1,
    color: ColorMode::Rgb,
};
