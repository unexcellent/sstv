//! Robot 36 SSTV encoder module
//!
//! Includes all necessary components for encoding an RGB image into Robot 36
//! SSTV audio samples.

use alloc::vec;
use alloc::vec::Vec;
use core::f64::consts::PI;

use crate::Error;
use crate::Result;
use crate::image::ImageData;
use crate::image::LineData;
use crate::image::YuvPixel;

const FREQ_SYNC: f64 = 1200.0;
const FREQ_BLACK: f64 = 1500.0;
const FREQ_WHITE: f64 = 2300.0;
const FREQ_VIS_BIT1: f64 = 1100.0;
const FREQ_VIS_BIT0: f64 = 1300.0;
const FREQ_VIS_BREAK: f64 = 1200.0;
const FREQ_SEPARATOR: f64 = 1900.0;
const FREQ_EVEN_MARKER: f64 = 2300.0;

const VIS_BIT_DURATION: f64 = 0.030;
const AUDIO_AMPLITUDE: f64 = 8000.0;
const SINE_TABLE_LEN: usize = 2048;

const ROBOT36_VIS: u8 = 0x88;
const ROBOT36_WIDTH: u32 = 320;
const ROBOT36_HEIGHT: u32 = 240;
const ROBOT36_IMAGE_TIME: f64 = 36.002;
const ROBOT36_SYNC_DUR: f64 = 0.009;
const ROBOT36_BP_DUR: f64 = 0.003;
const ROBOT36_BLANK_DUR: f64 = 0.0054;

#[inline]
fn pixel_to_freq(pixel: u8) -> f64 {
    FREQ_BLACK + (pixel as f64 * (FREQ_WHITE - FREQ_BLACK) / 255.0)
}

#[derive(Debug, Clone, Copy)]
struct Tone {
    freq: f64,
    duration: f64,
}

impl Tone {
    fn new(freq: f64, duration: f64) -> Self {
        Self { freq, duration }
    }
}

struct Synthesizer {
    phase: f64,
    sample_rate: f64,
    sine_table: [f64; SINE_TABLE_LEN],
    adjust: f64,
}

impl Synthesizer {
    fn new(sample_rate: u32) -> Self {
        let mut sine_table = [0.0; SINE_TABLE_LEN];
        for (i, v) in sine_table.iter_mut().enumerate() {
            *v = libm::sin(i as f64 * PI * 2.0 / SINE_TABLE_LEN as f64) * AUDIO_AMPLITUDE;
        }

        Self {
            phase: 0.0,
            sample_rate: sample_rate as f64,
            sine_table,
            adjust: 0.0,
        }
    }

    #[inline]
    fn next_sample(&mut self, freq: f64) -> f64 {
        let phase_increment = (freq / self.sample_rate) * SINE_TABLE_LEN as f64;
        self.phase = (self.phase + phase_increment) % SINE_TABLE_LEN as f64;
        let index = (self.phase + 0.5) as usize % SINE_TABLE_LEN;
        self.sine_table[index]
    }

    fn generate_tone(&mut self, duration: f64, freq: f64, concat: bool) -> Vec<f64> {
        if !concat {
            self.adjust = 0.0;
        }

        let num_samples = ((duration + self.adjust) * self.sample_rate + 0.5) as usize;
        self.adjust += duration - (num_samples as f64 / self.sample_rate);

        let mut samples = Vec::with_capacity(num_samples);
        for _ in 0..num_samples {
            samples.push(self.next_sample(freq));
        }
        samples
    }
}

fn tones_to_samples(synth: &mut Synthesizer, tones: &[Tone]) -> Vec<f64> {
    let mut samples = Vec::new();
    let mut first = true;
    for tone in tones {
        let tone_samples = synth.generate_tone(tone.duration, tone.freq, !first);
        samples.extend(tone_samples);
        first = false;
    }
    samples
}

fn generate_complete_vis(vis_code: u8) -> Vec<Tone> {
    let mut tones = vec![
        Tone::new(1900.0, 0.1),
        Tone::new(1500.0, 0.1),
        Tone::new(1900.0, 0.1),
        Tone::new(1500.0, 0.1),
        Tone::new(2300.0, 0.1),
        Tone::new(1500.0, 0.1),
        Tone::new(2300.0, 0.1),
        Tone::new(1500.0, 0.1),
        Tone::new(1900.0, 0.3),
        Tone::new(1200.0, 0.01),
        Tone::new(1900.0, 0.3),
        Tone::new(FREQ_VIS_BREAK, VIS_BIT_DURATION),
    ];

    let mut code = vis_code;
    for _ in 0..8 {
        let freq = if (code & 1) == 1 {
            FREQ_VIS_BIT1
        } else {
            FREQ_VIS_BIT0
        };
        tones.push(Tone::new(freq, VIS_BIT_DURATION));
        code >>= 1;
    }

    tones.push(Tone::new(FREQ_VIS_BREAK, VIS_BIT_DURATION));
    tones
}

fn generate_pixel_tones(values: &[u8], pixel_duration: f64) -> Vec<Tone> {
    values
        .iter()
        .map(|&v| Tone::new(pixel_to_freq(v), pixel_duration))
        .collect()
}

fn calc_robot36_visible_line_length() -> f64 {
    let half_lines = 240.0;
    let line_length = ROBOT36_IMAGE_TIME / half_lines;
    (line_length - ROBOT36_BP_DUR - ROBOT36_BLANK_DUR - ROBOT36_SYNC_DUR) / 3.0
}

fn encode_robot36_line_pair(even_line: &LineData, odd_line: &LineData) -> Vec<Tone> {
    let visible = calc_robot36_visible_line_length();
    let y_pixel_duration = (2.0 * visible) / ROBOT36_WIDTH as f64;
    let uv_pixel_duration = visible / ROBOT36_WIDTH as f64;

    let even_yuv: Vec<YuvPixel> = even_line.pixels.iter().map(|p| p.to_yuv()).collect();
    let odd_yuv: Vec<YuvPixel> = odd_line.pixels.iter().map(|p| p.to_yuv()).collect();

    let v_values: Vec<u8> = even_yuv
        .iter()
        .zip(odd_yuv.iter())
        .map(|(e, o)| ((e.chroma_red as u16 + o.chroma_red as u16) / 2) as u8)
        .collect();
    let u_values: Vec<u8> = even_yuv
        .iter()
        .zip(odd_yuv.iter())
        .map(|(e, o)| ((e.chroma_blue as u16 + o.chroma_blue as u16) / 2) as u8)
        .collect();

    let y_first: Vec<u8> = even_yuv.iter().map(|p| p.luma).collect();
    let y_second: Vec<u8> = odd_yuv.iter().map(|p| p.luma).collect();

    let mut tones = Vec::new();

    tones.extend(generate_pixel_tones(&y_first, y_pixel_duration));
    tones.push(Tone::new(FREQ_BLACK, (2.0 * ROBOT36_BLANK_DUR) / 3.0));
    tones.push(Tone::new(FREQ_SEPARATOR, ROBOT36_BLANK_DUR / 3.0));
    tones.extend(generate_pixel_tones(&v_values, uv_pixel_duration));
    tones.push(Tone::new(FREQ_SYNC, ROBOT36_SYNC_DUR));
    tones.push(Tone::new(FREQ_BLACK, ROBOT36_BP_DUR));

    tones.extend(generate_pixel_tones(&y_second, y_pixel_duration));
    tones.push(Tone::new(FREQ_EVEN_MARKER, (2.0 * ROBOT36_BLANK_DUR) / 3.0));
    tones.push(Tone::new(FREQ_SEPARATOR, ROBOT36_BLANK_DUR / 3.0));
    tones.extend(generate_pixel_tones(&u_values, uv_pixel_duration));
    tones.push(Tone::new(FREQ_SYNC, ROBOT36_SYNC_DUR));
    tones.push(Tone::new(FREQ_BLACK, ROBOT36_BP_DUR));

    tones
}

/// Encodes `ImageData` into an array of float audio samples representing a Robot36 transmission.
pub fn encode_robot36(image: &ImageData, sample_rate: u32) -> Result<Vec<f64>> {
    if image.width != ROBOT36_WIDTH || image.height != ROBOT36_HEIGHT {
        return Err(Error::DimensionMismatch {
            expected_width: ROBOT36_WIDTH,
            expected_height: ROBOT36_HEIGHT,
            actual_width: image.width,
            actual_height: image.height,
        });
    }

    let mut synthesizer = Synthesizer::new(sample_rate);
    let mut samples = Vec::new();

    let vis_tones = generate_complete_vis(ROBOT36_VIS);
    samples.extend(tones_to_samples(&mut synthesizer, &vis_tones));

    let num_pairs = ROBOT36_HEIGHT / 2;
    for pair_num in 0..num_pairs {
        let even_line = image.get_line(pair_num * 2);
        let odd_line = image.get_line(pair_num * 2 + 1);

        let pair_tones = encode_robot36_line_pair(&even_line, &odd_line);
        samples.extend(tones_to_samples(&mut synthesizer, &pair_tones));
    }

    Ok(samples)
}

/// Converts normalized f64 audio samples to 16-bit PCM integer samples.
pub fn samples_to_i16(samples: &[f64]) -> Vec<i16> {
    samples
        .iter()
        .map(|&s| s.clamp(-32768.0, 32767.0) as i16)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hound::WavReader;

    #[test]
    fn test_encode_robot36_against_golden_file() {
        let golden_reader = WavReader::open("examples/patch-robot36.wav").unwrap();

        let sample_rate = golden_reader.spec().sample_rate;
        let golden_samples: Vec<i16> = golden_reader
            .into_samples::<i16>()
            .map(|s| s.unwrap())
            .collect();

        let img = image::open("examples/patch.png").unwrap().to_rgb8();
        let (width, height) = img.dimensions();
        let image_data = ImageData::from_rgb_bytes(width, height, &img.into_raw()).unwrap();

        let generated_f64 = encode_robot36(&image_data, sample_rate).unwrap();
        let generated_samples = samples_to_i16(&generated_f64);

        assert_eq!(generated_samples.len(), golden_samples.len());

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
