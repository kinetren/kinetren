#!/usr/bin/env python3
"""The offline crate sources of the Flatpak build from `src/Cargo.lock` (change flatpak-flathub, D6).

flatpak-builder may not touch the network during a build, so every crate of the lock is a source of
the manifest: the `.crate` archive from crates.io unpacked into `cargo/vendor/<name>-<version>`, its
`.cargo-checksum.json`, and a cargo config that replaces crates.io with that directory. The layout is
the one `flatpak-cargo-generator.py` (flatpak-builder-tools) writes; this script exists because every
package of our lock comes from the registry, so the twenty lines that matter are kept here instead of
downloading a third-party generator into the build.

    python3 src/scripts/flatpak-cargo-sources.py            # (re)write the sources (make flatpak-sources)
    python3 src/scripts/flatpak-cargo-sources.py --check    # the committed file follows the lock (t968)

`--lock` / `--out` point at other files (the guard's own test).
"""
import argparse
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
LOCK = os.path.join(HERE, "..", "Cargo.lock")
OUT = os.path.join(HERE, "..", "packaging", "flatpak", "cargo-sources.json")
REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"
REGEN = "make flatpak-sources"


def lock_packages(text):
    """(name, version, checksum) of every registry package of a Cargo.lock; anything else is an error."""
    out = []
    for block in text.split("[[package]]")[1:]:
        get = lambda k: (re.search(rf'^{k} = "([^"]*)"', block, re.M) or [None, None])[1]
        name, version, source, checksum = get("name"), get("version"), get("source"), get("checksum")
        if source is None:
            continue  # a workspace member
        if source != REGISTRY or not checksum:
            raise SystemExit(f"flatpak-cargo-sources: {name} {version} comes from {source}: only crates.io is supported")
        out.append((name, version, checksum))
    return sorted(out)


def sources(packages):
    out = []
    for name, version, checksum in packages:
        dest = f"cargo/vendor/{name}-{version}"
        out.append({
            "type": "archive",
            "archive-type": "tar-gzip",
            "url": f"https://static.crates.io/crates/{name}/{name}-{version}.crate",
            "sha256": checksum,
            "dest": dest,
        })
        out.append({
            "type": "inline",
            "contents": json.dumps({"package": checksum, "files": {}}),
            "dest": dest,
            "dest-filename": ".cargo-checksum.json",
        })
    out.append({
        "type": "inline",
        "contents": (
            "[source.vendored-sources]\ndirectory = \"cargo/vendor\"\n\n"
            "[source.crates-io]\nreplace-with = \"vendored-sources\"\n"
        ),
        "dest": "cargo",
        "dest-filename": "config.toml",
    })
    return out


def packages_of(sources_list):
    """(name, version, checksum) named by the archive sources of a committed file."""
    out = []
    for s in sources_list:
        if s.get("type") != "archive":
            continue
        m = re.match(r"https://static\.crates\.io/crates/([^/]+)/\1-(.+)\.crate$", s.get("url", ""))
        if not m:
            raise SystemExit(f"flatpak-cargo-sources: an archive source that is not a crate: {s.get('url')}")
        out.append((m.group(1), m.group(2), s.get("sha256")))
    return sorted(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--lock", default=LOCK)
    ap.add_argument("--out", default=OUT)
    args = ap.parse_args()
    with open(args.lock, encoding="utf-8") as f:
        want = lock_packages(f.read())
    if not args.check:
        os.makedirs(os.path.dirname(args.out), exist_ok=True)
        with open(args.out, "w", encoding="utf-8") as f:
            json.dump(sources(want), f, indent=4)
            f.write("\n")
        print(f"flatpak-cargo-sources: {len(want)} crates written to {os.path.relpath(args.out)}")
        return 0
    try:
        with open(args.out, encoding="utf-8") as f:
            have = packages_of(json.load(f))
    except OSError:
        print(f"{args.out}: missing — run `{REGEN}`", file=sys.stderr)
        return 1
    errors = []
    have_set, want_set = set(have), set(want)
    for name, version, checksum in sorted(want_set - have_set):
        errors.append(f"{name} {version} (Cargo.lock) is not in cargo-sources.json — run `{REGEN}`")
    for name, version, checksum in sorted(have_set - want_set):
        errors.append(f"{name} {version} (cargo-sources.json) is not in Cargo.lock — run `{REGEN}`")
    if errors:
        for e in errors:
            print(e, file=sys.stderr)
        return 1
    print(f"flatpak-cargo-sources: the {len(want)} crates of Cargo.lock are the Flatpak's sources")
    return 0


if __name__ == "__main__":
    sys.exit(main())
