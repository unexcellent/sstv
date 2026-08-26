//! Encode examples/patch.png into an SSTV WAV using this crate's encoder.
//! Loading the image is done with the `image` crate.
//!
//! ```text
//! cargo run --features image --example encode -- local/encoded.wav [mode] [sample_rate]
//! ```

use sstv::{Encoder, Mode, Synthesizer};
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

    let image = image::open("examples/patch.png").expect("open examples/patch.png");
    let encoder = Encoder::from_image(mode, &image).expect("encode");

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
