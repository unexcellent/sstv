#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RgbPixel {
    red: u8,
    green: u8,
    blue: u8,
}
impl RgbPixel {
    pub fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }
    pub fn red(self) -> u8 {
        self.red
    }
    pub fn green(self) -> u8 {
        self.green
    }
    pub fn blue(self) -> u8 {
        self.blue
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct YuvPixel {
    luma: u8,
    chroma_red: u8,
    chroma_blue: u8,
}
impl YuvPixel {
    pub fn new(luma: u8, chroma_red: u8, chroma_blue: u8) -> Self {
        Self {
            luma,
            chroma_red,
            chroma_blue,
        }
    }
    pub fn luma(self) -> u8 {
        self.luma
    }
    pub fn chroma_red(self) -> u8 {
        self.chroma_red
    }
    pub fn chroma_blue(self) -> u8 {
        self.chroma_blue
    }
}

impl From<RgbPixel> for YuvPixel {
    fn from(rgb: RgbPixel) -> Self {
        let luma = (30 * rgb.red() as u32 + 59 * rgb.green() as u32 + 11 * rgb.blue() as u32) / 100;
        let luma = luma.min(255) as u8;

        let red_difference = rgb.red as i32 - luma as i32;
        let chroma_red = ((10 * red_difference + 7 * 255) / 14).clamp(0, 255) as u8;

        let blue_difference = rgb.blue as i32 - luma as i32;
        let chroma_blue = ((100 * blue_difference + 89 * 255) / 178).clamp(0, 255) as u8;

        Self {
            luma,
            chroma_red,
            chroma_blue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_to_yuv() {
        assert_eq!(
            YuvPixel::from(RgbPixel::new(67, 69, 42)),
            YuvPixel::new(65, 128, 114)
        )
    }
}
