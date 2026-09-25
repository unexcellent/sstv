//! Robot 72.
//!
//! Robot modes encode colour as luminance plus two colour differences, with
//! the colour differences scanned at half the luminance scan period. Robot 72
//! transmits both differences on every line.

use super::layout::{Channel, ColorMode, Layout, Step};
use crate::{Hz, ms, us};

const SYNC_PULSE: Step = Step::control(Hz!(1200), ms!(9));
const SYNC_PORCH: Step = Step::control(Hz!(1500), ms!(3));
const PORCH: Step = Step::control(Hz!(1900), us!(1_500));

const SEQUENCE: [Step; 9] = [
    SYNC_PULSE,
    SYNC_PORCH,
    Step::scan(Channel::Y, ms!(138)),
    Step::control(Hz!(1500), us!(4_500)), // even separator pulse
    PORCH,
    Step::scan(Channel::RY, ms!(69)),
    Step::control(Hz!(2300), us!(4_500)), // odd separator pulse
    Step::control(Hz!(1500), us!(1_500)),
    Step::scan(Channel::BY, ms!(69)),
];

/// 240 lines of 300ms each: a 72 second transmission.
pub const ROBOT_72: Layout = Layout {
    width: 320,
    height: 240,
    sequences: &[&SEQUENCE],
    lines_per_sequence: 1,
    color: ColorMode::Yuv,
};
