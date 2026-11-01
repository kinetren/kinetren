# Tasks

## 1. Diagnosis and the owner's case

- [x] 1.1 Reproduce on the owner's project (scratch copy, paths repointed): `kt-cli export` and the real GUI render of the region `rend` on master finish and the process lives on; locate his build a04b201d before the fix e72ea7a8 (`git merge-base --is-ancestor`)
- [x] 1.2 t1312 (`layout_tests.rs`): a real render of a 0.2 s off-grid region at 24 fps through the render window, progress never past the total, 6 frames, Close, MP4; verified red on the old code (floored total + unclamped fraction reverted: "cannot convert float seconds to Duration: value is negative") and green on master
- [x] 1.3 `lessons.rs`: the joke timer's duration through `render_dialog::secs` (t1312 and the lessons tests stay green)
- [x] 1.4 Live scenario `src/scripts/live/renderregion.py` (our fixture): Render → finish → Close → the program draws, no crash window, no crash report, the file is an MP4 — green (`KT_LIVE_DISPLAY=:231 ./scripts/live-check.sh renderregion`)

## 2. Tests never touch the person's data directory

- [x] 2.1 `journal::data_dir()` in a test build → `KINETREN_DATA_DIR` or `<temp>/kt-test-data-<pid>`, dead folders swept; `ffmpeg_setup::download_root()` follows; the render log default is `data_dir()`; the eframe test compares the resolver — t1310 green
- [x] 2.2 Source guard t1311: no direct request of the person's directories in kt-app outside the allow-list — green
- [x] 2.3 The whole kt-app test binary runs without writing into `~/.local/share/kinetren` (its journal, locks, recovery and workspace mtimes unchanged by `cargo test -p kt-app`) — verified: a full `cargo test --workspace` left the directory and its 7934 locks byte-identical

## 3. Records and close

- [x] 3.1 What's new line in `docs/guide/whats-new.md` `## kt26.11`; registry row B1 → done with t1312, t1310, t1311; `docs/user-feedback.md` round note on the diagnosis
- [x] 3.2 `cargo test --workspace` and `cargo clippy --all-targets` green; commit; `src/scripts/target-prune.py` — the load-sensitive t815, t411, t476, t640, t795, t951, t952 went red under load 40–50 and pass alone; clippy adds no warning in the touched files
- [ ] 3.3 Merge master; `src/scripts/finish.sh` green, version in the report; archive the change (specs merged into export and durability, the why into their design.md)
