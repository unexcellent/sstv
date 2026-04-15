use alloc::boxed::Box;

use crate::image::RgbPixel;
use crate::modes::Mode;
use crate::modes::robot36::Robot36Encoder;
use crate::synthesizer::Tone;

pub struct Encoder {
    inner: Box<dyn Iterator<Item = Tone>>,
}

impl Encoder {
    pub fn new<I>(mode: Mode, pixels: I) -> Self
    where
        I: Iterator<Item = RgbPixel> + 'static,
    {
        let inner: Box<dyn Iterator<Item = Tone>> = match mode {
            Mode::Robot36 => Box::new(Robot36Encoder::new(pixels)),
        };

        Self { inner }
    }
}

impl Iterator for Encoder {
    type Item = Tone;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}
