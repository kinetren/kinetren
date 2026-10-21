# Tasks

## 1. Format and import

- [x] 1.1 `kt-model::project_file`: `MARKER = "kinetren"`, `FORMER_MARKER = "kinotrain"`, `FileKind::FormerName` with an error naming Kinotrain and "Import older Kinetren project…"; English error strings say Kinetren; verify by t828 (`src/libs/kt-otio/tests/convert.rs`) and the existing project_file tests updated
- [x] 1.2 Import door accepts a former-marker `.kt` (key renamed, normal read) in kt-otio, the app and `kt-cli import`; verify by t828, t829 (`src/apps/kt-cli/tests/cli.rs`)
- [x] 1.3 Commit stage 1

## 2. Data directory and locks

- [x] 2.1 `journal::data_dir()` resolves `kinetren`, moving the former `kinotrain` directory once (rename, else copy); the recovery path and `kt-cli` lock path use the same directory; verify by t830 (`src/apps/kt-app/src/journal.rs`), t831 (`src/apps/kt-cli/tests/cli.rs`)
- [x] 2.2 Check where eframe persistence lives with the new app id. If it is outside the data dir, migrate it the same way. Verify with a live start under a temporary `XDG_DATA_HOME` that holds a former workspace
- [x] 2.3 Commit stage 2

## 3. Program and packaging names

- [x] 3.1 Window title, app id, file-dialog filter names, journal header, i18n catalogs (5 languages; "Import older Kinetren project…"), kt-cli messages, `kt-text` sample text; verify with `font_tests.rs`/`i18n_tests.rs`, `reach_tests.rs` (t818–t821, t783 texts) and t832 (former-name project through the UI)
- [x] 3.2 `src/packaging`: rename `kinotrain.desktop`/`.svg`/`-mime.xml`/icons and the app asset; MIME `application/x-kinetren`; `install.sh` removes the former files; `release-build.sh` tarball `kinetren-<ver>-linux-<arch>`; verify `./scripts/release-build.sh` checks (ldd, GLIBC) still pass, or at least `bash -n` plus a dry run of `install.sh` into a temporary prefix
- [x] 3.3 Guard t833: no `Kinotrain`/`kinotrain` in `src/apps`, `src/libs`, `src/packaging`, the catalogs and `src/scripts` outside an allow-list (FORMER_MARKER, the import repair, data-dir migration, their tests)
- [x] 3.4 Live scenarios and scripts that name the program (`kt_live.py`, `worker.py`, `ui-probe.py` window lookup, `readme_shots.py`, `import_old.py`) updated; `./scripts/live-check.sh` green
- [x] 3.5 Commit stage 3

## 4. Docs and site

- [x] 4.1 Root `CLAUDE.md`, crate `CLAUDE.md` notes, `openspec/config.yaml`, live spec texts that quote UI strings (via this change's deltas at archive), `docs/architecture.md`, `docs/file-format.md`: the program is Kinetren; `docs/porting-notes.md` and quotes stay. Verify with `grep -rn Kinotrain` reviewed by hand
- [x] 4.2 `site/`: content catalogs, structure, the logo file; `python3 site/build.py --check` green after `python3 site/build.py`; README icon path updated to the renamed SVG; `readme-check.py --check` green
- [x] 4.3 Commit stage 4

## 5. Finish

- [x] 5.1 `cargo test --workspace`, `cargo clippy --all-targets`, `src/scripts/finish.sh` green; report the version
