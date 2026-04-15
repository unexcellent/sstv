use crate::{
    Hz, Result,
    image::RgbPixel,
    modes::robot36::Robot36Encoder,
    ms,
    synthesizer::Tone,
    units::{Duration, Frequency},
    us,
};
use alloc::boxed::Box;
use core::array;

pub mod robot36;

pub enum Mode {
    Robot36,
}

impl Mode {
    const HEADER_SEQUENCE_LENGTH: usize = 21;

    pub const fn sync_frequency(&self) -> Frequency {
        Hz!(1200)
    }
    pub const fn black_frequency(&self) -> Frequency {
        Hz!(1500)
    }
    pub const fn white_frequency(&self) -> Frequency {
        Hz!(2300)
    }
    pub const fn break_frequency(&self) -> Frequency {
        Hz!(1200)
    }
    pub const fn leader_frequency(&self) -> Frequency {
        Hz!(1900)
    }
    pub const fn separator_frequency(&self) -> Frequency {
        Hz!(1900)
    }
    pub const fn zero_frequency(&self) -> Frequency {
        Hz!(1300)
    }
    pub const fn one_frequency(&self) -> Frequency {
        Hz!(1100)
    }

    pub const fn sync_duration(&self) -> Duration {
        ms!(9)
    }
    pub const fn back_porch_duration(&self) -> Duration {
        ms!(3)
    }
    pub const fn blank_duration(&self) -> Duration {
        us!(5400)
    }
    pub const fn bit_duration(&self) -> Duration {
        ms!(30)
    }
    pub const fn line_duration(&self) -> Duration {
        us!(150_008)
    }

    pub const fn identification(&self) -> u8 {
        136
    }

    pub fn header_sequence(&self) -> [Tone; Self::HEADER_SEQUENCE_LENGTH] {
        let tuning = [
            Tone(self.separator_frequency(), ms!(100)),
            Tone(self.black_frequency(), ms!(100)),
            Tone(self.separator_frequency(), ms!(100)),
            Tone(self.black_frequency(), ms!(100)),
            Tone(self.white_frequency(), ms!(100)),
            Tone(self.black_frequency(), ms!(100)),
            Tone(self.white_frequency(), ms!(100)),
            Tone(self.black_frequency(), ms!(100)),
        ];
        let calibration = [
            Tone(self.leader_frequency(), ms!(300)),
            Tone(self.break_frequency(), ms!(10)),
            Tone(self.leader_frequency(), ms!(300)),
            Tone(self.break_frequency(), self.bit_duration()),
        ];
        let identification: [Tone; 8] = array::from_fn(|i| {
            let freq = if (self.identification() >> i) & 1 == 1 {
                self.one_frequency()
            } else {
                self.zero_frequency()
            };
            Tone(freq, self.bit_duration())
        });

        array::from_fn(|i| match i {
            0..=7 => tuning[i],
            8..=11 => calibration[i - 8],
            12..=19 => identification[i - 12],
            _ => Tone(self.break_frequency(), self.bit_duration()),
        })
    }

    pub fn encoder<I>(&self, pixels: I) -> Result<Box<dyn Iterator<Item = Tone>>>
    where
        I: Iterator<Item = RgbPixel> + 'static,
    {
        match self {
            Mode::Robot36 => Ok(Box::new(Robot36Encoder::new(pixels)?)),
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::synthesizer::Tone;
    use crate::{Hz, ms};

    #[test]
    fn test_header_tones_robot36() {
        assert_eq!(
            Mode::Robot36.header_sequence(),
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
