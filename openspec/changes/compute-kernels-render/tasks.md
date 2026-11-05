## 1. Compute host and Datamosh

- [x] 1.1 Measure the first frame: no disk cache (D1)
- [x] 1.2 The host's buffer pool, bounded and clean (t1570, t1571)
- [x] 1.3 `decode.wgsl`, `predict` and `reconstruct` deleted (t1572)

## 2. A software adapter in parallel

- [x] 2.1 `export_engines`, a device per frame worker on a software adapter (t1573)
- [x] 2.2 Measure the render on the demo and the showcase (D4)

## 3. The composite on one executor

- [x] 3.1 The GPU tests hold to stored frames of the CPU composite (D5)
- [x] 3.2 Serials of frames are unique across caches (t1574)
- [x] 3.3 Preview, export, reel strips, posters and effect thumbnails compose on the GPU; the CPU
      composite, `compose_ahead`, `video_simd` and `mix_into` deleted (t1575)
- [x] 3.4 kt-engine's tests compose through `kt_gpu::testkit`
