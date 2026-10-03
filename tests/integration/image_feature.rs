//! Tests for the `image` feature: converting decoded images into `image`
//! crate buffers.

use crate::common::{save_decoded, save_decoded_buffer};
use sstv::{Decoder, Encoder, Mode, RgbPixel, Synthesizer, modes};

const SAMPLE_RATE: u32 = 24_000;

/// A full transmission of a test image with variation in all three channels.
fn transmission(mode: Mode) -> Vec<i16> {
    let (width, height) = mode.resolution();
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for y in 0..height {
        for x in 0..width {
            let red = (x * 255 / (width - 1)) as u8;
            let green = (y * 255 / (height - 1)) as u8;
            let blue = ((x + y) * 255 / (width + height - 2)) as u8;
            pixels.push(RgbPixel::new(red, green, blue));
        }
    }
    let encoder = Encoder::new(mode, pixels.into_iter()).expect("construct encoder");
    Synthesizer::new(encoder, SAMPLE_RATE).collect()
}

#[test]
fn encodes_image_buffers_resizing_them_to_the_mode_resolution() {
    let small = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(100, 80, |x, y| {
        image::Rgb([(x * 2) as u8, (y * 3) as u8, 128])
    }));
    let encoder = Encoder::from_image(modes::ROBOT_36, &small).expect("encode image");

    let decoded = Decoder::new(Synthesizer::new(encoder, SAMPLE_RATE), SAMPLE_RATE)
        .with_mode(modes::ROBOT_36)
        .decode()
        .expect("an image");
    save_decoded("resized", &decoded);
    assert!(decoded.complete(), "image should decode completely");
    let (width, height) = modes::ROBOT_36.resolution();
    assert_eq!(decoded.width() as u32, width);
    assert_eq!(decoded.height() as u32, height);
}

#[test]
fn decoded_images_convert_to_image_buffers() {
    let samples = transmission(modes::ROBOT_36);

    let decoded = Decoder::new(samples.into_iter(), SAMPLE_RATE)
        .with_mode(modes::ROBOT_36)
        .decode()
        .expect("an image");
    save_decoded("converted", &decoded);
    let buffer = image::RgbImage::from(&decoded);

    assert_eq!(buffer.width() as usize, decoded.width());
    assert_eq!(buffer.height() as usize, decoded.height());
    let pixel = decoded.pixels()[decoded.width() + 1];
    assert_eq!(
        buffer.get_pixel(1, 1),
        &image::Rgb([pixel.red(), pixel.green(), pixel.blue()])
    );
}

#[test]
fn decodes_to_image_buffers_and_saves_them() {
    let samples = transmission(modes::ROBOT_36);

    let decoded = Decoder::new(samples.into_iter(), SAMPLE_RATE)
        .with_mode(modes::ROBOT_36)
        .rgb_image()
        .expect("an image");
    save_decoded_buffer("rgb-image", &decoded);
    let (width, height) = modes::ROBOT_36.resolution();
    assert_eq!(decoded.width(), width);
    assert_eq!(decoded.height(), height);

    let path = std::env::temp_dir().join("sstv-image-feature-test.png");
    decoded.save(&path).expect("save decoded image");
    let reloaded = image::open(&path).expect("reload decoded image").to_rgb8();
    assert_eq!(reloaded, decoded);
    std::fs::remove_file(&path).expect("remove test output");
}
