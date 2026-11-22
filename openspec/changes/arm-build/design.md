# Design: arm-build

> DESIGN ONLY, revision 1 (2026-11-22), base `3d66d630`. Q1 in `proposal.md`; the default is recommended.

## Context

- Releases: Linux x86-64 tarball (Ubuntu 22.04 container, glibc ≥ 2.35, no libav in `ldd`) and the Windows zip
  (mingw cross-build), Flatpak (spec `distribution`).
- The audio fast path: `kt_plugin_api::fast!` compiles a block twice on x86-64 (baseline and AVX2 with a runtime
  choice), same bytes (no FMA, no reordered sums, exact rounding); `KT_SIMD=0` forces the baseline (spec plugins, t1981).
- Determinism: audio byte for byte across machines; frozen math for transcendental functions (t1066).

## D1. The build

An aarch64 stage in `packaging/Dockerfile` (Ubuntu 22.04 arm64 base, the same packages), `release-build.sh --target
linux-aarch64` → `kinetren-<version>-linux-aarch64.tar.gz`; the same `ldd` and GLIBC checks. Built natively on an ARM
runner (no QEMU: too slow for a full build).

## D2. Byte equality across architectures

On aarch64 `fast!` emits one path (NEON is in the baseline target features; no FMA feature enabled: rustc does not
contract `a*b+c` without it). The golden audio digests (t2/t3 and t1981's per-kind digests) run on the ARM job and must
equal the x86 values committed in the tests. A difference is a defect to find (an FMA, a libm call), never a second
golden.

## D3. CI and release

`ci.yml`: an `ubuntu-24.04-arm` job runs `cargo test --workspace` (the synthetic and the audio goldens; GPU tests on the
software adapter). `release.yml`: the aarch64 tarball beside the x86 one; Flathub builds aarch64 from the manifest's
arch list.

## D4. Class walk — "what differs on another architecture"

| Thing | Risk | Guard |
|---|---|---|
| audio kernels | FMA, libm, sum order | t3980 digests equal x86 |
| video | GPU driver differences | `FrameTol` (as across x86 devices) |
| FFmpeg dlopen | SONAMEs | the same search plan; t3981 on the ARM runner |
| the crash handler (kt-crash) | stack unwinding per arch | t3982 symbolizes a test crash on aarch64 |
| fonts, i18n | none | — |

## Rejected

- **A NEON intrinsics path** — the baseline already has NEON; an intrinsics path would be a second text to keep equal.
- **QEMU cross-builds** — hours per build; native ARM runners exist.
