//! DSP Constants for SSTV encoding
//!
//! All values are derived from the QSSTV project source code to ensure
//! 100% signal compatibility with existing SSTV decoders.
//!
//! Primary sources:
//! - qsstv/src/sstv/sstvparam.cpp (mode timing tables)
//! - qsstv/src/sstv/sstvtx.cpp (VIS code transmission)
//! - qsstv/src/sstv/modes/modebase.cpp (frequency constants)
//! - qsstv/src/dsp/synthes.cpp (FM synthesis)

// =============================================================================
// FREQUENCY CONSTANTS
// =============================================================================

/// Sync pulse frequency in Hz
/// Derived from qsstv/src/sstv/modes/modebase.cpp:60 - syncFreq=1200
pub const FREQ_SYNC: f64 = 1200.0;

/// Black level / blanking frequency in Hz
/// Derived from qsstv/src/sstv/modes/modebase.cpp:59 - lowerFreq=1500
pub const FREQ_BLACK: f64 = 1500.0;

/// White level frequency in Hz
/// Derived from qsstv/src/sstv/modes/modebase.cpp:503 - f=lowerFreq+((double)pixelArrayPtr[pixelCounter]*(2300-lowerFreq)/255.)
pub const FREQ_WHITE: f64 = 2300.0;

/// VIS code bit 1 (logic high) frequency in Hz
/// Derived from qsstv/src/sstv/sstvtx.cpp:75 - synthesPtr->sendTone(0.030,1100,0,true)
pub const FREQ_VIS_BIT1: f64 = 1100.0;

/// VIS code bit 0 (logic low) frequency in Hz
/// Derived from qsstv/src/sstv/sstvtx.cpp:76 - synthesPtr->sendTone(0.030,1300,0,true)
pub const FREQ_VIS_BIT0: f64 = 1300.0;

/// VIS header/separator frequency in Hz
/// Derived from qsstv/src/sstv/sstvtx.cpp:58 - synthesPtr->sendTone(0.300,1900,0,true)
pub const FREQ_VIS_HEADER: f64 = 1900.0;

/// VIS break frequency in Hz
/// Derived from qsstv/src/sstv/sstvtx.cpp:72 - synthesPtr->sendTone(0.030,1200,0,true)
pub const FREQ_VIS_BREAK: f64 = 1200.0;

/// Robot mode separator frequency in Hz
/// Derived from qsstv/src/sstv/modes/moderobot1.cpp:218 - txFreq=1900.
pub const FREQ_SEPARATOR: f64 = 1900.0;

/// Robot mode even line marker frequency in Hz
/// Derived from qsstv/src/sstv/modes/moderobot1.cpp:243 - txFreq=2300.
pub const FREQ_EVEN_MARKER: f64 = 2300.0;

// =============================================================================
// VIS CODE TIMING CONSTANTS
// =============================================================================

/// VIS header tone duration in seconds (300ms)
/// Derived from qsstv/src/sstv/sstvtx.cpp:58 - synthesPtr->sendTone(0.300,1900,0,true)
pub const VIS_HEADER_DURATION: f64 = 0.300;

/// VIS break tone duration in seconds (10ms)
/// Derived from qsstv/src/sstv/sstvtx.cpp:59 - synthesPtr->sendTone(0.100,2100,0,true) ... then 0.022 for narrow
/// For wide modes: 30ms startbit at 1200 Hz
pub const VIS_BREAK_DURATION: f64 = 0.010;

/// VIS bit duration in seconds (30ms)
/// Derived from qsstv/src/sstv/sstvtx.cpp:75-76 - synthesPtr->sendTone(0.030,...)
pub const VIS_BIT_DURATION: f64 = 0.030;

// =============================================================================
// PREAMBLE TIMING CONSTANTS
// =============================================================================

/// Preamble tone duration in seconds
/// Derived from qsstv/src/sstv/sstvtx.cpp:35-45 - sendPreamble() function
pub const PREAMBLE_TONE_SHORT: f64 = 0.100;
pub const PREAMBLE_TONE_LONG: f64 = 0.300;
pub const PREAMBLE_SYNC_PULSE: f64 = 0.010;

// =============================================================================
// FM SYNTHESIS CONSTANTS
// =============================================================================

/// Sine table length for FM synthesis
/// Derived from qsstv/src/dsp/synthes.h:29 - #define SINTABLEN 2048
pub const SINE_TABLE_LEN: usize = 2048;

/// Audio amplitude for 16-bit signed output
/// Derived from qsstv/src/dsp/synthes.cpp:46 - (sin(...)*8000.)
pub const AUDIO_AMPLITUDE: f64 = 8000.0;

// =============================================================================
// FREQUENCY CALCULATION HELPERS
// =============================================================================

/// Convert a pixel value (0-255) to frequency
/// Derived from qsstv/src/sstv/modes/modebase.cpp:503
/// f = lowerFreq + ((double)pixel * (2300 - lowerFreq) / 255.)
#[inline]
pub fn pixel_to_freq(pixel: u8) -> f64 {
    FREQ_BLACK + (pixel as f64 * (FREQ_WHITE - FREQ_BLACK) / 255.0)
}

/// Frequency deviation (half the bandwidth)
/// Derived from qsstv/src/sstv/sstvparam.cpp:309 - deviation=400
pub const FREQ_DEVIATION: f64 = 400.0;

/// Subcarrier center frequency
/// Derived from qsstv/src/sstv/sstvparam.cpp:309 - subcarrier=1900
pub const FREQ_SUBCARRIER: f64 = 1900.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_to_freq_black() {
        let freq = pixel_to_freq(0);
        assert!((freq - FREQ_BLACK).abs() < 0.001);
    }

    #[test]
    fn test_pixel_to_freq_white() {
        let freq = pixel_to_freq(255);
        assert!((freq - FREQ_WHITE).abs() < 0.001);
    }

    #[test]
    fn test_pixel_to_freq_gray() {
        let freq = pixel_to_freq(128);
        // Should be approximately halfway between black and white
        let expected = (FREQ_BLACK + FREQ_WHITE) / 2.0;
        assert!((freq - expected).abs() < 5.0); // Within 5 Hz
    }
}
