use alloc::boxed::Box;

use crate::Result;
use crate::image::RgbPixel;
use crate::modes::Mode;
use crate::synthesizer::Tone;

pub struct Encoder {
    inner: Box<dyn Iterator<Item = Tone>>,
}

impl Encoder {
    pub fn new<I>(mode: Mode, pixels: I) -> Result<Self>
    where
        I: Iterator<Item = RgbPixel> + 'static,
    {
        Ok(Self {
            inner: mode.encoder(pixels)?,
        })
    }
}

impl Iterator for Encoder {
    type Item = Tone;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}
