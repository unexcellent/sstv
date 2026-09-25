//! Wrasse SC2-180.
//!
//! The simplest colour mode: red, green and blue scans of every line follow
//! the sync pulse and porch directly, without separators.

use super::Mode;
use super::layout::{Channel, ColorMode, Layout, Step};
use crate::{ms, tone};

/// A 320x256 colour image in a 182 second transmission: 256 lines of 711.0225ms each.
pub const WRASSE_SC2_180: Mode = Mode {
    name: "WrasseSc2180",
    vis_code: 55,
    starting_sync_pulse: false,
    layout: Layout {
        width: 320,
        height: 256,
        sequences: &[&SEQUENCE],
        lines_per_sequence: 1,
        color: ColorMode::Rgb,
    },
};

const SEQUENCE: [Step; 5] = [
    Step::Control(tone!(1200 Hz, 5_522_500 ns)),
    Step::Control(tone!(1500 Hz, 500 us)),
    Step::Scan(Channel::Red, ms!(235)),
    Step::Scan(Channel::Green, ms!(235)),
    Step::Scan(Channel::Blue, ms!(235)),
];
