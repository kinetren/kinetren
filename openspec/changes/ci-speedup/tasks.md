# Tasks: ci-speedup

Test numbers t1420–t1429 are reserved by this change. Every stage ends with a commit (English
message, no co-author lines) and `src/scripts/target-prune.py`. The baseline is run 9e1f1a45
(proposal.md table). Pushing to GitHub needs the owner's go-ahead.

## 1. Guards first

- [x] 1.1 `dist-check.py`: the cache rule of D4 — t1420 with red copies for each form
- [x] 1.2 `dist-check.py`: every compiling job restores before it compiles, every key names what
  decides its content — t1421 with red copies
- [x] 1.3 `dist-check.py`: every test named in `src/.config/nextest.toml` exists — t1422

## 2. Caches in `ci.yml`

- [x] 2.1 D4: `ffmpeg9`'s image cache and `flatpak`'s builder cache split into restore + save on the
  default branch — t1420 green
- [x] 2.2 D1: `windows-build` restores/saves `src/target/win-container` and the registry, the prune
  leaves no workspace-member artifact. Run here twice: the second compiled the 32 workspace crates
  only, 405-424 against 906 CPU-s; on four pinned cores 99 s against 234 s of compile
- [x] 2.3 D3: `ffmpeg9` restores/saves `target/ffmpeg-test-container`
- [x] 2.4 D2: the `windows-cross` stage, its image saved/loaded by the Dockerfile's hash; 3.0 GB
  against 5.5 GB, 653 MB saved, `docker load` 29 s
- [x] 2.5 D6: the disk clean-up removed
- [x] 2.6 `CARGO_INCREMENTAL=0` reaches the container builds

## 3. Test order

- [x] 3.1 D5: `src/.config/nextest.toml` with the priority overrides, read by nextest 0.9.146 — t1422
- [x] 3.2 D5: threads inside t1053 and t943 dropped, with the measured reason

## 4. The local loop

- [x] 4.1 D8: `debug = 1` in the dev profile (`line-tables-only` refused by t1072); A/B recorded
- [x] 4.2 `finish.sh`: stages read from `finish-times.txt`, recorded in the design

## 5. Flatpak

- [x] 5.1 D7: `--ccache` in the CI job only, the manifest's no-op line, the prune before the save

## 6. Measure and close

- [x] 6.1 `cargo test -p kt-cli --test dist_guard`, `dist-check.py --check`, `cargo clippy --workspace
  --all-targets` (no errors, 33 warning lines as before), `cargo nextest run --workspace`: 1528 of
  1528. t815 went red twice under load, green alone: a wall-clock bound on a disk write, not this
  change (row 28 of `docs/found-defects.md`)
- [x] 6.2 A real `flatpak-build.sh` with `KT_FLATPAK_CCACHE=1`, cold and warm: the warm run green, 32
  crates compiled, no third-party one
- [x] 6.3 Archive: `openspec validate --strict`

What is left is not the tree's: after the merge, a cold and a warm master run give the per-job times
to set against the design's table, and the sizes against D4's budget. The owner sends those logs.
