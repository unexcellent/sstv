`sstv`, a slow-scan television crate using minimal memory and no access to rust std.

This crate can be used to encode images into slow-scan television signals on platforms with low memory availability, like microcontrollers.

It is still far from maturity. Currently, only encoding into Robot36 is supported

# Usage

Encoding in `sstv` works iterator bassed. You need to supply an iterator over `sstv::RgbPixel` to receive an iterator over `sstv::Tone`. These tones contain information about frequency and duration and can then be converted into 16 bit sound samples using `sstv::Synthesizer`.

```rust
use sstv::{Encoder, Mode, RgbPixel, Synthesizer};

let image = [RgbPixel::new(0, 0, 0); 320 * 240];
let encoder = Encoder::new(Mode::Robot36, image.into_iter()).expect("error during encoding");
for sample in Synthesizer::new(encoder, 8000) {
    // emit the samples
}
```

# Planned Features

- using rust features to optionally allow std crates (like image and hound)
- the missing standard modes
- resilient decoding
- upload to crates.io
