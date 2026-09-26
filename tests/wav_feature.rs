// Test helpers outside #[test] functions are not covered by the clippy.toml
// test allowances.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests for the `wav` feature: encoding to and decoding from in-memory WAVs.

mod common;
use common::{mean_abs_error, test_image};
use sstv::{Decoder, Encoder, Synthesizer, modes};

const SAMPLE_RATE: u32 = 24_000;

#[test]
fn round_trips_through_a_wav() {
    let image = test_image(modes::ROBOT_36);
    let encoder = Encoder::new(modes::ROBOT_36, image.clone().into_iter()).expect("encode");
    let wav = encoder.to_wav(SAMPLE_RATE);

    let decoded = Decoder::from_wav(&wav)
        .expect("parse wav")
        .images()
        .next()
        .expect("an image");

    assert_eq!(decoded.mode(), modes::ROBOT_36);
    assert!(decoded.complete(), "image should decode completely");
    let error = mean_abs_error(&image, decoded.pixels());
    assert!(error < 12.0, "mean abs error {error} too high");
}

/// Stereo float WAVs are down-converted: first channel, scaled to 16 bit.
#[test]
fn decodes_stereo_float_wavs() {
    let image = test_image(modes::ROBOT_36);
    let encoder = Encoder::new(modes::ROBOT_36, image.clone().into_iter()).expect("encode");
    let samples: Vec<i16> = Synthesizer::new(encoder, SAMPLE_RATE).collect();

    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut cursor = std::io::Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(&mut cursor, spec).expect("write wav");
    for sample in samples {
        let value = f32::from(sample) / f32::from(i16::MAX);
        writer.write_sample(value).expect("write sample");
        writer.write_sample(0.0f32).expect("write sample"); // silent right channel
    }
    writer.finalize().expect("finalize wav");

    let decoded = Decoder::from_wav(cursor.get_ref())
        .expect("parse wav")
        .expect_mode(modes::ROBOT_36)
        .images()
        .next()
        .expect("an image");

    assert!(decoded.complete(), "image should decode completely");
    let error = mean_abs_error(&image, decoded.pixels());
    assert!(error < 12.0, "mean abs error {error} too high");
}

#[test]
fn malformed_wav_reports_an_error() {
    assert!(Decoder::from_wav(b"not a wav").is_err());
}

/// A WAV whose data chunk is shorter than its header declares decodes up to
/// the cut, with the missing rows left black.
#[test]
fn decodes_a_truncated_wav() {
    let image = test_image(modes::ROBOT_36);
    let encoder = Encoder::new(modes::ROBOT_36, image.clone().into_iter()).expect("encode");
    let wav = encoder.to_wav(SAMPLE_RATE);

    // Cut a quarter of the audio without adjusting the header sizes.
    let truncated = &wav[..wav.len() - (wav.len() - 44) / 4];

    let decoded = Decoder::from_wav(truncated)
        .expect("parse truncated wav")
        .images()
        .next()
        .expect("an image");

    assert_eq!(decoded.mode(), modes::ROBOT_36);
    assert!(!decoded.complete(), "a truncated image is not complete");
    let pixels = decoded.pixels();
    let last = pixels.last().expect("pixels");
    assert_eq!((last.red(), last.green(), last.blue()), (0, 0, 0));

    let (width, height) = modes::ROBOT_36.resolution();
    let decoded_rows = pixels.len() / width as usize;
    assert_eq!(decoded_rows, height as usize);
    let error = mean_abs_error(&image[..pixels.len() / 2], &pixels[..pixels.len() / 2]);
    assert!(error < 12.0, "mean abs error {error} too high");
}
