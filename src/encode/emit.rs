//! Walking the transmission: tone emission and the state machine behind it.

use super::Encoder;
use crate::image::RgbPixel;
use crate::modes::step::Step;
use crate::modes::{Mode, value_frequency};
use crate::synthesizer::Tone;

impl<I> Iterator for Encoder<'_, I>
where
    I: Iterator<Item = RgbPixel>,
{
    type Item = Tone;

    fn next(&mut self) -> Option<Tone> {
        self.state.advance(self.mode);

        let pixel_iterator_is_empty =
            self.state.needs_next_lines() && self.lines.fill_next(&mut self.pixels).is_none();
        if pixel_iterator_is_empty {
            self.state = State::Finished;
        }

        self.emit()
    }
}

impl<I> Encoder<'_, I>
where
    I: Iterator<Item = RgbPixel>,
{
    /// The tone belonging to the current state.
    fn emit(&self) -> Option<Tone> {
        match self.state {
            State::NotStarted | State::Finished => None,
            State::Header(index) => self.mode.header_tone(index),
            State::Image { step, pixel, .. } => Some(self.emit_image_tone(step, pixel)),
        }
    }

    fn emit_image_tone(&self, step: usize, pixel: usize) -> Tone {
        let current_step = self.mode.sequence[step];
        match current_step {
            Step::Control(tone) => tone,
            Step::Scan(channel, duration) => {
                let value = self.lines.value(pixel, channel);
                Tone::new(
                    value_frequency(value),
                    duration / self.mode.resolution.0 as u32,
                )
            }
        }
    }
}

/// Where the encoder is within the transmission.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum State {
    NotStarted,
    Header(usize),
    Image {
        /// The first image row of the currently buffered line group.
        row: usize,
        /// The position within the timing sequence's steps.
        step: usize,
        /// The horizontal position within a scan step.
        pixel: usize,
    },
    Finished,
}

impl State {
    /// Step to the state that emits the next tone.
    fn advance(&mut self, mode: Mode) {
        *self = match *self {
            Self::NotStarted => Self::Header(0),
            Self::Header(index) => Self::advance_header(mode, index),
            Self::Image { row, step, pixel } => Self::advance_image(mode, row, step, pixel),
            Self::Finished => Self::Finished,
        };
    }

    /// The state after the `index`-th header tone.
    fn advance_header(mode: Mode, index: usize) -> Self {
        let header_has_more_tones = mode.header_tone(index + 1).is_some();

        if header_has_more_tones {
            return Self::Header(index + 1);
        }

        Self::Image {
            row: 0,
            step: 0,
            pixel: 0,
        }
    }

    /// The state after the given position within the image.
    fn advance_image(mode: Mode, row_index: usize, step_index: usize, pixel_index: usize) -> Self {
        let current_step = mode.sequence[step_index];
        let image_width = mode.resolution().0 as usize;
        let image_height = mode.resolution().1 as usize;

        let scan_is_ongoing = current_step.is_scan() && pixel_index + 1 < image_width;
        if scan_is_ongoing {
            return Self::Image {
                row: row_index,
                step: step_index,
                pixel: pixel_index + 1,
            };
        }

        let sequence_is_ongoing = step_index + 1 < mode.sequence.len();
        if sequence_is_ongoing {
            return Self::Image {
                row: row_index,
                step: step_index + 1,
                pixel: 0,
            };
        }

        let image_is_ongoing = row_index + mode.lines_per_sequence < image_height;
        if image_is_ongoing {
            return Self::Image {
                row: row_index + mode.lines_per_sequence,
                step: 0,
                pixel: 0,
            };
        }

        Self::Finished
    }

    /// Whether the state just moved onto the first tone of a line group whose
    /// lines are not buffered yet. The first group's lines are already
    /// buffered at construction.
    const fn needs_next_lines(&self) -> bool {
        matches!(
            self,
            Self::Image {
                row,
                step: 0,
                pixel: 0,
            } if *row > 0
        )
    }
}
