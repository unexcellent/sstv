//! End-to-end tests: encode an image to tones with the [`Encoder`], optionally
//! corrupt the audio with deterministic noise, and decode it back with
//! [`RowDecoder`], reassembling the event stream into images.

use rand::SeedableRng;
use rand_distr::{Distribution, Normal};
use sstv::{Encoder, Event, ImageInfo, Mode, RgbPixel, RgbRow, RowDecoder, Synthesizer};

/// Robot36 resolution.
const WIDTH: usize = 320;
const HEIGHT: usize = 240;
const SAMPLE_RATE: u32 = 48_000;

/// Signal-to-noise ratio used wherever noise is added.
const SNR_DB: f64 = 30.0;
/// Length of the "pure noise" padding regions.
const NOISE_PADDING: usize = 3 * SAMPLE_RATE as usize;

/// Acceptable mean absolute per-channel error for a clean decode.
const CLEAN_ERROR: f64 = 12.0;
/// Acceptable mean absolute per-channel error for a noisy decode.
const NOISY_ERROR: f64 = 20.0;

/// A 320x240 test image with variation in all three channels.
fn test_image() -> Vec<RgbPixel> {
    let mut pixels = Vec::with_capacity(WIDTH * HEIGHT);
    for y in 0..HEIGHT as u32 {
        for x in 0..WIDTH as u32 {
            let red = (x * 255 / (WIDTH as u32 - 1)) as u8;
            let green = (y * 255 / (HEIGHT as u32 - 1)) as u8;
            let blue = ((x + y) * 255 / (WIDTH as u32 - 1 + HEIGHT as u32 - 1)) as u8;
            pixels.push(RgbPixel::new(red, green, blue));
        }
    }
    pixels
}

/// Encode an image into a full Robot36 transmission (header + image tones).
fn encode(image: &[RgbPixel]) -> Vec<i16> {
    // `to_vec` is required: `Encoder::new` needs an owned (`'static`) iterator,
    // so borrowing with `iter().copied()` would not compile.
    #[allow(clippy::unnecessary_to_owned)]
    let encoder = Encoder::new(Mode::Robot36, image.to_vec().into_iter()).unwrap();
    Synthesizer::new(encoder, SAMPLE_RATE).collect()
}

/// The noise standard deviation that yields the given SNR against a full-scale
/// sinusoidal signal.
fn sigma_for_snr(snr_db: f64) -> f64 {
    let signal_rms = i16::MAX as f64 / std::f64::consts::SQRT_2;
    signal_rms / 10f64.powf(snr_db / 20.0)
}

/// A deterministic block of Gaussian noise samples.
fn noise(len: usize, seed: u64) -> Vec<i16> {
    let sigma = sigma_for_snr(SNR_DB);
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let normal = Normal::new(0.0, sigma).unwrap();
    (0..len)
        .map(|_| {
            normal
                .sample(&mut rng)
                .round()
                .clamp(i16::MIN as f64, i16::MAX as f64) as i16
        })
        .collect()
}

/// Mix deterministic Gaussian noise into a signal at [`SNR_DB`].
fn add_noise(samples: &[i16], seed: u64) -> Vec<i16> {
    samples
        .iter()
        .zip(noise(samples.len(), seed))
        .map(|(&sample, offset)| sample.saturating_add(offset))
        .collect()
}

/// Mean absolute per-channel error between two images of equal length.
fn mean_abs_error(a: &[RgbPixel], b: &[RgbPixel]) -> f64 {
    assert_eq!(a.len(), b.len());
    let total: u64 = a
        .iter()
        .zip(b)
        .map(|(p, q)| {
            let d = |x: u8, y: u8| (x as i32 - y as i32).unsigned_abs() as u64;
            d(p.red(), q.red()) + d(p.green(), q.green()) + d(p.blue(), q.blue())
        })
        .sum();
    total as f64 / (a.len() as f64 * 3.0)
}

/// One image reassembled from the decoder's event stream.
struct DecodedImage {
    #[allow(dead_code)]
    info: ImageInfo,
    rows: Vec<RgbRow>,
    complete: bool,
}

impl DecodedImage {
    /// Flatten the rows into a single raster-order pixel buffer.
    fn pixels(&self) -> Vec<RgbPixel> {
        let mut pixels = Vec::with_capacity(self.rows.len() * WIDTH);
        for row in &self.rows {
            pixels.extend_from_slice(row.pixels());
        }
        pixels
    }
}

/// Drive the decoder to completion, grouping its events into images.
fn decode_images(samples: Vec<i16>) -> Vec<DecodedImage> {
    let mut images = Vec::new();
    let mut current: Option<(ImageInfo, Vec<RgbRow>)> = None;

    for event in RowDecoder::new(samples.into_iter(), SAMPLE_RATE) {
        match event {
            Event::ImageStart(info) => current = Some((info, Vec::new())),
            Event::Row(row) => {
                let (_, rows) = current
                    .as_mut()
                    .expect("Row without a preceding ImageStart");
                rows.push(row);
            }
            Event::ImageEnd { complete } => {
                let (info, rows) = current.take().expect("ImageEnd without an ImageStart");
                images.push(DecodedImage {
                    info,
                    rows,
                    complete,
                });
            }
        }
    }

    images
}

/// Assert a decoded image is complete and close enough to the original.
fn assert_matches(decoded: &DecodedImage, original: &[RgbPixel], max_error: f64) {
    assert!(decoded.complete, "image should decode completely");
    assert_eq!(decoded.rows.len(), HEIGHT, "should decode every row");
    let error = mean_abs_error(original, &decoded.pixels());
    assert!(error < max_error, "mean abs error {error} too high");
}

/// A clean transmission with only the image tones decodes to one image.
#[test]
fn image_tones_only() {
    let image = test_image();
    let samples = encode(&image);

    let decoded = decode_images(samples);

    assert_eq!(decoded.len(), 1, "expected exactly one image");
    assert_matches(&decoded[0], &image, CLEAN_ERROR);
}

/// The image tones mixed with 30 dB SNR noise still decode to one image.
#[test]
fn image_tones_with_noise() {
    let image = test_image();
    let samples = add_noise(&encode(&image), 0x1);

    let decoded = decode_images(samples);

    assert_eq!(decoded.len(), 1, "expected exactly one image");
    assert_matches(&decoded[0], &image, NOISY_ERROR);
}

/// Noise trailing the transmission must not spawn a spurious second image.
#[test]
fn image_tones_with_noise_then_pure_noise() {
    let image = test_image();
    let mut samples = add_noise(&encode(&image), 0x1);
    samples.extend(noise(NOISE_PADDING, 0x2));

    let decoded = decode_images(samples);

    assert_eq!(decoded.len(), 1, "trailing noise should not add an image");
    assert_matches(&decoded[0], &image, NOISY_ERROR);
}

/// Noise leading the transmission must be skipped, then the image decoded.
#[test]
fn image_tones_with_noise_prefixed_by_pure_noise() {
    let image = test_image();
    let mut samples = noise(NOISE_PADDING, 0x2);
    samples.extend(add_noise(&encode(&image), 0x1));

    let decoded = decode_images(samples);

    assert_eq!(decoded.len(), 1, "leading noise should be skipped");
    assert_matches(&decoded[0], &image, NOISY_ERROR);
}

/// Real-world reception: a loud startup transient (a receiver click) would
/// latch naive all-time min/max extremes, the transmission carries a DC bias
/// that keeps it entirely on one side of that stale midline, and it fades in
/// rather than starting at full amplitude. The adaptive envelope must recover
/// and still decode the image.
#[test]
fn dc_biased_faded_in_transmission() {
    let image = test_image();

    // Attenuate for DC headroom and bias the whole transmission positive so it
    // never crosses zero, then fade it in over the first 200 ms.
    const DC_BIAS: i16 = 11_000;
    let fade_in = SAMPLE_RATE as usize / 5;
    let biased: Vec<i16> = encode(&image)
        .iter()
        .enumerate()
        .map(|(i, &sample)| {
            let gain = (i as f64 / fade_in as f64).min(1.0);
            ((sample / 3) as f64 * gain) as i16 + DC_BIAS
        })
        .collect();

    // Prepend a brief full-scale transient that latches a global min/max at
    // ±full-scale (midline 0) — which the biased signal never crosses.
    let mut samples = vec![i16::MAX, i16::MIN, i16::MAX, i16::MIN];
    samples.extend(biased);

    let decoded = decode_images(samples);

    assert_eq!(
        decoded.len(),
        1,
        "biased, faded-in transmission should decode"
    );
    assert_matches(&decoded[0], &image, NOISY_ERROR);
}

/// Two noisy images separated by a stretch of pure noise decode to two images.
#[test]
fn two_images_with_noise_and_noise_gap() {
    let image = test_image();
    let mut samples = add_noise(&encode(&image), 0x1);
    samples.extend(noise(NOISE_PADDING, 0x2));
    samples.extend(add_noise(&encode(&image), 0x3));

    let decoded = decode_images(samples);

    assert_eq!(decoded.len(), 2, "expected two images across the noise gap");
    for decoded_image in &decoded {
        assert_matches(decoded_image, &image, NOISY_ERROR);
    }
}
