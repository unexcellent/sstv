use crate::{
    Error, Hz, Result,
    image::{RgbPixel, YuvPixel},
    modes::Mode,
    synthesizer::Tone,
    units::Duration,
};

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

pub struct Robot36Encoder<I>
where
    I: Iterator<Item = RgbPixel>,
{
    state: EncoderState,
    pixel_iter: I,
    current_row: [YuvPixel; 320],
    next_row: [YuvPixel; 320],
    remaining_line_time: Duration,
    luma_time: Duration,
    chroma_time: Duration,
}

impl<I> Robot36Encoder<I>
where
    I: Iterator<Item = RgbPixel>,
{
    pub fn new(mut pixel_iter: I) -> Result<Self> {
        let current_row = match Self::fill_row(&mut pixel_iter) {
            Some(pixels) => pixels,
            None => return Err(Error::EmptyImage),
        };
        let next_row = match Self::fill_row(&mut pixel_iter) {
            Some(pixels) => pixels,
            None => return Err(Error::EmptyImage),
        };

        let fixed_remaining = Mode::Robot36.blank_duration()
            + Mode::Robot36.sync_duration()
            + Mode::Robot36.back_porch_duration();
        let total_pixel_time = Mode::Robot36.line_duration() - fixed_remaining;
        let chroma_time = total_pixel_time / 3;
        let luma_time = total_pixel_time - chroma_time;

        Ok(Self {
            state: EncoderState::Header(0),
            pixel_iter,
            current_row,
            next_row,
            remaining_line_time: Mode::Robot36.line_duration(),
            luma_time,
            chroma_time,
        })
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
        match Self::fill_row(&mut self.pixel_iter) {
            Some(row) => self.current_row = row,
            None => {
                self.state = EncoderState::Done;
                return;
            }
        }
        match Self::fill_row(&mut self.pixel_iter) {
            Some(row) => self.next_row = row,
            None => {
                self.state = EncoderState::Done;
            }
        }
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
                let header_tones = Mode::Robot36.header_sequence();
                let tone = header_tones[position];
                self.state = if position + 1 < header_tones.len() {
                    EncoderState::Header(position + 1)
                } else {
                    EncoderState::EvenLuma(0)
                };

                Some(tone)
            }
            EncoderState::EvenLuma(position) => match self.current_row.get(position) {
                Some(pixel) => {
                    self.state = EncoderState::EvenLuma(position + 1);
                    self.emit_tone(pixel.luma_tone(
                        Mode::Robot36.black_frequency(),
                        Mode::Robot36.white_frequency(),
                        self.luma_time / 320,
                    ))
                }
                None => {
                    self.state = EncoderState::EvenLumaToChroma;
                    self.emit_tone(Tone(
                        Mode::Robot36.black_frequency(),
                        Mode::Robot36.blank_duration() * 2 / 3,
                    ))
                }
            },
            EncoderState::EvenLumaToChroma => {
                self.state = EncoderState::EvenChroma(0);
                self.emit_tone(Tone(
                    Mode::Robot36.separator_frequency(),
                    Mode::Robot36.blank_duration() / 3,
                ))
            }
            EncoderState::EvenChroma(position) => {
                match (self.current_row.get(position), self.next_row.get(position)) {
                    (Some(current_row_pixel), Some(next_row_pixel)) => {
                        self.state = EncoderState::EvenChroma(position + 1);
                        let combined_pixel = YuvPixel::average(*current_row_pixel, *next_row_pixel);
                        self.emit_tone(combined_pixel.chroma_red_tone(
                            Mode::Robot36.black_frequency(),
                            Mode::Robot36.white_frequency(),
                            self.chroma_time / 320,
                        ))
                    }
                    _ => {
                        self.state = EncoderState::EvenToOdd;
                        self.emit_tone(Tone(
                            Mode::Robot36.sync_frequency(),
                            Mode::Robot36.sync_duration(),
                        ))
                    }
                }
            }
            EncoderState::EvenToOdd => {
                self.state = EncoderState::OddLuma(0);
                let tone = self.emit_tone(Tone(
                    Mode::Robot36.black_frequency(),
                    Mode::Robot36.back_porch_duration(),
                ));
                self.remaining_line_time = Mode::Robot36.line_duration();
                tone
            }
            EncoderState::OddLuma(position) => match self.next_row.get(position) {
                Some(pixel) => {
                    self.state = EncoderState::OddLuma(position + 1);
                    self.emit_tone(pixel.luma_tone(
                        Mode::Robot36.black_frequency(),
                        Mode::Robot36.white_frequency(),
                        self.luma_time / 320,
                    ))
                }
                None => {
                    self.state = EncoderState::OddLumaToChroma;
                    self.emit_tone(Tone(
                        Mode::Robot36.white_frequency(),
                        Mode::Robot36.blank_duration() * 2 / 3,
                    ))
                }
            },
            EncoderState::OddLumaToChroma => {
                self.state = EncoderState::OddChroma(0);
                self.emit_tone(Tone(
                    Mode::Robot36.separator_frequency(),
                    Mode::Robot36.blank_duration() / 3,
                ))
            }
            EncoderState::OddChroma(position) => {
                match (self.current_row.get(position), self.next_row.get(position)) {
                    (Some(current_row_pixel), Some(next_row_pixel)) => {
                        self.state = EncoderState::OddChroma(position + 1);
                        let combined_pixel = YuvPixel::average(*current_row_pixel, *next_row_pixel);
                        self.emit_tone(combined_pixel.chroma_blue_tone(
                            Mode::Robot36.black_frequency(),
                            Mode::Robot36.white_frequency(),
                            self.chroma_time / 320,
                        ))
                    }
                    _ => {
                        self.state = EncoderState::OddToEven;
                        self.emit_tone(Tone(
                            Mode::Robot36.sync_frequency(),
                            Mode::Robot36.sync_duration(),
                        ))
                    }
                }
            }
            EncoderState::OddToEven => {
                self.state = EncoderState::LineGap;
                self.fetch_next_rows();
                self.emit_tone(Tone(
                    Mode::Robot36.black_frequency(),
                    Mode::Robot36.back_porch_duration(),
                ))
            }
            EncoderState::LineGap => {
                self.state = EncoderState::EvenLuma(0);
                let gap_length = self.remaining_line_time;
                self.remaining_line_time = Mode::Robot36.line_duration();
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
