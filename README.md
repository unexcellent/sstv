`sstv`, a slow-scan television crate using minimal memory and no access to rust std.

This crate can be used to encode images into slow-scan television signals (and decode them back) on platforms with low memory availability, like microcontrollers.

Supported modes (encoding and decoding): Scottie 1/2/DX, Martin 1/2, Robot 36/72, Wrasse SC2-180, Pasokon P3/P5/P7 and PD-50/90/120/160/180/240/290.

# Features

The core encoder and decoder are `no_std` (plus `alloc`) and stay that way. Optional features add conveniences on top:

- `std` (default): APIs that need the Rust standard library.
- `image` (implies `std`): encode images straight from disk via the `image` crate — `Encoder::from_image_path(Mode::Pd120, "photo.png")`. Images are resized to the mode's resolution.

Embedded users disable the defaults:

```toml
sstv = { version = "0.1", default-features = false }
```

The minimal build is checked with `cargo build --no-default-features --target thumbv7em-none-eabihf`.

# Mode specifications

All mode timings follow the "Dayton paper": JL Barber (N7CXI), *Proposal for SSTV Mode Specifications*, presented at the Dayton SSTV forum, 20 May 2000. The code is structured to mirror the paper: each mode family lives in its own module under `src/modes/`, transcribing the paper's per-line timing tables (sync pulses, porches, separator pulses and channel scans). The encoder and decoder are generic over these timing sequences, so adding another mode from the paper only means transcribing its table.

The paper's FAX480 is deliberately out of scope: it is a monochrome fax format rather than a true SSTV mode, is essentially unused on air, and is the only mode without the shared calibration header and VIS code. AVT is likewise excluded (as it is from the paper itself).

# Usage

Encoding in `sstv` works iterator based. You need to supply an iterator over `sstv::RgbPixel` to receive an iterator over `sstv::Tone`. These tones contain information about frequency and duration and can then be converted into 16 bit sound samples using `sstv::Synthesizer`.

```rust
use sstv::{Encoder, Mode, RgbPixel, Synthesizer};

let image = [RgbPixel::new(0, 0, 0); 320 * 240];
let encoder = Encoder::new(Mode::Robot36, image.into_iter()).expect("error during encoding");
for sample in Synthesizer::new(encoder, 8000) {
    // emit the samples
}
```

Decoding is the streaming inverse: construct a `sstv::Decoder` from 16 bit samples (or an existing `Demodulator`) and consume it either as whole images or as a stream of scanline events. Pass a specific mode, or `Mode::Auto` to detect each image's mode from its header.

```rust,no_run
use sstv::{Decoder, Mode};

# let samples = std::vec::Vec::<i16>::new().into_iter();
for image in Decoder::from_samples(Mode::Auto, samples, 48000).images() {
    let _ = (image.mode(), image.pixels());
}
```

On memory-constrained receivers, use `.events()` instead of `.images()`: it streams scanlines as they are recovered and holds only about one line group in memory instead of a whole image.

# Planned Features

- reading and writing WAV files behind a `wav` feature
- upload to crates.io
