#!/usr/bin/env python3
"""Remove the STALE build artifacts of target/debug — what no current build of this tree needs.

Why (the owner, 2026-10-30: "the target folder in these sessions eats the disk very fast"):
a fresh full build of the workspace is ~22 GB, but every edit of a core crate leaves a NEW copy
of each of the ~96 test binaries (full debug info, hundreds of MB each) and of the incremental
cache under a new hash, and cargo never removes the old ones. One working day of an agent grew a
worktree's target to 142 GB.

How: cargo itself names what the current tree needs — `--message-format=json` reports every
unit's files, fresh ones included — for the builds the work runs (`cargo test --workspace
--no-run`, `cargo build -p kt-app -p kt-cli`). In `target/debug/deps` every file whose
`<name>-<hash>` is not among them is deleted; in `target/debug/incremental` a crate keeps its
newest directories, as many as the tree has units of it. A deleted artifact that a less usual command needs (a clippy `.rmeta`, another
feature set) is simply rebuilt by that command; nothing that the next `cargo test` or
`finish.sh` needs is touched, so they do not rebuild.

    scripts/target-prune.py            # prune, print what was freed
    scripts/target-prune.py --dry-run  # only print
"""
import json
import os
import re
import subprocess
import sys

SRC = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
TARGET = os.environ.get("CARGO_TARGET_DIR") or os.path.join(SRC, "target")
DEBUG = os.path.join(TARGET, "debug")
BUILDS = [
    ["cargo", "test", "--workspace", "--no-run", "--message-format=json"],
    ["cargo", "build", "-p", "kt-app", "-p", "kt-cli", "--message-format=json"],
]
STEM = re.compile(r"^(.+)-([0-9a-f]{16})(\..*)?$")


def needed_stems():
    keep = set()
    for cmd in BUILDS:
        out = subprocess.run(cmd, cwd=SRC, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
        if out.returncode != 0:
            sys.exit(f"target-prune: `{' '.join(cmd[:3])}` failed — nothing pruned (fix the build first)")
        for line in out.stdout.splitlines():
            try:
                msg = json.loads(line)
            except ValueError:
                continue
            if msg.get("reason") != "compiler-artifact":
                continue
            for f in msg.get("filenames", []) + ([msg["executable"]] if msg.get("executable") else []):
                m = STEM.match(os.path.basename(f))
                if m:
                    keep.add(f"{m.group(1)}-{m.group(2)}")
    return keep


def size(path):
    if os.path.isfile(path) or os.path.islink(path):
        return os.lstat(path).st_size
    total = 0
    for root, _, files in os.walk(path):
        for f in files:
            try:
                total += os.lstat(os.path.join(root, f)).st_size
            except OSError:
                pass
    return total


def main():
    dry = "--dry-run" in sys.argv
    if not os.path.isdir(DEBUG):
        print("target-prune: no target/debug — nothing to do")
        return
    keep = needed_stems()
    if not keep:
        sys.exit("target-prune: cargo named no artifacts — nothing pruned")
    victims = []
    deps = os.path.join(DEBUG, "deps")
    for name in os.listdir(deps) if os.path.isdir(deps) else []:
        m = STEM.match(name)
        if m and f"{m.group(1)}-{m.group(2)}" not in keep:
            victims.append(os.path.join(deps, name))
    # incremental/<crate>-<its own hash>: that hash is not the artifact's, so a crate keeps as many
    # of its NEWEST directories as the current tree has units of it (its lib, its test binary, …);
    # the older ones are the cache of earlier states of the tree
    units = {}
    for stem in keep:
        crate = stem.rsplit("-", 1)[0]
        units[crate] = units.get(crate, 0) + 1
    inc = os.path.join(DEBUG, "incremental")
    by_crate = {}
    for name in os.listdir(inc) if os.path.isdir(inc) else []:
        path = os.path.join(inc, name)
        by_crate.setdefault(name.rsplit("-", 1)[0], []).append((os.path.getmtime(path), path))
    for crate, dirs in by_crate.items():
        dirs.sort(reverse=True)
        victims.extend(p for _, p in dirs[units.get(crate, 0):])
    freed = 0
    for v in victims:
        freed += size(v)
        if not dry:
            if os.path.isdir(v) and not os.path.islink(v):
                subprocess.run(["rm", "-rf", v], check=False)
            else:
                try:
                    os.remove(v)
                except OSError:
                    pass
    verb = "would free" if dry else "freed"
    print(f"target-prune: {verb} {freed / 2**30:.1f} GB in {len(victims)} stale artifacts "
          f"({len(keep)} units of the current tree kept)")


if __name__ == "__main__":
    main()
