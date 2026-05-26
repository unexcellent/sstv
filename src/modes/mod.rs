use crate::{
    Hz, Result,
    image::RgbPixel,
    modes::robot36::Robot36Encoder,
    ms, ns,
    synthesizer::Tone,
    units::{Duration, Frequency},
    us,
};
use alloc::boxed::Box;
use core::array;

pub mod robot36;

/// A specific protocol for encoding an image as a tone-sequence.
pub enum Mode {
    /// A mode using pulse-width modulation to encode a 320x240 pixel image into a 36s transmission.
    Robot36,
}

impl Mode {
    const HEADER_SEQUENCE_LENGTH: usize = 21;

    /// The frequency representing the start of a new scanline.
    pub const fn sync_frequency(&self) -> Frequency {
        Hz!(1200)
    }
    /// The lower frequency bound of the analog image signal.
    pub const fn black_frequency(&self) -> Frequency {
        Hz!(1500)
    }
    /// The upper frequenyc bound of the analog image signal.
    pub const fn white_frequency(&self) -> Frequency {
        Hz!(2300)
    }
    /// The frequency used to separate calibration pulses in the header.
    pub const fn break_frequency(&self) -> Frequency {
        Hz!(1200)
    }
    /// The continuous frequency sent at the very beginning of a transmission to wake up receivers.
    pub const fn leader_frequency(&self) -> Frequency {
        Hz!(1900)
    }
    /// The frequency used between alternating color bands during the tuning sequence.
    pub const fn separator_frequency(&self) -> Frequency {
        Hz!(1900)
    }
    /// The frequency representing a digital '0' in the VIS code.
    pub const fn zero_frequency(&self) -> Frequency {
        Hz!(1300)
    }
    /// The frequency representing a digital '1' in the VIS code.
    pub const fn one_frequency(&self) -> Frequency {
        Hz!(1100)
    }

    /// The length of the horizontal synchronization tone.
    pub const fn sync_duration(&self) -> Duration {
        ms!(9)
    }
    /// The quiet period immediately following the synchronization tone.
    pub const fn back_porch_duration(&self) -> Duration {
        ms!(3)
    }
    /// The horizontal blanking interval before pixel data begins.
    pub const fn blank_duration(&self) -> Duration {
        us!(5400)
    }
    /// The length of a single digital tone in the VIS header.
    pub const fn bit_duration(&self) -> Duration {
        ms!(30)
    }
    /// The total time required to transmit a single row of the image.
    pub const fn line_duration(&self) -> Duration {
        us!(150_008)
    }
    /// The dead time inserted between scanlines.
    pub const fn line_gap_duration(&self) -> Duration {
        ns!(320)
    }
    /// The total overhead time per row not used for visual data.
    pub fn line_suffix_duration(&self) -> Duration {
        self.back_porch_duration() + self.blank_duration() + self.sync_duration()
    }
    /// The time per row dedicated to actively transmitting visual data.
    pub fn line_pixel_duration(&self) -> Duration {
        self.line_duration() - self.line_suffix_duration()
    }
    /// The time per row dedicated to the grayscale intensity channel.
    pub fn line_luma_duration(&self) -> Duration {
        self.line_pixel_duration() - self.line_chroma_duration()
    }
    /// The time per row dedicated to the color difference channels.
    pub fn line_chroma_duration(&self) -> Duration {
        self.line_pixel_duration() / 3
    }
    /// The time allocated to a single grayscale pixel.
    pub fn pixel_luma_duration(&self) -> Duration {
        self.line_luma_duration() / self.image_width()
    }
    /// The time allocated to a single color difference pixel.
    pub fn pixel_chroma_duration(&self) -> Duration {
        self.line_chroma_duration() / self.image_width()
    }

    /// The horizontal resolution in pixels.
    pub const fn image_width(&self) -> u32 {
        320
    }
    /// The vertical resolution in pixels.
    pub const fn image_height(&self) -> u32 {
        240
    }

    /// The unique VIS ID transmitted to inform the receiver of the encoding format.
    pub const fn identification(&self) -> u8 {
        136
    }

    /// Construct the initial calibration and VIS digital tones sent before the image.
    pub fn header_sequence(&self) -> [Tone; Self::HEADER_SEQUENCE_LENGTH] {
        let tuning = [
            Tone::new(self.separator_frequency(), ms!(100)),
            Tone::new(self.black_frequency(), ms!(100)),
            Tone::new(self.separator_frequency(), ms!(100)),
            Tone::new(self.black_frequency(), ms!(100)),
            Tone::new(self.white_frequency(), ms!(100)),
            Tone::new(self.black_frequency(), ms!(100)),
            Tone::new(self.white_frequency(), ms!(100)),
            Tone::new(self.black_frequency(), ms!(100)),
        ];
        let calibration = [
            Tone::new(self.leader_frequency(), ms!(300)),
            Tone::new(self.break_frequency(), ms!(10)),
            Tone::new(self.leader_frequency(), ms!(300)),
            Tone::new(self.break_frequency(), self.bit_duration()),
        ];
        let identification: [Tone; 8] = array::from_fn(|i| {
            let freq = if (self.identification() >> i) & 1 == 1 {
                self.one_frequency()
            } else {
                self.zero_frequency()
            };
            Tone::new(freq, self.bit_duration())
        });

        array::from_fn(|i| match i {
            0..=7 => tuning[i],
            8..=11 => calibration[i - 8],
            12..=19 => identification[i - 12],
            _ => Tone::new(self.break_frequency(), self.bit_duration()),
        })
    }

    /// Create an iterator that converts raw RGB data into audio tones.
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
                Tone::new(Hz!(1900), ms!(100)),
                Tone::new(Hz!(1500), ms!(100)),
                Tone::new(Hz!(1900), ms!(100)),
                Tone::new(Hz!(1500), ms!(100)),
                Tone::new(Hz!(2300), ms!(100)),
                Tone::new(Hz!(1500), ms!(100)),
                Tone::new(Hz!(2300), ms!(100)),
                Tone::new(Hz!(1500), ms!(100)),
                Tone::new(Hz!(1900), ms!(300)),
                Tone::new(Hz!(1200), ms!(10)),
                Tone::new(Hz!(1900), ms!(300)),
                Tone::new(Hz!(1200), ms!(30)),
                Tone::new(Hz!(1300), ms!(30)),
                Tone::new(Hz!(1300), ms!(30)),
                Tone::new(Hz!(1300), ms!(30)),
                Tone::new(Hz!(1100), ms!(30)),
                Tone::new(Hz!(1300), ms!(30)),
                Tone::new(Hz!(1300), ms!(30)),
                Tone::new(Hz!(1300), ms!(30)),
                Tone::new(Hz!(1100), ms!(30)),
                Tone::new(Hz!(1200), ms!(30)),
            ]
        )
    }
}
