## MODIFIED Requirements

### Requirement: A package installed without its Recommends names what the checks need
Every `apt-get install --no-install-recommends` in `.github/workflows/ci.yml`,
`.github/workflows/release.yml` and every Dockerfile SHALL also install, by name, each package
that a check run after it needs and that the install does not pull. The pairs live in one table
of `src/scripts/dist-check.py` (`RECOMMENDS_NEEDED`):
- `flatpak-builder` → `elfutils` (it strips with `eu-strip`);
- `flatpak-builder` → `librsvg2-common` (its `appstreamcli compose` renders the SVG icon through
  gdk-pixbuf, whose SVG loader is a Recommends of `librsvg2-2`);
- `xdg-utils` → `libfile-mimeinfo-perl` (xdg-mime without a desktop asks `mimetype`);
- `shared-mime-info` → `libglib2.0-bin` (`gio`, the oracle t935 asks).
A new pair is a new row.
Why: the CI run of 94e36e9e lost two jobs to dropped Recommends. The Flatpak build had no
`eu-strip`, and t935 got `application/json` for our project, because xdg-mime fell back to
libmagic's `file`, which never reads the MIME database. The run of 9e1f1a45 lost two more: compose
could not read the icon without the SVG loader, and perl's `mimetype` called a Kotlin source ours.
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

## ADDED Requirements

### Requirement: The AppStream metadata is composed before the Flatpak build
`src/scripts/flatpak-build.sh` SHALL run `src/scripts/appstream-compose-check.sh` before
`flatpak-builder`. The script installs the files of the manifest's own `install … /app/share/…`
lines into a temporary folder, the same way the build does. It then runs `appstreamcli compose` on
that folder with the arguments flatpak-builder 1.4 passes (`--prefix=/`, the origin, the result,
data and icons folders, `--components=<ID>,<ID>.desktop`). It fails with compose's own hints when
compose reports an error. Where `appstreamcli compose` is not available (no `appstream-compose`),
it says so and exits 0: flatpak-builder needs it anyway and fails there.
Why: on 9e1f1a45 the Flatpak job built for ten minutes, then compose could not read the icon. The
failure belongs to the listing and the host, not to the build, and shows in seconds without it.

#### Scenario: The listing composes, a broken icon is caught before the build
- **WHEN** `appstream-compose-check.sh` runs on the repository; then on a temporary copy whose SVG
  icon is not an image
- **THEN** the repository composes (exit 0, a catalog file with the component); the copy fails and
  names `file-read-error` and the icon's path. Without `appstreamcli compose` the test says why it
  did not run
- **TEST** t1381 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The pre-flight stays in the build script
- **WHEN** `dist-check.py --check` runs on a temporary copy whose `flatpak-build.sh` does not run
  `src/scripts/appstream-compose-check.sh`
- **THEN** it fails, naming the missing pre-flight
- **TEST** t1380 (`src/apps/kt-cli/tests/dist_guard.rs`)
