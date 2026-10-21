#!/usr/bin/env python3
"""Re-take every README / docs/guide image from real footage.

    python3 src/scripts/readme-shots.py --sources ../sources/README_SOURCES
    python3 src/scripts/readme-shots.py --sources DIR --out /tmp/shots --keep

Builds the showcase project from the videos in DIR (`kt-cli showcase`),
starts its own virtual screen (Xvfb + openbox, nothing touches your
desktop) and shoots the English UI of the freshly built `kinetren`. The
pipeline itself — gestures, probes, crops, the list of images — lives in
`scripts/live/readme_shots.py`, which `live-check.sh` also runs on the
fixture videos, so this command cannot rot unnoticed.

Build first: `cd src && cargo build -p kt-app -p kt-cli`. The footage is
referenced, never copied; only the WebP images land in `--out`
(default `docs/images/`).
"""
import argparse
import os
import shutil
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
LIVE = os.path.join(HERE, "live")
sys.path.insert(0, LIVE)


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--sources", help="directory with the footage (videos with audio)")
    ap.add_argument("--out", default=os.path.join(REPO, "docs", "images"),
                    help="where the WebP images go (default: docs/images)")
    ap.add_argument("--work", default=os.path.join(REPO, "src", "target", "readme-shots"),
                    help="scratch directory: project, sidecar index, raw screenshots")
    ap.add_argument("--display", default=os.environ.get("KT_SHOT_DISPLAY", ":96"))
    ap.add_argument("--keep", action="store_true", help="keep the full-screen PNGs in WORK/raw")
    args = ap.parse_args()
    if not args.sources:
        ap.print_usage(sys.stderr)
        print("readme-shots.py: --sources DIR is required", file=sys.stderr)
        return 2
    if not os.path.isdir(args.sources):
        print(f"readme-shots.py: {args.sources}: not a directory", file=sys.stderr)
        return 2
    for tool in ("kinetren", "kt-cli"):
        path = os.path.join(REPO, "src", "target", "debug", tool)
        if not os.path.exists(path):
            sys.exit(f"no {path} — build it: cd src && cargo build -p kt-app -p kt-cli")
    for tool in ("Xvfb", "openbox", "xwd", "convert"):
        if not shutil.which(tool):
            sys.exit(f"no {tool} in PATH")

    from worker import Screen

    os.makedirs(args.work, exist_ok=True)
    # our own screen, our own processes: the shared bench of live-check
    # must not leak in
    os.environ.pop("KT_LIVE_BENCH", None)
    with Screen(args.display, args.work):
        import readme_shots

        try:
            written = readme_shots.shoot(os.path.abspath(args.sources), os.path.abspath(args.out),
                                         os.path.abspath(args.work), keep_raw=args.keep)
        except readme_shots.StepError as e:
            sys.exit(f"FAILED: {e}\n  screenshots and logs: {os.path.join(args.work, 'raw')}")
    for path in written:
        print(f"  {os.path.relpath(path, REPO)}  {os.path.getsize(path) // 1024} KB")
    return 0


if __name__ == "__main__":
    sys.exit(main())
