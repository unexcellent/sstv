#!/usr/bin/env python3
"""Encode examples/patch.png into a Robot36 signal using PySSTV.

This generates the interop fixture consumed by the Rust decoder tests: a
gzipped, lossless WAV produced by an independent SSTV implementation. Decoding
it in the test suite verifies that our decoder handles signals it did not
itself encode.

Regenerate the fixture with:

    python3 scripts/encode_with_pysstv.py

Requires:  pip install pysstv pillow
"""

import gzip
import os
import shutil
import tempfile

from PIL import Image
from pysstv.color import Robot36

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
INPUT = os.path.join(REPO_ROOT, "examples", "patch.png")
# Written to the gitignored local/ dir: large and reproducible, so not committed.
OUTPUT = os.path.join(REPO_ROOT, "local", "patch-robot36-pysstv.wav.gz")

# 48 kHz gives the decoder plenty of samples per cycle; 16-bit matches the
# encoder's PCM output.
SAMPLE_RATE = 48_000
BITS = 16


def main() -> None:
    os.makedirs(os.path.dirname(OUTPUT), exist_ok=True)

    image = Image.open(INPUT).convert("RGB")
    image = image.resize((Robot36.WIDTH, Robot36.HEIGHT))

    sstv = Robot36(image, SAMPLE_RATE, BITS)

    # PySSTV writes an uncompressed WAV; gzip it to keep the repo lean.
    with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as tmp:
        wav_path = tmp.name
    try:
        sstv.write_wav(wav_path)
        with open(wav_path, "rb") as raw, gzip.open(OUTPUT, "wb") as compressed:
            shutil.copyfileobj(raw, compressed)
    finally:
        os.remove(wav_path)

    print(f"wrote {OUTPUT}")


if __name__ == "__main__":
    main()
