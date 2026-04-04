use hound::{SampleFormat, WavSpec, WavWriter};
use image::GenericImageView;
use sstv::encode::Encoder; // Change 'sstv' to your exact crate name if different

fn main() {
    let img = image::open("examples/patch.png").expect("Failed to load image");
    let pixels: std::vec::Vec<(u8, u8, u8)> = img
        .pixels()
        .map(|(_, _, rgba)| (rgba[0], rgba[1], rgba[2]))
        .collect();

    let sample_rate = 44100;
    let encoder = Encoder::new(pixels.into_iter(), sample_rate);

    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut writer = WavWriter::create("output.wav", spec).expect("Failed to create wav file");

    for sample in encoder {
        // NCO generates f32 between -1.0 and 1.0.
        // PCM requires 16-bit integers, so we scale by i16::MAX.
        let amplitude = (sample * std::i16::MAX as f32) as i16;
        writer
            .write_sample(amplitude)
            .expect("Failed to write sample");
    }

    writer.finalize().expect("Failed to finalize wav file");
}
