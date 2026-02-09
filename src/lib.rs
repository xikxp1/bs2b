//! Bauer stereophonic-to-binaural (bs2b) crossfeed DSP.
//!
//! This crate implements the classic bs2b algorithm with an ergonomic Rust API.
#![cfg_attr(feature = "no_std", no_std)]
#![forbid(unsafe_code)]

#[cfg(all(feature = "std", feature = "no_std"))]
compile_error!(
    "features `std` and `no_std` are mutually exclusive; disable default features to use `no_std`"
);

use core::f64::consts::{LN_10, PI};
use core::fmt;

#[cfg(test)]
extern crate std;

/// Minimum supported sample rate in Hz.
pub const MIN_SAMPLE_RATE: u32 = 2_000;
/// Maximum supported sample rate in Hz.
pub const MAX_SAMPLE_RATE: u32 = 384_000;
/// Default sample rate in Hz used by the original library.
pub const DEFAULT_SAMPLE_RATE: u32 = 44_100;

/// Minimum crossfeed cut frequency in Hz.
pub const MIN_CUT_FREQUENCY: u32 = 300;
/// Maximum crossfeed cut frequency in Hz.
pub const MAX_CUT_FREQUENCY: u32 = 2_000;

/// Minimum feed level in dB * 10.
pub const MIN_FEED_DB_TENTHS: u32 = 10;
/// Maximum feed level in dB * 10.
pub const MAX_FEED_DB_TENTHS: u32 = 150;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Crossfeed level definition.
pub struct Level {
    /// Low-pass crossover in Hz.
    cut_frequency_hz: u32,
    /// Crossfeed level at low frequencies in dB * 10.
    feed_db_tenths: u32,
}

impl Level {
    /// Original bs2b default profile.
    pub const DEFAULT: Self = Self {
        cut_frequency_hz: 700,
        feed_db_tenths: 45,
    };

    /// Chu Moy profile.
    pub const CMOY: Self = Self {
        cut_frequency_hz: 700,
        feed_db_tenths: 60,
    };

    /// Jan Meier profile.
    pub const JMEIER: Self = Self {
        cut_frequency_hz: 650,
        feed_db_tenths: 95,
    };

    /// Creates a validated level.
    pub fn new(cut_frequency_hz: u32, feed_db_tenths: u32) -> Result<Self, Bs2bError> {
        if !(MIN_CUT_FREQUENCY..=MAX_CUT_FREQUENCY).contains(&cut_frequency_hz) {
            return Err(Bs2bError::InvalidCutFrequency(cut_frequency_hz));
        }
        if !(MIN_FEED_DB_TENTHS..=MAX_FEED_DB_TENTHS).contains(&feed_db_tenths) {
            return Err(Bs2bError::InvalidFeedLevel(feed_db_tenths));
        }

        Ok(Self {
            cut_frequency_hz,
            feed_db_tenths,
        })
    }

    /// Packs the level into the original C format.
    pub const fn packed(self) -> u32 {
        self.cut_frequency_hz | (self.feed_db_tenths << 16)
    }

    /// Returns low-pass crossover in Hz.
    pub const fn cut_frequency_hz(self) -> u32 {
        self.cut_frequency_hz
    }

    /// Returns crossfeed level at low frequencies in dB * 10.
    pub const fn feed_db_tenths(self) -> u32 {
        self.feed_db_tenths
    }

    /// Unpacks the original C level representation and validates it.
    pub fn from_packed(value: u32) -> Result<Self, Bs2bError> {
        let cut_frequency_hz = value & 0xffff;
        let feed_db_tenths = value >> 16;
        Self::new(cut_frequency_hz, feed_db_tenths)
    }

    /// Delay at low frequencies, in microseconds.
    pub const fn delay_microseconds(self) -> u32 {
        (18_700 / self.cut_frequency_hz) * 10
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Configuration and processing errors.
pub enum Bs2bError {
    InvalidSampleRate(u32),

    InvalidCutFrequency(u32),

    InvalidFeedLevel(u32),

    OddInterleavedSamples(usize),

    MismatchedPlanarLengths { left: usize, right: usize },
}

impl fmt::Display for Bs2bError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSampleRate(sample_rate) => write!(
                f,
                "sample rate {sample_rate} is out of range [{MIN_SAMPLE_RATE}, {MAX_SAMPLE_RATE}]"
            ),
            Self::InvalidCutFrequency(cut_frequency_hz) => write!(
                f,
                "cut frequency {cut_frequency_hz} Hz is out of range [{MIN_CUT_FREQUENCY}, {MAX_CUT_FREQUENCY}]"
            ),
            Self::InvalidFeedLevel(feed_db_tenths) => write!(
                f,
                "feed level {feed_db_tenths} (dB*10) is out of range [{MIN_FEED_DB_TENTHS}, {MAX_FEED_DB_TENTHS}]"
            ),
            Self::OddInterleavedSamples(sample_count) => write!(
                f,
                "interleaved stereo buffer must have an even number of samples, got {sample_count}"
            ),
            Self::MismatchedPlanarLengths { left, right } => write!(
                f,
                "left/right planar buffers must have equal length, got {left} and {right}"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Bs2bError {}

#[derive(Debug, Clone, Copy)]
struct Coefficients {
    a0_lo: f64,
    b1_lo: f64,
    a0_hi: f64,
    a1_hi: f64,
    b1_hi: f64,
    gain: f64,
}

impl Coefficients {
    fn from_level(level: Level, sample_rate: u32) -> Self {
        let fc_lo = level.cut_frequency_hz as f64;
        let level_db = level.feed_db_tenths as f64 / 10.0;

        let gb_lo = level_db * -5.0 / 6.0 - 3.0;
        let gb_hi = level_db / 6.0 - 3.0;

        let g_lo = powf(10.0, gb_lo / 20.0);
        let g_hi = 1.0 - powf(10.0, gb_hi / 20.0);

        let fc_hi = fc_lo * powf(2.0, (gb_lo - 20.0 * log10(g_hi)) / 12.0);

        let x_lo = exp(-2.0 * PI * fc_lo / sample_rate as f64);
        let b1_lo = x_lo;
        let a0_lo = g_lo * (1.0 - x_lo);

        let x_hi = exp(-2.0 * PI * fc_hi / sample_rate as f64);
        let b1_hi = x_hi;
        let a0_hi = 1.0 - g_hi * (1.0 - x_hi);
        let a1_hi = -x_hi;

        let gain = 1.0 / (1.0 - g_hi + g_lo);

        Self {
            a0_lo,
            b1_lo,
            a0_hi,
            a1_hi,
            b1_hi,
            gain,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct FilterState {
    asis: [f64; 2],
    lo: [f64; 2],
    hi: [f64; 2],
}

/// Stateful bs2b DSP processor.
#[derive(Debug, Clone)]
pub struct Bs2b {
    sample_rate: u32,
    level: Level,
    coefficients: Coefficients,
    state: FilterState,
}

impl Default for Bs2b {
    fn default() -> Self {
        Self::new(DEFAULT_SAMPLE_RATE, Level::DEFAULT)
            .expect("default level and sample rate are valid")
    }
}

impl Bs2b {
    /// Creates a new processor.
    pub fn new(sample_rate: u32, level: Level) -> Result<Self, Bs2bError> {
        validate_sample_rate(sample_rate)?;

        Ok(Self {
            sample_rate,
            level,
            coefficients: Coefficients::from_level(level, sample_rate),
            state: FilterState::default(),
        })
    }

    /// Creates a processor from the packed C level representation.
    pub fn from_packed_level(sample_rate: u32, level: u32) -> Result<Self, Bs2bError> {
        let level = Level::from_packed(level)?;
        Self::new(sample_rate, level)
    }

    /// Returns the current sample rate in Hz.
    pub const fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Returns the current level.
    pub const fn level(&self) -> Level {
        self.level
    }

    /// Returns the current packed level representation.
    pub const fn packed_level(&self) -> u32 {
        self.level.packed()
    }

    /// Returns the low-frequency delay for the current level in microseconds.
    pub const fn level_delay_microseconds(&self) -> u32 {
        self.level.delay_microseconds()
    }

    /// Updates the sample rate and clears the filter history.
    pub fn set_sample_rate(&mut self, sample_rate: u32) -> Result<(), Bs2bError> {
        validate_sample_rate(sample_rate)?;

        if self.sample_rate == sample_rate {
            return Ok(());
        }

        self.sample_rate = sample_rate;
        self.coefficients = Coefficients::from_level(self.level, sample_rate);
        self.clear();
        Ok(())
    }

    /// Updates the crossfeed level while preserving filter history.
    pub fn set_level(&mut self, level: Level) {
        if self.level == level {
            return;
        }

        self.level = level;
        self.coefficients = Coefficients::from_level(level, self.sample_rate);
    }

    /// Clears filter history.
    pub fn clear(&mut self) {
        self.state = FilterState::default();
    }

    /// Returns true if filter history is fully cleared.
    pub fn is_clear(&self) -> bool {
        self.state.asis.iter().all(|v| *v == 0.0)
            && self.state.lo.iter().all(|v| *v == 0.0)
            && self.state.hi.iter().all(|v| *v == 0.0)
    }

    /// Processes one stereo frame and returns the transformed frame.
    pub fn process_frame<T: Sample>(&mut self, left: T, right: T) -> (T, T) {
        let (left, right) = self.process_frame_f64(left.to_f64(), right.to_f64());
        (
            T::from_f64(left.clamp(T::MIN_VALUE, T::MAX_VALUE)),
            T::from_f64(right.clamp(T::MIN_VALUE, T::MAX_VALUE)),
        )
    }

    /// Processes an interleaved stereo buffer in-place.
    pub fn process_interleaved<T: Sample>(&mut self, samples: &mut [T]) -> Result<(), Bs2bError> {
        if !samples.len().is_multiple_of(2) {
            return Err(Bs2bError::OddInterleavedSamples(samples.len()));
        }

        for frame in samples.chunks_exact_mut(2) {
            let (left, right) = self.process_frame(frame[0], frame[1]);
            frame[0] = left;
            frame[1] = right;
        }

        Ok(())
    }

    /// Processes left/right planar buffers in-place.
    pub fn process_planar<T: Sample>(
        &mut self,
        left: &mut [T],
        right: &mut [T],
    ) -> Result<(), Bs2bError> {
        if left.len() != right.len() {
            return Err(Bs2bError::MismatchedPlanarLengths {
                left: left.len(),
                right: right.len(),
            });
        }

        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            (*l, *r) = self.process_frame(*l, *r);
        }

        Ok(())
    }

    fn process_frame_f64(&mut self, left: f64, right: f64) -> (f64, f64) {
        self.state.lo[0] =
            self.coefficients.a0_lo * left + self.coefficients.b1_lo * self.state.lo[0];
        self.state.lo[1] =
            self.coefficients.a0_lo * right + self.coefficients.b1_lo * self.state.lo[1];

        self.state.hi[0] = self.coefficients.a0_hi * left
            + self.coefficients.a1_hi * self.state.asis[0]
            + self.coefficients.b1_hi * self.state.hi[0];
        self.state.hi[1] = self.coefficients.a0_hi * right
            + self.coefficients.a1_hi * self.state.asis[1]
            + self.coefficients.b1_hi * self.state.hi[1];

        self.state.asis[0] = left;
        self.state.asis[1] = right;

        let left = (self.state.hi[0] + self.state.lo[1]) * self.coefficients.gain;
        let right = (self.state.hi[1] + self.state.lo[0]) * self.coefficients.gain;

        (left, right)
    }
}

fn validate_sample_rate(sample_rate: u32) -> Result<(), Bs2bError> {
    if (MIN_SAMPLE_RATE..=MAX_SAMPLE_RATE).contains(&sample_rate) {
        Ok(())
    } else {
        Err(Bs2bError::InvalidSampleRate(sample_rate))
    }
}

fn log10(value: f64) -> f64 {
    ln(value) / LN_10
}

#[cfg(feature = "no_std")]
fn powf(value: f64, power: f64) -> f64 {
    libm::pow(value, power)
}

#[cfg(not(feature = "no_std"))]
fn powf(value: f64, power: f64) -> f64 {
    value.powf(power)
}

#[cfg(feature = "no_std")]
fn exp(value: f64) -> f64 {
    libm::exp(value)
}

#[cfg(not(feature = "no_std"))]
fn exp(value: f64) -> f64 {
    value.exp()
}

#[cfg(feature = "no_std")]
fn ln(value: f64) -> f64 {
    libm::log(value)
}

#[cfg(not(feature = "no_std"))]
fn ln(value: f64) -> f64 {
    value.ln()
}

mod private {
    pub trait Sealed {}
}

/// Sample type that can be processed by the bs2b processor.
pub trait Sample: private::Sealed + Copy {
    const MIN_VALUE: f64;
    const MAX_VALUE: f64;

    fn to_f64(self) -> f64;
    fn from_f64(value: f64) -> Self;
}

impl private::Sealed for f64 {}
impl Sample for f64 {
    const MIN_VALUE: f64 = -1.0;
    const MAX_VALUE: f64 = 1.0;

    #[inline]
    fn to_f64(self) -> f64 {
        self
    }

    #[inline]
    fn from_f64(value: f64) -> Self {
        value
    }
}

impl private::Sealed for f32 {}
impl Sample for f32 {
    const MIN_VALUE: f64 = -1.0;
    const MAX_VALUE: f64 = 1.0;

    #[inline]
    fn to_f64(self) -> f64 {
        self as f64
    }

    #[inline]
    fn from_f64(value: f64) -> Self {
        value as f32
    }
}

impl private::Sealed for i32 {}
impl Sample for i32 {
    const MIN_VALUE: f64 = i32::MIN as f64;
    const MAX_VALUE: f64 = i32::MAX as f64;

    #[inline]
    fn to_f64(self) -> f64 {
        self as f64
    }

    #[inline]
    fn from_f64(value: f64) -> Self {
        value as i32
    }
}

impl private::Sealed for u32 {}
impl Sample for u32 {
    const MIN_VALUE: f64 = i32::MIN as f64;
    const MAX_VALUE: f64 = i32::MAX as f64;

    #[inline]
    fn to_f64(self) -> f64 {
        ((self ^ 0x8000_0000) as i32) as f64
    }

    #[inline]
    fn from_f64(value: f64) -> Self {
        (value as i32 as u32) ^ 0x8000_0000
    }
}

impl private::Sealed for i16 {}
impl Sample for i16 {
    const MIN_VALUE: f64 = i16::MIN as f64;
    const MAX_VALUE: f64 = i16::MAX as f64;

    #[inline]
    fn to_f64(self) -> f64 {
        self as f64
    }

    #[inline]
    fn from_f64(value: f64) -> Self {
        value as i16
    }
}

impl private::Sealed for u16 {}
impl Sample for u16 {
    const MIN_VALUE: f64 = i16::MIN as f64;
    const MAX_VALUE: f64 = i16::MAX as f64;

    #[inline]
    fn to_f64(self) -> f64 {
        ((self ^ 0x8000) as i16) as f64
    }

    #[inline]
    fn from_f64(value: f64) -> Self {
        (value as i16 as u16) ^ 0x8000
    }
}

impl private::Sealed for i8 {}
impl Sample for i8 {
    const MIN_VALUE: f64 = i8::MIN as f64;
    const MAX_VALUE: f64 = i8::MAX as f64;

    #[inline]
    fn to_f64(self) -> f64 {
        self as f64
    }

    #[inline]
    fn from_f64(value: f64) -> Self {
        value as i8
    }
}

impl private::Sealed for u8 {}
impl Sample for u8 {
    const MIN_VALUE: f64 = i8::MIN as f64;
    const MAX_VALUE: f64 = i8::MAX as f64;

    #[inline]
    fn to_f64(self) -> f64 {
        ((self ^ 0x80) as i8) as f64
    }

    #[inline]
    fn from_f64(value: f64) -> Self {
        (value as i8 as u8) ^ 0x80
    }
}

#[cfg(test)]
mod tests {
    use approx::assert_abs_diff_eq;
    use rand::rngs::StdRng;
    use rand::{RngExt, SeedableRng};
    use std::vec::Vec;

    use super::*;

    #[test]
    fn level_validation_checks_bounds() {
        assert!(Level::new(700, 45).is_ok());
        assert!(matches!(
            Level::new(299, 45),
            Err(Bs2bError::InvalidCutFrequency(299))
        ));
        assert!(matches!(
            Level::new(700, 151),
            Err(Bs2bError::InvalidFeedLevel(151))
        ));
    }

    #[test]
    fn sample_rate_validation_checks_bounds() {
        assert!(Bs2b::new(MIN_SAMPLE_RATE, Level::DEFAULT).is_ok());
        assert!(matches!(
            Bs2b::new(MIN_SAMPLE_RATE - 1, Level::DEFAULT),
            Err(Bs2bError::InvalidSampleRate(1_999))
        ));
    }

    #[test]
    fn delay_microseconds_matches_reference_formula() {
        let level = Level::DEFAULT;
        assert_eq!(level.delay_microseconds(), (18_700 / 700) * 10);
    }

    #[test]
    fn clear_and_is_clear_roundtrip() {
        let mut bs2b = Bs2b::default();
        let _ = bs2b.process_frame(0.5_f32, -0.25_f32);
        assert!(!bs2b.is_clear());

        bs2b.clear();
        assert!(bs2b.is_clear());
    }

    #[test]
    fn set_sample_rate_clears_state() {
        let mut bs2b = Bs2b::default();
        let _ = bs2b.process_frame(0.2_f32, -0.1_f32);
        assert!(!bs2b.is_clear());

        bs2b.set_sample_rate(48_000)
            .expect("48 kHz should be accepted");
        assert!(bs2b.is_clear());
    }

    #[test]
    fn interleaved_rejects_odd_length() {
        let mut bs2b = Bs2b::default();
        let mut data = std::vec![0.1_f32, 0.2, 0.3];
        assert!(matches!(
            bs2b.process_interleaved(&mut data),
            Err(Bs2bError::OddInterleavedSamples(3))
        ));
    }

    #[test]
    fn planar_rejects_mismatched_lengths() {
        let mut bs2b = Bs2b::default();
        let mut left = std::vec![0.1_f32, 0.2];
        let mut right = std::vec![0.3_f32];

        assert!(matches!(
            bs2b.process_planar(&mut left, &mut right),
            Err(Bs2bError::MismatchedPlanarLengths { left: 2, right: 1 })
        ));
    }

    #[test]
    fn planar_and_interleaved_match() {
        let mut interleaved = std::vec![0.4_f32, -0.2, 0.1, 0.9, -0.8, 0.3, 0.05, -0.4];
        let mut left = std::vec![0.4_f32, 0.1, -0.8, 0.05];
        let mut right = std::vec![-0.2_f32, 0.9, 0.3, -0.4];

        let mut a = Bs2b::default();
        let mut b = Bs2b::default();

        a.process_interleaved(&mut interleaved)
            .expect("interleaved buffer should be valid");
        b.process_planar(&mut left, &mut right)
            .expect("planar buffers should be valid");

        for (idx, frame) in interleaved.chunks_exact(2).enumerate() {
            assert_abs_diff_eq!(frame[0], left[idx], epsilon = 1.0e-7);
            assert_abs_diff_eq!(frame[1], right[idx], epsilon = 1.0e-7);
        }
    }

    #[test]
    fn clips_float_output_to_unit_range() {
        let mut bs2b = Bs2b::default();
        let mut data = std::vec![10.0_f32, -10.0, 10.0, -10.0, 10.0, -10.0];
        bs2b.process_interleaved(&mut data)
            .expect("buffer should be valid");

        assert!(data.iter().all(|sample| (-1.0..=1.0).contains(sample)));
    }

    #[test]
    fn unsigned_and_signed_16_bit_paths_match() {
        let mut signed = std::vec![1000_i16, -2000, 8000, -100, -32000, 30000];
        let mut unsigned: Vec<u16> = signed.iter().map(|v| (*v as u16) ^ 0x8000).collect();

        let mut a = Bs2b::default();
        let mut b = Bs2b::default();

        a.process_interleaved(&mut signed)
            .expect("buffer should be valid");
        b.process_interleaved(&mut unsigned)
            .expect("buffer should be valid");

        let decoded: Vec<i16> = unsigned.into_iter().map(|v| (v ^ 0x8000) as i16).collect();

        assert_eq!(signed, decoded);
    }

    #[test]
    fn deterministic_against_reference_path() {
        let mut rng = StdRng::seed_from_u64(0x5eed_ba11);
        let mut input = Vec::with_capacity(512);
        for _ in 0..256 {
            input.push(rng.random_range(-1.0_f64..=1.0));
            input.push(rng.random_range(-1.0_f64..=1.0));
        }

        let mut a = Bs2b::default();
        let mut b = Bs2b::default();

        let mut output = input.clone();
        a.process_interleaved(&mut output)
            .expect("buffer should be valid");

        let mut expected = Vec::with_capacity(input.len());
        for frame in input.chunks_exact(2) {
            let (l, r) = reference_step(&mut b, frame[0], frame[1]);
            expected.push(l.clamp(-1.0, 1.0));
            expected.push(r.clamp(-1.0, 1.0));
        }

        for (lhs, rhs) in output.iter().zip(expected.iter()) {
            assert_abs_diff_eq!(lhs, rhs, epsilon = 1.0e-12);
        }
    }

    fn reference_step(bs2b: &mut Bs2b, left: f64, right: f64) -> (f64, f64) {
        let c = bs2b.coefficients;

        let lo_left = c.a0_lo * left + c.b1_lo * bs2b.state.lo[0];
        let lo_right = c.a0_lo * right + c.b1_lo * bs2b.state.lo[1];

        let hi_left = c.a0_hi * left + c.a1_hi * bs2b.state.asis[0] + c.b1_hi * bs2b.state.hi[0];
        let hi_right = c.a0_hi * right + c.a1_hi * bs2b.state.asis[1] + c.b1_hi * bs2b.state.hi[1];

        bs2b.state.asis = [left, right];
        bs2b.state.lo = [lo_left, lo_right];
        bs2b.state.hi = [hi_left, hi_right];

        ((hi_left + lo_right) * c.gain, (hi_right + lo_left) * c.gain)
    }
}
