//! VIS Code generation for SSTV modes
//!
//! The VIS (Vertical Interval Signaling) code identifies the SSTV mode
//! being transmitted. This module generates the VIS header and code
//! according to the QSSTV implementation.
//!
//! Reference: qsstv/src/sstv/sstvtx.cpp:50-81

use crate::constants::{FREQ_VIS_BIT0, FREQ_VIS_BIT1, FREQ_VIS_BREAK, VIS_BIT_DURATION};
use crate::synthesizer::{Synthesizer, Tone};

/// Generate the SSTV preamble tones
///
/// The preamble consists of alternating tones to help receivers sync.
///
/// Derived from qsstv/src/sstv/sstvtx.cpp:31-46 (sendPreamble function)
/// ```cpp
/// synthesPtr->sendTone(0.1,1900.,0,true);
/// synthesPtr->sendTone(0.1,1500.,0,true);
/// synthesPtr->sendTone(0.1,1900.,0,true);
/// synthesPtr->sendTone(0.1,1500.,0,true);
/// synthesPtr->sendTone(0.1,2300.,0,true);
/// synthesPtr->sendTone(0.1,1500.,0,true);
/// synthesPtr->sendTone(0.1,2300.,0,true);
/// synthesPtr->sendTone(0.1,1500.,0,true);
/// synthesPtr->sendTone(0.3,1900.,0,true);
/// synthesPtr->sendTone(0.01,1200.,0,true);
/// synthesPtr->sendTone(0.3,1900.,0,true);
/// ```
pub fn generate_preamble() -> Vec<Tone> {
    vec![
        Tone::new(1900.0, 0.1),
        Tone::new(1500.0, 0.1),
        Tone::new(1900.0, 0.1),
        Tone::new(1500.0, 0.1),
        Tone::new(2300.0, 0.1),
        Tone::new(1500.0, 0.1),
        Tone::new(2300.0, 0.1),
        Tone::new(1500.0, 0.1),
        Tone::new(1900.0, 0.3),
        Tone::new(1200.0, 0.01),
        Tone::new(1900.0, 0.3),
    ]
}

/// Generate VIS code tones for a standard 8-bit VIS code
///
/// VIS Code structure (from qsstv/src/sstv/sstvtx.cpp:68-80):
/// 1. Start bit: 30ms @ 1200 Hz
/// 2. 8 data bits: 30ms each (1100 Hz = 1, 1300 Hz = 0), LSB first
/// 3. Stop bit: 30ms @ 1200 Hz
///
/// # Arguments
/// * `vis_code` - The 8-bit VIS code for the SSTV mode
pub fn generate_vis_code(vis_code: u8) -> Vec<Tone> {
    let mut tones = Vec::with_capacity(10);

    // Start bit - 30ms @ 1200 Hz
    // Derived from qsstv/src/sstv/sstvtx.cpp:72
    tones.push(Tone::new(FREQ_VIS_BREAK, VIS_BIT_DURATION));

    // 8 data bits, LSB first
    // Derived from qsstv/src/sstv/sstvtx.cpp:73-77
    let mut code = vis_code;
    for _ in 0..8 {
        let freq = if (code & 1) == 1 {
            FREQ_VIS_BIT1 // 1100 Hz for bit 1
        } else {
            FREQ_VIS_BIT0 // 1300 Hz for bit 0
        };
        tones.push(Tone::new(freq, VIS_BIT_DURATION));
        code >>= 1;
    }

    // Stop bit - 30ms @ 1200 Hz
    // Derived from qsstv/src/sstv/sstvtx.cpp:79
    tones.push(Tone::new(FREQ_VIS_BREAK, VIS_BIT_DURATION));

    tones
}

/// Generate VIS code tones for extended 16-bit VIS codes (like Martin MP modes)
///
/// Extended VIS codes have 16 bits instead of 8.
///
/// # Arguments
/// * `vis_code` - The 16-bit VIS code
pub fn generate_vis_code_extended(vis_code: u16) -> Vec<Tone> {
    let mut tones = Vec::with_capacity(18);

    // Start bit
    tones.push(Tone::new(FREQ_VIS_BREAK, VIS_BIT_DURATION));

    // 16 data bits, LSB first
    let mut code = vis_code;
    for _ in 0..16 {
        let freq = if (code & 1) == 1 {
            FREQ_VIS_BIT1
        } else {
            FREQ_VIS_BIT0
        };
        tones.push(Tone::new(freq, VIS_BIT_DURATION));
        code >>= 1;
    }

    // Stop bit
    tones.push(Tone::new(FREQ_VIS_BREAK, VIS_BIT_DURATION));

    tones
}

/// Generate complete VIS header including preamble and VIS code
///
/// # Arguments
/// * `vis_code` - The VIS code for the SSTV mode
pub fn generate_complete_vis(vis_code: u8) -> Vec<Tone> {
    let mut tones = generate_preamble();
    tones.extend(generate_vis_code(vis_code));
    tones
}

/// Convert a list of tones to audio samples
///
/// # Arguments
/// * `synth` - The synthesizer to use
/// * `tones` - List of tones to generate
pub fn tones_to_samples(synth: &mut Synthesizer, tones: &[Tone]) -> Vec<f64> {
    let mut samples = Vec::new();
    let mut first = true;

    for tone in tones {
        // Use concat=true for all tones after the first to maintain phase continuity
        let tone_samples = synth.generate_tone(tone.duration, tone.freq, !first);
        samples.extend(tone_samples);
        first = false;
    }

    samples
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preamble_length() {
        let preamble = generate_preamble();
        assert_eq!(preamble.len(), 11);
    }

    #[test]
    fn test_vis_code_length() {
        let vis = generate_vis_code(0xAC); // Martin 1
        // Should have: 1 start + 8 data + 1 stop = 10 tones
        assert_eq!(vis.len(), 10);
    }

    #[test]
    fn test_vis_code_bits() {
        // Test with known VIS code 0xAC (Martin 1) = 10101100 binary
        let vis = generate_vis_code(0xAC);

        // Start bit at 1200 Hz
        assert_eq!(vis[0].freq, FREQ_VIS_BREAK);

        // Bit 0 = 0 -> 1300 Hz
        assert_eq!(vis[1].freq, FREQ_VIS_BIT0);

        // Bit 1 = 0 -> 1300 Hz
        assert_eq!(vis[2].freq, FREQ_VIS_BIT0);

        // Bit 2 = 1 -> 1100 Hz
        assert_eq!(vis[3].freq, FREQ_VIS_BIT1);

        // Bit 3 = 1 -> 1100 Hz
        assert_eq!(vis[4].freq, FREQ_VIS_BIT1);

        // Stop bit at 1200 Hz
        assert_eq!(vis[9].freq, FREQ_VIS_BREAK);
    }
}
