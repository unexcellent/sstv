// Test helpers outside #[test] functions are not covered by the clippy.toml
// test allowances.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests against transmissions published on Wikimedia Commons, made with
//! other encoders (QSSTV, MMSSTV), mostly together with a decode of them.
//! The files are fetched by `tests/scripts/fetch_assets.py`, which also
//! credits their authors and states their licences.

mod common;
use common::{asset, mean_abs_error, read_audio, read_image, save_decoded};
use sstv::{DecodedImage, Decoder, Mode, modes};

/// A Martin 1 transmission made with QSSTV at 11 025 Hz, compared with the
/// decode published alongside it.
#[test]
fn decodes_a_martin_1_transmission_by_qsstv() {
    let decoded = decode("martin1-sunset.ogg");

    assert_decodes_completely(&decoded, modes::MARTIN_1);
    assert_close_to_reference(&decoded, "martin1-sunset.png", 10.0);
}

/// A Robot 36 transmission made with MMSSTV. The published decode, made with
/// the Robot36 app, carries colour streaks ours does not, so it is only a
/// rough reference.
#[test]
fn decodes_a_robot_36_transmission_by_mmsstv() {
    let decoded = decode("robot36-french-logo.flac");

    assert_decodes_completely(&decoded, modes::ROBOT_36);
    assert_close_to_reference(&decoded, "robot36-french-logo.png", 25.0);
}

/// A further Martin 1 transmission, published without a decode.
#[test]
fn decodes_a_second_martin_1_transmission() {
    let decoded = decode("martin1-german-logo.ogg");

    assert_decodes_completely(&decoded, modes::MARTIN_1);
}

/// Decode a file from `tests/assets/commons/`, saving the image under the
/// file's name.
fn decode(name: &str) -> DecodedImage {
    let (samples, sample_rate) = read_audio(&asset(&format!("commons/{name}")));
    let decoded = Decoder::new(samples.into_iter(), sample_rate)
        .decode()
        .expect("an image");
    let stem = name.split('.').next().unwrap_or(name);
    save_decoded(stem, &decoded);
    decoded
}

fn assert_decodes_completely(decoded: &DecodedImage, mode: Mode) {
    assert_eq!(decoded.mode(), mode);
    assert!(decoded.complete(), "image should decode completely");
}

fn assert_close_to_reference(decoded: &DecodedImage, reference: &str, max_error: f64) {
    let reference = read_image(&asset(&format!("commons/{reference}")));
    let error = mean_abs_error(&reference, decoded.pixels());
    assert!(error < max_error, "mean abs error {error} too high");
}
