#!/usr/bin/env python3
"""Decode the Robot36 fixture with the independent `sstv` decoder as a control.

PySSTV is encode-only, so it cannot decode. This uses colaclanth's `sstv`
decoder on the same fixture our Rust decoder reads, to help tell whether a
timing offset originates in PySSTV's generated signal or in our decoder.

Requires:  pip install --no-deps -r scripts/requirements.txt
Usage:     python3 scripts/decode_with_sstv.py [output.png]
"""

import gzip
import os
import shutil
import subprocess
import sys
import tempfile

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FIXTURE = os.path.join(REPO_ROOT, "local", "patch-robot36-pysstv.wav.gz")
OUTPUT = (
    sys.argv[1]
    if len(sys.argv) > 1
    else os.path.join(REPO_ROOT, "local", "pysstv_decoded_by_pysstv.png")
)


def main() -> None:
    os.makedirs(os.path.dirname(OUTPUT), exist_ok=True)

    # The `sstv` decoder reads a plain WAV, so decompress the fixture first.
    with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as tmp:
        wav_path = tmp.name
    try:
        with gzip.open(FIXTURE, "rb") as src, open(wav_path, "wb") as dst:
            shutil.copyfileobj(src, dst)
        subprocess.run(
            [sys.executable, "-m", "sstv", "-d", wav_path, "-o", OUTPUT],
            check=True,
        )
    finally:
        os.remove(wav_path)

    print(f"wrote {OUTPUT}")


if __name__ == "__main__":
    main()
