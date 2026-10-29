# Tasks

Test numbers t966–t971 are reserved by this change. Preconditions: `hw-encode-context` is applied
(VAAPI encode, `kt-cli ffmpeg --probe-encoders`); `i18n-gaps` is applied before the desktop-name
rename (it guards `kinetren-mime.xml`); t970 lands together with `update-check` (whichever is
applied second adds the sandbox input). Every stage ends with a commit (English message, the first
line = the stage and what was done). Stage only this change's files.

## 0. Records and the owner's answers

- [x] 0.1 The owner's words (kkt item 5) and his choice "Flathub + a VAAPI context" are recorded in `docs/user-feedback.md`; the proposal, delta, design and tasks are written. Verify `openspec validate flatpak-flathub --strict` is green
- [x] 0.2 F-RES-008 (`docs/features/resilience/environment/packaging-and-install.md`) gets a dated Flatpak section (reconnaissance: runtime FFmpeg, codecs-extra, the sandbox against the GPU). Verify `python3 src/scripts/features-index.py --check` reports no error for this note
- [ ] 0.3 Ask the owner design Open Questions 1–6 (ID, file access, metainfo languages, settings, licence on Flathub, runtime branch); record the answers in `docs/user-feedback.md` and fold them into `design.md` (D1, D2, D4, D7). Verify the answers are in both files
  - Answered by the owner: Q1 (`com.kinetren.Kinetren`, 2026-10-28 — "да com.kinetren.Kinetren") and
    Q5 (the licence allows Flathub). Q2 (file access), Q3 (metainfo languages), Q4 (settings of a
    tarball install) and Q6 (runtime branch) are NOT answered yet: the work goes on with the
    design's recommendations (`--filesystem=host`; English + Russian in the metainfo; no settings
    migration; the newest stable branch), each marked in `design.md` as pending, and they are asked
    in the next report. This task closes when the answers are recorded.
- [x] 0.4 Commit stage 0
  - Stage 0 was committed with the proposal; the rest of 0.3 lands with stage 1.

## 1. One application ID (D1)

- [x] 1.1 Measure first: under Xvfb, `xprop WM_CLASS` and (under a Wayland compositor in the rig, if available) the `app_id` of today's `kinetren` window; record what `with_app_id` sets on X11 here. Then write t966 in `src/apps/kt-cli/tests/dist_guard.rs` + `src/scripts/dist-check.py`. Verify it is RED on the tree
  - Measured under Xvfb with `xprop` (no Wayland compositor in the rig): the build with
    `.with_app_id("kinetren")` reports `WM_CLASS = "", "kinetren"` — eframe/winit puts the app id
    into the X11 class (the old note "WM_CLASS is the binary name" described the same string by
    coincidence). t966 written (`dist_guard.rs` + `dist-check.py` `check_app_id`): RED on the tree —
    "src/apps/kt-app/src/app.rs: `pub const APP_ID …` is required", the install.sh/release-build
    names missing (8 violations).
- [x] 1.2 `APP_ID` constant in kt-app used by `with_app_id`; rename the desktop file, icons, MIME package file and SVG to the ID; `Icon=`/`StartupWMClass` per the 1.1 measurement; `install.sh` installs the new names and removes the former `kinetren.*`; `release-build.sh` packs them; move `i18n-gaps`' guard path in the same commit. Verify t966, t834, t835 green and the manual check of the modified scenario (install over an old install, `xprop`) recorded here
  - `pub const APP_ID = "com.kinetren.Kinetren"` in `app.rs`, used by `with_app_id`; `git mv` of the
    desktop file, the MIME package (`com.kinetren.Kinetren.xml`), the SVG and the eight PNG sizes;
    `Icon=`/`StartupWMClass=` = the ID; `install.sh` installs `$ID.*` and removes the former
    `kinetren.*` desktop names; `release-build.sh` packs them; `make-ico.py`, README's logo path,
    toolbar comment; `i18n-gaps`' guards (t930's file names in `i18n_tests.rs`, dist-check's
    `MIME`/`DESKTOP` now derived from the ID, t931/t935 paths) moved in the same commit. t966,
    t834, t835, t931, t935 green; kt-app i18n tests green. Manual check: the new build under Xvfb
    reports `WM_CLASS = "", "com.kinetren.Kinetren"` = `StartupWMClass`; install over an older
    install in a temp HOME (`kt-app`, `kinetren.desktop`, `kinetren.svg`, `kinetren-mime.xml`,
    `kinetren.png` present) leaves `kinetren`, the `com.kinetren.Kinetren.*` files and none of the
    old ones; the installed entry reads `Exec=kinetren %f`, `Icon=com.kinetren.Kinetren`,
    `StartupWMClass=com.kinetren.Kinetren`. The Wayland `app_id` is not measured (no compositor).
- [x] 1.3 Commit stage 1
  - Committed.

## 2. The manifest (D2–D6)

- [ ] 2.1 Write t967 and t968 (`dist_guard.rs` + `dist-check.py`, with the branch → FFmpeg-major table and its provenance lines). Verify RED
- [ ] 2.2 `src/packaging/flatpak/<ID>.yml`: runtime/SDK, rust-stable and the LLVM extension of that branch (record the exact name found), the `simple` module with `--offline` cargo, `KT_VERSION_*`, the telemetry key, the post-install `ldd` check, the `finish-args` of D5 each with its comment; `cargo-sources.json` via `flatpak-cargo-generator.py` and a `make flatpak-sources` target. Verify `flatpak-builder --user --install-deps-from=flathub build <manifest>` succeeds locally and t967, t968 green
- [ ] 2.3 D3 measurement: in the sandbox `flatpak run --command=kt-cli <ID> ffmpeg`; record the loaded place, major and whether `libx264`/`aac` exist; add the `KINETREN_FFMPEG_DIR` env only if the base FFmpeg was found. Verify the second run reports codecs-extra's place with both encoders
- [ ] 2.4 D4: read the `finish-args` of the Flathub manifests of Kdenlive and Shotcut and record what they request; apply the owner's answer to Open Question 2. Verify a fixture project with media outside `$HOME` (a temp mount in the rig) opens with no placeholders
- [ ] 2.5 Commit stage 2

## 3. Metainfo, sandbox behaviour, CI (D7–D9)

- [ ] 3.1 `src/packaging/<ID>.metainfo.xml` (languages per Open Question 3, screenshots by tag URLs, OARS, licence, `<releases>` written by `release-build.sh`); t969's metainfo part. Verify `appstreamcli validate --strict` and the Flathub linter on the local build pass, and t969 green for the metainfo
- [ ] 3.2 `kt_dirs::sandbox()` seam; the update-check consent input and the Flatpak FFmpeg hint in the five catalogs; t970 (`src/apps/kt-app/src/update_check.rs`, `i18n_tests.rs`). Verify t970 green and the update-check tests (t872–t878) still green
- [ ] 3.3 `src/scripts/flatpak-check.sh` (FFmpeg major = the runtime's, `libx264` + `aac`, `kt-cli export` of a committed fixture decoded to its last frame); `make flatpak`; the `flatpak` CI job and the release bundle (D9); t971's shape guard. Verify t971 green, `make flatpak` green locally, and the CI job green on a pushed branch (record the run URL here)
- [ ] 3.4 Commit stage 3

## 4. GPU access and the guide

- [ ] 4.1 On the owner's machines inside the sandbox: Intel/AMD — `kt-cli ffmpeg --probe-encoders` and a render with `h264_vaapi` (with and without `LIBVA_DRIVER_NAME`); NVIDIA — the same with `--device=dri` only and with `flatpak override --user --device=all <ID>`. Record per machine: driver, GL extension, which encoders open, here
- [ ] 4.2 `docs/flatpak.md`: the per-vendor guide from 4.1's observations (not guesses), the verify commands, the undo (`flatpak override --user --reset <ID>`), and the maintainer part (D10: the Flathub submission steps). Verify t969's guide part green and the metainfo links the guide
- [ ] 4.3 `openspec/specs/distribution/design.md` gets the "why" at archive (D1 one ID, D3 measured FFmpeg, D5 permissions, D9 no upload); F-RES-008 verdict updated. Verify `openspec validate --specs` and `features-index.py --check` green
- [ ] 4.4 Commit stage 4

## 5. Finish and submission

- [ ] 5.1 Run `./scripts/finish.sh` from `src/` (and `--release` once). Verify it is green and record the version `vYY.MM.DD-hash` here
- [ ] 5.2 The owner opens the Flathub PR (D10) from the first release tag; record the PR URL and the reviewers' requests here; answers that change the manifest go back through this change's guards
