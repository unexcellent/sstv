//! Encode examples/patch.png into an SSTV WAV using this crate's encoder.
//!
//! ```text
//! cargo run --example encode -- local/encoded.wav [mode] [sample_rate]
//! ```

use image::imageops::FilterType;
use sstv::{Encoder, Mode, RgbPixel, Synthesizer};
use std::env;

fn parse_mode(name: &str) -> Mode {
    Mode::ALL
        .into_iter()
        .chain([Mode::Auto])
        .find(|mode| format!("{mode:?}").eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("unknown mode {name}, expected one of {:?}", Mode::ALL))
}

fn main() {
    let mut args = env::args().skip(1);
    let output = args
        .next()
        .expect("usage: encode <output.wav> [mode] [sample_rate]");
    let mode = args
        .next()
        .map(|name| parse_mode(&name))
        .unwrap_or(Mode::Robot36);
    let sample_rate: u32 = args
        .next()
        .map(|s| s.parse().expect("sample rate must be an integer"))
        .unwrap_or(48_000);

    let (width, height) = (mode.image_width(), mode.image_height());
    let image = image::open("examples/patch.png")
        .expect("open examples/patch.png")
        .resize_exact(width, height, FilterType::Triangle)
        .to_rgb8();

    let mut pixels = std::vec![RgbPixel::new(0, 0, 0); (width * height) as usize];
    for (x, y, rgb) in image.enumerate_pixels() {
        pixels[(y * width + x) as usize] = RgbPixel::new(rgb[0], rgb[1], rgb[2]);
    }

    let encoder = Encoder::new(mode, pixels.into_iter()).expect("encode");

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&output, spec).expect("create wav");
    for sample in Synthesizer::new(encoder, sample_rate) {
        writer.write_sample(sample).expect("write sample");
    }
    writer.finalize().expect("finalize wav");

    println!("wrote {output} as {mode:?} at {sample_rate} Hz");
}
