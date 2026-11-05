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

## 4. The kernels

- [x] 4.1 `tap`, `frame`, `pixel`, the barrier context and the SIMD twins leave the plugin contract and
      every plugin; `warp` stays, the planner measures zoom through it (D7)
- [x] 4.2 kt-fx keeps the planner, `fit_into`, the mask rasterizer and the audio; its CPU executors go
- [x] 4.3 The physics tests of VHS and the camera read test-only models (D7)
- [x] 4.4 CI installs `mesa-vulkan-drivers` and requires a device on Linux; tests that read the picture
      skip without one

## 5. The merge with export-pipeline-determinism

- [x] 5.1 t1601, t1602, t1605 run on the GPU path; t1601 runs on six devices, the path of a software
      adapter, red without the hold, green with it
- [x] 5.2 A stage's own error wins over the "stopped" it caused downstream
