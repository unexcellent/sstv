//! Pasokon P3.
//!
//! High-resolution modes transmitting red, green and blue scans of every
//! line, with a porch after every scan. Sync pulse and porch lengths vary
//! with the sub-mode: they were chosen to divide evenly into standard RS232
//! clock rates.

use super::layout::{Channel, ColorMode, Layout, Step};
use crate::units::Duration;
use crate::{Hz, us};

const SYNC_PULSE: Step = Step::control(Hz!(1200), us!(5_208));
const PORCH: Step = Step::control(Hz!(1500), us!(1_042));
const SCAN: Duration = us!(133_333);

const SEQUENCE: [Step; 8] = [
    SYNC_PULSE,
    PORCH,
    Step::scan(Channel::Red, SCAN),
    PORCH,
    Step::scan(Channel::Green, SCAN),
    PORCH,
    Step::scan(Channel::Blue, SCAN),
    PORCH,
];

/// 496 lines of 409.375ms each: a 203 second transmission.
pub const PASOKON_P3: Layout = Layout {
    width: 640,
    height: 496,
    sequences: &[&SEQUENCE],
    lines_per_sequence: 1,
    color: ColorMode::Rgb,
};
