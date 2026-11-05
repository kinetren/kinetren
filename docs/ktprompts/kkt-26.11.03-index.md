# Registry of the owner's list `kkt-26.11.03`

Source — [`kkt-26.11.03.txt`](kkt-26.11.03.txt) (verbatim, the owner's message of 03.11.2026: two
requests). Columns and states — as in [`v26.10-index.md`](v26.10-index.md). The round is in
`docs/user-feedback.md` ("2026-11-03 — the final render is slow; one way to accelerate the math").

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| C1 | 1-2 | the final render (export) is VERY slow, slower even than the preview, and looks not parallel but strongly sequential | done export-throughput, t1451 t1452 t1453 t1455 t1456 t1457 t1458 | `export-throughput`: the showcase 324 s (llvmpipe) / 861 s (CPU) → 95.6 s (stages concurrent, the encoder threaded, two decode defects); before that it crashed in 3 s (t1450) | export-throughput |
| C2 | 4-5 | "the math" is separate in every place and not accelerated; one mechanism to accelerate computations on whatever the machine has (AVX, CUDA, OpenCL, Vulkan) — something like compiled shaders, or another approach altogether | partial: the owner chose to delete the CPU and AVX copies (2026-11-04, "да, удаляем"), the video and audio references may go; stage 1 done, t1502 t1503 t1504 t1505: the Datamosh encoder runs as a Vulkan compute shader, its CPU text deleted; stage 2 done, t1570–t1575 (`compute-kernels-render`, the owner, "удаляй"): the CPU composite and every CPU and SIMD copy of a video kernel deleted, the picture has one executor, a software adapter renders on several devices; generators and the other math without shaders are next (F-EXT-004) → plugins | `compute-backends` | the next math round |
