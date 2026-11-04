# Design: ci-speedup

## Context

See proposal.md for the measured run. The constraints that shape the choices:

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

## Goals / Non-Goals

**Goals:** the critical path from ~13 min to ~8–9 min; then the three ~8.5-min jobs shorter by
removing the long-test tail; no change to what is checked.

**Non-Goals:** skipping jobs on branches (the Flatpak job's 6.5-min LTO build stays on every
push — the owner's call); paid larger runners; changing the dev/test profile (`opt-level = 1`,
item 301) or the release `dist` profile.

## Decisions

### D1. The Windows cross-build restores its target directory and registry

`windows-build` mounts a registry through `cargo-registry.sh` (as `ffmpeg-test.sh` does) and
restores two paths before `cargo nextest archive`: `src/target/win-container` and that registry.
Key: `win-<RUST_TOOLCHAIN>-<hash(Cargo.lock)>-<hash(Dockerfile)>`, with restore-keys falling back
to the same toolchain and Dockerfile (dependencies that did not change stay compiled). Before
saving, the workspace crates' own artifacts are removed (they rebuild on every push anyway:
checkout resets mtimes), so the entry holds only third-party dependencies and stays bounded.

Preferred tool: `Swatinem/rust-cache` pointed at the container's target directory
(`workspaces: src -> target/win-container`) — it already does that pruning and the native job uses
it. It runs on the host, outside the container; if its cleaning cannot see the cross target
(`target/win-container/x86_64-pc-windows-gnu/debug`), fall back to `actions/cache/restore` +
`actions/cache/save` with a prune step that deletes the workspace members' `deps/*`,
`.fingerprint/*` and `build/*` entries by package name (`cargo metadata --no-deps`).

The container runs with `-u $(id -u):$(id -g)`, so restored files keep the runner user's
ownership and stay writable. Alternative rejected: `sccache` with the GitHub cache backend — a
second cache layer and a wrapper inside the container for what one directory cache does.

### D2. The `windows` image is saved and loaded like `ffmpeg-test`

The same pattern as the `ffmpeg9` job (`docker save | gzip -1`, keyed by the Dockerfile's hash).
It is kept only if `docker load` measures faster than the 1.5-min build: the image carries the
Rust toolchain and mingw and may be large. Alternative: buildx with `cache-to: type=gha` — the
layer cache would be shared by both stages, but it is a second mechanism next to the existing
`docker save` one; taken only if `docker save` does not pay.

### D3. The FFmpeg 9 build restores `target/ffmpeg-test-container`

As D1, for the 158 crates of `ffmpeg-test.sh` (2.7 min cold). The registry is already mounted.

### D4. Only the default branch writes a cache

Every `actions/cache` becomes a `restore` step plus a `save` step under
`if: github.ref == format('refs/heads/{0}', github.event.repository.default_branch)`;
`Swatinem/rust-cache` keeps its `save-if`. This also applies to the existing `ffmpeg9` image cache
and the `flatpak-builder` cache, which today save from any branch. Branch runs still read
master's entries (GitHub lets any branch read the default branch's caches). Guarded by t1420.

**Size.** Before merging, the executor lists the repository's caches (`gh cache list`, read-only)
and records the sizes of the new entries. If the sum of the latest default-branch entries exceeds
~8 GB, the Windows registry is left out of the cache first (it re-downloads in seconds), then the
image cache (D2).

### D5. The long tests start first, and two of them use idle cores

`src/.config/nextest.toml` gets `[[profile.default.overrides]]` with `priority` for the tests
measured as long in the run (t1053, t943, t399, t918, t923, t290, t1185, t1171). The default
profile is used, so `finish.sh` and CI both get it without a flag. The executor checks the
`priority` key on nextest 0.9.146 (the owner's) and on the version CI installs; if a version lacks
it, the overrides are not used and only the in-test threading below remains.

t1053 loops over every registered barrier and its value forms; t943 over sizes, grids and chains.
Each case is independent and pure (the CPU reference, no globals after `install()`). Their loops
run under `std::thread::scope`, one chunk per available core, the assertions unchanged, a failing
case still named in the panic message. They keep the registry-driven form (a new barrier is
checked without editing t1053). Rejected: splitting into one `#[test]` per barrier — the list
would have to be written out, which t1053 exists to avoid.

### D6. The disk clean-up runs in the background

`rm -rf` of the Android SDK, .NET etc. (0.8 min) starts detached and is awaited before the build
step, so it overlaps the package install, the toolchain and the cache restore. Kept only if the
job's time to the build step drops.

## Risks / Trade-offs

- [The cache entries exceed the 10 GB cap and evict each other] → D4's size check; prune the
  workspace crates (D1); drop the registry, then the image cache.
- [A stale restored target produces a wrong archive] → cargo's fingerprints decide every
  rebuild; the key changes with the toolchain, `Cargo.lock` and the Dockerfile. A cold run
  (deleted cache) must give a green run too — checked once.
- [The cache only pays after a master run writes it] → the measurement is a cold master run
  followed by a warm one (Migration).
- [Threads inside a test hide which case failed] → every assertion message names its case, as
  today; the scope joins and re-panics with the first failure.

## Migration Plan

1. The tree changes and guards land on the branch; `cargo test`, `dist-check.py --check` green.
2. Pushing the branch to GitHub and reading the runs are done only with the owner's go-ahead.
   After the merge, the first master run is cold (it writes the caches), the next push to master
   is warm; both runs' per-job times are recorded in tasks.md against 9e1f1a45.
3. Rollback: revert the workflow commit; the caches expire by themselves.
