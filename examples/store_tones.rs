use flate2::Compression;
use flate2::write::GzEncoder;
use image::GenericImageView;
use sstv::image::RgbPixel;
use sstv::modes::robot36::Robot36Encoder;
use std::fs::File;
use std::io::{BufWriter, Write};

fn main() {
    let img = image::open("examples/patch.png").expect("Failed to open examples/patch.png");
    let (width, height) = img.dimensions();

    assert_eq!(width, 320, "Image width must be exactly 320");
    assert_eq!(height, 240, "Image height must be exactly 240");

    let encoder = Robot36Encoder::new(
        img.pixels()
            .map(|(_, _, rgba)| RgbPixel::new(rgba[0], rgba[1], rgba[2])),
    );

    let file = File::create("examples/patch-robot36-tones.csv.gz")
        .expect("Failed to create examples/patch-robot36-tones.csv.gz");

    // GzEncoder handles the compression stream transparency
    let encoder_writer = GzEncoder::new(file, Compression::default());
    let mut writer = BufWriter::new(encoder_writer);

    writer.write_all(b"hz,nanos\n").unwrap();

    encoder.for_each(|tone| {
        writeln!(writer, "{},{}", tone.0.hz(), tone.1.nanos()).unwrap();
    });
}
