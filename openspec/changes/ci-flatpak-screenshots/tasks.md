# Tasks: ci-flatpak-screenshots

Test numbers t1410–t1419 are reserved by this change.

## 1. The screenshots reach the catalogue and the repository

- [x] 1.1 Reproduce the cause: in `ubuntu:24.04` with the CI packages of 21505ad2, compose the
  manifest's install lines with and without `--mirror-screenshots-url`; read flatpak-builder-lint's
  `checks/screenshots.py` and flatpak-builder 1.4.2/1.4.5/1.4.6/1.4.8 sources
  - Done. Both compose runs say `Success!` and keep no screenshot (four `screenshot-save-error`,
    "Unrecognized image file format"). With `webp-pixbuf-loader` and the mirror: four screenshots,
    13 PNGs, partial URLs. The URLs answer 200 `image/webp`. `--compose-url-policy` first appears
    in 1.4.6. The screenshots ref commit is in 1.4.5 NEWS. Ubuntu 24.04 has 1.4.2, and
    `ppa:flatpak/stable` has 1.4.8 for noble (design D1, D2)
- [x] 1.2 `appstream-compose-check.sh` composes with the mirror arguments and checks every
  screenshot (count, media URL, stored PNG); `flatpak-build.sh` refuses flatpak-builder < 1.4.6
  and builds with `--mirror-screenshots-url=https://dl.flathub.org/media --compose-url-policy=full`;
  `ci.yml` and `release.yml` add `ppa:flatpak/stable` and `webp-pixbuf-loader`
- [x] 1.3 `dist-check.py`: the `RECOMMENDS_NEEDED` row, the mirror pieces and the PPA — t1410
  (8 red copies); on the tree of master the guard reports all 10 violations. t1411 and the
  reworked t1381 (shared `compose_check`/`compose_can_run`)
  - Done. The kt-cli `dist_guard` binary in `ubuntu:24.04`: with the packages of 21505ad2, t1381
    and t1411 are RED ("0 of 4 screenshots reached the catalogue"); with `webp-pixbuf-loader`
    both are green. Here (no `appstreamcli compose`) both say they did not run; the whole suite
    is green (17 tests)
- [x] 1.4 t850 (`reach_tests.rs`) consumes its `ffmpeg` binding on Windows (no new warning)

## 2. Close

- [ ] 2.1 The distribution spec's scenarios and the design's "why" (archive); `cargo test
  --workspace`, clippy, `dist-check.py --check`
- [ ] 2.2 One real local run of `src/scripts/flatpak-build.sh` in a privileged `ubuntu:24.04`
  container with the CI job's steps (the PPA, the packages), ending green: the pre-flight, the
  build, the install, both linter runs, `appstreamcli validate --strict`, `flatpak-check.sh`
- [ ] 2.3 `src/scripts/finish.sh`, the version in the report
