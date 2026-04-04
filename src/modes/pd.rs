//! PD mode implementations (PD 120, PD 180, PD 290)
//!
//! PD modes use YUV color space with 2-line interleaving.
//! Each data line encodes 2 display lines.
//!
//! Reference: qsstv/src/sstv/modes/modepd.cpp
//! Line structure (from modepd.cpp:149-197 txSetupLine):
//!   0: Y-odd pixels
//!   1: Blank @ 1500 Hz
//!   2: R-Y (V) pixels
//!   3: Blank @ 1500 Hz
//!   4: B-Y (U) pixels
//!   5: Blank @ 1500 Hz
//!   6: Y-even pixels
//!   7: Front porch @ 1500 Hz
//!   8: Sync @ 1200 Hz
//!   9: Back porch @ 1500 Hz

use crate::constants::{FREQ_BLACK, FREQ_SYNC, pixel_to_freq};
use crate::modes::{ColorSpace, LineData, SSTVMode, YuvPixel};
use crate::synthesizer::Tone;

/// Generate pixel tones for a color channel
fn generate_pixel_tones(values: &[u8], pixel_duration: f64) -> Vec<Tone> {
    values
        .iter()
        .map(|&v| Tone::new(pixel_to_freq(v), pixel_duration))
        .collect()
}

/// Calculate visible line length for PD modes
/// Derived from qsstv/src/sstv/modes/modepd.cpp:44-47
/// visibleLineLength = (lineLength - fp - bp - syncDuration) / 4
fn pd_visible_line_length(image_time: f64, data_lines: u32, fp: f64, bp: f64, sync: f64) -> f64 {
    let line_length = image_time / data_lines as f64;
    (line_length - fp - bp - sync) / 4.0
}

/// Encode a line pair in PD format (Y-odd, V, U, Y-even)
/// Derived from qsstv/src/sstv/modes/modepd.cpp:149-197
fn encode_pd_line_pair(
    even_line: &LineData,
    odd_line: &LineData,
    visible_line_length: f64,
    blank: f64,
    fp: f64,
    sync: f64,
    bp: f64,
    num_pixels: u32,
) -> Vec<Tone> {
    let pixel_duration = visible_line_length / num_pixels as f64;
    let mut tones = Vec::new();

    // Convert to YUV
    let even_yuv: Vec<YuvPixel> = even_line.pixels.iter().map(|p| p.to_yuv()).collect();
    let odd_yuv: Vec<YuvPixel> = odd_line.pixels.iter().map(|p| p.to_yuv()).collect();

    // Extract channels
    let y_odd: Vec<u8> = odd_yuv.iter().map(|p| p.y).collect();
    let y_even: Vec<u8> = even_yuv.iter().map(|p| p.y).collect();

    // Average UV between the two lines
    let v_values: Vec<u8> = even_yuv
        .iter()
        .zip(odd_yuv.iter())
        .map(|(e, o)| ((e.cr as u16 + o.cr as u16) / 2) as u8)
        .collect();
    let u_values: Vec<u8> = even_yuv
        .iter()
        .zip(odd_yuv.iter())
        .map(|(e, o)| ((e.cb as u16 + o.cb as u16) / 2) as u8)
        .collect();

    // 0: Y-odd pixels (case 0 in txSetupLine)
    tones.extend(generate_pixel_tones(&y_odd, pixel_duration));

    // 1: Blank @ 1500 Hz (case 1)
    if blank > 0.0 {
        tones.push(Tone::new(FREQ_BLACK, blank));
    }

    // 2: R-Y (V) pixels (case 2)
    tones.extend(generate_pixel_tones(&v_values, pixel_duration));

    // 3: Blank @ 1500 Hz (case 3)
    if blank > 0.0 {
        tones.push(Tone::new(FREQ_BLACK, blank));
    }

    // 4: B-Y (U) pixels (case 4)
    tones.extend(generate_pixel_tones(&u_values, pixel_duration));

    // 5: Blank @ 1500 Hz (case 5)
    if blank > 0.0 {
        tones.push(Tone::new(FREQ_BLACK, blank));
    }

    // 6: Y-even pixels (case 6)
    tones.extend(generate_pixel_tones(&y_even, pixel_duration));

    // 7: Front porch @ 1500 Hz (case 7)
    tones.push(Tone::new(FREQ_BLACK, fp));

    // 8: Sync @ 1200 Hz (case 8)
    tones.push(Tone::new(FREQ_SYNC, sync));

    // 9: Back porch @ 1500 Hz (case 9)
    tones.push(Tone::new(FREQ_BLACK, bp));

    tones
}

// =============================================================================
// PD 120
// =============================================================================

/// PD 120 mode
///
/// Parameters from qsstv/src/sstv/sstvparam.cpp line 327:
/// {"PD120", "PD120", PD120, 126.11150, 640, 496, 248, 0x5F,
///  0.02000, 0.00000, 0.00208, 0.00000,  // rx
///  0.02000, 0.00000, 0.00230, 0.00000,  // tx
///  0., 1900, 400}
#[derive(Debug, Clone, Copy)]
pub struct Pd120;

impl SSTVMode for Pd120 {
    fn name(&self) -> &'static str {
        "PD 120"
    }

    fn short_name(&self) -> &'static str {
        "PD120"
    }

    /// VIS code 0x5F (95)
    /// Derived from qsstv/src/sstv/sstvparam.cpp:327
    fn vis_code(&self) -> u8 {
        0x5F
    }

    /// Resolution 640x496
    fn resolution(&self) -> (u32, u32) {
        (640, 496)
    }

    /// 248 data lines (each encodes 2 display lines)
    fn data_lines(&self) -> u32 {
        248
    }

    /// Image time 126.1115 seconds
    fn image_time(&self) -> f64 {
        126.11150
    }

    /// Sync duration 20.0ms
    fn sync_duration(&self) -> f64 {
        0.020
    }

    /// Front porch 0.0ms
    fn front_porch(&self) -> f64 {
        0.0
    }

    /// Back porch 2.3ms
    /// Derived from qsstv/src/sstv/sstvparam.cpp:327 - bpt=0.00230
    fn back_porch(&self) -> f64 {
        0.0023
    }

    /// Blank duration 0.0ms (PD modes don't use blanking between channels)
    fn blank_duration(&self) -> f64 {
        0.0
    }

    fn color_space(&self) -> ColorSpace {
        ColorSpace::Yuv
    }

    fn uses_line_pairs(&self) -> bool {
        true
    }

    fn visible_line_length(&self) -> f64 {
        pd_visible_line_length(
            self.image_time(),
            self.data_lines(),
            self.front_porch(),
            self.back_porch(),
            self.sync_duration(),
        )
    }

    fn encode_line(&self, line_data: &LineData, _line_num: u32) -> Vec<Tone> {
        // For single line, duplicate it for the pair
        self.encode_line_pair(line_data, line_data, 0)
    }

    fn encode_line_pair(
        &self,
        even_line: &LineData,
        odd_line: &LineData,
        _line_num: u32,
    ) -> Vec<Tone> {
        encode_pd_line_pair(
            even_line,
            odd_line,
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
// PD 180
// =============================================================================

/// PD 180 mode
///
/// Parameters from qsstv/src/sstv/sstvparam.cpp line 329:
/// {"PD180", "PD180", PD180, 187.06450, 640, 496, 248, 0x60,
///  0.02000, 0.00000, 0.00200, 0.00000,  // rx
///  0.02000, 0.00000, 0.00230, 0.00000,  // tx
///  0., 1900, 400}
#[derive(Debug, Clone, Copy)]
pub struct Pd180;

impl SSTVMode for Pd180 {
    fn name(&self) -> &'static str {
        "PD 180"
    }

    fn short_name(&self) -> &'static str {
        "PD180"
    }

    /// VIS code 0x60 (96)
    /// Derived from qsstv/src/sstv/sstvparam.cpp:329
    fn vis_code(&self) -> u8 {
        0x60
    }

    fn resolution(&self) -> (u32, u32) {
        (640, 496)
    }

    fn data_lines(&self) -> u32 {
        248
    }

    /// Image time 187.0645 seconds
    fn image_time(&self) -> f64 {
        187.06450
    }

    fn sync_duration(&self) -> f64 {
        0.020
    }

    fn front_porch(&self) -> f64 {
        0.0
    }

    fn back_porch(&self) -> f64 {
        0.0023
    }

    fn blank_duration(&self) -> f64 {
        0.0
    }

    fn color_space(&self) -> ColorSpace {
        ColorSpace::Yuv
    }

    fn uses_line_pairs(&self) -> bool {
        true
    }

    fn visible_line_length(&self) -> f64 {
        pd_visible_line_length(
            self.image_time(),
            self.data_lines(),
            self.front_porch(),
            self.back_porch(),
            self.sync_duration(),
        )
    }

    fn encode_line(&self, line_data: &LineData, _line_num: u32) -> Vec<Tone> {
        self.encode_line_pair(line_data, line_data, 0)
    }

    fn encode_line_pair(
        &self,
        even_line: &LineData,
        odd_line: &LineData,
        _line_num: u32,
    ) -> Vec<Tone> {
        encode_pd_line_pair(
            even_line,
            odd_line,
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
// PD 290
// =============================================================================

/// PD 290 mode
///
/// Parameters from qsstv/src/sstv/sstvparam.cpp line 331:
/// {"PD290", "PD290", PD290, 288.70200, 800, 616, 308, 0xDE,
///  0.02000, 0.00000, 0.00200, 0.00000,  // rx
///  0.02000, 0.00000, 0.00230, 0.00000,  // tx
///  0., 1900, 400}
#[derive(Debug, Clone, Copy)]
pub struct Pd290;

impl SSTVMode for Pd290 {
    fn name(&self) -> &'static str {
        "PD 290"
    }

    fn short_name(&self) -> &'static str {
        "PD290"
    }

    /// VIS code 0xDE (222)
    /// Derived from qsstv/src/sstv/sstvparam.cpp:331
    fn vis_code(&self) -> u8 {
        0xDE
    }

    /// Resolution 800x616
    fn resolution(&self) -> (u32, u32) {
        (800, 616)
    }

    /// 308 data lines
    fn data_lines(&self) -> u32 {
        308
    }

    /// Image time 288.702 seconds
    fn image_time(&self) -> f64 {
        288.70200
    }

    fn sync_duration(&self) -> f64 {
        0.020
    }

    fn front_porch(&self) -> f64 {
        0.0
    }

    fn back_porch(&self) -> f64 {
        0.0023
    }

    fn blank_duration(&self) -> f64 {
        0.0
    }

    fn color_space(&self) -> ColorSpace {
        ColorSpace::Yuv
    }

    fn uses_line_pairs(&self) -> bool {
        true
    }

    fn visible_line_length(&self) -> f64 {
        pd_visible_line_length(
            self.image_time(),
            self.data_lines(),
            self.front_porch(),
            self.back_porch(),
            self.sync_duration(),
        )
    }

    fn encode_line(&self, line_data: &LineData, _line_num: u32) -> Vec<Tone> {
        self.encode_line_pair(line_data, line_data, 0)
    }

    fn encode_line_pair(
        &self,
        even_line: &LineData,
        odd_line: &LineData,
        _line_num: u32,
    ) -> Vec<Tone> {
        encode_pd_line_pair(
            even_line,
            odd_line,
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
    fn test_pd120_params() {
        let pd = Pd120;
        assert_eq!(pd.vis_code(), 0x5F);
        assert_eq!(pd.resolution(), (640, 496));
        assert_eq!(pd.data_lines(), 248);
        assert!((pd.image_time() - 126.1115).abs() < 0.001);
    }

    #[test]
    fn test_pd180_params() {
        let pd = Pd180;
        assert_eq!(pd.vis_code(), 0x60);
        assert!((pd.image_time() - 187.0645).abs() < 0.001);
    }

    #[test]
    fn test_pd290_params() {
        let pd = Pd290;
        assert_eq!(pd.vis_code(), 0xDE);
        assert_eq!(pd.resolution(), (800, 616));
        assert_eq!(pd.data_lines(), 308);
        assert!((pd.image_time() - 288.702).abs() < 0.001);
    }

    #[test]
    fn test_pd120_line_pair_encoding() {
        let pd = Pd120;
        let even_pixels: Vec<RgbPixel> = (0..640)
            .map(|i| RgbPixel::new((i % 256) as u8, 128, 64))
            .collect();
        let odd_pixels: Vec<RgbPixel> = (0..640)
            .map(|i| RgbPixel::new((255 - i % 256) as u8, 64, 128))
            .collect();
        let even_line = LineData::new(even_pixels);
        let odd_line = LineData::new(odd_pixels);
        let tones = pd.encode_line_pair(&even_line, &odd_line, 0);

        // Should have: 640 Y-odd + 640 V + 640 U + 640 Y-even + fp + sync + bp
        // = 2560 pixels + 3 control tones = 2563 tones (no blanks for PD)
        assert_eq!(tones.len(), 2563);
    }

    #[test]
    fn test_visible_line_length() {
        let pd = Pd120;
        let vll = pd.visible_line_length();
        // Line time = 126.1115 / 248 ≈ 0.508 seconds
        // Visible = (0.508 - 0.020 - 0.0023) / 4 ≈ 0.121 seconds
        assert!(vll > 0.12 && vll < 0.13, "Visible line length: {}", vll);
    }
}
