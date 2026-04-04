//! Scottie mode implementations (Scottie 1, Scottie 2, Scottie DX)
//!
//! Scottie modes use a different line structure with sync in the middle.
//! Color order: Green-Blue-Sync-Red
//!
//! Reference: qsstv/src/sstv/modes/modegbr2.cpp
//! Line structure (from modegbr2.cpp:161-201 txSetupLine):
//!   1. Green pixels
//!   2. Blank @ 1500 Hz
//!   3. Blue pixels
//!   4. Front porch @ 1500 Hz
//!   5. Sync @ 1200 Hz
//!   6. Back porch @ 1500 Hz
//!   7. Red pixels
//!   8. Blank @ 1500 Hz

use crate::constants::{FREQ_BLACK, FREQ_SYNC, pixel_to_freq};
use crate::modes::{ColorSpace, LineData, SSTVMode};
use crate::synthesizer::Tone;

/// Calculate visible line length for Scottie modes
/// Derived from qsstv/src/sstv/modes/modegbr2.cpp:44-47
/// visibleLineLength = (lineLength - fp - bp - 2*blank - syncDuration) / 3
fn scottie_visible_line_length(
    image_time: f64,
    data_lines: u32,
    fp: f64,
    bp: f64,
    blank: f64,
    sync: f64,
) -> f64 {
    let line_length = image_time / data_lines as f64;
    (line_length - fp - bp - 2.0 * blank - sync) / 3.0
}

/// Generate pixel tones for a color channel
fn generate_pixel_tones(pixels: &[u8], pixel_duration: f64) -> Vec<Tone> {
    pixels
        .iter()
        .map(|&p| Tone::new(pixel_to_freq(p), pixel_duration))
        .collect()
}

/// Encode a line in Scottie format (Green-Blue-Sync-Red)
/// Derived from qsstv/src/sstv/modes/modegbr2.cpp:161-201
fn encode_scottie_line(
    line_data: &LineData,
    visible_line_length: f64,
    blank: f64,
    fp: f64,
    sync: f64,
    bp: f64,
    num_pixels: u32,
) -> Vec<Tone> {
    let pixel_duration = visible_line_length / num_pixels as f64;
    let mut tones = Vec::new();

    // Extract color channels
    let green: Vec<u8> = line_data.pixels.iter().map(|p| p.g).collect();
    let blue: Vec<u8> = line_data.pixels.iter().map(|p| p.b).collect();
    let red: Vec<u8> = line_data.pixels.iter().map(|p| p.r).collect();

    // 1. Green pixels (case 0 in txSetupLine)
    tones.extend(generate_pixel_tones(&green, pixel_duration));

    // 2. Blank @ 1500 Hz (case 1)
    tones.push(Tone::new(FREQ_BLACK, blank));

    // 3. Blue pixels (case 2)
    tones.extend(generate_pixel_tones(&blue, pixel_duration));

    // 4. Front porch @ 1500 Hz (case 3)
    tones.push(Tone::new(FREQ_BLACK, fp));

    // 5. Sync @ 1200 Hz (case 4)
    tones.push(Tone::new(FREQ_SYNC, sync));

    // 6. Back porch @ 1500 Hz (case 5)
    tones.push(Tone::new(FREQ_BLACK, bp));

    // 7. Red pixels (case 6)
    tones.extend(generate_pixel_tones(&red, pixel_duration));

    // 8. Blank @ 1500 Hz (case 7)
    tones.push(Tone::new(FREQ_BLACK, blank));

    tones
}

// =============================================================================
// Scottie 1
// =============================================================================

/// Scottie 1 mode
///
/// Parameters from qsstv/src/sstv/sstvparam.cpp line 311:
/// {"Scottie 1", "S1", S1, 109.63250, 320, 256, 256, 0x3C,
///  0.00900, 0.00010, 0.00125, 0.00125,  // rx
///  0.00900, 0.00080, 0.00080, 0.00125,  // tx
///  0., 1900, 400}
#[derive(Debug, Clone, Copy)]
pub struct Scottie1;

impl SSTVMode for Scottie1 {
    fn name(&self) -> &'static str {
        "Scottie 1"
    }

    fn short_name(&self) -> &'static str {
        "S1"
    }

    /// VIS code 0x3C (60)
    /// Derived from qsstv/src/sstv/sstvparam.cpp:311
    fn vis_code(&self) -> u8 {
        0x3C
    }

    /// Resolution 320x256
    fn resolution(&self) -> (u32, u32) {
        (320, 256)
    }

    fn data_lines(&self) -> u32 {
        256
    }

    /// Image time 109.6325 seconds
    /// Derived from qsstv/src/sstv/sstvparam.cpp:311
    fn image_time(&self) -> f64 {
        109.63250
    }

    /// Sync duration 9.0ms
    /// Derived from qsstv/src/sstv/sstvparam.cpp:311 - synct=0.00900
    fn sync_duration(&self) -> f64 {
        0.009
    }

    /// Front porch 0.8ms
    /// Derived from qsstv/src/sstv/sstvparam.cpp:311 - fpt=0.00080
    fn front_porch(&self) -> f64 {
        0.0008
    }

    /// Back porch 0.8ms
    /// Derived from qsstv/src/sstv/sstvparam.cpp:311 - bpt=0.00080
    fn back_porch(&self) -> f64 {
        0.0008
    }

    /// Blank duration 1.25ms
    /// Derived from qsstv/src/sstv/sstvparam.cpp:311 - blankt=0.00125
    fn blank_duration(&self) -> f64 {
        0.00125
    }

    fn color_space(&self) -> ColorSpace {
        ColorSpace::Rgb
    }

    fn visible_line_length(&self) -> f64 {
        scottie_visible_line_length(
            self.image_time(),
            self.data_lines(),
            self.front_porch(),
            self.back_porch(),
            self.blank_duration(),
            self.sync_duration(),
        )
    }

    fn encode_line(&self, line_data: &LineData, _line_num: u32) -> Vec<Tone> {
        encode_scottie_line(
            line_data,
            self.visible_line_length(),
            self.blank_duration(),
            self.front_porch(),
            self.sync_duration(),
            self.back_porch(),
            self.resolution().0,
        )
    }
}

// =============================================================================
// Scottie 2
// =============================================================================

/// Scottie 2 mode
///
/// Parameters from qsstv/src/sstv/sstvparam.cpp line 312:
/// {"Scottie 2", "S2", S2, 71.09450, 320, 256, 256, 0xB8,
///  0.00900, 0.00010, 0.00150, 0.00150,  // rx
///  0.00900, 0.00000, 0.00110, 0.00125,  // tx
///  0., 1900, 400}
#[derive(Debug, Clone, Copy)]
pub struct Scottie2;

impl SSTVMode for Scottie2 {
    fn name(&self) -> &'static str {
        "Scottie 2"
    }

    fn short_name(&self) -> &'static str {
        "S2"
    }

    /// VIS code 0xB8 (184)
    /// Derived from qsstv/src/sstv/sstvparam.cpp:312
    fn vis_code(&self) -> u8 {
        0xB8
    }

    fn resolution(&self) -> (u32, u32) {
        (320, 256)
    }

    fn data_lines(&self) -> u32 {
        256
    }

    /// Image time 71.0945 seconds
    fn image_time(&self) -> f64 {
        71.09450
    }

    fn sync_duration(&self) -> f64 {
        0.009
    }

    /// Front porch 0.0ms for TX
    /// Derived from qsstv/src/sstv/sstvparam.cpp:312 - fpt=0.00000
    fn front_porch(&self) -> f64 {
        0.0
    }

    /// Back porch 1.1ms
    /// Derived from qsstv/src/sstv/sstvparam.cpp:312 - bpt=0.00110
    fn back_porch(&self) -> f64 {
        0.0011
    }

    /// Blank duration 1.25ms
    fn blank_duration(&self) -> f64 {
        0.00125
    }

    fn color_space(&self) -> ColorSpace {
        ColorSpace::Rgb
    }

    fn visible_line_length(&self) -> f64 {
        scottie_visible_line_length(
            self.image_time(),
            self.data_lines(),
            self.front_porch(),
            self.back_porch(),
            self.blank_duration(),
            self.sync_duration(),
        )
    }

    fn encode_line(&self, line_data: &LineData, _line_num: u32) -> Vec<Tone> {
        encode_scottie_line(
            line_data,
            self.visible_line_length(),
            self.blank_duration(),
            self.front_porch(),
            self.sync_duration(),
            self.back_porch(),
            self.resolution().0,
        )
    }
}

// =============================================================================
// Scottie DX
// =============================================================================

/// Scottie DX mode
///
/// Parameters from qsstv/src/sstv/sstvparam.cpp line 313:
/// {"Scottie DX", "SDX", SDX, 268.89380, 320, 256, 256, 0xCC,
///  0.00900, 0.00000, 0.00000, 0.00100,  // rx
///  0.00900, 0.00000, 0.00000, 0.00100,  // tx
///  0., 1900, 400}
#[derive(Debug, Clone, Copy)]
pub struct ScottieDx;

impl SSTVMode for ScottieDx {
    fn name(&self) -> &'static str {
        "Scottie DX"
    }

    fn short_name(&self) -> &'static str {
        "SDX"
    }

    /// VIS code 0xCC (204)
    /// Derived from qsstv/src/sstv/sstvparam.cpp:313
    fn vis_code(&self) -> u8 {
        0xCC
    }

    fn resolution(&self) -> (u32, u32) {
        (320, 256)
    }

    fn data_lines(&self) -> u32 {
        256
    }

    /// Image time 268.8938 seconds
    fn image_time(&self) -> f64 {
        268.89380
    }

    fn sync_duration(&self) -> f64 {
        0.009
    }

    fn front_porch(&self) -> f64 {
        0.0
    }

    fn back_porch(&self) -> f64 {
        0.0
    }

    /// Blank duration 1.0ms
    fn blank_duration(&self) -> f64 {
        0.001
    }

    fn color_space(&self) -> ColorSpace {
        ColorSpace::Rgb
    }

    fn visible_line_length(&self) -> f64 {
        scottie_visible_line_length(
            self.image_time(),
            self.data_lines(),
            self.front_porch(),
            self.back_porch(),
            self.blank_duration(),
            self.sync_duration(),
        )
    }

    fn encode_line(&self, line_data: &LineData, _line_num: u32) -> Vec<Tone> {
        encode_scottie_line(
            line_data,
            self.visible_line_length(),
            self.blank_duration(),
            self.front_porch(),
            self.sync_duration(),
            self.back_porch(),
            self.resolution().0,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::RgbPixel;

    #[test]
    fn test_scottie1_params() {
        let s1 = Scottie1;
        assert_eq!(s1.vis_code(), 0x3C);
        assert_eq!(s1.resolution(), (320, 256));
        assert!((s1.image_time() - 109.6325).abs() < 0.001);
    }

    #[test]
    fn test_scottie2_params() {
        let s2 = Scottie2;
        assert_eq!(s2.vis_code(), 0xB8);
        assert!((s2.image_time() - 71.0945).abs() < 0.001);
    }

    #[test]
    fn test_scottie_dx_params() {
        let sdx = ScottieDx;
        assert_eq!(sdx.vis_code(), 0xCC);
        assert!((sdx.image_time() - 268.8938).abs() < 0.001);
    }

    #[test]
    fn test_scottie1_line_encoding() {
        let s1 = Scottie1;
        let pixels: Vec<RgbPixel> = (0..320).map(|i| RgbPixel::new(i as u8, 128, 64)).collect();
        let line_data = LineData::new(pixels);
        let tones = s1.encode_line(&line_data, 0);

        // Should have: 320 green + blank + 320 blue + fp + sync + bp + 320 red + blank
        // = 960 pixels + 5 control tones = 965 tones
        assert_eq!(tones.len(), 965);
    }
}
