use crate::{
    image::{RgbPixel, YuvPixel},
    modes::mode::Mode,
    ms,
    synthesizer::Tone,
    units::{Duration, Frequency},
};
use core::array;

pub struct Robot36;

impl Mode for Robot36 {
    const IDENTIFICATION: u8 = 136;
    const IMAGE_WIDTH: u16 = 320;
    const IMAGE_HEIGHT: u16 = 240;
}

enum EncoderState {
    Header { position: u8 },
    Image { row: u16, col: u16 },
}

pub struct Robot36Encoder<I> {
    pixel_iter: I,
    state: EncoderState,
    next_row: MergedRows,
}

impl<I> Robot36Encoder<I>
where
    I: Iterator<Item = RgbPixel>,
{
    pub fn new(mut pixel_iter: I) -> Self {
        let two_rows: [RgbPixel; 640] = array::from_fn(|_| {
            pixel_iter
                .next()
                .expect("Iterator exhausted before yielding 640 pixels")
        });
        Self {
            pixel_iter,
            state: EncoderState::Header { position: 0 },
            next_row: MergedRows::new(&two_rows),
        }
    }

    fn pixel_to_tones(pixel: YuvPixel) -> [Tone; 3] {
        let y_dur = Duration::from_micros(276);
        let uv_dur = Duration::from_micros(44112);

        [
            Self::subpixel_to_tone(pixel.luma(), y_dur),
            Self::subpixel_to_tone(pixel.chroma_blue(), uv_dur),
            Self::subpixel_to_tone(pixel.chroma_red(), uv_dur),
        ]
    }

    fn subpixel_to_tone(subpixel_value: u8, duration: Duration) -> Tone {
        let black = Robot36::BLACK.hz() as u32;
        let white = Robot36::WHITE.hz() as u32;
        let freq = black + (subpixel_value as u32 * (white - black) / 255);

        Tone(Frequency::from_hz(freq as u16), duration)
    }
}

impl<I> Iterator for Robot36Encoder<I>
where
    I: Iterator<Item = RgbPixel>,
{
    type Item = Tone;

    fn next(&mut self) -> Option<Self::Item> {
        match self.state {
            EncoderState::Header { position } => {
                let header_tones = Robot36.header_sequence();
                let tone = header_tones[position as usize];
                self.state = if (position as usize) + 1 < header_tones.len() {
                    EncoderState::Header {
                        position: position + 1,
                    }
                } else {
                    EncoderState::Image { row: 0, col: 0 }
                };

                Some(tone)
            }
            EncoderState::Image { .. } => match self.next_row.next() {
                Some(pixel) => Some(Self::pixel_to_tones(pixel)[0]),
                None => {
                    let two_rows: [RgbPixel; 640] = array::from_fn(|_| {
                        self.pixel_iter
                            .next()
                            .expect("Iterator exhausted before yielding 640 pixels")
                    });
                    self.next_row = MergedRows::new(&two_rows);
                    None
                }
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MergedRows([YuvPixel; 320], usize);

impl MergedRows {
    pub fn new(two_rows: &[RgbPixel; 640]) -> Self {
        let yuv_rows: [YuvPixel; 640] = array::from_fn(|i| YuvPixel::from(two_rows[i]));
        let merged_rows: [YuvPixel; 320] = array::from_fn(|i| {
            let first_row_pixel = yuv_rows[i];
            let second_row_pixel = yuv_rows[i + 320];
            YuvPixel::new(
                first_row_pixel.luma(),
                second_row_pixel.chroma_red(),
                second_row_pixel.chroma_blue(),
            )
        });
        Self(merged_rows, 0)
    }
}

impl Iterator for MergedRows {
    type Item = YuvPixel;
    fn next(&mut self) -> Option<Self::Item> {
        if self.1 < 320 {
            let pixel = self.0[self.1];
            self.1 += 1;
            Some(pixel)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::robot36::Robot36;
    use crate::synthesizer::Tone;
    use crate::units::{Duration, Frequency};
    use crate::{ms, Hz};
    use alloc::vec::Vec;

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

    #[test]
    fn test_row_merging() {
        let rgb_pixels: [RgbPixel; 640] = array::from_fn(|i| {
            if i < 320 {
                RgbPixel::new(0, 0, 255)
            } else {
                RgbPixel::new(255, 0, 0)
            }
        });
        let merged_yuv: [YuvPixel; 320] = array::from_fn(|_| YuvPixel::new(28, 255, 84));

        assert_eq!(MergedRows::new(&rgb_pixels), MergedRows(merged_yuv, 0))
    }

    #[test]
    fn test_compare_encoder_tones_with_encode_rs() {
        use crate::encode::{generate_robot36_tones, ImageData, RgbPixel as EncodeRgbPixel};

        let width = 320;
        let height = 240;

        let mut encode_pixels = Vec::with_capacity(width * height);
        let mut image_pixels = Vec::with_capacity(width * height);

        for _ in 0..(width * height) {
            encode_pixels.push(EncodeRgbPixel::new(100, 150, 200));
            image_pixels.push(RgbPixel::new(100, 150, 200));
        }

        let image_data = ImageData::new(width as u32, height as u32, encode_pixels);
        let expected_encode_tones = generate_robot36_tones(&image_data).unwrap();

        let mut encoder = Robot36Encoder::new(image_pixels.into_iter());

        for (i, exp_tone) in expected_encode_tones.into_iter().enumerate() {
            let actual = encoder
                .next()
                .unwrap_or_else(|| panic!("Missing tone at index {}", i));

            let exp_hz = exp_tone.freq.round() as u16;
            let exp_micros = (exp_tone.duration * 1_000_000.0).round() as u32;

            let expected_synth_tone = Tone(
                Frequency::from_hz(exp_hz),
                Duration::from_micros(exp_micros),
            );

            assert_eq!(
                actual.0.hz(),
                expected_synth_tone.0.hz(),
                "Tone {} frequency mismatch: Expected {} Hz, got {} Hz",
                i,
                expected_synth_tone.0.hz(),
                actual.0.hz()
            );

            assert!(
                actual.1.micros().abs_diff(expected_synth_tone.1.micros()) <= 5,
                "Tone {} duration mismatch: Expected {} us, got {} us",
                i,
                expected_synth_tone.1.micros(),
                actual.1.micros()
            );
        }
    }
}
