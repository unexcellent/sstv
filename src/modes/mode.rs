use crate::units::{Duration, Frequency};
use crate::{Hz, ms};

pub trait Mode {
    const SYNC: Frequency = Hz!(1200);
    const BLACK: Frequency = Hz!(1500);
    const WHITE: Frequency = Hz!(2300);
    const BINARY_0: Frequency = Hz!(1300);
    const BINARY_1: Frequency = Hz!(1100);
    const BREAK: Frequency = Hz!(1200);
    const SEPARATOR: Frequency = Hz!(1900);
    const BIT_DURATION: Duration = ms!(30);

    const IDENTIFICATION: u8;
    const IMAGE_WIDTH: u16;
    const IMAGE_HEIGHT: u16;
    const SYNC_DURATION: Duration = ms!(9);
    const BACK_PORCH_DURATION: Duration = ms!(3);
    const BLANK_DURATION: Duration = ms!(54);
}
