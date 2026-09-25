//! Scottie 2.
//!
//! Scottie modes transmit green, blue and red scans of every line. They are
//! unusual in two ways: the sync pulse sits between the blue and the red scan
//! instead of at the line break, and a single extra sync pulse precedes the
//! very first line (emitted here as the last header tone).

use super::layout::{Channel, ColorMode, Layout, Step};
use crate::units::Duration;
use crate::{Hz, ms, us};

const SYNC_PULSE: Step = Step::control(Hz!(1200), ms!(9));
const SYNC_PORCH: Step = Step::control(Hz!(1500), us!(1_500));
const SEPARATOR_PULSE: Step = Step::control(Hz!(1500), us!(1_500));
const SCAN: Duration = us!(88_064);

const SEQUENCE: [Step; 7] = [
    SEPARATOR_PULSE,
    Step::scan(Channel::Green, SCAN),
    SEPARATOR_PULSE,
    Step::scan(Channel::Blue, SCAN),
    SYNC_PULSE,
    SYNC_PORCH,
    Step::scan(Channel::Red, SCAN),
];

/// 256 lines of 277.692ms each: a 71 second transmission.
pub const SCOTTIE_2: Layout = Layout {
    width: 320,
    height: 256,
    sequences: &[&SEQUENCE],
    lines_per_sequence: 1,
    color: ColorMode::Rgb,
};
