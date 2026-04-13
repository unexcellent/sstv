use crate::{
    Hz,
    image::{RgbPixel, YuvPixel},
    modes::mode::Mode,
    synthesizer::Tone,
    units::{Duration, Frequency},
    us,
};

pub struct Robot36;

impl Mode for Robot36 {
    const IDENTIFICATION: u8 = 136;
    const IMAGE_WIDTH: u16 = 320;
    const IMAGE_HEIGHT: u16 = 240;
    const LINE_DURATION: Duration = us!(150_008);
}

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
}

impl<I> Robot36Encoder<I>
where
    I: Iterator<Item = RgbPixel>,
{
    pub fn new(mut pixel_iter: I) -> Self {
        let current_row = Self::fill_row(&mut pixel_iter)
            .unwrap_or([YuvPixel::from(RgbPixel::new(0, 0, 0)); 320]);
        let next_row = Self::fill_row(&mut pixel_iter)
            .unwrap_or([YuvPixel::from(RgbPixel::new(0, 0, 0)); 320]);

        Self {
            state: EncoderState::Header(0),
            pixel_iter,
            current_row,
            next_row,
            remaining_line_time: Robot36::LINE_DURATION,
        }
    }

    fn fill_row(iter: &mut I) -> Option<[YuvPixel; 320]> {
        let mut row = [YuvPixel::from(RgbPixel::new(0, 0, 0)); 320];
        for pixel in row.iter_mut() {
            *pixel = iter.next()?.into();
        }
        Some(row)
    }

    fn emit_tone(&mut self, tone: Tone) -> Option<Tone> {
        if self.remaining_line_time.micros() < tone.1.micros() {
            panic!("Underflow for tone {}Hz {}ms", tone.0.hz(), tone.1.micros());
        }
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

    fn pixel_luma_tone(pixel: &YuvPixel, duration: Duration) -> Tone {
        let frequency: u32 = Robot36::BLACK.hz()
            + (pixel.luma() as u32 * (Robot36::WHITE.hz() - Robot36::BLACK.hz()) / 255);
        Tone(Frequency::from_hz(frequency), duration)
    }

    fn pixel_chroma_red_tone(
        current_row_pixel: &YuvPixel,
        next_row_pixel: &YuvPixel,
        duration: Duration,
    ) -> Tone {
        let average_chroma: u32 =
            (current_row_pixel.chroma_red() as u32 + next_row_pixel.chroma_red() as u32) / 2;
        let frequency: u32 = Robot36::BLACK.hz()
            + (average_chroma * (Robot36::WHITE.hz() - Robot36::BLACK.hz()) / 255);
        Tone(Frequency::from_hz(frequency), duration)
    }

    fn pixel_chroma_blue_tone(
        current_row_pixel: &YuvPixel,
        next_row_pixel: &YuvPixel,
        duration: Duration,
    ) -> Tone {
        let average_chroma: u32 =
            (current_row_pixel.chroma_blue() as u32 + next_row_pixel.chroma_blue() as u32) / 2;
        let frequency: u32 = Robot36::BLACK.hz()
            + (average_chroma * (Robot36::WHITE.hz() - Robot36::BLACK.hz()) / 255);
        Tone(Frequency::from_hz(frequency), duration)
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
                let header_tones = Robot36.header_sequence();
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
                    let fixed_remaining = Robot36::BLANK_DURATION
                        + Robot36::SYNC_DURATION
                        + Robot36::BACK_PORCH_DURATION;
                    let total_pixel_time = Robot36::LINE_DURATION - fixed_remaining;
                    let chroma_time = total_pixel_time / 3;
                    let remaining_luma_time =
                        self.remaining_line_time - fixed_remaining - chroma_time;
                    let duration = remaining_luma_time / (320 - position as u32);
                    self.emit_tone(Self::pixel_luma_tone(pixel, duration))
                }
                None => {
                    self.state = EncoderState::EvenLumaToChroma;
                    self.emit_tone(Tone(Robot36::BLACK, Robot36::BLANK_DURATION * 2 / 3))
                }
            },
            EncoderState::EvenLumaToChroma => {
                self.state = EncoderState::EvenChroma(0);
                self.emit_tone(Tone(Robot36::SEPARATOR, Robot36::BLANK_DURATION / 3))
            }
            EncoderState::EvenChroma(position) => {
                match (self.current_row.get(position), self.next_row.get(position)) {
                    (Some(current_row_pixel), Some(next_row_pixel)) => {
                        self.state = EncoderState::EvenChroma(position + 1);
                        let fixed_remaining = Robot36::SYNC_DURATION + Robot36::BACK_PORCH_DURATION;
                        let remaining_chroma_time = self.remaining_line_time - fixed_remaining;
                        let duration = remaining_chroma_time / (320 - position as u32);
                        self.emit_tone(Self::pixel_chroma_red_tone(
                            current_row_pixel,
                            next_row_pixel,
                            duration,
                        ))
                    }
                    _ => {
                        self.state = EncoderState::EvenToOdd;
                        self.emit_tone(Tone(Robot36::SYNC, Robot36::SYNC_DURATION))
                    }
                }
            }
            EncoderState::EvenToOdd => {
                self.state = EncoderState::OddLuma(0);
                let tone = self.emit_tone(Tone(Robot36::BLACK, Robot36::BACK_PORCH_DURATION));
                self.remaining_line_time = Robot36::LINE_DURATION;
                tone
            }
            EncoderState::OddLuma(position) => match self.next_row.get(position) {
                Some(pixel) => {
                    self.state = EncoderState::OddLuma(position + 1);
                    let fixed_remaining = Robot36::BLANK_DURATION
                        + Robot36::SYNC_DURATION
                        + Robot36::BACK_PORCH_DURATION;
                    let total_pixel_time = Robot36::LINE_DURATION - fixed_remaining;
                    let chroma_time = total_pixel_time / 3;
                    let remaining_luma_time =
                        self.remaining_line_time - fixed_remaining - chroma_time;
                    let duration = remaining_luma_time / (320 - position as u32);
                    self.emit_tone(Self::pixel_luma_tone(pixel, duration))
                }
                None => {
                    self.state = EncoderState::OddLumaToChroma;
                    self.emit_tone(Tone(Robot36::WHITE, Robot36::BLANK_DURATION * 2 / 3))
                }
            },
            EncoderState::OddLumaToChroma => {
                self.state = EncoderState::OddChroma(0);
                self.emit_tone(Tone(Robot36::SEPARATOR, Robot36::BLANK_DURATION / 3))
            }
            EncoderState::OddChroma(position) => {
                match (self.current_row.get(position), self.next_row.get(position)) {
                    (Some(current_row_pixel), Some(next_row_pixel)) => {
                        self.state = EncoderState::OddChroma(position + 1);
                        let fixed_remaining = Robot36::SYNC_DURATION + Robot36::BACK_PORCH_DURATION;
                        let remaining_chroma_time = self.remaining_line_time - fixed_remaining;
                        let duration = remaining_chroma_time / (320 - position as u32);
                        self.emit_tone(Self::pixel_chroma_blue_tone(
                            current_row_pixel,
                            next_row_pixel,
                            duration,
                        ))
                    }
                    _ => {
                        self.state = EncoderState::OddToEven;
                        self.emit_tone(Tone(Robot36::SYNC, Robot36::SYNC_DURATION))
                    }
                }
            }
            EncoderState::OddToEven => {
                self.state = EncoderState::LineGap;
                self.fetch_next_rows();
                self.emit_tone(Tone(Robot36::BLACK, Robot36::BACK_PORCH_DURATION))
            }
            EncoderState::LineGap => {
                self.state = EncoderState::EvenLuma(0);
                let gap_length = self.remaining_line_time;
                self.remaining_line_time = Robot36::LINE_DURATION;
                self.emit_tone(Tone(Hz!(0), gap_length))
            }
            EncoderState::Done => None,
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use image::GenericImageView;
    use std::f64::consts::PI;
    use std::vec::Vec;

    use super::*;
    use crate::modes::robot36::Robot36;
    use crate::synthesizer::Tone;
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

    #[test]
    fn test_encode_robot36_against_golden_file_with_tolerance() {
        let img = image::open("examples/patch.png").expect("Failed to open examples/patch.png");
        let (width, height) = img.dimensions();

        assert_eq!(width, 320);
        assert_eq!(height, 240);

        let mut pixels = std::vec![[RgbPixel::new(0, 0, 0); 320]; 240];

        img.pixels().for_each(|(x, y, rgba)| {
            pixels[y as usize][x as usize] = RgbPixel::new(rgba[0], rgba[1], rgba[2]);
        });

        let encoder = Robot36Encoder::new(pixels.into_iter().flatten());

        let sample_rate = 48000.0;
        let mut phase: f64 = 0.0;
        let mut sample_adjust: f64 = 0.0;
        let mut generated_samples = Vec::new();

        encoder.for_each(|tone| {
            let freq = tone.0.hz() as f64;
            let duration_sec = tone.1.micros() as f64 / 1_000_000.0;

            let exact_samples = (duration_sec * sample_rate) + sample_adjust;
            let num_samples = exact_samples.round() as usize;
            sample_adjust = exact_samples - num_samples as f64;

            let phase_increment = 2.0 * PI * freq / sample_rate;

            (0..num_samples).for_each(|_| {
                let sample = (phase.sin() * i16::MAX as f64) as i16;
                generated_samples.push(sample);
                phase = (phase + phase_increment) % (2.0 * PI);
            });
        });

        let mut reader = hound::WavReader::open("examples/patch-robot36.wav")
            .expect("Failed to open golden WAV");
        let golden_samples: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();

        // Duration tolerance: Ensure total length is within 0.1 seconds (4800 samples)
        let len_diff = (generated_samples.len() as isize - golden_samples.len() as isize).abs();
        assert!(
            len_diff < 4800,
            "Duration deviation exceeded. Length difference: {} samples",
            len_diff
        );

        let min_len = generated_samples.len().min(golden_samples.len());

        // Amplitude tolerance: Allows capturing phase drift without hard-failing instantly.
        let amplitude_tolerance = 4000;
        let mut error_count = 0;

        for i in 0..min_len {
            if (generated_samples[i] as i32 - golden_samples[i] as i32).abs() > amplitude_tolerance
            {
                error_count += 1;
            }
        }

        let error_rate = error_count as f64 / min_len as f64;

        // Frequency tolerance: Allow up to 10% of the transmission to exceed the drift threshold
        assert!(
            error_rate < 0.10,
            "Phase/Frequency deviation exceeded. Error rate: {:.2}%",
            error_rate * 100.0
        );
    }
}
