## 1. Storage

- [x] 1.1 `kt_media::segment`: the traits, `RawSegments`, `ffmpeg::FfmpegSegments` (hardware only, forced by `KT_RENDER_CACHE_ENCODER`) (D1, t2986)
- [x] 1.2 `DiskStore` of segments and manifests, eviction by segment, the reader thread (D1, D2, t2980)
- [x] 1.3 The prerender writes runs into segments; no encoder → the store writes nothing, `status()` says why

## 2. Hosts, trace, docs

- [x] 2.1 kt-app Diagnostics line (five catalogs), kt-app and kt-cli hand `FfmpegSegments::from_env()`
- [x] 2.2 `trace cache`: segments, encoders, no-encoder reason (t2987)
- [x] 2.3 The key documented for F-EXP-003 (D3)

## 3. Checks

- [x] 3.1 Measurements (design); live `render_cache` with the forced encoder
- [x] 3.2 Tests, clippy, target-prune; found-defects 775–776
