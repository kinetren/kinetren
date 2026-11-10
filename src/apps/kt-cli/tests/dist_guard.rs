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
    // the scripts that mount the cargo registry, and the one body they take it from (t1064)
    "src/scripts/cargo-registry.sh",
    "src/scripts/ffmpeg-test.sh",
    "src/scripts/windows-test.sh",
    "Makefile",
    ".github/workflows/ci.yml",
    ".github/workflows/release.yml",
    "README.md",
    "src/apps/kt-app/Cargo.toml",
    "src/packaging/com.kinetren.Kinetren.desktop",
    "src/packaging/com.kinetren.Kinetren.xml",
    "src/packaging/install.sh",
    "src/scripts/vendor-ffmpeg-headers.sh",
    "src/scripts/fetch-ffmpeg-win.sh",
    "src/apps/kt-app/src/ffmpeg_setup.rs",
    "src/libs/kt-dirs/src/lib.rs",
    // the one application ID (flatpak-flathub): the constant and the files it names
    "src/apps/kt-app/src/app.rs",
    "src/packaging/com.kinetren.Kinetren.svg",
    "src/packaging/icons/16/com.kinetren.Kinetren.png",
    "src/packaging/icons/22/com.kinetren.Kinetren.png",
    "src/packaging/icons/24/com.kinetren.Kinetren.png",
    "src/packaging/icons/32/com.kinetren.Kinetren.png",
    "src/packaging/icons/48/com.kinetren.Kinetren.png",
    "src/packaging/icons/64/com.kinetren.Kinetren.png",
    "src/packaging/icons/128/com.kinetren.Kinetren.png",
    "src/packaging/icons/256/com.kinetren.Kinetren.png",
    // the Flatpak (flatpak-flathub): the manifest, the branch table, the loader's majors, the sources
    "src/packaging/flatpak/com.kinetren.Kinetren.yml",
    "src/packaging/flatpak/runtime-ffmpeg.txt",
    "src/libs/kt-ffmpeg/src/search.rs",
    "src/packaging/flatpak/cargo-sources.json",
    "src/Cargo.lock",
    "src/scripts/flatpak-cargo-sources.py",
    "src/packaging/com.kinetren.Kinetren.metainfo.xml",
    "LICENSE",
    "docs/flatpak.md",
    "src/scripts/flatpak-build.sh",
    // the AppStream compose before the build (ci-red-again, t1380, t1381)
    "src/scripts/appstream-compose-check.sh",
    "src/scripts/ui-probe.py",
    // the test order (ci-speedup, t1422): the profile and a file of each test it names
    "src/.config/nextest.toml",
    "src/apps/kt-app/src/util.rs",
    "src/libs/kt-media/tests/index_db.rs",
    "src/apps/kt-app/src/layout_tests.rs",
    "src/apps/kt-cli/tests/cli.rs",
    "src/plugins/fx-video-tape/tests/physics.rs",
    "src/libs/kt-engine/tests/pitch_edges.rs",
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
    let cases: [(&str, Break, &str); 10] = [
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
            |d| edit(d, "src/packaging/com.kinetren.Kinetren.desktop", "Exec=kinetren", "Exec=kt-app"),
            "src/packaging/com.kinetren.Kinetren.desktop: `Exec=kinetren`",
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
        (
            // the GUI compiles in docs/guide/*.md: a container that sees src/ alone fails every
            // release build (2026-10-31)
            "src-only-mount",
            |d| edit(d, "src/scripts/release-build.sh", "-v \"$PWD/..:/work\" -w /work/src", "-v \"$PWD:/work\" -w /work"),
            "src/scripts/release-build.sh: the build container must mount the repository root",
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
            |d| edit(d, "src/packaging/Dockerfile", "FROM base AS windows-cross", "FROM base AS win"),
            "src/packaging/Dockerfile: no `FROM base AS windows-cross` stage",
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
            |d| edit(d, "src/packaging/com.kinetren.Kinetren.xml", "offset=\"0:64\"", "offset=\"x\""),
            "the magic rule on its root marker",
        ),
        (
            "ktproj-in-project",
            |d| {
                edit(
                    d,
                    "src/packaging/com.kinetren.Kinetren.xml",
                    "<glob pattern=\"*.kt\"/>",
                    "<glob pattern=\"*.kt\"/>\n    <glob pattern=\"*.ktproj\"/>",
                )
            },
            "`*.ktproj` belongs to the legacy type only",
        ),
        (
            "no-legacy-type",
            |d| edit(d, "src/packaging/com.kinetren.Kinetren.xml", "<glob pattern=\"*.ktproj\"/>", ""),
            "the legacy type must glob `*.ktproj`",
        ),
        (
            "mimetype-list",
            |d| {
                edit(
                    d,
                    "src/packaging/com.kinetren.Kinetren.desktop",
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

/// Test 935 (i18n-gaps, D2; ci-red-again, D1): the system's MIME database tells our project from
/// Kotlin, asked as a file manager asks it.
///
/// In a temporary `XDG_DATA_HOME` the MIME file is installed next to the system's own database
/// (which declares `text/x-kotlin` with `*.kt`), `update-mime-database` builds it, and GIO — what
/// Nautilus, Nemo, Caja and Thunar ask, and xdg-mime on those desktops — is asked the type of
/// three files: a project written by `project_file::to_text`, a Kotlin source and the legacy
/// fixture. Where perl's `mimetype` exists (xdg-mime without a desktop, so `xdg-open` under a bare
/// window manager), it is asked about our two files only: it takes the first of two equal-weight
/// globs without reading the file, and that order differs between shared-mime-info versions (CI of
/// 9e1f1a45 got our type for `Main.kt`). Without `update-mime-database` or `gio` the test says why
/// it did not run; CI and the build container install both, held by t1380.
#[test]
fn t935_the_mime_database_tells_our_project_from_kotlin() {
    let have = |tool: &str| {
        std::process::Command::new("sh")
            .args(["-c", &format!("command -v {tool}")])
            .output()
            .is_ok_and(|o| o.status.success())
    };
    if cfg!(windows) || !have("update-mime-database") {
        eprintln!("t935 not run: update-mime-database (shared-mime-info) is not on this machine");
        return;
    }
    if !have("gio") {
        eprintln!("t935 not run: no `gio` (libglib2.0-bin) — the oracle a file manager asks");
        return;
    }
    let dir = std::env::temp_dir().join(format!("kt-mime-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let data = dir.join("data");
    let packages = data.join("mime/packages");
    std::fs::create_dir_all(&packages).unwrap();
    std::fs::copy(repo().join("src/packaging/com.kinetren.Kinetren.xml"), packages.join("com.kinetren.Kinetren.xml"))
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
    let gio = |f: &str| -> String {
        let out = std::process::Command::new("gio")
            .args(["info", "-a", "standard::content-type"])
            .arg(files.join(f))
            .env("XDG_DATA_HOME", &data)
            .env("XDG_DATA_DIRS", &data)
            // the local file, not a gvfs daemon of the desktop the test may run under
            .env("GIO_USE_VFS", "local")
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        text.lines()
            .find_map(|l| l.trim().strip_prefix("standard::content-type:"))
            .map(|t| t.trim().to_string())
            .unwrap_or_else(|| format!("(no answer: {text:?} {:?})", String::from_utf8_lossy(&out.stderr)))
    };
    assert_eq!(gio("film.kt"), "application/x-kinetren-project", "GIO: our project");
    if freedesktop.exists() {
        assert_eq!(gio("Main.kt"), "text/x-kotlin", "GIO: a Kotlin source is not ours");
    }
    assert_eq!(gio("demo.ktproj"), "application/x-kinotrain-legacy-project", "GIO: the older format");
    // perl's File::MimeInfo reads the priority-80 magic before the globs: our files stay ours
    if have("mimetype") {
        let perl = |f: &str| -> String {
            let out = std::process::Command::new("mimetype")
                .args(["--brief", "--dereference"])
                .arg(files.join(f))
                .env("XDG_DATA_HOME", &data)
                .env("XDG_DATA_DIRS", &data)
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        assert_eq!(perl("film.kt"), "application/x-kinetren-project", "mimetype: our project");
        assert_eq!(perl("demo.ktproj"), "application/x-kinotrain-legacy-project", "mimetype: the older format");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// t1060 (ci-green-again): a package installed without its Recommends names the one a check
/// needs, in every workflow and Dockerfile.
///
/// CI of 94e36e9e lost two jobs to `--no-install-recommends`: the Flatpak build had no `eu-strip`
/// (elfutils, a Recommends of flatpak-builder) and t935 got `application/json` for our project,
/// because xdg-mime without a desktop asks `mimetype` (libfile-mimeinfo-perl, a Recommends of
/// xdg-utils) and fell back to libmagic. Each break is one of those lines losing its package.
#[test]
fn t1060_packages_without_recommends_name_what_the_checks_need() {
    if cfg!(windows) {
        eprintln!("t1060 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let cases: [Break; 4] = [
        (
            "ci-elfutils",
            |d| edit(d, ".github/workflows/ci.yml", "flatpak-builder appstream elfutils", "flatpak-builder appstream"),
            "`flatpak-builder` without recommends must install `elfutils`",
        ),
        (
            "release-elfutils",
            |d| edit(d, ".github/workflows/release.yml", "flatpak-builder appstream elfutils", "flatpak-builder appstream"),
            "release.yml: `flatpak-builder` without recommends must install `elfutils`",
        ),
        (
            "ci-mimetype",
            |d| edit(d, ".github/workflows/ci.yml", "xdg-utils libfile-mimeinfo-perl", "xdg-utils"),
            "ci.yml: `xdg-utils` without recommends must install `libfile-mimeinfo-perl`",
        ),
        (
            "container-mimetype",
            |d| edit(d, "src/packaging/Dockerfile", "xdg-utils libfile-mimeinfo-perl", "xdg-utils"),
            "Dockerfile: `xdg-utils` without recommends must install `libfile-mimeinfo-perl`",
        ),
    ];
    expect_red(&cases);
}

/// t1380 (ci-red-again): the SVG loader compose needs, the MIME oracle t935 asks and the compose
/// pre-flight are named.
///
/// CI of 9e1f1a45: flatpak-builder's `appstreamcli compose` could not read our SVG icon (gdk-pixbuf's
/// SVG loader is librsvg2-common, a Recommends of librsvg2-2), and t935's xdg-mime asked perl's
/// `mimetype`, which called a Kotlin source ours; t935 asks `gio` now (libglib2.0-bin). The compose
/// learnt of the loader only after a ten-minute build, so `flatpak-build.sh` runs it first.
#[test]
fn t1380_the_svg_loader_the_mime_oracle_and_the_compose_preflight_are_named() {
    if cfg!(windows) {
        eprintln!("t1380 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let cases: [Break; 6] = [
        (
            "ci-rsvg",
            |d| edit(d, ".github/workflows/ci.yml", "elfutils librsvg2-common", "elfutils"),
            "ci.yml: `flatpak-builder` without recommends must install `librsvg2-common`",
        ),
        (
            "release-rsvg",
            |d| edit(d, ".github/workflows/release.yml", "elfutils librsvg2-common", "elfutils"),
            "release.yml: `flatpak-builder` without recommends must install `librsvg2-common`",
        ),
        (
            "ci-gio",
            |d| edit(d, ".github/workflows/ci.yml", "shared-mime-info libglib2.0-bin", "shared-mime-info"),
            "ci.yml: `shared-mime-info` without recommends must install `libglib2.0-bin`",
        ),
        (
            "container-gio",
            |d| edit(d, "src/packaging/Dockerfile", "shared-mime-info libglib2.0-bin", "shared-mime-info"),
            "Dockerfile: `shared-mime-info` without recommends must install `libglib2.0-bin`",
        ),
        (
            "no-preflight",
            |d| edit(d, "src/scripts/flatpak-build.sh", "src/scripts/appstream-compose-check.sh\n", "\n"),
            "the compose pre-flight `src/scripts/appstream-compose-check.sh` must run before flatpak-builder",
        ),
        (
            "no-script",
            |d| std::fs::remove_file(d.join("src/scripts/appstream-compose-check.sh")).unwrap(),
            "src/scripts/appstream-compose-check.sh: missing",
        ),
    ];
    expect_red(&cases);
}

/// `appstream-compose-check.sh` on a tree, its exit and its output.
fn compose_check(root: &Path) -> (bool, String) {
    let out = Command::new("bash")
        .arg(repo().join("src/scripts/appstream-compose-check.sh"))
        .arg(root)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    (out.status.success(), text)
}

/// Whether the compose pre-flight can run here: `appstreamcli compose` is installed and the
/// listing's first screenshot is reachable (compose fetches them); if not, `test` says why it did
/// not run. The Flatpak CI job runs the script through `flatpak-build.sh` either way.
fn compose_can_run(test: &str) -> bool {
    let compose = Command::new("appstreamcli").args(["compose", "--help"]).output();
    if cfg!(windows) || !compose.is_ok_and(|o| o.status.success()) {
        eprintln!("{test} not run: no `appstreamcli compose` (appstream-compose) on this machine");
        return false;
    }
    let metainfo = std::fs::read_to_string(repo().join("src/packaging/com.kinetren.Kinetren.metainfo.xml")).unwrap();
    let url = metainfo.split("<image>").nth(1).and_then(|t| t.split("</image>").next()).unwrap();
    let head = Command::new("curl").args(["-sfIL", "--max-time", "20", "-o", "/dev/null", url]).status();
    if !head.is_ok_and(|s| s.success()) {
        eprintln!("{test} not run: the screenshot {url} is not reachable from here (compose fetches it)");
        return false;
    }
    true
}

/// t1381 (ci-red-again, D3): the AppStream listing composes as flatpak-builder composes it, and a
/// broken icon is caught by the pre-flight, not after the build.
///
/// `appstream-compose-check.sh` installs the manifest's own `/app/share` lines into a temporary
/// folder and runs `appstreamcli compose` with flatpak-builder's arguments. Where compose is
/// not installed (the package appstream-compose; flatpak-builder depends on it) or the screenshots
/// cannot be fetched, the test says why it did not run.
#[test]
fn t1381_the_appstream_listing_composes_before_the_flatpak_build() {
    if !compose_can_run("t1381") {
        return;
    }
    let (ok, text) = compose_check(&repo());
    assert!(ok, "the listing must compose:\n{text}");
    assert!(text.contains("com.kinetren.Kinetren composes"), "{text}");
    let dir = copy_tree("compose-icon");
    std::fs::write(dir.join("src/packaging/com.kinetren.Kinetren.svg"), "not an image\n").unwrap();
    let (ok, text) = compose_check(&dir);
    assert!(!ok, "a broken icon must fail the pre-flight:\n{text}");
    assert!(text.contains("file-read-error"), "{text}");
    assert!(text.contains("com.kinetren.Kinetren.svg"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// t1410 (ci-flatpak-screenshots, D1–D2): the screenshots are mirrored as Flathub's linter wants.
///
/// CI of 21505ad2 built and installed the Flatpak, then `flatpak-builder-lint repo` refused it:
/// `appstream-missing-screenshots` (compose had dropped all four WebP screenshots: gdk-pixbuf had no
/// WebP loader) and `appstream-screenshots-not-mirrored-in-ostree` (flatpak-builder ran without
/// `--mirror-screenshots-url`, so no `screenshots/<arch>` ref). The linter also wants the URLs
/// under `https://dl.flathub.org/media` in full, which `--compose-url-policy=full` gives, and that
/// needs flatpak-builder 1.4.6+ from `ppa:flatpak/stable` (Ubuntu 24.04 ships 1.4.2).
#[test]
fn t1410_the_screenshots_are_mirrored_as_flathubs_linter_wants() {
    if cfg!(windows) {
        eprintln!("t1410 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let cases: [Break; 8] = [
        (
            "ci-webp",
            |d| edit(d, ".github/workflows/ci.yml", "librsvg2-common webp-pixbuf-loader", "librsvg2-common"),
            "ci.yml: `flatpak-builder` without recommends must install `webp-pixbuf-loader`",
        ),
        (
            "release-webp",
            |d| edit(d, ".github/workflows/release.yml", "librsvg2-common webp-pixbuf-loader", "librsvg2-common"),
            "release.yml: `flatpak-builder` without recommends must install `webp-pixbuf-loader`",
        ),
        (
            "ci-ppa",
            |d| edit(d, ".github/workflows/ci.yml", "sudo add-apt-repository -y ppa:flatpak/stable\n", ""),
            "ci.yml: flatpak-builder comes from `ppa:flatpak/stable`",
        ),
        (
            "release-ppa",
            |d| edit(d, ".github/workflows/release.yml", "sudo add-apt-repository -y ppa:flatpak/stable\n", ""),
            "release.yml: flatpak-builder comes from `ppa:flatpak/stable`",
        ),
        (
            "no-mirror",
            |d| edit(d, "src/scripts/flatpak-build.sh", "--mirror-screenshots-url=\"$MEDIA_URL\" ", ""),
            "flatpak-build.sh: the screenshots must be mirrored as Flathub's linter wants (`--mirror-screenshots-url",
        ),
        (
            "partial-urls",
            |d| edit(d, "src/scripts/flatpak-build.sh", " --compose-url-policy=full", ""),
            "flatpak-build.sh: the screenshots must be mirrored as Flathub's linter wants (`--compose-url-policy=full`)",
        ),
        (
            "preflight-no-mirror",
            |d| edit(d, "src/scripts/appstream-compose-check.sh", "--media-baseurl=\"$MEDIA_URL\" ", ""),
            "appstream-compose-check.sh: the screenshots must be mirrored as Flathub's linter wants (`--media-baseurl",
        ),
        (
            "preflight-partial-urls",
            |d| edit(d, "src/scripts/appstream-compose-check.sh", " --no-partial-urls", ""),
            "appstream-compose-check.sh: the screenshots must be mirrored as Flathub's linter wants (`--no-partial-urls`)",
        ),
    ];
    expect_red(&cases);
}

/// t1411 (ci-flatpak-screenshots, D3): the pre-flight composes every screenshot of the listing into
/// the catalogue under Flathub's media URL, and a screenshot compose cannot read fails it in
/// seconds, not after the build and the linter.
///
/// On the runner of 21505ad2 compose said `Success!` and dropped all four screenshots with a
/// `screenshot-save-error` hint ("Unrecognized image file format": no WebP loader). The red copy
/// points one screenshot at a text file of the same release tag, which fails the same way on any
/// machine.
#[test]
fn t1411_the_preflight_keeps_every_screenshot() {
    if !compose_can_run("t1411") {
        return;
    }
    let (ok, text) = compose_check(&repo());
    assert!(ok, "every screenshot must reach the catalogue:\n{text}");
    assert!(text.contains("4 screenshots mirrored"), "{text}");
    let dir = copy_tree("compose-shots");
    edit(
        &dir,
        "src/packaging/com.kinetren.Kinetren.metainfo.xml",
        "docs/images/morpher.webp",
        "README.md",
    );
    let (ok, text) = compose_check(&dir);
    assert!(!ok, "a screenshot compose cannot read must fail the pre-flight:\n{text}");
    assert!(text.contains("3 of 4 screenshots reached the catalogue"), "{text}");
    assert!(text.contains("screenshot-save-error"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// t1064 (ci-green-again): every script that mounts the crates.io registry into a container takes
/// it from `cargo-registry.sh`, and that script hands a fresh volume to the user.
///
/// The ffmpeg9 CI job of 94e36e9e died in `cargo metadata`: the runner has no `~/.cargo/registry`,
/// `ffmpeg-test.sh` mounted a fresh named volume (root's), and the container runs as the runner's
/// user. `release-build.sh` handed the volume over; the two test scripts had their own copy
/// without it.
#[test]
fn t1064_a_registry_volume_is_handed_to_the_user() {
    if cfg!(windows) {
        eprintln!("t1064 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let cases: [Break; 3] = [
        (
            "own-copy",
            |d| {
                edit(
                    d,
                    "src/scripts/ffmpeg-test.sh",
                    "REGISTRY=\"$(scripts/cargo-registry.sh \"$IMAGE\")\"",
                    "REGISTRY=kinetren-cargo-registry",
                )
            },
            "ffmpeg-test.sh: mounts the cargo registry without scripts/cargo-registry.sh",
        ),
        (
            "no-chown",
            |d| edit(d, "src/scripts/cargo-registry.sh", "chown", "true"),
            "a fresh volume must be handed to the user",
        ),
        (
            "no-helper",
            |d| std::fs::remove_file(d.join("src/scripts/cargo-registry.sh")).unwrap(),
            "cargo-registry.sh: missing",
        ),
    ];
    expect_red(&cases);
}

/// t966 (flatpak-flathub, D1): every copy of the application ID is the same string.
///
/// A desktop ties the running window to its menu entry by this string, and Flathub rejects a
/// listing whose desktop file, metainfo and manifest disagree. It used to be `kinetren` in five
/// hand-written places; the constant in kt-app is now the one source, and each copy is checked.
#[test]
fn t966_every_copy_of_the_app_id_is_the_same_string() {
    if cfg!(windows) {
        eprintln!("t966 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let id = "com.kinetren.Kinetren";
    let dir = copy_tree("t966-clean");
    let (ok, text) = guard(&dir);
    assert!(ok, "an unchanged copy must pass:\n{text}");
    let _ = std::fs::remove_dir_all(&dir);

    type Break = fn(&Path);
    let cases: [(&str, Break, &str); 5] = [
        (
            "ui-probe",
            |d| edit(d, "src/scripts/ui-probe.py", "WM_CLASS = \"com.kinetren.Kinetren\"", "WM_CLASS = \"kinetren\""),
            "src/scripts/ui-probe.py: `WM_CLASS = \"kinetren\"` differs from APP_ID",
        ),
        (
            "metainfo-id",
            |d| {
                let meta = d.join("src/packaging/com.kinetren.Kinetren.metainfo.xml");
                std::fs::write(
                    meta,
                    "<component><id>io.github.kinetren.Kinetren</id>\
                     <launchable type=\"desktop-id\">com.kinetren.Kinetren.desktop</launchable></component>",
                )
                .unwrap()
            },
            "src/packaging/com.kinetren.Kinetren.metainfo.xml: `<id>io.github.kinetren.Kinetren</id>` differs from APP_ID `com.kinetren.Kinetren`",
        ),
        (
            "desktop-icon",
            |d| edit(d, "src/packaging/com.kinetren.Kinetren.desktop", "Icon=com.kinetren.Kinetren", "Icon=kinetren"),
            "src/packaging/com.kinetren.Kinetren.desktop: `Icon=kinetren` differs from APP_ID `com.kinetren.Kinetren`",
        ),
        (
            "wm-class",
            |d| edit(d, "src/packaging/com.kinetren.Kinetren.desktop", "StartupWMClass=com.kinetren.Kinetren", "StartupWMClass=kinetren"),
            "`StartupWMClass=kinetren` differs from APP_ID `com.kinetren.Kinetren`",
        ),
        (
            "constant",
            |d| {
                edit(
                    d,
                    "src/apps/kt-app/src/app.rs",
                    "pub const APP_ID: &str = \"com.kinetren.Kinetren\";",
                    "pub const APP_ID: &str = \"com.kinetren.Editor\";",
                )
            },
            "src/packaging/com.kinetren.Editor.desktop: missing",
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
    // the window's id is the constant: the guard reads the same line kt-app compiles
    let app = std::fs::read_to_string(repo().join("src/apps/kt-app/src/app.rs")).unwrap();
    assert!(app.contains(".with_app_id(APP_ID)"), "the window does not use APP_ID");
    assert!(app.contains(&format!("pub const APP_ID: &str = \"{id}\";")));
}

/// One break of a guard test: its tag, the edit of a fresh copy, the text the guard must name.
type Break = (&'static str, fn(&Path), &'static str);

/// Run each break on a fresh copy and expect the guard to name it.
fn expect_red(cases: &[Break]) {
    for (tag, brk, want) in cases {
        let dir = copy_tree(tag);
        brk(&dir);
        let (ok, text) = guard(&dir);
        assert!(!ok, "{tag}: the guard must fail:\n{text}");
        assert!(text.contains(want), "{tag}: expected {want:?} in:\n{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

const MANIFEST: &str = "src/packaging/flatpak/com.kinetren.Kinetren.yml";

/// t967 (flatpak-flathub, D2–D6): the manifest's shape is guarded.
///
/// A runtime bump onto an FFmpeg major the loader has no module for, the replaced `ffmpeg-full`
/// extension, a broad permission, a permission without its reason, a bundled FFmpeg or a build that
/// says `nogit` — each would reach Flathub's review or the user before anyone noticed.
#[test]
fn t967_flatpak_manifest_shape_is_guarded() {
    if cfg!(windows) {
        eprintln!("t967 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let cases: [Break; 7] = [
        (
            "unsupported-major",
            |d| {
                edit(d, "src/packaging/flatpak/runtime-ffmpeg.txt", "26.08     8", "26.08     10");
                edit(d, MANIFEST, "runtime-version: '25.08'", "runtime-version: '26.08'");
            },
            "runtime-version 26.08 has FFmpeg 10, the loader supports",
        ),
        (
            "ffmpeg-full",
            |d| edit(d, MANIFEST, "command: kinetren", "add-extensions:\n  org.freedesktop.Platform.ffmpeg-full: {}\ncommand: kinetren"),
            "`ffmpeg-full` on runtime 25.08",
        ),
        ("device-all", |d| edit(d, MANIFEST, "- --device=dri", "- --device=all"), "finish-args `--device=all` is forbidden"),
        ("x11", |d| edit(d, MANIFEST, "- --socket=fallback-x11", "- --socket=x11"), "finish-args `--socket=x11` is forbidden"),
        (
            "no-reason",
            |d| edit(d, MANIFEST, "  # the window on Wayland\n", ""),
            "finish-args `--socket=wayland` has no reason comment",
        ),
        (
            "bundled-ffmpeg",
            |d| edit(d, MANIFEST, "modules:\n", "modules:\n  - name: ffmpeg\n    buildsystem: autotools\n"),
            "a module ships FFmpeg",
        ),
        (
            "no-version",
            |d| edit(d, MANIFEST, "    KT_VERSION_FULL: ", "    KT_VERSION_X: "),
            "the version environment `KT_VERSION_FULL` is missing",
        ),
    ];
    expect_red(&cases);
}

/// t968 (flatpak-flathub, D6): the offline crate sources follow `src/Cargo.lock`.
///
/// flatpak-builder builds without network: a crate bumped in the lock but absent from
/// `cargo-sources.json` fails the store build, not ours — so the guard compares them on every run.
#[test]
fn t968_offline_crate_sources_follow_the_lock() {
    if cfg!(windows) {
        eprintln!("t968 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let cases: [Break; 1] = [(
        "bumped-crate",
        |d| {
            let lock = std::fs::read_to_string(d.join("src/Cargo.lock")).unwrap();
            // the first registry package: its version moves on by one patch
            let at = lock.find("source = \"registry+").expect("a registry package");
            let head = &lock[..at];
            let v = head.rfind("version = \"").unwrap() + "version = \"".len();
            let end = v + lock[v..].find('"').unwrap();
            let bumped = format!("{}{}.999{}", &lock[..v], &lock[v..end], &lock[end..]);
            std::fs::write(d.join("src/Cargo.lock"), bumped).unwrap();
        },
        "is not in cargo-sources.json — run `make flatpak-sources`",
    )];
    expect_red(&cases);
}

const METAINFO: &str = "src/packaging/com.kinetren.Kinetren.metainfo.xml";

/// t969 (flatpak-flathub, D7): the listing is complete and the GPU guide names its commands.
///
/// Flathub lists an app only with valid metainfo that matches the manifest, and the owner asked
/// "at least give recommendations how to grant the application access in flatpak": a listing
/// without its translation, a screenshot that moves, a version of another form or a guide without
/// the undo would each reach a person.
#[test]
fn t969_metainfo_is_complete_and_the_guide_names_its_commands() {
    if cfg!(windows) {
        eprintln!("t969 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let cases: [Break; 5] = [
        (
            "no-ru",
            |d| {
                // every Russian paragraph goes
                let p = d.join(METAINFO);
                let t = std::fs::read_to_string(&p).unwrap();
                std::fs::write(&p, t.replace("<p xml:lang=\"ru\">", "<p xml:lang=\"xx\">")).unwrap();
            },
            "the ru description is missing",
        ),
        (
            "http-shot",
            |d| edit(d, METAINFO, "<image>https://", "<image>http://"),
            "is not an HTTPS URL of a release tag",
        ),
        (
            "version-form",
            |d| edit(d, METAINFO, "<release version=\"", "<release version=\"kt-"),
            "is not of the release tag form YY.MM.DD",
        ),
        (
            "no-undo",
            |d| edit(d, "docs/flatpak.md", "flatpak override --user --reset", "flatpak override --user --show"),
            "the guide lacks `flatpak override --user --reset com.kinetren.Kinetren`",
        ),
        (
            "no-guide-link",
            |d| edit(d, METAINFO, "<url type=\"help\">", "<url type=\"faq\">"),
            "the listing does not link the GPU guide docs/flatpak.md",
        ),
    ];
    expect_red(&cases);
    // the real metainfo also passes appstreamcli's strict check when the tool is here
    let have = std::process::Command::new("sh").args(["-c", "command -v appstreamcli"]).output();
    if have.is_ok_and(|o| o.status.success()) {
        let out = std::process::Command::new("appstreamcli")
            .args(["validate", "--strict", "--no-net"])
            .arg(repo().join(METAINFO))
            .output()
            .unwrap();
        assert!(out.status.success(), "appstreamcli validate --strict:\n{}", String::from_utf8_lossy(&out.stdout));
    }
}

/// t971 (flatpak-flathub, D9): the Flatpak job and the release bundle keep their shape.
///
/// The runner has no GPU, so CI proves the sandbox's software path — exactly the failure "the
/// package found the encoder-less FFmpeg" — and the free-plan decision keeps uploads out of CI.
/// The run itself is `src/scripts/flatpak-check.sh` in that job.
#[test]
fn t971_flatpak_job_and_release_bundle_keep_their_shape() {
    if cfg!(windows) {
        eprintln!("t971 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let cases: [Break; 5] = [
        ("no-job", |d| edit(d, ".github/workflows/ci.yml", "\n  flatpak:\n", "\n  flatpak-gone:\n"), "no `flatpak` job"),
        (
            "upload",
            |d| {
                edit(
                    d,
                    ".github/workflows/ci.yml",
                    "        run: src/scripts/flatpak-build.sh\n",
                    "        run: src/scripts/flatpak-build.sh\n      - uses: actions/upload-artifact@v4\n",
                )
            },
            "the flatpak job uploads nothing",
        ),
        (
            "no-lint",
            |d| edit(d, "src/scripts/flatpak-build.sh", "flatpak-builder-lint org.flatpak.Builder repo", "true"),
            "the linter on the repository is missing",
        ),
        (
            "no-bundle",
            |d| edit(d, ".github/workflows/release.yml", " \\\n            kinetren-*-linux-x86_64.flatpak kinetren-*-linux-x86_64.flatpak.sha256", ""),
            "the release must build and attach",
        ),
        ("no-make", |d| edit(d, "Makefile", "\nflatpak: ##", "\nflatpak-gone: ##"), "target `flatpak` must run flatpak-build.sh"),
    ];
    expect_red(&cases);
}

const CI: &str = ".github/workflows/ci.yml";
const MAIN_ONLY: &str = "github.ref == format('refs/heads/{0}', github.event.repository.default_branch)";

fn assert_each_is_caught(tag: &str, cases: &[(&str, fn(&Path), &str)]) {
    let dir = copy_tree(tag);
    let (ok, text) = guard(&dir);
    assert!(ok, "an unchanged copy must pass:\n{text}");
    let _ = std::fs::remove_dir_all(&dir);
    for (name, breaks, says) in cases {
        let dir = copy_tree(&format!("{tag}-{name}"));
        breaks(&dir);
        let (ok, text) = guard(&dir);
        assert!(!ok, "{name}: the guard must go red");
        assert!(text.contains(says), "{name}: expected {says:?} in\n{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// t1420 (ci-speedup): only the default branch writes a cache. Side branches compete with master's
/// entries for the repository's 10 GB.
#[test]
fn t1420_only_the_default_branch_writes_a_cache() {
    if cfg!(windows) {
        eprintln!("t1420 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let cases: [(&str, fn(&Path), &str); 3] = [
        (
            "bare-cache",
            |d| edit(d, CI, "uses: actions/cache/restore@v4\n        with:\n          path: .flatpak-builder", "uses: actions/cache@v4\n        with:\n          path: .flatpak-builder"),
            "flatpak: `actions/cache` saves from any branch",
        ),
        (
            "save-anywhere",
            |d| edit(d, CI, &format!("if: {MAIN_ONLY} && steps.builder-cache.outputs.cache-hit != 'true'\n        uses: actions/cache/save@v4"), "uses: actions/cache/save@v4"),
            "flatpak: an `actions/cache/save` step without a condition",
        ),
        (
            "rust-cache-anywhere",
            |d| edit(d, CI, "save-if: ${{ github.ref == format('refs/heads/{0}', github.event.repository.default_branch) }}", "save-if: true"),
            "test: `Swatinem/rust-cache` without a `save-if`",
        ),
    ];
    assert_each_is_caught("t1420", &cases);
}

/// t1421 (ci-speedup): every compiling job restores a cache keyed by what decides its content.
#[test]
fn t1421_every_compiling_job_restores_a_cache_keyed_by_its_inputs() {
    if cfg!(windows) {
        eprintln!("t1421 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let cases: [(&str, fn(&Path), &str); 6] = [
        (
            "flatpak-no-restore",
            |d| edit(d, CI, "uses: actions/cache/restore@v4\n        with:\n          path: .flatpak-builder", "uses: actions/checkout@v4\n        with:\n          path: .flatpak-keep"),
            "flatpak: compiles without restoring .flatpak-builder",
        ),
        (
            "windows-no-restore",
            |d| edit(d, CI, "uses: actions/cache/restore@v4\n        with:\n          path: |\n            src/target/win-container", "uses: actions/checkout@v4\n        with:\n          path: |\n            src/target/win-keep"),
            "windows-build: compiles without restoring src/target/win-container",
        ),
        (
            "ffmpeg9-no-restore",
            |d| edit(d, CI, "uses: actions/cache/restore@v4\n        with:\n          path: |\n            src/target/ffmpeg-test-container", "uses: actions/checkout@v4\n        with:\n          path: |\n            src/target/ffmpeg-keep"),
            "ffmpeg9: compiles without restoring src/target/ffmpeg-test-container",
        ),
        (
            "key-without-lock",
            |d| edit(d, CI, "key: windows-target-${{ env.RUST_TOOLCHAIN }}-${{ hashFiles('src/packaging/Dockerfile') }}-${{ hashFiles('src/Cargo.lock') }}", "key: windows-target-${{ env.RUST_TOOLCHAIN }}-${{ hashFiles('src/packaging/Dockerfile') }}"),
            "windows-build: the key of `src/target/` lacks hashFiles('src/Cargo.lock')",
        ),
        (
            "key-without-toolchain",
            |d| edit(d, CI, "key: ffmpeg-test-target-${{ env.RUST_TOOLCHAIN }}-", "key: ffmpeg-test-target-"),
            "ffmpeg9: the key of `src/target/` lacks env.RUST_TOOLCHAIN",
        ),
        (
            "key-by-run",
            |d| edit(d, CI, "key: windows-cross-image-${{ hashFiles('src/packaging/Dockerfile') }}", "key: windows-cross-image-${{ hashFiles('src/packaging/Dockerfile') }}-${{ github.run_id }}"),
            "windows-build: a cache key names a run or a date",
        ),
    ];
    assert_each_is_caught("t1421", &cases);
}

/// t1422 (ci-speedup): the tests the nextest profile starts first exist; a renamed test would
/// silently fall out of the order.
#[test]
fn t1422_the_ordered_tests_exist() {
    if cfg!(windows) {
        eprintln!("t1422 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let cases: [(&str, fn(&Path), &str); 2] = [
        (
            "missing-test",
            |d| edit(d, "src/.config/nextest.toml", "test(t399_)", "test(t9999_)"),
            "src/.config/nextest.toml: no `#[test]` named t9999_",
        ),
        (
            "no-profile",
            |d| std::fs::remove_file(d.join("src/.config/nextest.toml")).unwrap(),
            "src/.config/nextest.toml: missing",
        ),
    ];
    assert_each_is_caught("t1422", &cases);
}

/// t1670 (test-stability): fixture generation runs meet. A full run starts one per test process,
/// and on a fresh tree all of them find the fixtures missing; on a shared temp name one moved
/// another's half-written file into place and then failed on its own `mv` (t939, t796c red, green
/// alone). A stand-in `ffmpeg` writes its output in two steps: a file that reached `out/` before
/// its writer ended says so.
#[test]
fn t1670_fixture_runs_that_meet_do_not_spoil_each_other() {
    if cfg!(windows) {
        eprintln!("t1670 skipped on Windows: the fixtures come generated with the test archive");
        return;
    }
    let dir = std::env::temp_dir().join(format!("kt-t1670-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::copy(repo().join("src/fixtures/gen.sh"), dir.join("gen.sh")).unwrap();
    let stand_in = "#!/usr/bin/env bash\nfor out; do :; done\nprintf half > \"$out\"\nsleep 0.05\nprintf whole > \"$out\"\n";
    std::fs::write(bin.join("ffmpeg"), stand_in).unwrap();
    std::fs::write(bin.join("python3"), "#!/usr/bin/env bash\nexit 0\n").unwrap();
    for f in ["ffmpeg", "python3"] {
        assert!(Command::new("chmod").arg("+x").arg(bin.join(f)).status().unwrap().success());
    }
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let runs: Vec<_> = (0..6)
        .map(|_| {
            Command::new("bash")
                .arg(dir.join("gen.sh"))
                .env("PATH", &path)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .expect("bash for gen.sh")
        })
        .collect();
    for run in runs {
        let out = run.wait_with_output().unwrap();
        assert!(out.status.success(), "a run failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    let mut made = 0;
    for f in std::fs::read_dir(dir.join("out")).unwrap().flatten() {
        let name = f.file_name().to_string_lossy().into_owned();
        if !name.starts_with('.') {
            made += 1;
            assert_eq!(std::fs::read_to_string(f.path()).unwrap(), "whole", "{name}: half-written");
        }
    }
    assert!(made >= 10, "scene guard: the script made its fixtures: {made}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// t1671 (test-stability): a test waits for what must happen with `kt_time::test_deadline()`. A
/// bound of its own measures the machine: "the download did not end" in 20 s, "probe not
/// published" in 45 s, "writing the restore record took 44 s", all on correct code. What is left
/// is a limit that IS the check, and each of those says so here.
#[test]
fn t1671_a_test_does_not_bound_its_wait_by_the_clock() {
    const LIMIT_IS_THE_CHECK: &[(&str, &str)] = &[
        // inside the 5 s busy timeout: a thread still running here sits behind a lock across an fsync
        ("libs/kt-media/tests/sidecar_stall.rs", "from_secs(3)"),
        // the output ring is the yardstick of audio after a jump, and the video must follow it
        ("libs/kt-engine/tests/video_stall.rs", "from_millis(250)"),
        ("libs/kt-engine/tests/video_stall.rs", "from_millis(900)"),
        // a call that waits for the black hole takes the whole request timeout: the bound IS that
        // wait, and correct code takes milliseconds (test-hygiene)
        ("apps/kt-app/src/telemetry.rs", "enqueue blocked"),
        ("apps/kt-app/src/telemetry.rs", "exit held"),
        ("apps/kt-app/src/update_check.rs", "start or exit held"),
        // the test thread's own processor time, two scenes' rounds interleaved, the cheapest of
        // seven: a ratio of the frame's cost, judged against what the machine gave both (t1913, t1914)
        ("apps/kt-app/src/frame_cost_tests.rs", "let t0 = thread_ns();"),
        ("apps/kt-app/src/frame_cost_tests.rs", "*b = (*b).min(thread_ns() - t0);"),
        // the window the display's latencies are sampled over while the other classes load the
        // ring: a fixed span of presents, not a wait for something to happen (gpu-arbiter, t2254)
        ("libs/kt-gpu/tests/arbiter.rs", "while start.elapsed() < run {"),
        // the output thread measures the first callback after an open; past a second the play goes
        // on and the open is reported as it is (kept-mix-settle, D7): the bound IS the measurement
        ("apps/kt-app/src/audio_out.rs", "let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);"),
    ];
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let src = repo().join("src");
    let mut files = Vec::new();
    for d in ["apps", "libs", "plugins"] {
        walk(&src.join(d), &mut files);
    }
    let (bound, elapsed) = ("Instant::now() + ", ".elapsed() < ");
    let clock = |l: &str| {
        let at = l.find(bound).map(|i| i + bound.len()).or(l.find(elapsed).map(|i| i + elapsed.len()));
        at.is_some_and(|i| l[i..].trim_start_matches("std::time::").starts_with("Duration::from_"))
    };
    // test-hygiene: in TEST code any bound on `.elapsed()` but the ten-minute guard, and any
    // processor-time reading, is a clock in the check too (t1914 read `thread_ns` and split 8.5 ms
    // against 4.2 ms under load; the telemetry bounds were constants, not `Duration::from_`)
    let test_clock = |l: &str| {
        let bounded = [".elapsed() < ", ".elapsed() <= "].iter().any(|p| {
            l.find(p).is_some_and(|i| {
                let rhs = l[i + p.len()..].trim_start_matches("kt_time::");
                !rhs.starts_with("TEST_WAIT") && !rhs.starts_with("test_deadline")
            })
        });
        bounded || (l.contains("thread_ns()") && !l.trim_start().starts_with("//") && !l.contains("use "))
    };
    let mut seen = Vec::new();
    let mut bad = Vec::new();
    for f in files {
        let rel = f.strip_prefix(&src).unwrap().to_string_lossy().replace('\\', "/");
        let text = std::fs::read_to_string(&f).unwrap();
        // the guard's own patterns are not clocks
        if rel.ends_with("kt-cli/tests/dist_guard.rs") {
            continue;
        }
        // test code: a file of tests, or what follows a file's first `#[cfg(test)]`
        let whole = rel.contains("/tests/") || rel.ends_with("_tests.rs");
        let from = if whole { 0 } else { text.lines().position(|l| l.starts_with("#[cfg(test)]")).unwrap_or(usize::MAX) };
        let hit = |n: usize, l: &str| clock(l) || (n >= from && test_clock(l));
        for (n, l) in text.lines().enumerate().filter(|(n, l)| hit(*n, l)) {
            match LIMIT_IS_THE_CHECK.iter().find(|(p, w)| *p == rel && l.contains(w)) {
                Some(e) => seen.push(*e),
                None => bad.push(format!("{rel}:{}: {}", n + 1, l.trim())),
            }
        }
    }
    assert!(bad.is_empty(), "a wait bounded by the clock, use kt_time::test_deadline():\n{}", bad.join("\n"));
    for e in LIMIT_IS_THE_CHECK {
        assert!(seen.contains(e), "the list names a limit that is gone: {e:?}");
    }
}
