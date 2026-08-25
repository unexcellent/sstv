//! Decode a Robot36 WAV back into an image.
//!
//! Accepts a plain `.wav` or a gzipped `.wav.gz` and writes the decoded image
//! to the given path (format inferred from its extension).
//!
//! ```text
//! cargo run --example decode -- examples/patch-robot36-pysstv.wav.gz local/decoded.png
//! ```
//!
//! Rows that could not be decoded are left black, so a misaligned or truncated
//! decode is still visible rather than fatal.

use std::env;
use std::fs::File;
use std::io::{Cursor, Read};

use sstv::{Event, Mode, RgbPixel, RowDecoder};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;

fn main() {
    let mut args = env::args().skip(1);
    let input = args
        .next()
        .expect("usage: decode <input.wav|input.wav.gz> <output image>");
    let output = args
        .next()
        .expect("usage: decode <input.wav|input.wav.gz> <output image>");

    let (samples, sample_rate) = read_samples(&input);
    println!("read {} samples at {sample_rate} Hz", samples.len());

    // Reconstruct the first image in the stream from the decoder's events.
    let mut decoded: Vec<RgbPixel> = Vec::new();
    for event in RowDecoder::new(Mode::Robot36, samples.into_iter(), sample_rate) {
        match event {
            Event::ImageStart(_) if !decoded.is_empty() => break,
            Event::ImageStart(_) => {}
            Event::Row(row) => decoded.extend_from_slice(row.pixels()),
            Event::ImageEnd { .. } => break,
        }
    }
    println!(
        "decoded {} pixels ({} of {HEIGHT} rows)",
        decoded.len(),
        decoded.len() / WIDTH as usize
    );

    save_image(&decoded, &output);
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

fn save_image(pixels: &[RgbPixel], path: &str) {
    let mut image = image::RgbImage::new(WIDTH, HEIGHT);
    for (index, pixel) in pixels.iter().take((WIDTH * HEIGHT) as usize).enumerate() {
        let x = index as u32 % WIDTH;
        let y = index as u32 / WIDTH;
        image.put_pixel(x, y, image::Rgb([pixel.red(), pixel.green(), pixel.blue()]));
    }
    image.save(path).expect("save output image");
}
