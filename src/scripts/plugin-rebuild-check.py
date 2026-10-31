#!/usr/bin/env python3
"""What an edit of one effect recompiles, and how long it takes (change plugin-api-stage1, t977).

On a warm tree, make a real one-line edit of a file, run cargo, and report the wall time and the
workspace units cargo compiled. `check` is the acceptance t977: after an edit of a plugin kernel
the recompiled LIBRARIES and BINARIES are exactly the family crate, the assembler
(kt-plugins-builtin), kt-app and kt-cli — no core crate is compiled again; test targets of any
crate may follow (they depend on the set as a dev-dependency, which is the point of the split).

    python3 src/scripts/plugin-rebuild-check.py check [<file>]
        # default file: the pilot's kernel, plugins/fx-video-color/src/lib.rs; exit 1 on a core unit
    python3 src/scripts/plugin-rebuild-check.py measure <file> [-- cargo args…]
        # the measuring form of task 1.1; default cargo args: build -p kt-app -p kt-cli

A unit is told apart by cargo's JSON artifact message (its target kind and the profile's `test`),
not by the "Compiling" line: that line reads the same for a crate's library and for its test
binary, and only the library rebuild is the cascade this change removes. A run that compiled
nothing at all is refused: an empty set proves nothing (the first form of this check parsed
`cargo -v` for env vars it does not print, and passed an edit of kt-fx).

R.68's numbers (32 cores, warm tree, 2026-09-27): a kt-model edit cascades in 61 s
(`cargo test --workspace --no-run`), kt-app + kt-cli 43 s, kt-gpu 10 s. Task 1.1 (cb338e6, before
any kind moved): an edit next to Posterize's tap in kt-fx — 2.0 s build, 79.3 s test --no-run; in
kt-model — 3.7 s, 169.7 s.
"""
import json
import os
import re
import subprocess
import sys
import time

SRC = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
REPO = os.path.dirname(SRC)
DEFAULT = ["build", "-p", "kt-app", "-p", "kt-cli"]
PILOT = "plugins/fx-video-color/src/lib.rs"
# the packages whose library or binary an edit of a plugin kernel may rebuild, besides the family
LEAVES = {"kt-plugins-builtin", "kt-app", "kt-cli"}
BASELINE = {
    "R.68 (2026-09-27), a kt-model edit": (None, 61.0),
    "task 1.1 (cb338e6), an edit in kt-fx": (2.0, 79.3),
    "task 1.1 (cb338e6), an edit in kt-model": (3.7, 169.7),
}


def units(stdout):
    """The workspace units cargo COMPILED (not fresh): (package, target, kind) with kind
    lib/bin/test/build, read from `--message-format=json`'s `compiler-artifact` messages."""
    out = set()
    for line in stdout.splitlines():
        try:
            m = json.loads(line)
        except ValueError:
            continue
        if m.get("reason") != "compiler-artifact" or m.get("fresh", True):
            continue
        pid = m.get("package_id", "")
        # a workspace member is a path package under this repository
        if not pid.startswith("path+file://" + REPO):
            continue
        pkg = re.search(r"#([^@]+)@", pid) or re.search(r"/([^/#]+)#", pid)
        t = m.get("target", {})
        kinds = t.get("kind", [])
        if m.get("profile", {}).get("test") or "test" in kinds:
            kind = "test"
        elif "custom-build" in kinds:
            kind = "build"
        elif "bin" in kinds:
            kind = "bin"
        else:
            kind = "lib"
        out.add((pkg.group(1) if pkg else pid, t.get("name", "?"), kind))
    return sorted(out)


def cargo(args):
    """Run cargo in src/; the workspace units it compiled, the wall time, the exit code."""
    t0 = time.monotonic()
    out = subprocess.run(["cargo", *args, "--message-format=json"], cwd=SRC, capture_output=True, text=True)
    return units(out.stdout), time.monotonic() - t0, out.returncode, out.stderr


def edited(path, runs):
    """Warm every run, make a REAL one-line edit, run them again, restore and rebuild.

    Not a touch: rustc's incremental cache makes a bare touch nearly free, while a changed item
    changes the crate's metadata and every dependent is compiled again. The tree is left as it
    was (rebuilt once more after the line is removed)."""
    full = path if os.path.isabs(path) else os.path.join(SRC, path)
    for args in runs:
        _, _, code, err = cargo(args)
        if code != 0:
            sys.exit(f"plugin-rebuild-check: the warm-up `cargo {' '.join(args)}` failed:\n{err[-2000:]}")
    with open(full, encoding="utf-8") as f:
        original = f.read()
    probe = f"\n#[allow(dead_code)]\nconst KT_REBUILD_PROBE_{int(time.time())}: u32 = 1;\n"
    results = []
    try:
        with open(full, "w", encoding="utf-8") as f:
            f.write(original + probe)
        for args in runs:
            got, dt, code, err = cargo(args)
            if code != 0:
                sys.exit(f"plugin-rebuild-check: `cargo {' '.join(args)}` failed:\n{err[-2000:]}")
            results.append((args, got, dt))
    finally:
        with open(full, "w", encoding="utf-8") as f:
            f.write(original)
    for args in runs:
        cargo(args)
    return os.path.relpath(full, SRC), results


def measure(path, args):
    touched, [(_, got, dt)] = edited(path, [args])
    return {
        "touched": touched,
        "cargo": " ".join(args),
        "seconds": round(dt, 1),
        "compiled": sorted({p for p, _, _ in got}),
        "cores": os.cpu_count(),
    }


def family_of(touched):
    """The package that owns the touched file (its nearest Cargo.toml)."""
    d = os.path.dirname(os.path.join(SRC, touched))
    while d.startswith(SRC):
        toml = os.path.join(d, "Cargo.toml")
        if os.path.exists(toml):
            m = re.search(r'^name\s*=\s*"([^"]+)"', open(toml, encoding="utf-8").read(), re.M)
            return m.group(1) if m else None
        d = os.path.dirname(d)
    return None


def check(path):
    # the load BEFORE the measurement: another build on the machine makes the seconds meaningless
    load = os.getloadavg()[0]
    touched, results = edited(path, [DEFAULT, ["test", "--workspace", "--no-run"]])
    family = family_of(touched)
    allowed = LEAVES | {family}
    bad = []
    print(f"t977: an edit of {touched} (family {family}), {os.cpu_count()} cores, dev profile, warm tree,"
          f" load average {load:.1f} before it (the seconds mean little when it is high)")
    print(f"{'cargo':32} {'seconds':>8}  libraries and binaries compiled again; test targets")
    times = []
    for args, got, dt in results:
        leaves = sorted({p for p, _, k in got if k in ("lib", "bin", "build")})
        tests = sorted({p for p, _, k in got if k == "test"})
        bad += [p for p in leaves if p not in allowed]
        times.append(dt)
        print(f"{' '.join(args):32} {dt:8.1f}  {', '.join(leaves) or '—'}; tests of {len(tests)} packages")
    print("baseline (build / test --no-run):")
    for what, (b, t) in BASELINE.items():
        print(f"  {what:44} {('%.1f s' % b) if b is not None else '—':>8} / {t:.1f} s")
    if not any(got for _, got, _ in results):
        print("t977 RED: cargo reported no compiled unit at all — the measurement saw nothing")
        return 1
    if bad:
        print(f"t977 RED: the edit recompiled core units {sorted(set(bad))} — only {sorted(allowed)} may follow")
        return 1
    print(f"t977 ok: no core library or binary was compiled again ({times[0]:.1f} s / {times[1]:.1f} s)")
    return 0


def main():
    if len(sys.argv) >= 2 and sys.argv[1] == "check":
        return check(sys.argv[2] if len(sys.argv) > 2 else PILOT)
    if len(sys.argv) < 3 or sys.argv[1] != "measure":
        print(__doc__)
        return 2
    args = sys.argv[4:] if len(sys.argv) > 3 and sys.argv[3] == "--" else DEFAULT
    print(json.dumps(measure(sys.argv[2], args), indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
