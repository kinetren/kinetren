# Proposal: ci-speedup

## Why

The owner asked whether CI can be faster (`docs/user-feedback.md`, 2026-11-03), then set the stage:
"все что больше 10мин+ это ужас". The run of master 9e1f1a45 (his log archive
`logs_112101614406.zip`) takes 13.0 min, and almost all of it is one chain: `windows-build` compiles
the 308 crates of the Windows test archive from scratch on every push (6.6 of its 8.8 min) and only
then `windows-test` starts (4.1 min). The other jobs end at ~8.4 min, and the Flatpak job of that run
stopped early. Locally, a plugin edit cost 158 s of test build (row 31 of `docs/found-defects.md`).

## What Changes

The table of where each minute goes, and the per-job before and after, are in design.md.

1. **The Windows cross-build reuses a cache** of its third-party artifacts and registry, and the FFmpeg
   9 build the same way. What is saved is pruned of the workspace's own crates
   (`src/scripts/ci-cache-prune.py`).
2. **The Windows CI job compiles in a `windows-cross` stage** without wine, and that image is cached
   by the Dockerfile's hash, as `ffmpeg-test` already is.
3. **The Flatpak build keeps its third-party artifacts between runs** through `--ccache`'s directory.
   Only the CI job asks for it; the manifest's one line does nothing for Flathub and the release.
4. **Every build cache is written only by the default branch**; branches and pull requests read it.
5. **The long tests start first** (a committed nextest profile). The threads inside t1053 and t943
   that were proposed are dropped: measured, they could not help (design D5).
6. **The disk clean-up of the `test` job goes**: 108 GB free against 5 GB used.
7. **The container builds run without incremental compilation**, as the native job does.
8. **The dev profile keeps debug level 1** instead of full debug info: the local loop's plugin edit
   (row 31) waited on the disk, and the target directory halves.

Not changed: what CI checks. Skipping the Flatpak job on branches would save runner time but
changes what a push proves; left to the owner.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `distribution`: the CI requirement gains the cache rule (written only by the default branch,
  every compiling job restores one) and the committed test-order profile, each guarded by
  `dist-check.py`.

## Impact

- `.github/workflows/ci.yml` (all compiling jobs), `src/packaging/Dockerfile` (the `windows-cross`
  stage), `src/packaging/flatpak/com.kinetren.Kinetren.yml`, `src/scripts/flatpak-build.sh`,
  `src/scripts/ffmpeg-test.sh`.
- `src/scripts/ci-cache-prune.py` (new), `src/.config/nextest.toml` (new).
- `src/Cargo.toml`: `[profile.dev] debug = 1`; the first build of every worktree after the merge is a
  full one, once.
- `src/scripts/dist-check.py`, `src/apps/kt-cli/tests/dist_guard.rs`: new t1420-t1422.
- No program code, no user-visible behaviour; no What's new line (internal CI work).
