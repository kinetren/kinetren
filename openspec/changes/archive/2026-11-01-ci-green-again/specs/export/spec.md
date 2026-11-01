## ADDED Requirements

### Requirement: A forced encoder outside the system's cascade is a named error
When an export forces a video encoder that this system's cascade never tries (`h264_vaapi` on
Windows, `h264_mf` on Linux), the export SHALL fail before writing a file, with an error naming
the encoder and the cascade it is not part of ("`h264_vaapi`: not an encoder this system tries
(h264_nvenc, …)"). Why: the loop over the cascade skipped every candidate and the error read
"no video encoder could be opened (last error: )" — on windows-latest t962, which forced VAAPI,
failed on that empty string.

#### Scenario: The other system's encoder is forced
- **WHEN** an export forces the encoder of the other system's cascade
- **THEN** it fails, the error names the encoder and lists this system's cascade, and no file is
  left behind
- **TEST** t1063 (`src/libs/kt-engine/tests/export_options.rs`)

#### Scenario: A forced VAAPI on a system without it
- **WHEN** t962 forces `h264_vaapi` on a system whose cascade has no VAAPI (Windows)
- **THEN** the error names that, and the device half of the test (which needs VAAPI in the
  cascade) does not apply; on Linux the device half runs as before
- **TEST** t962 (`src/libs/kt-engine/tests/export_options.rs`)

### Requirement: A frozen path never calls the platform's libm
Every transcendental function an audio or pixel kernel, a generator, a curve or the engine calls
on a path whose bytes are frozen by a reference SHALL come from `kt_plugin_api::math`
(`FrozenMath`: `fsin`, `fcos`, `fsin_cos`, `ftan`, `fexp`, `fpowf`, `flog10`, `fatan2`, `fasin`,
`fmul_add`), never from `f32`/`f64`'s methods, which call the platform's libm. The `f32` functions
are the `f64` ones of the pure-Rust `libm` crate rounded once to `f32` — correctly rounded except
on about 2⁻²⁸ of inputs — and every function stays within one ulp of glibc. The warp's sinc
table and dot product are ours as well (`kt-fx/src/audio/sinc.rs`): rubato's table on our math
and its AVX+FMA product as the one rounding, the portable path bit-equal. Exact functions
(`sqrt`, `floor`, `rem_euclid`, `powi`…) are not concerned. This is invariant 11 across platforms:
the same project gives the same audio bytes and CPU pixels on Linux and Windows and on any CPU.
Why: on 94e36e9e windows-latest failed t973c (slowed_reverb, phaser, pitch_shift), t1019 (flip)
and t1048 (smpte, leader) — the references were printed with glibc, and mingw/msvcrt round some
inputs differently. The owner (01.11): "own math, close to Linux, where Linux is the more
accurate one".

#### Scenario: The frozen math stays within one ulp of glibc
- **WHEN** t1065 sweeps 200 000 inputs of every function over the ranges the kernels use and
  beyond, on Linux with glibc
- **THEN** every result is within one ulp of glibc's; the share that differs is printed per
  function (the `f32` ones differ only where glibc itself is not correctly rounded)
- **TEST** t1065 (`src/libs/kt-plugin-api/tests/math.rs`)

#### Scenario: No frozen crate calls the platform's functions
- **WHEN** t1066 scans kt-time, kt-model, kt-plugin-api, kt-fx, kt-text, kt-media, kt-engine and
  every plugin family for `x.sin(`, `f64::powf(` and their kin outside comments (its own scanner
  first proven on lines it must and must not name)
- **THEN** it finds none
- **TEST** t1066 (`src/apps/kt-cli/tests/frozen_math.rs`)

#### Scenario: The warp's product does not depend on the CPU
- **WHEN** the warp's sinc product runs on the AVX path and on the portable path over the same
  table and wave
- **THEN** the results are equal bit for bit
- **TEST** t1067 (`src/libs/kt-fx/src/audio/sinc.rs`)

#### Scenario: The audio kinds' sound is the same on Linux and Windows
- **WHEN** t973c renders every audio kind's four cases on Linux and on windows-latest
- **THEN** both give the table's hashes; eight of them were re-based by this change and equal
  what windows-latest printed for them on 94e36e9e, no other value moved; t1019 and t1048 pass
  on both platforms unchanged
- **AND** the export reference t11 has one audio value for both platforms — the one windows-latest
  had measured — and its video value did not move
- **TEST** t973c (`src/libs/kt-engine/tests/plugin_sound.rs`), t11
  (`src/libs/kt-engine/tests/golden_export.rs`), t1019
  (`src/libs/kt-engine/tests/trans_plugin.rs`), t1048 (`src/libs/kt-media/tests/gen_plugin.rs`)
