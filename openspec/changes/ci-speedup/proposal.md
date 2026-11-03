# Proposal: ci-speedup

## Why

The owner asked whether CI can be faster (`docs/user-feedback.md`, 2026-11-03). The run of master
9e1f1a45 (his log archive `logs_112101614406.zip`) takes ~13 min, and almost all of it is one
chain: `windows-build` compiles the 308 crates of the Windows test archive from scratch on every
push (6.4 of its 8.8 min) and only then `windows-test` starts (4.2 min). The other four jobs end at
~8.5 min. Inside the test jobs a few long tests run last on one core while the others idle.

## What Changes

Measured per step in that run (minutes):

| Job | Total | Where the time goes |
|---|---|---|
| windows-build | 8.8 | image `windows` stage 1.5 (rebuilt every run), `cargo nextest archive` 6.4 (cold) |
| windows-test | 4.2 | waits for windows-build; tests 3.4 (181 s, last: t1053 67 s, t943 52 s, t1171 42 s) |
| test | 8.3 | free disk 0.8, build 1.3, clippy 0.5, tests 5.0 (1.75 compile + 191 s; last: t1053 74 s, t943 48 s, t399 59 s) |
| ffmpeg9 | 8.0 | image 3.9 (cache miss: Dockerfile changed), build 2.7 (cold, 158 crates), tests 0.4 |
| flatpak | 8.5 | LTO `dist` build 6.5 (cache miss) |

1. **The Windows cross-build reuses a cache.** The container's target directory and crate registry
   are restored before `cargo nextest archive` and saved after it, keyed by the toolchain,
   `Cargo.lock` and the Dockerfile. Expected: 6.4 → ~2 min.
2. **The `windows` build image is cached** (`docker save`, keyed by the Dockerfile's hash), as the
   `ffmpeg-test` image already is — kept only if loading beats building (1.5 min).
3. **The FFmpeg 9 build reuses a cache** the same way (its container target directory).
4. **Every build cache is written only by the default branch**; branches and pull requests read
   it. Today only the `test` job follows this; `actions/cache` in `ffmpeg9` and `flatpak` saves
   from any branch, and branch caches eat the 10 GB repository cap.
5. **The long tests start first and use idle cores.** A committed nextest profile starts the
   tests measured as long first; t1053 and t943 spread their independent cases over threads
   inside the test (same cases, same assertions, same names). This also shortens the owner's
   local `finish.sh`, which runs nextest too.
6. **The disk clean-up of the `test` job no longer blocks it** (0.8 min of `rm -rf`, run in the
   background while the packages and toolchain install), kept only if it measures faster.

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

- `.github/workflows/ci.yml` (windows-build, ffmpeg9, flatpak, test jobs).
- `src/.config/nextest.toml` (new): the long-tests-first profile.
- `src/libs/kt-fx/tests/barrier_plugin.rs` (t1053), `src/libs/kt-fx/tests/grid_cost.rs` (t943):
  cases run on scoped threads; coverage and names unchanged.
- `src/scripts/dist-check.py`, `src/apps/kt-cli/tests/dist_guard.rs`: new t1420–t1422.
- No program code, no user-visible behaviour; no What's new line (internal CI work).
