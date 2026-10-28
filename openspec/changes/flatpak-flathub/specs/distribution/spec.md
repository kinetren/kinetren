## MODIFIED Requirements

### Requirement: The program is called `kinetren` on disk
The GUI binary SHALL be named `kinetren` everywhere a person meets it as a file or a command: the
build output (`src/target/<profile>/kinetren`), the release tarball, `~/.local/bin/kinetren` after
installation, the Flatpak's `command`, the desktop entry's `Exec=kinetren %f`, and `make run`. The
names a DESKTOP uses to tie the window to its entry — the window's application id (Wayland
`app_id`), the desktop file name, `Icon=`, `StartupWMClass`, the icon files and the MIME package
file — SHALL all be the one application ID of the requirement "One application ID ties the window,
the menu entry and the store listing". The cargo package MAY stay `kt-app` (`cargo run -p kt-app`
keeps working); `kt-cli` keeps its name. `install.sh` SHALL remove a `~/.local/bin/kt-app` left by an
older install, the same way it already removes `kinotrain` leftovers, and SHALL remove the
`kinetren.desktop`, `kinetren.svg`, `kinetren.png` and `kinetren-mime.xml` an older install left
under the former desktop names, so the menu doesn't show two entries or start an old build. No
tarball, desktop entry or script that people run may start a binary named `kt-app`.
Why: the owner's request ("when building and distributing, call it kinetren, not kt-app …");
`kt-app` is an internal crate name that leaked into what people install. A Flatpak exports only
files named by its application ID (its desktop file, icons and MIME package), and Flathub requires
the ID to be the desktop file's name and the AppStream `<id>`; one name for both deliveries keeps
the taskbar link working in each (flatpak-flathub).

#### Scenario: Packaging names agree
- **WHEN** `dist-check.py --check` reads the manifest of `kt-app`, `install.sh`, the desktop file,
  `release-build.sh` and the Makefile
- **THEN** the binary is `kinetren` in all of them, the desktop `Exec` names `kinetren`, and
  `install.sh` installs `kinetren`, removes `kt-app` and the former `kinetren.*` desktop names; a
  temporary copy with `Exec=kt-app` or a tarball list with `kt-app` fails with that file named
- **TEST** t834, t835 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The installed menu entry starts the installed binary
- **WHEN** a tarball is installed over an older install that has `~/.local/bin/kt-app` and
  `~/.local/share/applications/kinetren.desktop`
- **THEN** `~/.local/bin/kinetren` exists, `~/.local/bin/kt-app` and the old `kinetren.desktop` do
  not, the desktop file named by the application ID exists, and the window's Wayland `app_id` and
  X11 `WM_CLASS` match its `StartupWMClass` (checked with `xprop` under Xvfb)
- **TEST** manual check recorded in `tasks.md`

## ADDED Requirements

### Requirement: One application ID ties the window, the menu entry and the store listing
Kinetren SHALL have exactly one reverse-DNS application ID (design D1), defined once in kt-app as a
constant and used for the window's application id. The same string SHALL be: the Flatpak
manifest's `id` and its file name, the desktop file name (`<ID>.desktop`) and its `Icon=` and
`StartupWMClass=`, the icon file names in every size, the MIME package file name (`<ID>.xml`),
the AppStream `<id>` and `<launchable type="desktop-id">`, and the names `install.sh` and
`release-build.sh` pack and install. The MIME type names themselves
(`application/x-kinetren-project`, `application/x-opentimelineio`) do not change.
Why: a desktop binds the running window to its menu entry by this string; Flathub rejects a
listing whose desktop file, metainfo and manifest disagree; and five hand-written copies of one
name drift — the guard keeps them one.

#### Scenario: Every copy of the ID is the same string
- **WHEN** `dist-check.py --check` reads the kt-app constant, the Flatpak manifest (`id` and file
  name), the desktop file (name, `Icon`, `StartupWMClass`), the icon files, the MIME package file
  name, the metainfo (`<id>`, `<launchable>`), `install.sh` and `release-build.sh`; then a
  temporary copy where, one at a time, the metainfo `<id>`, the desktop `Icon=` or the constant
  differs
- **THEN** on the tree it exits 0; on each copy it exits non-zero naming the file and the two
  differing values
- **TEST** t966 (`src/apps/kt-cli/tests/dist_guard.rs`)

### Requirement: A Flatpak manifest builds Kinetren against the runtime's FFmpeg
The repository SHALL carry a flatpak-builder manifest (`src/packaging/flatpak/<ID>.yml`) that builds
`kinetren` and `kt-cli` from this repository's source, offline, with the crate sources generated
from `src/Cargo.lock` (`src/packaging/flatpak/cargo-sources.json`). Its runtime SHALL be
`org.freedesktop.Platform` (SDK `org.freedesktop.Sdk`) on a branch whose FFmpeg major is in the
loader's table of supported majors; the guard SHALL read the branch → FFmpeg major mapping from a
committed table with the provenance of each row (24.08 → 7, 25.08 → 7, 26.08 → 8, read from the
runtime's `elements/include/ffmpeg.yml` of that branch). For a runtime ≥ 25.08 the manifest MUST NOT
add the `org.freedesktop.Platform.ffmpeg-full` extension (replaced there by `codecs-extra`, which the
runtime installs). The manifest MUST NOT ship an FFmpeg of its own (invariant 10): the program loads
the runtime's libraries by SONAME, or from the directory set by `KINETREN_FFMPEG_DIR` in `finish-args`
when the investigation (design D3) shows the base runtime's encoder-less libraries would be found
first. The build SHALL set `KT_VERSION_FULL`/`KT_VERSION_SHORT` from the tagged commit and
`KINETREN_POSTHOG_KEY` from `src/packaging/posthog-key`, as `release-build.sh` does.

The `finish-args` SHALL be exactly: `--share=ipc`, `--socket=wayland`, `--socket=fallback-x11`,
`--socket=pulseaudio`, `--device=dri`, `--share=network`, the file-system access of design D4, and
at most the one `--env=KINETREN_FFMPEG_DIR=…` above; each SHALL carry a comment with its reason.
`--device=all`, `--socket=x11` (without fallback), `--filesystem=host-os`/`host-etc`,
`--talk-name=org.freedesktop.Flatpak` and `--socket=session-bus` MUST NOT appear.
Why: a store user gets FFmpeg with the runtime instead of hunting for distribution packages, and
our dlopen loader already accepts majors 6–9 — but only if the runtime branch is one of them, and
`codecs-extra` replaced `ffmpeg-full` in 25.08 (Flathub announcement of freedesktop-sdk 25.08.0). The
base runtime's own FFmpeg is built `--disable-encoders`; loading it instead of `codecs-extra`'s would
leave no H.264 and no AAC encoder at all. Flathub requires static permissions kept to a minimum and
portals used where they exist (the file chooser already goes through the portal: rfd 0.15's
xdg-portal backend).

#### Scenario: The manifest's shape is guarded
- **WHEN** `dist-check.py --check` reads the manifest; then a temporary copy where, one at a time,
  the runtime branch maps to an FFmpeg major outside the loader's table, `ffmpeg-full` is added on
  a 25.08 runtime, `--device=all` or `--socket=x11` is added, a `finish-args` line has no reason
  comment, a module ships `libavcodec`, or the version environment is missing
- **THEN** on the tree it exits 0; each copy exits non-zero naming the file and the rule
- **TEST** t967 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The offline crate sources follow the lock file
- **WHEN** the guard compares `cargo-sources.json` with `src/Cargo.lock`; then a temporary copy of
  the lock with one crate's version bumped
- **THEN** every registry package of the lock (name, version, checksum) appears in the sources and
  no source names a package absent from the lock; the bumped copy fails naming the crate and the
  command that regenerates the file
- **TEST** t968 (`src/apps/kt-cli/tests/dist_guard.rs`)

### Requirement: AppStream metainfo describes the listing, and a guide tells how to reach the GPU
The repository SHALL carry `src/packaging/<ID>.metainfo.xml` with: `<id>` and the desktop launchable
of the application ID; name, summary and description in English and in every language of the
program's translation catalogs (`src/apps/kt-app/i18n/*.toml`); the licence as declared by `LICENSE`
(an SPDX `LicenseRef-…` for the Kinetren Non-Commercial License); the developer name; the homepage
and bug-tracker URLs of `github.com/kinetren/kinetren`; at least three screenshots by HTTPS URLs that
do not move between releases; a `<releases>` entry for every published release with the version
form of `kt-version`; an OARS content rating that declares the anonymous usage counter. It SHALL pass
`appstreamcli validate` and the Flathub linter in CI. The description SHALL link the GPU guide.

The GPU guide (`docs/flatpak.md`) SHALL say, for each vendor, what works without action and what to
do otherwise: Intel and AMD VAAPI through the runtime's Mesa and Intel media-driver extensions, with
`flatpak override --user --env=LIBVA_DRIVER_NAME=<driver> <ID>` for a wrong pick; NVIDIA through the
GL extension matching the host driver version, and `flatpak override --user --device=all <ID>` when
NVENC does not open with `--device=dri` only; and how to verify from inside the sandbox with
`flatpak run --command=kt-cli <ID> ffmpeg --probe-encoders` (capability `media-index`) and the
render window's video row (capability `export`), plus how to undo an override.
Why: Flathub lists an app only with valid metainfo that matches the manifest and the desktop file,
and the owner asked "at least give recommendations how to grant the application access in flatpak";
the per-vendor cause of "no hardware encoder" is in the sandbox's device and driver set, which the
program can report but not change (F-RES-008: "the more hermetic the packaging, the less of the
user's hardware is in it").

#### Scenario: The metainfo is complete and the guide names its commands
- **WHEN** the guard reads the metainfo and `docs/flatpak.md`; then a temporary metainfo without the
  Russian description, with a screenshot by `http://`, or with a `<releases>` version not of the
  `kt-version` form
- **THEN** on the tree: the id and launchable match the application ID, every catalog language has
  a translated name, summary and description, the licence matches `LICENSE`'s title, the newest
  release equals the version form, and the guide contains the `--probe-encoders` command, both
  `flatpak override` forms with the ID and an undo (`flatpak override --user --reset <ID>`); each
  temporary copy fails naming the missing piece
- **TEST** t969 (`src/apps/kt-cli/tests/dist_guard.rs`); the per-vendor results inside the sandbox — manual check on the owner's machines recorded in `tasks.md`

### Requirement: Inside a Flatpak the program does not send the person to the tarball
When the program runs inside a Flatpak sandbox (`/.flatpak-info` exists or `FLATPAK_ID` is set —
read through one seam), the daily update check (change `update-check`) SHALL NOT run and SHALL NOT
post its notice or mark the title: updates come through Flathub; the View checkbox SHALL be
disabled with the reason "updated by Flatpak". The "FFmpeg not found" hint on Linux SHALL, in the
sandbox, name the Flatpak runtime and the `kt-cli ffmpeg` report instead of distribution packages.
Why: a notice "Kinetren kt26.11.01 is out — github.com/kinetren/kinetren/releases" would lead a
store user to a tarball that installs a second copy beside the Flatpak; and distribution packages
installed on the host are invisible inside the sandbox.

#### Scenario: The sandbox switches the update notice off and changes the FFmpeg hint
- **WHEN** the consent of the update check is resolved with the sandbox seam reporting a Flatpak,
  for a release build with the checkbox stored `on`; separately, the FFmpeg-not-found hint is built
  for Linux inside and outside the sandbox
- **THEN** inside the sandbox the check is blocked with the reason "updated by Flatpak", no request
  is started and no title word appears; outside it the result is the change `update-check`'s;
  the hint inside names the runtime and `kt-cli ffmpeg`, the one outside names distribution
  packages; both texts exist in the five catalogs
- **TEST** t970 (`src/apps/kt-app/src/update_check.rs`, `src/apps/kt-app/src/i18n_tests.rs`)

### Requirement: CI builds and checks the Flatpak; a release carries the bundle
`.github/workflows/ci.yml` SHALL have a `flatpak` job that, on every push, builds the manifest with
`flatpak-builder` (runtime and SDK extensions from Flathub), runs the Flathub linter on the manifest
and the built repository, runs `appstreamcli validate` on the metainfo, and runs
`src/scripts/flatpak-check.sh` against the build: inside the sandbox `kt-cli ffmpeg` reports a loaded
FFmpeg whose major is the runtime's and which has the `libx264` and `aac` encoders, and
`kt-cli export` of a committed fixture project into the sandbox's temporary directory finishes and
decodes. The job MUST NOT upload anything. `make flatpak` SHALL run the same build and check
locally. The release workflow SHALL attach `kinetren-<version>-linux-x86_64.flatpak` (a single-file
bundle of that build) with its `.sha256` next to the tarball.
Why: the runner has no GPU, so CI proves the sandbox's software path (the FFmpeg it finds, an
export that decodes) — exactly the failure "the package found the encoder-less FFmpeg" — and the
hardware path stays a manual check; the free-plan decision keeps CI uploads out (owner, distribution
design); a bundle in the release lets people try a build before Flathub has it.

#### Scenario: The job's shape is guarded and the sandbox exports
- **WHEN** `dist-check.py --check` reads `ci.yml`, `release.yml` and the Makefile; and (in CI)
  `flatpak-check.sh` runs on the built repository
- **THEN** the `flatpak` job exists, runs on push and pull_request, uploads nothing, and calls the
  linter, `appstreamcli validate` and `flatpak-check.sh`; the release workflow names the `.flatpak`
  bundle and its `.sha256`; the Makefile has `flatpak`; the check prints the loaded FFmpeg major
  equal to the runtime's, `libx264` and `aac` present, and an export of the fixture that our decoder
  reads to its last frame
- **TEST** t971 (`src/apps/kt-cli/tests/dist_guard.rs` for the shape; the run is `src/scripts/flatpak-check.sh` in the CI job `flatpak`)
