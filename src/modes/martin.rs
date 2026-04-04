//! Martin mode implementations (Martin 1 and Martin 2)
//!
//! Martin modes use GBR (Green-Blue-Red) color order with sync at the end of each line.
//!
//! Reference: qsstv/src/sstv/modes/modegbr.cpp
//! Line structure (from modegbr.cpp:128-167 txSetupLine):
//!   1. Green pixels
//!   2. Blank @ 1500 Hz
//!   3. Blue pixels
//!   4. Blank @ 1500 Hz
//!   5. Red pixels
//!   6. Front porch @ 1500 Hz
//!   7. Sync @ 1200 Hz
//!   8. Back porch @ 1500 Hz

use crate::constants::{pixel_to_freq, FREQ_BLACK, FREQ_SYNC};
use crate::modes::{ColorSpace, LineData, SSTVMode};
use crate::synthesizer::Tone;

/// Calculate visible line length for Martin modes
/// Derived from qsstv/src/sstv/modes/modegbr.cpp:37-40
/// visibleLineLength = (lineLength - fp - bp - 2*blank - syncDuration) / 3
fn martin_visible_line_length(
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
    pixels.iter().map(|&p| Tone::new(pixel_to_freq(p), pixel_duration)).collect()
}

/// Encode a line in Martin format (GBR order)
/// Derived from qsstv/src/sstv/modes/modegbr.cpp:128-167
fn encode_martin_line(
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

    // 4. Blank @ 1500 Hz (case 3)
    tones.push(Tone::new(FREQ_BLACK, blank));

    // 5. Red pixels (case 4)
    tones.extend(generate_pixel_tones(&red, pixel_duration));

    // 6. Front porch @ 1500 Hz (case 5)
    tones.push(Tone::new(FREQ_BLACK, fp));

    // 7. Sync @ 1200 Hz (case 6)
    tones.push(Tone::new(FREQ_SYNC, sync));

    // 8. Back porch @ 1500 Hz (case 7)
    tones.push(Tone::new(FREQ_BLACK, bp));

    tones
}

// =============================================================================
// Martin 1
// =============================================================================

/// Martin 1 mode
///
/// Parameters from qsstv/src/sstv/sstvparam.cpp line 309:
/// {"Martin 1", "M1", M1, 114.29700, 320, 256, 256, 0xAC,
///  0.00500, 0.00080, 0.00050, 0.00050,  // rx: sync, fp, bp, blank
///  0.00500, 0.00080, 0.00000, 0.00050,  // tx: synct, fpt, bpt, blankt
///  0., 1900, 400}
#[derive(Debug, Clone, Copy)]
pub struct Martin1;

impl SSTVMode for Martin1 {
    fn name(&self) -> &'static str {
        "Martin 1"
    }

    fn short_name(&self) -> &'static str {
        "M1"
    }

    /// VIS code 0xAC (172)
    /// Derived from qsstv/src/sstv/sstvparam.cpp:309
    fn vis_code(&self) -> u8 {
        0xAC
    }

    /// Resolution 320x256
    /// Derived from qsstv/src/sstv/sstvparam.cpp:309
    fn resolution(&self) -> (u32, u32) {
        (320, 256)
    }

    fn data_lines(&self) -> u32 {
        256
    }

    /// Image time 114.297 seconds
    /// Derived from qsstv/src/sstv/sstvparam.cpp:309
    fn image_time(&self) -> f64 {
        114.29700
    }

    /// Sync duration 4.862ms (0.004862s)
    /// Derived from qsstv/src/sstv/sstvparam.cpp:309 - synct=0.00500
    fn sync_duration(&self) -> f64 {
        0.004862
    }

    /// Front porch 0.572ms
    /// Derived from qsstv/src/sstv/sstvparam.cpp:309 - fpt=0.00080
    fn front_porch(&self) -> f64 {
        0.000572
    }

    /// Back porch 0.0ms (Martin 1 uses 0 for TX)
    /// Derived from qsstv/src/sstv/sstvparam.cpp:309 - bpt=0.00000
    fn back_porch(&self) -> f64 {
        0.0
    }

    /// Blank duration 0.572ms
    /// Derived from qsstv/src/sstv/sstvparam.cpp:309 - blankt=0.00050
    fn blank_duration(&self) -> f64 {
        0.000572
    }

    fn color_space(&self) -> ColorSpace {
        ColorSpace::Rgb
    }

    fn visible_line_length(&self) -> f64 {
        martin_visible_line_length(
            self.image_time(),
            self.data_lines(),
            self.front_porch(),
            self.back_porch(),
            self.blank_duration(),
            self.sync_duration(),
        )
    }

    fn encode_line(&self, line_data: &LineData, _line_num: u32) -> Vec<Tone> {
        encode_martin_line(
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
// Martin 2
// =============================================================================

/// Martin 2 mode
///
/// Parameters from qsstv/src/sstv/sstvparam.cpp line 310:
/// {"Martin 2", "M2", M2, 58.06400, 320, 256, 256, 0x28,
///  0.00500, 0.00080, 0.00050, 0.00050,
///  0.00500, 0.00080, 0.00000, 0.00050,
///  0., 1900, 400}
#[derive(Debug, Clone, Copy)]
pub struct Martin2;

impl SSTVMode for Martin2 {
    fn name(&self) -> &'static str {
        "Martin 2"
    }

    fn short_name(&self) -> &'static str {
        "M2"
    }

    /// VIS code 0x28 (40)
    /// Derived from qsstv/src/sstv/sstvparam.cpp:310
    fn vis_code(&self) -> u8 {
        0x28
    }

    /// Resolution 320x256
    fn resolution(&self) -> (u32, u32) {
        (320, 256)
    }

    fn data_lines(&self) -> u32 {
        256
    }

    /// Image time 58.064 seconds
    /// Derived from qsstv/src/sstv/sstvparam.cpp:310
    fn image_time(&self) -> f64 {
        58.06400
    }

    fn sync_duration(&self) -> f64 {
        0.004862
    }

    fn front_porch(&self) -> f64 {
        0.000572
    }

    fn back_porch(&self) -> f64 {
        0.0
    }

    fn blank_duration(&self) -> f64 {
        0.000572
    }

    fn color_space(&self) -> ColorSpace {
        ColorSpace::Rgb
    }

    fn visible_line_length(&self) -> f64 {
        martin_visible_line_length(
            self.image_time(),
            self.data_lines(),
            self.front_porch(),
            self.back_porch(),
            self.blank_duration(),
            self.sync_duration(),
        )
    }

    fn encode_line(&self, line_data: &LineData, _line_num: u32) -> Vec<Tone> {
        encode_martin_line(
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
    fn test_martin1_params() {
        let m1 = Martin1;
        assert_eq!(m1.vis_code(), 0xAC);
        assert_eq!(m1.resolution(), (320, 256));
        assert!((m1.image_time() - 114.297).abs() < 0.001);
    }

    #[test]
    fn test_martin2_params() {
        let m2 = Martin2;
        assert_eq!(m2.vis_code(), 0x28);
        assert_eq!(m2.resolution(), (320, 256));
        assert!((m2.image_time() - 58.064).abs() < 0.001);
    }

    #[test]
    fn test_martin1_line_encoding() {
        let m1 = Martin1;
        let pixels: Vec<RgbPixel> = (0..320).map(|i| RgbPixel::new(i as u8, 128, 64)).collect();
        let line_data = LineData::new(pixels);
        let tones = m1.encode_line(&line_data, 0);
        
        // Should have: 320 green + blank + 320 blue + blank + 320 red + fp + sync + bp
        // = 960 pixels + 5 control tones = 965 tones
        assert_eq!(tones.len(), 965);
    }

    #[test]
    fn test_visible_line_length() {
        let m1 = Martin1;
        let vll = m1.visible_line_length();
        // Each color channel should be roughly 1/3 of total line time minus overheads
        // Line time = 114.297 / 256 ≈ 0.446 seconds
        // Visible should be around 0.146 seconds per channel
        assert!(vll > 0.14 && vll < 0.16, "Visible line length: {}", vll);
    }
}
