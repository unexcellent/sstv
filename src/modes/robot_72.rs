//! Robot 72.
//!
//! Robot modes encode colour as luminance plus two colour differences, with
//! the colour differences scanned at half the luminance scan period. Robot 72
//! transmits both differences on every line.

use super::Mode;
use super::layout::{Channel, ColorMode, Layout, Step};
use crate::{ms, tone};

/// A 320x240 colour image in a 72 second transmission: 240 lines of 300ms each.
pub const ROBOT_72: Mode = Mode {
    name: "Robot72",
    vis_code: 12,
    starting_sync_pulse: false,
    layout: Layout {
        width: 320,
        height: 240,
        sequences: &[&SEQUENCE],
        lines_per_sequence: 1,
        color: ColorMode::Yuv,
    },
};

const SYNC_PULSE: Step = Step::Control(tone!(1200 Hz, 9 ms));
const SYNC_PORCH: Step = Step::Control(tone!(1500 Hz, 3 ms));
const PORCH: Step = Step::Control(tone!(1900 Hz, 1_500 us));

const SEQUENCE: [Step; 9] = [
    SYNC_PULSE,
    SYNC_PORCH,
    Step::Scan(Channel::Y, ms!(138)),
    Step::Control(tone!(1500 Hz, 4_500 us)), // even separator pulse
    PORCH,
    Step::Scan(Channel::RY, ms!(69)),
    Step::Control(tone!(2300 Hz, 4_500 us)), // odd separator pulse
    Step::Control(tone!(1500 Hz, 1_500 us)),
    Step::Scan(Channel::BY, ms!(69)),
];
