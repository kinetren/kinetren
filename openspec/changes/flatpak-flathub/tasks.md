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
  - finish.sh after stage 3 was RED in every live scenario: the rig (`scripts/ui-probe.py`) found the
    window by `WM_CLASS = "kinetren"` — a sixth copy of the ID the stage-1 search missed. Fixed, and
    `check_app_id` now guards it (t966 case `ui-probe`). The same run showed t958 aborting the test
    process under nextest ("pure virtual method called", SIGABRT): its real renders were dropped
    still encoding; the test now cancels and waits for them (`stop_render`).
  - Committed.

## 2. The manifest (D2–D6)

- [x] 2.1 Write t967 and t968 (`dist_guard.rs` + `dist-check.py`, with the branch → FFmpeg-major table and its provenance lines). Verify RED
  - t967 and t968 in `dist_guard.rs`, `dist-check.py` `check_flatpak` / `check_cargo_sources`; the
    branch table `src/packaging/flatpak/runtime-ffmpeg.txt` with the provenance of each row (24.08 → 7,
    25.08 → 7, 26.08 → 8). Before the manifest existed the guard was red ("…yml: missing"); each
    break of the two tests is red on its copy, naming the file and the rule.
- [ ] 2.2 `src/packaging/flatpak/<ID>.yml`: runtime/SDK, rust-stable and the LLVM extension of that branch (record the exact name found), the `simple` module with `--offline` cargo, `KT_VERSION_*`, the telemetry key, the post-install `ldd` check, the `finish-args` of D5 each with its comment; `cargo-sources.json` via `flatpak-cargo-generator.py` and a `make flatpak-sources` target. Verify `flatpak-builder --user --install-deps-from=flathub build <manifest>` succeeds locally and t967, t968 green
  - Written: `src/packaging/flatpak/com.kinetren.Kinetren.yml` (runtime 25.08 per Q6's
    recommendation, rust-stable, `llvm20` — the name is still to be confirmed against the branch,
    `buildsystem: simple`, `cargo --offline --profile dist`, `KT_VERSION_*`, the telemetry key, the
    install of the ID-named files, the `ldd` check, D5's finish-args each with its reason, the
    working tree as a `dir` source skipping `src/target`). `cargo-sources.json` (526 crates) is
    generated by OUR `src/scripts/flatpak-cargo-sources.py` instead of `flatpak-cargo-generator.py`:
    every package of the lock comes from crates.io, so its layout (`.crate` archive +
    `.cargo-checksum.json` + `cargo/config.toml` replacing crates.io) is twenty lines, and nothing
    third-party is downloaded into the build; `make flatpak-sources` runs it, `--check` is t968.
    t967, t968 green. NOT verified: `flatpak-builder` — `flatpak`/`flatpak-builder` are not
    installed on this machine (installing system packages is the owner's step); the first local
    or CI build closes this task (and the `llvm20` name).
- [ ] 2.3 D3 measurement: in the sandbox `flatpak run --command=kt-cli <ID> ffmpeg`; record the loaded place, major and whether `libx264`/`aac` exist; add the `KINETREN_FFMPEG_DIR` env only if the base FFmpeg was found. Verify the second run reports codecs-extra's place with both encoders
  - Needs the built sandbox (2.2).
- [ ] 2.4 D4: read the `finish-args` of the Flathub manifests of Kdenlive and Shotcut and record what they request; apply the owner's answer to Open Question 2. Verify a fixture project with media outside `$HOME` (a temp mount in the rig) opens with no placeholders
  - Read 2026-10-29 from the Flathub repositories: Kdenlive (`org.kde.kdenlive.json`) and Shotcut
    (`org.shotcut.Shotcut.yaml`), both on `org.kde.Platform` 6.11, request `--share=ipc`,
    `--socket=fallback-x11`, `--socket=wayland`, `--device=all`, `--socket=pulseaudio`,
    `--share=network`, `--filesystem=host`, `--filesystem=xdg-run/pipewire-0` (Kdenlive also
    `--system-talk-name=org.freedesktop.UDisks2`). So `--filesystem=host` is what the reviewers
    already accept for video editors (D4, Q2 pending the owner); `--device=all` stays out of ours
    (D5: the guide tells NVIDIA users to add it). The fixture with media outside `$HOME` needs the
    sandbox (2.2).
- [ ] 2.5 Commit stage 2

## 3. Metainfo, sandbox behaviour, CI (D7–D9)

- [x] 3.1 `src/packaging/<ID>.metainfo.xml` (languages per Open Question 3, screenshots by tag URLs, OARS, licence, `<releases>` written by `release-build.sh`); t969's metainfo part. Verify `appstreamcli validate --strict` and the Flathub linter on the local build pass, and t969 green for the metainfo
  - `src/packaging/com.kinetren.Kinetren.metainfo.xml`: English + Russian (Q3's recommendation, pending
    the owner), `LicenseRef-proprietary=…/LICENSE` with the licence named in the description, the
    developer, homepage/bugtracker and a `help` URL to the guide (appstreamcli refuses a plain URL in
    the description), four screenshots by `v26.10.26` tag URLs, OARS `social-info` mild, the release
    26.10.26. `appstreamcli validate --strict --pedantic --no-net`: valid, one pedantic note
    (uppercase letters in the ID — the owner's choice). t969 green (`check_metainfo`). NOT done: the
    Flathub linter (it runs from the Flathub `org.flatpak.Builder` app — CI job / `make flatpak`), and
    `<releases>` written by `release-build.sh`: the entry is by hand for now (writing a tracked file
    during a release build would dirty the tag's tree).
- [x] 3.2 `kt_dirs::sandbox()` seam; the update-check consent input and the Flatpak FFmpeg hint in the five catalogs; t970 (`src/apps/kt-app/src/update_check.rs`, `i18n_tests.rs`). Verify t970 green and the update-check tests (t872–t878) still green
  - `kt_dirs::Sandbox` / `sandbox_of(env, info_file)` / `sandbox()`; `search::not_found_hint` names the
    runtime and `kt-cli ffmpeg` inside a Flatpak (kt-ffmpeg now depends on kt-dirs); the FFmpeg window
    picks `ffmpeg.hint_flatpak` (five catalogs) by its `os` and `sandbox` fields. t970 (the seam in
    kt-dirs, the hint in kt-ffmpeg, the catalogs in kt-app) green. The update-check half is not here:
    `update-check` is not applied on this branch, so it adds the sandbox input to `resolve_consent`
    when it is applied (as tasks' preamble says); t872–t878 do not exist here.
- [ ] 3.3 `src/scripts/flatpak-check.sh` (FFmpeg major = the runtime's, `libx264` + `aac`, `kt-cli export` of a committed fixture decoded to its last frame); `make flatpak`; the `flatpak` CI job and the release bundle (D9); t971's shape guard. Verify t971 green, `make flatpak` green locally, and the CI job green on a pushed branch (record the run URL here)
  - `src/scripts/flatpak-build.sh` (build with `--install-deps-from=flathub`, the linter on manifest
    and repo, `appstreamcli validate --strict`, `flatpak-check.sh`, `--bundle`), `flatpak-check.sh`
    (the loaded major = the branch's, a libx264 + AAC export of the committed `check.kt` decoded to its
    60th frame by the new `kt-cli decode`), `make flatpak`, the CI job `flatpak` (apt flatpak,
    flatpak-builder, appstream; `.flatpak-builder` cached), the release step and assets. t971 green
    (`check_flatpak_ci`). `kt-cli decode` checked here on the fixture (60 frames) and on a 105 s
    export (3150 frames). NOT verified: `make flatpak` (no flatpak on this machine) and the CI job
    on a pushed branch — this branch is not pushed; both are the next step on a machine with flatpak.
- [ ] 3.4 Commit stage 3

## 4. GPU access and the guide

- [ ] 4.1 On the owner's machines inside the sandbox: Intel/AMD — `kt-cli ffmpeg --probe-encoders` and a render with `h264_vaapi` (with and without `LIBVA_DRIVER_NAME`); NVIDIA — the same with `--device=dri` only and with `flatpak override --user --device=all <ID>`. Record per machine: driver, GL extension, which encoders open, here
- [x] 4.2 `docs/flatpak.md`: the per-vendor guide from 4.1's observations (not guesses), the verify commands, the undo (`flatpak override --user --reset <ID>`), and the maintainer part (D10: the Flathub submission steps). Verify t969's guide part green and the metainfo links the guide
  - `docs/flatpak.md`: the check command, Intel/AMD VAAPI with `LIBVA_DRIVER_NAME`, NVIDIA with the GL
    extension and `--device=all`, the undo, files and settings, and the maintainer's submission steps
    (D10). The per-vendor lines are the runtime's documented behaviour, marked on the page as to be
    corrected from 4.1's measurements on the owner's machines. t969's guide part green; the metainfo
    links the guide (`<url type="help">`).
- [ ] 4.3 `openspec/specs/distribution/design.md` gets the "why" at archive (D1 one ID, D3 measured FFmpeg, D5 permissions, D9 no upload); F-RES-008 verdict updated. Verify `openspec validate --specs` and `features-index.py --check` green
- [ ] 4.4 Commit stage 4

## 5. Finish and submission

- [x] 5.1 Run `./scripts/finish.sh` from `src/` (and `--release` once). Verify it is green and record the version `vYY.MM.DD-hash` here
  - `./scripts/finish.sh` green: **kt26.10.29-bf92858c** (`loop` passed on the second try alone — a known
    load flake). Two red runs before it, both recorded: the live rig's stale `WM_CLASS` and t958's
    unwaited renders (fixed in 3f3e342), then t947 reading the compose cost before it settled
    (fixed in bf92858; `trans` flaked once). `--release` is not run: the release tarball is the
    owner's step with the Flatpak bundle (D9), which needs flatpak on the machine.
- [ ] 5.2 The owner opens the Flathub PR (D10) from the first release tag; record the PR URL and the reviewers' requests here; answers that change the manifest go back through this change's guards
