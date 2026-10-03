//! Round-trip test for every mode: encode a test image, decode the samples
//! back, and compare against the original.

use crate::common::{mean_abs_error, save_decoded, test_image};
use sstv::{Decoder, Encoder, Mode, RgbPixel, Synthesizer, modes};

const SAMPLE_RATE: u32 = 24_000;

/// Acceptable mean absolute per-channel error between original and decode.
const MAX_ERROR: f64 = 12.0;

/// Decode `samples`, expecting a complete image in the given mode close to
/// `image`. `expected_mode` pins the decoder's mode; `None` detects it from
/// the header. The decoded image is saved as `name`.
fn assert_decodes(
    name: &str,
    expected_mode: Option<Mode>,
    samples: &[i16],
    mode: Mode,
    image: &[RgbPixel],
) {
    let mut decoder = Decoder::new(samples.iter().copied(), SAMPLE_RATE);
    if let Some(expected) = expected_mode {
        decoder = decoder.with_mode(expected);
    }

    let image_decoded = decoder.decode().expect("an image");
    save_decoded(name, &image_decoded);

    assert_eq!(image_decoded.mode(), mode);
    assert!(image_decoded.complete(), "image should decode completely");
    let error = mean_abs_error(image, image_decoded.pixels());
    assert!(error < MAX_ERROR, "mean abs error {error} too high");
}

/// Encode an image, then decode it back — once with the mode given explicitly
/// and once detecting it from the header.
fn round_trip(mode: Mode) {
    let image = test_image(mode);

    let encoder = Encoder::new(mode, image.clone().into_iter()).expect("construct encoder");
    let samples: Vec<i16> = Synthesizer::new(encoder, SAMPLE_RATE).collect();

    assert_decodes(
        &format!("{mode:?}-with-mode"),
        Some(mode),
        &samples,
        mode,
        &image,
    );
    assert_decodes(&format!("{mode:?}-detected"), None, &samples, mode, &image);
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
