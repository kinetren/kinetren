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
    "src/packaging/kinetren-mime.xml",
    "src/packaging/install.sh",
    "src/scripts/vendor-ffmpeg-headers.sh",
    "src/scripts/fetch-ffmpeg-win.sh",
    "src/apps/kt-app/src/ffmpeg_setup.rs",
    "src/libs/kt-dirs/src/lib.rs",
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
    let cases: [(&str, Break, &str); 11] = [
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
        // ffmpeg-9: the tests load the build people download — one pin in four places
        (
            "pin-fetch-hash",
            |d| edit(d, "src/scripts/fetch-ffmpeg-win.sh", "SHA256=4d", "SHA256=5d"),
            "src/scripts/fetch-ffmpeg-win.sh: the Windows FFmpeg pin must be",
        ),
        (
            "pin-ci-version",
            |d| edit(d, ".github/workflows/ci.yml", "packages/ffmpeg-9.0.2-full_build", "packages/ffmpeg-9.0.1-full_build"),
            ".github/workflows/ci.yml: the Windows FFmpeg pin must be",
        ),
        (
            "pin-dirs-version",
            |d| edit(d, "src/libs/kt-dirs/src/lib.rs", "FFMPEG_DOWNLOAD_VERSION: &str = \"9.0.2\"", "FFMPEG_DOWNLOAD_VERSION: &str = \"8.1.2\""),
            "src/libs/kt-dirs/src/lib.rs: the Windows FFmpeg pin must be",
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

/// Test 931 (i18n-gaps, D2): the MIME file names our files and only ours.
///
/// `*.kt` is also Kotlin's glob, so the project type carries a magic rule on its root marker;
/// `*.ktproj` is its own legacy type (the former program's format, opened to name the import door);
/// the desktop entry lists exactly the three types. Each class of drift is red on a copy.
#[test]
fn t931_mime_names_our_files_and_only_ours() {
    if cfg!(windows) {
        eprintln!("t931 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    type Break = fn(&Path);
    let cases: [(&str, Break, &str); 4] = [
        (
            "no-magic",
            |d| edit(d, "src/packaging/kinetren-mime.xml", "offset=\"0:64\"", "offset=\"x\""),
            "the magic rule on its root marker",
        ),
        (
            "ktproj-in-project",
            |d| {
                edit(
                    d,
                    "src/packaging/kinetren-mime.xml",
                    "<glob pattern=\"*.kt\"/>",
                    "<glob pattern=\"*.kt\"/>\n    <glob pattern=\"*.ktproj\"/>",
                )
            },
            "`*.ktproj` belongs to the legacy type only",
        ),
        (
            "no-legacy-type",
            |d| edit(d, "src/packaging/kinetren-mime.xml", "<glob pattern=\"*.ktproj\"/>", ""),
            "the legacy type must glob `*.ktproj`",
        ),
        (
            "mimetype-list",
            |d| {
                edit(
                    d,
                    "src/packaging/kinetren.desktop",
                    "application/x-kinotrain-legacy-project;",
                    "",
                )
            },
            "`MimeType=` must list exactly",
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

/// Test 935 (i18n-gaps, D2): the system's MIME database tells our project from Kotlin.
///
/// In a temporary `XDG_DATA_HOME` the MIME file is installed next to the system's own database
/// (which declares `text/x-kotlin` with `*.kt`), `update-mime-database` builds it, and the type of
/// three files is asked: a project written by `project_file::to_text`, a Kotlin source and the
/// legacy fixture. Without `update-mime-database` on the machine the test says why it did not run;
/// the build container installs `shared-mime-info`, so CI always runs it.
#[test]
fn t935_the_mime_database_tells_our_project_from_kotlin() {
    let have = |tool: &str| {
        std::process::Command::new("sh")
            .args(["-c", &format!("command -v {tool}")])
            .output()
            .is_ok_and(|o| o.status.success())
    };
    if cfg!(windows) || !have("update-mime-database") || !have("xdg-mime") {
        eprintln!("t935 not run: update-mime-database / xdg-mime are not on this machine");
        return;
    }
    let dir = std::env::temp_dir().join(format!("kt-mime-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let data = dir.join("data");
    let packages = data.join("mime/packages");
    std::fs::create_dir_all(&packages).unwrap();
    std::fs::copy(repo().join("src/packaging/kinetren-mime.xml"), packages.join("kinetren-mime.xml"))
        .unwrap();
    // the system's own definitions (Kotlin's `*.kt` among them) are part of the question
    let freedesktop = Path::new("/usr/share/mime/packages/freedesktop.org.xml");
    if freedesktop.exists() {
        std::fs::copy(freedesktop, packages.join("freedesktop.org.xml")).unwrap();
    }
    let st = std::process::Command::new("update-mime-database")
        .arg(data.join("mime"))
        .env("XDG_DATA_HOME", &data)
        .env("XDG_DATA_DIRS", &data)
        .status()
        .unwrap();
    assert!(st.success(), "update-mime-database failed");
    let files = dir.join("files");
    std::fs::create_dir_all(&files).unwrap();
    let p = kt_model::Project::new(kt_model::ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    std::fs::write(files.join("film.kt"), kt_model::project_file::to_text(&p, None)).unwrap();
    std::fs::write(files.join("Main.kt"), "package demo\n\nfun main() {\n    println(\"hi\")\n}\n")
        .unwrap();
    std::fs::copy(repo().join("src/fixtures/demo.ktproj"), files.join("demo.ktproj")).unwrap();
    let ask = |f: &str| -> String {
        let out = std::process::Command::new("xdg-mime")
            .args(["query", "filetype"])
            .arg(files.join(f))
            .env("XDG_DATA_HOME", &data)
            .env("XDG_DATA_DIRS", &data)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };
    assert_eq!(ask("film.kt"), "application/x-kinetren-project", "our project");
    if freedesktop.exists() {
        assert_eq!(ask("Main.kt"), "text/x-kotlin", "a Kotlin source is not ours");
    }
    assert_eq!(ask("demo.ktproj"), "application/x-kinotrain-legacy-project", "the older format");
    let _ = std::fs::remove_dir_all(&dir);
}
