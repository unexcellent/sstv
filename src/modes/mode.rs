use crate::synthesizer::Tone;
use crate::units::{Duration, Frequency};
use crate::{ms, us, Hz};

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
    const LINE_DURATION: Duration;
    const SYNC_DURATION: Duration = ms!(9);
    const BACK_PORCH_DURATION: Duration = ms!(3);
    const BLANK_DURATION: Duration = us!(5400);

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
