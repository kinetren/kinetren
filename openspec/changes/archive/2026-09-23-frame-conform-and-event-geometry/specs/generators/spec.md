# Spec Delta

## MODIFIED Requirements

### Requirement: A generator frame is a pure function of exactly the requested quality
`GeneratorSource` SHALL deliver a frame of EXACTLY the requested Quality (the composite silently
discards a foreign size), deterministically from (state, pts, Quality) — two
runs byte for byte. "No sound" SHALL be expressed as `has_audio:false` AND
`Ok(silence)`, never `Err`. `info()` SHALL report duration=0 ("no duration"),
fps=None (the container's grid) and a native equal to the raster of the CONTAINER (the project or the
reel in which the event sits), SAR 1:1 — so the generator is conformed identically and a
title in a square reel is computed in its raster; the margin of detail for the decode step is given by the
step formula (native ≥ Quality → step by the chain's zoom). Pseudo-paths do not take part in
"Max (by sources)". The frame cache by Quality SHALL hold several qualities without double copying and be disabled only for kinds that depend on time, and for
animated (with curves) states. The 3×5 pixel font (placeholder, timecode, countdown)
SHALL be a shared module and is not a rasterization of user text.
Why: `Err` on "no sound" accumulated false underruns in the worker's negative cache; a native
4096×4096 with the conform rule would mean "as is" at 4096 and fitting a
square into 16:9 — the generator must have the container's proportions.

#### Scenario: Determinism and the `info()` contract
- **WHEN** the same generator is rendered twice at one Quality and `info()` is queried in a 1920×1080 project
- **THEN** the bytes match, the size is exactly Quality, duration=0, fps=None, native 1920×1080
- **TEST** t471 (`src/libs/kt-media/src/generator.rs`)

#### Scenario: Silence instead of an error
- **WHEN** an audio burst is read from a generator without sound
- **THEN** `Ok` with silence is returned, `has_audio` is false
- **TEST** t472 (`src/libs/kt-media/src/generator.rs`)

#### Scenario: The native of the generator feeds the decode step
- **WHEN** a zooming chain sits above a title
- **THEN** S×Quality is ordered by the step formula from the container's native; without FX — exactly Quality
- **TEST** t484b (`src/libs/kt-engine/tests/decode_scale.rs`)

#### Scenario: A title in the reel of its own raster
- **WHEN** a title sits in a 1080×1080 reel, the reel is in a 1920×1080 project
- **THEN** the title is computed at 1080×1080 (native = the reel's raster), the instance is fitted with margins, the bytes are deterministic
- **TEST** t726 (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: Cache by quality
- **WHEN** one frame is requested repeatedly at several Qualities
- **THEN** each quality is rendered once; for an animated title the cache is bypassed, a static one does not suffer from the appearance of an animation
- **TEST** t507, t539 (`src/libs/kt-media/src/generator.rs`)

#### Scenario: The placeholder and the common pixel font are deterministic
- **WHEN** the placeholder frame for a missing file is rendered with a caption, including non-ASCII
- **THEN** the bytes are repeatable and do not depend on time; non-ASCII gives "?", the sound is silence
- **TEST** t208 (`src/libs/kt-media/src/placeholder.rs`)
