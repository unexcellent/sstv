use hound::{SampleFormat, WavSpec, WavWriter};
use image::GenericImageView;
use sstv::image::RgbPixel;
use sstv::modes::robot36::Robot36Encoder;
use std::f64::consts::PI;
use std::fs;
use std::path::Path;

fn main() {
    let img = image::open("examples/patch.png").expect("Failed to open examples/patch.png");
    let (width, height) = img.dimensions();

    assert_eq!(width, 320, "Image width must be exactly 320");
    assert_eq!(height, 240, "Image height must be exactly 240");

    let encoder = Robot36Encoder::new(
        img.pixels()
            .map(|(_, _, rgba)| RgbPixel::new(rgba[0], rgba[1], rgba[2])),
    );

    fs::create_dir_all("local").expect("Failed to create local/ directory");
    let out_path = Path::new("local").join("output.wav");

    let sample_rate = 48000;
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut writer = WavWriter::create(out_path, spec).expect("Failed to create WAV writer");
    let mut phase: f64 = 0.0;
    let mut sample_adjust: f64 = 0.0;

    encoder.for_each(|tone| {
        let freq = tone.0.hz() as f64;
        let duration_sec = tone.1.micros() as f64 / 1_000_000.0;

        let exact_samples = (duration_sec * sample_rate as f64) + sample_adjust;
        let num_samples = exact_samples.round() as usize;

        sample_adjust = exact_samples - num_samples as f64;

        let phase_increment = 2.0 * PI * freq / sample_rate as f64;

        (0..num_samples).for_each(|_| {
            let sample = (phase.sin() * i16::MAX as f64) as i16;
            writer.write_sample(sample).unwrap();
            phase = (phase + phase_increment) % (2.0 * PI);
        });
    });

    writer.finalize().expect("Failed to finalize output.wav");
}
