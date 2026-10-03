// Test helpers outside #[test] functions are not covered by the clippy.toml
// test allowances.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests against real off-air ISS SSTV recordings, captured by KG4AKV (Space
//! Comms) and encoded on orbit with MMSSTV — the de-facto standard encoder.
//!
//! The recordings stay outside the git history; the first test run fetches
//! them (~130 MB) via `tests/scripts/fetch_assets.py`, together with the
//! decodes KG4AKV published of them.

mod common;
use common::{asset, mean_abs_error, read_audio, read_image, save_decoded};
use sstv::{DecodedImage, Decoder, Demodulator, Encoder, Mode, Synthesizer, modes};

const PD_120_PERIOD: f64 = 0.508_48;
const PD_180_PERIOD: f64 = 0.754_24;

struct Recording {
    /// The recording's file name in `tests/assets/iss/`, without extension.
    name: &'static str,
    mode: Mode,
    /// The mode's line period in seconds.
    period: f64,
    /// KG4AKV's decode of the recording in `tests/assets/iss/`.
    reference: &'static str,
}

const RECORDINGS: &[Recording] = &[
    Recording {
        name: "pd180-gagarin-80",
        mode: modes::PD_180,
        period: PD_180_PERIOD,
        reference: "pd180-gagarin-80.kg4akv.jpg",
    },
    Recording {
        name: "pd180-apollo-soyuz",
        mode: modes::PD_180,
        period: PD_180_PERIOD,
        reference: "pd180-apollo-soyuz.kg4akv.jpg",
    },
    Recording {
        name: "pd180-ariss-qso-astros",
        mode: modes::PD_180,
        period: PD_180_PERIOD,
        reference: "pd180-ariss-qso-astros.kg4akv.png",
    },
    Recording {
        name: "pd180-ariss-qso-cristoforetti",
        mode: modes::PD_180,
        period: PD_180_PERIOD,
        reference: "pd180-ariss-qso-cristoforetti.kg4akv.png",
    },
    Recording {
        name: "pd180-mai75-suitsat",
        mode: modes::PD_180,
        period: PD_180_PERIOD,
        reference: "pd180-mai75-suitsat.kg4akv.png",
    },
    Recording {
        name: "pd120-ariss-20-year-1",
        mode: modes::PD_120,
        period: PD_120_PERIOD,
        reference: "pd120-ariss-20-year-1.kg4akv.png",
    },
    Recording {
        name: "pd120-ariss-20-year-2",
        mode: modes::PD_120,
        period: PD_120_PERIOD,
        reference: "pd120-ariss-20-year-2.kg4akv.png",
    },
];

/// The bytes of the recording's WAV file.
fn recording(name: &str) -> Vec<u8> {
    std::fs::read(asset(&format!("iss/{name}.wav"))).expect("read the recording")
}

/// `expected_mode` pins the decoder's mode; `None` detects it from the header.
fn decode(expected_mode: Option<Mode>, wav: &[u8]) -> DecodedImage {
    let mut decoder = Decoder::from_wav(wav).expect("parse wav");
    if let Some(expected) = expected_mode {
        decoder = decoder.with_mode(expected);
    }
    decoder.decode().expect("an image")
}

/// The median spacing and length of the line sync pulses in a signal, in
/// seconds. Spacings are filtered to those near `expected_period` so that
/// header tones and reception glitches do not skew the median.
fn line_timing(samples: &[i16], sample_rate: u32, expected_period: f64) -> (f64, f64) {
    let mut starts: Vec<usize> = Vec::new();
    let mut lengths: Vec<usize> = Vec::new();
    let mut run = 0usize;
    // PD sync pulses are 20 ms; half of that separates them from glitches.
    let min_run = (f64::from(sample_rate) * 0.010) as usize;
    for (index, frequency) in Demodulator::new(samples.iter().copied(), sample_rate).enumerate() {
        if frequency.hz().abs_diff(1200) <= 150 {
            run += 1;
        } else {
            if run >= min_run {
                starts.push(index - run);
                lengths.push(run);
            }
            run = 0;
        }
    }

    let mut periods: Vec<f64> = starts
        .windows(2)
        .map(|pair| (pair[1] - pair[0]) as f64 / f64::from(sample_rate))
        .filter(|period| (period - expected_period).abs() < expected_period * 0.1)
        .collect();
    periods.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut lengths: Vec<f64> = lengths
        .into_iter()
        .map(|length| length as f64 / f64::from(sample_rate))
        .collect();
    lengths.sort_by(|a, b| a.partial_cmp(b).unwrap());

    assert!(periods.len() > 20, "too few line syncs found");
    (periods[periods.len() / 2], lengths[lengths.len() / 2])
}

/// The recordings decode completely, with the mode detected from the
/// transmitted VIS code, and close to KG4AKV's own decodes. Those were made
/// with other software (MMSSTV, a phone app), possibly from other passes, so
/// they differ in noise and colour; a misaligned or colour-swapped decode
/// would be far off (40+).
#[test]
fn decodes_the_recordings() {
    for entry in RECORDINGS {
        let image = decode(None, &recording(entry.name));
        save_decoded(entry.name, &image);

        assert_eq!(image.mode(), entry.mode, "{}", entry.name);
        assert!(image.complete(), "{} should decode completely", entry.name);
        let reference = read_image(&asset(&format!("iss/{}", entry.reference)));
        let error = mean_abs_error(&reference, image.pixels());
        assert!(
            error < 25.0,
            "{}: mean abs error {error} too high",
            entry.name
        );
    }
}

/// Re-encoding the decoded image must reproduce the recorded transmission:
/// the synthesized tones match the recording's line timing, and decoding them
/// returns the same image.
#[test]
fn reencoding_matches_the_recorded_tones_and_images() {
    for entry in RECORDINGS {
        let (path, mode, period) = (entry.name, entry.mode, entry.period);
        let wav = recording(path);
        let (recorded_samples, sample_rate) = read_audio(&asset(&format!("iss/{path}.wav")));
        let recorded_image = decode(Some(mode), &wav);

        let encoder = Encoder::new(mode, recorded_image.pixels().to_vec().into_iter())
            .expect("construct encoder");
        let synthesized: Vec<i16> = Synthesizer::new(encoder, sample_rate).collect();

        // The tones: line sync pulses must be spaced and sized like the
        // recording's (which MMSSTV derived from the same paper timings).
        let (recorded_period, recorded_sync) = line_timing(&recorded_samples, sample_rate, period);
        let (our_period, our_sync) = line_timing(&synthesized, sample_rate, period);
        let period_error = (recorded_period - our_period).abs() / recorded_period;
        assert!(
            period_error < 0.003,
            "{path}: line period differs by {:.3}% ({recorded_period}s vs {our_period}s)",
            period_error * 100.0
        );
        let sync_error = (recorded_sync - our_sync).abs() / recorded_sync;
        assert!(
            sync_error < 0.15,
            "{path}: sync length differs by {:.1}% ({recorded_sync}s vs {our_sync}s)",
            sync_error * 100.0
        );

        // The images: scan frequencies map linearly to pixel values, so this
        // bounds the mean tone error (one pixel step is ~3.1 Hz).
        let mut wav_out = std::io::Cursor::new(Vec::new());
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::new(&mut wav_out, spec).expect("write wav");
        for sample in &synthesized {
            writer.write_sample(*sample).expect("write sample");
        }
        writer.finalize().expect("finalize wav");
        let reencoded_image = decode(Some(mode), wav_out.get_ref());

        let error = mean_abs_error(recorded_image.pixels(), reencoded_image.pixels());
        assert!(error < 10.0, "{path}: mean abs error {error} too high");
    }
}
