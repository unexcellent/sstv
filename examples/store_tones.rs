use flate2::Compression;
use flate2::write::GzEncoder;
use image::GenericImageView;
use sstv::Encoder;
use sstv::Mode;
use sstv::RgbPixel;
use std::fs::File;
use std::io::{BufWriter, Write};

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

    let file = File::create("examples/patch-robot36-tones.csv.gz")
        .expect("Failed to create examples/patch-robot36-tones.csv.gz");

    // GzEncoder handles the compression stream transparency
    let encoder_writer = GzEncoder::new(file, Compression::default());
    let mut writer = BufWriter::new(encoder_writer);

    writer.write_all(b"hz,nanos\n").unwrap();

    encoder.for_each(|tone| {
        writeln!(writer, "{},{}", tone.frequency.hz(), tone.duration.ns()).unwrap();
    });
}
