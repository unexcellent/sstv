//! Wrasse SC2-180.
//!
//! The simplest colour mode: red, green and blue scans of every line follow
//! the sync pulse and porch directly, without separators.

use super::layout::{Channel, ColorMode, Layout, Step};
use crate::{Hz, ms, ns, us};

const SEQUENCE: [Step; 5] = [
    Step::control(Hz!(1200), ns!(5_522_500)),
    Step::control(Hz!(1500), us!(500)),
    Step::scan(Channel::Red, ms!(235)),
    Step::scan(Channel::Green, ms!(235)),
    Step::scan(Channel::Blue, ms!(235)),
];

/// 256 lines of 711.0225ms each: a 182 second transmission.
pub const WRASSE_SC2_180: Layout = Layout {
    width: 320,
    height: 256,
    sequences: &[&SEQUENCE],
    lines_per_sequence: 1,
    color: ColorMode::Rgb,
};
