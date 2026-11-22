## ADDED Requirements

### Requirement: A Linux aarch64 build with the same audio bytes as x86
The release SHALL carry a Linux aarch64 tarball built natively in the same container family with the same checks (no
libav in `ldd`, the glibc ceiling), and the Flatpak SHALL list aarch64; CI SHALL run the workspace tests on an ARM runner on
every push, and the audio golden digests there SHALL equal the x86 values byte for byte.
Why: the owner, 2026-11-04 — "п.5. да ARM нужен"; v26.10 M4; invariant 11.

#### Scenario: The ARM job proves the bytes
- **WHEN** the ARM CI job runs the audio goldens and the per-kind digests
- **THEN** every digest equals the committed x86 value; `KT_SIMD` reports "neon"
- **TEST** t3980 (`src/libs/kt-engine/tests/audio_fx_cost.rs`)

#### Scenario: The aarch64 tarball runs with the system FFmpeg
- **WHEN** the aarch64 tarball is unpacked on the ARM runner and `kt-cli` exports a test project
- **THEN** `ldd` names no libav library, FFmpeg is found by the search plan, the export's audio equals the x86 export's byte for byte
- **TEST** t3981 (`src/apps/kt-cli/tests/cross_render.rs`)

#### Scenario: A crash is reported on aarch64
- **WHEN** the test child crashes on purpose on the ARM runner
- **THEN** the raw report is written and symbolized with function names
- **TEST** t3982 (`src/libs/kt-crash/tests/handler.rs`)
