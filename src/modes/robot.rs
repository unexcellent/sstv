//! "ROBOT 36 COLOR" — Dayton paper.
//!
//! Robot 36 uses Y, R-Y, B-Y colour encoding (Appendix B) and is "probably
//! the most complex of all SSTV modes": the R-Y colour information is
//! averaged over two lines and transmitted on even lines, the B-Y likewise on
//! odd lines, and the colour-difference scans have only half the period
//! (44ms) of the Y scan (88ms). Even lines use a 1500hz "separator" pulse,
//! odd lines 2300hz.

use super::layout::{Channel, ColorMode, Layout, Step};
use crate::{Hz, ms, us};

/// "(1) Sync pulse: 9.0ms 1200hz".
const SYNC_PULSE: Step = Step::tone(Hz!(1200), ms!(9));
/// "(2) Sync porch: 3.0ms 1500hz".
const SYNC_PORCH: Step = Step::tone(Hz!(1500), ms!(3));
/// "(5)/(11) Porch: 1.5ms 1900hz".
const PORCH: Step = Step::tone(Hz!(1900), us!(1_500));

/// TIMING SEQUENCE, steps (1)-(6): the even line of a pair.
const ROBOT_36_EVEN_SEQUENCE: [Step; 6] = [
    SYNC_PULSE,
    SYNC_PORCH,
    Step::scan(Channel::Y, ms!(88)),
    Step::tone(Hz!(1500), us!(4_500)), // "(4) 'Even' separator pulse: 4.5ms 1500hz"
    PORCH,
    Step::scan(Channel::RY, ms!(44)),
];

/// TIMING SEQUENCE, steps (7)-(12): the odd line of a pair.
const ROBOT_36_ODD_SEQUENCE: [Step; 6] = [
    SYNC_PULSE,
    SYNC_PORCH,
    Step::scan(Channel::Y, ms!(88)),
    Step::tone(Hz!(2300), us!(4_500)), // "(10) 'Odd' separator pulse: 4.5ms 2300hz"
    PORCH,
    Step::scan(Channel::BY, ms!(44)),
];

/// NUMBER OF LINES: 240. TRANSMISSION TIME: 36 seconds (150ms per line).
pub(crate) const ROBOT_36: Layout = Layout {
    width: 320,
    height: 240,
    start: &[],
    sequences: &[&ROBOT_36_EVEN_SEQUENCE, &ROBOT_36_ODD_SEQUENCE],
    lines_per_sequence: 1,
    color: ColorMode::YuvAveragedPair,
};
