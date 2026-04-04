use crate::dsp::Nco;
use crate::mode::{Mode, Robot36};

/// 910ms VIS header sequence for Robot 36 (Mode 8: 0b00001000).
/// Formatted as (Frequency in Hz, Duration in ms).
const VIS_SEQUENCE: [(f32, f32); 13] = [
    (1900.0, 300.0), // Leader tone
    (1200.0, 10.0),  // Break
    (1900.0, 300.0), // Leader tone
    (1200.0, 30.0),  // Start bit
    (1300.0, 30.0),  // Bit 0 (0)
    (1300.0, 30.0),  // Bit 1 (0)
    (1300.0, 30.0),  // Bit 2 (0)
    (1100.0, 30.0),  // Bit 3 (1)
    (1300.0, 30.0),  // Bit 4 (0)
    (1300.0, 30.0),  // Bit 5 (0)
    (1300.0, 30.0),  // Bit 6 (0)
    (1100.0, 30.0),  // Parity (Even)
    (1200.0, 30.0),  // Stop bit
];

/// Converts an RGB pixel iterator into a continuous stream of audio samples.
pub struct Encoder<I> {
    pixel_iter: I,
    nco: Nco,
    state: EncoderState,
    sample_rate: u32,
    current_line: u16,
    line_buffer_y: [f32; 320],
    line_buffer_color: [f32; 320],
}

enum EncoderState {
    Vis {
        seq_idx: usize,
        samples_left: u32,
    },
    Sync {
        samples_left: u32,
    },
    SyncPorch {
        samples_left: u32,
    },
    Luminance {
        samples_left: u32,
        total_samples: u32,
    },
    ColorPorch {
        samples_left: u32,
    },
    Chrominance {
        samples_left: u32,
        total_samples: u32,
    },
    Done,
}

impl<I> Encoder<I>
where
    I: Iterator<Item = (u8, u8, u8)>,
{
    pub fn new(pixel_iter: I, sample_rate: u32) -> Self {
        let samples_left = Self::ms_to_samples(sample_rate, VIS_SEQUENCE[0].1);
        Self {
            pixel_iter,
            nco: Nco::new(sample_rate),
            state: EncoderState::Vis {
                seq_idx: 0,
                samples_left,
            },
            sample_rate,
            current_line: 0,
            line_buffer_y: [0.0; 320],
            line_buffer_color: [0.0; 320],
        }
    }

    fn load_next_line(&mut self) -> bool {
        if self.current_line >= 240 {
            return false;
        }

        for i in 0..320 {
            if let Some((r, g, b)) = self.pixel_iter.next() {
                let r = r as f32;
                let g = g as f32;
                let b = b as f32;

                self.line_buffer_y[i] = 16.0 + 0.003906 * (65.738 * r + 129.057 * g + 25.064 * b);

                if self.current_line % 2 == 0 {
                    self.line_buffer_color[i] =
                        128.0 + 0.003906 * (112.439 * r - 94.154 * g - 18.285 * b);
                } else {
                    self.line_buffer_color[i] =
                        128.0 + 0.003906 * (-37.945 * r - 74.494 * g + 112.439 * b);
                }
            } else {
                return false;
            }
        }
        true
    }

    fn ms_to_samples(sample_rate: u32, ms: f32) -> u32 {
        (sample_rate as f32 * (ms / 1000.0)) as u32
    }

    fn val_to_freq(val: f32) -> f32 {
        1500.0 + (libm::fmaxf(0.0, libm::fminf(255.0, val)) / 255.0) * 800.0
    }
}

impl<I> Iterator for Encoder<I>
where
    I: Iterator<Item = (u8, u8, u8)>,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample_rate = self.sample_rate;

        let (freq, advance_state) = match &mut self.state {
            EncoderState::Vis {
                seq_idx,
                samples_left,
            } => {
                *samples_left -= 1;
                (VIS_SEQUENCE[*seq_idx].0, *samples_left == 0)
            }
            EncoderState::Sync { samples_left } => {
                *samples_left -= 1;
                (Robot36::SYNC_FREQ_HZ, *samples_left == 0)
            }
            EncoderState::SyncPorch { samples_left } => {
                *samples_left -= 1;
                (1500.0, *samples_left == 0)
            }
            EncoderState::Luminance {
                samples_left,
                total_samples,
            } => {
                *samples_left -= 1;
                let progress = (*total_samples - *samples_left) as f32 / *total_samples as f32;
                let pixel_idx = ((progress * 320.0) as usize).min(319);
                let freq = Self::val_to_freq(self.line_buffer_y[pixel_idx]);
                (freq, *samples_left == 0)
            }
            EncoderState::ColorPorch { samples_left } => {
                *samples_left -= 1;
                let freq = if self.current_line % 2 == 0 {
                    1500.0
                } else {
                    1900.0
                };
                (freq, *samples_left == 0)
            }
            EncoderState::Chrominance {
                samples_left,
                total_samples,
            } => {
                *samples_left -= 1;
                let progress = (*total_samples - *samples_left) as f32 / *total_samples as f32;
                let pixel_idx = ((progress * 320.0) as usize).min(319);
                let freq = Self::val_to_freq(self.line_buffer_color[pixel_idx]);
                (freq, *samples_left == 0)
            }
            EncoderState::Done => return None,
        };

        if advance_state {
            self.state = match self.state {
                EncoderState::Vis { seq_idx, .. } => {
                    if seq_idx + 1 < VIS_SEQUENCE.len() {
                        EncoderState::Vis {
                            seq_idx: seq_idx + 1,
                            samples_left: Self::ms_to_samples(
                                sample_rate,
                                VIS_SEQUENCE[seq_idx + 1].1,
                            ),
                        }
                    } else if !self.load_next_line() {
                        EncoderState::Done
                    } else {
                        EncoderState::Sync {
                            samples_left: Self::ms_to_samples(sample_rate, 9.0),
                        }
                    }
                }
                EncoderState::Sync { .. } => EncoderState::SyncPorch {
                    samples_left: Self::ms_to_samples(sample_rate, 3.0),
                },
                EncoderState::SyncPorch { .. } => {
                    let total = Self::ms_to_samples(sample_rate, 88.0);
                    EncoderState::Luminance {
                        samples_left: total,
                        total_samples: total,
                    }
                }
                EncoderState::Luminance { .. } => EncoderState::ColorPorch {
                    samples_left: Self::ms_to_samples(sample_rate, 1.5),
                },
                EncoderState::ColorPorch { .. } => {
                    let total = Self::ms_to_samples(sample_rate, 44.0);
                    EncoderState::Chrominance {
                        samples_left: total,
                        total_samples: total,
                    }
                }
                EncoderState::Chrominance { .. } => {
                    self.current_line += 1;
                    if self.current_line >= 240 {
                        EncoderState::Done
                    } else if !self.load_next_line() {
                        EncoderState::Done
                    } else {
                        EncoderState::Sync {
                            samples_left: Self::ms_to_samples(sample_rate, 9.0),
                        }
                    }
                }
                EncoderState::Done => EncoderState::Done,
            };
        }

        Some(self.nco.next_sample(freq))
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use image::GenericImageView;
    use std::vec::Vec;

    #[test]
    fn test_encode_architecture_and_length() {
        let img = image::open("patch.png").expect("Failed to load image");
        let pixels: Vec<(u8, u8, u8)> = img
            .pixels()
            .map(|(_, _, rgba)| (rgba[0], rgba[1], rgba[2]))
            .collect();

        // 44100 Hz strictly required to compare to the ARISS-EA web output
        let sample_rate = 44100;
        let encoder = Encoder::new(pixels.into_iter(), sample_rate);
        let generated_samples: Vec<f32> = encoder.collect();

        // Total Expected Duration:
        // VIS Header: 910 ms
        // 240 Lines * (9 + 3 + 88 + 1.5 + 44) ms = 34,920 ms
        // Total Time: 35,830 ms.
        // 35.83 * 44100 = 1,580,103 samples exact (ignoring floating truncations).
        assert_eq!(
            generated_samples.len(),
            1580103,
            "Generated audio does not match the deterministic mathematical spec duration."
        );

        // Note: The ARISS-EA web encoder file ('patch-robot36.wav') is 1,627,491 samples.
        // It appends ~1.07 seconds of padded VOX/silence and utilizes different anti-alias
        // filters. Comparing raw wave MSE will always fail here. To properly test audio validity
        // against it, you must process the wave through a phase-locked loop (PLL) decoder.
    }
}
