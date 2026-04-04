//! SSTV Mode definitions and trait
//!
//! This module defines the `SSTVMode` trait that all SSTV modes must implement,
//! along with common types and utilities.
//!
//! Reference: qsstv/src/sstv/sstvparam.h and mode implementation files

pub mod martin;
pub mod scottie;
pub mod robot;
pub mod pd;
pub mod bw;

use crate::synthesizer::Tone;

/// Color space used by the SSTV mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSpace {
    /// RGB color space (Martin, Scottie modes)
    Rgb,
    /// YUV/YCrCb color space (Robot, PD modes)
    Yuv,
    /// Grayscale (B/W modes)
    Grayscale,
}

/// RGB pixel data
#[derive(Debug, Clone, Copy)]
pub struct RgbPixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbPixel {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Convert to YUV color space
    /// Using the same conversion as QSSTV (modebase.cpp:525-546)
    /// Y = 0.30*R + 0.59*G + 0.11*B
    /// Cr = (R - Y) * 0.7 + 128
    /// Cb = (B - Y) * 0.89 + 128
    pub fn to_yuv(&self) -> YuvPixel {
        let y = (30 * self.r as u32 + 59 * self.g as u32 + 11 * self.b as u32) / 100;
        let y = y.min(255) as u8;
        
        // Cr (R-Y) component - derived from modebase.cpp:545-546
        let r_diff = self.r as i32 - y as i32;
        let cr = ((10 * r_diff + 7 * 255) / 14).clamp(0, 255) as u8;
        
        // Cb (B-Y) component - derived from modebase.cpp:546
        let b_diff = self.b as i32 - y as i32;
        let cb = ((100 * b_diff + 89 * 255) / 178).clamp(0, 255) as u8;
        
        YuvPixel { y, cr, cb }
    }
}

/// YUV pixel data (used by Robot and PD modes)
#[derive(Debug, Clone, Copy)]
pub struct YuvPixel {
    pub y: u8,   // Luminance
    pub cr: u8,  // Red chrominance (R-Y)
    pub cb: u8,  // Blue chrominance (B-Y)
}

/// Line data for encoding
#[derive(Debug, Clone)]
pub struct LineData {
    /// RGB pixel data for this line
    pub pixels: Vec<RgbPixel>,
}

impl LineData {
    pub fn new(pixels: Vec<RgbPixel>) -> Self {
        Self { pixels }
    }

    /// Get YUV pixel data
    pub fn to_yuv(&self) -> Vec<YuvPixel> {
        self.pixels.iter().map(|p| p.to_yuv()).collect()
    }

    /// Get number of pixels
    pub fn len(&self) -> usize {
        self.pixels.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.pixels.is_empty()
    }
}

/// Core trait defining SSTV mode properties and encoding behavior
///
/// All values and methods are derived from qsstv/src/sstv/sstvparam.h
/// and the corresponding mode implementation files.
pub trait SSTVMode: Send + Sync {
    /// Mode name (e.g., "Martin 1")
    fn name(&self) -> &'static str;

    /// Short name (e.g., "M1")
    fn short_name(&self) -> &'static str;

    /// VIS code for this mode
    /// Derived from qsstv/src/sstv/sstvparam.cpp SSTVTable
    fn vis_code(&self) -> u8;

    /// Image resolution (width, height) in pixels
    /// Derived from numberOfPixels and numberOfDisplayLines in SSTVTable
    fn resolution(&self) -> (u32, u32);

    /// Number of data lines to transmit
    /// May differ from display lines for modes with line interleaving
    fn data_lines(&self) -> u32;

    /// Total image transmission time in seconds
    /// Derived from imageTime in SSTVTable
    fn image_time(&self) -> f64;

    /// Sync pulse duration in seconds
    /// Derived from synct in SSTVTable
    fn sync_duration(&self) -> f64;

    /// Front porch duration in seconds
    /// Derived from fpt in SSTVTable
    fn front_porch(&self) -> f64;

    /// Back porch duration in seconds
    /// Derived from bpt in SSTVTable
    fn back_porch(&self) -> f64;

    /// Blanking duration in seconds
    /// Derived from blankt in SSTVTable
    fn blank_duration(&self) -> f64;

    /// Color encoding type
    fn color_space(&self) -> ColorSpace;

    /// Calculate the visible line length in seconds
    /// This is the time spent transmitting actual pixel data for one color channel
    fn visible_line_length(&self) -> f64;

    /// Calculate pixel duration in seconds
    fn pixel_duration(&self) -> f64 {
        self.visible_line_length() / self.resolution().0 as f64
    }

    /// Calculate total line duration in seconds
    fn line_duration(&self) -> f64 {
        self.image_time() / self.data_lines() as f64
    }

    /// Encode a scan line and return the tones to transmit
    ///
    /// # Arguments
    /// * `line_data` - The pixel data for this line
    /// * `line_num` - The line number (0-indexed)
    ///
    /// Returns a vector of tones representing the encoded line
    fn encode_line(&self, line_data: &LineData, line_num: u32) -> Vec<Tone>;

    /// Whether this mode uses 2-line pair encoding (Robot36, PD modes)
    /// If true, encoder should call encode_line_pair instead of encode_line
    fn uses_line_pairs(&self) -> bool {
        false
    }

    /// Encode a pair of lines (for modes with 2-line interleaving like PD)
    /// Default implementation just encodes lines individually
    fn encode_line_pair(&self, even_line: &LineData, odd_line: &LineData, line_num: u32) -> Vec<Tone> {
        let mut tones = self.encode_line(even_line, line_num);
        tones.extend(self.encode_line(odd_line, line_num + 1));
        tones
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Martin1,
    Martin2,
    Scottie1,
    Scottie2,
    ScottieDx,
    Robot36,
    Robot72,
    Pd120,
    Pd180,
    Pd290,
    Bw8,
    Bw12,
}

impl Mode {
    /// Get the mode implementation
    pub fn get_impl(&self) -> Box<dyn SSTVMode> {
        match self {
            Mode::Martin1 => Box::new(martin::Martin1),
            Mode::Martin2 => Box::new(martin::Martin2),
            Mode::Scottie1 => Box::new(scottie::Scottie1),
            Mode::Scottie2 => Box::new(scottie::Scottie2),
            Mode::ScottieDx => Box::new(scottie::ScottieDx),
            Mode::Robot36 => Box::new(robot::Robot36),
            Mode::Robot72 => Box::new(robot::Robot72),
            Mode::Pd120 => Box::new(pd::Pd120),
            Mode::Pd180 => Box::new(pd::Pd180),
            Mode::Pd290 => Box::new(pd::Pd290),
            Mode::Bw8 => Box::new(bw::Bw8),
            Mode::Bw12 => Box::new(bw::Bw12),
        }
    }

    /// Parse mode from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "martin1" | "m1" => Some(Mode::Martin1),
            "martin2" | "m2" => Some(Mode::Martin2),
            "scottie1" | "s1" => Some(Mode::Scottie1),
            "scottie2" | "s2" => Some(Mode::Scottie2),
            "scottiedx" | "sdx" => Some(Mode::ScottieDx),
            "robot36" | "r36" => Some(Mode::Robot36),
            "robot72" | "r72" => Some(Mode::Robot72),
            "pd120" => Some(Mode::Pd120),
            "pd180" => Some(Mode::Pd180),
            "pd290" => Some(Mode::Pd290),
            "bw8" => Some(Mode::Bw8),
            "bw12" => Some(Mode::Bw12),
            _ => None,
        }
    }

    /// List all available modes
    pub fn all() -> &'static [Mode] {
        &[
            Mode::Martin1,
            Mode::Martin2,
            Mode::Scottie1,
            Mode::Scottie2,
            Mode::ScottieDx,
            Mode::Robot36,
            Mode::Robot72,
            Mode::Pd120,
            Mode::Pd180,
            Mode::Pd290,
            Mode::Bw8,
            Mode::Bw12,
        ]
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Mode::Martin1 => "martin1",
            Mode::Martin2 => "martin2",
            Mode::Scottie1 => "scottie1",
            Mode::Scottie2 => "scottie2",
            Mode::ScottieDx => "scottiedx",
            Mode::Robot36 => "robot36",
            Mode::Robot72 => "robot72",
            Mode::Pd120 => "pd120",
            Mode::Pd180 => "pd180",
            Mode::Pd290 => "pd290",
            Mode::Bw8 => "bw8",
            Mode::Bw12 => "bw12",
        };
        write!(f, "{}", name)
    }
}
