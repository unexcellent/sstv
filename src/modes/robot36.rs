use crate::{
    Error, Hz, Result,
    image::{RgbPixel, YuvPixel},
    modes::Mode,
    synthesizer::Tone,
    units::Duration,
};
use core::array;

enum EncoderState {
    Header(usize),
    EvenLuma(usize),
    EvenLumaToChroma,
    EvenChroma(usize),
    EvenToOdd,
    OddLuma(usize),
    OddLumaToChroma,
    OddChroma(usize),
    OddToEven,
    LineGap,
    Done,
}

impl EncoderState {
    /// Increment the inner position in the states that have an inner position
    pub fn increment(&mut self) {
        match self {
            Self::Header(pos) => *self = Self::Header(*pos + 1),
            Self::EvenLuma(pos) => *self = Self::EvenLuma(*pos + 1),
            Self::EvenChroma(pos) => *self = Self::EvenChroma(*pos + 1),
            Self::OddLuma(pos) => *self = Self::OddLuma(*pos + 1),
            Self::OddChroma(pos) => *self = Self::OddChroma(*pos + 1),
            _ => (),
        }
    }

    /// Advance to the next state
    pub fn advance(&mut self) {
        match self {
            Self::Header(_) => *self = Self::EvenLuma(0),
            Self::EvenLuma(_) => *self = Self::EvenLumaToChroma,
            Self::EvenLumaToChroma => *self = Self::EvenChroma(0),
            Self::EvenChroma(_) => *self = Self::EvenToOdd,
            Self::EvenToOdd => *self = Self::OddLuma(0),
            Self::OddLuma(_) => *self = Self::OddLumaToChroma,
            Self::OddLumaToChroma => *self = Self::OddChroma(0),
            Self::OddChroma(_) => *self = Self::OddToEven,
            Self::OddToEven => *self = Self::LineGap,
            Self::LineGap => *self = Self::EvenLuma(0),
            Self::Done => *self = Self::Done,
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
    remaining_line_time: Duration,
    luma_time: Duration,
    chroma_time: Duration,
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

        let total_pixel_time = Self::mode().line_pixel_duration();
        let chroma_time = total_pixel_time / 3;
        let luma_time = total_pixel_time - chroma_time;

        Ok(Self {
            state: EncoderState::Header(0),
            pixel_iter,
            even_row: averaged_even_row,
            odd_row: averaged_odd_row,
            remaining_line_time: Self::mode().line_duration(),
            luma_time,
            chroma_time,
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

    fn emit_tone(&mut self, tone: Tone) -> Option<Tone> {
        self.remaining_line_time = self.remaining_line_time - tone.1;
        Some(tone)
    }

    fn fetch_next_rows(&mut self) {
        let first_row = match Self::fill_row(&mut self.pixel_iter) {
            Some(row) => row,
            None => {
                self.state = EncoderState::Done;
                return;
            }
        };
        let second_row = match Self::fill_row(&mut self.pixel_iter) {
            Some(row) => row,
            None => {
                self.state = EncoderState::Done;
                return;
            }
        };
        self.even_row = Self::average_rows(&first_row, &second_row);
        self.odd_row = Self::average_rows(&second_row, &first_row);
    }
}

impl<I> Iterator for Robot36Encoder<I>
where
    I: Iterator<Item = RgbPixel>,
{
    type Item = Tone;

    fn next(&mut self) -> Option<Self::Item> {
        match self.state {
            EncoderState::Header(position) => {
                let header_tones = Self::mode().header_sequence();

                if position < Mode::HEADER_SEQUENCE_LENGTH - 1 {
                    self.state.increment();
                } else {
                    self.state.advance();
                }

                Some(header_tones[position])
            }
            EncoderState::EvenLuma(position) => match self.even_row.get(position) {
                Some(pixel) => {
                    self.state.increment();
                    self.emit_tone(pixel.luma_tone(
                        Self::mode().black_frequency(),
                        Self::mode().white_frequency(),
                        self.luma_time / Self::mode().image_width(),
                    ))
                }
                None => {
                    self.state.advance();
                    self.emit_tone(Tone(
                        Self::mode().black_frequency(),
                        Self::mode().blank_duration() * 2 / 3,
                    ))
                }
            },
            EncoderState::EvenLumaToChroma => {
                self.state.advance();
                self.emit_tone(Tone(
                    Self::mode().separator_frequency(),
                    Self::mode().blank_duration() / 3,
                ))
            }
            EncoderState::EvenChroma(position) => match self.even_row.get(position) {
                Some(pixel) => {
                    self.state.increment();
                    self.emit_tone(pixel.chroma_red_tone(
                        Self::mode().black_frequency(),
                        Self::mode().white_frequency(),
                        self.chroma_time / Self::mode().image_width(),
                    ))
                }
                None => {
                    self.state.advance();
                    self.emit_tone(Tone(
                        Self::mode().sync_frequency(),
                        Self::mode().sync_duration(),
                    ))
                }
            },
            EncoderState::EvenToOdd => {
                self.state.advance();
                let tone = self.emit_tone(Tone(
                    Self::mode().black_frequency(),
                    Self::mode().back_porch_duration(),
                ));
                self.remaining_line_time = Self::mode().line_duration();
                tone
            }
            EncoderState::OddLuma(position) => match self.odd_row.get(position) {
                Some(pixel) => {
                    self.state.increment();
                    self.emit_tone(pixel.luma_tone(
                        Self::mode().black_frequency(),
                        Self::mode().white_frequency(),
                        self.luma_time / Self::mode().image_width(),
                    ))
                }
                None => {
                    self.state.advance();
                    self.emit_tone(Tone(
                        Self::mode().white_frequency(),
                        Self::mode().blank_duration() * 2 / 3,
                    ))
                }
            },
            EncoderState::OddLumaToChroma => {
                self.state.advance();
                self.emit_tone(Tone(
                    Self::mode().separator_frequency(),
                    Self::mode().blank_duration() / 3,
                ))
            }
            EncoderState::OddChroma(position) => match self.odd_row.get(position) {
                Some(pixel) => {
                    self.state.increment();
                    self.emit_tone(pixel.chroma_blue_tone(
                        Self::mode().black_frequency(),
                        Self::mode().white_frequency(),
                        self.chroma_time / Self::mode().image_width(),
                    ))
                }
                None => {
                    self.state.advance();
                    self.emit_tone(Tone(
                        Self::mode().sync_frequency(),
                        Self::mode().sync_duration(),
                    ))
                }
            },
            EncoderState::OddToEven => {
                self.state.advance();
                self.fetch_next_rows();
                self.emit_tone(Tone(
                    Self::mode().black_frequency(),
                    Self::mode().back_porch_duration(),
                ))
            }
            EncoderState::LineGap => {
                self.state.advance();
                let gap_length = self.remaining_line_time;
                self.remaining_line_time = Self::mode().line_duration();
                self.emit_tone(Tone(Hz!(0), gap_length))
            }
            EncoderState::Done => None,
        }
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
