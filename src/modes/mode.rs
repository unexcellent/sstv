use crate::synthesizer::Tone;
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

    fn identification_tones(&self) -> [Tone; 8] {
        core::array::from_fn(|i| {
            let freq = if (Self::IDENTIFICATION >> i) & 1 == 1 {
                Self::BINARY_1
            } else {
                Self::BINARY_0
            };
            Tone(freq, Self::BIT_DURATION)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::robot36::Robot36;

    #[test]
    fn test_identification_tones_robot36() {
        assert_eq!(
            Robot36.identification_tones(),
            [
                Tone(Robot36::BINARY_0, Robot36::BIT_DURATION),
                Tone(Robot36::BINARY_0, Robot36::BIT_DURATION),
                Tone(Robot36::BINARY_0, Robot36::BIT_DURATION),
                Tone(Robot36::BINARY_1, Robot36::BIT_DURATION),
                Tone(Robot36::BINARY_0, Robot36::BIT_DURATION),
                Tone(Robot36::BINARY_0, Robot36::BIT_DURATION),
                Tone(Robot36::BINARY_0, Robot36::BIT_DURATION),
                Tone(Robot36::BINARY_1, Robot36::BIT_DURATION),
            ]
        )
    }
}
