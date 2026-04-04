//! B/W (Black & White) SSTV Modes
//!
//! This module implements the grayscale SSTV modes:
//! - BW8: 8s transmission, 160×120 resolution
//! - BW12: 12s transmission, 160×120 resolution
//!
//! Reference: QSSTV's modebw.cpp and sstvparam.cpp
//!
//! Line structure (from modebw.cpp txSetupLine):
//! 1. Back porch (bp) - 1500 Hz
//! 2. Grayscale pixels - mapped to frequency range
//! 3. Front porch (fp) - 1500 Hz
//! 4. Sync pulse - 1200 Hz

use super::{ColorSpace, LineData, RgbPixel, SSTVMode};
use crate::synthesizer::Tone;

/// Standard SSTV frequencies (Hz)
const FREQ_SYNC: f64 = 1200.0;
const FREQ_BLACK: f64 = 1500.0;
const FREQ_WHITE: f64 = 2300.0;

/// Generate frequency tones for grayscale pixel values
/// Maps 0-255 grayscale to 1500-2300 Hz
fn generate_grayscale_tones(pixels: &[u8], pixel_duration: f64) -> Vec<Tone> {
    pixels
        .iter()
        .map(|&gray| {
            // Map grayscale 0-255 to frequency 1500-2300 Hz
            let freq = FREQ_BLACK + (gray as f64 / 255.0) * (FREQ_WHITE - FREQ_BLACK);
            Tone::new(freq, pixel_duration)
        })
        .collect()
}

/// Convert RGB to grayscale using standard luminance weights
/// Y = 0.30*R + 0.59*G + 0.11*B (same as QSSTV's getLineBW)
fn rgb_to_grayscale(pixel: &RgbPixel) -> u8 {
    let y = (30 * pixel.r as u32 + 59 * pixel.g as u32 + 11 * pixel.b as u32) / 100;
    y.min(255) as u8
}

// ============================================================================
// BW8 Mode - 8 second B/W mode
// ============================================================================

/// BW8 SSTV Mode
///
/// Parameters from QSSTV sstvparam.cpp:
/// - Image time: 8.02800 seconds
/// - Resolution: 160×120
/// - VIS code: 0x82
/// - Sync: 6.0 ms
/// - Front porch: 0.5 ms (TX: 1.0 ms)
/// - Back porch: 0.5 ms (TX: 0.5 ms)
#[derive(Debug, Clone, Copy)]
pub struct Bw8;

impl SSTVMode for Bw8 {
    fn name(&self) -> &'static str {
        "B/W 8"
    }

    fn short_name(&self) -> &'static str {
        "BW8"
    }

    fn vis_code(&self) -> u8 {
        0x82
    }

    fn resolution(&self) -> (u32, u32) {
        (160, 120)
    }

    fn data_lines(&self) -> u32 {
        120
    }

    fn image_time(&self) -> f64 {
        8.02800
    }

    fn sync_duration(&self) -> f64 {
        0.00600 // 6.0 ms (synct from sstvparam.cpp)
    }

    fn front_porch(&self) -> f64 {
        0.00100 // 1.0 ms (fpt TX from sstvparam.cpp)
    }

    fn back_porch(&self) -> f64 {
        0.00050 // 0.5 ms (bpt TX from sstvparam.cpp)
    }

    fn blank_duration(&self) -> f64 {
        0.0 // No blanking for BW modes
    }

    fn color_space(&self) -> ColorSpace {
        ColorSpace::Grayscale
    }

    /// Calculate visible line length
    /// From modebw.cpp: visibleLineLength = lineLength - fp - bp - syncDuration
    fn visible_line_length(&self) -> f64 {
        let line_length = self.image_time() / self.data_lines() as f64;
        line_length - self.front_porch() - self.back_porch() - self.sync_duration()
    }

    /// Encode a single B/W scan line
    ///
    /// TX sequence from modebw.cpp txSetupLine:
    /// Case 0: bp (back porch, 1500 Hz)
    /// Case 1: grayscale pixels
    /// Case 2: fp (front porch, 1500 Hz)
    /// Case 3: sync (1200 Hz)
    fn encode_line(&self, line_data: &LineData, _line_num: u32) -> Vec<Tone> {
        let mut tones = Vec::new();

        let visible = self.visible_line_length();
        let num_pixels = self.resolution().0;
        let pixel_duration = visible / num_pixels as f64;

        // Convert RGB pixels to grayscale
        let grayscale: Vec<u8> = line_data.pixels.iter().map(rgb_to_grayscale).collect();

        // Case 0: Back porch (1500 Hz)
        tones.push(Tone::new(FREQ_BLACK, self.back_porch()));

        // Case 1: Grayscale pixels
        tones.extend(generate_grayscale_tones(&grayscale, pixel_duration));

        // Case 2: Front porch (1500 Hz)
        tones.push(Tone::new(FREQ_BLACK, self.front_porch()));

        // Case 3: Sync pulse (1200 Hz)
        tones.push(Tone::new(FREQ_SYNC, self.sync_duration()));

        tones
    }
}

// ============================================================================
// BW12 Mode - 12 second B/W mode
// ============================================================================

/// BW12 SSTV Mode
///
/// Parameters from QSSTV sstvparam.cpp:
/// - Image time: 12.00100 seconds
/// - Resolution: 160×120
/// - VIS code: 0x86
/// - Sync: 6.0 ms
/// - Front porch: 0.5 ms (TX: 1.0 ms)
/// - Back porch: 1.0 ms (TX: 1.0 ms)
#[derive(Debug, Clone, Copy)]
pub struct Bw12;

impl SSTVMode for Bw12 {
    fn name(&self) -> &'static str {
        "B/W 12"
    }

    fn short_name(&self) -> &'static str {
        "BW12"
    }

    fn vis_code(&self) -> u8 {
        0x86
    }

    fn resolution(&self) -> (u32, u32) {
        (160, 120)
    }

    fn data_lines(&self) -> u32 {
        120
    }

    fn image_time(&self) -> f64 {
        12.00100
    }

    fn sync_duration(&self) -> f64 {
        0.00600 // 6.0 ms
    }

    fn front_porch(&self) -> f64 {
        0.00100 // 1.0 ms (TX)
    }

    fn back_porch(&self) -> f64 {
        0.00100 // 1.0 ms (TX)
    }

    fn blank_duration(&self) -> f64 {
        0.0 // No blanking for BW modes
    }

    fn color_space(&self) -> ColorSpace {
        ColorSpace::Grayscale
    }

    /// Calculate visible line length
    /// From modebw.cpp: visibleLineLength = lineLength - fp - bp - syncDuration
    fn visible_line_length(&self) -> f64 {
        let line_length = self.image_time() / self.data_lines() as f64;
        line_length - self.front_porch() - self.back_porch() - self.sync_duration()
    }

    /// Encode a single B/W scan line (same structure as BW8)
    fn encode_line(&self, line_data: &LineData, _line_num: u32) -> Vec<Tone> {
        let mut tones = Vec::new();

        let visible = self.visible_line_length();
        let num_pixels = self.resolution().0;
        let pixel_duration = visible / num_pixels as f64;

        // Convert RGB pixels to grayscale
        let grayscale: Vec<u8> = line_data.pixels.iter().map(rgb_to_grayscale).collect();

        // Case 0: Back porch (1500 Hz)
        tones.push(Tone::new(FREQ_BLACK, self.back_porch()));

        // Case 1: Grayscale pixels
        tones.extend(generate_grayscale_tones(&grayscale, pixel_duration));

        // Case 2: Front porch (1500 Hz)
        tones.push(Tone::new(FREQ_BLACK, self.front_porch()));

        // Case 3: Sync pulse (1200 Hz)
        tones.push(Tone::new(FREQ_SYNC, self.sync_duration()));

        tones
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bw8_parameters() {
        let mode = Bw8;
        assert_eq!(mode.name(), "B/W 8");
        assert_eq!(mode.short_name(), "BW8");
        assert_eq!(mode.vis_code(), 0x82);
        assert_eq!(mode.resolution(), (160, 120));
        assert_eq!(mode.data_lines(), 120);
        assert_eq!(mode.color_space(), ColorSpace::Grayscale);

        // Verify timing adds up correctly
        let line_duration = mode.line_duration();
        let expected = mode.back_porch()
            + mode.visible_line_length()
            + mode.front_porch()
            + mode.sync_duration();
        assert!((line_duration - expected).abs() < 0.0001);
    }

    #[test]
    fn test_bw12_parameters() {
        let mode = Bw12;
        assert_eq!(mode.name(), "B/W 12");
        assert_eq!(mode.short_name(), "BW12");
        assert_eq!(mode.vis_code(), 0x86);
        assert_eq!(mode.resolution(), (160, 120));
        assert_eq!(mode.data_lines(), 120);
        assert_eq!(mode.color_space(), ColorSpace::Grayscale);
    }

    #[test]
    fn test_rgb_to_grayscale() {
        // Pure white should give 255
        let white = RgbPixel::new(255, 255, 255);
        assert_eq!(rgb_to_grayscale(&white), 255);

        // Pure black should give 0
        let black = RgbPixel::new(0, 0, 0);
        assert_eq!(rgb_to_grayscale(&black), 0);

        // Mid gray
        let gray = RgbPixel::new(128, 128, 128);
        let result = rgb_to_grayscale(&gray);
        assert!(result >= 127 && result <= 129);
    }

    #[test]
    fn test_bw8_line_encoding() {
        let mode = Bw8;

        // Create a test line with 160 gray pixels
        let pixels: Vec<RgbPixel> = (0..160)
            .map(|i| RgbPixel::new(i as u8, i as u8, i as u8))
            .collect();
        let line_data = LineData::new(pixels);

        let tones = mode.encode_line(&line_data, 0);

        // Should have: 1 bp + 160 pixels + 1 fp + 1 sync = 163 tones
        assert_eq!(tones.len(), 163);

        // First tone is back porch (1500 Hz)
        assert!((tones[0].freq - FREQ_BLACK).abs() < 0.01);

        // Last tone is sync (1200 Hz)
        assert!((tones[162].freq - FREQ_SYNC).abs() < 0.01);

        // Second to last is front porch (1500 Hz)
        assert!((tones[161].freq - FREQ_BLACK).abs() < 0.01);
    }

    #[test]
    fn test_grayscale_frequency_mapping() {
        let pixels = vec![0, 127, 255];
        let tones = generate_grayscale_tones(&pixels, 0.001);

        // Black (0) should map to 1500 Hz
        assert!((tones[0].freq - FREQ_BLACK).abs() < 0.01);

        // Mid gray (127) should map to ~1900 Hz
        let mid_freq = (FREQ_BLACK + FREQ_WHITE) / 2.0;
        assert!((tones[1].freq - mid_freq).abs() < 10.0);

        // White (255) should map to 2300 Hz
        assert!((tones[2].freq - FREQ_WHITE).abs() < 0.01);
    }
}
