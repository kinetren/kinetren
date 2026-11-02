## MODIFIED Requirements

### Requirement: A package installed without its Recommends names what the checks need
Every `apt-get install --no-install-recommends` in `.github/workflows/ci.yml`,
`.github/workflows/release.yml` and every Dockerfile SHALL also install, by name, each package
that a check run after it needs and that the install does not pull. The pairs live in one table
of `src/scripts/dist-check.py` (`RECOMMENDS_NEEDED`):
- `flatpak-builder` → `elfutils` (it strips with `eu-strip`);
- `flatpak-builder` → `librsvg2-common` (its `appstreamcli compose` renders the SVG icon through
  gdk-pixbuf, whose SVG loader is a Recommends of `librsvg2-2`);
- `flatpak-builder` → `webp-pixbuf-loader` (compose reads the WebP screenshots through gdk-pixbuf
  too, and its WebP loader is a package nothing in the install pulls);
- `xdg-utils` → `libfile-mimeinfo-perl` (xdg-mime without a desktop asks `mimetype`);
- `shared-mime-info` → `libglib2.0-bin` (`gio`, the oracle t935 asks).
A new pair is a new row.
Why: the CI run of 94e36e9e lost two jobs to dropped Recommends. The Flatpak build had no
`eu-strip`, and t935 got `application/json` for our project, because xdg-mime fell back to
libmagic's `file`, which never reads the MIME database. The run of 9e1f1a45 lost two more: compose
could not read the icon without the SVG loader, and perl's `mimetype` called a Kotlin source ours.
The run of 21505ad2 lost the Flatpak job once more: without the WebP loader compose dropped every
screenshot, and Flathub's linter found none in the catalogue.
Each was silent on the owner's desktop, which has the full packages and asks GIO.

#### Scenario: Each dropped Recommends a check needs is named
- **WHEN** `dist-check.py --check` runs on the repository; then on temporary copies where, one at
  a time, `ci.yml` or `release.yml` installs `flatpak-builder` without `elfutils`, or `ci.yml` or
  the Dockerfile installs `xdg-utils` without `libfile-mimeinfo-perl`
- **THEN** the repository passes; each copy fails, naming the file, the package and the one it
  must install
- **TEST** t1060 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The SVG loader and the MIME oracle are named
- **WHEN** `dist-check.py --check` runs on temporary copies where, one at a time, `ci.yml` or
  `release.yml` installs `flatpak-builder` without `librsvg2-common`, or `ci.yml` or the
  Dockerfile installs `shared-mime-info` without `libglib2.0-bin`
- **THEN** each copy fails, naming the file, the package and the one it must install
- **TEST** t1380 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The WebP loader is named
- **WHEN** `dist-check.py --check` runs on temporary copies where, one at a time, `ci.yml` or
  `release.yml` installs `flatpak-builder` without `webp-pixbuf-loader`
- **THEN** each copy fails, naming the file, the package and the one it must install
- **TEST** t1410 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The MIME database is asked as a file manager asks it
- **WHEN** t935 builds a MIME database from our MIME file and the system's `freedesktop.org.xml`
  and asks GIO (`gio info -a standard::content-type`) about a project written by
  `project_file::to_text`, a Kotlin source `Main.kt` and the legacy fixture
- **THEN** GIO names `application/x-kinetren-project`, `text/x-kotlin` and
  `application/x-kinotrain-legacy-project`. Where perl's `mimetype` exists, it names our project
  and the legacy fixture too. It is not asked about `Main.kt`: it takes the first of two
  equal-weight globs without reading the file, and that order differs between shared-mime-info
  versions
- **TEST** t935 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The MIME check runs only where it asks the database
- **WHEN** t935 runs on a machine with `update-mime-database` but no `gio`
- **THEN** it says it did not run and names `libglib2.0-bin`, instead of failing on another
  program's answer; on CI and in the build container it runs (t1380 holds the package)
- **TEST** t935 (`src/apps/kt-cli/tests/dist_guard.rs`)

### Requirement: The AppStream metadata is composed before the Flatpak build
`src/scripts/flatpak-build.sh` SHALL run `src/scripts/appstream-compose-check.sh` before
`flatpak-builder`. The script installs the files of the manifest's own `install … /app/share/…`
lines into a temporary folder, the same way the build does. It then runs `appstreamcli compose` on
that folder with the arguments that flatpak-builder 1.4.6+ passes under `flatpak-build.sh`'s
screenshot mirror (`--no-partial-urls`, `--prefix=/`, the origin, `--media-baseurl` =
`https://dl.flathub.org/media`, the media, result, data and icons folders,
`--components=<ID>,<ID>.desktop`). It fails with compose's own hints when compose reports an
error. It also fails, with the hints and the WebP loader's package name, unless the catalogue holds
as many screenshots as the metainfo and every image URL starts with the media URL, names a `.png`,
and exists in the media folder. Where `appstreamcli compose` is not available (no
`appstream-compose`), it says so and exits 0: flatpak-builder needs it anyway and fails there.
Why: on 9e1f1a45 the Flatpak job built for ten minutes, then compose could not read the icon. The
failure belongs to the listing and the host, not to the build, and shows in seconds without it.
On 21505ad2 compose said `Success!` and dropped all four WebP screenshots (`screenshot-save-error`,
no WebP loader); only the linter noticed, after the build.

#### Scenario: The listing composes, a broken icon is caught before the build
- **WHEN** `appstream-compose-check.sh` runs on the repository; then on a temporary copy whose SVG
  icon is not an image
- **THEN** the repository composes (exit 0, a catalog file with the component); the copy fails and
  names `file-read-error` and the icon's path. Without `appstreamcli compose`, or when the
  listing's first screenshot is unreachable (compose fetches the screenshots), the test says why it
  did not run
- **TEST** t1381 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: Every screenshot reaches the catalogue, an unreadable one fails before the build
- **WHEN** `appstream-compose-check.sh` runs on the repository; then on a temporary copy whose
  last screenshot URL points at a text file of the same release tag (what compose saw of every
  WebP file without the WebP loader)
- **THEN** the repository passes and says `4 screenshots mirrored`; the copy fails, saying
  `3 of 4 screenshots reached the catalogue` and naming `screenshot-save-error`. On a host
  without `webp-pixbuf-loader` (the packages of 21505ad2) the repository itself fails, in about a
  second, with `0 of 4`
- **TEST** t1411 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The pre-flight stays in the build script
- **WHEN** `dist-check.py --check` runs on a temporary copy whose `flatpak-build.sh` does not run
  `src/scripts/appstream-compose-check.sh`
- **THEN** it fails, naming the missing pre-flight
- **TEST** t1380 (`src/apps/kt-cli/tests/dist_guard.rs`)

## ADDED Requirements

### Requirement: The Flatpak's screenshots are mirrored as Flathub's linter wants
`src/scripts/flatpak-build.sh` SHALL build with `--mirror-screenshots-url=https://dl.flathub.org/media
--compose-url-policy=full` (the media URL named once, `MEDIA_URL`), so that the catalogue names
every screenshot by its full Flathub media URL and the repository carries the `screenshots/<arch>`
ref with the PNGs. Before anything else it SHALL refuse a flatpak-builder older than 1.4.6,
naming `ppa:flatpak/stable`. `ci.yml` and `release.yml` SHALL add that PPA before the install that
names `flatpak-builder`. `dist-check.py` holds the two arguments, the pre-flight's matching
`--media-baseurl` and `--no-partial-urls`, and the PPA.
Why: the run of 21505ad2 built and installed the Flatpak, and `flatpak-builder-lint repo` refused
it: `appstream-missing-screenshots` (D1 of change ci-flatpak-screenshots) and
`appstream-screenshots-not-mirrored-in-ostree`. The linter also refuses partial screenshot URLs
(`appstream-external-screenshot-url`). Flathub's linter page says these exceptions are "never
granted" and names this flow for builds made outside Flathub. `--compose-url-policy` came in
flatpak-builder 1.4.6, and the screenshots ref is committed by flatpak-builder itself since 1.4.5;
Ubuntu 24.04 ships 1.4.2.

#### Scenario: The mirror, the URL policy and the PPA are guarded
- **WHEN** `dist-check.py --check` runs on temporary copies where, one at a time, `flatpak-build.sh`
  loses `--mirror-screenshots-url` or `--compose-url-policy=full`, `appstream-compose-check.sh`
  loses `--media-baseurl` or `--no-partial-urls`, or `ci.yml` or `release.yml` loses
  `add-apt-repository -y ppa:flatpak/stable`
- **THEN** each copy fails, naming the file and the missing piece
- **TEST** t1410 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The whole Flatpak job passes the linter
- **WHEN** `src/scripts/flatpak-build.sh` runs in `ubuntu:24.04` with the packages of the CI job
  (`ppa:flatpak/stable`; flatpak, flatpak-builder, appstream, elfutils, librsvg2-common,
  webp-pixbuf-loader)
- **THEN** the pre-flight, the build, the install, both `flatpak-builder-lint` runs (no errors),
  `appstreamcli validate --strict` and `flatpak-check.sh` pass
- **TEST** manual check recorded in `tasks.md` (Docker isn't available to `cargo test`); the CI job `flatpak` runs it
