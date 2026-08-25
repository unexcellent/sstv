use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::image::{RgbPixel, YuvPixel};
use crate::modes::layout::{Channel, ColorMode, Layout, Step};
use crate::modes::{Mode, value_frequency};
use crate::synthesizer::Tone;
use crate::{Error, Result};

/// `Encoder` is the main struct for converting an image into SSTV tones.
///
/// Construct `Encoder` with your desired mode and an iterator over the pixels
/// you want to encode, supplied row by row, top to bottom. The image must be
/// [`Mode::image_width`] pixels wide and [`Mode::image_height`] pixels tall.
/// ```rust
/// use sstv::{Encoder, Error, Mode, RgbPixel};
///
/// let image = [RgbPixel::new(0, 0, 0); 320 * 240];
/// let encoder = Encoder::new(Mode::Robot36, image.into_iter()).expect("error during encoding");
/// for tone in encoder {
///     // emit or save the tones
/// }
/// ```
pub struct Encoder {
    inner: Box<dyn Iterator<Item = Tone>>,
}

impl Encoder {
    /// Construct an `Encoder` from the mode and a pixel iterator.
    pub fn new<I>(mode: Mode, pixels: I) -> Result<Self>
    where
        I: Iterator<Item = RgbPixel> + 'static,
    {
        Ok(Self {
            inner: Box::new(LineEncoder::new(mode, pixels)?),
        })
    }
}

impl Iterator for Encoder {
    type Item = Tone;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// Where the encoder is within the transmission.
#[derive(Debug, Clone, Copy)]
enum Phase {
    /// Emitting the mode's header tones.
    Header(usize),
    /// Emitting the tones sent once between the header and the first line.
    Start(usize),
    /// Emitting the repeating timing sequences. `line` is the index of the
    /// first image line buffered for the current pass through the sequences.
    Line {
        line: usize,
        sequence: usize,
        step: usize,
        pixel: usize,
    },
    Finished,
}

/// Encodes any mode by walking its [`Layout`]: the header, then for each
/// group of buffered lines the mode's timing sequences, emitting fixed tones
/// verbatim and expanding each scan step into one tone per pixel.
struct LineEncoder<I>
where
    I: Iterator<Item = RgbPixel>,
{
    mode: Mode,
    layout: Layout,
    pixels: I,
    /// The image lines carried by the current pass through the sequences —
    /// one line for most modes, the line pair for Robot 36 and PD modes.
    lines: Vec<Vec<RgbPixel>>,
    phase: Phase,
}

impl<I> LineEncoder<I>
where
    I: Iterator<Item = RgbPixel>,
{
    fn new(mode: Mode, mut pixels: I) -> Result<Self> {
        let layout = mode.layout();
        let mut lines = Vec::with_capacity(layout.lines_per_cycle());
        for _ in 0..layout.lines_per_cycle() {
            let mut line = Vec::with_capacity(layout.width);
            if Self::fill_line(&mut pixels, &mut line, layout.width).is_none() {
                return Err(Error::EmptyImage);
            }
            lines.push(line);
        }
        Ok(Self {
            mode,
            layout,
            pixels,
            lines,
            phase: Phase::Header(0),
        })
    }

    /// Refill `line` in place from the pixel iterator, reusing its allocation.
    fn fill_line(pixels: &mut I, line: &mut Vec<RgbPixel>, width: usize) -> Option<()> {
        line.clear();
        for _ in 0..width {
            line.push(pixels.next()?);
        }
        Some(())
    }

    /// Replace the buffered lines with the next ones from the pixel iterator.
    /// `None` once the image runs out of complete line groups.
    fn buffer_next_lines(&mut self) -> Option<()> {
        for line in self.lines.iter_mut() {
            Self::fill_line(&mut self.pixels, line, self.layout.width)?;
        }
        Some(())
    }

    /// The pixel value a scan step transmits at horizontal position `x`.
    fn value(&self, sequence: usize, channel: Channel, x: usize) -> u8 {
        // The first buffered line the running sequence scans from.
        let base = sequence * self.layout.lines_per_sequence;
        let rgb = |line: usize| self.lines[line][x];
        let yuv = |line: usize| YuvPixel::from(rgb(line));
        // "The R-Y color information is averaged for two lines" — colour
        // difference scans average all buffered lines where the mode says so.
        let chroma = |component: fn(YuvPixel) -> u8| match self.layout.color {
            ColorMode::YuvAveragedPair | ColorMode::YuvSharedPair => {
                let sum: u16 = (0..self.lines.len())
                    .map(|line| component(yuv(line)) as u16)
                    .sum();
                (sum / self.lines.len() as u16) as u8
            }
            _ => component(yuv(base)),
        };

        match channel {
            Channel::Red => rgb(base).red(),
            Channel::Green => rgb(base).green(),
            Channel::Blue => rgb(base).blue(),
            Channel::Y => yuv(base).luma(),
            Channel::YSecond => yuv(base + 1).luma(),
            Channel::RY => chroma(YuvPixel::chroma_red),
            Channel::BY => chroma(YuvPixel::chroma_blue),
        }
    }
}

impl<I> Iterator for LineEncoder<I>
where
    I: Iterator<Item = RgbPixel>,
{
    type Item = Tone;

    fn next(&mut self) -> Option<Tone> {
        loop {
            match self.phase {
                Phase::Header(index) => match self.mode.header_tone(index) {
                    Some(tone) => {
                        self.phase = Phase::Header(index + 1);
                        return Some(tone);
                    }
                    None => self.phase = Phase::Start(0),
                },
                Phase::Start(index) => match self.layout.start.get(index) {
                    Some(Step::Tone(tone)) => {
                        self.phase = Phase::Start(index + 1);
                        return Some(*tone);
                    }
                    // Start steps are tones only; skip anything else.
                    Some(Step::Scan(..)) => self.phase = Phase::Start(index + 1),
                    None => {
                        self.phase = Phase::Line {
                            line: 0,
                            sequence: 0,
                            step: 0,
                            pixel: 0,
                        }
                    }
                },
                Phase::Line {
                    line,
                    sequence,
                    step,
                    pixel,
                } => match self.layout.sequences[sequence].get(step) {
                    Some(Step::Tone(tone)) => {
                        self.phase = Phase::Line {
                            line,
                            sequence,
                            step: step + 1,
                            pixel: 0,
                        };
                        return Some(*tone);
                    }
                    Some(Step::Scan(channel, duration)) if pixel < self.layout.width => {
                        let value = self.value(sequence, *channel, pixel);
                        let tone =
                            Tone::new(value_frequency(value), *duration / self.layout.width as u32);
                        self.phase = Phase::Line {
                            line,
                            sequence,
                            step,
                            pixel: pixel + 1,
                        };
                        return Some(tone);
                    }
                    Some(Step::Scan(..)) => {
                        self.phase = Phase::Line {
                            line,
                            sequence,
                            step: step + 1,
                            pixel: 0,
                        }
                    }
                    None if sequence + 1 < self.layout.sequences.len() => {
                        self.phase = Phase::Line {
                            line,
                            sequence: sequence + 1,
                            step: 0,
                            pixel: 0,
                        }
                    }
                    None => {
                        let next_line = line + self.layout.lines_per_cycle();
                        if next_line >= self.layout.height || self.buffer_next_lines().is_none() {
                            self.phase = Phase::Finished;
                        } else {
                            self.phase = Phase::Line {
                                line: next_line,
                                sequence: 0,
                                step: 0,
                                pixel: 0,
                            };
                        }
                    }
                },
                Phase::Finished => return None,
            }
        }
    }
}
