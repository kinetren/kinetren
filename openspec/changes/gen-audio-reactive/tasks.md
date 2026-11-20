## 1. The series

- [x] 1.1 `kt_plugin_api::audio_series`: the type, its bytes, `frame_values` with the hold
- [x] 1.2 `kt_media::listen`: `analyze` from a source, `SeriesStore` with the sidecar behind it; t3714

## 2. The feed

- [x] 2.1 `GenState.feed`; `kt_engine::listen` registry; `gen_sources` builds the feed; `src_rev` and the memo carry it; t3715
- [x] 2.2 `GenKernel::listen` and `shader_frame_audio`; `GeneratorSource` hands the frame's values; t3716

## 3. The kinds

- [x] 3.1 `generators-reactive`: waveform, spectrum, level rings; registry, golden digests, menu rows; t3717
- [x] 3.2 the window: the Track row; catalogs ×5; t3718

## 4. The jobs

- [x] 4.1 kt-app background job; the render computes a missing series; kt-cli export; t3719
- [ ] 4.2 live scenario `gen_reactive`, idle check, What's new, registry row
