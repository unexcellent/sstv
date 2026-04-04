//! Robot mode implementations (Robot 36 and Robot 72)
//!
//! Robot modes use YUV color space with interleaved Y (luminance) and UV (chrominance).
//!
//! Robot 36 uses 2-line interleaving (from moderobot1.cpp):
//!   - Y-odd, V (R-Y), sync, Y-even, U (B-Y), sync
//!
//! Robot 72 encodes each line individually (from moderobot2.cpp):
//!   - Y, V, U, sync
//!
//! Reference: qsstv/src/sstv/modes/moderobot1.cpp and moderobot2.cpp

use crate::constants::{FREQ_BLACK, FREQ_EVEN_MARKER, FREQ_SEPARATOR, FREQ_SYNC, pixel_to_freq};
use crate::modes::{ColorSpace, LineData, SSTVMode, YuvPixel};
use crate::synthesizer::Tone;

/// Generate pixel tones for a color channel (YUV values use same frequency mapping)
fn generate_pixel_tones(values: &[u8], pixel_duration: f64) -> Vec<Tone> {
    values
        .iter()
        .map(|&v| Tone::new(pixel_to_freq(v), pixel_duration))
        .collect()
}

// =============================================================================
// Robot 36
// =============================================================================

/// Robot 36 mode
///
/// Uses 2-line interleaving with odd/even Y lines sharing UV data.
///
/// Parameters from qsstv/src/sstv/sstvparam.cpp line 318:
/// {"Robot 36", "R36", R36, 36.00200, 320, 240, 240, 0x88,
///  0.00900, 0.00040, 0.00250, 0.00700,  // rx: sync, fp, bp, blank (as gap)
///  0.00900, 0.00000, 0.00300, 0.00540,  // tx: synct, fpt, bpt, blankt
///  0., 1900, 400}
///
/// Line structure from moderobot1.cpp:204-268:
/// Each "line" transmits 2 display lines:
///   0: Y-odd pixels (2× duration)
///   1: 1500 Hz gap (2/3 blank)
///   2: 1900 Hz separator (1/3 blank)
///   3: R-Y (V) pixels
///   4: fp (1500 Hz)
///   5: sync (1200 Hz)
///   6: bp (1500 Hz)
///   7: Y-even pixels (2× duration)
///   8: 2300 Hz marker (2/3 blank)
///   9: 1900 Hz separator (1/3 blank)
///  10: B-Y (U) pixels
///  11: fp (1500 Hz)
///  12: sync (1200 Hz)
#[derive(Debug, Clone, Copy)]
pub struct Robot36;

impl Robot36 {
    /// Calculate visible line length for one half of a line pair
    /// From moderobot1.cpp:44-47
    /// visibleLineLength = (lineLength - fp - bp - blank - syncDuration) / 3
    /// Note: QSSTV uses 240 "half lines" for timing calculation
    fn calc_visible_line_length(&self) -> f64 {
        // 240 half-lines (each line pair has 2 halves)
        let half_lines = 240.0;
        let line_length = self.image_time() / half_lines;
        (line_length
            - self.front_porch()
            - self.back_porch()
            - self.blank_duration()
            - self.sync_duration())
            / 3.0
    }
}

impl SSTVMode for Robot36 {
    fn name(&self) -> &'static str {
        "Robot 36"
    }

    fn short_name(&self) -> &'static str {
        "R36"
    }

    /// VIS code 0x88 (136)
    /// Derived from qsstv/src/sstv/sstvparam.cpp:318
    fn vis_code(&self) -> u8 {
        0x88
    }

    /// Resolution 320x240
    fn resolution(&self) -> (u32, u32) {
        (320, 240)
    }

    /// 240 data lines (QSSTV counts half-lines, encoder processes in pairs)
    fn data_lines(&self) -> u32 {
        240
    }

    /// Image time 36.002 seconds
    fn image_time(&self) -> f64 {
        36.00200
    }

    /// Sync duration 9.0ms
    fn sync_duration(&self) -> f64 {
        0.009
    }

    /// Front porch 0.0ms for TX
    fn front_porch(&self) -> f64 {
        0.0
    }

    /// Back porch 3.0ms
    /// Derived from qsstv/src/sstv/sstvparam.cpp:318 - bpt=0.00300
    fn back_porch(&self) -> f64 {
        0.003
    }

    /// Blank/gap duration 5.4ms
    /// Derived from qsstv/src/sstv/sstvparam.cpp:318 - blankt=0.00540
    fn blank_duration(&self) -> f64 {
        0.0054
    }

    fn color_space(&self) -> ColorSpace {
        ColorSpace::Yuv
    }

    fn uses_line_pairs(&self) -> bool {
        true
    }

    fn visible_line_length(&self) -> f64 {
        self.calc_visible_line_length()
    }

    /// Encode a pair of lines (Robot 36 processes 2 lines at a time)
    fn encode_line(&self, line_data: &LineData, line_num: u32) -> Vec<Tone> {
        // Robot 36 needs pair encoding - this single line version
        // is used when we only have one line of data
        self.encode_single_line(line_data, line_num.is_multiple_of(2))
    }

    fn encode_line_pair(
        &self,
        even_line: &LineData,
        odd_line: &LineData,
        _line_num: u32,
    ) -> Vec<Tone> {
        let visible = self.visible_line_length();
        let blank = self.blank_duration();
        let fp = self.front_porch();
        let bp = self.back_porch();
        let sync = self.sync_duration();
        let num_pixels = self.resolution().0;

        // Y uses 2× duration per pixel
        let y_pixel_duration = (2.0 * visible) / num_pixels as f64;
        // UV uses 1× duration
        let uv_pixel_duration = visible / num_pixels as f64;

        let mut tones = Vec::new();

        // Convert to YUV
        let even_yuv: Vec<YuvPixel> = even_line.pixels.iter().map(|p| p.to_yuv()).collect();
        let odd_yuv: Vec<YuvPixel> = odd_line.pixels.iter().map(|p| p.to_yuv()).collect();

        // Average UV between the two lines (R-Y and B-Y chrominance)
        // V = Cr = R-Y, U = Cb = B-Y
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

        // Y values for each line
        // CRITICAL: QSSTV's decoder expects EVEN line Y first, ODD line Y second
        // See rxSetupLine: case 1 (first Y) -> yArrayPtr -> ye
        //                  case 8 (second Y) -> greenArrayPtr -> yo
        let y_first: Vec<u8> = even_yuv.iter().map(|p| p.y).collect(); // Even line Y (sent first)
        let y_second: Vec<u8> = odd_yuv.iter().map(|p| p.y).collect(); // Odd line Y (sent second)

        // === First half: Y-even + V (R-Y) ===
        // Position 0: Y from EVEN line (2× pixel duration)
        tones.extend(generate_pixel_tones(&y_first, y_pixel_duration));

        // 1: 1500 Hz gap (2/3 blank) - from moderobot1.cpp:214
        tones.push(Tone::new(FREQ_BLACK, (2.0 * blank) / 3.0));

        // 2: 1900 Hz separator (1/3 blank) - from moderobot1.cpp:218
        tones.push(Tone::new(FREQ_SEPARATOR, blank / 3.0));

        // 3: V (R-Y) pixels
        tones.extend(generate_pixel_tones(&v_values, uv_pixel_duration));

        // 4: fp (1500 Hz)
        if fp > 0.0 {
            tones.push(Tone::new(FREQ_BLACK, fp));
        }

        // 5: sync (1200 Hz)
        tones.push(Tone::new(FREQ_SYNC, sync));

        // 6: bp (1500 Hz)
        tones.push(Tone::new(FREQ_BLACK, bp));

        // === Second half: Y-odd + U (B-Y) ===
        // Position 7: Y from ODD line (2× pixel duration)
        tones.extend(generate_pixel_tones(&y_second, y_pixel_duration));

        // 8: 2300 Hz marker (2/3 blank) - from moderobot1.cpp:243
        tones.push(Tone::new(FREQ_EVEN_MARKER, (2.0 * blank) / 3.0));

        // 9: 1900 Hz separator (1/3 blank)
        tones.push(Tone::new(FREQ_SEPARATOR, blank / 3.0));

        // 10: U (B-Y) pixels
        tones.extend(generate_pixel_tones(&u_values, uv_pixel_duration));

        // 11: fp (1500 Hz)
        if fp > 0.0 {
            tones.push(Tone::new(FREQ_BLACK, fp));
        }

        // 12: sync (1200 Hz)
        tones.push(Tone::new(FREQ_SYNC, sync));

        // 13: bp (1500 Hz) - CRITICAL: from moderobot1.cpp:262-265
        // This was missing! Case 13 adds bp after the second sync
        tones.push(Tone::new(FREQ_BLACK, bp));

        tones
    }
}

impl Robot36 {
    /// Encode a single line (for when we only have one line)
    fn encode_single_line(&self, line_data: &LineData, is_even: bool) -> Vec<Tone> {
        let visible = self.visible_line_length();
        let blank = self.blank_duration();
        let fp = self.front_porch();
        let bp = self.back_porch();
        let sync = self.sync_duration();
        let num_pixels = self.resolution().0;

        let y_pixel_duration = (2.0 * visible) / num_pixels as f64;
        let uv_pixel_duration = visible / num_pixels as f64;

        let mut tones = Vec::new();

        let yuv: Vec<YuvPixel> = line_data.pixels.iter().map(|p| p.to_yuv()).collect();
        let y_values: Vec<u8> = yuv.iter().map(|p| p.y).collect();
        let uv_values: Vec<u8> = if is_even {
            yuv.iter().map(|p| p.cb).collect() // U for even
        } else {
            yuv.iter().map(|p| p.cr).collect() // V for odd
        };

        // Y pixels
        tones.extend(generate_pixel_tones(&y_values, y_pixel_duration));

        // Gap
        let marker_freq = if is_even {
            FREQ_EVEN_MARKER
        } else {
            FREQ_BLACK
        };
        tones.push(Tone::new(marker_freq, (2.0 * blank) / 3.0));
        tones.push(Tone::new(FREQ_SEPARATOR, blank / 3.0));

        // UV pixels
        tones.extend(generate_pixel_tones(&uv_values, uv_pixel_duration));

        // Front porch
        if fp > 0.0 {
            tones.push(Tone::new(FREQ_BLACK, fp));
        }

        // Sync
        tones.push(Tone::new(FREQ_SYNC, sync));

        // Back porch (only for odd lines going to next line)
        if !is_even {
            tones.push(Tone::new(FREQ_BLACK, bp));
        }

        tones
    }
}

// =============================================================================
// Robot 72
// =============================================================================

/// Robot 72 mode
///
/// Each line is encoded individually with Y, V, U, sync.
///
/// Parameters from qsstv/src/sstv/sstvparam.cpp line 319:
/// {"Robot 72", "R72", R72, 72.00500, 320, 240, 240, 0x0C,
///  0.00900, 0.00040, 0.00350, 0.00600,  // rx
///  0.00900, 0.00040, 0.00250, 0.00600,  // tx
///  0., 1900, 400}
///
/// Line structure from moderobot2.cpp:155-203:
///   0: bp (1500 Hz) - with +6 samples adjustment
///   1: Y pixels (2× duration)
///   2: 1500 Hz gap (2/3 blank)
///   3: 1900 Hz separator (1/3 blank)
///   4: V (R-Y) pixels
///   5: 2300 Hz gap (2/3 blank)
///   6: 1900 Hz separator (1/3 blank)
///   7: U (B-Y) pixels
///   8: fp (1500 Hz)
///   9: sync (1200 Hz)
#[derive(Debug, Clone, Copy)]
pub struct Robot72;

impl Robot72 {
    /// Calculate visible line length
    /// From moderobot2.cpp:43-48
    /// visibleLineLength = (lineLength - fp - bp - 2*blank - syncDuration) / 4
    fn calc_visible_line_length(&self) -> f64 {
        let line_length = self.image_time() / self.data_lines() as f64;
        (line_length
            - self.front_porch()
            - self.back_porch()
            - 2.0 * self.blank_duration()
            - self.sync_duration())
            / 4.0
    }
}

impl SSTVMode for Robot72 {
    fn name(&self) -> &'static str {
        "Robot 72"
    }

    fn short_name(&self) -> &'static str {
        "R72"
    }

    /// VIS code 0x0C (12)
    /// Derived from qsstv/src/sstv/sstvparam.cpp:319
    fn vis_code(&self) -> u8 {
        0x0C
    }

    fn resolution(&self) -> (u32, u32) {
        (320, 240)
    }

    fn data_lines(&self) -> u32 {
        240
    }

    /// Image time 72.005 seconds
    fn image_time(&self) -> f64 {
        72.00500
    }

    fn sync_duration(&self) -> f64 {
        0.009
    }

    /// Front porch 0.4ms
    fn front_porch(&self) -> f64 {
        0.0004
    }

    /// Back porch 2.5ms
    fn back_porch(&self) -> f64 {
        0.0025
    }

    /// Blank duration 6.0ms
    fn blank_duration(&self) -> f64 {
        0.006
    }

    fn color_space(&self) -> ColorSpace {
        ColorSpace::Yuv
    }

    fn visible_line_length(&self) -> f64 {
        self.calc_visible_line_length()
    }

    fn encode_line(&self, line_data: &LineData, _line_num: u32) -> Vec<Tone> {
        let visible = self.visible_line_length();
        let blank = self.blank_duration();
        let fp = self.front_porch();
        let bp = self.back_porch();
        let sync = self.sync_duration();
        let num_pixels = self.resolution().0;

        // Y uses 2× duration, UV uses 1×
        let y_pixel_duration = (2.0 * visible) / num_pixels as f64;
        let uv_pixel_duration = visible / num_pixels as f64;

        let mut tones = Vec::new();

        // Convert to YUV
        let yuv: Vec<YuvPixel> = line_data.pixels.iter().map(|p| p.to_yuv()).collect();
        let y_values: Vec<u8> = yuv.iter().map(|p| p.y).collect();
        let v_values: Vec<u8> = yuv.iter().map(|p| p.cr).collect();
        let u_values: Vec<u8> = yuv.iter().map(|p| p.cb).collect();

        // 0: bp (1500 Hz) - moderobot2.cpp:161-163 adds +6 samples
        tones.push(Tone::new(FREQ_BLACK, bp));

        // 1: Y pixels (2× duration)
        tones.extend(generate_pixel_tones(&y_values, y_pixel_duration));

        // 2: 1500 Hz gap (2/3 blank)
        tones.push(Tone::new(FREQ_BLACK, (2.0 * blank) / 3.0));

        // 3: 1900 Hz separator (1/3 blank)
        tones.push(Tone::new(FREQ_SEPARATOR, blank / 3.0));

        // 4: V (R-Y) pixels
        tones.extend(generate_pixel_tones(&v_values, uv_pixel_duration));

        // 5: 2300 Hz gap (2/3 blank)
        tones.push(Tone::new(FREQ_EVEN_MARKER, (2.0 * blank) / 3.0));

        // 6: 1900 Hz separator (1/3 blank)
        tones.push(Tone::new(FREQ_SEPARATOR, blank / 3.0));

        // 7: U (B-Y) pixels
        tones.extend(generate_pixel_tones(&u_values, uv_pixel_duration));

        // 8: fp (1500 Hz)
        tones.push(Tone::new(FREQ_BLACK, fp));

        // 9: sync (1200 Hz)
        tones.push(Tone::new(FREQ_SYNC, sync));

        tones
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::RgbPixel;

    #[test]
    fn test_robot36_params() {
        let r36 = Robot36;
        assert_eq!(r36.vis_code(), 0x88);
        assert_eq!(r36.resolution(), (320, 240));
        assert!((r36.image_time() - 36.002).abs() < 0.001);
    }

    #[test]
    fn test_robot72_params() {
        let r72 = Robot72;
        assert_eq!(r72.vis_code(), 0x0C);
        assert_eq!(r72.resolution(), (320, 240));
        assert!((r72.image_time() - 72.005).abs() < 0.001);
    }

    #[test]
    fn test_robot72_line_encoding() {
        let r72 = Robot72;
        let pixels: Vec<RgbPixel> = (0..320).map(|i| RgbPixel::new(i as u8, 128, 64)).collect();
        let line_data = LineData::new(pixels);
        let tones = r72.encode_line(&line_data, 0);

        // Should have: bp + 320 Y + gap + sep + 320 V + gap + sep + 320 U + fp + sync
        // = 960 pixels + 7 control tones = 967 tones
        assert_eq!(tones.len(), 967);
    }

    #[test]
    fn test_robot36_line_pair_encoding() {
        let r36 = Robot36;
        let even_pixels: Vec<RgbPixel> =
            (0..320).map(|i| RgbPixel::new(i as u8, 128, 64)).collect();
        let odd_pixels: Vec<RgbPixel> = (0..320)
            .map(|i| RgbPixel::new(255 - i as u8, 64, 128))
            .collect();
        let even_line = LineData::new(even_pixels);
        let odd_line = LineData::new(odd_pixels);
        let tones = r36.encode_line_pair(&even_line, &odd_line, 0);

        // Should have significant number of tones for 2 lines
        assert!(tones.len() > 1200, "Got {} tones", tones.len());
    }
}
