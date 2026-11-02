# Tasks: ci-red-again

Test numbers t1380–t1389 are reserved by this change.

## 1. t935 asks GIO

- [x] 1.1 t935 (`src/apps/kt-cli/tests/dist_guard.rs`): ask `gio info -a standard::content-type`
  (`GIO_USE_VFS=local`) about the three files; perl's `mimetype` only about `film.kt` and
  `demo.ktproj`; skip naming `libglib2.0-bin` without `gio`. Verify red first: on the old MIME
  setup the old t935 fails on this machine under a desktop-less xdg-mime (`env -i`), and the new
  t935 is green; the new one also runs in `ubuntu:22.04` and `ubuntu:24.04` containers (D1 table)
  - Done. Red: `env -i … xdg-mime query filetype Main.kt` on this machine answers our type (the
    CI failure, through perl `mimetype` 0.34), while under Cinnamon xdg-mime asks GIO — why the old
    t935 was green here. GIO answers right in `ubuntu:22.04` (2.72), `ubuntu:24.04` (2.80) and
    `fedora:41` (2.82); the new t935 is green here and in `ubuntu:24.04`
- [x] 1.2 `libglib2.0-bin` in `ci.yml`'s test job and the Dockerfile's base stage; the
  `RECOMMENDS_NEEDED` row `shared-mime-info` → `libglib2.0-bin`; the MIME file's comment names
  the oracle — t1380 (cases `ci-gio`, `container-gio`), t1060 green

## 2. Compose reads the icon

- [x] 2.1 `librsvg2-common` in the flatpak installs of `ci.yml` and `release.yml`; the row
  `flatpak-builder` → `librsvg2-common` — t1380 (cases `ci-rsvg`, `release-rsvg`)
- [x] 2.2 `src/scripts/appstream-compose-check.sh`: the manifest's install lines into a temporary
  folder, flatpak-builder's compose arguments, the hints on failure, exit 0 with a note without
  `appstreamcli compose`; `flatpak-build.sh` runs it first; `dist-check.py` `check_flatpak`
  requires it — t1380 (case `no-preflight`), t1381 (the tree composes, a broken icon fails; skips
  here). Verify in `ubuntu:24.04`: without `librsvg2-common` the script fails naming
  `file-read-error`, with it exits 0; t1381 runs green there
  - Done. The kt-cli test binary run in `ubuntu:24.04`: with the runner's packages of 9e1f1a45
    (no `librsvg2-common`, no `libglib2.0-bin`) t1381 is RED on `file-read-error …/com.kinetren.Kinetren.svg`
    and t935 says it did not run, naming `libglib2.0-bin`; with this change's packages both are
    green (t1381 composes for real). The script without `appstreamcli compose` (this machine) exits 0
    with its note. The whole `dist_guard` suite is green here (15 tests)

## 3. Close

- [x] 3.1 The distribution spec's scenarios and the design's "why" (archive); `cargo test -p kt-cli
  --test dist_guard`, `dist-check.py --check`, clippy
- [x] 3.2 `git merge master`, `src/scripts/finish.sh`, the version in the report
  - Done. master had not moved (9e1f1a45). finish.sh: `kt26.11.02-70b8c241`. The workspace tests ran
    1442 of 1443 green. t796c (kt-app) failed once in `fixtures/gen.sh`: `mv: cannot stat
    'out/.tmp.sine44k_mono.m4a'`, two parallel fixture generations sharing one temp name. It passes
    alone and has nothing to do with this change. The live `welcome` scenario passed on its second
    try: ui-probe hit an X `BadWindow` while walking a window that closed. Not done: the whole
    Flatpak job in a container. The local docker daemon stopped creating containers after a
    `--privileged` run hung, and a shared system service was not restarted
