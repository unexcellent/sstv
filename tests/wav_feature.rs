//! Tests for the `wav` feature: encoding to and decoding from in-memory WAVs.

use sstv::{Decoder, Encoder, Mode, RgbPixel, Synthesizer};

const SAMPLE_RATE: u32 = 24_000;

/// A test image with variation in all three channels.
fn test_image(mode: Mode) -> Vec<RgbPixel> {
    let (width, height) = (mode.image_width(), mode.image_height());
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for y in 0..height {
        for x in 0..width {
            let red = (x * 255 / (width - 1)) as u8;
            let green = (y * 255 / (height - 1)) as u8;
            let blue = ((x + y) * 255 / (width + height - 2)) as u8;
            pixels.push(RgbPixel::new(red, green, blue));
        }
    }
    pixels
}

/// Mean absolute per-channel error between two images of equal length.
fn mean_abs_error(a: &[RgbPixel], b: &[RgbPixel]) -> f64 {
    assert_eq!(a.len(), b.len());
    let total: u64 = a
        .iter()
        .zip(b)
        .map(|(p, q)| {
            let d = |x: u8, y: u8| (x as i32 - y as i32).unsigned_abs() as u64;
            d(p.red(), q.red()) + d(p.green(), q.green()) + d(p.blue(), q.blue())
        })
        .sum();
    total as f64 / (a.len() as f64 * 3.0)
}

#[test]
fn round_trips_through_a_wav() {
    let image = test_image(Mode::Robot36);
    let encoder = Encoder::new(Mode::Robot36, image.clone().into_iter()).expect("encode");
    let wav = encoder.to_wav(SAMPLE_RATE);

    let decoded = Decoder::from_wav(Mode::Auto, &wav)
        .expect("parse wav")
        .images()
        .next()
        .expect("an image");

    assert_eq!(decoded.mode(), Mode::Robot36);
    assert!(decoded.complete(), "image should decode completely");
    let error = mean_abs_error(&image, decoded.pixels());
    assert!(error < 12.0, "mean abs error {error} too high");
}

/// Stereo float WAVs are down-converted: first channel, scaled to 16 bit.
#[test]
fn decodes_stereo_float_wavs() {
    let image = test_image(Mode::Robot36);
    let encoder = Encoder::new(Mode::Robot36, image.clone().into_iter()).expect("encode");
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
        let value = sample as f32 / i16::MAX as f32;
        writer.write_sample(value).expect("write sample");
        writer.write_sample(0.0f32).expect("write sample"); // silent right channel
    }
    writer.finalize().expect("finalize wav");

    let decoded = Decoder::from_wav(Mode::Robot36, cursor.get_ref())
        .expect("parse wav")
        .images()
        .next()
        .expect("an image");

    assert!(decoded.complete(), "image should decode completely");
    let error = mean_abs_error(&image, decoded.pixels());
    assert!(error < 12.0, "mean abs error {error} too high");
}

#[test]
fn malformed_wav_reports_an_error() {
    assert!(Decoder::from_wav(Mode::Auto, b"not a wav").is_err());
}
