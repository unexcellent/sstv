// Test helpers outside #[test] functions are not covered by the clippy.toml
// test allowances.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! The integration tests, built into a single test binary: every binary
//! costs a link and, on macOS, a malware scan of its first launch, which
//! together outweighed the tests themselves. The allocation test stays a
//! binary of its own (`tests/encoder_allocations.rs`), as it replaces the
//! global allocator.

mod common;

mod all_modes;
mod commons_recordings;
mod end_to_end;
#[cfg(feature = "image")]
mod image_feature;
mod interop;
mod iss_recordings;
#[cfg(feature = "mp3")]
mod mp3_feature;
mod real_world;
#[cfg(feature = "wav")]
mod wav_feature;
