---
id: F-EXT-007
title: ARM builds — the program on aarch64 machines, with the same bytes
status: candidate
priority: B
cost: M
category: plugins/kernels
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-fx, kt-gpu, kt-ffmpeg, kt-app]
invariants: [10, 11]
ui_rules: []
round: null
point: null
tests: []
depends_on: [F-EXT-004]
checked: 2026-11-22
---

# F-EXT-007 — ARM builds — the program on aarch64 machines, with the same bytes

## Essence

ARM machines are common now: Apple silicon, Snapdragon laptops, ARM Linux boards and servers. The owner answered on
2026-11-04: "п.5. да ARM нужен". We ship Linux x86-64 (tarball, Flatpak) and Windows x86-64 only.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| DaVinci Resolve | 21 | native Apple silicon; Windows on ARM [NOT VERIFIED] | macOS universal; Linux x86-64 only [NOT VERIFIED live] |
| Premiere | 26 | native Apple silicon; Windows on ARM (beta) [NOT VERIFIED] | — |
| Final Cut Pro | 12 | Apple silicon native | — |
| After Effects | 26 | Apple silicon native | — |
| VEGAS Pro | 2026 | Windows x86-64 only [NOT VERIFIED] | — |

## The trick behind it

**1. The bytes must not move between x86 and ARM.** Audio is byte for byte across machines (invariant 11); the risks are
fused multiply-add (a compiler contracting a·b+c differently), library math (sin/exp from the platform's libm) and
SIMD paths that reorder sums. We already guard against all three: no FMA in the fast paths, the frozen math
(`kt_plugin_api::math`), no reordered sums.

**2. NEON is the baseline on aarch64.** The AVX2 dispatch has no ARM twin to write: every aarch64 CPU has NEON, so the
baseline build already vectorizes.

## Pros

- One more class of machines with no code fork; Flathub builds aarch64 itself.

## Cons

- CI time: a second architecture in the release and its tests.

## What users say

> п.5. да ARM нужен
> — the owner, answering the `compute-backends` memo, 2026-11-04 (`docs/user-feedback.md`, section "2026-11-04 — the
> math is everywhere, not one effect")

The registry item it serves: `docs/ktprompts/v26.10.txt` line 140 (M4): "avx/simd для аудио-эффектов".

NO QUOTES: users' requests for ARM builds of editors were not searched in this session (the editor rows are [NOT
VERIFIED] for the same reason); the search would be the Resolve and Premiere forums for "Windows on ARM" and "aarch64
Linux". The owner's own words above are the reason for the change.

## How it fits Kinotrain

### What already exists

- The audio fast path compiled twice with an AVX2 runtime choice (`src/libs/kt-fx/src/simd.rs`, spec plugins).
- The release scripts for Linux x86-64 and Windows (`src/scripts/release-build.sh`, `packaging/Dockerfile`).

### What is missing

- An aarch64 Linux build, its CI, the cross-architecture byte check, an aarch64 Flatpak.

### What hinders

- FFmpeg dlopen paths and SONAMEs are the same on aarch64 distributions; the GPU path needs a Vulkan driver or the
  software adapter.

### Which invariants it touches

- **10** — no libav link on aarch64 either.
- **11** — audio byte for byte across x86 and ARM.
- CAPABILITY: openspec/specs/distribution/

## Estimate

| What | Answer |
|---|---|
| Scope | M |
| Where | build scripts, CI, kt-fx (the dispatch), kt-gpu (adapter choice) |
| Project format | none |
| Tests | the audio golden digests equal on aarch64; the release tarball runs on an aarch64 runner |
| Depends on | F-EXT-004 (compute backends) |
| Breaks compatibility | no |

## Verdict

Take it as a distribution change: a Linux aarch64 tarball and Flatpak built in the same container family, tests on an ARM
CI runner, the audio digests equal to x86's; Windows on ARM and macOS later.

## Sources

- `docs/user-feedback.md` (2026-11-04, item 5); `docs/ktprompts/v26.10.txt` line 140 (M4).
