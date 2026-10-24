#!/usr/bin/env python3
"""How Kinetren reaches people stays consistent (capability `distribution`).

One build container (src/packaging/Dockerfile) with one pinned toolchain that
CI installs too; release-build.sh builds from that file and carries no recipe
of its own; the root Makefile has the documented targets; the CI workflow is
read-only, cancels superseded runs and uploads nothing that lingers; the
release workflow runs only on `v*` tags with `contents: write` and nothing
else; the README's install section names `make` and `make install`; the GUI
binary is `kinetren` wherever a person meets it. The Windows delivery (change
windows-build): the `windows` stage of the same Dockerfile (and a final `linux`
stage, so a plain build stays Linux), `make windows` / `make windows-test`, the PE
import check and no DLL in the zip, the release attaching the zip, and the CI job
that runs the cross-built archive on windows-latest without compiling there.

The workflows are checked by line patterns, not parsed: the standard library
has no YAML parser, and these are shape guarantees, not a YAML validator.

    dist-check.py --check [--root DIR]

Exit 0 when consistent; otherwise every violation is printed as
`<file>: <what>` and the exit code is 1. Run by `cargo test` (t834, t835 in
src/apps/kt-cli/tests/dist_guard.rs) and by finish.sh.
"""

import argparse
import os
import re
import sys

DOCKERFILE = "src/packaging/Dockerfile"
RELEASE_BUILD = "src/scripts/release-build.sh"
MAKEFILE = "Makefile"
CI = ".github/workflows/ci.yml"
RELEASE = ".github/workflows/release.yml"
README = "README.md"
APP_MANIFEST = "src/apps/kt-app/Cargo.toml"
DESKTOP = "src/packaging/kinetren.desktop"
INSTALL = "src/packaging/install.sh"
MAKE_TARGETS = ["all", "build", "install", "run", "test", "clean", "help"]
SKIP_DIRS = {".git", "target", "node_modules", "vendor"}


class Report:
    def __init__(self, root):
        self.root = root
        self.errors = []

    def read(self, rel):
        try:
            with open(os.path.join(self.root, rel), encoding="utf-8") as f:
                return f.read()
        except OSError:
            self.err(rel, "missing")
            return None

    def err(self, rel, what):
        self.errors.append(f"{rel}: {what}")


def dockerfiles(root):
    found = []
    for d, dirs, files in os.walk(root):
        dirs[:] = [x for x in dirs if x not in SKIP_DIRS]
        for f in files:
            if f == "Dockerfile" or f.startswith("Dockerfile.") or f == "Containerfile":
                found.append(os.path.relpath(os.path.join(d, f), root).replace(os.sep, "/"))
    return sorted(found)


def check_container(r):
    found = dockerfiles(r.root)
    if found != [DOCKERFILE]:
        r.err(DOCKERFILE, f"exactly one build container is allowed, found: {found or 'none'}")
    df = r.read(DOCKERFILE)
    pinned = None
    if df is not None:
        m = re.search(r"^ARG RUST_TOOLCHAIN=(\d+\.\d+\.\d+)\s*$", df, re.M)
        if not m:
            r.err(DOCKERFILE, "no exact `ARG RUST_TOOLCHAIN=x.y.z`")
        else:
            pinned = m.group(1)
        if "$RUST_TOOLCHAIN" not in df:
            r.err(DOCKERFILE, "RUST_TOOLCHAIN is declared but rustup does not install it")
    rb = r.read(RELEASE_BUILD)
    if rb is not None:
        if re.search(r"^\s*FROM\s", rb, re.M):
            r.err(RELEASE_BUILD, "an inline image recipe (`FROM`): build from src/packaging/Dockerfile")
        if "-f packaging/Dockerfile" not in rb:
            r.err(RELEASE_BUILD, "does not build through `-f packaging/Dockerfile`")
        if not re.search(r"^BINS=\"kinetren kt-cli\"", rb, re.M):
            r.err(RELEASE_BUILD, "the tarball must carry `kinetren` and `kt-cli` (BINS)")
        if re.search(r"\bkt-app\b", rb.replace("-p kt-app", "")):
            r.err(RELEASE_BUILD, "names a `kt-app` binary: the GUI is `kinetren` on disk")
    return pinned


def check_make(r):
    mk = r.read(MAKEFILE)
    if mk is None:
        return
    for t in MAKE_TARGETS:
        if not re.search(rf"^{t}:", mk, re.M):
            r.err(MAKEFILE, f"target `{t}` is missing")
    if "release-build.sh" not in mk:
        r.err(MAKEFILE, "`make build` must call src/scripts/release-build.sh")
    if re.search(r"\bkt-app\b", mk):
        r.err(MAKEFILE, "names `kt-app`: the GUI is `kinetren` on disk")


def check_ci(r, pinned):
    ci = r.read(CI)
    if ci is None:
        return
    on = re.search(r"^on:\s*\n((?:[ \t]+.*\n|\s*\n)*)", ci, re.M)
    block = on.group(1) if on else ""
    if not re.search(r"^\s+push:", block, re.M) or not re.search(r"^\s+pull_request:", block, re.M):
        r.err(CI, "must run on both `push` and `pull_request`")
    if re.search(r"^\s+(branches|tags|paths)(-ignore)?:", block, re.M):
        r.err(CI, "must run on every push and pull request (no branch/tag/path filters)")
    if not re.search(r"^permissions:\s*\n\s+contents:\s*read\s*$", ci, re.M):
        r.err(CI, "top-level `permissions: contents: read` is required")
    if re.search(r":\s*write\b", ci):
        r.err(CI, "no write permission in CI")
    if not re.search(r"^concurrency:", ci, re.M) or not re.search(r"cancel-in-progress:\s*true", ci):
        r.err(CI, "a concurrency group with `cancel-in-progress: true` is required")
    m = re.search(r"RUST_TOOLCHAIN:\s*\"?(\d+\.\d+\.\d+)\"?", ci)
    if not m:
        r.err(CI, "no `RUST_TOOLCHAIN: x.y.z` in env")
    elif pinned and m.group(1) != pinned:
        r.err(CI, f"RUST_TOOLCHAIN {m.group(1)} differs from the Dockerfile's {pinned}")
    if not re.search(r"RUSTFLAGS:\s*\"\"", ci):
        r.err(CI, "`RUSTFLAGS: \"\"` is required (the host's mold setting is not on the runner)")
    uploads = len(re.findall(r"uses:\s*actions/upload-artifact@", ci))
    short = [int(x) for x in re.findall(r"retention-days:\s*(\d+)", ci)]
    if uploads != len([d for d in short if d <= 3]) or len(short) != uploads:
        r.err(CI, "every artifact upload needs `retention-days` of 3 or less (only releases are kept)")


def check_release(r):
    rel = r.read(RELEASE)
    if rel is None:
        return
    on = re.search(r"^on:\s*\n((?:[ \t]+.*\n|\s*\n)*)", rel, re.M)
    block = on.group(1) if on else ""
    tags = re.search(r"^\s+tags:\s*(.*)$", block, re.M)
    keys = set(re.findall(r"^  (\w[\w-]*):", block, re.M))
    if keys != {"push"} or not tags or re.sub(r"[\s\[\]'\"]", "", tags.group(1)) != "v*" \
            or re.search(r"^\s+(branches|paths)(-ignore)?:", block, re.M):
        r.err(RELEASE, "must run only on pushed tags `v*`")
    writes = re.findall(r"^\s+([\w-]+):\s*write\s*$", rel, re.M)
    if writes != ["contents"]:
        r.err(RELEASE, f"exactly `contents: write` is allowed, found: {writes or 'none'}")
    if "src/scripts/release-build.sh" not in rel:
        r.err(RELEASE, "must build with src/scripts/release-build.sh")
    if "upload-artifact" in rel:
        r.err(RELEASE, "uploads no artifacts: files go to the release itself")
    if "--verify-tag" not in rel or ".sha256" not in rel:
        r.err(RELEASE, "`gh release create --verify-tag` with the tarball and its .sha256 is required")


def check_windows(r):
    df = r.read(DOCKERFILE)
    if df is not None:
        froms = re.findall(r"^FROM\s+(.*)$", df, re.M)
        if not any(re.match(r"base\s+AS\s+windows\s*$", f) for f in froms):
            r.err(DOCKERFILE, "no `FROM base AS windows` stage (the Windows cross-build)")
        if not froms or not re.match(r"base\s+AS\s+linux\s*$", froms[-1]):
            r.err(DOCKERFILE, "the last stage must be `FROM base AS linux`: a plain build stays Linux")
    mk = r.read(MAKEFILE)
    if mk is not None:
        for t in ["windows", "windows-test"]:
            if not re.search(rf"^{t}:", mk, re.M):
                r.err(MAKEFILE, f"target `{t}` is missing")
    rb = r.read(RELEASE_BUILD)
    if rb is not None:
        if "pe_checks()" not in rb or "x86_64-w64-mingw32-objdump" not in rb:
            r.err(RELEASE_BUILD, "no PE import check (`pe_checks` with mingw objdump): invariant 10 on Windows")
        if re.search(r"^\s*cp\b[^\n]*\.dll", rb, re.M | re.I):
            r.err(RELEASE_BUILD, "the zip must carry no DLL: FFmpeg is found or downloaded, never shipped")
    rel = r.read(RELEASE)
    if rel is not None:
        if "release-build.sh --target windows" not in rel:
            r.err(RELEASE, "must build the Windows zip with `release-build.sh --target windows`")
        if "windows-x86_64.zip" not in rel or "windows-x86_64.zip.sha256" not in rel:
            r.err(RELEASE, "must attach the Windows zip and its .sha256")
    ci = r.read(CI)
    if ci is not None:
        job = re.search(r"^  windows-test:\s*\n((?:    .*\n|\s*\n)*)", ci, re.M)
        if not job:
            r.err(CI, "no `windows-test` job")
        else:
            body = job.group(1)
            if not re.search(r"runs-on:\s*windows-latest", body):
                r.err(CI, "the windows-test job runs on `windows-latest`")
            if not re.search(r"needs:\s*windows-build", body):
                r.err(CI, "the windows-test job needs `windows-build` (the cross-built archive)")
            if "nextest run --archive-file" not in body:
                r.err(CI, "the windows-test job runs `cargo nextest run --archive-file …`")
            if re.search(r"cargo (build|test)\b", body):
                r.err(CI, "nothing is compiled on the Windows runner (one toolchain: the cross-build)")
            if "sha256sum -c" not in body:
                r.err(CI, "the Windows job checks the pinned FFmpeg's SHA-256")


def check_readme(r):
    md = r.read(README)
    if md is None:
        return
    m = re.search(r"^## Install\s*\n(.*?)(?=^## )", md, re.M | re.S)
    if not m:
        r.err(README, "no `## Install` section")
        return
    sec = m.group(1)
    if not re.search(r"^make\s*$", sec, re.M) or not re.search(r"^make install\s*$", sec, re.M):
        r.err(README, "the install section must show `make` and `make install`")
    if re.search(r"\bkt-app\b", sec.replace("-p kt-app", "")):
        r.err(README, "the install section names `kt-app`: the program is `kinetren`")


def check_binary_name(r):
    man = r.read(APP_MANIFEST)
    if man is not None and not re.search(
        r'^\[\[bin\]\]\s*\nname = "kinetren"\s*\npath = "src/main.rs"', man, re.M
    ):
        r.err(APP_MANIFEST, '`[[bin]] name = "kinetren"` is required')
    de = r.read(DESKTOP)
    if de is not None:
        if not re.search(r"^Exec=kinetren(\s|$)", de, re.M):
            r.err(DESKTOP, "`Exec=kinetren` is required")
        if not re.search(r"^StartupWMClass=kinetren\s*$", de, re.M):
            r.err(DESKTOP, "`StartupWMClass=kinetren` is required")
    ins = r.read(INSTALL)
    if ins is not None:
        if not re.search(r"^install -m 755 kinetren kt-cli ", ins, re.M):
            r.err(INSTALL, "must install `kinetren` and `kt-cli`")
        if 'rm -f "$BIN/kt-app"' not in ins:
            r.err(INSTALL, "must remove the `kt-app` an older install left")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", required=True)
    here = os.path.dirname(os.path.abspath(__file__))
    ap.add_argument("--root", default=os.path.normpath(os.path.join(here, "..", "..")))
    args = ap.parse_args()
    r = Report(args.root)
    pinned = check_container(r)
    check_make(r)
    check_ci(r, pinned)
    check_release(r)
    check_windows(r)
    check_readme(r)
    check_binary_name(r)
    if r.errors:
        for e in r.errors:
            print(e, file=sys.stderr)
        print(f"dist-check: {len(r.errors)} violation(s)", file=sys.stderr)
        return 1
    print("dist-check: container, make, workflows, README and binary name agree")
    return 0


if __name__ == "__main__":
    sys.exit(main())
