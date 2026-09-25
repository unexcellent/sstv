//! Robot 36.
//!
//! Robot modes encode colour as luminance plus two colour differences, with
//! the colour differences scanned at half the luminance scan period. Robot 36
//! halves the colour information further by averaging it over a line pair:
//! even lines carry the red difference, odd lines the blue one, with a
//! separator pulse marking the parity (1500hz on even lines, 2300hz on odd
//! lines).

use super::Mode;
use super::layout::{Channel, ColorMode, Layout, Step};
use crate::{Hz, ms, us};

/// A 320x240 colour image in a 36 second transmission: 240 lines of 150ms each.
pub const ROBOT_36: Mode = Mode {
    name: "Robot36",
    vis_code: 8,
    starting_sync_pulse: false,
    layout: Layout {
        width: 320,
        height: 240,
        sequences: &[&EVEN_SEQUENCE, &ODD_SEQUENCE],
        lines_per_sequence: 1,
        color: ColorMode::YuvAveragedPair,
    },
};

const SYNC_PULSE: Step = Step::control(Hz!(1200), ms!(9));
const SYNC_PORCH: Step = Step::control(Hz!(1500), ms!(3));
const PORCH: Step = Step::control(Hz!(1900), us!(1_500));

/// The even line of a pair.
const EVEN_SEQUENCE: [Step; 6] = [
    SYNC_PULSE,
    SYNC_PORCH,
    Step::scan(Channel::Y, ms!(88)),
    Step::control(Hz!(1500), us!(4_500)), // even-line separator pulse
    PORCH,
    Step::scan(Channel::RY, ms!(44)),
];

/// The odd line of a pair.
const ODD_SEQUENCE: [Step; 6] = [
    SYNC_PULSE,
    SYNC_PORCH,
    Step::scan(Channel::Y, ms!(88)),
    Step::control(Hz!(2300), us!(4_500)), // odd-line separator pulse
    PORCH,
    Step::scan(Channel::BY, ms!(44)),
];
