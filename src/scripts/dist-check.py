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
that runs the cross-built archive on windows-latest without compiling there. FFmpeg 9
on Linux (change ffmpeg-9): the test-only `ffmpeg-test` stage with its pinned source
tarball (the same pin as the vendored headers), `make ffmpeg-test` and the `ffmpeg9`
CI job with the expected major, and no release taking from that stage.

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
# One application ID (change flatpak-flathub, D1): the kt-app constant names every desktop file.
APP_ID_SRC = "src/apps/kt-app/src/app.rs"
APP_ID_RE = r'^pub const APP_ID: &str = "([^"]+)";'
ICON_SIZES = [16, 22, 24, 32, 48, 64, 128, 256]
# the names an older install left under the former desktop names (install.sh removes them)
FORMER_DESKTOP_NAMES = ["kinetren.desktop", "kinetren.svg", "kinetren.png", "kinetren-mime.xml"]
# set by `main` from the constant: the desktop file and the MIME package are named by the ID
DESKTOP = None
MIME = None
MIME_TYPES = [
    "application/x-opentimelineio",
    "application/x-kinetren-project",
    "application/x-kinotrain-legacy-project",
]
INSTALL = "src/packaging/install.sh"
UI_PROBE = "src/scripts/ui-probe.py"
RUNTIME_FFMPEG = "src/packaging/flatpak/runtime-ffmpeg.txt"
LICENSE = "LICENSE"
FLATPAK_GUIDE = "docs/flatpak.md"
# the metainfo's languages besides English (design Open Question 3: English + Russian until a speaker
# reviews es / zh-Hans / ar — Flathub refuses machine translations)
METAINFO_LANGS = ["ru"]
LOADER_MAJORS = "src/libs/kt-ffmpeg/src/search.rs"
CARGO_SOURCES = "src/scripts/flatpak-cargo-sources.py"
# finish-args (change flatpak-flathub, D5): exactly these, each with its reason comment
FINISH_ARGS = [
    "--share=ipc",
    "--socket=wayland",
    "--socket=fallback-x11",
    "--socket=pulseaudio",
    "--device=dri",
    "--share=network",
    "--filesystem=host",
]
FORBIDDEN_ARGS = [
    "--device=all",
    "--socket=x11",
    "--filesystem=host-os",
    "--filesystem=host-etc",
    "--talk-name=org.freedesktop.Flatpak",
    "--socket=session-bus",
]
VENDOR_HEADERS = "src/scripts/vendor-ffmpeg-headers.sh"
FETCH_WIN = "src/scripts/fetch-ffmpeg-win.sh"
FFMPEG_SETUP = "src/apps/kt-app/src/ffmpeg_setup.rs"
KT_DIRS = "src/libs/kt-dirs/src/lib.rs"
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
        # the GUI compiles in files from outside src/ (docs/guide/*.md): the build container must
        # see the repository root, or every release build fails (2026-10-31)
        if '-v "$PWD/..:/work" -w /work/src' not in rb:
            r.err(RELEASE_BUILD, "the build container must mount the repository root "
                  "(`-v \"$PWD/..:/work\" -w /work/src`): kt-app compiles in docs/guide/*.md")
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
            check_windows_pin(r, body)


def check_windows_pin(r, ci_job):
    """One Windows FFmpeg pin (ffmpeg-9, t860): the version and SHA-256 of the program's
    Download button (`ffmpeg_setup.rs`, the folder name in kt-dirs), of the tests' fetch script
    and of the windows-latest job are one — the tests must load the build people download."""
    pins = {}
    v = re.search(r"ffmpeg-(\d+\.\d+(?:\.\d+)?)-full_build-shared\.7z", ci_job)
    h = re.search(r'echo "([0-9a-f]{64})  ffmpeg\.7z"', ci_job)
    pins[CI] = (v.group(1) if v else None, h.group(1) if h else None)
    fetch = r.read(FETCH_WIN)
    if fetch is not None:
        v = re.search(r"^VERSION=(\S+)\s*$", fetch, re.M)
        h = re.search(r"^SHA256=([0-9a-f]{64})\s*$", fetch, re.M)
        pins[FETCH_WIN] = (v.group(1) if v else None, h.group(1) if h else None)
    setup = r.read(FFMPEG_SETUP)
    if setup is not None:
        v = re.search(r'PINNED_ARCHIVE: &str = "ffmpeg-([^"]+)-full_build-shared\.7z"', setup)
        h = re.search(r'PINNED_SHA256: &str = "([0-9a-f]{64})"', setup)
        pins[FFMPEG_SETUP] = (v.group(1) if v else None, h.group(1) if h else None)
    dirs = r.read(KT_DIRS)
    if dirs is not None:
        v = re.search(r'FFMPEG_DOWNLOAD_VERSION: &str = "([^"]+)"', dirs)
        pins[KT_DIRS] = (v.group(1) if v else None, None)
    want = pins.get(FFMPEG_SETUP)
    if not want or None in want:
        r.err(FFMPEG_SETUP, "no `PINNED_ARCHIVE`/`PINNED_SHA256` of the Download button")
        return
    for rel, (ver, sha) in pins.items():
        if ver != want[0] or (sha is not None and sha != want[1]) or (rel != KT_DIRS and sha is None):
            r.err(rel, f"the Windows FFmpeg pin must be {want[0]} / {want[1][:12]}… (the Download button's), found {ver} / {(sha or '-')[:12]}")


def check_ffmpeg_test(r):
    """FFmpeg 9 on Linux (change ffmpeg-9, t866): the test-only `ffmpeg-test` stage builds the
    pinned source tarball, the vendored headers come from the same pin, `make ffmpeg-test` and
    the `ffmpeg9` CI job run it with the expected major, and no release takes from it."""
    df = r.read(DOCKERFILE)
    pin = None
    if df is not None:
        m = re.search(r"^FROM base AS ffmpeg-test\s*\n(.*?)(?=^FROM\s)", df, re.M | re.S)
        if not m:
            r.err(DOCKERFILE, "no `FROM base AS ffmpeg-test` stage (FFmpeg 9 for the Linux tests)")
        else:
            stage = m.group(1)
            v = re.search(r"^ARG FFMPEG_VERSION=(\S+)\s*$", stage, re.M)
            h = re.search(r"^ARG FFMPEG_SHA256=([0-9a-f]{64})\s*$", stage, re.M)
            if not v or not h:
                r.err(DOCKERFILE, "the ffmpeg-test stage pins `ARG FFMPEG_VERSION` and `ARG FFMPEG_SHA256`")
            else:
                pin = (v.group(1), h.group(1))
            if "sha256sum -c" not in stage:
                r.err(DOCKERFILE, "the ffmpeg-test stage checks the tarball's SHA-256")
    vendor = r.read(VENDOR_HEADERS)
    if vendor is not None and pin is not None:
        v = re.search(r'^VERSION="\$\{1:-([^}]+)\}"', vendor, re.M)
        h = re.search(r'^SHA256="\$\{2:-([0-9a-f]{64})\}"', vendor, re.M)
        if not v or not h or (v.group(1), h.group(1)) != pin:
            r.err(VENDOR_HEADERS, f"the headers' pin must equal the ffmpeg-test stage's {pin}")
    mk = r.read(MAKEFILE)
    if mk is not None and not re.search(r"^ffmpeg-test:.*\n\t.*scripts/ffmpeg-test\.sh", mk, re.M):
        r.err(MAKEFILE, "target `ffmpeg-test` calling src/scripts/ffmpeg-test.sh is missing")
    ci = r.read(CI)
    if ci is not None:
        job = re.search(r"^  ffmpeg9:\s*\n((?:    .*\n|\s*\n)*)", ci, re.M)
        if not job:
            r.err(CI, "no `ffmpeg9` job (the FFmpeg tests against FFmpeg 9)")
        else:
            body = job.group(1)
            if not re.search(r"KT_EXPECT_FFMPEG_MAJOR:\s*\"?9\"?", body):
                r.err(CI, "the ffmpeg9 job sets `KT_EXPECT_FFMPEG_MAJOR: \"9\"` (t865)")
            if "src/scripts/ffmpeg-test.sh" not in body:
                r.err(CI, "the ffmpeg9 job runs src/scripts/ffmpeg-test.sh")
    rb = r.read(RELEASE_BUILD)
    if rb is not None and "ffmpeg-test" in rb:
        r.err(RELEASE_BUILD, "takes from the ffmpeg-test stage: its FFmpeg is for tests, never shipped")


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
    ins = r.read(INSTALL)
    if ins is not None:
        if not re.search(r"^install -m 755 kinetren kt-cli ", ins, re.M):
            r.err(INSTALL, "must install `kinetren` and `kt-cli`")
        if 'rm -f "$BIN/kt-app"' not in ins:
            r.err(INSTALL, "must remove the `kt-app` an older install left")


def read_app_id(r):
    """The application ID from the kt-app constant; `None` (with the error) when it is missing."""
    src = r.read(APP_ID_SRC)
    m = re.search(APP_ID_RE, src or "", re.M)
    if not m:
        r.err(APP_ID_SRC, "`pub const APP_ID: &str = \"…\";` is required (the one application ID)")
        return None
    app_id = m.group(1)
    if not re.fullmatch(r"[A-Za-z][A-Za-z0-9_]*(\.[A-Za-z][A-Za-z0-9_-]*){2,4}", app_id):
        r.err(APP_ID_SRC, f"APP_ID {app_id!r} is not a reverse-DNS ID of 3-5 parts")
    return app_id


def check_app_id(r, app_id):
    """Every copy of the ID is the same string (change flatpak-flathub, t966): the desktop file's name,
    `Icon=` and `StartupWMClass=`, the icon and MIME package file names, what install.sh and
    release-build.sh install and pack, and — once they exist — the Flatpak manifest and the metainfo."""
    de = r.read(DESKTOP)
    if de is not None:
        for key in ["Icon", "StartupWMClass"]:
            m = re.search(rf"^{key}=(.*)$", de, re.M)
            got = m.group(1).strip() if m else "(none)"
            if got != app_id:
                r.err(DESKTOP, f"`{key}={got}` differs from APP_ID `{app_id}` ({APP_ID_SRC})")
    for rel in [f"src/packaging/{app_id}.svg"] + [f"src/packaging/icons/{s}/{app_id}.png" for s in ICON_SIZES]:
        if not os.path.exists(os.path.join(r.root, rel)):
            r.err(rel, f"missing: the icon is named by APP_ID `{app_id}`")
    ins = r.read(INSTALL)
    if ins is not None:
        # the names are written once through `ID=<app id>` and `$ID.<ext>`, or literally
        via_var = re.search(rf"^ID={re.escape(app_id)}\s*$", ins, re.M) is not None
        for name in [f"{app_id}.desktop", f"{app_id}.svg", f"{app_id}.png", f"{app_id}.xml"]:
            ext = name[len(app_id):]
            if name not in ins and not (via_var and f"$ID{ext}" in ins):
                r.err(INSTALL, f"must install `{name}` (named by APP_ID `{app_id}`)")
        for former in FORMER_DESKTOP_NAMES:
            if not re.search(rf"^\s*rm -f .*{re.escape(former)}", ins, re.M):
                r.err(INSTALL, f"must remove `{former}` an older install left under the former desktop name")
    rb = r.read(RELEASE_BUILD)
    if rb is not None:
        for name in [f"packaging/{app_id}.desktop", f"packaging/{app_id}.svg", f"packaging/{app_id}.xml"]:
            if name not in rb:
                r.err(RELEASE_BUILD, f"the tarball must carry `{name}` (named by APP_ID `{app_id}`)")
    # the live rig finds the window by its X11 class, which is the ID (a stale copy made every
    # live scenario report "the kt-app window did not appear")
    probe = r.read(UI_PROBE)
    if probe is not None:
        m = re.search(r'^WM_CLASS = "([^"]*)"', probe, re.M)
        got = m.group(1) if m else "(none)"
        if got != app_id:
            r.err(UI_PROBE, f"`WM_CLASS = \"{got}\"` differs from APP_ID `{app_id}`")
    manifest = f"src/packaging/flatpak/{app_id}.yml"
    flatpak_dir = os.path.join(r.root, "src/packaging/flatpak")
    if os.path.isdir(flatpak_dir):
        for f in os.listdir(flatpak_dir):
            if f.endswith(".yml") and f != f"{app_id}.yml":
                r.err(f"src/packaging/flatpak/{f}", f"a manifest named `{f}` differs from APP_ID `{app_id}`")
        text = r.read(manifest)
        if text is not None:
            m = re.search(r"^id:\s*(\S+)", text, re.M)
            got = m.group(1) if m else "(none)"
            if got != app_id:
                r.err(manifest, f"`id: {got}` differs from APP_ID `{app_id}`")
    meta = f"src/packaging/{app_id}.metainfo.xml"
    meta_files = [f for f in os.listdir(os.path.join(r.root, "src/packaging")) if f.endswith(".metainfo.xml")] \
        if os.path.isdir(os.path.join(r.root, "src/packaging")) else []
    for f in meta_files:
        if f != f"{app_id}.metainfo.xml":
            r.err(f"src/packaging/{f}", f"a metainfo named `{f}` differs from APP_ID `{app_id}`")
    if meta_files:
        text = r.read(meta) or ""
        m = re.search(r"<id>([^<]+)</id>", text)
        got = m.group(1) if m else "(none)"
        if got != app_id:
            r.err(meta, f"`<id>{got}</id>` differs from APP_ID `{app_id}`")
        m = re.search(r'<launchable type="desktop-id">([^<]+)</launchable>', text)
        got = m.group(1) if m else "(none)"
        if got != f"{app_id}.desktop":
            r.err(meta, f"`<launchable>{got}</launchable>` differs from `{app_id}.desktop`")


def check_flatpak(r, app_id):
    """The manifest's shape (change flatpak-flathub, t967): the runtime branch maps to an FFmpeg major
    the loader supports, no `ffmpeg-full` on >= 25.08, exactly the finish-args of D5 with a reason
    each, no module shipping FFmpeg, the version and telemetry environment, the `ldd` check."""
    manifest = f"src/packaging/flatpak/{app_id}.yml"
    text = r.read(manifest)
    if text is None:
        return
    lines = text.splitlines()
    def field(key):
        m = re.search(rf"^{key}:\s*'?([^'\s]+)'?\s*$", text, re.M)
        return m.group(1) if m else None
    if field("runtime") != "org.freedesktop.Platform" or field("sdk") != "org.freedesktop.Sdk":
        r.err(manifest, "runtime/sdk must be org.freedesktop.Platform/org.freedesktop.Sdk")
    branch = field("runtime-version")
    table = {}
    for line in (r.read(RUNTIME_FFMPEG) or "").splitlines():
        cols = line.split()
        if len(cols) >= 3 and not line.startswith("#"):
            table[cols[0]] = int(cols[1])
    supported = [int(m) for m in re.findall(r"^\s*\((\d+), Majors", r.read(LOADER_MAJORS) or "", re.M)]
    if branch not in table:
        r.err(manifest, f"runtime-version {branch} has no row in {RUNTIME_FFMPEG} (branch -> FFmpeg major)")
    elif table[branch] not in supported:
        r.err(manifest, f"runtime-version {branch} has FFmpeg {table[branch]}, the loader supports {supported}")
    try:
        newer = branch is not None and tuple(int(x) for x in branch.split(".")) >= (25, 8)
    except ValueError:
        newer = False
    # the rules read the manifest without its comments (a comment may name what is forbidden)
    code = "\n".join(l for l in lines if not l.strip().startswith("#"))
    if newer and "ffmpeg-full" in code:
        r.err(manifest, f"`ffmpeg-full` on runtime {branch}: codecs-extra replaced it in 25.08")
    # finish-args: the block, each item preceded by a comment
    start = next((i for i, l in enumerate(lines) if l.startswith("finish-args:")), None)
    args = []
    if start is None:
        r.err(manifest, "no finish-args")
    else:
        prev_comment = False
        for l in lines[start + 1:]:
            if l and not l.startswith(" "):
                break
            t = l.strip()
            if t.startswith("#"):
                prev_comment = True
                continue
            if t.startswith("- "):
                arg = t[2:].strip()
                args.append(arg)
                if not prev_comment:
                    r.err(manifest, f"finish-args `{arg}` has no reason comment above it")
                prev_comment = False
    for a in args:
        if a in FORBIDDEN_ARGS:
            r.err(manifest, f"finish-args `{a}` is forbidden (design D5)")
        elif a not in FINISH_ARGS and not a.startswith("--env=KINETREN_FFMPEG_DIR="):
            r.err(manifest, f"finish-args `{a}` is not one of design D5")
    for a in FINISH_ARGS:
        if a not in args:
            r.err(manifest, f"finish-args lack `{a}`")
    if sum(a.startswith("--env=") for a in args) > 1:
        r.err(manifest, "at most one `--env=KINETREN_FFMPEG_DIR=…`")
    # invariant 10: nothing ships FFmpeg
    if re.search(r"^\s*-?\s*name:\s*\S*(ffmpeg|libav)", code, re.M | re.I) or re.search(
        r"^\s*(url|path):\s*\S*(ffmpeg|libav)", code, re.M | re.I
    ):
        r.err(manifest, "a module ships FFmpeg (invariant 10: the runtime's is loaded at run time)")
    for key in ["KT_VERSION_FULL", "KT_VERSION_SHORT"]:
        if not re.search(rf"^\s+{key}:\s*kt\d\d\.\d\d", text, re.M):
            r.err(manifest, f"the version environment `{key}` is missing (the build would say `nogit`)")
    if "KINETREN_POSTHOG_KEY" not in text or "src/packaging/posthog-key" not in text:
        r.err(manifest, "the telemetry key is not taken from src/packaging/posthog-key")
    if "ldd /app/bin/kinetren | grep -q libav" not in text:
        r.err(manifest, "the post-install `ldd` check of invariant 10 is missing")
    if "cargo-sources.json" not in text or "--offline" not in text:
        r.err(manifest, "the build must be offline over cargo-sources.json")


def check_metainfo(r, app_id):
    """The listing is complete and the guide names its commands (change flatpak-flathub, t969)."""
    meta = f"src/packaging/{app_id}.metainfo.xml"
    text = r.read(meta)
    if text is None:
        return
    if not re.search(r"<summary>[^<]+</summary>", text) or not re.search(r"<description>\s*<p>", text):
        r.err(meta, "the English summary and description are required")
    for lang in METAINFO_LANGS:
        if not re.search(rf'<summary xml:lang="{lang}">[^<]+</summary>', text):
            r.err(meta, f"the {lang} summary is missing")
        if not re.search(rf'<p xml:lang="{lang}">', text):
            r.err(meta, f"the {lang} description is missing")
    title = (r.read(LICENSE) or "").splitlines()[:1]
    title = title[0].strip() if title else ""
    lic = re.search(r"<project_license>([^<]+)</project_license>", text)
    if not lic or not lic.group(1).startswith("LicenseRef-proprietary=") or not lic.group(1).endswith("/LICENSE"):
        r.err(meta, "`<project_license>` must be LicenseRef-proprietary= the LICENSE file")
    if title and title not in text:
        r.err(meta, f"the description does not name the licence `{title}` (LICENSE)")
    shots = re.findall(r"<image>([^<]+)</image>", text)
    if len(shots) < 3:
        r.err(meta, f"at least three screenshots are required, there are {len(shots)}")
    for u in shots:
        if not u.startswith("https://") or not re.search(r"/v\d\d\.\d\d\.\d\d(\.\d+)?/", u):
            r.err(meta, f"the screenshot `{u}` is not an HTTPS URL of a release tag")
    rel = re.search(r'<release version="([^"]+)"', text)
    if not rel or not re.fullmatch(r"\d\d\.\d\d\.\d\d(\.\d+)?", rel.group(1)):
        got = rel.group(1) if rel else "(none)"
        r.err(meta, f"the newest <release> version `{got}` is not of the release tag form YY.MM.DD")
    if 'content_rating type="oars-1.1"' not in text or "social-info" not in text:
        r.err(meta, "the OARS rating must declare the anonymous usage counter (social-info)")
    if not re.search(rf'<url type="help">[^<]*{re.escape(FLATPAK_GUIDE)}</url>', text):
        r.err(meta, f"the listing does not link the GPU guide {FLATPAK_GUIDE}")
    guide = r.read(FLATPAK_GUIDE)
    if guide is not None:
        for cmd in [
            f"flatpak run --command=kt-cli {app_id} ffmpeg --probe-encoders",
            f"flatpak override --user --env=LIBVA_DRIVER_NAME=",
            f"flatpak override --user --device=all {app_id}",
            f"flatpak override --user --reset {app_id}",
        ]:
            if cmd not in guide:
                r.err(FLATPAK_GUIDE, f"the guide lacks `{cmd}`")


FLATPAK_BUILD = "src/scripts/flatpak-build.sh"
COMPOSE_CHECK = "src/scripts/appstream-compose-check.sh"
# Flathub's screenshot mirror, named once in each of the two scripts (ci-flatpak-screenshots, t1410)
MEDIA_LINE = "MEDIA_URL=https://dl.flathub.org/media\n"
FLATPAK_PPA = "ppa:flatpak/stable"


def check_flatpak_ci(r):
    """The Flatpak job and the release bundle (change flatpak-flathub, t971): the CI job `flatpak`
    builds, lints, validates the metainfo and checks the sandbox, uploads nothing; the release
    attaches the bundle and its .sha256; `make flatpak` runs the same script."""
    ci = r.read(CI) or ""
    job = re.search(r"^  flatpak:\s*\n((?:    .*\n|\s*\n)*)", ci, re.M)
    if not job:
        r.err(CI, "no `flatpak` job")
    else:
        body = job.group(1)
        if "src/scripts/flatpak-build.sh" not in body:
            r.err(CI, "the flatpak job must run src/scripts/flatpak-build.sh")
        if "upload-artifact" in body or "gh release" in body:
            r.err(CI, "the flatpak job uploads nothing")
    if not (re.search(r"^on:\s*\n\s+push:", ci, re.M) and "pull_request:" in ci):
        r.err(CI, "CI must run on push and pull_request")
    build = r.read(FLATPAK_BUILD) or ""
    for piece, what in [
        ("flatpak-builder --user --install-deps-from=flathub", "the build"),
        ("flatpak-builder-lint org.flatpak.Builder manifest", "the linter on the manifest"),
        ("flatpak-builder-lint org.flatpak.Builder repo", "the linter on the repository"),
        ("appstreamcli validate", "the metainfo validation"),
        ("src/scripts/flatpak-check.sh", "the sandbox check"),
    ]:
        if piece not in build:
            r.err(FLATPAK_BUILD, f"{what} is missing (`{piece}`)")
    # the metadata compose runs in seconds BEFORE the ten-minute build (ci-red-again, t1380): CI of
    # 9e1f1a45 learnt of a host without the SVG loader only after the build
    pre, full = build.find(f"{COMPOSE_CHECK}\n"), build.find("flatpak-builder --user")
    if pre < 0 or (full >= 0 and pre > full):
        r.err(FLATPAK_BUILD, f"the compose pre-flight `{COMPOSE_CHECK}` must run before flatpak-builder")
    compose = r.read(COMPOSE_CHECK)  # "missing" when it is gone
    # the screenshots are mirrored to Flathub's media URL and committed as `screenshots/<arch>`, or
    # flatpak-builder-lint refuses the repository after the whole build; the pre-flight composes
    # them the same way, so a missing WebP loader fails in seconds (change ci-flatpak-screenshots,
    # t1410). flatpak-builder 1.4.6+ has the URL policy; Ubuntu 24.04 ships 1.4.2.
    for text, rel, pieces in [
        (build, FLATPAK_BUILD, [MEDIA_LINE, '--mirror-screenshots-url="$MEDIA_URL"', "--compose-url-policy=full"]),
        (compose or "", COMPOSE_CHECK, [MEDIA_LINE, '--media-baseurl="$MEDIA_URL"', "--no-partial-urls"]),
    ]:
        if compose is None and rel == COMPOSE_CHECK:
            continue
        for piece in pieces:
            if piece not in text:
                r.err(rel, f"the screenshots must be mirrored as Flathub's linter wants (`{piece.strip()}`)")
    for rel in [CI, RELEASE]:
        text = r.read(rel) or ""
        install = next((m.start() for m in re.finditer(r"apt-get install[^\n]*flatpak-builder", text)), -1)
        ppa = text.find(f"add-apt-repository -y {FLATPAK_PPA}")
        if install >= 0 and (ppa < 0 or ppa > install):
            r.err(rel, f"flatpak-builder comes from `{FLATPAK_PPA}`, added before its install: Ubuntu's "
                  "1.4.2 cannot mirror the screenshots with full URLs (--compose-url-policy, 1.4.6)")
    rel = r.read(RELEASE) or ""
    if "flatpak-build.sh --bundle" not in rel or ".flatpak.sha256" not in rel or "linux-x86_64.flatpak " not in rel:
        r.err(RELEASE, "the release must build and attach kinetren-<version>-linux-x86_64.flatpak and its .sha256")
    mk = r.read(MAKEFILE) or ""
    if not re.search(r"^flatpak:.*\n\t.*flatpak-build\.sh", mk, re.M):
        r.err(MAKEFILE, "target `flatpak` must run flatpak-build.sh")


# A package installed with `--no-install-recommends` loses its Recommends; where a check needs
# one of them, it is named on the same line (change ci-green-again, t1060). Each row is a CI
# failure of 94e36e9e: flatpak-builder strips with `eu-strip` (elfutils), and xdg-mime without a
# desktop answers through `mimetype` (libfile-mimeinfo-perl) — without it libmagic's `file`
# called our project JSON, or nothing answered at all (t935). CI of 9e1f1a45 added two (change
# ci-red-again, t1380): flatpak-builder's `appstreamcli compose` reads the SVG icon through
# gdk-pixbuf, whose SVG loader is librsvg2-common (a Recommends of librsvg2-2), and t935 asks GIO,
# because perl's `mimetype` takes the first of two equal-weight globs without reading the file.
# CI of 21505ad2 added one more (change ci-flatpak-screenshots, t1410): compose reads the WebP
# screenshots through gdk-pixbuf too, whose WebP loader is a package of its own that nothing pulls;
# without it compose dropped all four and Flathub's linter found none.
RECOMMENDS_NEEDED = [
    ("flatpak-builder", "elfutils", "flatpak-builder strips with eu-strip"),
    ("flatpak-builder", "librsvg2-common", "appstreamcli compose reads the SVG icon through gdk-pixbuf's SVG loader"),
    ("flatpak-builder", "webp-pixbuf-loader", "appstreamcli compose reads the WebP screenshots through gdk-pixbuf's WebP loader"),
    ("xdg-utils", "libfile-mimeinfo-perl", "xdg-mime without a desktop asks `mimetype` (t935)"),
    ("shared-mime-info", "libglib2.0-bin", "t935 asks `gio`, as a file manager does"),
]


def apt_installs(text):
    """Every `apt-get install` command of a workflow or a Dockerfile, backslash continuations
    joined and cut at the next `&&`, as a list of words."""
    joined = re.sub(r"\\[ \t]*\n", " ", text)
    out = []
    for m in re.finditer(r"apt-get install([^\n]*)", joined):
        out.append(m.group(1).split("&&")[0].split())
    return out


def check_recommends(r):
    """A package installed without its Recommends names the one a check needs (t1060)."""
    for rel in [CI, RELEASE] + dockerfiles(r.root):
        text = r.read(rel)
        if text is None:
            continue
        for words in apt_installs(text):
            if "--no-install-recommends" not in words:
                continue
            for pkg, need, why in RECOMMENDS_NEEDED:
                if pkg in words and need not in words:
                    r.err(rel, f"`{pkg}` without recommends must install `{need}` too: {why}")


REGISTRY_SCRIPT = "src/scripts/cargo-registry.sh"


def check_registry(r):
    """Every script that mounts the crates.io registry into a container takes it from
    cargo-registry.sh, which hands a fresh volume to the user (t1064): a root-owned volume killed
    the ffmpeg9 CI job of 94e36e9e, whose runner has no ~/.cargo/registry."""
    helper = r.read(REGISTRY_SCRIPT)
    if helper is None:
        r.err(REGISTRY_SCRIPT, "missing: the one body that picks the registry a container mounts")
    elif not re.search(r"-u 0 .*\n?.*chown", helper):
        r.err(REGISTRY_SCRIPT, "a fresh volume must be handed to the user (`docker run -u 0 … chown`)")
    scripts = os.path.join(r.root, "src/scripts")
    for f in sorted(os.listdir(scripts)):
        rel = f"src/scripts/{f}"
        if not f.endswith(".sh") or rel == REGISTRY_SCRIPT:
            continue
        text = r.read(rel) or ""
        if ":/usr/local/cargo/registry" in text and 'REGISTRY="$(scripts/cargo-registry.sh' not in text:
            r.err(rel, "mounts the cargo registry without scripts/cargo-registry.sh (a root-owned volume)")


def check_cargo_sources(r):
    """The offline crate sources follow the lock file (change flatpak-flathub, t968)."""
    import subprocess
    out = subprocess.run(
        [sys.executable, os.path.join(r.root, CARGO_SOURCES), "--check",
         "--lock", os.path.join(r.root, "src/Cargo.lock"),
         "--out", os.path.join(r.root, "src/packaging/flatpak/cargo-sources.json")],
        capture_output=True, text=True,
    )
    if out.returncode != 0:
        for line in (out.stderr or out.stdout).strip().splitlines():
            r.err("src/packaging/flatpak/cargo-sources.json", line)


def check_mime(r):
    """The MIME file names our files and only ours (change i18n-gaps, D2): `*.kt` is shared with
    Kotlin, so the project type carries a magic rule on its root marker; `*.ktproj` is the older
    format's own type; the desktop entry opens exactly our three types."""
    mime = r.read(MIME)
    if mime is None:
        return
    blocks = {}
    for b in mime.split("<mime-type ")[1:]:
        m = re.match(r'type="([^"]+)"', b)
        if m:
            blocks[m.group(1)] = b.split("</mime-type>")[0]
    proj = blocks.get("application/x-kinetren-project", "")
    if '<glob pattern="*.kt"/>' not in proj:
        r.err(MIME, "the project type must glob `*.kt`")
    if not re.search(r'<match type="string" value="&quot;kinetren&quot;" offset="0:\d+"/>', proj):
        r.err(MIME, "the project type needs the magic rule on its root marker \"kinetren\" (Kotlin shares `*.kt`)")
    for ty, b in blocks.items():
        if '<glob pattern="*.ktproj"/>' in b and ty != "application/x-kinotrain-legacy-project":
            r.err(MIME, f"`*.ktproj` belongs to the legacy type only, not {ty}")
    legacy = blocks.get("application/x-kinotrain-legacy-project", "")
    if '<glob pattern="*.ktproj"/>' not in legacy or "Kinotrain" not in legacy:
        r.err(MIME, "the legacy type must glob `*.ktproj` and name the old program (Kinotrain)")
    if '<glob pattern="*.otio"/>' not in blocks.get("application/x-opentimelineio", ""):
        r.err(MIME, "the OpenTimelineIO type must glob `*.otio`")
    de = r.read(DESKTOP)
    if de is not None:
        m = re.search(r"^MimeType=(.*)$", de, re.M)
        listed = [t for t in (m.group(1) if m else "").split(";") if t]
        if sorted(listed) != sorted(MIME_TYPES):
            r.err(DESKTOP, f"`MimeType=` must list exactly {';'.join(MIME_TYPES)}; it lists {listed}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", required=True)
    here = os.path.dirname(os.path.abspath(__file__))
    ap.add_argument("--root", default=os.path.normpath(os.path.join(here, "..", "..")))
    args = ap.parse_args()
    r = Report(args.root)
    global DESKTOP, MIME
    app_id = read_app_id(r) or "kinetren"
    DESKTOP = f"src/packaging/{app_id}.desktop"
    MIME = f"src/packaging/{app_id}.xml"
    pinned = check_container(r)
    check_make(r)
    check_ci(r, pinned)
    check_release(r)
    check_windows(r)
    check_ffmpeg_test(r)
    check_readme(r)
    check_binary_name(r)
    check_mime(r)
    check_app_id(r, app_id)
    check_flatpak(r, app_id)
    check_metainfo(r, app_id)
    check_flatpak_ci(r)
    check_recommends(r)
    check_registry(r)
    check_cargo_sources(r)
    if r.errors:
        for e in r.errors:
            print(e, file=sys.stderr)
        print(f"dist-check: {len(r.errors)} violation(s)", file=sys.stderr)
        return 1
    print("dist-check: container, make, workflows, README and binary name agree")
    return 0


if __name__ == "__main__":
    sys.exit(main())
