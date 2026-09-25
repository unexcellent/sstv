//! The SSTV modes specified in the Dayton paper — JL Barber (N7CXI),
//! "Proposal for SSTV Mode Specifications", presented at the Dayton SSTV
//! forum, 20 May 2000.
//!
//! Each mode lives in its own module, transcribing its timing table from the
//! paper into a `Layout`. Everything shared between modes — the frequency
//! range, the calibration header and the VIS code — is defined here, as in
//! the paper's common sections.

pub(crate) mod layout;

mod martin_1;
mod martin_2;
mod pasokon_p3;
mod pasokon_p5;
mod pasokon_p7;
mod pd_120;
mod pd_160;
mod pd_180;
mod pd_240;
mod pd_290;
mod pd_50;
mod pd_90;
mod robot_36;
mod robot_72;
mod scottie_1;
mod scottie_2;
mod scottie_dx;
mod wrasse_sc2_180;

use crate::Error;
use crate::synthesizer::Tone;
use crate::units::{Duration, Frequency};
use crate::{Hz, ms, tone};
use layout::Layout;

pub use martin_1::MARTIN_1;
pub use martin_2::MARTIN_2;
pub use pasokon_p3::PASOKON_P3;
pub use pasokon_p5::PASOKON_P5;
pub use pasokon_p7::PASOKON_P7;
pub use pd_50::PD_50;
pub use pd_90::PD_90;
pub use pd_120::PD_120;
pub use pd_160::PD_160;
pub use pd_180::PD_180;
pub use pd_240::PD_240;
pub use pd_290::PD_290;
pub use robot_36::ROBOT_36;
pub use robot_72::ROBOT_72;
pub use scottie_1::SCOTTIE_1;
pub use scottie_2::SCOTTIE_2;
pub use scottie_dx::SCOTTIE_DX;
pub use wrasse_sc2_180::WRASSE_SC2_180;

/// Every transmission mode, in the paper's order.
pub const ALL: [Mode; 18] = [
    SCOTTIE_1,
    SCOTTIE_2,
    SCOTTIE_DX,
    MARTIN_1,
    MARTIN_2,
    ROBOT_36,
    ROBOT_72,
    WRASSE_SC2_180,
    PASOKON_P3,
    PASOKON_P5,
    PASOKON_P7,
    PD_50,
    PD_90,
    PD_120,
    PD_160,
    PD_180,
    PD_240,
    PD_290,
];

/// The sync pulse frequency, shared by every mode.
pub(crate) const SYNC_FREQUENCY: Frequency = Hz!(1200);
/// Pure black — the lower end of the luminance range.
pub(crate) const BLACK_FREQUENCY: Frequency = Hz!(1500);
/// Pure white — the upper end of the luminance range.
pub(crate) const WHITE_FREQUENCY: Frequency = Hz!(2300);
/// The leader tone of the calibration header.
pub(crate) const LEADER_FREQUENCY: Frequency = Hz!(1900);
const VIS_ONE_FREQUENCY: Frequency = Hz!(1100);
const VIS_ZERO_FREQUENCY: Frequency = Hz!(1300);
/// Every VIS bit (start, data, parity, stop) lasts 30ms.
const VIS_BIT_DURATION: Duration = ms!(30);

/// The frequency representing a pixel value, mapped linearly onto the
/// luminance range.
pub(crate) fn value_frequency(value: u8) -> Frequency {
    BLACK_FREQUENCY + (WHITE_FREQUENCY - BLACK_FREQUENCY) * u32::from(value) / 255
}

/// A 7-bit VIS (Vertical Interval Signaling) code, transmitted in the
/// calibration header to identify the mode to a receiving system.
///
/// Construct one with [`VisCode::try_new`], or with
/// [`vis_code!`](crate::vis_code) to validate at compile time.
///
/// ```rust
/// use sstv::{Mode, modes, vis_code};
///
/// assert_eq!(
///     Mode::try_from(vis_code!(8)),
///     Ok(modes::ROBOT_36),
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VisCode(u8);

impl VisCode {
    /// Whether the value fits in the 7 bits of a VIS code.
    ///
    /// ```rust
    /// use sstv::VisCode;
    ///
    /// assert!(VisCode::is_valid(8));
    /// assert!(!VisCode::is_valid(200));
    /// ```
    #[must_use]
    pub const fn is_valid(code: u8) -> bool {
        code < 128
    }

    /// Construct a `VisCode` from its value.
    ///
    /// ```rust
    /// use sstv::{Error, VisCode, vis_code};
    ///
    /// assert_eq!(
    ///     VisCode::try_new(8),
    ///     Ok(vis_code!(8)),
    /// );
    /// assert_eq!(
    ///     VisCode::try_new(200),
    ///     Err(Error::BadVisCode),
    /// );
    /// ```
    ///
    /// # Errors
    ///
    /// [`Error::BadVisCode`] if the value does not fit in 7 bits.
    pub const fn try_new(code: u8) -> Result<Self, Error> {
        if Self::is_valid(code) {
            Ok(Self(code))
        } else {
            Err(Error::BadVisCode)
        }
    }

    /// `vis_code!` machinery; use [`VisCode::try_new`] or
    /// [`vis_code!`](crate::vis_code) instead. Masks the value to 7 bits —
    /// the macro asserts validity beforehand, so the mask never alters it.
    #[doc(hidden)]
    #[must_use]
    pub const fn new_masked(code: u8) -> Self {
        Self(code & 0x7F)
    }
}

/// The code's value.
///
/// ```rust
/// use sstv::vis_code;
///
/// assert_eq!(u8::from(vis_code!(8)), 8);
/// ```
impl From<VisCode> for u8 {
    fn from(code: VisCode) -> Self {
        code.0
    }
}

#[macro_export]
/// Construct a [`VisCode`](crate::VisCode), validated at compile time.
///
/// ```rust
/// use sstv::vis_code;
///
/// let code = vis_code!(8);
/// ```
///
/// A value that does not fit in 7 bits fails to compile:
///
/// ```compile_fail
/// use sstv::vis_code;
///
/// let code = vis_code!(200);
/// ```
macro_rules! vis_code {
    ($code:expr) => {
        const {
            assert!($crate::VisCode::is_valid($code), "VIS codes are 7 bit");
            $crate::VisCode::new_masked($code)
        }
    };
}

/// Tuning (VOX) tones customarily sent ahead of the calibration header to
/// open receiver squelch. They are not part of the paper's specification.
const VOX_TONES: [Tone; 8] = [
    tone!(1900 Hz, 100 ms),
    tone!(1500 Hz, 100 ms),
    tone!(1900 Hz, 100 ms),
    tone!(1500 Hz, 100 ms),
    tone!(2300 Hz, 100 ms),
    tone!(1500 Hz, 100 ms),
    tone!(2300 Hz, 100 ms),
    tone!(1500 Hz, 100 ms),
];

/// A specific protocol for encoding an image as a tone sequence.
///
/// The modes themselves are constants in [`modes`](crate::modes)
/// ([`modes::ROBOT_36`](crate::modes::ROBOT_36),
/// [`modes::SCOTTIE_1`](crate::modes::SCOTTIE_1), …), each defined in its own
/// module.
#[derive(Clone, Copy)]
pub struct Mode {
    /// The mode's name, as [`Debug`](core::fmt::Debug) prints it.
    name: &'static str,
    /// The VIS code identifying the mode to a receiving system.
    vis_code: VisCode,
    /// Whether one extra sync pulse precedes the first line (Scottie modes).
    starting_sync_pulse: bool,
    /// The scanline structure specified by the mode's timing-sequence table.
    layout: Layout,
}

impl Mode {
    /// The mode's VIS code, identifying it to a receiving system.
    #[must_use]
    pub const fn vis_code(&self) -> VisCode {
        self.vis_code
    }

    /// The mode's scanline structure as specified by its timing-sequence
    /// table in the paper.
    pub(crate) const fn layout(self) -> Layout {
        self.layout
    }

    /// The horizontal resolution in pixels.
    #[must_use]
    pub const fn image_width(&self) -> u32 {
        self.layout.width as u32
    }

    /// The vertical resolution in pixels.
    #[must_use]
    pub const fn image_height(&self) -> u32 {
        self.layout.height as u32
    }

    /// Whether the mode transmits one extra sync pulse between the header and
    /// the first line. Only Scottie modes do.
    pub(crate) const fn has_starting_sync_pulse(self) -> bool {
        self.starting_sync_pulse
    }

    /// The tones sent before the image: the VOX tuning tones, the calibration
    /// header carrying the VIS code, and the starting sync pulse for modes
    /// that transmit one. The image data begins immediately after the last
    /// header tone.
    pub fn header_tones(&self) -> impl Iterator<Item = Tone> + '_ {
        (0..).map_while(move |index| self.header_tone(index))
    }

    /// The `index`-th header tone, or `None` past the end of the header.
    pub(crate) fn header_tone(self, index: usize) -> Option<Tone> {
        let code = u8::from(self.vis_code);
        let bit = |one: bool| {
            let frequency = if one {
                VIS_ONE_FREQUENCY
            } else {
                VIS_ZERO_FREQUENCY
            };
            Tone::new(frequency, VIS_BIT_DURATION)
        };
        match index {
            0..=7 => Some(VOX_TONES[index]),
            8 | 10 => Some(Tone::new(LEADER_FREQUENCY, ms!(300))),
            9 => Some(Tone::new(SYNC_FREQUENCY, ms!(10))), // break
            11 | 20 => Some(Tone::new(SYNC_FREQUENCY, VIS_BIT_DURATION)), // start and stop bits
            12..=18 => Some(bit((code >> (index - 12)) & 1 == 1)), // code bits, least significant first
            19 => Some(bit(code.count_ones() % 2 == 1)),           // even parity
            21 if self.has_starting_sync_pulse() => {
                Some(Tone::new(SYNC_FREQUENCY, self.layout().sync_pulse().1))
            }
            _ => None,
        }
    }
}

/// Look up the mode a [`VisCode`] identifies.
impl TryFrom<VisCode> for Mode {
    type Error = Error;

    /// # Errors
    ///
    /// [`Error::UnknownMode`] if no mode carries the code.
    fn try_from(code: VisCode) -> Result<Self, Error> {
        ALL.into_iter()
            .find(|mode| mode.vis_code == code)
            .ok_or(Error::UnknownMode)
    }
}

// The VIS code is unique to each mode, so it serves as the mode's identity —
// comparing the layouts would walk their timing sequences.
impl PartialEq for Mode {
    fn eq(&self, other: &Self) -> bool {
        self.vis_code == other.vis_code
    }
}

impl Eq for Mode {}

impl core::hash::Hash for Mode {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.vis_code.hash(state);
    }
}

impl core::fmt::Debug for Mode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name)
    }
}

/// The shared assertions behind every mode's tests.
#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use crate::us;

    /// Assert that every timing sequence sums to the paper's line period —
    /// the decoder relies on the sync pulses being evenly spaced.
    pub fn assert_line_period(mode: Mode, expected: Duration) {
        for sequence in mode.layout().sequences {
            let sum = sequence
                .iter()
                .fold(us!(0), |sum, step| sum + step.duration());
            assert_eq!(sum, expected);
        }
    }

    /// Assert that the transcribed steps, summed over all lines, reproduce
    /// the transmission time the paper publishes alongside the per-step
    /// timings — this catches a transcription mistake in any single step.
    pub fn assert_transmission_time(mode: Mode, expected_seconds: f64) {
        let layout = mode.layout();
        let passes = (layout.height / layout.lines_per_sequence) as f64;
        let seconds = passes * layout.sequence_duration().ns() as f64 / 1e9;
        assert!(
            (seconds - expected_seconds).abs() < 0.1,
            "{seconds}s instead of {expected_seconds}s",
        );
    }

    /// Assert that the mode's VIS code round-trips through the lookup — a
    /// collision between two modes fails the round trip.
    pub fn assert_vis_code_round_trips(mode: Mode) {
        assert_eq!(Mode::try_from(mode.vis_code()), Ok(mode));
    }
}
