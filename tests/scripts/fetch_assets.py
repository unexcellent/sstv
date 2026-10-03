#!/usr/bin/env python3
"""Download the media the test suite and the examples use into tests/assets/.

No audio or image is committed to the repository: this script fetches every
one that is missing, and the tests run it on first use. Already present files
are kept, so running it again only fills gaps.

An already-downloaded ISS archive can be passed as the first argument to skip
its ~130 MB download.

Sources and licences:

- patch.png, real_recording.wav.gz: this project's own image and an off-air
  Robot 36 recording of it captured by a ground station, published as assets
  of the repository's `test-assets` release.
- iss/*.wav: off-air ISS recordings by KG4AKV (Space Comms,
  https://spacecomms.wordpress.com/iss-sstv-audio-recordings/), distributed
  as a single zip archive. iss/*.kg4akv.* are the decodes of the same
  transmissions published on that page. No licence is stated.
- commons/: files from Wikimedia Commons, by their authors:
  - martin1-sunset.ogg: "SSTV sunset audio.ogg" by Mysid, CC BY 2.5;
    a Martin 1 transmission made with QSSTV.
  - martin1-sunset.png: "SSTV Sunset.png", decoded by Little Professor from
    Mysid's photo and recording, CC BY-SA 3.0.
  - robot36-french-logo.flac: "French Wikipedia logo in SSTV.flac" by
    Wizly-08, CC0; a Robot 36 transmission made with MMSSTV.
  - robot36-french-logo.png: "French Wikipedia logo in SSTV.png" by Kilyann
    Le Hen, CC0; its decode by the Robot36 app.
  - martin1-german-logo.ogg: "Wikipedia-Logo SSTV.ogg" by Jv at German
    Wikipedia, CC BY-SA 3.0; a Martin 1 transmission.
"""

import os
import shutil
import sys
import tempfile
import urllib.request
import zipfile

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
ASSETS = os.path.join(REPO_ROOT, "tests", "assets")

RELEASE = "https://github.com/unexcellent/sstv/releases/download/test-assets"
KG4AKV_IMAGES = "https://spacecomms.wordpress.com/wp-content/uploads/2017/11"
COMMONS = "https://upload.wikimedia.org/wikipedia/commons"

DOWNLOADS = {
    "patch.png": f"{RELEASE}/patch.png",
    "real_recording.wav.gz": f"{RELEASE}/real_recording.wav.gz",
    "iss/pd180-gagarin-80.kg4akv.jpg": f"{KG4AKV_IMAGES}/pass2-sstv2-201504120428.jpg",
    "iss/pd180-apollo-soyuz.kg4akv.jpg": f"{KG4AKV_IMAGES}/2015-07-19-0631.jpg",
    "iss/pd180-ariss-qso-astros.kg4akv.png":
        f"{KG4AKV_IMAGES}/20160412_175406_-447464744-image_9_12.png",
    "iss/pd180-ariss-qso-cristoforetti.kg4akv.png":
        f"{KG4AKV_IMAGES}/20160413_152853_-447464744.png",
    "iss/pd180-mai75-suitsat.kg4akv.png": f"{KG4AKV_IMAGES}/20160415_153103_-632339566.png",
    "iss/pd120-ariss-20-year-1.kg4akv.png":
        f"{KG4AKV_IMAGES}/image-01of12-2017-07-23-0246-utc-mmsstv.png",
    "iss/pd120-ariss-20-year-2.kg4akv.png": f"{KG4AKV_IMAGES}/image04.png",
    "commons/martin1-sunset.ogg": f"{COMMONS}/c/ce/SSTV_sunset_audio.ogg",
    "commons/martin1-sunset.png": f"{COMMONS}/c/cf/SSTV_Sunset.png",
    "commons/robot36-french-logo.flac": f"{COMMONS}/2/29/French_Wikipedia_logo_in_SSTV.flac",
    "commons/robot36-french-logo.png": f"{COMMONS}/3/3d/French_Wikipedia_logo_in_SSTV.png",
    "commons/martin1-german-logo.ogg": f"{COMMONS}/1/1a/Wikipedia-Logo_SSTV.ogg",
}

ISS_ARCHIVE = (
    "https://www.dropbox.com/s/ljghu4pte455az7/"
    "ISS_SSTV_Audio_Recordings_Space_Comms_KG4AKV_wav.zip?dl=1"
)
ISS_RECORDINGS = {
    "Space_Comms_-_2015-04-12_-_0428_UTC_-_80th_Yuri_Gagarin_image_5.wav":
        "pd180-gagarin-80.wav",
    "Space_Comms_-_2015-07-19_-_0227_UTC_-_Apollo_Souz_American_and_USSR_flag.wav":
        "pd180-apollo-soyuz.wav",
    "Space_Comms_-_2016-04-12_-_2134_UTC_-_ARISS_1st_QSO_-_Astros_-_and_Kids_image_9.wav":
        "pd180-ariss-qso-astros.wav",
    "Space_Comms_-_2016-04-13_-_1904_UTC_-_ARISS_1st_QSO_-_Cristoforetti_Garriot_image_4.wav":
        "pd180-ariss-qso-cristoforetti.wav",
    "Space_Comms_-_2016-04-15_-_1856_UTC_-_MAI-75_-_SuitSat_image_9.wav":
        "pd180-mai75-suitsat.wav",
    "Space_Comms_-_2017-07-23 _-_0246_UTC_-_ARISS_20_Year_-_image_1.wav":
        "pd120-ariss-20-year-1.wav",
    "Space_Comms_-_2017-07-23 _-_0246_UTC_-_ARISS_20_Year_-_image_2.wav":
        "pd120-ariss-20-year-2.wav",
}

# Wikimedia rejects requests without a descriptive User-Agent.
USER_AGENT = "sstv-test-suite/1.0 (https://github.com/unexcellent/sstv)"


def is_missing(name: str) -> bool:
    return not os.path.exists(os.path.join(ASSETS, name))


def download(url: str, destination) -> None:
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request) as response:
        shutil.copyfileobj(response, destination)


def fetch_downloads() -> None:
    for name, url in DOWNLOADS.items():
        if not is_missing(name):
            continue
        print(f"downloading {name}")
        path = os.path.join(ASSETS, name)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "wb") as destination:
            download(url, destination)


def fetch_iss_recordings(archive_path) -> None:
    missing = {
        member: name
        for member, name in ISS_RECORDINGS.items()
        if is_missing(os.path.join("iss", name))
    }
    if not missing:
        return

    if archive_path:
        extract(archive_path, missing)
        return

    print(f"downloading the ISS recordings (~130 MB) from {ISS_ARCHIVE}")
    with tempfile.NamedTemporaryFile(suffix=".zip") as archive:
        download(ISS_ARCHIVE, archive)
        archive.flush()
        extract(archive.name, missing)


def extract(archive_path: str, missing: dict) -> None:
    os.makedirs(os.path.join(ASSETS, "iss"), exist_ok=True)
    with zipfile.ZipFile(archive_path) as archive:
        for member, name in missing.items():
            print(f"extracting iss/{name}")
            with archive.open(member) as source:
                with open(os.path.join(ASSETS, "iss", name), "wb") as out:
                    shutil.copyfileobj(source, out)


def main() -> None:
    fetch_downloads()
    fetch_iss_recordings(sys.argv[1] if len(sys.argv) > 1 else None)
    print(f"assets ready in {ASSETS}")


if __name__ == "__main__":
    main()
