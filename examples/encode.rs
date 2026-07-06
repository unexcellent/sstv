//! Encode examples/patch.png into a Robot36 WAV using this crate's encoder.
//!
//! ```text
//! cargo run --example encode -- local/encoded.wav [sample_rate]
//! ```

use image::GenericImageView;
use sstv::{Encoder, Mode, RgbPixel, Synthesizer};
use std::env;

fn main() {
    let mut args = env::args().skip(1);
    let output = args
        .next()
        .expect("usage: encode <output.wav> [sample_rate]");
    let sample_rate: u32 = args
        .next()
        .map(|s| s.parse().expect("sample rate must be an integer"))
        .unwrap_or(48_000);

    let image = image::open("examples/patch.png").expect("open examples/patch.png");
    let (width, height) = image.dimensions();
    assert_eq!((width, height), (320, 240), "image must be 320x240");

    let mut pixels = std::vec![RgbPixel::new(0, 0, 0); (width * height) as usize];
    image.pixels().for_each(|(x, y, rgba)| {
        pixels[(y * width + x) as usize] = RgbPixel::new(rgba[0], rgba[1], rgba[2]);
    });

    let encoder = Encoder::new(Mode::Robot36, pixels.into_iter()).expect("encode");

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

    println!("wrote {output} at {sample_rate} Hz");
}
