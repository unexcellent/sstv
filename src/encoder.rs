use crate::error::{Result, SstvError};
use crate::modes::{ColorSpace, LineData, Mode, RgbPixel, SSTVMode};
use crate::synthesizer::Synthesizer;
use crate::vis::{generate_complete_vis, tones_to_samples};

/// Image data for encoding
pub struct ImageData {
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// RGB pixel data (row-major order)
    pub pixels: Vec<RgbPixel>,
}

impl ImageData {
    /// Create new image data
    pub fn new(width: u32, height: u32, pixels: Vec<RgbPixel>) -> Self {
        Self {
            width,
            height,
            pixels,
        }
    }

    /// Create from raw RGB bytes
    pub fn from_rgb_bytes(width: u32, height: u32, bytes: &[u8]) -> Result<Self> {
        if bytes.len() != (width * height * 3) as usize {
            return Err(SstvError::EncodingError(format!(
                "Expected {} bytes, got {}",
                width * height * 3,
                bytes.len()
            )));
        }

        let pixels: Vec<RgbPixel> = bytes
            .chunks(3)
            .map(|chunk| RgbPixel::new(chunk[0], chunk[1], chunk[2]))
            .collect();

        Ok(Self::new(width, height, pixels))
    }

    /// Create from raw RGBA bytes (alpha is ignored)
    pub fn from_rgba_bytes(width: u32, height: u32, bytes: &[u8]) -> Result<Self> {
        if bytes.len() != (width * height * 4) as usize {
            return Err(SstvError::EncodingError(format!(
                "Expected {} bytes, got {}",
                width * height * 4,
                bytes.len()
            )));
        }

        let pixels: Vec<RgbPixel> = bytes
            .chunks(4)
            .map(|chunk| RgbPixel::new(chunk[0], chunk[1], chunk[2]))
            .collect();

        Ok(Self::new(width, height, pixels))
    }

    /// Get a line of pixel data
    pub fn get_line(&self, line_num: u32) -> LineData {
        let start = (line_num * self.width) as usize;
        let end = start + self.width as usize;
        LineData::new(self.pixels[start..end].to_vec())
    }
}

/// SSTV Encoder
pub struct Encoder {
    /// The SSTV mode to use
    mode: Box<dyn SSTVMode>,
    /// Audio sample rate
    sample_rate: u32,
    /// FM Synthesizer
    synthesizer: Synthesizer,
}

impl Encoder {
    /// Create a new encoder for the specified mode
    pub fn new(mode: Mode, sample_rate: u32) -> Result<Self> {
        if sample_rate == 0 {
            return Err(SstvError::InvalidSampleRate(sample_rate));
        }

        Ok(Self {
            mode: mode.get_impl(),
            sample_rate,
            synthesizer: Synthesizer::new(sample_rate),
        })
    }

    /// Get the mode being used
    pub fn mode(&self) -> &dyn SSTVMode {
        self.mode.as_ref()
    }

    /// Get the sample rate
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get the expected image resolution for this mode
    pub fn expected_resolution(&self) -> (u32, u32) {
        self.mode.resolution()
    }

    /// Encode an image to audio samples
    ///
    /// The image must match the mode's expected resolution.
    /// Returns audio samples as f64 values (can be converted to i16 or f32).
    pub fn encode(&mut self, image: &ImageData) -> Result<Vec<f64>> {
        let (expected_width, expected_height) = self.mode.resolution();

        if image.width != expected_width || image.height != expected_height {
            return Err(SstvError::DimensionMismatch {
                expected_width,
                expected_height,
                actual_width: image.width,
                actual_height: image.height,
            });
        }

        self.synthesizer.reset();
        let mut samples = Vec::new();

        // Generate VIS code and preamble
        let vis_tones = generate_complete_vis(self.mode.vis_code());
        samples.extend(tones_to_samples(&mut self.synthesizer, &vis_tones));

        // Encode image lines
        let color_space = self.mode.color_space();
        let display_lines = expected_height;

        match color_space {
            ColorSpace::Rgb => {
                // RGB modes: encode each line individually
                for line_num in 0..display_lines {
                    let line_data = image.get_line(line_num);
                    let line_tones = self.mode.encode_line(&line_data, line_num);
                    samples.extend(tones_to_samples(&mut self.synthesizer, &line_tones));
                }
            }
            ColorSpace::Yuv => {
                // YUV modes - check for 2-line interleaving
                if self.mode.uses_line_pairs() {
                    // 2-line interleaving (PD modes, Robot 36)
                    // Process display lines in pairs
                    let num_pairs = display_lines / 2;
                    for pair_num in 0..num_pairs {
                        let even_line_num = pair_num * 2;
                        let odd_line_num = pair_num * 2 + 1;

                        let even_line = image.get_line(even_line_num);
                        let odd_line = image.get_line(odd_line_num);

                        let pair_tones =
                            self.mode
                                .encode_line_pair(&even_line, &odd_line, even_line_num);
                        samples.extend(tones_to_samples(&mut self.synthesizer, &pair_tones));
                    }
                } else {
                    // No interleaving (like Robot 72)
                    for line_num in 0..display_lines {
                        let line_data = image.get_line(line_num);
                        let line_tones = self.mode.encode_line(&line_data, line_num);
                        samples.extend(tones_to_samples(&mut self.synthesizer, &line_tones));
                    }
                }
            }
            ColorSpace::Grayscale => {
                // Grayscale modes: single channel per line
                for line_num in 0..display_lines {
                    let line_data = image.get_line(line_num);
                    let line_tones = self.mode.encode_line(&line_data, line_num);
                    samples.extend(tones_to_samples(&mut self.synthesizer, &line_tones));
                }
            }
        }

        Ok(samples)
    }

    /// Convert f64 samples to i16 for WAV output
    pub fn samples_to_i16(samples: &[f64]) -> Vec<i16> {
        samples
            .iter()
            .map(|&s| {
                // Clamp to i16 range and convert
                let clamped = s.clamp(-32768.0, 32767.0);
                clamped as i16
            })
            .collect()
    }

    /// Convert f64 samples to f32 for audio output
    pub fn samples_to_f32(samples: &[f64]) -> Vec<f32> {
        samples
            .iter()
            .map(|&s| {
                // Normalize to -1.0 to 1.0 range
                (s / 32768.0) as f32
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;
    use hound::WavReader;
    use std::vec::Vec;

    #[test]
    fn test_encode_against_golden_file() {
        let golden_reader = WavReader::open("examples/patch-robot36.wav").unwrap();

        let sample_rate = golden_reader.spec().sample_rate;
        let golden_samples: Vec<i16> = golden_reader
            .into_samples::<i16>()
            .map(|s| s.unwrap())
            .collect();

        let img = image::open("examples/patch.png").unwrap().to_rgb8();
        let (width, height) = img.dimensions();
        let image_data = ImageData::from_rgb_bytes(width, height, &img.into_raw()).unwrap();

        let mut encoder = Encoder::new(Mode::Robot36, sample_rate).unwrap();
        let generated_f64 = encoder.encode(&image_data).unwrap();
        let generated_samples = Encoder::samples_to_i16(&generated_f64);

        assert_eq!(generated_samples.len(), golden_samples.len());

        // A small delta is permitted due to precision differences between floating point implementations
        // across differing architectures.
        for (actual, gold) in generated_samples.into_iter().zip(golden_samples) {
            assert!(
                (actual - gold).abs() <= 2,
                "Sample mismatch: generated {}, expected {}",
                actual,
                gold
            );
        }
    }
}
