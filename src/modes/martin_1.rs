//! Martin 1.
//!
//! Martin modes transmit green, blue and red scans of every line, with the
//! sync pulse at the line break and a short separator pulse after each scan.

use super::Mode;
use super::layout::{Channel, ColorMode, Layout, Step};
use crate::units::Duration;
use crate::{tone, us};

/// A 320x256 colour image in a 114 second transmission: 256 lines of 446.446ms each.
pub const MARTIN_1: Mode = Mode {
    name: "Martin1",
    vis_code: 44,
    starting_sync_pulse: false,
    layout: Layout {
        width: 320,
        height: 256,
        sequences: &[&SEQUENCE],
        lines_per_sequence: 1,
        color: ColorMode::Rgb,
    },
};

const SYNC_PULSE: Step = Step::Control(tone!(1200 Hz, 4_862 us));
const SYNC_PORCH: Step = Step::Control(tone!(1500 Hz, 572 us));
const SEPARATOR_PULSE: Step = Step::Control(tone!(1500 Hz, 572 us));
const SCAN: Duration = us!(146_432);

const SEQUENCE: [Step; 8] = [
    SYNC_PULSE,
    SYNC_PORCH,
    Step::Scan(Channel::Green, SCAN),
    SEPARATOR_PULSE,
    Step::Scan(Channel::Blue, SCAN),
    SEPARATOR_PULSE,
    Step::Scan(Channel::Red, SCAN),
    SEPARATOR_PULSE,
];
