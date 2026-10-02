//! Assembling an image's tones by the mode's timing: each timing sequence is
//! anchored on its sync pulse, stretched to end where the next sequence's
//! sync pulse puts it, and cut into one tone per control step and one per
//! pixel, as the encoder emits them.

use super::Assembler;
use crate::modes::step::Step;
use crate::modes::{Mode, SYNC_FREQUENCY};
use crate::synthesizer::Tone;
use crate::units::{Duration, Frequency};
use crate::{Hz, ms};

/// How long after a sync pulse is expected to end the assembler waits for it
/// before assuming it was lost. A tone is reported only once the next one
/// has lasted a few milliseconds.
const SYNC_GRACE: Duration = ms!(10);
const SYNC_FREQUENCY_TOLERANCE: Frequency = Hz!(50);

/// Where the image being assembled stands.
pub(super) struct ImageTiming {
    mode: Mode,
    /// The (fractional) sample at which the current timing sequence begins.
    sequence_start: f64,
    /// Where the next sequence begins, once its sync pulse has been seen.
    next_sequence_start: Option<f64>,
    /// The sequences still to be assembled.
    sequences_left: usize,
}

impl<I: Iterator<Item = Frequency>> Assembler<I> {
    /// Begin assembling an image whose header was identified by the tone that
    /// ended at sample `end`: the stop bit merged with the following sync
    /// pulse, which is the first sequence's own sync pulse or, for modes
    /// with a starting sync pulse, the one that precedes the first sequence.
    pub(super) fn start_image(&mut self, mode: Mode, end: u64) {
        let sequence_start = if mode.has_starting_sync_pulse() {
            end as f64
        } else {
            end as f64 - self.samples_in(mode.sync_pulse().1)
        };
        self.estimates.forget_before(sequence_start as u64);
        self.image = Some(ImageTiming {
            mode,
            sequence_start,
            next_sequence_start: None,
            sequences_left: mode.resolution.1 / mode.lines_per_sequence,
        });
    }

    /// Take a tone found by where the frequency changes, which ended at
    /// sample `end`, as the next sequence's sync pulse if it is one.
    pub(super) fn anchor_on_sync(&mut self, tone: Tone, end: u64) {
        let Some(image) = &self.image else {
            return;
        };
        let (sync_offset, sync_duration) = image.mode.sync_pulse();
        let expected_sync = Tone::new(SYNC_FREQUENCY, sync_duration);
        let tolerance = Tone::new(SYNC_FREQUENCY_TOLERANCE, sync_duration / 2);
        if !tone.is_near(expected_sync, tolerance) {
            return;
        }

        let sequence_start =
            end as f64 - self.samples_in(sync_duration) - self.samples_in(sync_offset);
        let window = self.samples_in(sync_duration);
        let expected_sequence_start = self.expected_next_sequence_start();
        if let Some(image) = &mut self.image
            && image.next_sequence_start.is_none()
            && (sequence_start - expected_sequence_start).abs() <= window
        {
            image.next_sequence_start = Some(sequence_start);
        }
    }

    /// Assemble the current sequence once the next one's sync pulse has been
    /// seen, or once it is overdue.
    pub(super) fn assemble_ready_sequence(&mut self) {
        let Some(image) = &self.image else {
            return;
        };
        let (sync_offset, sync_duration) = image.mode.sync_pulse();
        let next_sequence_start = if let Some(start) = image.next_sequence_start {
            start
        } else {
            let expected = self.expected_next_sequence_start();
            let deadline = expected
                + self.samples_in(sync_offset + sync_duration)
                + self.samples_in(sync_duration)
                + self.samples_in(SYNC_GRACE);
            if (self.estimates.end() as f64) < deadline {
                return;
            }
            expected
        };
        self.assemble_sequence(next_sequence_start);
    }

    /// Assemble what the frequency track still holds of the image once it
    /// has run out: every sequence whose scans have all begun, with the
    /// missing tail standing in from the newest estimate.
    pub(super) fn assemble_remaining_sequences(&mut self) {
        while let Some(image) = &self.image {
            let last_scan_start = image
                .mode
                .step_offsets()
                .filter(|(_, step)| step.is_scan())
                .map(|(offset, _)| offset)
                .last()
                .unwrap_or(Duration::from_ns(0));
            if image.sequence_start + self.samples_in(last_scan_start)
                >= self.estimates.end() as f64
            {
                self.end_image();
                return;
            }
            let next_sequence_start = image
                .next_sequence_start
                .unwrap_or_else(|| self.expected_next_sequence_start());
            self.assemble_sequence(next_sequence_start);
        }
    }

    /// Where the sequence after the current one begins if the signal keeps
    /// its nominal timing.
    fn expected_next_sequence_start(&self) -> f64 {
        self.image.as_ref().map_or(0.0, |image| {
            image.sequence_start + self.samples_in(image.mode.sequence_duration())
        })
    }

    /// Cut the current sequence, stretched to end at `next_sequence_start`,
    /// into one tone per control step and one per pixel.
    fn assemble_sequence(&mut self, next_sequence_start: f64) {
        let Some(image) = &self.image else {
            return;
        };
        let mode = image.mode;
        let start = image.sequence_start;
        let stretch = (next_sequence_start - start) / self.samples_in(mode.sequence_duration());
        let width = mode.resolution.0;

        for (offset, step) in mode.step_offsets() {
            let step_start = start + self.samples_in(offset) * stretch;
            match step {
                Step::Control(tone) => {
                    let length = self.samples_in(tone.duration) * stretch;
                    self.emit(step_start, length);
                }
                Step::Scan(_, duration) => {
                    let pixel_length = self.samples_in(*duration) * stretch / width as f64;
                    for column in 0..width {
                        self.emit(step_start + column as f64 * pixel_length, pixel_length);
                    }
                }
            }
        }

        self.estimates.forget_before(next_sequence_start as u64);
        let Some(image) = &mut self.image else {
            return;
        };
        image.sequence_start = next_sequence_start;
        image.next_sequence_start = None;
        image.sequences_left -= 1;
        if image.sequences_left == 0 {
            self.end_image();
        }
    }

    /// Queue the tone made of the estimates from `start` over `length`
    /// samples.
    fn emit(&mut self, start: f64, length: f64) {
        let duration = Duration::from_ns((length * 1e9 / f64::from(self.sample_rate)) as u64);
        let tone = Tone::new(self.estimates.mean(start, length), duration);
        self.ready.push_back(tone);
    }

    /// Stop assembling the image and search for the next header.
    fn end_image(&mut self) {
        self.image = None;
        self.restart_search();
    }

    /// The duration in (fractional) samples.
    fn samples_in(&self, duration: Duration) -> f64 {
        duration.ns() as f64 * f64::from(self.sample_rate) / 1e9
    }
}
