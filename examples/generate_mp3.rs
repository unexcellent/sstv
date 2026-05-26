use image::GenericImageView;
use mp3lame_encoder::{Builder, FlushNoGap, MonoPcm};
use sstv::Encoder;
use sstv::Mode;
use sstv::RgbPixel;
use std::f64::consts::PI;
use std::fs;
use std::io::Write;
use std::path::Path;

fn main() {
    let img = image::open("examples/patch.png").expect("Failed to open examples/patch.png");
    let (width, height) = img.dimensions();

    assert_eq!(width, 320, "Image width must be exactly 320");
    assert_eq!(height, 240, "Image height must be exactly 240");

    let mut pixels = std::vec![[RgbPixel::new(0, 0, 0); 320]; 240];

    img.pixels().for_each(|(x, y, rgba)| {
        pixels[y as usize][x as usize] = RgbPixel::new(rgba[0], rgba[1], rgba[2]);
    });

    let encoder = Encoder::new(Mode::Robot36, pixels.into_iter().flatten()).unwrap();

    fs::create_dir_all("local").expect("Failed to create local/ directory");
    let out_path = Path::new("local").join("output.mp3");

    let sample_rate = 48000;
    let mut phase: f64 = 0.0;
    let mut sample_adjust: f64 = 0.0;
    let mut pcm_samples: Vec<i16> = Vec::new();

    encoder.for_each(|tone| {
        let freq = tone.frequency.hz() as f64;
        let duration_sec = tone.duration.ns() as f64 / 1_000_000_000.0;

        let exact_samples = (duration_sec * sample_rate as f64) + sample_adjust;
        let num_samples = exact_samples.round() as usize;

        sample_adjust = exact_samples - num_samples as f64;

        let phase_increment = 2.0 * PI * freq / sample_rate as f64;

        (0..num_samples).for_each(|_| {
            let sample = (phase.sin() * i16::MAX as f64) as i16;
            pcm_samples.push(sample);
            phase = (phase + phase_increment) % (2.0 * PI);
        });
    });

    let mut mp3_builder = Builder::new().expect("Create LAME builder");
    mp3_builder.set_num_channels(1).unwrap();
    mp3_builder.set_sample_rate(sample_rate).unwrap();
    mp3_builder
        .set_brate(mp3lame_encoder::Bitrate::Kbps192)
        .unwrap();
    mp3_builder
        .set_quality(mp3lame_encoder::Quality::Best)
        .unwrap();

    let mut mp3_encoder = mp3_builder.build().expect("To initialize LAME encoder");
    let mut mp3_out_buffer = Vec::new();
    mp3_out_buffer.reserve(mp3lame_encoder::max_required_buffer_size(pcm_samples.len()));

    mp3_encoder
        .encode_to_vec(MonoPcm(pcm_samples.as_slice()), &mut mp3_out_buffer)
        .expect("Failed to encode PCM data");

    mp3_encoder
        .flush_to_vec::<FlushNoGap>(&mut mp3_out_buffer)
        .expect("Failed to flush MP3 encoder");

    let mut file = fs::File::create(out_path).expect("Failed to create output.mp3");
    file.write_all(&mp3_out_buffer)
        .expect("Failed to write MP3 data");
}
