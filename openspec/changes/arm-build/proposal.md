# Proposal: arm-build

> DESIGN ONLY (spec session, 2026-11-22; owner's answer folded in 2026-11-23). Registry `v26.10-index.md` M4 (partial: AVX2 done). Note F-EXT-007. Tests
> reserved: t3980–t3982.

## Why

The owner, `docs/ktprompts/v26.10.txt` line 140, whole (item 4 of that list): "4. avx/simd для аудио-эффектов".

His answer of 2026-11-04 (`docs/user-feedback.md`, "2026-11-04 — the math is everywhere, not one effect"), item 5, whole:
"п.5. да ARM нужен".

**How this design reads the two**: line 140 asks the audio effects to use the CPU's vector units — done for x86 by
`audio-fx-cost` (every audio plugin's block compiled again with AVX2, the same bytes, t1981). The ARM answer asks for the
program on ARM machines; on aarch64 NEON is the baseline, so the audio fast path needs no second dispatch there — what is
missing is an ARM build at all. This design takes that: a Linux aarch64 release with the same audio bytes as x86.

## What Changes

- **A Linux aarch64 release**: the tarball and the Flatpak for aarch64, built in the same container family as the x86
  ones (an aarch64 stage of `packaging/Dockerfile`), with the same checks (no libav in `ldd`, the glibc ceiling).
- **CI on an ARM runner**: the workspace tests and the audio golden digests on an aarch64 runner on every push; the
  digests must equal x86's byte for byte (invariant 11).
- **The audio fast path on aarch64** is the baseline build (NEON); `fast!` compiles one path there; `KT_SIMD` reports
  "neon".
- **The GPU path**: Vulkan where the machine's driver has it, the software adapter otherwise — the same rule as x86.
- **Later, named**: Windows on ARM (the cross-toolchain and a runner), macOS on Apple silicon (a platform of its own).

## Capabilities

### Modified Capabilities

- `distribution`: the aarch64 build, its CI job, its release artifacts, the cross-architecture byte check.

## Impact

- `packaging/Dockerfile`, `src/scripts/release-build.sh --target linux-aarch64`, `.github/workflows/{ci,release}.yml`
  (an `ubuntu-24.04-arm` job), the Flatpak manifest's arch list.
- kt-fx `simd.rs`: the level name on aarch64; no new kernel code.
- No model, engine or format change.

## Questions to the owner

None open. **Decided** (`kkt-26.11.22.txt` line 8, D5, the answer to Q58): "Linux aarch64 (Recommended)" — Linux
aarch64 (tarball + Flatpak) now; Windows on ARM next; macOS a separate platform later.
