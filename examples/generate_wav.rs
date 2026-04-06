//! Example demonstrating how to generate a Robot 36 SSTV WAV file from an image.

use hound::{SampleFormat, WavSpec, WavWriter};
use sstv::encode::{ImageData, encode_robot36, samples_to_i16};
use std::error::Error;

pub fn generate_wav() -> Result<(), Box<dyn Error>> {
    let img = image::open("examples/patch.png")?.to_rgb8();
    let (width, height) = img.dimensions();

    let image_data = ImageData::from_rgb_bytes(width, height, &img.into_raw())?;

    let sample_rate = 48000;
    let generated_f64 = encode_robot36(&image_data, sample_rate)?;
    let generated_samples = samples_to_i16(&generated_f64);

    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut writer = WavWriter::create("local/output.wav", spec)?;
    for sample in generated_samples {
        writer.write_sample(sample)?;
    }
    writer.finalize()?;

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    generate_wav()?;
    println!("Successfully generated output.wav");
    Ok(())
}
