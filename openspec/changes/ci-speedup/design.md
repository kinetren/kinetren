# Design: ci-speedup

## Context

The owner, stage 2: "все что больше 10мин+ это ужас". The measured run and the per-job table are
below. The constraints that shape the choices:

- The owner's decisions in `openspec/specs/distribution/design.md`: free plan, nothing uploaded
  that lingers (a cache is not an upload), one Dockerfile, the Windows build cross-compiled in
  its `windows` stage, CI tests natively on ubuntu-24.04.
- The Actions cache holds 10 GB per repository and evicts the least recently used. The native
  `test` job already writes a `Swatinem/rust-cache` entry from master only.
- `ci-green-again` handed the container a crate registry through `src/scripts/cargo-registry.sh`
  (the host's `~/.cargo/registry`, or a volume owned by the container's user). `windows-build`
  does not mount one today; `ffmpeg9` does, through `ffmpeg-test.sh`.
- The owner's rule (2026-11-01): caches are fine where they pay, keyed by what decides the
  result, bounded, and invisible to determinism. A test build's bytes do not depend on the cache.
- The owner sends CI logs after a push and nobody pushes from a session, so everything below
  marked "estimated" is an estimate with its basis, not a result.

## Where the minutes go

The only log archive is `logs_112101614406.zip`, run of master 9e1f1a45, 2026-11-01 23:54:28 to
2026-11-02 00:07:29 UTC: **13.0 min of wall time**. The "21 min" of `docs/orchestrator.md` and row 32
of `docs/found-defects.md` (windows-build 13, windows-test 7) has no log behind it; nothing here is
measured against it. If such a run exists its archive tells whether the runners were slower or the
caches colder: the same scale (1.6×) on the "after" column below would land above 10 min.

That run's Flatpak job stopped at `appstreamcli compose` (fixed since by `ci-red-again` and
`ci-flatpak-screenshots`), so **no green Flatpak run has a log**: it is longer than the 8.4 min
below by the mirror, two linter runs, the linter's install and the sandbox check.

Per job, in seconds, from the step marks of the run:

| Job | Total | Steps |
|---|---|---|
| windows-build | 528 | packages + fixtures 20, image 91 (rebuilt every run), `cargo nextest archive` 397 (compile 381, all 308 crates; pack 16), upload 17 |
| windows-test | 248 | waits for windows-build; checkout + nextest 15, download 23, FFmpeg 7, unpack 20, tests 181 |
| test | 492 | disk clean-up 48, packages 20, toolchain + nextest + cache 26, build 79, clippy 28, tests 297 (compile 105, run 191) |
| ffmpeg9 | 476 | packages + fixtures 16, image 263 (cache miss: Dockerfile changed), tests 191 (compile 160, run 24) |
| flatpak | 505+ | packages 11, runtime and SDK install 87, compile 393, stopped at compose |

Three things found on the way that the proposal did not have:

- **The runner's disk is not short.** After the clean-up `/` had 108 GB free out of 145 GB and the
  job used 5 GB (`src/target` 2.7 GB with `CARGO_PROFILE_DEV_DEBUG=0`). The clean-up was 48 s of
  nothing, so it goes rather than moving to the background (D6).
- **The test tail is small.** Sum of test times over four cores: Linux 651 s / 4 = 163 s against
  191 s; Windows 698 s / 4 = 174 s against 181 s. No ordering or threading can go below that, so the
  order is worth at most 28 s and 7 s (D5).
- **The container builds ran with incremental on**: `CARGO_INCREMENTAL=0` is job-level env and
  `docker run` does not pass it. An incremental cache that is thrown away is pure cost.

## Before and after

Measured locally with the cross-build's own command, four jobs, CPU taken from the container's
cgroup (`cpu.stat`), because the machine ran under other sessions' load of 20 to 200 and wall time
was noise:

| Build | Cold | Warm (third-party restored, workspace rebuilt) | Warm / cold |
|---|---|---|---|
| `cargo nextest archive`, Windows | 906 CPU-s, 308 crates | 405-424 CPU-s, 32 crates | 0.46 |
| `cargo build --profile dist` | 846 CPU-s, 384 crates | 511 CPU-s, 32 crates | 0.60 |

The pruned Windows target is 1.19 GB, 343 MB compressed (saved in 8 s); the `dist` one 1.55 GB,
456 MB. The `windows-cross` image is 3.0 GB against 5.5 GB with wine, 684 MB as `docker save | gzip
-1`. What CI spent is scaled by the ratio: **estimated**, per job, on a warm run (caches exist, the
Dockerfile and `Cargo.lock` unchanged):

| Job | Before, min | After, min (estimated) | Basis |
|---|---|---|---|
| windows-build | 8.8 | ~4.7 | compile 381 s × 0.46 = 175 s; image 91 s → ~35 s (684 MB at the 150 MB/s of the run's cache restore, plus `docker load`); target restore ~10 s |
| windows-test | 4.1 | ~4.0 | order: 7 s at most; nothing else changes |
| **Windows chain** | **12.9** | **~8.7** | the critical path |
| test | 8.2 | ~7.0 | clean-up -48 s; order up to -28 s |
| ffmpeg9 | 7.9 | ~2.8, 6.7 when the Dockerfile changes | image hit ~45 s against 263 s; compile 160 s × 0.46 = 74 s |
| flatpak | 8.4 to compose, green unknown | to compose ~5.8; green FLATPAK_GREEN | compile 393 s × 0.60 = 236 s |
| **wall** | **13.0** | **~9** | the longest path |

The margin under 10 min is thin. A cold run stays what it was plus the saves: the first master run
after this change (the Dockerfile changed), and any later Dockerfile or toolchain change, takes about
what 9e1f1a45 took. A `Cargo.lock` change falls back to the previous entry and compiles what moved.

## Goals / Non-Goals

**Goals:** CI wall under 10 min on a warm run; no change to what is checked: the same tests run.
The local loop: the plugin edit and `finish.sh`.

**Non-Goals:** skipping jobs on branches (the Flatpak job stays on every push: the owner's call);
paid larger runners; changing `opt-level` (item 301) or the release `dist` profile.

## Decisions

### D1. The Windows cross-build restores its target directory and registry

`windows-build` restores `src/target/win-container` and `~/.cargo/registry` before `cargo nextest
archive` and mounts that registry (`cargo-registry.sh` prints `~/.cargo/registry` when it exists).
Key: toolchain, Dockerfile hash, `Cargo.lock` hash; restore-keys fall back to the same toolchain and
Dockerfile. `registry/src` stays in the entry: a build script with `rerun-if-changed` on a file there
reruns if unpacking gave the file a new mtime.

Before saving, `src/scripts/ci-cache-prune.py` removes every workspace member's artifacts, by the
names `cargo metadata --no-deps` gives. Rust-cache was the plan's first choice and is rejected for
this job: it needs a host toolchain that matches, and its cleaning could not be run here, while the
prune script was run on the real target: the warm build compiled exactly the 32 workspace crates and
no third-party one. Rejected too: `sccache` with the GitHub backend, a second cache layer and a
wrapper inside the container for what one directory does.

The entry can only cost a compile, never a stale result: cargo's fingerprints decide. Old crates stay
in it after a `Cargo.lock` change until the Dockerfile or toolchain changes; that growth is small and
a new key clears it.

### D2. The `windows-cross` image, saved and loaded like `ffmpeg-test`

The CI job runs no Windows binary, so it does not need wine, which is 2.5 GB of the 5.5 GB image and
the slow half of its build. The `windows` stage becomes `windows-cross` plus wine; `make windows`,
the release and `windows-test.sh` stay on `windows`, the CI job compiles in `windows-cross`, and that
image is saved like `ffmpeg-test` (684 MB).

### D3. The FFmpeg 9 build restores `target/ffmpeg-test-container`

As D1, with `ffmpeg-test.sh`'s own registry mount. The prune and save run under `always()`: a red
test must not cost the next run its compile.

### D4. Only the default branch writes a cache

Every `actions/cache` is a `restore` step plus a `save` step under the default-branch condition, and
`Swatinem/rust-cache` keeps its `save-if`. Branch runs read master's entries. Guarded by t1420;
t1421 guards that every compiling job restores before it compiles and what a key names.

**Size**: estimated, nothing was read from GitHub: test 0.6 GB (measured in the log), Windows target
0.35 + registry ~0.2, image 0.7, FFmpeg image and target ~1, Flatpak ~1: about 4 GB of the 10 GB.

### D5. The long tests start first; no threads inside tests

`src/.config/nextest.toml` gives `priority` to the tests measured as long on either runner, in the
default profile, so `finish.sh` and CI both get it. The proposal also spread t1053 and t943 over
threads. **Dropped**: the four cores of a runner are already busy (the bound above), so threads
inside a test take from the tests next to it; and locally t1053 and t943 are not among the long tests
at all (the longest there wait: t1171, t1087, t1088, t935).

### D6. The disk clean-up goes

See above: 108 GB free against 5 GB used. "Disk usage" stays, under `always()`, so a change is seen.

### D7. The Flatpak build keeps third-party artifacts between runs

After D1 the Flatpak job is the likely longest. Inside the sandbox only `--ccache`'s directory
survives (`/run/ccache`, kept in `.flatpak-builder/ccache`). `flatpak-build.sh` passes `--ccache`
when `KT_FLATPAK_CCACHE` is set, which only the CI job does: the release builds from nothing. The
manifest gets one line that links `src/target` into that directory when `CCACHE_DIR` is set, and does
nothing otherwise, so Flathub's build is unchanged. The prune runs before the save, as in D1.

Rejected: lowering `lto` or `codegen-units` for CI. That would be 2 to 3 min, but the job is the only
CI build of the `dist` profile, so it would stop checking what ships. A cache of the installed
runtime and SDK (87 s) is also left out: at ~3 GB it is the largest entry for the smallest saving.

### D8. The dev profile keeps debug level 1 (row 31 of `docs/found-defects.md`)

A plugin edit recompiles the family, the assembler and every test target of the crates that have the
assembler as a dev-dependency: cargo has no early cut-off, and `tests/*.rs` are one binary each, 120
of them. CI builds ALL of them on four cores in 105 s; the workshop took 158 s on 32 cores for a part.
The difference is debug info: `CARGO_PROFILE_DEV_DEBUG=0` there, full here. In the workshop the link
waited on the disk: 240 s of wall for 367 CPU-s, `ld.mold` in uninterruptible sleep.

`debug = 1` is what the release profiles use. A/B on one tree, one after the other:

LOCAL_AB

`"line-tables-only"` was tried first and t1072 refused it: the crash report loses the function's
module path. What `debug = 1` gives up: variables and types in a debugger. Rejected, for now: merging
each crate's `tests/*.rs` into one binary. It would remove most of the cascade, but `cargo test` then
runs all of a crate's tests in one process, and tests that set the environment for their binary
(`KT_SIMD=0`) stop being isolated: a change of its own, for the owner to want.

### `finish.sh`

Stages over five runs on 2026-11-01 to 11-04, seconds: build 2-106, tests 38-745, SIMD 42-114, live
1082-4049. The live scenarios are the bulk and are not touched here: 72 scenarios, 3269 s in sequence
(run of 2026-11-04), four workers, so 817 s at best, and one of them, `readme_shots`, takes 593 s.
The workers' number is load (one kt-app in software rendering holds about seven cores), and nothing
here measured whether more would stay green on a quiet machine. The test stage gets the D8 effect.

## Risks / Trade-offs

- [The first master run is cold and writes the caches] → the measurement is a cold master run and a
  warm one after it (Migration).
- [A stale restored target gives a wrong archive] → cargo's fingerprints decide every rebuild.
- [The estimate holds only for runners like 9e1f1a45's] → the wall table is an estimate; the first
  warm log replaces it.
- [`--ccache` is not what Flathub runs] → the release and Flathub build without it; CI proves the
  same manifest, whose one extra line is a no-op there.

## Migration Plan

1. The tree changes land on the branch; `cargo test`, `dist-check.py --check` green.
2. Nobody pushes from a session. After the merge the first master run is cold, the next push warm;
   the owner's two log archives are compared with this table.
3. Rollback: revert the workflow commit; the caches expire by themselves.
