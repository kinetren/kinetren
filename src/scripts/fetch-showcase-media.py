#!/usr/bin/env python3
"""Download the free footage and music of the README showcase.

    python3 src/scripts/fetch-showcase-media.py                # into src/target/showcase-media
    python3 src/scripts/fetch-showcase-media.py --dest DIR
    python3 src/scripts/fetch-showcase-media.py --list         # what, how big, which license

Every item is on Wikimedia Commons under CC BY or in the public domain. The
script asks the Commons API for each file's current URL, size and SHA-1, then
downloads one file at a time with a descriptive User-Agent (Wikimedia policy),
resumes partial files, retries with backoff on 429/5xx, and verifies the
SHA-1. Clips whose own soundtrack is not cleared (NASA SVS videos may carry
licensed music) are re-muxed without audio with the `ffmpeg` binary.

Nothing here is committed: the files stay in the destination directory, which
`kt-cli showcase --sources DIR` and `src/scripts/readme-shots.py --sources DIR`
read. `CREDITS.txt` with the attribution of every item is written next to them;
the same credits are in the README.

Total download ≈ 0.9 GB.
"""
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

API = "https://commons.wikimedia.org/w/api.php"
USER_AGENT = ("KinetrenShowcaseFetcher/1.0 (https://github.com/kinetren/kinetren; "
              "log@kinetren.com) python-urllib")
HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_DEST = os.path.join(os.path.dirname(HERE), "target", "showcase-media")

# (local name, Commons file name, title, author, year, license, license URL, mute)
# Local names are ordered: kt-cli showcase sorts its sources by file name.
ITEMS = [
    ("01-charge.webm", "Charge_-_Blender_Open_Movie-full_movie.webm",
     "Charge", "Blender Studio", 2022, "CC BY 4.0",
     "https://creativecommons.org/licenses/by/4.0/", False),
    ("02-spring.webm", "Spring_-_Blender_Open_Movie.webm",
     "Spring", "Andy Goralczyk / Blender Studio", 2019, "CC BY 4.0",
     "https://creativecommons.org/licenses/by/4.0/", False),
    ("03-coffee-run.webm", "Coffee_Run_-_Blender_Open_Movie-full_movie.webm",
     "Coffee Run", "Blender Studio", 2020, "CC BY 4.0",
     "https://creativecommons.org/licenses/by/4.0/", False),
    ("04-lake-bled.webm", "Slovenia_-_Lake_Bled_-_4K_drone_video.webm",
     "Slovenia – Lake Bled – 4K drone video", "Danny Rest", 2018, "CC BY 3.0",
     "https://creativecommons.org/licenses/by/3.0/", False),
    ("05-charlotte-by-night.webm", "Charlotte_By_Night_-_4K_Drone_Footage.webm",
     "Charlotte By Night – 4K Drone Footage", "Hallum Creative", 2021, "CC BY 3.0",
     "https://creativecommons.org/licenses/by/3.0/", False),
    ("06-webb-jupiter-vertical.webm",
     "Webb_Captures_Jupiter’s_Aurora_(SVS14839_-_VERTICAL).webm",
     "Webb Captures Jupiter's Aurora (vertical)", "NASA Scientific Visualization Studio",
     2025, "Public domain", "https://www.nasa.gov/nasa-brand-center/images-and-media/", True),
    ("07-cobe-wmap-square.webm",
     "Astrophysics-_Observing_the_Universe_Vertical_Video_(SVS14799_-_COBE_WMAP_detail).webm",
     "Astrophysics: Observing the Universe – COBE/WMAP detail",
     "NASA Scientific Visualization Studio", 2025, "Public domain",
     "https://www.nasa.gov/nasa-brand-center/images-and-media/", True),
    ("80-driving-to-the-night.ogg", "Driving_to_the_Night_by_Frank_Schroeter.ogg",
     "Driving to the Night", "Frank Schroeter", 2021, "CC BY 4.0",
     "https://creativecommons.org/licenses/by/4.0/", False),
    ("81-far-away.opus", "DOS-88_-_Far_Away.opus",
     "Far Away", "DOS-88", 2016, "CC BY 3.0",
     "https://creativecommons.org/licenses/by/3.0/", False),
]


def http(url, headers=None, tries=6):
    """GET with the Wikimedia User-Agent; backoff on 429 and 5xx."""
    delay = 2.0
    for attempt in range(tries):
        req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT, **(headers or {})})
        try:
            return urllib.request.urlopen(req, timeout=60)
        except urllib.error.HTTPError as e:
            if e.code == 416:  # range past the end: the file is complete
                return e
            if e.code not in (429, 500, 502, 503, 504) or attempt == tries - 1:
                raise
            wait = float(e.headers.get("Retry-After") or delay)
        except urllib.error.URLError:
            if attempt == tries - 1:
                raise
            wait = delay
        print(f"    retry in {wait:.0f} s", flush=True)
        time.sleep(wait)
        delay *= 2
    raise RuntimeError("unreachable")


def file_info(name):
    q = urllib.parse.urlencode({
        "action": "query", "format": "json", "prop": "imageinfo",
        "iiprop": "url|sha1|size|mime", "titles": f"File:{name}",
    })
    with http(f"{API}?{q}") as r:
        data = json.load(r)
    page = next(iter(data["query"]["pages"].values()))
    if "imageinfo" not in page:
        raise SystemExit(f"Commons has no file “{name}” (renamed or deleted?)")
    ii = page["imageinfo"][0]
    return ii["url"], ii["sha1"], ii["size"], ii.get("descriptionurl", "")


def sha1_of(path):
    h = hashlib.sha1()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def download(url, path, size):
    part = path + ".part"
    have = os.path.getsize(part) if os.path.exists(part) else 0
    if have >= size:
        return part
    headers = {"Range": f"bytes={have}-"} if have else {}
    with http(url, headers) as r, open(part, "ab" if have else "wb") as out:
        if have and getattr(r, "status", 200) != 206:
            out.truncate(0)  # the server ignored the range: start over
            have = 0
        done, last = have, time.time()
        while True:
            chunk = r.read(1 << 20)
            if not chunk:
                break
            out.write(chunk)
            done += len(chunk)
            if time.time() - last > 2:
                print(f"    {done / size:6.1%}  {done >> 20} / {size >> 20} MB", flush=True)
                last = time.time()
    return part


def mute(src, dst):
    if not shutil.which("ffmpeg"):
        raise SystemExit("ffmpeg is needed to strip the soundtrack of the NASA clips")
    tmp = dst + ".tmp.webm"
    subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-i", src,
                    "-map", "0:v", "-c", "copy", tmp], check=True)
    os.replace(tmp, dst)


def credits_text(rows):
    out = ["Footage and music of the Kinetren showcase (fetched by",
           "src/scripts/fetch-showcase-media.py). Excerpts were edited.", ""]
    for local, _, title, author, year, lic, lic_url, muted, page in rows:
        line = f"{local}: \"{title}\" by {author} ({year}), {lic} — {lic_url}"
        out.append(line)
        out.append(f"    source: {page}" + ("; soundtrack removed" if muted else ""))
    return "\n".join(out) + "\n"


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--dest", default=DEFAULT_DEST)
    ap.add_argument("--list", action="store_true", help="print the items and exit")
    args = ap.parse_args()

    if args.list:
        for local, name, title, author, year, lic, _, muted in ITEMS:
            print(f"{local:30} {lic:14} {title} — {author} ({year})"
                  + ("  [muted]" if muted else ""))
        return 0

    os.makedirs(args.dest, exist_ok=True)
    rows = []
    for local, name, title, author, year, lic, lic_url, muted in ITEMS:
        path = os.path.join(args.dest, local)
        url, sha1, size, page = file_info(name)
        rows.append((local, name, title, author, year, lic, lic_url, muted, page))
        stamp = path + ".sha1"
        if os.path.exists(path) and os.path.exists(stamp) and open(stamp).read().strip() == sha1:
            print(f"ok   {local} (already here)")
            continue
        print(f"get  {local}  {size >> 20} MB  {lic}", flush=True)
        part = download(url, path, size)
        got = sha1_of(part)
        if got != sha1:
            os.remove(part)
            raise SystemExit(f"{local}: SHA-1 mismatch ({got} != {sha1}); removed, run again")
        if muted:
            mute(part, path)
            os.remove(part)
        else:
            os.replace(part, path)
        with open(stamp, "w") as f:
            f.write(sha1 + "\n")
        print(f"ok   {local}")
        time.sleep(1.0)  # be gentle with upload.wikimedia.org
    with open(os.path.join(args.dest, "CREDITS.txt"), "w") as f:
        f.write(credits_text(rows))
    print(f"\n{len(rows)} items in {args.dest}; credits in CREDITS.txt")
    print("next:  cd src && cargo build -p kt-app -p kt-cli && cd .. && "
          f"python3 src/scripts/readme-shots.py --sources {args.dest}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
