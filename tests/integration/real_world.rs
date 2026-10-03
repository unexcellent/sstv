//! End-to-end test against a real off-air recording captured by a ground
//! station — the true reception path, exercising receiver imperfections (drift,
//! noise, level and DC variation) that synthetic signals do not exhibit.

use crate::common::{asset, mean_abs_error, read_audio, read_image, save_decoded};
use sstv::{Decoder, modes};

/// The decoder should reconstruct a real off-air Robot 36 recording (32 kHz,
/// mono) captured by a ground station — the true end-to-end path, with real
/// receiver imperfections that synthetic signals do not exhibit.
#[test]
fn decodes_real_ground_station_recording() {
    let (samples, sample_rate) = read_audio(&asset("real_recording.wav.gz"));

    let decoded = Decoder::new(samples.into_iter(), sample_rate)
        .with_mode(modes::ROBOT_36)
        .decode()
        .expect("an image in the recording");
    save_decoded("ground-station", &decoded);

    assert!(decoded.complete(), "image should decode completely");
    let error = mean_abs_error(&read_image(&asset("patch.png")), decoded.pixels());
    // Real reception drifts a little in colour/timing; a broken decode is 40+.
    assert!(error < 15.0, "decode error {error} too high");
}
