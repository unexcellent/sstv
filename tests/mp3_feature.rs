//! Tests for the `mp3` feature: encoding the synthesized audio into an
//! in-memory MP3.

use sstv::{Encoder, Mode, RgbPixel};

#[test]
fn encodes_a_transmission_into_an_mp3() {
    let image = vec![RgbPixel::new(128, 64, 32); 320 * 240];
    let encoder = Encoder::new(Mode::Robot36, image.into_iter()).expect("construct encoder");

    let mp3 = encoder.to_mp3(24_000).expect("encode mp3");

    // A 36 second transmission at 128 kbps is roughly half a megabyte; a
    // broken encoding would be empty or tiny. Frames start on a sync word.
    assert!(mp3.len() > 100_000, "mp3 is only {} bytes", mp3.len());
    assert_eq!(mp3[0], 0xFF, "mp3 should start with a frame sync word");
}
