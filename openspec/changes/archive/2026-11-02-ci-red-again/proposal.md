# Proposal: ci-red-again

## Why

The CI run on master 9e1f1a45 (the owner's log archive `logs_112101614406.zip`) is red in three
jobs for two causes: t935 in `test` and `ffmpeg9`, and `appstreamcli compose` in `flatpak`. Both
are defects of the tree, found only now because `ci-green-again` made the jobs reach these steps
for the first time. A red CI hides every later regression, so it has to be green before the other
executors merge on top of it.

## What Changes

Two causes, each reproduced on this machine and in an `ubuntu:24.04` container (the runner's
system):

1. **t935** — `xdg-mime query filetype Main.kt` answered `application/x-kinetren-project`. Without a
   desktop, xdg-mime asks perl's `mimetype` (File::MimeInfo 0.34). It reads high-priority magic,
   then takes the FIRST type of an extension in `globs2`, without sniffing when two types share an
   equal-weight glob. The order of those two lines depends on the shared-mime-info version: ours
   first on Ubuntu (2.1, 2.4), Kotlin first on Fedora (2.3). The owner's desktop asks GIO, which
   sniffs, so t935 was green there and red on the runner. GIO (`gio info`, what Nautilus, Nemo,
   Caja and Thunar use) and Qt (`kmimetypefinder`, Dolphin) follow the shared-mime-info spec and
   name all three files right on Ubuntu 22.04, 24.04 and Fedora 41. t935 asks GIO; perl's
   `mimetype` is asked only about our two own files, which its magic-first order does get right.
   CI and the build container install `libglib2.0-bin` (the `gio` tool). The MIME file is
   unchanged.
2. **flatpak** — after the build and install succeeded, `appstreamcli compose` failed:
   `file-read-error` "Unable to read data from file …/com.kinetren.Kinetren.svg: Unrecognized image
   file format", then `filters-but-no-output` because the one component was dropped. flatpak-builder
   runs compose on the HOST, and compose renders the SVG icon through gdk-pixbuf. Its SVG loader
   is `librsvg2-common`, a Recommends of `librsvg2-2` that `--no-install-recommends` drops.
   `ci.yml` and `release.yml` name it. A new script, `src/scripts/appstream-compose-check.sh`, runs
   the same compose on the manifest's own install lines in a temporary folder. `flatpak-build.sh`
   runs it before the ten-minute build, so a missing loader or a broken listing fails in seconds.

Both new packages are rows of the `RECOMMENDS_NEEDED` table of `dist-check.py`. That table holds
what a check needs next to a package installed without its Recommends.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `distribution`: the table of packages a check needs gains `libglib2.0-bin` (t935's oracle) and
  `librsvg2-common` (compose's SVG loader). t935 asks GIO and says what it lacks when it cannot.
  The metadata compose runs before the Flatpak build.

## Impact

- `src/apps/kt-cli/tests/dist_guard.rs`: t935 rewritten around GIO; new t1380 (the new table rows
  and the pre-flight step, red copies) and t1381 (the compose pre-flight on the real tree and on a
  broken icon, where `appstreamcli compose` exists).
- `src/scripts/dist-check.py`: `RECOMMENDS_NEEDED` rows; `check_flatpak` requires the pre-flight.
- `src/scripts/appstream-compose-check.sh` (new), `src/scripts/flatpak-build.sh`.
- `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `src/packaging/Dockerfile` (the base
  stage gains `libglib2.0-bin`: the container images rebuild once).
- `src/packaging/com.kinetren.Kinetren.xml`: only its comment, which named the wrong oracle.
- No program code and no user-visible behaviour change; no What's new line (internal CI work).
