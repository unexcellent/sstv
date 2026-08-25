//! Decode an SSTV WAV back into an image.
//!
//! Accepts a plain `.wav` or a gzipped `.wav.gz` and writes the decoded image
//! to the given path (format inferred from its extension).
//!
//! ```text
//! cargo run --example decode -- tests/assets/real_recording.wav.gz local/decoded.png [mode]
//! ```
//!
//! Rows that could not be decoded are left black, so a misaligned or truncated
//! decode is still visible rather than fatal.

use std::env;
use std::fs::File;
use std::io::{Cursor, Read};

use sstv::{Decoder, Mode, RgbPixel};

fn parse_mode(name: &str) -> Mode {
    Mode::ALL
        .into_iter()
        .chain([Mode::Auto])
        .find(|mode| format!("{mode:?}").eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("unknown mode {name}, expected one of {:?}", Mode::ALL))
}

fn main() {
    let mut args = env::args().skip(1);
    let usage = "usage: decode <input.wav|input.wav.gz> <output image> [mode]";
    let input = args.next().expect(usage);
    let output = args.next().expect(usage);
    let mode = args
        .next()
        .map(|name| parse_mode(&name))
        .unwrap_or(Mode::Auto);

    let (samples, sample_rate) = read_samples(&input);
    println!("read {} samples at {sample_rate} Hz", samples.len());

    let Some(image) = Decoder::from_samples(mode, samples.into_iter(), sample_rate)
        .images()
        .next()
    else {
        panic!("no image found in {input}");
    };
    println!(
        "decoded a {}x{} {:?} image (complete: {})",
        image.width(),
        image.height(),
        image.mode(),
        image.complete(),
    );

    save_image(
        image.pixels(),
        image.width() as u32,
        image.height() as u32,
        &output,
    );
    println!("wrote {output}");
}

/// Read 16-bit PCM samples from a WAV, decompressing first if the path ends in
/// `.gz`. Returns the first channel and the sample rate.
fn read_samples(path: &str) -> (Vec<i16>, u32) {
    let file = File::open(path).expect("open input file");

    let mut bytes = Vec::new();
    if path.ends_with(".gz") {
        flate2::read::GzDecoder::new(file)
            .read_to_end(&mut bytes)
            .expect("gunzip input");
    } else {
        let mut file = file;
        file.read_to_end(&mut bytes).expect("read input");
    }

    let reader = hound::WavReader::new(Cursor::new(bytes)).expect("parse wav");
    let channels = reader.spec().channels as usize;
    let sample_rate = reader.spec().sample_rate;
    let samples = reader
        .into_samples::<i16>()
        .map(|sample| sample.expect("read sample"))
        .step_by(channels)
        .collect();
    (samples, sample_rate)
}

fn save_image(pixels: &[RgbPixel], width: u32, height: u32, path: &str) {
    let mut image = image::RgbImage::new(width, height);
    for (index, pixel) in pixels.iter().take((width * height) as usize).enumerate() {
        let x = index as u32 % width;
        let y = index as u32 / width;
        image.put_pixel(x, y, image::Rgb([pixel.red(), pixel.green(), pixel.blue()]));
    }
    image.save(path).expect("save output image");
}
