//! Helpers shared by the integration tests.

// Test helpers outside #[test] functions are not covered by the clippy.toml
// test allowances, and each test binary uses only a subset of the helpers.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, dead_code)]

use sstv::{Mode, RgbPixel};

/// A test image at the mode's resolution with variation in all three channels.
pub fn test_image(mode: Mode) -> Vec<RgbPixel> {
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
    pixels
}

/// Mean absolute per-channel error between two images of equal length.
pub fn mean_abs_error(a: &[RgbPixel], b: &[RgbPixel]) -> f64 {
    assert_eq!(a.len(), b.len());
    let total: u64 = a
        .iter()
        .zip(b)
        .map(|(p, q)| {
            let d = |x: u8, y: u8| u64::from((i32::from(x) - i32::from(y)).unsigned_abs());
            d(p.red(), q.red()) + d(p.green(), q.green()) + d(p.blue(), q.blue())
        })
        .sum();
    total as f64 / (a.len() as f64 * 3.0)
}

/// Mean absolute error between two raw channel-byte buffers of equal length.
pub fn mean_abs_error_bytes(a: &[u8], b: &[u8]) -> f64 {
    assert_eq!(a.len(), b.len());
    let sum: u64 = a
        .iter()
        .zip(b)
        .map(|(x, y)| u64::from((i32::from(*x) - i32::from(*y)).unsigned_abs()))
        .sum();
    sum as f64 / a.len() as f64
}
