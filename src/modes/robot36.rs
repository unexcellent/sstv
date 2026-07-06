use crate::{
    Error, Hz, Result,
    image::{RgbPixel, YuvPixel},
    modes::Mode,
    synthesizer::Tone,
};
use core::array;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Row {
    Even,
    Odd,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Yuv {
    Luma,
    Chroma,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum EncoderState {
    NotStarted,
    Header(usize),
    Pixel(usize, Row, Yuv),
    EvenLumaToChroma(usize),
    EvenToOdd(usize),
    OddLumaToChroma(usize),
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
                    *self = Self::Pixel(0, Row::Even, Yuv::Luma);
                }
            }
            Self::Pixel(pos, row, yuv) => {
                if *pos < Mode::Robot36.image_width() as usize - 1 {
                    *self = Self::Pixel(*pos + 1, *row, *yuv);
                } else {
                    match (row, yuv) {
                        (Row::Even, Yuv::Luma) => *self = Self::EvenLumaToChroma(0),
                        (Row::Even, Yuv::Chroma) => *self = Self::EvenToOdd(0),
                        (Row::Odd, Yuv::Luma) => *self = Self::OddLumaToChroma(0),
                        (Row::Odd, Yuv::Chroma) => *self = Self::OddToEven(0),
                    }
                }
            }
            Self::EvenLumaToChroma(pos) => {
                if *pos == 0 {
                    *self = Self::EvenLumaToChroma(1);
                } else {
                    *self = Self::Pixel(0, Row::Even, Yuv::Chroma);
                }
            }
            Self::EvenToOdd(pos) => {
                if *pos == 0 {
                    *self = Self::EvenToOdd(1);
                } else {
                    *self = Self::Pixel(0, Row::Odd, Yuv::Luma);
                }
            }
            Self::OddLumaToChroma(pos) => {
                if *pos == 0 {
                    *self = Self::OddLumaToChroma(1);
                } else {
                    *self = Self::Pixel(0, Row::Odd, Yuv::Chroma);
                }
            }
            Self::OddToEven(pos) => {
                if *pos < 2 {
                    *self = Self::OddToEven(*pos + 1);
                } else {
                    *self = Self::Pixel(0, Row::Even, Yuv::Luma);
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
            EncoderState::Pixel(pos, Row::Even, Yuv::Luma) => Some(self.even_row[pos].luma_tone(
                Self::mode().black_frequency(),
                Self::mode().white_frequency(),
                Self::mode().pixel_luma_duration(),
            )),
            EncoderState::EvenLumaToChroma(pos) => match pos {
                0 => Some(Tone::new(
                    Self::mode().black_frequency(),
                    Self::mode().blank_duration() * 2 / 3,
                )),
                _ => Some(Tone::new(
                    Self::mode().separator_frequency(),
                    Self::mode().blank_duration() / 3,
                )),
            },
            EncoderState::Pixel(pos, Row::Even, Yuv::Chroma) => {
                Some(self.even_row[pos].chroma_red_tone(
                    Self::mode().black_frequency(),
                    Self::mode().white_frequency(),
                    Self::mode().pixel_chroma_duration(),
                ))
            }
            EncoderState::EvenToOdd(pos) => match pos {
                0 => Some(Tone::new(
                    Self::mode().sync_frequency(),
                    Self::mode().sync_duration(),
                )),
                _ => Some(Tone::new(
                    Self::mode().black_frequency(),
                    Self::mode().back_porch_duration(),
                )),
            },
            EncoderState::Pixel(pos, Row::Odd, Yuv::Luma) => Some(self.odd_row[pos].luma_tone(
                Self::mode().black_frequency(),
                Self::mode().white_frequency(),
                Self::mode().pixel_luma_duration(),
            )),
            EncoderState::OddLumaToChroma(pos) => match pos {
                0 => Some(Tone::new(
                    Self::mode().white_frequency(),
                    Self::mode().blank_duration() * 2 / 3,
                )),
                _ => Some(Tone::new(
                    Self::mode().separator_frequency(),
                    Self::mode().blank_duration() / 3,
                )),
            },
            EncoderState::Pixel(pos, Row::Odd, Yuv::Chroma) => {
                Some(self.odd_row[pos].chroma_blue_tone(
                    Self::mode().black_frequency(),
                    Self::mode().white_frequency(),
                    Self::mode().pixel_chroma_duration(),
                ))
            }
            EncoderState::OddToEven(pos) => match pos {
                0 => Some(Tone::new(
                    Self::mode().sync_frequency(),
                    Self::mode().sync_duration(),
                )),
                1 => Some(Tone::new(
                    Self::mode().black_frequency(),
                    Self::mode().back_porch_duration(),
                )),
                _ => {
                    self.fetch_next_rows();
                    if let EncoderState::Finished = self.state {
                        None
                    } else {
                        Some(Tone::new(Hz!(0), Self::mode().line_gap_duration()))
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

    /// Load an image as raw row-major RGB bytes, with its dimensions.
    fn rgb_bytes(path: &str) -> (std::vec::Vec<u8>, (u32, u32)) {
        let image = image::open(path).expect("open image").to_rgb8();
        let dimensions = image.dimensions();
        (image.into_raw(), dimensions)
    }

    /// Mean absolute per-channel error between two equal-length byte buffers.
    fn mean_abs_error(a: &[u8], b: &[u8]) -> f64 {
        assert_eq!(a.len(), b.len());
        let sum: u64 = a
            .iter()
            .zip(b)
            .map(|(x, y)| (*x as i32 - *y as i32).unsigned_abs() as u64)
            .sum();
        sum as f64 / a.len() as f64
    }

    /// Validate the encoder by decoding its output with the independent `sstv`
    /// decoder and comparing the result to the source image.
    ///
    /// The reference is produced by `python3 scripts/decode_reference.py` and
    /// stored in the (gitignored) `local/` directory; the test fails if it is
    /// absent.
    #[test]
    fn encoder_output_decodes_via_sstv() {
        const REFERENCE: &str = "local/encoder-robot36-sstv.png";
        assert!(
            std::path::Path::new(REFERENCE).exists(),
            "{REFERENCE} not found. Generate it with `python3 scripts/decode_reference.py`",
        );

        let (decoded, decoded_dimensions) = rgb_bytes(REFERENCE);
        let (original, original_dimensions) = rgb_bytes("examples/patch.png");
        assert_eq!(
            decoded_dimensions, original_dimensions,
            "decoded image dimensions differ from the source",
        );

        let error = mean_abs_error(&original, &decoded);
        std::eprintln!("encoder -> sstv decode error: {error}");
        // A foreign decoder introduces its own timing/colourspace interpretation,
        // so this is tolerance-based. Measured ~13.6; 20 leaves margin while still
        // catching a broken encoder (which would spike far higher).
        assert!(
            error < 20.0,
            "encoder -> sstv decode error {error} too high"
        );
    }
}
