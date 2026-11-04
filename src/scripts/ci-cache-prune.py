#!/usr/bin/env python3
"""Keep only the third-party artifacts of a build target directory (change ci-speedup, D1).

The CI jobs that compile inside a container (`windows-build`, `ffmpeg9`) save their target
directory in the Actions cache, which holds 10 GB per repository. The workspace's own crates
rebuild on every push anyway (a checkout resets their mtimes), and their test binaries are most
of the directory, so they do not go in: what is saved is what does not change between pushes.
Cargo's fingerprints decide every rebuild, so a pruned directory can only cost a compile, never a
stale result.

The names come from cargo, not from a convention: `cargo metadata --no-deps --format-version 1`
is read from a file, because the host may have no cargo (the container does). Removed from every
profile directory under the target, a cross target's included: the top-level outputs and the
workspace members' entries in `deps`, `.fingerprint` and `build`, plus `incremental`.

    ci-cache-prune.py <target-dir> <metadata.json>     # prune, print what is left
    ci-cache-prune.py <target-dir> <metadata.json> --dry-run
"""
import json
import os
import re
import shutil
import sys


def names(metadata):
    """The members' package names and their target names, `-` as rustc writes it."""
    packages, targets = set(), set()
    for p in metadata["packages"]:
        packages.add(p["name"])
        targets.add(p["name"].replace("-", "_"))
        for t in p["targets"]:
            targets.add(t["name"].replace("-", "_"))
    return packages, targets


def profiles(target):
    """Every profile directory: `<target>/<profile>` and `<target>/<triple>/<profile>`."""
    for name in sorted(os.listdir(target)):
        path = os.path.join(target, name)
        if not os.path.isdir(path):
            continue
        if os.path.isdir(os.path.join(path, ".fingerprint")):
            yield path
        else:
            yield from (
                os.path.join(path, n)
                for n in sorted(os.listdir(path))
                if os.path.isdir(os.path.join(path, n, ".fingerprint"))
            )


def stem(entry):
    """`libkt_fx-0123abcd.rlib`, `kt_fx-0123abcd.d`, `kt-fx-0123abcd` -> the name before the hash."""
    m = re.match(r"(?:lib)?(.+?)-[0-9a-f]{16}(?:\.|$)", entry)
    return m.group(1) if m else None


def size(path):
    if os.path.islink(path) or os.path.isfile(path):
        return os.lstat(path).st_size
    return sum(os.lstat(os.path.join(d, f)).st_size for d, _, fs in os.walk(path) for f in fs)


def remove(path, dry):
    freed = size(path)
    if not dry:
        if os.path.isdir(path) and not os.path.islink(path):
            shutil.rmtree(path)
        else:
            os.remove(path)
    return freed


def prune(profile, packages, targets, dry):
    freed = 0
    for entry in os.listdir(profile):
        path = os.path.join(profile, entry)
        # the `deps`/`build`/`.fingerprint` of the third-party crates stay; everything else at
        # this level is a member's output (a binary, its `.d`, `examples`, `incremental`)
        if entry not in ("deps", "build", ".fingerprint"):
            freed += remove(path, dry)
    for sub, wanted in (("deps", targets), (".fingerprint", packages), ("build", packages)):
        directory = os.path.join(profile, sub)
        if not os.path.isdir(directory):
            continue
        for entry in os.listdir(directory):
            name = stem(entry)
            if name in wanted or (sub != "deps" and (name or "").replace("_", "-") in packages):
                freed += remove(os.path.join(directory, entry), dry)
    return freed


def main():
    args = [a for a in sys.argv[1:] if a != "--dry-run"]
    if len(args) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    target, meta = args
    dry = "--dry-run" in sys.argv
    with open(meta, encoding="utf-8") as f:
        packages, targets = names(json.load(f))
    if not packages:
        print("ci-cache-prune.py: no workspace members in the metadata", file=sys.stderr)
        return 1
    freed = sum(prune(p, packages, targets, dry) for p in profiles(target))
    print(f"ci-cache-prune.py: freed {freed / 2**30:.2f} GB, {size(target) / 2**30:.2f} GB left in {target}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
