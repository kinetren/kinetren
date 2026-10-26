//! t834/t835 (github-ci-docker-build): the guard of how Kinetren reaches
//! people — `src/scripts/dist-check.py`. One build container with one pinned
//! toolchain shared with CI, a release script without a recipe of its own,
//! the documented make targets, a read-only CI that keeps nothing but a
//! short-lived failure log, releases only from `v*` tags, `make install` in
//! the README, and the GUI binary called `kinetren` wherever a person meets it.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    // CARGO_MANIFEST_DIR = src/apps/kt-cli
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn guard(root: &Path) -> (bool, String) {
    let out = Command::new("python3")
        .arg(repo().join("src/scripts/dist-check.py"))
        .arg("--check")
        .arg("--root")
        .arg(root)
        .output()
        .expect("python3 for the distribution guard");
    let text = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    (out.status.success(), text)
}

#[test]
fn t834_the_committed_tree_is_consistent() {
    if cfg!(windows) {
        eprintln!("t834 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let (ok, text) = guard(&repo());
    assert!(ok, "distribution guard is red:\n{text}");
}

/// The files the guard reads, copied from the repository as they are.
const FILES: &[&str] = &[
    "src/packaging/Dockerfile",
    "src/scripts/release-build.sh",
    "Makefile",
    ".github/workflows/ci.yml",
    ".github/workflows/release.yml",
    "README.md",
    "src/apps/kt-app/Cargo.toml",
    "src/packaging/kinetren.desktop",
    "src/packaging/install.sh",
    "src/scripts/vendor-ffmpeg-headers.sh",
];

fn copy_tree(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kt-dist-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for f in FILES {
        let to = dir.join(f);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(repo().join(f), &to).unwrap();
    }
    dir
}

fn edit(dir: &Path, file: &str, from: &str, to: &str) {
    let p = dir.join(file);
    let text = std::fs::read_to_string(&p).unwrap();
    assert!(text.contains(from), "{file} no longer contains {from:?}: update the test case");
    std::fs::write(&p, text.replacen(from, to, 1)).unwrap();
}

#[test]
fn t835_guard_catches_each_class_of_drift() {
    if cfg!(windows) {
        eprintln!("t835 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let dir = copy_tree("clean");
    let (ok, text) = guard(&dir);
    assert!(ok, "an unchanged copy must pass:\n{text}");
    let _ = std::fs::remove_dir_all(&dir);

    type Break = fn(&Path);
    let cases: [(&str, Break, &str); 9] = [
        (
            "toolchain",
            |d| edit(d, ".github/workflows/ci.yml", "RUST_TOOLCHAIN: \"", "RUST_TOOLCHAIN: \"9"),
            ".github/workflows/ci.yml: RUST_TOOLCHAIN",
        ),
        (
            "inline-recipe",
            |d| edit(d, "src/scripts/release-build.sh", "set -euo pipefail\n", "set -euo pipefail\ncat <<'EOF'\nFROM ubuntu:22.04\nEOF\n"),
            "src/scripts/release-build.sh: an inline image recipe",
        ),
        (
            "make-target",
            |d| edit(d, "Makefile", "\nclean:", "\ntidy:"),
            "Makefile: target `clean` is missing",
        ),
        (
            "retention",
            |d| edit(d, ".github/workflows/ci.yml", "retention-days: 3", "retention-days: 30"),
            ".github/workflows/ci.yml: every artifact upload",
        ),
        (
            "release-trigger",
            |d| edit(d, ".github/workflows/release.yml", "tags: ['v*']", "branches: [master]"),
            ".github/workflows/release.yml: must run only on pushed tags",
        ),
        (
            "readme",
            |d| edit(d, "README.md", "make install\n", "sudo make install-everything\n"),
            "README.md: the install section must show",
        ),
        (
            "desktop",
            |d| edit(d, "src/packaging/kinetren.desktop", "Exec=kinetren", "Exec=kt-app"),
            "src/packaging/kinetren.desktop: `Exec=kinetren`",
        ),
        (
            "tarball",
            |d| edit(d, "src/scripts/release-build.sh", "BINS=\"kinetren kt-cli\"", "BINS=\"kt-app kt-cli\""),
            "src/scripts/release-build.sh: the tarball must carry",
        ),
        (
            "second-dockerfile",
            |d| std::fs::write(d.join("Dockerfile"), "FROM ubuntu:24.04\n").unwrap(),
            "src/packaging/Dockerfile: exactly one build container",
        ),
    ];
    for (tag, brk, want) in cases {
        let dir = copy_tree(tag);
        brk(&dir);
        let (ok, text) = guard(&dir);
        assert!(!ok, "{tag}: the guard must fail:\n{text}");
        assert!(text.contains(want), "{tag}: expected {want:?} in:\n{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// t860 (windows-build, distribution): the Windows parts of the tree are consistent, and the
/// guard catches each Windows class of drift.
#[test]
fn t860_guard_catches_windows_drift() {
    if cfg!(windows) {
        eprintln!("t860 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let dir = copy_tree("win-clean");
    let (ok, text) = guard(&dir);
    assert!(ok, "an unchanged copy must pass:\n{text}");
    let _ = std::fs::remove_dir_all(&dir);

    type Break = fn(&Path);
    let cases: [(&str, Break, &str); 8] = [
        (
            "stage",
            |d| edit(d, "src/packaging/Dockerfile", "FROM base AS windows", "FROM base AS win"),
            "src/packaging/Dockerfile: no `FROM base AS windows` stage",
        ),
        (
            "last-stage",
            |d| edit(d, "src/packaging/Dockerfile", "\nFROM base AS linux\n", "\n"),
            "src/packaging/Dockerfile: the last stage must be `FROM base AS linux`",
        ),
        (
            "make-windows-test",
            |d| edit(d, "Makefile", "\nwindows-test:", "\nwin-test:"),
            "Makefile: target `windows-test` is missing",
        ),
        (
            "release-zip",
            |d| edit(d, ".github/workflows/release.yml", "release-build.sh --target windows", "release-build.sh"),
            ".github/workflows/release.yml: must build the Windows zip",
        ),
        (
            "ci-job",
            |d| edit(d, ".github/workflows/ci.yml", "\n  windows-test:\n", "\n  win-test:\n"),
            ".github/workflows/ci.yml: no `windows-test` job",
        ),
        (
            "ci-compiles",
            |d| edit(d, ".github/workflows/ci.yml", "      - name: Tests (the cross-built archive, no compilation)\n", "      - name: Build\n        run: cargo build\n\n      - name: Tests (the cross-built archive, no compilation)\n"),
            ".github/workflows/ci.yml: nothing is compiled on the Windows runner",
        ),
        (
            "pe-check",
            |d| edit(d, "src/scripts/release-build.sh", "pe_checks() {", "pe_skipped() {"),
            "src/scripts/release-build.sh: no PE import check",
        ),
        (
            "dll-in-zip",
            |d| edit(d, "src/scripts/release-build.sh", "    cp packaging/README-windows.txt \"$STAGE/\"\n", "    cp packaging/README-windows.txt \"$STAGE/\"\n    cp target/ffmpeg-win/8.1.2/avcodec-62.dll \"$STAGE/\"\n"),
            "src/scripts/release-build.sh: the zip must carry no DLL",
        ),
    ];
    for (tag, brk, want) in cases {
        let dir = copy_tree(&format!("win-{tag}"));
        brk(&dir);
        let (ok, text) = guard(&dir);
        assert!(!ok, "{tag}: the guard must fail:\n{text}");
        assert!(text.contains(want), "{tag}: expected {want:?} in:\n{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// t866 (ffmpeg-9, distribution): the FFmpeg 9 parts of the tree are consistent — the
/// test-only `ffmpeg-test` stage and its hash check, the vendored headers' pin equal to the
/// stage's, `make ffmpeg-test`, the `ffmpeg9` CI job with the expected major — and no release
/// takes from that stage.
#[test]
fn t866_guard_catches_ffmpeg9_drift() {
    if cfg!(windows) {
        eprintln!("t866 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let dir = copy_tree("ff9-clean");
    let (ok, text) = guard(&dir);
    assert!(ok, "an unchanged copy must pass:\n{text}");
    let _ = std::fs::remove_dir_all(&dir);

    type Break = fn(&Path);
    let cases: [(&str, Break, &str); 7] = [
        (
            "stage",
            |d| edit(d, "src/packaging/Dockerfile", "FROM base AS ffmpeg-test", "FROM base AS ff-test"),
            "src/packaging/Dockerfile: no `FROM base AS ffmpeg-test` stage",
        ),
        (
            "hash-check",
            |d| edit(d, "src/packaging/Dockerfile", "/tmp/ffmpeg.tar.xz\" | sha256sum -c", "/tmp/ffmpeg.tar.xz\""),
            "src/packaging/Dockerfile: the ffmpeg-test stage checks the tarball's SHA-256",
        ),
        (
            "headers-pin",
            |d| edit(d, "src/scripts/vendor-ffmpeg-headers.sh", "VERSION=\"${1:-9.0.2}\"", "VERSION=\"${1:-9.0.1}\""),
            "src/scripts/vendor-ffmpeg-headers.sh: the headers' pin must equal",
        ),
        (
            "make-target",
            |d| edit(d, "Makefile", "\nffmpeg-test:", "\nff-test:"),
            "Makefile: target `ffmpeg-test`",
        ),
        (
            "ci-job",
            |d| edit(d, ".github/workflows/ci.yml", "\n  ffmpeg9:\n", "\n  ff9:\n"),
            ".github/workflows/ci.yml: no `ffmpeg9` job",
        ),
        (
            "expected-major",
            |d| edit(d, ".github/workflows/ci.yml", "      KT_EXPECT_FFMPEG_MAJOR: \"9\"\n", ""),
            ".github/workflows/ci.yml: the ffmpeg9 job sets `KT_EXPECT_FFMPEG_MAJOR",
        ),
        (
            "shipped",
            |d| edit(d, "src/scripts/release-build.sh", "set -euo pipefail\n", "set -euo pipefail\n# docker build --target ffmpeg-test\n"),
            "src/scripts/release-build.sh: takes from the ffmpeg-test stage",
        ),
    ];
    for (tag, brk, want) in cases {
        let dir = copy_tree(&format!("ff9-{tag}"));
        brk(&dir);
        let (ok, text) = guard(&dir);
        assert!(!ok, "{tag}: the guard must fail:\n{text}");
        assert!(text.contains(want), "{tag}: expected {want:?} in:\n{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
