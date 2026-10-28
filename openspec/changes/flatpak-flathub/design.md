# Design

## Context

See proposal.md — Why. Observed in the tree and in the runtime recipes (2026-10-28):

- **Packaging today** (`src/packaging/`): `Dockerfile` (one build container, Ubuntu 22.04),
  `kinetren.desktop` (`Exec=kinetren %f`, `Icon=kinetren`, `StartupWMClass=kinetren`),
  `kinetren-mime.xml` (two types; translations are the parallel change `i18n-gaps`), `kinetren.svg`
  and `icons/<size>/kinetren.png`, `install.sh` (into `~/.local`, removes `kinotrain`/`kt-app`
  leftovers), `posthog-key`, `README-windows.txt`. `src/scripts/release-build.sh` passes
  `KT_VERSION_FULL/SHORT` and `KINETREN_POSTHOG_KEY` into the container (`:166-187`).
- **The application id is set, but not reverse-DNS**: `.with_app_id("kinetren")`
  (`src/apps/kt-app/src/app.rs:9100`), with a comment that on X11 `WM_CLASS` stays the binary name.
  The distribution spec ties `StartupWMClass=kinetren` to that id.
- **Licence**: `LICENSE` is the "Kinetren Non-Commercial License 1.0" (sharing free of charge allowed,
  commercial use not); the workspace manifest says `license = "UNLICENSED"` (`src/Cargo.toml:23`).
- **FFmpeg of the Freedesktop runtime** (freedesktop-sdk GitLab, `elements/include/ffmpeg.yml` per
  release branch, read 2026-10-28): 24.08 → `n7.0.3`, 25.08 → `n7.1.3`, 26.08 → `n8.1.3`; the base
  build is `--disable-encoders --disable-decoders` with `--enable-vaapi`, `--enable-vulkan`,
  `--enable-vdpau` and, on x86_64, `--enable-ffnvcodec`. The `codecs-extra` extension
  (`elements/extensions/codecs-extra/ffmpeg.bst`) rebuilds the same FFmpeg with
  `--enable-encoders --enable-decoders --enable-libx264 --enable-libx265 --enable-gpl` into
  `/usr/lib/<triplet>/codecs-extra/lib`; no libvpl (no QSV). The Flathub announcement of 25.08.0:
  "openh264 and ffmpeg-full extensions have been replaced by codecs-extra runtime extension", which
  installs with the runtime; "intel-vaapi-driver has been removed (replacement is
  intel-media-driver)"; a new NVIDIA VA-API driver extension (decode only — nvidia-vaapi-driver
  does not encode). All three majors are in `search::MAJORS` (`src/libs/kt-ffmpeg/src/search.rs:56-61`).
- **Both FFmpeg builds have SONAME `libavcodec.so.61` (or `.62`)** — which one `dlopen` finds depends
  on the runtime's library path order. Not measured yet.
- **Flathub rules** (docs.flathub.org "Requirements", 2026-10-28): the ID is reverse-DNS, 3–5
  components, must equal the metainfo `<id>`; GitHub-hosted apps use `io.github.` with ≥ 4
  components; an own domain must be controlled and may be verified by a token at
  `https://<domain>/.well-known/org.flathub.VerifiedApps.txt`; static permissions minimal, portals
  mandatory where they exist; metainfo must be upstream and valid, licence must match the source;
  "complete English localisation"; "Machine-generated or mixed translations are not acceptable".
- **Network users**: telemetry (`src/apps/kt-app/src/telemetry.rs`) and the daily update check of
  the parallel change `update-check` (consent in `resolve_consent`, its design D9); the Windows
  FFmpeg download is not built on Linux.
- **CI** (`.github/workflows/ci.yml`): jobs `test`, `ffmpeg9`, `windows-build`, `windows-test`;
  `dist-check.py` guards workflow shapes by line patterns; the distribution design rejected a
  third-party release action on audit grounds and keeps uploads out of CI.

## Goals / Non-Goals

**Goals:**
- A Flatpak that starts, opens media and exports with the runtime's FFmpeg on any Flathub machine,
  and encodes in hardware where the sandbox reaches the GPU.
- Every permission explained in the manifest; nothing a portal already provides.
- One application ID; the tarball and the Flatpak agree on it.

**Non-Goals:**
- Snap, AppImage, distribution packages.
- Bundling an FFmpeg of our own in the Flatpak (invariant 10 and the owner's rule "no FFmpeg build
  of its own").
- aarch64 (the release builds x86_64 only; `only-arches` in `flathub.json`).
- Hardware decode for playback in the sandbox (another change).
- Self-update or a Flathub-specific in-app update notice.

## Decisions

### D1. The application ID: one constant, chosen by the owner
`pub const APP_ID: &str` in kt-app is used by `with_app_id` and read by the guard (t966) together
with every file name. Recommended `com.kinetren.Kinetren` if `https://kinetren.com` serves (the
commit identity is `log@kinetren.com` and a site repository exists) — the ID then survives a move
off GitHub, and Flathub renames are "conservative"; otherwise `io.github.kinetren.Kinetren`. This is
Open Question 1; tasks stage 1 starts after the answer. The tarball moves to the same ID (Revision of
the distribution requirement "The program is called `kinetren` on disk": `StartupWMClass` and the
desktop file name were `kinetren` — reason: a Flatpak exports only files named by its ID, and two
deliveries with two names would break one of the taskbar links). On X11 the WM_CLASS must match
`StartupWMClass`: stage 1 measures with `xprop` what eframe's `with_app_id` sets there; if WM_CLASS
stays the binary name, `StartupWMClass` stays `kinetren` and the guard accepts exactly that pair.

### D2. Runtime branch: the newest stable `org.freedesktop.Platform` whose FFmpeg we load
Recommended 25.08 (FFmpeg 7.1.3, the current Flathub default when this was written) moving to
26.08 (FFmpeg 8.1.3) once it is the stable branch on Flathub; never `ffmpeg-full` on ≥ 25.08. The
guard (t967) maps the branch to the major by a committed table with the source line of each row, so
a runtime bump that lands on an unsupported major (a future FFmpeg 10) is red before Flathub builds
it. SDK extensions: `org.freedesktop.Sdk.Extension.rust-stable` (its toolchain must be ≥ the
Dockerfile's pinned one — checked in the build by `rustc --version`) and the LLVM extension of that
branch for libclang (bindgen); the exact LLVM extension name is read in stage 2, not assumed.

### D3. Which FFmpeg the sandbox loads is measured, then pinned if needed
Stage 2's first build runs `kt-cli ffmpeg` in the sandbox: if the loaded place is codecs-extra's
directory and `libx264`/`aac` exist, nothing is added; if the base runtime's encoder-less
`libavcodec` is found first, the manifest adds
`--env=KINETREN_FFMPEG_DIR=/usr/lib/x86_64-linux-gnu/codecs-extra/lib` (the loader tries that
directory first, media-index requirement) — never a copy of the libraries. `flatpak-check.sh`
(t971) keeps either outcome honest on every CI run.

### D4. File-system access: `--filesystem=host` (Open Question 2)
A project references media by paths relative to the `.kt` file or absolute anywhere (USB disks,
`/media`, `/mnt`, network mounts); the document portal grants only the file the person picked, so the
project would open with every clip a placeholder. Recommended `--filesystem=host` (what other
Flathub video editors request, to be confirmed in stage 2 from their manifests), with the reason in
the comment. The narrower alternative — `home` plus `/media`, `/run/media`, `/mnt` — breaks projects
on other mounts. `xdg-run/pipewire-0` is not needed (audio goes through `pulseaudio`).

### D5. Permissions and why each one
`ipc` (X11 shared memory), `wayland`, `fallback-x11` (x11rb `QueryPointer` during DnD on X11 —
kt-app CLAUDE.md "File DnD"; never plain `x11`), `pulseaudio` (cpal's ALSA goes through the runtime's
pulse plugin), `dri` (wgpu, VAAPI, and the NVIDIA device nodes flatpak exposes with it), `network`
(telemetry, update check — both switchable in View), the D4 file access, and the optional D3 env.
No `--device=all` in the manifest: the guide tells NVIDIA users to add it themselves when NVENC needs
`/dev/nvidia-uvm` and similar nodes that `dri` does not expose on their flatpak version — measured on
the owner's NVIDIA machine in stage 4, and the guide states the observed result, not a guess.

### D6. The build: flatpak-builder over the same source, offline crates
One module, `buildsystem: simple`: `cargo --offline build --profile dist -p kt-app -p kt-cli` with
`CARGO_HOME` pointing at the vendored sources from `cargo-sources.json` (generated by
`flatpak-cargo-generator.py` from `src/Cargo.lock`, committed under `src/packaging/flatpak/`,
regenerated by `make flatpak-sources`; t968 catches a stale file). The version comes from
`KT_VERSION_FULL/SHORT` set by the manifest for the tagged commit (flatpak-builder sources carry no
`.git` reliably, and `kt-version` would fall back to `nogit`); the telemetry key from the committed
`posthog-key`. Installs: `/app/bin/kinetren`, `/app/bin/kt-cli`, the desktop file, icons, MIME
package and metainfo under the ID. The release-build delivery checks (`ldd` without libav*) run in
the module's post-install as well. Rejected: building in our Docker image and repackaging the
tarball into a Flatpak (`extra-data` or a prebuilt binary — Flathub wants a build from source, and
a glibc-2.35 binary would ignore the runtime's newer libraries).

### D7. Metainfo with the program's languages — and the translation risk
The metainfo carries English plus the catalog languages (`ru`, `es`, `zh-Hans`, `ar`). Flathub's
rule against machine-generated translations is a risk for languages no speaker reviewed: Open
Question 3. Screenshots come from the README showcase pipeline (`kt-cli showcase`, the site
screenshots) by URLs of a tag, so they do not move. OARS: `social-info` "mild" (an anonymous usage
counter); everything else none. `<releases>` is written by `release-build.sh` from the tag's version
— one place, not edited by hand.

### D8. In the sandbox: one seam, two behaviours
`kt_dirs::sandbox()` (`/.flatpak-info` exists or `FLATPAK_ID` set; an injected `Env` in tests)
returns `Flatpak`/`None`. The update check's `resolve_consent` (change `update-check`, D9) gains this
input and blocks with "updated by Flatpak" — whichever of the two changes is applied second adds the
input and t970; the FFmpeg hint builder picks the sandbox text. Data lives in the sandbox's
`XDG_DATA_HOME` (`~/.var/app/<ID>/data/kinetren`); a tarball install's settings are not migrated
(Open Question 4).

### D9. CI job `flatpak` and the release bundle
A job on `ubuntu-24.04` that installs `flatpak` and `flatpak-builder` from the distribution, adds the
Flathub remote, runs `flatpak-builder --user --install-deps-from=flathub --repo=repo build <manifest>`,
the linter (`flatpak run --command=flatpak-builder-lint org.flatpak.Builder manifest|repo`),
`appstreamcli validate`, and `flatpak-check.sh`. No third-party action (the distribution design's
audit rule), no upload. `actions/cache` keeps `.flatpak-builder` keyed by the manifest and
`cargo-sources.json`. The release workflow builds the same and attaches
`flatpak build-bundle repo kinetren-<version>-linux-x86_64.flatpak <ID>` with its `.sha256`.

### D10. The Flathub submission is the owner's sequence, written down
`docs/flatpak.md` (maintainer part): fork `flathub/flathub`, branch from `new-pr`, add the manifest
(sources pointing at the release tag of `github.com/kinetren/kinetren`), `cargo-sources.json` and
`flathub.json` (`only-arches: [x86_64]`), open the PR titled "Add <ID>", answer the review; for
`com.kinetren.*` put the verification token on the site; after merge, updates are PRs to
`flathub/<ID>` (or an external-data-checker entry on the tag). The ID and the licence label
("Proprietary" on Flathub for a non-OSI licence) are decided before the PR.

## Risks / Trade-offs

- [Flathub review rejects `--filesystem=host`] → the reason is in the manifest; fallback is the
  narrower set of D4 with the known limitation written in the description.
- [The non-commercial licence: Flathub lists it as proprietary and its terms must allow Flathub's
  free redistribution] → the owner confirms (Open Question 5); `src/Cargo.toml`'s `UNLICENSED` is
  aligned with `LICENSE` in the metainfo, not in cargo (publish stays false).
- [Rust in `rust-stable` older than our code needs] → the build checks the version; the pin in the
  Dockerfile is the lower bound.
- [NVENC in the sandbox depends on the host driver version having a Flathub GL extension] → the guide
  says so and points to `kt-cli ffmpeg --probe-encoders`.
- [Telemetry over `--share=network` in a sandbox reads as tracking to reviewers] → the description
  says what is sent and where it is switched off (the telemetry spec's allow-list).
- [Renaming the desktop names breaks the guard of `i18n-gaps` (it reads `kinetren-mime.xml`)] → this
  change renames after `i18n-gaps` is applied, and moves its guard's path in the same commit.

## Migration Plan

The tarball's `install.sh` removes the former `kinetren.*` desktop names on the next install; a
person who also installs the Flatpak gets two menu entries (two deliveries) until he removes one —
the guide says so. Rollback of the ID: Flathub renames are hard, hence Open Question 1 before
submission.

## Open Questions

1. **Application ID** — `com.kinetren.Kinetren` (domain verification on kinetren.com) or
   `io.github.kinetren.Kinetren`? Recommended: `com.kinetren.Kinetren` if the site serves HTTPS on
   kinetren.com; the token file goes into the site repository.
2. **File access** — `--filesystem=host` or `home` + removable mounts? Recommended: `host`.
3. **Metainfo languages** — ship all five catalog languages, or English + Russian until a speaker
   reviews es/zh-Hans/ar (Flathub forbids machine-generated translations)? Recommended: English +
   Russian first.
4. **Settings of a tarball install** — copy them into the sandbox on first start? Recommended: no
   (two deliveries, two settings; the sandbox cannot read `~/.local/share` without extra access).
5. **Licence on Flathub** — confirm that the Non-Commercial License allows Flathub to distribute
   and that the listing shows as proprietary. Recommended: confirm, with donations off.
6. **Runtime branch at submission** — 25.08 or 26.08? Recommended: the newest branch Flathub marks
   stable on the submission day (both load).
