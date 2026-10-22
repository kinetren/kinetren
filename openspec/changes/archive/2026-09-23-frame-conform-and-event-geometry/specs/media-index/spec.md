# Spec Delta

## MODIFIED Requirements

### Requirement: Source contract — audio 48 kHz/2ch f32 by bursts, frame at exactly the requested quality
Any `MediaSource` SHALL deliver audio only as 48 kHz stereo f32 interleaved, exactly in
1024-frame bursts, where burst idx covers the samples [idx·1024, (idx+1)·1024) from the start of the stream;
format conversion happens INSIDE the source. A video frame SHALL be exactly the requested
Quality (the composite discards a foreign size); the requested Quality is computed by the layer's geometry
(capability `video-plan`), not by the consumer from the project's raster. `info()` SHALL report the
pixel aspect ratio (SAR, default 1:1) and the frame view = native × SAR; all conforming,
thumbnails and "Max (by sources)" are computed from the view. A source without sound SHALL answer
`has_audio: false` AND `Ok(silence)`, not `Err`. The parsing of pseudo-paths (`synth:`, `gen:`) SHALL
live only in the factory and be case-insensitive; a broken pseudo-path is an honest error, it does not go to
FFmpeg.
Why: no consumer should see the file's sample rate (a defect of the old engine —
hiss in slowed&reverb); an `Err` for "no sound" turned into false underruns through the worker's
negative cache; two parsings of `synth:` diverged by case; two files with identical pixels and a
different SAR gave byte-for-byte one frame — anamorphic was inexpressible.

#### Scenario: Sample-accurate bursts
- **WHEN** a source is read out of order and sequentially
- **THEN** the results match byte for byte
- **TEST** t10 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: The pixel aspect ratio reaches the kind of the frame
- **WHEN** two 640×480 files with identical pixels and SAR 1:1 / 2:1; a synthetic source with `sar`
- **THEN** `info()` gives different views (4:3 and 8:3), the conform frames differ, the synthetic source without FFmpeg carries the same field
- **TEST** t711 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`), t711b (`src/libs/kt-media/src/synthetic.rs`)

#### Scenario: The retune predicate matches the retune itself
- **WHEN** the factory answers whether the source can `retune`
- **THEN** the answer exactly matches the behavior of `MediaSource::retune` (a file — never, a generator — yes)
- **TEST** t499 (`src/libs/kt-media/src/factory.rs`)

#### Scenario: Test sources are honest about what they measure
- **WHEN** a test asserts "zero decodes" or "zoom sharpness"
- **THEN** "zero decodes" is measured by the counters of `SyntheticSource`, and sharpness — only on `CheckerSource` with a real native and box minification (the synthetic's native is fictitious)
- **TEST** t13 (`src/libs/kt-engine/tests/viewport.rs`), t315 (`src/libs/kt-engine/tests/decode_sharpness.rs`, capability `video-plan`)
