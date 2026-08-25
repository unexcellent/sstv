`sstv`, a slow-scan television crate using minimal memory and no access to rust std.

This crate can be used to encode images into slow-scan television signals (and decode them back) on platforms with low memory availability, like microcontrollers.

It is still far from maturity. Currently, only Robot 36 is supported.

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

Decoding is the streaming inverse: feed 16 bit samples into `sstv::RowDecoder` and receive `sstv::Event`s grouping decoded scanlines into images.

```rust,no_run
use sstv::{Event, Mode, RowDecoder};

# let samples = std::vec::Vec::<i16>::new().into_iter();
for event in RowDecoder::new(Mode::Robot36, samples, 48000) {
    match event {
        Event::ImageStart(info) => { /* a new image begins */ }
        Event::Row(row) => { /* one decoded scanline */ }
        Event::ImageEnd { complete } => { /* the image finished */ }
    }
}
```

# Planned Features

- using rust features to optionally allow std crates (like image and hound)
- the remaining modes from the Dayton paper (Scottie, Martin, Robot 72, Wrasse, Pasokon, PD)
- upload to crates.io
