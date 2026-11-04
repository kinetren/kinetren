# Registry of the owner's list `kkt-26.11.03`

Source — [`kkt-26.11.03.txt`](kkt-26.11.03.txt) (verbatim, the owner's message of 03.11.2026: two
requests). Columns and states — as in [`v26.10-index.md`](v26.10-index.md). The round is in
`docs/user-feedback.md` ("2026-11-03 — the final render is slow; one way to accelerate the math").

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
<<<<<<< HEAD
| C1 | 1-2 | the final render (export) is VERY slow, slower even than the preview, and looks not parallel but strongly sequential | no: defect, measured before the fix → export, video-plan | `export-throughput` | queue 1 (next free executor) |
| C2 | 4-5 | "the math" is separate in every place and not accelerated; one mechanism to accelerate computations on whatever the machine has (AVX, CUDA, OpenCL, Vulkan) — something like compiled shaders, or another approach altogether | question: the owner allows removing or updating the video and audio references (2026-11-04), and a machine with no graphics card runs the kernels through Vulkan on LLVM; delete the CPU and AVX copies of the moved effects, leaving Windows with no driver and Linux with no mesa-vulkan-drivers without them? (F-EXT-004: Datamosh encoder as compute, 34 ms against 67 ms on software Vulkan, 46.01 dB both) → plugins | `compute-backends` | the owner's answer, then the migration round |
=======
| C1 | 1-2 | the final render (export) is VERY slow, slower even than the preview, and looks not parallel but strongly sequential | done export-throughput, t1451 t1452 t1453 t1455 t1456 t1457 t1458 | `export-throughput`: the showcase 324 s (llvmpipe) / 861 s (CPU) → 95.6 s (stages concurrent, the encoder threaded, two decode defects); before that it crashed in 3 s (t1450) | export-throughput |
| C2 | 4-5 | "the math" is separate in every place and not accelerated; one mechanism to accelerate computations on whatever the machine has (AVX, CUDA, OpenCL, Vulkan) — something like compiled shaders, or another approach altogether | no: architecture, discussed with the owner first → plugins, video-fx, audio-fx | `compute-backends` | queue 2: reconnaissance note + one-effect prototype, then the owner decides the migration |
>>>>>>> orch-26-11-01
