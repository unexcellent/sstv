//! Martin 2.
//!
//! Martin modes transmit green, blue and red scans of every line, with the
//! sync pulse at the line break and a short separator pulse after each scan.

use super::Mode;
use super::layout::{Channel, ColorMode, Layout, Step};
use crate::units::Duration;
use crate::{Hz, us};

/// A 320x256 colour image in a 58 second transmission: 256 lines of 226.798ms each.
pub const MARTIN_2: Mode = Mode {
    name: "Martin2",
    vis_code: 40,
    starting_sync_pulse: false,
    layout: Layout {
        width: 320,
        height: 256,
        sequences: &[&SEQUENCE],
        lines_per_sequence: 1,
        color: ColorMode::Rgb,
    },
};

const SYNC_PULSE: Step = Step::control(Hz!(1200), us!(4_862));
const SYNC_PORCH: Step = Step::control(Hz!(1500), us!(572));
const SEPARATOR_PULSE: Step = Step::control(Hz!(1500), us!(572));
const SCAN: Duration = us!(73_216);

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
