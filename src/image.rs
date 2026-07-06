use crate::{
    synthesizer::Tone,
    units::{Duration, Frequency},
};

#[derive(Debug, Clone, Copy, PartialEq)]
/// A single pixel in an image represented by red, green, blue components.
pub struct RgbPixel {
    red: u8,
    green: u8,
    blue: u8,
}
impl RgbPixel {
    /// Construct from the color components.
    pub fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }
    /// Return the red value.
    pub fn red(self) -> u8 {
        self.red
    }
    /// Return the green value.
    pub fn green(self) -> u8 {
        self.green
    }
    /// Return the blue value.
    pub fn blue(self) -> u8 {
        self.blue
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// A single pixel in an image represented by separate brightness and color information.
pub struct YuvPixel {
    /// Brightness of the pixel.
    luma: u8,
    /// Red color component.
    chroma_red: u8,
    /// Blue color component.
    chroma_blue: u8,
}
impl YuvPixel {
    /// Construct from the individual components.
    pub fn new(luma: u8, chroma_red: u8, chroma_blue: u8) -> Self {
        Self {
            luma,
            chroma_red,
            chroma_blue,
        }
    }
    /// Construct the average of the color components of two pixels.
    ///
    /// The returned pixel has the luma of the first argument and the average red/blue chroma from both components.
    pub fn average(first: Self, second: Self) -> Self {
        Self {
            luma: first.luma(),
            chroma_red: ((first.chroma_red() as u16 + second.chroma_red() as u16) / 2) as u8,
            chroma_blue: ((first.chroma_blue() as u16 + second.chroma_blue() as u16) / 2) as u8,
        }
    }
    /// Return the brightness of the pixel.
    pub fn luma(self) -> u8 {
        self.luma
    }
    /// Return the red color component.
    pub fn chroma_red(self) -> u8 {
        self.chroma_red
    }
    /// Return the blue color component.
    pub fn chroma_blue(self) -> u8 {
        self.chroma_blue
    }
    /// Return the frequency associated with the brightness.
    pub fn luma_frequency(self, lower: Frequency, upper: Frequency) -> Frequency {
        lower + (upper - lower) * self.luma as u32 / 255
    }
    /// Return the tone associated with the brightness.
    pub fn luma_tone(self, black: Frequency, white: Frequency, duration: Duration) -> Tone {
        Tone::new(self.luma_frequency(black, white), duration)
    }
    /// Return the frequency associated with the red chroma.
    pub fn chroma_red_frequency(self, black: Frequency, white: Frequency) -> Frequency {
        black + (white - black) * self.chroma_red as u32 / 255
    }
    /// Return the tone associated with the red chroma.
    pub fn chroma_red_tone(self, black: Frequency, white: Frequency, duration: Duration) -> Tone {
        Tone::new(self.chroma_red_frequency(black, white), duration)
    }
    /// Return the frequency associated with the blue chroma.
    pub fn chroma_blue_frequency(self, black: Frequency, white: Frequency) -> Frequency {
        black + (white - black) * self.chroma_blue as u32 / 255
    }
    /// Return the tone associated with the blue chroma.
    pub fn chroma_blue_tone(self, black: Frequency, white: Frequency, duration: Duration) -> Tone {
        Tone::new(self.chroma_blue_frequency(black, white), duration)
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

impl From<YuvPixel> for RgbPixel {
    fn from(yuv: YuvPixel) -> Self {
        let luma = yuv.luma() as i32;

        // Inverse of the clamped conversions in `From<RgbPixel>`.
        let red_difference = (14 * yuv.chroma_red() as i32 - 7 * 255) / 10;
        let blue_difference = (178 * yuv.chroma_blue() as i32 - 89 * 255) / 100;

        let red = (luma + red_difference).clamp(0, 255);
        let blue = (luma + blue_difference).clamp(0, 255);
        let green = ((100 * luma - 30 * red - 11 * blue) / 59).clamp(0, 255);

        RgbPixel::new(red as u8, green as u8, blue as u8)
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

    #[test]
    fn rgb_survives_yuv_round_trip() {
        // The YUV conversion quantizes and clamps, so a small deviation is
        // expected rather than exact equality.
        for rgb in [
            RgbPixel::new(0, 0, 0),
            RgbPixel::new(255, 255, 255),
            RgbPixel::new(200, 100, 50),
            RgbPixel::new(30, 180, 220),
        ] {
            let restored = RgbPixel::from(YuvPixel::from(rgb));
            let deviation = |a: u8, b: u8| (a as i32 - b as i32).abs();
            assert!(
                deviation(restored.red(), rgb.red()) <= 6
                    && deviation(restored.green(), rgb.green()) <= 6
                    && deviation(restored.blue(), rgb.blue()) <= 6,
                "{rgb:?} -> {restored:?}",
            );
        }
    }
}
