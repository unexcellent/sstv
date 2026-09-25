// Test helpers outside #[test] functions are not covered by the clippy.toml
// test allowances.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Round-trip test for every mode: encode a test image, decode the samples
//! back, and compare against the original.

use sstv::{Decoder, Encoder, Event, Mode, RgbPixel, Synthesizer, modes};

const SAMPLE_RATE: u32 = 24_000;

/// Acceptable mean absolute per-channel error between original and decode.
const MAX_ERROR: f64 = 12.0;

/// A test image with variation in all three channels.
fn test_image(width: usize, height: usize) -> Vec<RgbPixel> {
    let mut pixels = Vec::with_capacity(width * height);
    for y in 0..height as u32 {
        for x in 0..width as u32 {
            let red = (x * 255 / (width as u32 - 1)) as u8;
            let green = (y * 255 / (height as u32 - 1)) as u8;
            let blue = ((x + y) * 255 / (width as u32 - 1 + height as u32 - 1)) as u8;
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
            let d = |x: u8, y: u8| u64::from((i32::from(x) - i32::from(y)).unsigned_abs());
            d(p.red(), q.red()) + d(p.green(), q.green()) + d(p.blue(), q.blue())
        })
        .sum();
    total as f64 / (a.len() as f64 * 3.0)
}

/// Decode `samples` event by event, expecting an image in the given mode with
/// its rows complete, in order, and close to `image`. `expected_mode` pins the
/// decoder's mode; `None` detects it from the header.
fn assert_decodes(expected_mode: Option<Mode>, samples: &[i16], mode: Mode, image: &[RgbPixel]) {
    let width = mode.image_width() as usize;
    let height = mode.image_height() as usize;

    let mut events = Decoder::from_samples(samples.iter().copied(), SAMPLE_RATE);
    if let Some(expected) = expected_mode {
        events = events.expect_mode(expected);
    }

    let mut decoded: Vec<RgbPixel> = Vec::new();
    let mut complete = None;
    for event in events.events() {
        match event {
            Event::ImageStart(started) => assert_eq!(started, mode),
            Event::Row(row) => {
                assert_eq!(row.index() * width, decoded.len(), "row out of order");
                decoded.extend_from_slice(row.pixels());
            }
            Event::ImageEnd { complete: flag } => complete = Some(flag),
        }
    }

    assert_eq!(complete, Some(true), "image should decode completely");
    assert_eq!(decoded.len(), width * height, "should decode every row");
    let error = mean_abs_error(image, &decoded);
    assert!(error < MAX_ERROR, "mean abs error {error} too high");
}

/// Encode an image, then decode it back — once with the mode given explicitly
/// and once detecting it from the header.
fn round_trip(mode: Mode) {
    let width = mode.image_width() as usize;
    let height = mode.image_height() as usize;
    let image = test_image(width, height);

    let encoder = Encoder::new(mode, image.clone().into_iter()).expect("construct encoder");
    let samples: Vec<i16> = Synthesizer::new(encoder, SAMPLE_RATE).collect();

    assert_decodes(Some(mode), &samples, mode, &image);
    assert_decodes(None, &samples, mode, &image);
}

#[test]
fn scottie_1() {
    round_trip(modes::SCOTTIE_1);
}

#[test]
fn scottie_2() {
    round_trip(modes::SCOTTIE_2);
}

#[test]
fn scottie_dx() {
    round_trip(modes::SCOTTIE_DX);
}

#[test]
fn martin_1() {
    round_trip(modes::MARTIN_1);
}

#[test]
fn martin_2() {
    round_trip(modes::MARTIN_2);
}

#[test]
fn robot_36() {
    round_trip(modes::ROBOT_36);
}

#[test]
fn robot_72() {
    round_trip(modes::ROBOT_72);
}

#[test]
fn wrasse_sc2_180() {
    round_trip(modes::WRASSE_SC2_180);
}

#[test]
fn pasokon_p3() {
    round_trip(modes::PASOKON_P3);
}

#[test]
fn pasokon_p5() {
    round_trip(modes::PASOKON_P5);
}

#[test]
fn pasokon_p7() {
    round_trip(modes::PASOKON_P7);
}

#[test]
fn pd_50() {
    round_trip(modes::PD_50);
}

#[test]
fn pd_90() {
    round_trip(modes::PD_90);
}

#[test]
fn pd_120() {
    round_trip(modes::PD_120);
}

#[test]
fn pd_160() {
    round_trip(modes::PD_160);
}

#[test]
fn pd_180() {
    round_trip(modes::PD_180);
}

#[test]
fn pd_240() {
    round_trip(modes::PD_240);
}

#[test]
fn pd_290() {
    round_trip(modes::PD_290);
}
