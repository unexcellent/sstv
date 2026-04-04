use alloc::format;
use alloc::vec::Vec;

use crate::Error;
use crate::Result;

#[derive(Debug, Clone, Copy)]
pub struct RgbPixel {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl RgbPixel {
    pub fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    pub fn to_yuv(self) -> YuvPixel {
        let luma = (30 * self.red as u32 + 59 * self.green as u32 + 11 * self.blue as u32) / 100;
        let luma = luma.min(255) as u8;

        let red_difference = self.red as i32 - luma as i32;
        let chroma_red = ((10 * red_difference + 7 * 255) / 14).clamp(0, 255) as u8;

        let blue_difference = self.blue as i32 - luma as i32;
        let chroma_blue = ((100 * blue_difference + 89 * 255) / 178).clamp(0, 255) as u8;

        YuvPixel {
            luma,
            chroma_red,
            chroma_blue,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct YuvPixel {
    pub luma: u8,
    pub chroma_red: u8,
    pub chroma_blue: u8,
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
