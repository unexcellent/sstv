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
    const LEADER: Frequency = Hz!(1900);
    const SEPARATOR: Frequency = Hz!(1900);
    const BIT_DURATION: Duration = ms!(30);

    const IDENTIFICATION: u8;
    const IMAGE_WIDTH: u16;
    const IMAGE_HEIGHT: u16;
    const SYNC_DURATION: Duration = ms!(9);
    const BACK_PORCH_DURATION: Duration = ms!(3);
    const BLANK_DURATION: Duration = ms!(54);

    fn header_sequence(&self) -> [Tone; 21] {
        let tuning = self.tuning_sequence();
        let calibration = self.calibration_header();
        let id = self.identification_sequence();

        core::array::from_fn(|i| match i {
            0..=7 => tuning[i],
            8..=11 => calibration[i - 8],
            12..=19 => id[i - 12],
            20 => Tone(Self::BREAK, Self::BIT_DURATION),
            _ => unreachable!(),
        })
    }

    fn tuning_sequence(&self) -> [Tone; 8] {
        [
            Tone(Self::SEPARATOR, ms!(100)),
            Tone(Self::BLACK, ms!(100)),
            Tone(Self::SEPARATOR, ms!(100)),
            Tone(Self::BLACK, ms!(100)),
            Tone(Self::WHITE, ms!(100)),
            Tone(Self::BLACK, ms!(100)),
            Tone(Self::WHITE, ms!(100)),
            Tone(Self::BLACK, ms!(100)),
        ]
    }

    fn calibration_header(&self) -> [Tone; 4] {
        [
            Tone(Self::LEADER, ms!(300)),
            Tone(Self::BREAK, ms!(10)),
            Tone(Self::LEADER, ms!(300)),
            Tone(Self::BREAK, Self::BIT_DURATION),
        ]
    }

    fn identification_sequence(&self) -> [Tone; 8] {
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
    use crate::{Hz, ms};

    #[test]
    fn test_identification_tones_robot36() {
        assert_eq!(
            Robot36.identification_sequence(),
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

    #[test]
    fn test_header_tones_robot36() {
        assert_eq!(
            Robot36.header_sequence(),
            [
                Tone(Hz!(1900), ms!(100)),
                Tone(Hz!(1500), ms!(100)),
                Tone(Hz!(1900), ms!(100)),
                Tone(Hz!(1500), ms!(100)),
                Tone(Hz!(2300), ms!(100)),
                Tone(Hz!(1500), ms!(100)),
                Tone(Hz!(2300), ms!(100)),
                Tone(Hz!(1500), ms!(100)),
                Tone(Hz!(1900), ms!(300)),
                Tone(Hz!(1200), ms!(10)),
                Tone(Hz!(1900), ms!(300)),
                Tone(Hz!(1200), ms!(30)),
                Tone(Hz!(1300), ms!(30)),
                Tone(Hz!(1300), ms!(30)),
                Tone(Hz!(1300), ms!(30)),
                Tone(Hz!(1100), ms!(30)),
                Tone(Hz!(1300), ms!(30)),
                Tone(Hz!(1300), ms!(30)),
                Tone(Hz!(1300), ms!(30)),
                Tone(Hz!(1100), ms!(30)),
                Tone(Hz!(1200), ms!(30)),
            ]
        )
    }
}
