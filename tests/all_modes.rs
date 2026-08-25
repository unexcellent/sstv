//! Round-trip test for every mode: encode a test image, decode the samples
//! back, and compare against the original.

use sstv::{Encoder, Event, Mode, RgbPixel, RowDecoder, Synthesizer};

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
            let d = |x: u8, y: u8| (x as i32 - y as i32).unsigned_abs() as u64;
            d(p.red(), q.red()) + d(p.green(), q.green()) + d(p.blue(), q.blue())
        })
        .sum();
    total as f64 / (a.len() as f64 * 3.0)
}

fn round_trip(mode: Mode) {
    let width = mode.image_width() as usize;
    let height = mode.image_height() as usize;
    let image = test_image(width, height);

    let encoder = Encoder::new(mode, image.clone().into_iter()).expect("construct encoder");
    let samples: Vec<i16> = Synthesizer::new(encoder, SAMPLE_RATE).collect();

    let mut decoded: Vec<RgbPixel> = Vec::new();
    let mut complete = None;
    for event in RowDecoder::new(mode, samples.into_iter(), SAMPLE_RATE) {
        match event {
            Event::ImageStart(info) => {
                assert_eq!(info.mode(), mode);
                assert_eq!(info.width(), width);
                assert_eq!(info.height(), height);
            }
            Event::Row(row) => {
                assert_eq!(row.index() * width, decoded.len(), "row out of order");
                decoded.extend_from_slice(row.pixels());
            }
            Event::ImageEnd { complete: flag } => complete = Some(flag),
        }
    }

    assert_eq!(complete, Some(true), "image should decode completely");
    assert_eq!(decoded.len(), width * height, "should decode every row");
    let error = mean_abs_error(&image, &decoded);
    assert!(error < MAX_ERROR, "mean abs error {error} too high");
}

#[test]
fn scottie_1() {
    round_trip(Mode::Scottie1);
}

#[test]
fn scottie_2() {
    round_trip(Mode::Scottie2);
}

#[test]
fn scottie_dx() {
    round_trip(Mode::ScottieDx);
}

#[test]
fn martin_1() {
    round_trip(Mode::Martin1);
}

#[test]
fn martin_2() {
    round_trip(Mode::Martin2);
}

#[test]
fn robot_36() {
    round_trip(Mode::Robot36);
}

#[test]
fn robot_72() {
    round_trip(Mode::Robot72);
}

#[test]
fn wrasse_sc2_180() {
    round_trip(Mode::WrasseSc2180);
}

#[test]
fn pasokon_p3() {
    round_trip(Mode::PasokonP3);
}

#[test]
fn pasokon_p5() {
    round_trip(Mode::PasokonP5);
}

#[test]
fn pasokon_p7() {
    round_trip(Mode::PasokonP7);
}

#[test]
fn pd_50() {
    round_trip(Mode::Pd50);
}

#[test]
fn pd_90() {
    round_trip(Mode::Pd90);
}

#[test]
fn pd_120() {
    round_trip(Mode::Pd120);
}

#[test]
fn pd_160() {
    round_trip(Mode::Pd160);
}

#[test]
fn pd_180() {
    round_trip(Mode::Pd180);
}

#[test]
fn pd_240() {
    round_trip(Mode::Pd240);
}

#[test]
fn pd_290() {
    round_trip(Mode::Pd290);
}
