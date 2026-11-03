# Tasks: ci-speedup

Test numbers t1420–t1429 are reserved by this change. Every stage ends with a commit (English
message, no co-author lines) and `src/scripts/target-prune.py`. The baseline is run 9e1f1a45
(proposal.md table). Pushing to GitHub needs the owner's go-ahead.

## 1. Guards first

- [ ] 1.1 `dist-check.py`: the cache rule of D4 (no bare `actions/cache`, every `save` step and
  `rust-cache` writes only on the default branch) — t1420 with red copies for each form; the
  current tree is RED on `ffmpeg9` and `flatpak` before 2.x
- [ ] 1.2 `dist-check.py`: `windows-build` and `ffmpeg9` restore a target cache before compiling,
  every key names the toolchain and `Cargo.lock` (or the Dockerfile/manifest for an image or
  flatpak-builder) — t1421 with red copies
- [ ] 1.3 `dist-check.py`: every test named in `src/.config/nextest.toml` exists as a `#[test]` —
  t1422 with a red copy naming a missing test

## 2. Caches in `ci.yml`

- [ ] 2.1 D4: `ffmpeg9`'s image cache and `flatpak`'s builder cache split into restore + save on
  the default branch — t1420 green
- [ ] 2.2 D1: `windows-build` mounts a registry through `cargo-registry.sh` and restores/saves
  `src/target/win-container` (rust-cache on the container target, or the restore/save + prune
  fallback); the prune leaves no workspace-member artifact — t1421 green; locally, the same
  `docker run` twice with the restored directory recompiles only the workspace crates (count the
  `Compiling` lines)
- [ ] 2.3 D3: `ffmpeg9` restores/saves `target/ffmpeg-test-container` — t1421 green; locally,
  `ffmpeg-test.sh` twice: the second compiles only the workspace crates
- [ ] 2.4 D2: the `windows` image saved/loaded by the Dockerfile's hash; measure `docker load` of
  the saved image against the 1.5-min build here, keep only if it is faster (record both)
- [ ] 2.5 D6: the disk clean-up in the background, awaited before the build step

## 3. Test order

- [ ] 3.1 D5: `src/.config/nextest.toml` with the priority overrides; check the `priority` key on
  nextest 0.9.146 (`cargo nextest show-config test-groups`/a run's start order) — t1422 green;
  `cargo nextest run --workspace` here: the named tests start in the first wave
- [ ] 3.2 D5: t1053 and t943 run their cases on `std::thread::scope`; same cases, same messages;
  break one case on purpose and see the test fail naming it, then restore — t1053, t943 green;
  their wall time here before/after recorded

## 4. Measure and close

- [ ] 4.1 `cargo test -p kt-cli --test dist_guard`, `dist-check.py --check`, `cargo clippy
  --workspace --all-targets`, the whole `cargo nextest run --workspace` green; wall time against
  the run before 3.x recorded
- [ ] 4.2 With the owner's go-ahead: after the merge, a cold and a warm master run; per-job times
  against 9e1f1a45 recorded here; `gh cache list` sizes recorded and within D4's budget
- [ ] 4.3 Archive: the distribution design's "why" (the measured table, D1–D6, the rejected
  sccache/one-test-per-barrier/branch-skipping) — `openspec validate --strict`
- [ ] 4.4 `git merge master`, `src/scripts/finish.sh`, the version in the report
