/// Timing and frequency constants for a specific transmission mode.
pub trait Mode {
    const VIS_CODE: u8;
    const SYNC_FREQ_HZ: f32 = 1200.0;
    const BLACK_FREQ_HZ: f32 = 1500.0;
    const WHITE_FREQ_HZ: f32 = 2300.0;
}

/// Robot 36 color mode.
/// 320x240 resolution. Transmits Y (luminance) for every line,
/// and alternates R-Y (V) and B-Y (U) for alternating lines.
pub struct Robot36;

impl Mode for Robot36 {
    const VIS_CODE: u8 = 8;
}
