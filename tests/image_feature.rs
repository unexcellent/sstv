//! Tests for the `image` feature: encoding images loaded from disk.

use sstv::{Decoder, Encoder, Error, Event, Mode, Synthesizer};

const SAMPLE_RATE: u32 = 24_000;

/// Decode a transmission and return its pixels once complete.
fn decode(mode: Mode, samples: Vec<i16>) -> Vec<sstv::RgbPixel> {
    let mut decoded = Vec::new();
    let mut complete = None;
    for event in Decoder::from_samples(mode, samples.into_iter(), SAMPLE_RATE).events() {
        match event {
            Event::ImageStart(_) => {}
            Event::Row(row) => decoded.extend_from_slice(row.pixels()),
            Event::ImageEnd { complete: flag } => complete = Some(flag),
        }
    }
    assert_eq!(complete, Some(true), "image should decode completely");
    decoded
}

#[test]
fn encodes_an_image_file() {
    let encoder =
        Encoder::from_image_path(Mode::Robot36, "examples/patch.png").expect("load image");
    let samples: Vec<i16> = Synthesizer::new(encoder, SAMPLE_RATE).collect();

    let decoded = decode(Mode::Robot36, samples);
    let expected = Mode::Robot36.image_width() * Mode::Robot36.image_height();
    assert_eq!(decoded.len(), expected as usize);
}

#[test]
fn resizes_to_the_mode_resolution() {
    let small = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(100, 80, |x, y| {
        image::Rgb([(x * 2) as u8, (y * 3) as u8, 128])
    }));
    let encoder = Encoder::from_image(Mode::Robot36, &small).expect("encode resized image");
    let samples: Vec<i16> = Synthesizer::new(encoder, SAMPLE_RATE).collect();

    let decoded = decode(Mode::Robot36, samples);
    let expected = Mode::Robot36.image_width() * Mode::Robot36.image_height();
    assert_eq!(decoded.len(), expected as usize);
}

#[test]
fn missing_file_reports_an_error() {
    assert!(matches!(
        Encoder::from_image_path(Mode::Robot36, "does-not-exist.png"),
        Err(Error::Image(_))
    ));
}
