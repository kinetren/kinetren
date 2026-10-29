#!/usr/bin/env python3
"""What an edit of one effect recompiles, and how long it takes (change plugin-api-stage1).

The measuring form (task 1.1): on a warm tree, touch a file, run a cargo command, and report the
wall time and the workspace crates cargo compiled after a real one-line edit. Stage 7 turns it into the acceptance check t977
(the recompiled set after touching a plugin kernel is the family, the assembler and the binaries).

    python3 src/scripts/plugin-rebuild-check.py measure <file> [-- cargo args…]
        # default cargo args: build -p kt-app -p kt-cli

R.68's numbers (32 cores, warm tree, 2026-09-27): a kt-model edit cascades in 61 s
(`cargo test --workspace --no-run`), kt-app + kt-cli 43 s, kt-gpu 10 s.
"""
import json
import os
import re
import subprocess
import sys
import time

SRC = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
DEFAULT = ["build", "-p", "kt-app", "-p", "kt-cli"]


def cargo(args):
    """Run cargo in src/; the workspace crates it compiled, the wall time, the exit code."""
    t0 = time.monotonic()
    out = subprocess.run(["cargo", *args, "-v"], cwd=SRC, capture_output=True, text=True)
    dt = time.monotonic() - t0
    compiled = []
    for line in out.stderr.splitlines():
        m = re.match(r"\s*Compiling (\S+) v\S+ \((/[^)]*)\)", line)
        if m and m.group(2).startswith(os.path.dirname(SRC)):
            compiled.append(m.group(1))
    return sorted(set(compiled)), dt, out.returncode, out.stderr


def measure(path, args):
    full = os.path.join(SRC, path) if not os.path.isabs(path) else path
    # warm first: the measurement starts from a tree where nothing is stale
    _, warm, code, err = cargo(args)
    if code != 0:
        sys.exit(f"plugin-rebuild-check: the warm-up build failed:\n{err[-2000:]}")
    # A REAL edit, not a touch: rustc's incremental cache makes a bare touch nearly free, while
    # a changed item changes the crate's metadata and every dependent is compiled again. One
    # line is appended and removed afterwards (the tree is left as it was, rebuilt once more).
    with open(full, encoding="utf-8") as f:
        original = f.read()
    probe = f"\n#[allow(dead_code)]\nconst KT_REBUILD_PROBE_{int(time.time())}: u32 = 1;\n"
    try:
        with open(full, "w", encoding="utf-8") as f:
            f.write(original + probe)
        compiled, dt, code, err = cargo(args)
    finally:
        with open(full, "w", encoding="utf-8") as f:
            f.write(original)
    if code != 0:
        sys.exit(f"plugin-rebuild-check: the build failed:\n{err[-2000:]}")
    cargo(args)
    return {
        "touched": os.path.relpath(full, SRC),
        "cargo": " ".join(args),
        "seconds": round(dt, 1),
        "compiled": compiled,
        "cores": os.cpu_count(),
    }


def main():
    if len(sys.argv) < 3 or sys.argv[1] != "measure":
        print(__doc__)
        return 2
    args = sys.argv[4:] if len(sys.argv) > 3 and sys.argv[3] == "--" else DEFAULT
    print(json.dumps(measure(sys.argv[2], args), indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
