use alloc::format;
use alloc::vec::Vec;

use crate::Error;
use crate::Result;

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

    pub fn to_yuv(self) -> YuvPixel {
        let y = (30 * self.r as u32 + 59 * self.g as u32 + 11 * self.b as u32) / 100;
        let y = y.min(255) as u8;

        let r_diff = self.r as i32 - y as i32;
        let cr = ((10 * r_diff + 7 * 255) / 14).clamp(0, 255) as u8;

        let b_diff = self.b as i32 - y as i32;
        let cb = ((100 * b_diff + 89 * 255) / 178).clamp(0, 255) as u8;

        YuvPixel { y, cr, cb }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct YuvPixel {
    pub y: u8,
    pub cr: u8,
    pub cb: u8,
}

pub struct LineData {
    pub pixels: Vec<RgbPixel>,
}

impl LineData {
    pub fn new(pixels: Vec<RgbPixel>) -> Self {
        Self { pixels }
    }
}

pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<RgbPixel>,
}

impl ImageData {
    pub fn new(width: u32, height: u32, pixels: Vec<RgbPixel>) -> Self {
        Self {
            width,
            height,
            pixels,
        }
    }

    pub fn from_rgb_bytes(width: u32, height: u32, bytes: &[u8]) -> Result<Self> {
        if bytes.len() != (width * height * 3) as usize {
            return Err(Error::EncodingError(format!(
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

    pub fn get_line(&self, line_num: u32) -> LineData {
        let start = (line_num * self.width) as usize;
        let end = start + self.width as usize;
        LineData::new(self.pixels[start..end].to_vec())
    }
}
