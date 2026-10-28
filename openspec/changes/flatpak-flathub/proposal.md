# Proposal

## Why

The owner (2026-10-27, kkt item 5, `docs/user-feedback.md`): "let's think how to release into a
flatpak repo, and video acceleration must work in render, or at least give recommendations how to
grant the application access in flatpak"; he chose **Flathub + a VAAPI context** (the VAAPI part is
the prerequisite change `hw-encode-context`). Today Kinetren ships only a tarball and a Windows zip
(capability `distribution`); a Linux user who installs software from a store cannot find it, and the
tarball needs a system FFmpeg 6–9 that some distributions do not install. A Flatpak brings the
FFmpeg of the Freedesktop runtime with it, which our loader already accepts (invariant 10). Feature
note F-RES-008 (`docs/features/resilience/environment/packaging-and-install.md`).

## What Changes

- **A Flatpak manifest in the repository** (`src/packaging/flatpak/`): runtime
  `org.freedesktop.Platform` on a branch whose FFmpeg major is one we load (24.08 → FFmpeg 7.0,
  25.08 → 7.1, 26.08 → 8.1, all read from the runtime's recipes; since 25.08 the old
  `org.freedesktop.Platform.ffmpeg-full` extension is replaced by `codecs-extra`, installed with
  the runtime — libx264, VAAPI and NVENC enabled), the Rust and LLVM SDK extensions (bindgen needs
  libclang), an offline crate list generated from `src/Cargo.lock`, the version string and the
  telemetry key passed the way `release-build.sh` passes them.
- **Sandbox permissions, each with its reason**: `--share=ipc`, `--socket=wayland`,
  `--socket=fallback-x11` (x11rb reads the pointer during file DnD on X11), `--socket=pulseaudio`
  (cpal/ALSA goes through it), `--device=dri` (wgpu composition, VAAPI encode), `--share=network`
  (telemetry and the update check only — the parallel change `update-check`), and file-system access
  wide enough for projects whose media live anywhere (design: an open question between `host` and a
  narrower set).
- **One reverse-DNS application ID** for everything a desktop meets: the Wayland `app_id` set by
  the program (today the plain `kinetren`, `src/apps/kt-app/src/app.rs:9100`), the desktop file name,
  `Icon=`, `StartupWMClass`, the icon file names, the MIME package file name and the AppStream
  `<id>`; the tarball's `install.sh` moves to the same names and removes the old `kinetren.*` ones.
  Candidates: `com.kinetren.Kinetren` (needs the domain verified on Flathub) or
  `io.github.kinetren.Kinetren` (no domain, tied to the GitHub organisation) — the owner chooses.
- **AppStream metainfo** (`src/packaging/<id>.metainfo.xml`): name, summary and description with
  translations for the program's languages, screenshots, releases with the program's version, the
  licence as declared in `LICENSE`, content rating (online usage counter), launchable; validated in
  CI.
- **The program knows it is in a Flatpak**: the update check does not tell the person to download
  a tarball (updates come from Flathub); the FFmpeg window's Linux hint names the runtime, not
  distribution packages.
- **A user guide to GPU access in the sandbox**: what works out of the box (Mesa VAAPI, the Intel
  media driver extension), NVIDIA (the GL extension matching the host driver, NVENC, and
  `flatpak override --device=all` when NVENC does not open with `dri` only), the VAAPI driver
  override (`LIBVA_DRIVER_NAME`), and how to verify inside the sandbox with
  `flatpak run --command=kt-cli <id> ffmpeg --probe-encoders` (from `hw-encode-context`).
- **CI builds the Flatpak** on every push (a job in `.github/workflows/ci.yml` with
  `flatpak-builder` and the Flathub linter; nothing uploaded), `make flatpak` builds it locally, and
  the release attaches a `.flatpak` bundle next to the tarball. The Flathub submission is a
  documented sequence of steps (fork, manifest PR, review, the app repository), done by the owner.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `distribution`: adds the Flatpak manifest and its permissions, the runtime/FFmpeg-major rule,
  the offline crate sources, the AppStream metainfo, the CI build and the release bundle, the
  in-sandbox behaviour (update notice, FFmpeg hint) and the GPU-access guide; modifies "The program
  is called `kinetren` on disk" — the binary keeps its name, the desktop-facing names move to the
  application ID.

## Impact

- New files: `src/packaging/flatpak/<id>.yml`, `src/packaging/flatpak/cargo-sources.json`
  (generated), `src/packaging/<id>.metainfo.xml`, `src/scripts/flatpak-check.sh`, `docs/flatpak.md`
  (the GPU guide); renamed: `src/packaging/kinetren.desktop`, `kinetren-mime.xml`, `kinetren.svg`
  and `icons/*/kinetren.png` to the ID.
- Changed: `src/apps/kt-app/src/app.rs` (one `APP_ID` constant for `with_app_id`),
  `src/packaging/install.sh`, `src/scripts/release-build.sh` (packs the renamed files),
  `src/scripts/dist-check.py` + `src/apps/kt-cli/tests/dist_guard.rs` (the new guards), `Makefile`
  (`flatpak`), `.github/workflows/ci.yml` (a `flatpak` job), `.github/workflows/release.yml` (the
  bundle), `src/apps/kt-app/src/update_check.rs` (of the parallel change `update-check`) and the
  FFmpeg hint text in the catalogs.
- Depends on `hw-encode-context` (VAAPI encode, `kt-cli ffmpeg --probe-encoders`); touches the same
  MIME file as `i18n-gaps` (translations) and the same consent function as `update-check`.
- No engine change, no project-format change; invariants 9 and 10 kept — the sandbox uses the
  runtime's FFmpeg, loaded by SONAME or `KINETREN_FFMPEG_DIR`, never a copy of our own.
