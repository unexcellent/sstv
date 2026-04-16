use crate::{
    Error, Hz, Result,
    image::{RgbPixel, YuvPixel},
    modes::Mode,
    synthesizer::Tone,
};
use core::array;

enum EncoderState {
    NotStarted,
    Header(usize),
    EvenLuma(usize),
    EvenLumaToChroma(usize),
    EvenChroma(usize),
    EvenToOdd(usize),
    OddLuma(usize),
    OddLumaToChroma(usize),
    OddChroma(usize),
    OddToEven(usize),
    Finished,
}

impl EncoderState {
    pub fn advance(&mut self) {
        match self {
            Self::NotStarted => *self = Self::Header(0),
            Self::Header(pos) => {
                if *pos < Mode::HEADER_SEQUENCE_LENGTH - 1 {
                    *self = Self::Header(*pos + 1);
                } else {
                    *self = Self::EvenLuma(0);
                }
            }
            Self::EvenLuma(pos) => {
                if *pos < Mode::Robot36.image_width() as usize - 1 {
                    *self = Self::EvenLuma(*pos + 1);
                } else {
                    *self = Self::EvenLumaToChroma(0);
                }
            }
            Self::EvenLumaToChroma(pos) => {
                if *pos == 0 {
                    *self = Self::EvenLumaToChroma(1);
                } else {
                    *self = Self::EvenChroma(0);
                }
            }
            Self::EvenChroma(pos) => {
                if *pos < Mode::Robot36.image_width() as usize - 1 {
                    *self = Self::EvenChroma(*pos + 1);
                } else {
                    *self = Self::EvenToOdd(0);
                }
            }
            Self::EvenToOdd(pos) => {
                if *pos == 0 {
                    *self = Self::EvenToOdd(1);
                } else {
                    *self = Self::OddLuma(0);
                }
            }
            Self::OddLuma(pos) => {
                if *pos < Mode::Robot36.image_width() as usize - 1 {
                    *self = Self::OddLuma(*pos + 1);
                } else {
                    *self = Self::OddLumaToChroma(0);
                }
            }
            Self::OddLumaToChroma(pos) => {
                if *pos == 0 {
                    *self = Self::OddLumaToChroma(1);
                } else {
                    *self = Self::OddChroma(0);
                }
            }
            Self::OddChroma(pos) => {
                if *pos < Mode::Robot36.image_width() as usize - 1 {
                    *self = Self::OddChroma(*pos + 1);
                } else {
                    *self = Self::OddToEven(0);
                }
            }
            Self::OddToEven(pos) => {
                if *pos < 2 {
                    *self = Self::OddToEven(*pos + 1);
                } else {
                    *self = Self::EvenLuma(0);
                }
            }
            Self::Finished => (),
        }
    }
}

pub struct Robot36Encoder<I>
where
    I: Iterator<Item = RgbPixel>,
{
    state: EncoderState,
    pixel_iter: I,
    even_row: [YuvPixel; 320],
    odd_row: [YuvPixel; 320],
}

impl<I> Robot36Encoder<I>
where
    I: Iterator<Item = RgbPixel>,
{
    pub fn new(mut pixel_iter: I) -> Result<Self> {
        let first_row = match Self::fill_row(&mut pixel_iter) {
            Some(pixels) => pixels,
            None => return Err(Error::EmptyImage),
        };
        let second_row = match Self::fill_row(&mut pixel_iter) {
            Some(pixels) => pixels,
            None => return Err(Error::EmptyImage),
        };
        let averaged_even_row = Self::average_rows(&first_row, &second_row);
        let averaged_odd_row = Self::average_rows(&second_row, &first_row);

        Ok(Self {
            state: EncoderState::NotStarted,
            pixel_iter,
            even_row: averaged_even_row,
            odd_row: averaged_odd_row,
        })
    }

    pub const fn mode() -> Mode {
        Mode::Robot36
    }

    fn average_rows(
        primary_row: &[YuvPixel; 320],
        secondary_row: &[YuvPixel; 320],
    ) -> [YuvPixel; 320] {
        array::from_fn(|i| YuvPixel::average(primary_row[i], secondary_row[i]))
    }

    fn fill_row(iter: &mut I) -> Option<[YuvPixel; 320]> {
        let mut row = [YuvPixel::from(RgbPixel::new(0, 0, 0)); 320];
        for pixel in row.iter_mut() {
            *pixel = iter.next()?.into();
        }
        Some(row)
    }

    fn fetch_next_rows(&mut self) {
        let first_row = match Self::fill_row(&mut self.pixel_iter) {
            Some(row) => row,
            None => {
                self.state = EncoderState::Finished;
                return;
            }
        };
        let second_row = match Self::fill_row(&mut self.pixel_iter) {
            Some(row) => row,
            None => {
                self.state = EncoderState::Finished;
                return;
            }
        };
        self.even_row = Self::average_rows(&first_row, &second_row);
        self.odd_row = Self::average_rows(&second_row, &first_row);
    }

    fn emit(&mut self) -> Option<Tone> {
        match self.state {
            EncoderState::NotStarted => None,
            EncoderState::Header(pos) => Some(Self::mode().header_sequence()[pos]),
            EncoderState::EvenLuma(pos) => Some(self.even_row[pos].luma_tone(
                Self::mode().black_frequency(),
                Self::mode().white_frequency(),
                Self::mode().pixel_luma_duration(),
            )),
            EncoderState::EvenLumaToChroma(pos) => match pos {
                0 => Some(Tone(
                    Self::mode().black_frequency(),
                    Self::mode().blank_duration() * 2 / 3,
                )),
                _ => Some(Tone(
                    Self::mode().separator_frequency(),
                    Self::mode().blank_duration() / 3,
                )),
            },
            EncoderState::EvenChroma(pos) => Some(self.even_row[pos].chroma_red_tone(
                Self::mode().black_frequency(),
                Self::mode().white_frequency(),
                Self::mode().pixel_chroma_duration(),
            )),
            EncoderState::EvenToOdd(pos) => match pos {
                0 => Some(Tone(
                    Self::mode().sync_frequency(),
                    Self::mode().sync_duration(),
                )),
                _ => Some(Tone(
                    Self::mode().black_frequency(),
                    Self::mode().back_porch_duration(),
                )),
            },
            EncoderState::OddLuma(pos) => Some(self.odd_row[pos].luma_tone(
                Self::mode().black_frequency(),
                Self::mode().white_frequency(),
                Self::mode().pixel_luma_duration(),
            )),
            EncoderState::OddLumaToChroma(pos) => match pos {
                0 => Some(Tone(
                    Self::mode().white_frequency(),
                    Self::mode().blank_duration() * 2 / 3,
                )),
                _ => Some(Tone(
                    Self::mode().separator_frequency(),
                    Self::mode().blank_duration() / 3,
                )),
            },
            EncoderState::OddChroma(pos) => Some(self.odd_row[pos].chroma_blue_tone(
                Self::mode().black_frequency(),
                Self::mode().white_frequency(),
                Self::mode().pixel_chroma_duration(),
            )),
            EncoderState::OddToEven(pos) => match pos {
                0 => Some(Tone(
                    Self::mode().sync_frequency(),
                    Self::mode().sync_duration(),
                )),
                1 => Some(Tone(
                    Self::mode().black_frequency(),
                    Self::mode().back_porch_duration(),
                )),
                _ => {
                    self.fetch_next_rows();
                    if let EncoderState::Finished = self.state {
                        None
                    } else {
                        Some(Tone(Hz!(0), Self::mode().line_gap_duration()))
                    }
                }
            },
            EncoderState::Finished => None,
        }
    }
}

impl<I> Iterator for Robot36Encoder<I>
where
    I: Iterator<Item = RgbPixel>,
{
    type Item = Tone;

    fn next(&mut self) -> Option<Self::Item> {
        self.state.advance();
        self.emit()
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use flate2::read::GzDecoder;
    use image::GenericImageView;
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    use std::vec::Vec;

    use super::*;
    use crate::encoder::Encoder;
    use crate::synthesizer::Tone;
    use crate::{Hz, ns};

    #[test]
    fn test_empty_image() {
        let empty_image: Vec<RgbPixel> = vec![];

        assert!(matches!(
            Encoder::new(Mode::Robot36, empty_image.into_iter()),
            Err(Error::EmptyImage)
        ));
    }

    #[test]
    fn test_encode_robot36_against_golden_tones() {
        let img = image::open("examples/patch.png").expect("Failed to open examples/patch.png");
        let (_, height) = img.dimensions();
        assert_eq!(height, 240);

        let mut pixels = std::vec![[RgbPixel::new(0, 0, 0); 320]; 240];

        img.pixels().for_each(|(x, y, rgba)| {
            pixels[y as usize][x as usize] = RgbPixel::new(rgba[0], rgba[1], rgba[2]);
        });

        let encoder = Encoder::new(Mode::Robot36, pixels.into_iter().flatten());

        let file = File::open("examples/patch-robot36-tones.csv.gz").expect(
            "Failed to open golden tones file. Run 'cargo run --example store_tones' first.",
        );
        let decoder = GzDecoder::new(file);
        let reader = BufReader::new(decoder);
        let mut expected_tones = Vec::new();

        for line in reader.lines().skip(1) {
            let line = line.unwrap();
            let parts: Vec<&str> = line.split(',').collect();
            let hz: u32 = parts[0].parse().unwrap();
            let nanos: u64 = parts[1].parse().unwrap();
            expected_tones.push(Tone(Hz!(hz), ns!(nanos)));
        }

        let generated_tones: Vec<Tone> = encoder.unwrap().collect();

        assert_eq!(
            generated_tones.len(),
            expected_tones.len(),
            "Number of generated tones does not match golden file"
        );

        for (i, (g, exp)) in generated_tones
            .iter()
            .zip(expected_tones.iter())
            .enumerate()
        {
            assert_eq!(g, exp, "Tone mismatch at index {}", i);
        }
    }
}
